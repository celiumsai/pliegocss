import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const path = resolve(root, "docs/product/release-readiness-0.1.0.json");
if (!existsSync(path)) throw new Error("release readiness document is missing");

const value = JSON.parse(readFileSync(path, "utf8"));
const fail = (message) => {
  throw new Error(`release readiness: ${message}`);
};
const requiredChecks = new Set([
  "targeted-corrections",
  "brand-system",
  "fast-profile-current-source",
  "pliegors-current-contract",
  "browser-current-source",
  "package-replay-current-source",
  "benchmark-evidence-current-source",
  "supply-chain-policy",
  "website-current-source",
  "hosted-cross-os-current-source",
  "registry-replay",
  "final-promotion-authorization",
]);
const allowedStatuses = new Set([
  "passed",
  "pending",
  "blocked",
  "not-authorized",
  "not-applicable",
]);
const allowedEvidence = new Set(["measured", "inherited", "pending", "uncertain"]);

if (
  value.schemaVersion !== 2 ||
  value.targetVersion !== "0.1.0" ||
  value.candidateVersion !== "0.1.0-rc.2"
) {
  fail("header drifted");
}
if (value.repository !== "https://github.com/celiumsai/pliegocss") {
  fail("repository drifted");
}
if (
  value.repositoryVisibility !== "private" ||
  value.productStage !== "public-preview"
) {
  fail("repository visibility and public-preview product stage drifted");
}
if (!Array.isArray(value.checks)) fail("checks must be an array");
const ids = value.checks.map((check) => check.id);
if (new Set(ids).size !== ids.length) fail("check identifiers must be unique");
for (const id of requiredChecks) {
  if (!ids.includes(id)) fail(`missing required check ${id}`);
}
for (const check of value.checks) {
  if (!allowedStatuses.has(check.status)) fail(`invalid status for ${check.id}`);
  if (!allowedEvidence.has(check.evidenceClass)) {
    fail(`invalid evidence class for ${check.id}`);
  }
  if (check.status === "passed" && check.evidenceClass === "pending") {
    fail(`${check.id} cannot pass on pending evidence`);
  }
  if (!check.summary || typeof check.summary !== "string") {
    fail(`${check.id} must include a summary`);
  }
}
const blocking = value.checks.filter(
  (check) => check.requiredForPromotion && check.status !== "passed",
);
const derivedResult = blocking.length === 0 ? "ready" : "blocked";
if (value.result !== derivedResult) fail("result does not match required checks");
if (!value.promotionRule || typeof value.promotionRule !== "string") {
  fail("promotion rule is missing");
}

process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: value.schemaVersion,
      candidate: value.candidateVersion,
      result: value.result,
      passed: value.checks.filter((check) => check.status === "passed").length,
      blocking: blocking.map((check) => check.id),
    },
    null,
    2,
  )}\n`,
);
