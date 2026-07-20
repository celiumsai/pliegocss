import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const matrixPath = resolve(root, "docs/benchmarks/tailwind-v4-competitive-matrix.json");
const matrix = JSON.parse(readFileSync(matrixPath, "utf8"));

function fail(message) {
  throw new Error(`Tailwind competitive matrix: ${message}`);
}
function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}
function array(value, label) {
  if (!Array.isArray(value) || value.length === 0) fail(`${label} must be a non-empty array`);
  return value;
}

if (matrix.schemaVersion !== 1) fail("schemaVersion must be 1");
if (matrix.baseline?.product !== "Tailwind CSS" || matrix.baseline?.version !== "4.3.2") {
  fail("baseline must pin Tailwind CSS 4.3.2");
}
if (matrix.baseline?.license !== "MIT") fail("baseline license must be MIT");
const packagePath = resolve(root, matrix.baseline.packageManifest);
const fixturePath = resolve(root, matrix.baseline.fixture);
for (const [path, expected, label] of [
  [packagePath, matrix.baseline.packageManifestSha256, "package manifest"],
  [fixturePath, matrix.baseline.fixtureSha256, "fixture"],
]) {
  if (!existsSync(path)) fail(`${label} is missing`);
  const actual = sha256(path);
  if (actual !== expected) fail(`${label} hash drifted: expected ${expected}, found ${actual}`);
}
const packageJson = JSON.parse(readFileSync(packagePath, "utf8"));
if (packageJson.name !== "tailwindcss" || packageJson.version !== matrix.baseline.version) {
  fail("installed Tailwind package identity drifted");
}
const fixture = readFileSync(fixturePath, "utf8");
const classValues = [...fixture.matchAll(/class="([^"]+)"/g)].map((match) => match[1]);
const utilities = classValues.flatMap((value) => value.trim().split(/\s+/u));
if (classValues.length !== matrix.baseline.fixtureClassAttributes) fail("fixture class count drifted");
if (utilities.length !== matrix.baseline.fixtureUtilityOccurrences) fail("fixture utility count drifted");
if (new Set(utilities).size !== matrix.baseline.fixtureUniqueUtilities) {
  fail("fixture unique utility count drifted");
}
const statuses = new Set([
  "matched",
  "pliego-differentiator",
  "tailwind-advantage",
  "different-by-design",
  "open-gap",
]);
const impacts = new Set(["keep-green", "blocking", "should", "non-blocking", "post-0.1"]);
if (Object.keys(matrix.statuses ?? {}).sort().join("\n") !== [...statuses].sort().join("\n")) {
  fail("status definitions drifted");
}
const sourceIds = new Set();
for (const source of array(matrix.sources, "sources")) {
  if (sourceIds.has(source.id)) fail(`duplicate source ${source.id}`);
  sourceIds.add(source.id);
  if (source.kind === "local-evidence" || source.kind === "local-contract") {
    if (!existsSync(resolve(root, source.path))) fail(`source ${source.id} path is missing`);
  } else if (source.kind === "upstream-doc") {
    if (!/^https:\/\/tailwindcss\.com\/docs\//u.test(source.url ?? "")) {
      fail(`source ${source.id} is not a Tailwind documentation URL`);
    }
    if (source.versionObserved !== "v4.3") fail(`source ${source.id} version is not v4.3`);
  } else {
    fail(`source ${source.id} has unknown kind`);
  }
}
const dimensionIds = new Set();
const counts = Object.fromEntries([...statuses].map((status) => [status, 0]));
for (const dimension of array(matrix.dimensions, "dimensions")) {
  if (!/^[a-z][a-z0-9-]*$/u.test(dimension.id ?? "")) fail("dimension id is not canonical");
  if (dimensionIds.has(dimension.id)) fail(`duplicate dimension ${dimension.id}`);
  dimensionIds.add(dimension.id);
  if (!statuses.has(dimension.status)) fail(`${dimension.id} has unknown status`);
  if (!impacts.has(dimension.releaseImpact)) fail(`${dimension.id} has unknown releaseImpact`);
  for (const field of ["area", "tailwind", "pliego", "nextAction"]) {
    if (typeof dimension[field] !== "string" || dimension[field].trim() === "") {
      fail(`${dimension.id}.${field} is empty`);
    }
  }
  for (const evidence of array(dimension.evidence, `${dimension.id}.evidence`)) {
    if (!sourceIds.has(evidence)) fail(`${dimension.id} references unknown evidence ${evidence}`);
  }
  counts[dimension.status] += 1;
}
const expectedCounts = {
  matched: 5,
  "pliego-differentiator": 6,
  "tailwind-advantage": 5,
  "different-by-design": 3,
  "open-gap": 3,
};
if (JSON.stringify(counts) !== JSON.stringify(expectedCounts)) {
  fail(`status counts drifted: ${JSON.stringify(counts)}`);
}
const human = readFileSync(resolve(root, "docs/benchmarks/tailwind-v4-competitive-matrix.md"), "utf8");
for (const marker of ["Tailwind CSS v4.3.2", "Blocking `0.1.0`", "Explicitly post-0.1"]) {
  if (!human.includes(marker)) fail(`human matrix is missing ${marker}`);
}
process.stdout.write(
  `${JSON.stringify({ schemaVersion: 1, baseline: matrix.baseline.version, dimensions: matrix.dimensions.length, counts }, null, 2)}\n`,
);
