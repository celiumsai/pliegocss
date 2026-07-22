import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  corpusMetrics,
  loadAuthority,
  materializeCorpus,
  repositoryRoot,
} from "./benchmark-authority-v2.mjs";

const matrixPath = resolve(repositoryRoot, "docs/benchmarks/tailwind-v4-competitive-matrix.json");
const matrix = JSON.parse(readFileSync(matrixPath, "utf8"));
const authority = loadAuthority();

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

if (matrix.schemaVersion !== 2) fail("schemaVersion must be 2");
if (matrix.baseline?.product !== "Tailwind CSS" || matrix.baseline?.license !== "MIT") {
  fail("baseline identity must be Tailwind CSS/MIT");
}
if (matrix.baseline.authority !== "benchmarks/benchmark-authority-v2/oracle.json") {
  fail("baseline must delegate competitor identity to Benchmark Authority v2");
}
if (matrix.baseline.primaryLane !== authority.primaryLane) fail("primary lane drifted");
const primaryLane = authority.lanes.find((lane) => lane.id === authority.primaryLane);
if (!primaryLane || matrix.baseline.version !== primaryLane.version) fail("primary version drifted");
const packagePath = resolve(repositoryRoot, matrix.baseline.packageManifest);
const fixturePath = resolve(repositoryRoot, matrix.baseline.fixture);
if (sha256(packagePath) !== matrix.baseline.packageManifestSha256) fail("package manifest hash drifted");
const installed = JSON.parse(readFileSync(packagePath, "utf8"));
if (installed.name !== "tailwindcss" || installed.version !== primaryLane.version) {
  fail("installed primary Tailwind package drifted");
}
const fixtureMetrics = corpusMetrics(readFileSync(fixturePath, "utf8").replaceAll("\r\n", "\n"));
for (const [field, expected] of [
  ["sha256", matrix.baseline.fixtureSha256],
  ["classAttributeCount", matrix.baseline.fixtureClassAttributes],
  ["utilityOccurrences", matrix.baseline.fixtureUtilityOccurrences],
  ["uniqueUtilityTokens", matrix.baseline.fixtureUniqueUtilities],
]) {
  if (fixtureMetrics[field] !== expected) fail(`fixture ${field} drifted`);
}
const medium = authority.corpora.find((corpus) => corpus.id === "medium");
if (JSON.stringify(corpusMetrics(materializeCorpus(medium))) !== JSON.stringify(medium.expected)) {
  fail("matrix fixture is not the authority medium corpus");
}

const statuses = new Set([
  "matched",
  "pliego-differentiator",
  "tailwind-advantage",
  "different-by-design",
  "open-gap",
]);
const impacts = new Set(["keep-green", "blocking", "should", "non-blocking", "post-0.1"]);
if (JSON.stringify(Object.keys(matrix.statuses).sort()) !== JSON.stringify([...statuses].sort())) {
  fail("status definitions drifted");
}
const sourceIds = new Set();
for (const source of array(matrix.sources, "sources")) {
  if (sourceIds.has(source.id)) fail(`duplicate source ${source.id}`);
  sourceIds.add(source.id);
  if (new Set(["local-evidence", "historical-evidence", "local-contract"]).has(source.kind)) {
    if (!existsSync(resolve(repositoryRoot, source.path))) fail(`source ${source.id} is missing`);
  } else if (source.kind === "upstream-doc") {
    if (!/^https:\/\/tailwindcss\.com\/docs\//u.test(source.url ?? "")) fail(`${source.id} is not an upstream docs URL`);
    if (source.versionObserved !== "v4.3") fail(`${source.id} upstream version drifted`);
  } else {
    fail(`source ${source.id} has unknown kind`);
  }
}
const dimensionIds = new Set();
const counts = Object.fromEntries([...statuses].map((status) => [status, 0]));
for (const dimension of array(matrix.dimensions, "dimensions")) {
  if (!/^[a-z][a-z0-9-]*$/u.test(dimension.id ?? "") || dimensionIds.has(dimension.id)) {
    fail(`invalid or duplicate dimension ${dimension.id}`);
  }
  dimensionIds.add(dimension.id);
  if (!statuses.has(dimension.status)) fail(`${dimension.id} has unknown status`);
  if (!impacts.has(dimension.releaseImpact)) fail(`${dimension.id} has unknown releaseImpact`);
  for (const field of ["area", "tailwind", "pliego", "nextAction"]) {
    if (typeof dimension[field] !== "string" || !dimension[field].trim()) fail(`${dimension.id}.${field} is empty`);
  }
  for (const evidence of array(dimension.evidence, `${dimension.id}.evidence`)) {
    if (!sourceIds.has(evidence)) fail(`${dimension.id} references unknown evidence ${evidence}`);
  }
  counts[dimension.status] += 1;
}
if (dimensionIds.size !== 22) fail(`expected 22 competitive dimensions, found ${dimensionIds.size}`);
for (const [status, count] of Object.entries(counts)) {
  if (count < 1) fail(`matrix has no ${status} dimension`);
}
for (const id of ["build-latency-fixture", "payload-fixture"]) {
  const dimension = matrix.dimensions.find((item) => item.id === id);
  if (dimension?.status !== "open-gap" || dimension.releaseImpact !== "blocking") {
    fail(`${id} must remain blocking until clean schema-2 evidence exists`);
  }
}

const renderer = spawnSync(process.execPath, ["scripts/render-tailwind-matrix.mjs", "--check"], {
  cwd: repositoryRoot,
  encoding: "utf8",
  windowsHide: true,
});
if (renderer.error) throw renderer.error;
if (renderer.status !== 0) fail(renderer.stderr.trim() || "generated Markdown check failed");

process.stdout.write(
  `${JSON.stringify({ schemaVersion: 2, baseline: primaryLane.version, dimensions: dimensionIds.size, counts }, null, 2)}\n`,
);
