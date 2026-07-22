import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  authorityPath,
  canonicalLaneMap,
  corpusMetrics,
  loadAuthority,
  materializeCorpus,
  readInstalledPackage,
  repositoryRoot,
  resolveLaneCli,
  sha256,
} from "./benchmark-authority-v2.mjs";

const network = process.argv.slice(2).includes("--network");
if (process.argv.length > (network ? 3 : 2)) {
  throw new Error("usage: node scripts/check-benchmark-authority-v2.mjs [--network]");
}

function fail(message) {
  throw new Error(`Benchmark Authority v2: ${message}`);
}

function equal(actual, expected, label) {
  if (actual !== expected) fail(`${label}: expected ${JSON.stringify(expected)}, found ${JSON.stringify(actual)}`);
}

function nonEmpty(value, label) {
  if (typeof value !== "string" || value.trim() === "") fail(`${label} must be non-empty`);
}

const authorityBytes = readFileSync(authorityPath);
const authority = loadAuthority();
equal(authority.schemaVersion, 2, "schemaVersion");
equal(authority.status, "active", "status");
equal(authority.registry, "https://registry.npmjs.org", "registry");
equal(authority.primaryLane, "tailwind-latest", "primaryLane");
equal(authority.resetContract, "no-preflight", "resetContract");
equal(authority.maximumAgeHours, 168, "maximumAgeHours");

const observedAt = Date.parse(authority.observedAtUtc);
const expiresAt = Date.parse(authority.expiresAtUtc);
if (!Number.isFinite(observedAt) || !Number.isFinite(expiresAt)) fail("oracle dates must be ISO timestamps");
equal(expiresAt - observedAt, authority.maximumAgeHours * 60 * 60 * 1_000, "oracle validity window");
const now = Date.now();
if (observedAt > now + 5 * 60 * 1_000) fail("oracle observation is in the future");
if (now > expiresAt) fail(`oracle expired at ${authority.expiresAtUtc}; refresh it before comparison`);

const expectedLaneIds = ["tailwind-latest", "tailwind-v3-lts", "tailwind-frozen-release"];
if (!Array.isArray(authority.lanes)) fail("lanes must be an array");
equal(JSON.stringify(authority.lanes.map((lane) => lane.id)), JSON.stringify(expectedLaneIds), "lane order");
const laneMap = canonicalLaneMap(authority);
equal(laneMap.get("tailwind-latest")?.distTag, "latest", "latest dist-tag");
equal(laneMap.get("tailwind-v3-lts")?.distTag, "v3-lts", "v3 LTS dist-tag");
equal(laneMap.get("tailwind-frozen-release")?.distTag, null, "frozen lane dist-tag");
if (laneMap.get("tailwind-frozen-release")?.version === laneMap.get("tailwind-latest")?.version) {
  fail("frozen release must remain distinct from the current competitor lane");
}

const packageJson = JSON.parse(readFileSync(resolve(repositoryRoot, "package.json"), "utf8"));
const lockText = readFileSync(resolve(repositoryRoot, "pnpm-lock.yaml"), "utf8");
for (const lane of authority.lanes) {
  for (const field of ["role", "packageAlias", "packageName", "version", "integrity", "cliPackageAlias", "cliPackageName", "cliVersion", "cliIntegrity"]) {
    nonEmpty(lane[field], `${lane.id}.${field}`);
  }
  const packageSpec = packageJson.devDependencies?.[lane.packageAlias];
  const expectedPackageSpec =
    lane.packageAlias === lane.packageName ? lane.version : `npm:${lane.packageName}@${lane.version}`;
  equal(packageSpec, expectedPackageSpec, `${lane.id} package pin`);
  const cliSpec = packageJson.devDependencies?.[lane.cliPackageAlias];
  const expectedCliSpec =
    lane.cliPackageAlias === lane.cliPackageName
      ? lane.cliVersion
      : `npm:${lane.cliPackageName}@${lane.cliVersion}`;
  equal(cliSpec, expectedCliSpec, `${lane.id} CLI pin`);

  const installedPackage = readInstalledPackage(lane.packageAlias).value;
  equal(installedPackage.name, lane.packageName, `${lane.id} installed package name`);
  equal(installedPackage.version, lane.version, `${lane.id} installed package version`);
  const installedCli = readInstalledPackage(lane.cliPackageAlias).value;
  equal(installedCli.name, lane.cliPackageName, `${lane.id} installed CLI name`);
  equal(installedCli.version, lane.cliVersion, `${lane.id} installed CLI version`);
  resolveLaneCli(lane);
  if (!lockText.includes(lane.integrity) || !lockText.includes(lane.cliIntegrity)) {
    fail(`${lane.id} registry integrity is not bound by pnpm-lock.yaml`);
  }
}

const expectedCorpora = ["micro", "medium", "large"];
if (!Array.isArray(authority.corpora)) fail("corpora must be an array");
equal(JSON.stringify(authority.corpora.map((corpus) => corpus.id)), JSON.stringify(expectedCorpora), "corpus order");
const corpusResults = [];
for (const corpus of authority.corpora) {
  if (!Number.isSafeInteger(corpus.repeat) || corpus.repeat < 1) fail(`${corpus.id}.repeat is invalid`);
  nonEmpty(corpus.source, `${corpus.id}.source`);
  nonEmpty(corpus.scope, `${corpus.id}.scope`);
  const html = materializeCorpus(corpus);
  const metrics = corpusMetrics(html);
  for (const field of ["sha256", "htmlBytes", "classAttributeCount", "utilityOccurrences", "uniqueUtilityTokens", "uniqueStyleInputs"]) {
    equal(corpus.expected?.[field], metrics[field], `${corpus.id}.expected.${field}`);
  }
  corpusResults.push({ id: corpus.id, ...metrics });
}
if (!(corpusResults[0].htmlBytes < corpusResults[1].htmlBytes && corpusResults[1].htmlBytes < corpusResults[2].htmlBytes)) {
  fail("corpus byte sizes must increase micro < medium < large");
}

const methodology = authority.methodology;
equal(methodology.design, "paired-interleaved-alternating-order", "methodology.design");
equal(methodology.warmupPairs, 5, "methodology.warmupPairs");
equal(methodology.measuredPairs, 30, "methodology.measuredPairs");
equal(methodology.freshProcess, true, "methodology.freshProcess");
equal(methodology.freshOutputVerification, true, "methodology.freshOutputVerification");
equal(methodology.completeSamples, true, "methodology.completeSamples");
equal(methodology.compression?.gzipLevel, 9, "methodology gzip");
equal(methodology.compression?.brotliQuality, 11, "methodology Brotli");
nonEmpty(methodology.memoryMetric, "methodology.memoryMetric");
nonEmpty(methodology.scorePolicy, "methodology.scorePolicy");

const requiredFiles = [
  "scripts/benchmark-process-metrics.rs",
  "scripts/measure-benchmark-authority-v2.mjs",
  "scripts/render-benchmark-authority-v2.mjs",
  authority.artifacts.generatedDocumentation,
];
for (const path of requiredFiles) {
  readFileSync(resolve(repositoryRoot, path));
}
const legacyGate = readFileSync(resolve(repositoryRoot, "scripts/measure-pliego-gate-b.mjs"), "utf8");
if (legacyGate.includes("frozenTailwind") || legacyGate.includes("frozen_tailwind_baseline")) {
  fail("Gate B still owns a handwritten Tailwind baseline");
}

const networkResults = [];
if (network) {
  const responses = new Map();
  for (const packageName of new Set(authority.lanes.flatMap((lane) => [lane.packageName, lane.cliPackageName]))) {
    const response = await fetch(`${authority.registry}/${packageName.replaceAll("/", "%2f")}`);
    if (!response.ok) fail(`registry request for ${packageName} returned HTTP ${response.status}`);
    responses.set(packageName, await response.json());
  }
  for (const lane of authority.lanes) {
    const packageMetadata = responses.get(lane.packageName);
    if (lane.distTag) equal(packageMetadata["dist-tags"]?.[lane.distTag], lane.version, `${lane.id} live dist-tag`);
    equal(packageMetadata.versions?.[lane.version]?.dist?.integrity, lane.integrity, `${lane.id} live package integrity`);
    const cliMetadata = responses.get(lane.cliPackageName);
    equal(cliMetadata.versions?.[lane.cliVersion]?.dist?.integrity, lane.cliIntegrity, `${lane.id} live CLI integrity`);
    networkResults.push({ id: lane.id, registryVerified: true });
  }
}

process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 2,
      status: "passed",
      authoritySha256: sha256(authorityBytes),
      observedAtUtc: authority.observedAtUtc,
      expiresAtUtc: authority.expiresAtUtc,
      lanes: authority.lanes.map((lane) => ({ id: lane.id, version: lane.version })),
      corpora: corpusResults,
      network: network ? networkResults : "not-requested",
    },
    null,
    2,
  )}\n`,
);
