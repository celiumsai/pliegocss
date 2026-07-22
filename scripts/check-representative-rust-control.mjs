import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { cargoTargetRoot, isolatedCargoEnvironment } from "./rust-target.mjs";

const root = resolve(import.meta.dirname, "..");
const fixture = join(root, "integration-tests", "representative", "rust-typed-control");
const cli = join(cargoTargetRoot(root), "debug", process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc");
const msrvEnvironment = isolatedCargoEnvironment(root, {
  env: process.env,
  toolchain: "1.85.0",
});
function fail(message, detail) { throw new Error(detail === undefined ? message : `${message}\n${JSON.stringify(detail, null, 2)}`); }
function run(command, args, cwd = root) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8", windowsHide: true });
  if (result.error) throw result.error;
  if (result.status !== 0) fail(`command failed: ${command} ${args.join(" ")}`, result);
  return result;
}
function sha256(bytes) { return createHash("sha256").update(bytes).digest("hex"); }
for (const name of ["Cargo.toml", "src/main.rs"]) if (!existsSync(join(fixture, name))) fail(`missing ${name}`);
const msrv = spawnSync(
  "cargo",
  ["+1.85.0", "check", "--quiet", "--locked", "--manifest-path", join(fixture, "Cargo.toml")],
  { cwd: root, env: msrvEnvironment, encoding: "utf8", windowsHide: true },
);
if (msrv.error) throw msrv.error;
if (msrv.status !== 0) fail("MSRV fixture check failed", msrv);
run("cargo", ["build", "--quiet", "--locked", "-p", "pliego-cssc"]);
let output;
try {
  output = mkdtempSync(join(tmpdir(), "pliegocss-rust-control-"));
  const css = join(output, "app.css");
  const manifest = join(output, "app.manifest.json");
  const args = [
    "compile", "--source", "src", "--seed", "--theme", "--targets", "modern",
    "--output", css, "--manifest", manifest, "--manifest-version", "3", "--control-dir", output,
  ];
  run(cli, args, fixture);
  const before = Object.fromEntries([
    "app.css", "app.css.map", "app.manifest.json", "pliego.tokens.json", "pliego.css.findings.json",
    "pliego.css.manifest.json", "pliego.css.receipt.json",
  ].map((name) => {
    const path = join(output, name); if (!existsSync(path)) fail(`missing controlled artifact ${name}`);
    const bytes = readFileSync(path); return [name, { bytes: bytes.length, sha256: sha256(bytes) }];
  }));
  run(cli, [...args, "--check"], fixture);
  const after = Object.fromEntries(Object.keys(before).map((name) => {
    const bytes = readFileSync(join(output, name)); return [name, { bytes: bytes.length, sha256: sha256(bytes) }];
  }));
  if (JSON.stringify(before) !== JSON.stringify(after)) fail("controlled artifacts drifted under --check");
  const styleManifest = JSON.parse(readFileSync(manifest, "utf8"));
  const controlManifest = JSON.parse(readFileSync(join(output, "pliego.css.manifest.json"), "utf8"));
  const receipt = JSON.parse(readFileSync(join(output, "pliego.css.receipt.json"), "utf8"));
  if (styleManifest.schemaVersion !== 3 || styleManifest.styles?.length !== 4) fail("style manifest contract drifted", styleManifest);
  if (controlManifest.schemaVersion !== "1.0.0" || receipt.schemaVersion !== "1.0.0") fail("control schema drifted");
  const origins = styleManifest.styles.flatMap((style) => style.origins ?? []);
  if (!origins.every((origin) => origin.macroKind === "pc" || origin.macroKind === "pcx")) fail("typed origins drifted", origins);
  process.stdout.write(`${JSON.stringify({
    schema: "pliegocss/representative-application/1", id: "rust-typed-controlled-artifacts", passed: true,
    application: { msrv: "1.85.0", sourceSha256: sha256(readFileSync(join(fixture, "src/main.rs"))), styleCount: styleManifest.styles.length, originKinds: [...new Set(origins.map((origin) => origin.macroKind))].sort() },
    artifacts: before,
    evidenceLimits: [
      "This gate proves one typed Rust application's MSRV compilation and deterministic seven-artifact control group.",
      "It does not measure a clean Cargo build, application rendering, or browser behavior."
    ]
  }, null, 2)}\n`);
} finally { if (output) rmSync(output, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 }); }
