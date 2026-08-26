import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { isAbsolute, join, resolve, sep } from "node:path";
import { runTests } from "@vscode/test-electron";

const EXTENSION = resolve(import.meta.dirname, "..");
const ROOT = resolve(EXTENSION, "..", "..");
const configuredBase = process.env.PLIEGOCSS_TARGET_BASE ?? process.env.CARGO_TARGET_DIR;
const targetBase = configuredBase
  ? (isAbsolute(configuredBase) ? configuredBase : resolve(ROOT, configuredBase))
  : resolve(ROOT, "target");
const rustc = spawnSync("rustup", ["run", "1.85.0", "rustc", "-Vv"], {
  encoding: "utf8",
  windowsHide: true,
});
if (rustc.error || rustc.status !== 0) {
  throw new Error(`cannot identify Rust 1.85.0: ${rustc.error?.message ?? rustc.stderr}`);
}
const identity = rustc.stdout.replaceAll("\r\n", "\n").trim();
const release = identity.match(/^release:\s*(.+)$/mu)?.[1] ?? "unknown";
const host = identity.match(/^host:\s*(.+)$/mu)?.[1] ?? `${process.platform}-${process.arch}`;
/** @param {string} value */
const safeSegment = (value) =>
  value.toLowerCase().replaceAll(/[^a-z0-9._-]+/gu, "-").replaceAll(/^-+|-+$/gu, "");
const targetSegment = `${safeSegment(host)}-rustc-${safeSegment(release)}-${createHash("sha256").update(identity).digest("hex").slice(0, 16)}`;
const target = process.env.PLIEGOCSS_ISOLATED_TARGET === "1" && process.env.CARGO_TARGET_DIR
  ? (isAbsolute(process.env.CARGO_TARGET_DIR)
      ? process.env.CARGO_TARGET_DIR
      : resolve(ROOT, process.env.CARGO_TARGET_DIR))
  : join(targetBase, "toolchains", targetSegment);
const workspace = resolve(target, "lsp-integration-workspace");
const executable = process.platform === "win32" ? ".exe" : "";
const lsp = resolve(target, "debug", `pliego-css-lsp${executable}`);

/** @param {string} message */
function fail(message) {
  throw new Error(message);
}

if (!workspace.startsWith(`${target}${sep}`)) fail("unsafe VS Code host fixture path");
const fixture = spawnSync(process.execPath, [resolve(ROOT, "scripts", "check-lsp-integration.mjs")], {
  cwd: ROOT,
  encoding: "utf8",
  env: {
    ...process.env,
    CARGO_TARGET_DIR: target,
    PLIEGOCSS_ISOLATED_TARGET: "1",
    PLIEGOCSS_KEEP_LSP_FIXTURE: "1",
    PLIEGOCSS_TARGET_BASE: targetBase,
  },
  stdio: ["ignore", "pipe", "pipe"],
});
if (fixture.error) fail(`cannot prepare VS Code host fixture: ${fixture.error.message}`);
if (fixture.status !== 0) fail(`${fixture.stdout}${fixture.stderr}`.trim());

mkdirSync(resolve(workspace, ".vscode"), { recursive: true });
writeFileSync(
  resolve(workspace, ".vscode", "settings.json"),
  `${JSON.stringify(
    {
      "pliegocss.server.path": lsp,
      "pliegocss.theme.mode": "seed",
      "pliegocss.projectIndex.path": "out/pliego.index.json",
    },
    null,
    2,
  )}\n`,
);

try {
  await runTests({
    version: "1.105.1",
    extensionDevelopmentPath: EXTENSION,
    extensionTestsPath: resolve(EXTENSION, "tests", "host", "index.js"),
    launchArgs: [
      workspace,
      "--disable-extensions",
      "--disable-workspace-trust",
      "--skip-welcome",
      "--skip-release-notes",
    ],
    extensionTestsEnv: {
      PLIEGOCSS_HOST_WORKSPACE: workspace,
    },
  });
} finally {
  rmSync(workspace, { recursive: true, force: true });
}

process.stdout.write(
  `${JSON.stringify({
    schemaVersion: 1,
    vscode: "1.105.1",
    extension: "celiums.pliegocss-vscode",
    diagnostics: "shared-engine-pcs-and-pcx",
    definition: "project-index-to-physical-css",
    serverDownload: false,
  }, null, 2)}\n`,
);
