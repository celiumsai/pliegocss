import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { isAbsolute, relative, resolve } from "node:path";

const COMMIT = /^[0-9a-f]{40}$/u;
const DIGEST = /^sha256:[0-9a-f]{64}$/u;
const INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/u;

export class AuthorityError extends Error {
  constructor(message) {
    super(message);
    this.name = "AuthorityError";
  }
}

function fail(message) {
  throw new AuthorityError(message);
}

function object(value, role) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    fail(`${role} must be an object`);
  }
  return value;
}

function exactKeys(value, keys, role) {
  object(value, role);
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${role} fields drifted: ${actual.join(", ")}`);
  }
}

function strings(value, role) {
  if (!Array.isArray(value) || value.some((entry) => typeof entry !== "string" || !entry)) {
    fail(`${role} must be an array of non-empty strings`);
  }
  if (new Set(value).size !== value.length) fail(`${role} contains duplicates`);
  return value;
}

function instant(value, role) {
  if (typeof value !== "string" || !INSTANT.test(value) || Number.isNaN(Date.parse(value))) {
    fail(`${role} must be a canonical UTC second instant`);
  }
  return Date.parse(value);
}

function source(value, role, { tag = false } = {}) {
  const keys = tag ? ["tag", "commit", "gitTree"] : ["commit", "gitTree"];
  exactKeys(value, keys, role);
  if (!COMMIT.test(value.commit) || !COMMIT.test(value.gitTree)) {
    fail(`${role} commit and Git tree must be full lowercase object IDs`);
  }
  if (tag && !/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/u.test(value.tag)) {
    fail(`${role}.tag is not a release tag`);
  }
  return value;
}

function sourceEquals(left, right) {
  return left.commit === right.commit && left.gitTree === right.gitTree;
}

function localPath(root, value, role) {
  if (
    typeof value !== "string" ||
    !value ||
    isAbsolute(value) ||
    value.includes("\\") ||
    value.split("/").includes("..")
  ) {
    fail(`${role} must be a portable repository-relative path`);
  }
  const absolute = resolve(root, value);
  const child = relative(root, absolute);
  if (!child || child.startsWith("..") || isAbsolute(child)) {
    fail(`${role} escapes the repository`);
  }
  if (!existsSync(absolute)) fail(`${role} does not exist: ${value}`);
  return absolute;
}

function artifact(root, value, role) {
  exactKeys(value, ["path", "url", "sha256"], role);
  if (typeof value.url !== "string" || !value.url.startsWith("https://")) {
    fail(`${role}.url must use HTTPS`);
  }
  if (!DIGEST.test(value.sha256)) fail(`${role}.sha256 is not canonical`);
  const path = localPath(root, value.path, `${role}.path`);
  const actual = `sha256:${createHash("sha256").update(readFileSync(path)).digest("hex")}`;
  if (actual !== value.sha256) fail(`${role} hash drifted for ${value.path}`);
}

function reference(value, role) {
  exactKeys(value, ["kind", "url"], role);
  if (typeof value.kind !== "string" || !value.kind) fail(`${role}.kind is missing`);
  if (typeof value.url !== "string" || !value.url.startsWith("https://")) {
    fail(`${role}.url must use HTTPS`);
  }
}

function environment(value, role) {
  exactKeys(value, ["os", "arch", "rust", "node", "browser"], role);
  for (const key of Object.keys(value)) {
    if (value[key] !== null && (typeof value[key] !== "string" || !value[key])) {
      fail(`${role}.${key} must be a non-empty string or null`);
    }
  }
}

function waiver(root, value, role) {
  if (value === null) return;
  const path = localPath(root, value, role);
  const child = relative(resolve(root, "docs", "adr"), path);
  if (!child || child.startsWith("..") || isAbsolute(child)) {
    fail(`${role} must point inside docs/adr`);
  }
}

function verifyTag(root, value) {
  const commands = [
    [["rev-parse", `${value.tag}^{commit}`], value.commit, "tag commit"],
    [["show", "-s", "--format=%T", value.commit], value.gitTree, "Git tree"],
  ];
  for (const [args, expected, role] of commands) {
    const result = spawnSync("git", args, { cwd: root, encoding: "utf8", windowsHide: true });
    if (result.error || result.status !== 0) {
      fail(`cannot verify ${role}: ${result.error?.message ?? result.stderr.trim()}`);
    }
    if (result.stdout.trim() !== expected) fail(`${role} does not match the repository`);
  }
}

function validWindow(observedAt, expiresAt, role, now) {
  const observed = instant(observedAt, `${role}.observedAt`);
  const expires = instant(expiresAt, `${role}.expiresAt`);
  if (expires <= observed) fail(`${role} expires before it is observed`);
  return now <= expires;
}

function completeCoverage(required, covered) {
  const actual = new Set(covered);
  return required.every((item) => actual.has(item));
}

export function validateReleaseReadiness(value, { root, now = Date.now(), verifyGit = true }) {
  exactKeys(
    value,
    [
      "schemaVersion",
      "kind",
      "state",
      "targetVersion",
      "candidateVersion",
      "repository",
      "productStage",
      "source",
      "observedAt",
      "expiresAt",
      "result",
      "dimensions",
      "checks",
      "promotionRule",
    ],
    "readiness",
  );
  if (
    value.schemaVersion !== 3 ||
    value.kind !== "pliegocss-release-readiness" ||
    value.state !== "current" ||
    value.targetVersion !== "0.1.0" ||
    value.candidateVersion !== "0.1.0-rc.2" ||
    value.repository !== "https://github.com/celiumsai/pliegocss" ||
    value.productStage !== "public-preview"
  ) {
    fail("readiness header drifted");
  }
  const rootSource = source(value.source, "readiness.source", { tag: true });
  if (verifyGit) verifyTag(root, rootSource);
  validWindow(value.observedAt, value.expiresAt, "readiness", now);
  exactKeys(
    value.dimensions,
    ["technicalReady", "operationallyReady", "adoptionReady", "authorized", "promotionReady"],
    "readiness.dimensions",
  );
  if (!Array.isArray(value.checks)) fail("readiness.checks must be an array");
  const requiredIds = new Set([
    "exact-candidate-ci",
    "exact-candidate-codeql",
    "g0-corrections-on-candidate",
    "browser-release-matrix",
    "adapter-coexistence-matrix",
    "registry-replay-refresh",
    "production-deployment-refresh",
    "signed-binary-distribution",
    "external-adoption-evidence",
    "final-promotion-authorization",
  ]);
  const ids = value.checks.map((check) => check.id);
  if (new Set(ids).size !== ids.length) fail("readiness check IDs are not unique");
  if (ids.length !== requiredIds.size || ids.some((id) => !requiredIds.has(id))) {
    fail("readiness check inventory drifted");
  }
  const allowedDimensions = new Set(["technical", "operational", "adoption", "authorization"]);
  const allowedStatuses = new Set(["passed", "pending", "blocked", "not-authorized", "not-applicable"]);
  const allowedEvidence = new Set(["measured", "inherited", "pending", "uncertain"]);
  for (const check of value.checks) {
    const role = `readiness.checks.${check.id}`;
    exactKeys(
      check,
      [
        "id",
        "dimension",
        "status",
        "evidenceClass",
        "requiredForPromotion",
        "summary",
        "source",
        "observedAt",
        "expiresAt",
        "environment",
        "requiredCoverage",
        "coveredCoverage",
        "waiverAdr",
        "artifacts",
        "references",
      ],
      role,
    );
    if (!allowedDimensions.has(check.dimension)) fail(`${role}.dimension is invalid`);
    if (!allowedStatuses.has(check.status)) fail(`${role}.status is invalid`);
    if (!allowedEvidence.has(check.evidenceClass)) fail(`${role}.evidenceClass is invalid`);
    if (typeof check.requiredForPromotion !== "boolean") fail(`${role}.requiredForPromotion must be boolean`);
    if (typeof check.summary !== "string" || !check.summary) fail(`${role}.summary is missing`);
    const checkSource = source(check.source, `${role}.source`);
    if (!sourceEquals(checkSource, rootSource)) fail(`${role} is not bound to the candidate source`);
    const unexpired = validWindow(check.observedAt, check.expiresAt, role, now);
    environment(check.environment, `${role}.environment`);
    const required = strings(check.requiredCoverage, `${role}.requiredCoverage`);
    const covered = strings(check.coveredCoverage, `${role}.coveredCoverage`);
    waiver(root, check.waiverAdr, `${role}.waiverAdr`);
    if (!Array.isArray(check.artifacts)) fail(`${role}.artifacts must be an array`);
    check.artifacts.forEach((entry, index) => artifact(root, entry, `${role}.artifacts[${index}]`));
    if (!Array.isArray(check.references)) fail(`${role}.references must be an array`);
    check.references.forEach((entry, index) => reference(entry, `${role}.references[${index}]`));
    if (check.status === "passed") {
      if (check.evidenceClass !== "measured") fail(`${role} passed without measured evidence`);
      if (!unexpired) fail(`${role} passed on expired evidence`);
      if (!completeCoverage(required, covered)) fail(`${role} passed with incomplete coverage`);
      if (check.waiverAdr !== null) fail(`${role} passed through a waiver`);
      if (check.artifacts.length === 0) fail(`${role} passed without a hashed artifact`);
      if (check.references.length === 0) fail(`${role} passed without a source URL`);
    }
  }
  const dimension = (name) =>
    value.checks
      .filter((check) => check.requiredForPromotion && check.dimension === name)
      .every((check) => check.status === "passed");
  const derived = {
    technicalReady: dimension("technical"),
    operationallyReady: dimension("operational"),
    adoptionReady: dimension("adoption"),
    authorized: dimension("authorization"),
  };
  derived.promotionReady = Object.values(derived).every(Boolean);
  if (JSON.stringify(value.dimensions) !== JSON.stringify(derived)) {
    fail("readiness dimensions do not match the required checks");
  }
  const result = derived.promotionReady ? "ready" : "blocked";
  if (value.result !== result) fail("readiness result does not match its dimensions");
  if (typeof value.promotionRule !== "string" || !value.promotionRule) {
    fail("readiness promotion rule is missing");
  }
  return {
    schemaVersion: value.schemaVersion,
    candidate: value.candidateVersion,
    sourceCommit: rootSource.commit,
    gitTree: rootSource.gitTree,
    result,
    dimensions: derived,
    blockers: value.checks
      .filter((check) => check.requiredForPromotion && check.status !== "passed")
      .map((check) => check.id),
  };
}

export function validateBrowserMatrix(value, { root, now = Date.now(), verifyGit = true }) {
  exactKeys(
    value,
    [
      "schemaVersion",
      "kind",
      "state",
      "source",
      "observedAt",
      "expiresAt",
      "profiles",
      "hosts",
      "result",
      "missingRequiredHosts",
      "releaseBoundary",
    ],
    "browserMatrix",
  );
  if (
    value.schemaVersion !== 2 ||
    value.kind !== "pliegocss-hosted-browser-matrix" ||
    value.state !== "current"
  ) {
    fail("browser matrix header drifted");
  }
  const rootSource = source(value.source, "browserMatrix.source", { tag: true });
  if (verifyGit) verifyTag(root, rootSource);
  validWindow(value.observedAt, value.expiresAt, "browserMatrix", now);
  exactKeys(value.profiles, ["release"], "browserMatrix.profiles");
  exactKeys(
    value.profiles.release,
    ["requiredHosts", "allowNotConfigured", "allowStale", "allowWaivers"],
    "browserMatrix.profiles.release",
  );
  const requiredHosts = strings(
    value.profiles.release.requiredHosts,
    "browserMatrix.profiles.release.requiredHosts",
  );
  if (
    value.profiles.release.allowNotConfigured !== false ||
    value.profiles.release.allowStale !== false ||
    value.profiles.release.allowWaivers !== false
  ) {
    fail("release browser profile must fail closed");
  }
  if (!Array.isArray(value.hosts)) fail("browserMatrix.hosts must be an array");
  const ids = value.hosts.map((host) => host.id);
  if (new Set(ids).size !== ids.length) fail("browser host IDs are not unique");
  if (ids.length !== requiredHosts.length || requiredHosts.some((id) => !ids.includes(id))) {
    fail("browser host inventory does not match release coverage");
  }
  const allowedStatuses = new Set(["passed", "failed", "not-configured", "stale"]);
  for (const host of value.hosts) {
    const role = `browserMatrix.hosts.${host.id}`;
    exactKeys(
      host,
      [
        "id",
        "os",
        "arch",
        "browser",
        "browserVersion",
        "status",
        "sourceCommit",
        "gitTree",
        "observedAt",
        "expiresAt",
        "automation",
        "requiredCoverage",
        "coveredCoverage",
        "waiverAdr",
        "artifacts",
      ],
      role,
    );
    for (const key of ["id", "os", "arch", "browser", "automation"]) {
      if (typeof host[key] !== "string" || !host[key]) fail(`${role}.${key} is missing`);
    }
    if (!allowedStatuses.has(host.status)) fail(`${role}.status is invalid`);
    if (host.sourceCommit !== rootSource.commit || host.gitTree !== rootSource.gitTree) {
      fail(`${role} is not bound to the candidate source`);
    }
    const required = strings(host.requiredCoverage, `${role}.requiredCoverage`);
    const covered = strings(host.coveredCoverage, `${role}.coveredCoverage`);
    waiver(root, host.waiverAdr, `${role}.waiverAdr`);
    if (!Array.isArray(host.artifacts)) fail(`${role}.artifacts must be an array`);
    host.artifacts.forEach((entry, index) => artifact(root, entry, `${role}.artifacts[${index}]`));
    if (host.status === "passed") {
      if (host.automation !== "passed") fail(`${role} passed without automation`);
      if (typeof host.browserVersion !== "string" || !host.browserVersion) {
        fail(`${role} passed without a browser version`);
      }
      if (host.observedAt === null || host.expiresAt === null) fail(`${role} passed without a validity window`);
      if (!validWindow(host.observedAt, host.expiresAt, role, now)) fail(`${role} passed on expired evidence`);
      if (!completeCoverage(required, covered)) fail(`${role} passed with incomplete coverage`);
      if (host.waiverAdr !== null) fail(`${role} passed through a waiver`);
      if (host.artifacts.length === 0) fail(`${role} passed without hashed artifacts`);
    } else if (host.status === "not-configured") {
      if (
        host.automation !== "not-configured" ||
        host.browserVersion !== null ||
        host.observedAt !== null ||
        host.expiresAt !== null ||
        covered.length !== 0 ||
        host.artifacts.length !== 0 ||
        host.waiverAdr !== null
      ) {
        fail(`${role} claims evidence while not configured`);
      }
    }
  }
  const missing = requiredHosts.filter(
    (id) => value.hosts.find((host) => host.id === id)?.status !== "passed",
  );
  if (JSON.stringify(value.missingRequiredHosts) !== JSON.stringify(missing)) {
    fail("browser missing-host projection drifted");
  }
  const result = missing.length === 0 ? "ready" : "blocked";
  if (value.result !== result) fail("browser result does not match required hosts");
  if (typeof value.releaseBoundary !== "string" || !value.releaseBoundary) {
    fail("browser release boundary is missing");
  }
  return {
    schemaVersion: value.schemaVersion,
    sourceCommit: rootSource.commit,
    gitTree: rootSource.gitTree,
    result,
    passed: value.hosts.filter((host) => host.status === "passed").map((host) => host.id),
    missingRequiredHosts: missing,
  };
}
