import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const authorityPath = resolve(
  root,
  "benchmarks",
  "browser-output-certification-v1",
  "authority.json",
);
const authorityBytes = readFileSync(authorityPath);
const authority = JSON.parse(authorityBytes);
const competitor = JSON.parse(readFileSync(resolve(root, authority.competitorAuthority), "utf8"));
const competitorBytes = readFileSync(resolve(root, authority.competitorAuthority));
const lane = competitor.lanes.find((candidate) => candidate.id === authority.competitorLane);
assert(lane);

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const workRoot = resolve(
  root,
  "target",
  "browser-output-certification",
  `matrix-contract-${process.pid}`,
);
const incoming = resolve(workRoot, "incoming");
const output = resolve(workRoot, "matrix.json");
const generated = new Date();
generated.setMilliseconds(0);
const expires = new Date(generated.getTime() + authority.maximumEvidenceAgeHours * 60 * 60 * 1_000);
const source = {
  commit: spawnSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).stdout.trim(),
  gitTree: spawnSync("git", ["rev-parse", "HEAD^{tree}"], { cwd: root, encoding: "utf8" }).stdout.trim(),
  dirty: false,
  statusEntryCount: 0,
  statusSha256: sha256(Buffer.alloc(0)),
};

function runMatrix() {
  return spawnSync(
    process.execPath,
    [
      "scripts/check-browser-output-matrix.mjs",
      `--evidence-root=${relative(root, incoming).replaceAll("\\", "/")}`,
      `--output=${relative(root, output).replaceAll("\\", "/")}`,
    ],
    { cwd: root, encoding: "utf8", windowsHide: true },
  );
}

function evidence(host, directory) {
  const artifacts = {};
  for (const [name, bytes] of [
    ["pliego", Buffer.from("pliego")],
    ["tailwind", Buffer.from("tailwind")],
    ["diff", Buffer.from("diff")],
  ]) {
    const path = resolve(directory, `${name}.png`);
    writeFileSync(path, bytes);
    artifacts[name] = { path: `${name}.png`, sha256: sha256(bytes), bytes: bytes.byteLength };
  }
  const observations = authority.resetModes.flatMap((mode) =>
    authority.scenarios.map((scenario) => ({
      resetMode: mode.id,
      scenario: scenario.id,
      viewport: { width: scenario.width, height: scenario.height },
      action: scenario.action,
      computed: {
        equal: true,
        differenceCount: 0,
        differences: [],
        maximumLayoutGeometryDeltaCssPx: 0,
        pliegoSha256: "3".repeat(64),
        tailwindSha256: "3".repeat(64),
        nodeCount: 44,
      },
      screenshots: {
        passed: true,
        byteEqual: true,
        width: scenario.width,
        height: scenario.height,
        tailwindWidth: scenario.width,
        tailwindHeight: scenario.height,
        mismatchPixels: 0,
        mismatchRatio: 0,
        diff: artifacts.diff,
        threshold: authority.comparison.screenshotPixelThreshold,
        maximumMismatchRatio: authority.comparison.maximumScreenshotMismatchRatio,
        pliego: artifacts.pliego,
        tailwind: artifacts.tailwind,
      },
    })),
  );
  return {
    schemaVersion: 1,
    kind: "pliegocss-browser-output-host-evidence",
    result: "passed",
    generatedAtUtc: generated.toISOString().replace(/\.000Z$/u, "Z"),
    expiresAtUtc: expires.toISOString().replace(/\.000Z$/u, "Z"),
    source,
    host: {
      ...host,
      os: host.os,
      arch: host.arch,
      node: process.version,
      playwright: "1.61.1",
      browserVersion: "contract-test",
    },
    authority: {
      path: "benchmarks/browser-output-certification-v1/authority.json",
      sha256: sha256(authorityBytes),
    },
    compiler: {
      sha256: "4".repeat(64),
      rustc: "rustc 1.96.0 (contract-test)",
      host: "contract-test",
    },
    inputs: {
      competitorAuthoritySha256: sha256(competitorBytes),
      competitorObservedAtUtc: competitor.observedAtUtc,
      competitorExpiresAtUtc: competitor.expiresAtUtc,
      tailwindVersion: lane.version,
      tailwindCliVersion: lane.cliVersion,
      tailwindPackageManifestSha256: sha256(
        readFileSync(resolve(root, "node_modules", lane.packageAlias, "package.json")),
      ),
      tailwindCliManifestSha256: sha256(
        readFileSync(resolve(root, "node_modules", ...lane.cliPackageAlias.split("/"), "package.json")),
      ),
      fixtureSha256: authority.fixture.sha256,
      resetSha256: authority.reset.sha256,
      pliegoCssSha256: "5".repeat(64),
      tailwindCssSha256: "6".repeat(64),
    },
    resetModes: authority.resetModes,
    computedProperties: authority.computedProperties,
    observations,
    coverage: authority.requiredCoverage,
    summary: {
      resetModeCount: authority.resetModes.length,
      scenarioCount: observations.length,
      computedMismatchCount: 0,
      screenshotMismatchCount: 0,
      maximumLayoutGeometryDeltaCssPx: 0,
    },
    claimBoundary: authority.comparison.claimBoundary,
  };
}

try {
  mkdirSync(incoming, { recursive: true });
  const paths = [];
  for (const host of authority.hosts) {
    const directory = resolve(incoming, host.id);
    mkdirSync(directory, { recursive: true });
    const path = resolve(directory, "evidence.json");
    writeFileSync(path, `${JSON.stringify(evidence(host, directory), null, 2)}\n`);
    paths.push(path);
  }

  const passing = runMatrix();
  assert.equal(passing.status, 0, passing.stderr);
  const matrix = JSON.parse(readFileSync(output, "utf8"));
  assert.equal(matrix.result, "passed");
  assert.equal(matrix.hosts.length, authority.hosts.length);

  for (let index = 0; index < paths.length; index += 1) {
    const drifted = evidence(authority.hosts[index], dirname(paths[index]));
    drifted.source = { ...drifted.source, commit: "f".repeat(40) };
    writeFileSync(paths[index], `${JSON.stringify(drifted, null, 2)}\n`);
  }
  const checkoutDrift = runMatrix();
  assert.notEqual(checkoutDrift.status, 0);
  assert.match(checkoutDrift.stderr, /not bound to the aggregator checkout/u);

  for (let index = 0; index < paths.length; index += 1) {
    writeFileSync(
      paths[index],
      `${JSON.stringify(evidence(authority.hosts[index], dirname(paths[index])), null, 2)}\n`,
    );
  }
  const first = JSON.parse(readFileSync(paths[0], "utf8"));
  first.source = { ...first.source, dirty: true, statusEntryCount: 1 };
  writeFileSync(paths[0], `${JSON.stringify(first, null, 2)}\n`);
  const dirty = runMatrix();
  assert.notEqual(dirty.status, 0);
  assert.match(dirty.stderr, /source drifted|not clean-tree evidence/u);

  writeFileSync(paths[0], `${JSON.stringify(evidence(authority.hosts[0], dirname(paths[0])), null, 2)}\n`);
  const geometryFailure = JSON.parse(readFileSync(paths[0], "utf8"));
  geometryFailure.observations[0].computed.maximumLayoutGeometryDeltaCssPx =
    authority.comparison.maximumLayoutGeometryDeltaCssPx + 0.01;
  geometryFailure.summary.maximumLayoutGeometryDeltaCssPx =
    geometryFailure.observations[0].computed.maximumLayoutGeometryDeltaCssPx;
  writeFileSync(paths[0], `${JSON.stringify(geometryFailure, null, 2)}\n`);
  const geometry = runMatrix();
  assert.notEqual(geometry.status, 0);
  assert.match(geometry.stderr, /layout geometry failed/u);

  writeFileSync(paths[0], `${JSON.stringify(evidence(authority.hosts[0], dirname(paths[0])), null, 2)}\n`);
  writeFileSync(resolve(dirname(paths[0]), "diff.png"), "tampered");
  const tampered = runMatrix();
  assert.notEqual(tampered.status, 0);
  assert.match(tampered.stderr, /hash or size drifted/u);

  process.stdout.write("browser/output matrix fail-closed contract: pass\n");
} finally {
  rmSync(workRoot, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
}
