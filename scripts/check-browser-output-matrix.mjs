import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { repositoryRoot, sha256 } from "./benchmark-authority-v2.mjs";

const authorityPath = resolve(
  repositoryRoot,
  "benchmarks",
  "browser-output-certification-v1",
  "authority.json",
);
const authority = JSON.parse(readFileSync(authorityPath, "utf8"));
const competitorPath = resolve(repositoryRoot, authority.competitorAuthority);
const competitorBytes = readFileSync(competitorPath);
const competitor = JSON.parse(competitorBytes);
const lane = competitor.lanes.find((candidate) => candidate.id === authority.competitorLane);
if (!lane) throw new Error(`Browser/output matrix: missing ${authority.competitorLane}`);
if (Date.now() > Date.parse(competitor.expiresAtUtc)) {
  throw new Error("Browser/output matrix: competitor authority expired");
}
const tailwindManifestSha256 = sha256(
  readFileSync(resolve(repositoryRoot, "node_modules", lane.packageAlias, "package.json")),
);
const tailwindCliManifestSha256 = sha256(
  readFileSync(resolve(repositoryRoot, "node_modules", ...lane.cliPackageAlias.split("/"), "package.json")),
);
const arguments_ = process.argv.slice(2).filter((argument) => argument !== "--");
let evidenceRoot = resolve(
  repositoryRoot,
  "target",
  "browser-output-certification",
  "incoming",
);
let outputPath = resolve(
  repositoryRoot,
  "target",
  "browser-output-certification",
  "browser-output-matrix.json",
);
for (const argument of arguments_) {
  if (argument.startsWith("--evidence-root=")) evidenceRoot = resolve(repositoryRoot, argument.slice(16));
  else if (argument.startsWith("--output=")) outputPath = resolve(repositoryRoot, argument.slice(9));
  else throw new Error("usage: node scripts/check-browser-output-matrix.mjs [--evidence-root=<path>] [--output=<path>]");
}

function fail(message) {
  throw new Error(`Browser/output matrix: ${message}`);
}

function repositoryChild(path, label) {
  const child = relative(repositoryRoot, path);
  if (!child || child === ".." || child.startsWith(`..${sep}`) || isAbsolute(child)) {
    fail(`${label} escapes the repository`);
  }
  return child.replaceAll("\\", "/");
}

repositoryChild(evidenceRoot, "evidence root");
repositoryChild(outputPath, "output");

function jsonFiles(directory) {
  if (!existsSync(directory)) fail(`evidence root does not exist: ${directory}`);
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) return jsonFiles(path);
    return entry.isFile() && entry.name.endsWith(".json") ? [path] : [];
  });
}

function canonicalInstant(value, label) {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/u.test(value ?? "")) {
    fail(`${label} is not a canonical UTC instant`);
  }
  return Date.parse(value);
}

function same(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) fail(`${label} drifted`);
}

function gitObject(arguments_, label) {
  const result = spawnSync("git", arguments_, {
    cwd: repositoryRoot,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.error || result.status !== 0) fail(`cannot resolve checkout ${label}`);
  const value = result.stdout.trim();
  if (!/^[0-9a-f]{40}$/u.test(value)) fail(`checkout ${label} is invalid`);
  return value;
}

function verifyArtifact(evidencePath, artifact, label) {
  if (!artifact || typeof artifact.path !== "string" || !/^[0-9a-f]{64}$/u.test(artifact.sha256)) {
    fail(`${label} metadata is invalid`);
  }
  const path = resolve(dirname(evidencePath), artifact.path);
  const child = relative(dirname(evidencePath), path);
  if (!child || child === ".." || child.startsWith(`..${sep}`) || isAbsolute(child)) {
    fail(`${label} escapes its host artifact`);
  }
  const bytes = readFileSync(path);
  if (sha256(bytes) !== artifact.sha256 || bytes.byteLength !== artifact.bytes) {
    fail(`${label} hash or size drifted`);
  }
}

const authoritySha256 = sha256(readFileSync(authorityPath));
const documents = jsonFiles(evidenceRoot)
  .map((path) => ({ path, value: JSON.parse(readFileSync(path, "utf8")) }))
  .filter(({ value }) => value.kind === "pliegocss-browser-output-host-evidence");
if (documents.length !== authority.hosts.length) {
  fail(`expected ${authority.hosts.length} host documents, found ${documents.length}`);
}

const source = documents[0]?.value.source;
const inputs = documents[0]?.value.inputs;
const checkoutSource = {
  commit: gitObject(["rev-parse", "HEAD"], "commit"),
  gitTree: gitObject(["rev-parse", "HEAD^{tree}"], "Git tree"),
};
if (source?.commit !== checkoutSource.commit || source?.gitTree !== checkoutSource.gitTree) {
  fail("host evidence is not bound to the aggregator checkout");
}
const hostSummaries = [];
for (const expectedHost of authority.hosts) {
  const entry = documents.find(({ value }) => value.host?.id === expectedHost.id);
  if (!entry) fail(`missing ${expectedHost.id}`);
  const document = entry.value;
  if (document.schemaVersion !== 1 || document.result !== "passed") fail(`${expectedHost.id} did not pass`);
  same(document.source, source, `${expectedHost.id} source`);
  if (document.source.dirty || document.source.statusEntryCount !== 0) {
    fail(`${expectedHost.id} is not clean-tree evidence`);
  }
  if (document.source.statusSha256 !== sha256(Buffer.alloc(0))) {
    fail(`${expectedHost.id} clean status hash drifted`);
  }
  if (!/^[0-9a-f]{40}$/u.test(document.source.commit) || !/^[0-9a-f]{40}$/u.test(document.source.gitTree)) {
    fail(`${expectedHost.id} source IDs are invalid`);
  }
  const generated = canonicalInstant(document.generatedAtUtc, `${expectedHost.id}.generatedAtUtc`);
  const expires = canonicalInstant(document.expiresAtUtc, `${expectedHost.id}.expiresAtUtc`);
  if (expires - generated !== authority.maximumEvidenceAgeHours * 60 * 60 * 1_000) {
    fail(`${expectedHost.id} validity window drifted`);
  }
  if (Date.now() > expires) fail(`${expectedHost.id} evidence expired`);
  same(
    {
      id: document.host.id,
      runner: document.host.runner,
      os: document.host.os,
      arch: document.host.arch,
      browser: document.host.browser,
    },
    expectedHost,
    `${expectedHost.id} host identity`,
  );
  if (!document.host.browserVersion || document.host.playwright !== "1.61.1") {
    fail(`${expectedHost.id} browser/tooling identity is incomplete`);
  }
  if (
    typeof document.compiler?.rustc !== "string" ||
    typeof document.compiler?.host !== "string" ||
    !/^[0-9a-f]{64}$/u.test(document.compiler?.sha256 ?? "") ||
    Object.hasOwn(document.compiler ?? {}, "executable")
  ) {
    fail(`${expectedHost.id} compiler identity is incomplete or non-portable`);
  }
  if (
    document.authority?.path !== "benchmarks/browser-output-certification-v1/authority.json" ||
    document.authority?.sha256 !== authoritySha256
  ) {
    fail(`${expectedHost.id} authority drifted`);
  }
  same(document.resetModes, authority.resetModes, `${expectedHost.id} reset modes`);
  same(document.computedProperties, authority.computedProperties, `${expectedHost.id} computed properties`);
  same(document.coverage, authority.requiredCoverage, `${expectedHost.id} coverage`);
  same(document.inputs, inputs, `${expectedHost.id} inputs`);
  if (
    JSON.stringify(Object.keys(document.inputs ?? {}).sort()) !== JSON.stringify([
      "competitorAuthoritySha256",
      "competitorExpiresAtUtc",
      "competitorObservedAtUtc",
      "fixtureSha256",
      "pliegoCssSha256",
      "resetSha256",
      "tailwindCliManifestSha256",
      "tailwindCliVersion",
      "tailwindCssSha256",
      "tailwindPackageManifestSha256",
      "tailwindVersion",
    ]) ||
    document.inputs.competitorAuthoritySha256 !== sha256(competitorBytes) ||
    document.inputs.competitorObservedAtUtc !== competitor.observedAtUtc ||
    document.inputs.competitorExpiresAtUtc !== competitor.expiresAtUtc ||
    document.inputs?.tailwindVersion !== lane.version ||
    document.inputs?.tailwindCliVersion !== lane.cliVersion ||
    document.inputs.tailwindPackageManifestSha256 !== tailwindManifestSha256 ||
    document.inputs.tailwindCliManifestSha256 !== tailwindCliManifestSha256 ||
    document.inputs.fixtureSha256 !== authority.fixture.sha256 ||
    document.inputs.resetSha256 !== authority.reset.sha256 ||
    !/^[0-9a-f]{64}$/u.test(document.inputs.pliegoCssSha256 ?? "") ||
    !/^[0-9a-f]{64}$/u.test(document.inputs.tailwindCssSha256 ?? "")
  ) {
    fail(`${expectedHost.id} competitor provenance is incomplete`);
  }
  const expectedObservationIds = authority.resetModes.flatMap((mode) =>
    authority.scenarios.map((scenario) => `${mode.id}/${scenario.id}`),
  );
  const actualObservationIds = document.observations.map(
    (observation) => `${observation.resetMode}/${observation.scenario}`,
  );
  same(actualObservationIds, expectedObservationIds, `${expectedHost.id} observations`);
  let maximumMismatchRatio = 0;
  let maximumLayoutGeometryDeltaCssPx = 0;
  for (const observation of document.observations) {
    const scenario = authority.scenarios.find((candidate) => candidate.id === observation.scenario);
    if (!scenario) fail(`${expectedHost.id}/${observation.scenario} scenario is not authoritative`);
    same(
      observation.viewport,
      { width: scenario.width, height: scenario.height },
      `${expectedHost.id}/${observation.resetMode}/${observation.scenario} viewport`,
    );
    if (observation.action !== scenario.action) {
      fail(`${expectedHost.id}/${observation.resetMode}/${observation.scenario} action drifted`);
    }
    if (!observation.computed.equal || observation.computed.differenceCount !== 0) {
      fail(`${expectedHost.id}/${observation.resetMode}/${observation.scenario} computed styles differ`);
    }
    if (
      !Array.isArray(observation.computed.differences) ||
      observation.computed.differences.length !== 0 ||
      !/^[0-9a-f]{64}$/u.test(observation.computed.pliegoSha256 ?? "") ||
      !/^[0-9a-f]{64}$/u.test(observation.computed.tailwindSha256 ?? "") ||
      !Number.isSafeInteger(observation.computed.nodeCount) ||
      observation.computed.nodeCount < 1
    ) {
      fail(`${expectedHost.id}/${observation.resetMode}/${observation.scenario} computed evidence is incomplete`);
    }
    const geometryDelta = observation.computed.maximumLayoutGeometryDeltaCssPx;
    if (
      !Number.isFinite(geometryDelta) ||
      geometryDelta < 0 ||
      geometryDelta > authority.comparison.maximumLayoutGeometryDeltaCssPx
    ) {
      fail(`${expectedHost.id}/${observation.resetMode}/${observation.scenario} layout geometry failed`);
    }
    maximumLayoutGeometryDeltaCssPx = Math.max(
      maximumLayoutGeometryDeltaCssPx,
      geometryDelta,
    );
    const screenshots = observation.screenshots;
    if (
      !screenshots.passed ||
      screenshots.threshold !== authority.comparison.screenshotPixelThreshold ||
      screenshots.maximumMismatchRatio !== authority.comparison.maximumScreenshotMismatchRatio ||
      !Number.isSafeInteger(screenshots.mismatchPixels) ||
      screenshots.mismatchPixels < 0 ||
      !Number.isFinite(screenshots.mismatchRatio) ||
      screenshots.mismatchRatio < 0 ||
      screenshots.mismatchRatio > authority.comparison.maximumScreenshotMismatchRatio ||
      !Number.isSafeInteger(screenshots.width) ||
      !Number.isSafeInteger(screenshots.height) ||
      screenshots.width !== scenario.width ||
      screenshots.height < scenario.height ||
      screenshots.tailwindWidth !== screenshots.width ||
      screenshots.tailwindHeight !== screenshots.height ||
      screenshots.mismatchPixels > screenshots.width * screenshots.height ||
      screenshots.mismatchRatio !==
        Number((screenshots.mismatchPixels / (screenshots.width * screenshots.height)).toFixed(9))
    ) {
      fail(`${expectedHost.id}/${observation.resetMode}/${observation.scenario} screenshot comparison failed`);
    }
    maximumMismatchRatio = Math.max(maximumMismatchRatio, screenshots.mismatchRatio ?? 1);
    verifyArtifact(entry.path, screenshots.pliego, `${expectedHost.id} Pliego screenshot`);
    verifyArtifact(entry.path, screenshots.tailwind, `${expectedHost.id} Tailwind screenshot`);
    verifyArtifact(entry.path, screenshots.diff, `${expectedHost.id} screenshot diff`);
  }
  if (
    document.summary.computedMismatchCount !== 0 ||
    document.summary.screenshotMismatchCount !== 0 ||
    document.summary.resetModeCount !== authority.resetModes.length ||
    document.summary.scenarioCount !== expectedObservationIds.length ||
    document.summary.maximumLayoutGeometryDeltaCssPx !==
      Number(maximumLayoutGeometryDeltaCssPx.toFixed(6))
  ) {
    fail(`${expectedHost.id} summary drifted`);
  }
  hostSummaries.push({
    id: expectedHost.id,
    runner: expectedHost.runner,
    os: expectedHost.os,
    arch: expectedHost.arch,
    browser: expectedHost.browser,
    browserVersion: document.host.browserVersion,
    generatedAtUtc: document.generatedAtUtc,
    expiresAtUtc: document.expiresAtUtc,
    scenarios: document.observations.length,
    maximumLayoutGeometryDeltaCssPx: Number(maximumLayoutGeometryDeltaCssPx.toFixed(6)),
    maximumScreenshotMismatchRatio: Number(maximumMismatchRatio.toFixed(9)),
    evidence: {
      path: repositoryChild(entry.path, `${expectedHost.id} evidence`),
      sha256: sha256(readFileSync(entry.path)),
    },
  });
}

const matrix = {
  schemaVersion: 1,
  kind: "pliegocss-browser-output-certification-matrix",
  result: "passed",
  generatedAtUtc: new Date().toISOString().replace(/\.\d{3}Z$/u, "Z"),
  source: {
    commit: source.commit,
    gitTree: source.gitTree,
  },
  authority: {
    path: "benchmarks/browser-output-certification-v1/authority.json",
    sha256: authoritySha256,
  },
  coverage: authority.requiredCoverage,
  resetModes: authority.resetModes.map((mode) => mode.id),
  hosts: hostSummaries,
  claimBoundary: authority.comparison.claimBoundary,
};
mkdirSync(dirname(outputPath), { recursive: true });
writeFileSync(outputPath, `${JSON.stringify(matrix, null, 2)}\n`);
process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 1,
      result: "passed",
      source: matrix.source,
      hosts: matrix.hosts.length,
      browsers: authority.browsers,
      output: repositoryChild(outputPath, "output"),
    },
    null,
    2,
  )}\n`,
);
