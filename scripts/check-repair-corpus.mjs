import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const root = resolve(import.meta.dirname, "..");
const corpusPath = resolve(root, "benchmarks", "repair-corpus", "cases.json");
const corpusBytes = readFileSync(corpusPath);
const corpus = JSON.parse(corpusBytes.toString("utf8"));
const reportPrefix = "PLIEGOCSS_REPAIR_CORPUS_REPORT ";

function fail(message) {
  throw new Error(message);
}

function requireKeys(value, expected, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    fail(`${label} fields drifted: ${actual.join(", ")}`);
  }
}

requireKeys(
  corpus,
  ["schemaVersion", "corpusId", "provenance", "claimBoundary", "cases"],
  "corpus",
);
if (
  corpus.schemaVersion !== 1 ||
  corpus.corpusId !== "repair-authority-conformance-v1" ||
  corpus.claimBoundary !== "conformance-only-not-real-incident-or-agent-turn-evidence"
) {
  fail("repair corpus identity or claim boundary drifted");
}
requireKeys(corpus.provenance, ["kind", "source", "consent", "redaction"], "provenance");
if (
  corpus.provenance.kind !== "synthetic" ||
  corpus.provenance.source !== "engineered from the public repair contract" ||
  corpus.provenance.consent !== "not-applicable" ||
  corpus.provenance.redaction !== "not-required"
) {
  fail("repair corpus provenance drifted");
}
if (!Array.isArray(corpus.cases) || corpus.cases.length !== 16) {
  fail("repair corpus must retain exactly 16 synthetic conformance cases");
}
const ids = [];
for (const [index, testCase] of corpus.cases.entries()) {
  requireKeys(
    testCase,
    ["id", "category", "mutation", "expected", "reasonContains"],
    `cases[${index}]`,
  );
  for (const field of ["id", "category", "mutation", "reasonContains"]) {
    if (typeof testCase[field] !== "string" || testCase[field].length === 0) {
      fail(`cases[${index}].${field} must be nonempty`);
    }
  }
  if (!new Set(["accepted", "rejected"]).has(testCase.expected)) {
    fail(`cases[${index}].expected is unsupported`);
  }
  ids.push(testCase.id);
}
if (
  new Set(ids).size !== ids.length ||
  ids.some((id, index) => index > 0 && ids[index - 1] >= id)
) {
  fail("repair corpus case ids must be unique and sorted");
}
if (corpus.cases.filter((testCase) => testCase.expected === "accepted").length !== 1) {
  fail("repair corpus must retain one accepted control");
}

const result = spawnSync(
  "cargo",
  ["test", "--locked", "-p", "pliego-css-agent", "--test", "repair_corpus", "--", "--nocapture"],
  {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 16 * 1024 * 1024,
  },
);
if (result.error) {
  throw result.error;
}
if (result.status !== 0) {
  fail(`repair corpus execution failed\n${result.stdout}${result.stderr}`);
}
const marker = result.stdout
  .split(/\r?\n/u)
  .find((line) => line.startsWith(reportPrefix));
if (!marker) {
  fail("repair corpus test did not emit its canonical report marker");
}
const execution = JSON.parse(marker.slice(reportPrefix.length));
requireKeys(
  execution,
  ["schemaVersion", "corpusId", "claimBoundary", "total", "accepted", "rejected", "cases"],
  "execution report",
);
if (
  execution.schemaVersion !== 1 ||
  execution.corpusId !== corpus.corpusId ||
  execution.claimBoundary !== corpus.claimBoundary ||
  execution.total !== corpus.cases.length ||
  execution.accepted !== 1 ||
  execution.rejected !== corpus.cases.length - 1 ||
  !Array.isArray(execution.cases) ||
  execution.cases.length !== corpus.cases.length
) {
  fail("repair corpus execution summary drifted");
}
for (const [index, caseResult] of execution.cases.entries()) {
  requireKeys(
    caseResult,
    ["id", "category", "expected", "actual", "reasonMatched"],
    `execution.cases[${index}]`,
  );
  const expected = corpus.cases[index];
  if (
    caseResult.id !== expected.id ||
    caseResult.category !== expected.category ||
    caseResult.expected !== expected.expected ||
    caseResult.actual !== expected.expected ||
    caseResult.reasonMatched !== true
  ) {
    fail(`${expected.id}: execution result drifted`);
  }
}

process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 1,
      kind: "synthetic-repair-boundary-conformance",
      corpusId: corpus.corpusId,
      corpusSha256: createHash("sha256").update(corpusBytes).digest("hex"),
      claimBoundary: corpus.claimBoundary,
      total: execution.total,
      accepted: execution.accepted,
      rejected: execution.rejected,
      result: "passed",
      cases: execution.cases,
    },
    null,
    2,
  )}\n`,
);
