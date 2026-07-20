import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const root = resolve(scriptDirectory, "..");
const fixtureRoot = join(root, "integration-tests", "public-api-smoke");
const fixtureManifest = join(fixtureRoot, "Cargo.toml");
const fixtureSource = join(fixtureRoot, "src");
const fixtureTheme = join(fixtureRoot, "pliego.theme.toml");
const fixtureResolver = join(fixtureRoot, "product.resolver.json");
const fixedStyle = "flex gap-4 md:grid";
const resolverSelection = [
  "--tokens",
  fixtureResolver,
  "--token-input",
  "appearance=dark",
  "--token-input",
  "channel=light",
];

function fail(message) {
  throw new Error(message);
}

function run(command, args, { cwd = root, env = process.env, expect = 0 } = {}) {
  const result = spawnSync(command, args, {
    cwd,
    env,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.error) {
    fail(`cannot run ${command}: ${result.error.message}`);
  }
  if (result.status !== expect) {
    fail(
      `${command} ${args.join(" ")} exited ${result.status}; expected ${expect}\n${result.stdout}${result.stderr}`,
    );
  }
  return result;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function assertIdentityVersions(document, schema, role) {
  if (
    document.schemaVersion !== schema ||
    document.styleIdFormatVersion !== 2 ||
    document.classNameFormatVersion !== 1 ||
    document.themeIdFormatVersion !== 2
  ) {
    fail(`${role} does not expose schema ${schema} with identity formats 2/1/2`);
  }
}

function removeTemporaryWorkspace(path) {
  const temporaryRoot = resolve(tmpdir());
  const candidate = resolve(path);
  const child = relative(temporaryRoot, candidate);
  if (
    !child ||
    child.startsWith("..") ||
    isAbsolute(child) ||
    !basename(candidate).startsWith("pliego-public-api-")
  ) {
    fail(`refusing to remove unsafe temporary path: ${candidate}`);
  }
  rmSync(candidate, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
}

const rustcVersion = run("rustc", ["+1.96.0", "-vV"]).stdout;
const hostMatch = /^host:\s+(\S+)$/mu.exec(rustcVersion);
if (!hostMatch) {
  fail(`cannot determine the Rust 1.96 host triple from ${JSON.stringify(rustcVersion)}`);
}
const hostTriple = hostMatch[1];
const cliBuild = run("cargo", [
  "+1.96.0",
  "build",
  "--release",
  "--locked",
  "--target",
  hostTriple,
  "--message-format=json-render-diagnostics",
  "-p",
  "pliego-cssc",
]);
let executable;
for (const line of cliBuild.stdout.split(/\r?\n/u).filter(Boolean)) {
  let message;
  try {
    message = JSON.parse(line);
  } catch (error) {
    fail(`Cargo emitted a non-JSON build message: ${JSON.stringify(line)} (${error.message})`);
  }
  if (
    message.reason === "compiler-artifact" &&
    message.target?.name === "pliego-cssc" &&
    message.target.kind?.includes("bin") &&
    typeof message.executable === "string"
  ) {
    executable = resolve(message.executable);
  }
}
if (!executable) {
  fail("Cargo did not report the pliego-cssc executable it built");
}

const fixtureTarget = join(root, "target", "public-api-smoke");
const fixtureRun = run(
  "cargo",
  [
    "+1.85.0",
    "run",
    "--release",
    "--quiet",
    "--locked",
    "--target",
    hostTriple,
    "--manifest-path",
    fixtureManifest,
  ],
  { env: { ...process.env, CARGO_TARGET_DIR: fixtureTarget } },
);
const publicLine = fixtureRun.stdout.trim();
const publicMatch = /^([0-9a-f]{32})\t(pc_[0-9a-z]+)$/u.exec(publicLine);
if (!publicMatch) {
  fail(`public API fixture produced an unexpected line: ${JSON.stringify(publicLine)}`);
}
const [, rustStyleId, rustClassName] = publicMatch;

const workspace = mkdtempSync(join(tmpdir(), "pliego-public-api-"));
try {
  const cssPath = join(workspace, "app.css");
  const manifestPath = join(workspace, "app.manifest.json");
  run(
    executable,
    [
      "compile",
      "--source",
      fixtureSource,
      ...resolverSelection,
      "--theme",
      "--output",
      cssPath,
      "--manifest",
      manifestPath,
    ],
    { cwd: workspace },
  );

  const css = readFileSync(cssPath);
  const cssText = css.toString("utf8");
  const manifestBytes = readFileSync(manifestPath);
  const manifest = JSON.parse(manifestBytes.toString("utf8"));
  assertIdentityVersions(manifest, 3, "manifest");
  if (manifest.cssBytes !== css.byteLength || manifest.cssSha256 !== sha256(css)) {
    fail("manifest CSS integrity fields do not bind the emitted bytes");
  }
  const fixedEntry = manifest.styles.find((style) =>
    style.origins.some((origin) => origin.source === fixedStyle),
  );
  if (!fixedEntry) {
    fail("manifest does not contain the public fixture style");
  }
  if (fixedEntry.styleId !== rustStyleId || fixedEntry.className !== rustClassName) {
    fail("Rust macro identity does not match the standalone CLI manifest");
  }
  if (!cssText.includes(`.${rustClassName}{`)) {
    fail("emitted CSS does not contain the Rust macro class selector");
  }
  if (
    !cssText.includes("gap:1rem") ||
    !cssText.includes("@media (width>=48rem)") ||
    !cssText.includes("--color-brand:color(srgb .1 .1 .1)")
  ) {
    fail("selected DTCG registry did not reach the emitted CSS");
  }

  const check = run(
    executable,
    ["check", "--source", fixtureSource, ...resolverSelection],
    { cwd: workspace },
  );
  if (!check.stdout.startsWith("ok:")) {
    fail("check command did not return its stable success prefix");
  }

  const inspection = JSON.parse(
    run(
      executable,
      ["inspect", "--source", fixtureSource, ...resolverSelection],
      { cwd: workspace },
    ).stdout,
  );
  assertIdentityVersions(inspection, 2, "inspection");

  const catalog = JSON.parse(
    run(executable, ["catalog", "--config", fixtureTheme, "--format", "json"], {
      cwd: workspace,
    }).stdout,
  );
  assertIdentityVersions(catalog, 3, "catalog");

  const explanation = JSON.parse(
    run(
      executable,
      ["explain", "--style", fixedStyle, "--config", fixtureTheme, "--format", "json"],
      { cwd: workspace },
    ).stdout,
  );
  assertIdentityVersions(explanation, 2, "explanation");
  if (explanation.styleId !== rustStyleId || explanation.className !== rustClassName) {
    fail("explain identity does not match the Rust macro identity");
  }

  const duplicate = run(
    executable,
    ["check", "--style", "flex", "--seed", "--seed"],
    { cwd: workspace, expect: 1 },
  );
  if (!duplicate.stderr.includes("may only be provided once") || duplicate.stdout !== "") {
    fail("duplicate single-value CLI option did not fail closed on standard error");
  }

  process.stdout.write(
    `${JSON.stringify(
      {
        status: "ok",
        rust: "1.85",
        styleId: rustStyleId,
        className: rustClassName,
        themeId: manifest.themeId,
        cssBytes: css.byteLength,
        cssSha256: sha256(css),
        manifestSchema: manifest.schemaVersion,
        inspectionSchema: inspection.schemaVersion,
        catalogSchema: catalog.schemaVersion,
        explainSchema: explanation.schemaVersion,
        identityFormats: [2, 1, 2],
        renamedFacadeDependency: true,
        dtcgBuildMacro: true,
        tomlRegistryConvergence: true,
        resolverInputs: ["appearance=dark", "channel=light"],
        standaloneCwd: true,
      },
      null,
      2,
    )}\n`,
  );
} finally {
  removeTemporaryWorkspace(workspace);
}
