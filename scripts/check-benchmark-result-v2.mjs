import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { isAbsolute, join, relative, resolve, sep } from "node:path";
import {
  authorityPath,
  loadAuthority,
  readInstalledPackage,
  repositoryRoot,
  round,
  sha256,
  summarize,
} from "./benchmark-authority-v2.mjs";

const arguments_ = process.argv.slice(2);
let canonical = false;
let resultPath = resolve(
  repositoryRoot,
  "benchmarks",
  "results",
  "benchmark-authority-v2.local.json",
);
for (const argument of arguments_) {
  if (argument === "--") continue;
  else if (argument === "--canonical") canonical = true;
  else if (argument.startsWith("--result=")) resultPath = resolve(repositoryRoot, argument.slice(9));
  else throw new Error("usage: node scripts/check-benchmark-result-v2.mjs [--canonical] [--result=<path>]");
}

function fail(message) {
  throw new Error(`Benchmark result schema 2: ${message}`);
}
function equal(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${label} drifted`);
  }
}
function positive(value, label, allowNegative = false) {
  if (!Number.isFinite(value) || (!allowNegative && value <= 0)) fail(`${label} is invalid`);
}
function checkSummary(values, document, unit, label) {
  equal(document, summarize(values, unit), label);
}
function checkCurrentFile(document, expectedPath, label) {
  const repositoryPath = relative(repositoryRoot, expectedPath).replaceAll("\\", "/");
  if (document?.path !== repositoryPath) fail(`${label} path drifted`);
  if (document?.sha256 !== sha256(readFileSync(expectedPath))) fail(`${label} hash drifted`);
}
function git(arguments_) {
  const result = spawnSync("git", arguments_, {
    cwd: repositoryRoot,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) fail(`git ${arguments_.join(" ")} failed`);
  return result.stdout;
}
function currentGitState() {
  const status = git(["status", "--porcelain=v1", "-z", "--untracked-files=all"]);
  return {
    commit: git(["rev-parse", "HEAD"]).trim(),
    dirty: status.length > 0,
    statusEntryCount: status.split("\0").filter(Boolean).length,
    statusSha256: sha256(Buffer.from(status, "utf8")),
  };
}

const resultChild = relative(repositoryRoot, resultPath);
if (!resultChild || resultChild === ".." || resultChild.startsWith(`..${sep}`) || isAbsolute(resultChild)) {
  fail("result path escapes the repository");
}
const authority = loadAuthority();
const document = JSON.parse(readFileSync(resultPath, "utf8"));
if (document.schemaVersion !== 2) fail("schemaVersion must be 2");
if (document.benchmark !== "PliegoCSS Benchmark Authority v2 paired competitor oracle") {
  fail("benchmark identity drifted");
}
if (document.authority?.sha256 !== sha256(readFileSync(authorityPath))) fail("authority hash drifted");
if (document.authority?.path !== "benchmarks/benchmark-authority-v2/oracle.json") {
  fail("authority path drifted");
}
if (document.authority?.observedAtUtc !== authority.observedAtUtc) fail("authority observation drifted");
if (document.authority?.expiresAtUtc !== authority.expiresAtUtc) fail("authority expiration drifted");
if (document.authority?.primaryLane !== authority.primaryLane) fail("primary lane drifted");
if (document.authority?.resetContract !== "no-preflight") fail("reset contract drifted");
const generatedAt = Date.parse(document.generatedAtUtc);
if (!Number.isFinite(generatedAt)) fail("generatedAtUtc is invalid");
if (generatedAt < Date.parse(authority.observedAtUtc) || generatedAt > Date.parse(authority.expiresAtUtc)) {
  fail("result was not generated inside the oracle validity window");
}
if (document.methodology?.design !== authority.methodology.design) fail("pair design drifted");
if (canonical) {
  if (document.mode === "smoke") fail("canonical result cannot be smoke evidence");
  if (document.methodology.warmupPairs !== authority.methodology.warmupPairs) fail("warmup count drifted");
  if (document.methodology.measuredPairs !== authority.methodology.measuredPairs) fail("pair count drifted");
}
if (
  !document.methodology?.freshProcess ||
  !document.methodology?.completeSamples ||
  !document.methodology?.freshOutputVerification
) {
  fail("fresh processes, complete samples, and fresh verification are required");
}
if (document.methodology.pairOrder !== authority.methodology.pairOrder) fail("pair order drifted");
if (document.methodology.memoryMetric !== authority.methodology.memoryMetric) fail("memory metric drifted");
equal(document.methodology.compression, authority.methodology.compression, "compression contract");
if (document.claimBoundary !== authority.methodology.scorePolicy) fail("claim boundary drifted");

const harnessPath = resolve(repositoryRoot, "scripts", "measure-benchmark-authority-v2.mjs");
const runnerPath = resolve(repositoryRoot, "scripts", "benchmark-process-metrics.rs");
checkCurrentFile(document.provenance?.harness, harnessPath, "harness provenance");
checkCurrentFile(document.provenance?.processMetricsRunner, runnerPath, "process runner provenance");
if (document.provenance?.packageJsonSha256 !== sha256(readFileSync(resolve(repositoryRoot, "package.json")))) {
  fail("package.json provenance drifted");
}
if (document.provenance?.pnpmLockSha256 !== sha256(readFileSync(resolve(repositoryRoot, "pnpm-lock.yaml")))) {
  fail("pnpm lock provenance drifted");
}
equal(document.provenance?.git, currentGitState(), "Git source state");
if (document.mode === "immutable-evidence" && document.provenance.git.dirty) {
  fail("immutable evidence was captured from a dirty source tree");
}
const executablePath = join(
  repositoryRoot,
  document.environment?.cargoTarget ?? "",
  document.environment?.toolchain?.host ?? "",
  "release",
  process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc",
);
if (document.provenance?.executableSha256 !== sha256(readFileSync(executablePath))) {
  fail("measured PliegoCSS executable provenance drifted");
}
if (document.mode === "immutable-evidence") {
  const securityStatus = document.environment?.securityTooling?.status;
  if (!new Set(["active", "disabled", "not-installed", "unknown"]).has(securityStatus)) {
    fail("immutable evidence has no explicit security-tooling state");
  }
}

const expectedLanes = canonical
  ? authority.lanes.map((lane) => lane.id)
  : Object.keys(document.lanes ?? {});
if (expectedLanes.length === 0) fail("no measured lanes");
if (canonical) equal(Object.keys(document.lanes), expectedLanes, "lane set");
const expectedCorpora = authority.corpora.map((corpus) => corpus.id);
let comparisons = 0;
for (const laneId of expectedLanes) {
  const laneAuthority = authority.lanes.find((lane) => lane.id === laneId);
  const lane = document.lanes[laneId];
  if (!laneAuthority || !lane) fail(`unknown or missing lane ${laneId}`);
  if (lane.version !== laneAuthority.version || lane.cliVersion !== laneAuthority.cliVersion) {
    fail(`${laneId} version drifted`);
  }
  if (
    lane.packageManifestSha256 !==
      sha256(readFileSync(readInstalledPackage(laneAuthority.packageAlias).path)) ||
    lane.cliManifestSha256 !==
      sha256(readFileSync(readInstalledPackage(laneAuthority.cliPackageAlias).path))
  ) {
    fail(`${laneId} installed package provenance drifted`);
  }
  if (canonical) equal(Object.keys(lane.corpora), expectedCorpora, `${laneId} corpus set`);
  for (const [corpusId, comparison] of Object.entries(lane.corpora)) {
    const corpusAuthority = authority.corpora.find((corpus) => corpus.id === corpusId);
    if (!corpusAuthority) fail(`${laneId} has unknown corpus ${corpusId}`);
    equal(comparison.fixture, corpusAuthority.expected, `${laneId}/${corpusId} fixture`);
    const pairs = comparison.pairs;
    if (!Array.isArray(pairs) || pairs.length !== document.methodology.measuredPairs) {
      fail(`${laneId}/${corpusId} pair count drifted`);
    }
    const pliegoLatency = [];
    const tailwindLatency = [];
    const deltas = [];
    const ratios = [];
    const pliegoMemory = [];
    const tailwindMemory = [];
    const memoryDeltas = [];
    for (const [index, pair] of pairs.entries()) {
      const expectedOrder = index % 2 === 0 ? ["pliego", "tailwind"] : ["tailwind", "pliego"];
      if (pair.pair !== index + 1) fail(`${laneId}/${corpusId} pair index drifted`);
      equal(pair.order, expectedOrder, `${laneId}/${corpusId} pair order`);
      for (const kind of ["pliego", "tailwind"]) {
        positive(pair[kind]?.elapsedNs, `${laneId}/${corpusId} ${kind} elapsed`);
        positive(pair[kind]?.peakWorkingSetBytes, `${laneId}/${corpusId} ${kind} memory`);
        if (pair[kind].exitCode !== 0) fail(`${laneId}/${corpusId} ${kind} failed`);
      }
      if (pair.deltaNs !== pair.pliego.elapsedNs - pair.tailwind.elapsedNs) fail("paired latency delta drifted");
      if (pair.tailwindToPliegoRatio !== round(pair.tailwind.elapsedNs / pair.pliego.elapsedNs)) {
        fail("paired latency ratio drifted");
      }
      if (pair.peakWorkingSetDeltaBytes !== pair.pliego.peakWorkingSetBytes - pair.tailwind.peakWorkingSetBytes) {
        fail("paired memory delta drifted");
      }
      pliegoLatency.push(pair.pliego.elapsedNs / 1_000_000);
      tailwindLatency.push(pair.tailwind.elapsedNs / 1_000_000);
      deltas.push(pair.deltaNs / 1_000_000);
      ratios.push(pair.tailwindToPliegoRatio);
      pliegoMemory.push(pair.pliego.peakWorkingSetBytes);
      tailwindMemory.push(pair.tailwind.peakWorkingSetBytes);
      memoryDeltas.push(pair.peakWorkingSetDeltaBytes);
    }
    const summaries = comparison.summaries;
    checkSummary(pliegoLatency, summaries.pliegoLatency, "ms", "Pliego latency summary");
    checkSummary(tailwindLatency, summaries.tailwindLatency, "ms", "Tailwind latency summary");
    checkSummary(deltas, summaries.pairedLatencyDelta, "ms", "paired latency summary");
    checkSummary(ratios, summaries.tailwindToPliegoRatio, "ratio", "paired ratio summary");
    checkSummary(pliegoMemory, summaries.pliegoPeakWorkingSet, "bytes", "Pliego memory summary");
    checkSummary(tailwindMemory, summaries.tailwindPeakWorkingSet, "bytes", "Tailwind memory summary");
    checkSummary(memoryDeltas, summaries.pairedPeakWorkingSetDelta, "bytes", "paired memory summary");
    if (!comparison.determinism?.freshOutputVerified || comparison.determinism.measuredPairs !== pairs.length) {
      fail(`${laneId}/${corpusId} determinism contract drifted`);
    }
    if (comparison.coverage?.ratio !== 1 || comparison.coverage.missing?.length !== 0) {
      fail(`${laneId}/${corpusId} candidate coverage is incomplete`);
    }
    for (const kind of ["pliego", "tailwind"]) {
      const output = comparison.output[kind];
      for (const part of ["css", "html"]) {
        for (const field of ["rawBytes", "gzipBytes", "brotliBytes"]) {
          positive(output[part][field], `${laneId}/${corpusId} ${kind} ${part} ${field}`);
        }
        if (!/^[0-9a-f]{64}$/u.test(output[part].sha256)) fail("output SHA-256 is invalid");
      }
      if (comparison.determinism[`${kind}Sha256`] !== output.css.sha256) {
        fail(`${laneId}/${corpusId} ${kind} deterministic output hash drifted`);
      }
      if (output.transferGzipBytes !== output.css.gzipBytes + output.html.gzipBytes) {
        fail("gzip transfer sum drifted");
      }
      if (output.transferBrotliBytes !== output.css.brotliBytes + output.html.brotliBytes) {
        fail("Brotli transfer sum drifted");
      }
    }
    comparisons += 1;
  }
}
if (canonical && comparisons !== authority.lanes.length * authority.corpora.length) {
  fail("canonical result is incomplete");
}

process.stdout.write(
  `${JSON.stringify({ schemaVersion: 2, status: "passed", mode: document.mode, lanes: expectedLanes.length, comparisons, pairsPerComparison: document.methodology.measuredPairs }, null, 2)}\n`,
);
