import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  AuthorityError,
  validateBrowserMatrix,
  validateReleaseReadiness,
} from "./release-authority.mjs";

const root = resolve(import.meta.dirname, "..");
const fixedNow = Date.parse("2026-07-22T15:00:00Z");
const readiness = JSON.parse(
  readFileSync(resolve(root, "docs/product/release-readiness-0.1.0.json"), "utf8"),
);
assert(!Object.hasOwn(readiness.dimensions, "adoptionReady"));
assert(!readiness.checks.some((check) => check.dimension === "adoption"));
const readyCandidate = structuredClone(readiness);
for (const check of readyCandidate.checks) {
  check.status = "passed";
  check.evidenceClass = "measured";
  check.coveredCoverage = check.requiredCoverage;
  check.waiverAdr = null;
  check.artifacts = [
    {
      path: "docs/product/evidence/v0.1.0-rc.2-source.json",
      url: "https://github.com/celiumsai/pliegocss/blob/main/docs/product/evidence/v0.1.0-rc.2-source.json",
      sha256: "sha256:51afb97c3b86fbd34ad7d827e7fc5347f1e8d85e5dd1456e44d0c9389a82fecb",
    },
  ];
  check.references = [
    {
      kind: "contract-test",
      url: "https://github.com/celiumsai/pliegocss",
    },
  ];
}
Object.assign(readyCandidate.dimensions, {
  technicalReady: true,
  operationallyReady: true,
  authorized: true,
  promotionReady: true,
});
readyCandidate.result = "ready";
assert.equal(
  validateReleaseReadiness(readyCandidate, { root, now: fixedNow, verifyGit: false }).result,
  "ready",
);
const optionalBlocker = structuredClone(readiness);
optionalBlocker.checks[0].requiredForPromotion = false;
assert.throws(
  () => validateReleaseReadiness(optionalBlocker, { root, now: fixedNow, verifyGit: false }),
  (error) => error instanceof AuthorityError && /requiredForPromotion must be true/u.test(error.message),
);
const oldSchema = structuredClone(readiness);
oldSchema.schemaVersion = 3;
assert.throws(
  () => validateReleaseReadiness(oldSchema, { root, now: fixedNow, verifyGit: false }),
  (error) => error instanceof AuthorityError && /header drifted/u.test(error.message),
);
const browser = JSON.parse(
  readFileSync(resolve(root, "docs/benchmarks/hosted-browser-matrix.json"), "utf8"),
);
assert.equal(
  validateReleaseReadiness(readiness, { root, now: fixedNow, verifyGit: false }).result,
  "blocked",
);
assert.equal(
  validateBrowserMatrix(browser, { root, now: fixedNow, verifyGit: false }).result,
  "blocked",
);

const unbound = structuredClone(readiness);
unbound.checks[0].source.commit = "0".repeat(40);
assert.throws(
  () => validateReleaseReadiness(unbound, { root, now: fixedNow, verifyGit: false }),
  (error) => error instanceof AuthorityError && /not bound/u.test(error.message),
);

const artifactless = structuredClone(readiness);
artifactless.checks[0].status = "passed";
artifactless.checks[0].evidenceClass = "measured";
artifactless.checks[0].artifacts = [];
assert.throws(
  () => validateReleaseReadiness(artifactless, { root, now: fixedNow, verifyGit: false }),
  (error) => error instanceof AuthorityError && /hashed artifact/u.test(error.message),
);

const externalWaiver = structuredClone(readiness);
externalWaiver.checks[2].waiverAdr = "docs/index.md";
assert.throws(
  () => validateReleaseReadiness(externalWaiver, { root, now: fixedNow, verifyGit: false }),
  (error) => error instanceof AuthorityError && /inside docs\/adr/u.test(error.message),
);

const optimisticBrowser = structuredClone(browser);
Object.assign(optimisticBrowser.hosts[0], {
  status: "passed",
  automation: "passed",
  browserVersion: "150.0.0",
  observedAt: "2026-07-22T14:10:00Z",
  expiresAt: "2026-07-29T14:10:00Z",
  coveredCoverage: optimisticBrowser.hosts[0].requiredCoverage,
});
assert.throws(
  () => validateBrowserMatrix(optimisticBrowser, { root, now: fixedNow, verifyGit: false }),
  (error) => error instanceof AuthorityError && /hashed artifacts/u.test(error.message),
);

for (const script of ["check-release-readiness.mjs", "check-hosted-browser-matrix.mjs"]) {
  const result = spawnSync(process.execPath, [resolve(root, "scripts", script), "--profile=release"], {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
  });
  assert.equal(result.status, 1, `${script} must block the release profile`);
  assert.match(result.stderr, /blocked/u);
}

process.stdout.write("release authority fail-closed contracts: pass\n");
