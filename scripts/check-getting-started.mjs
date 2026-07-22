import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { cargoTargetRoot, isolatedCargoEnvironment } from "./rust-target.mjs";

const ROOT = resolve(import.meta.dirname, "..");
const cargoEnvironment = isolatedCargoEnvironment(ROOT, {
  env: process.env,
  toolchain: "1.85.0",
});
const TARGET_ROOT = cargoTargetRoot(ROOT, cargoEnvironment);
const EXECUTABLE_SUFFIX = process.platform === "win32" ? ".exe" : "";
const BASIC_EXECUTABLE = join(TARGET_ROOT, "debug", `pliego-css-basic-example${EXECUTABLE_SUFFIX}`);
const CLI_EXECUTABLE = join(TARGET_ROOT, "debug", `pliego-cssc${EXECUTABLE_SUFFIX}`);
const STYLE =
  "flex flex-col gap-4 rounded-lg border border-line bg-surface p-6 " +
  "md:flex-row md:items-center hover:bg-surface-raised";

function fail(message, detail = undefined) {
  throw new Error(detail === undefined ? message : `${message}\n${JSON.stringify(detail, null, 2)}`);
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: ROOT,
    env: cargoEnvironment,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    fail(`Command failed: ${command} ${args.join(" ")}`, {
      exitCode: result.status,
      stderr: result.stderr,
      stdout: result.stdout,
    });
  }
  return result.stdout;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function documentedRust() {
  const markdown = readFileSync(
    join(ROOT, "docs", "getting-started", "first-compile.md"),
    "utf8",
  );
  const match = markdown.match(
    /<!-- docs-smoke:basic-rust:start -->\s*```rust\r?\n([\s\S]*?)\r?\n```\s*<!-- docs-smoke:basic-rust:end -->/u,
  );
  if (!match) fail("First compile must contain one marked Rust example");
  return match[1].replaceAll("\r\n", "\n").trim();
}

function fixtureRust() {
  const source = readFileSync(join(ROOT, "examples", "basic", "src", "main.rs"), "utf8")
    .replaceAll("\r\n", "\n");
  return source.replace(/^\/\/![^\n]*\n\s*/u, "").trim();
}

const documented = documentedRust();
const fixture = fixtureRust();
if (documented !== fixture) {
  fail("The first-compile Rust snippet drifted from examples/basic/src/main.rs");
}

let outputRoot;
try {
  outputRoot = mkdtempSync(join(tmpdir(), "pliegocss-getting-started-"));
  run("cargo", [
    "+1.85.0",
    "build",
    "--quiet",
    "--locked",
    "-p",
    "pliego-css-basic-example",
    "-p",
    "pliego-cssc",
  ]);
  const className = run(BASIC_EXECUTABLE, []).trim();
  if (!/^pc_[a-z0-9]+$/u.test(className)) fail("The basic example returned an invalid class", className);

  const sourceCssPath = join(outputRoot, "source.css");
  const sourceManifestPath = join(outputRoot, "source.manifest.json");
  run(CLI_EXECUTABLE, [
    "compile",
    "--source",
    "examples/basic/src",
    "--seed",
    "--theme",
    "--output",
    sourceCssPath,
    "--manifest",
    sourceManifestPath,
  ]);

  const literalCssPath = join(outputRoot, "literal.css");
  run(CLI_EXECUTABLE, [
    "compile",
    "--style",
    STYLE,
    "--seed",
    "--theme",
    "--output",
    literalCssPath,
  ]);

  const css = readFileSync(sourceCssPath);
  const literalCss = readFileSync(literalCssPath);
  const manifest = JSON.parse(readFileSync(sourceManifestPath, "utf8"));
  const style = manifest.styles?.[0];
  const passed =
    manifest.schemaVersion === 3 &&
    manifest.styles?.length === 1 &&
    style?.className === className &&
    style?.origins?.length === 1 &&
    style?.origins[0].source === STYLE &&
    css.equals(literalCss) &&
    css.includes(Buffer.from(`.${className}{`, "utf8")) &&
    css.includes(Buffer.from(":root{", "utf8")) &&
    manifest.cssBytes === css.length &&
    manifest.cssSha256 === sha256(css);
  const report = {
    schema: "pliegocss/getting-started-gate/1",
    passed,
    rust: { msrv: "1.85.0", package: "pliego-css-basic-example" },
    className,
    artifact: {
      cssBytes: css.length,
      cssSha256: sha256(css),
      manifestSchemaVersion: manifest.schemaVersion,
      sourceAndLiteralCssEqual: css.equals(literalCss),
    },
  };
  if (!passed) fail("Getting-started example did not converge", report);
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
} finally {
  if (outputRoot) rmSync(outputRoot, { force: true, recursive: true });
}
