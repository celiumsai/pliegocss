import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const fixture = join(root, "integration-tests", "representative", "plain-html-audit");
const cli = join(root, "target", "debug", process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc");

function fail(message, detail) {
  throw new Error(detail === undefined ? message : `${message}\n${JSON.stringify(detail, null, 2)}`);
}
function run(command, args, cwd = root, accepted = [0]) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8", windowsHide: true });
  if (result.error) throw result.error;
  if (!accepted.includes(result.status)) {
    fail(`command failed: ${command} ${args.join(" ")}`, {
      exitCode: result.status,
      stderr: result.stderr,
      stdout: result.stdout,
    });
  }
  return result;
}
function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}
function exactKeys(value, expected, label) {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) fail(`${label} keys drifted`, { actual, wanted });
}

for (const name of ["app.css", "index.html", "pliego.accessibility.json", "pliego.budgets.json"]) {
  if (!existsSync(join(fixture, name))) fail(`missing representative application file ${name}`);
}
run("cargo", ["build", "--quiet", "--locked", "-p", "pliego-cssc"]);

let output;
let transformedPath;
try {
  output = mkdtempSync(join(tmpdir(), "pliegocss-representative-audit-"));
  transformedPath = join(fixture, ".app.transformed.css");
  const transformArgs = [
    "transform-css",
    "--input",
    "app.css",
    "--output",
    ".app.transformed.css",
    "--targets",
    "modern",
    "--format",
    "minified",
  ];
  run(cli, transformArgs, fixture);
  run(cli, [...transformArgs, "--check"], fixture);
  const transformed = readFileSync(transformedPath);
  const authored = readFileSync(join(fixture, "app.css"));
  if (!transformed.toString("utf8").endsWith("\n") || transformed.equals(authored)) {
    fail("standard CSS transformation did not produce a distinct canonical artifact");
  }
  const args = [
    "audit",
    "--input",
    "app.css",
    "--targets",
    "baseline-widely",
    "--budget-policy",
    "pliego.budgets.json",
    "--accessibility-policy",
    "pliego.accessibility.json",
    "--control-dir",
    output,
    "--format",
    "json",
  ];
  const first = run(cli, args, fixture);
  const findingDocument = JSON.parse(first.stdout);
  if (findingDocument.command !== "audit" || !Array.isArray(findingDocument.findings)) {
    fail("audit output is not a canonical FindingDocument", findingDocument);
  }
  const codes = findingDocument.findings.map((finding) => finding.code);
  for (const required of ["PCSS-AUDIT-000", "PCSS-A11Y-000", "PCSS-BUDGET-100"]) {
    if (!codes.includes(required)) fail(`representative audit is missing ${required}`, codes);
  }
  const blockingFindings = findingDocument.findings
    .filter((finding) => finding.severity === "error")
    .map((finding) => ({ code: finding.code, summary: finding.summary }));
  if (blockingFindings.length > 0) {
    fail("representative audit contains a blocking finding", blockingFindings);
  }

  run(cli, [...args, "--check"], fixture);
  const files = ["pliego.css.findings.json", "pliego.css.manifest.json", "pliego.css.receipt.json"];
  const artifacts = {};
  for (const file of files) {
    const path = join(output, file);
    if (!existsSync(path)) fail(`audit control group is missing ${file}`);
    const bytes = readFileSync(path);
    artifacts[file] = { bytes: bytes.length, sha256: sha256(bytes) };
  }
  const manifest = JSON.parse(readFileSync(join(output, "pliego.css.manifest.json"), "utf8"));
  const receipt = JSON.parse(readFileSync(join(output, "pliego.css.receipt.json"), "utf8"));
  exactKeys(artifacts, files, "control artifacts");
  if (manifest.schemaVersion !== "1.0.0" || receipt.schemaVersion !== "1.0.0") {
    fail("control schemas drifted", { manifest: manifest.schemaVersion, receipt: receipt.schemaVersion });
  }
  const css = readFileSync(join(fixture, "app.css"));
  const html = readFileSync(join(fixture, "index.html"));
  const report = {
    schema: "pliegocss/representative-application/1",
    id: "plain-html-audit-first",
    passed: true,
    adoption: {
      standardCssUnchanged: true,
      stylingRuntimeBytes: 0,
      cssBytes: css.length,
      cssSha256: sha256(css),
      htmlBytes: html.length,
      htmlSha256: sha256(html),
      transformedCssBytes: transformed.length,
      transformedCssSha256: sha256(transformed),
      transformedCssCheckPassed: true,
    },
    audit: {
      target: "baseline-widely",
      findingCount: findingDocument.findings.length,
      codes,
      controlArtifacts: artifacts,
    },
    evidenceLimits: [
      "This gate proves static audit and receipt behavior for one ordinary CSS application.",
      "It does not prove visual correctness or complete CSS feature classification.",
    ],
  };
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
} finally {
  if (transformedPath) rmSync(transformedPath, { force: true });
  if (output) rmSync(output, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 });
}
