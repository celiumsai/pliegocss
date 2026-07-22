import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, relative, resolve, sep } from "node:path";
import { cargoTargetRoot, isolatedCargoEnvironment } from "./rust-target.mjs";

const ROOT = resolve(import.meta.dirname, "..");
const EDITOR = resolve(ROOT, "editors", "neovim");
const VERSION = "0.12.4";
const WINDOWS = {
  asset: "nvim-win64.zip",
  sha256: "9fc3572829ffd13debb6e32555da2c8cc02555568260a9fc4cf1f65bbcca319c",
  executable: ["nvim-win64", "bin", "nvim.exe"],
};
const LINUX = {
  asset: "nvim-linux-x86_64.tar.gz",
  sha256: "012bf3fcac5ade43914df3f174668bf64d05e049a4f032a388c027b1ebd78628",
  executable: ["nvim-linux-x86_64", "bin", "nvim"],
};
const platform =
  process.platform === "win32" ? WINDOWS : process.platform === "linux" ? LINUX : null;

function fail(message) {
  throw new Error(message);
}

if (!platform || process.arch !== "x64") {
  fail(`unsupported Neovim gate host ${process.platform}/${process.arch}`);
}
const expectedFiles = ["README.md", "lua/pliegocss/init.lua", "tests/host/init.lua"];
function filesBelow(directory) {
  return readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.name !== ".nvim-test")
    .flatMap((entry) => {
      const path = resolve(directory, entry.name);
      return entry.isDirectory() ? filesBelow(path) : [relative(EDITOR, path).replaceAll("\\", "/")];
    });
}
const files = filesBelow(EDITOR).sort();
if (JSON.stringify(files) !== JSON.stringify(expectedFiles)) {
  fail(`unexpected Neovim client payload: ${files.join(", ")}`);
}
const modulePath = resolve(EDITOR, "lua", "pliegocss", "init.lua");
const moduleText = readFileSync(modulePath, "utf8");
if (/https?:|download|curl|fetch/i.test(moduleText)) fail("Neovim client contains a download surface");
const clientBytes = [resolve(EDITOR, "README.md"), modulePath]
  .map((path) => statSync(path).size)
  .reduce((sum, size) => sum + size, 0);
if (clientBytes > 16 * 1024) fail(`Neovim client payload exceeds 16 KiB: ${clientBytes}`);
const cacheRoot = resolve(EDITOR, ".nvim-test");
const cache = resolve(cacheRoot, VERSION);
const archive = resolve(cache, platform.asset);
const extracted = resolve(cache, "runtime");
const executable = resolve(extracted, ...platform.executable);
for (const path of [cache, archive, extracted, executable]) {
  if (!path.startsWith(`${cacheRoot}${sep}`)) fail(`unsafe cache path: ${path}`);
}

function digest(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

mkdirSync(cache, { recursive: true });
if (!existsSync(archive) || digest(archive) !== platform.sha256) {
  rmSync(archive, { force: true });
  const url = `https://github.com/neovim/neovim/releases/download/v${VERSION}/${platform.asset}`;
  const response = await fetch(url);
  if (!response.ok) fail(`cannot download ${url}: HTTP ${response.status}`);
  writeFileSync(archive, Buffer.from(await response.arrayBuffer()));
  if (digest(archive) !== platform.sha256) {
    rmSync(archive, { force: true });
    fail(`Neovim ${VERSION} archive digest mismatch`);
  }
}
if (!existsSync(executable)) {
  rmSync(extracted, { recursive: true, force: true });
  mkdirSync(extracted, { recursive: true });
  const extraction = spawnSync(
    "tar",
    [process.platform === "win32" ? "-xf" : "-xzf", archive, "-C", extracted],
    { encoding: "utf8" },
  );
  if (extraction.error || extraction.status !== 0) {
    fail(
      `cannot extract ${basename(archive)}: ${extraction.error?.message ?? extraction.stderr}`,
    );
  }
}
if (!existsSync(executable)) fail(`Neovim executable is missing after extraction: ${executable}`);

const cargoEnvironment = isolatedCargoEnvironment(ROOT, {
  env: process.env,
  toolchain: "1.85",
});
const target = cargoTargetRoot(ROOT, cargoEnvironment);
const workspace = resolve(target, "lsp-integration-workspace");
const output = resolve(target, "neovim-host-result.json");
const binarySuffix = process.platform === "win32" ? ".exe" : "";
const lsp = resolve(target, "debug", `pliego-css-lsp${binarySuffix}`);
for (const path of [workspace, output]) {
  if (!path.startsWith(`${target}${sep}`)) fail(`unsafe fixture path: ${path}`);
}
const fixture = spawnSync(
  process.execPath,
  [resolve(ROOT, "scripts", "check-lsp-integration.mjs")],
  {
    cwd: ROOT,
    encoding: "utf8",
    env: { ...cargoEnvironment, PLIEGOCSS_KEEP_LSP_FIXTURE: "1" },
  },
);
if (fixture.error || fixture.status !== 0) {
  fail(`${fixture.error?.message ?? ""}${fixture.stdout}${fixture.stderr}`.trim());
}

try {
  rmSync(output, { force: true });
  const host = spawnSync(
    executable,
    ["--headless", "--clean", "-u", resolve(EDITOR, "tests", "host", "init.lua")],
    {
      cwd: workspace,
      encoding: "utf8",
      env: {
        ...process.env,
        PLIEGOCSS_NEOVIM_EDITOR: EDITOR,
        PLIEGOCSS_NEOVIM_WORKSPACE: workspace,
        PLIEGOCSS_NEOVIM_OUTPUT: output,
        PLIEGOCSS_NEOVIM_LSP: lsp,
      },
      timeout: 60_000,
    },
  );
  if (host.error || host.status !== 0) {
    fail(`${host.error?.message ?? ""}${host.stdout}${host.stderr}`.trim());
  }
  if (!existsSync(output)) fail("Neovim host did not write its result");
  const result = JSON.parse(readFileSync(output, "utf8"));
  if (
    result.schemaVersion !== 1 ||
    result.diagnostics?.pcs !== "PCS001" ||
    result.diagnostics?.pcx !== "PCX003"
  ) {
    fail("Neovim host result does not satisfy the closed gate");
  }
  process.stdout.write(
    `${JSON.stringify(
      {
        ...result,
        clientFiles: expectedFiles.slice(0, 2),
        clientBytes,
        runtimeArchive: platform.asset,
        runtimeSha256: platform.sha256,
      },
      null,
      2,
    )}\n`,
  );
} finally {
  rmSync(workspace, { recursive: true, force: true });
  rmSync(output, { force: true });
}
