import assert from "node:assert/strict";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  ExternalAdoptionError,
  loadExternalAdoptionBundle,
  validateExternalAdoptionBundle,
} from "./external-adoption-v1.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const current = loadExternalAdoptionBundle(root);
const status = validateExternalAdoptionBundle(current, { root });
assert.equal(status.result, "blocked");
assert.deepEqual(status.counts, {
  qualifiedInterviews: 0,
  qualifiedIncidents: 0,
  qualifiedPilots: 0,
  independentProjects: 0,
  independentOrganizations: 0,
});
assert.deepEqual(status.coveredCoverage, ["frozen-adapter-support-policy"]);
assert(status.missingCoverage.includes("ten-to-fifteen-interviews"));
assert(status.missingCoverage.includes("twenty-incidents"));
assert(status.missingCoverage.includes("three-to-five-pilots"));

function clone() {
  return structuredClone(current);
}

function hex(value, width) {
  return value.toString(16).padStart(width, "0");
}

const ready = clone();
for (let index = 1; index <= 3; index += 1) {
  ready.pilots.records.push({
    id: `pilot-${index}`,
    projectId: `project-${index}`,
    organizationId: `organization-${index}`,
    repositoryOwner: `external-owner-${index}`,
    relationship: "external-community",
    adapter: ["html", "vite", "pliegors"][index - 1],
    tailwindProfile: index === 1 ? "tailwind-v3-lts" : "tailwind-v4-current",
    startedAt: "2026-07-01T00:00:00Z",
    completedAt: "2026-07-02T00:00:00Z",
    status: "completed",
    maintainerIds: [`maintainer-${index}`],
    consent: {
      status: "recorded",
      recordedAt: "2026-06-30T00:00:00Z",
      researchUse: true,
      publicRedactedRecord: true,
    },
    source: {
      url: `https://github.com/external-owner-${index}/project-${index}`,
      commit: hex(index, 40),
      sha256: `sha256:${hex(100 + index, 64)}`,
    },
    outcomes: {
      adoptionCompleted: true,
      rollbackVerified: true,
      browserEquivalenceVerified: true,
      blockingFailures: 0,
    },
    review: {
      reviewer: `reviewer-${index}`,
      reviewedAt: "2026-07-03T00:00:00Z",
      status: "approved",
    },
  });
}
for (let index = 1; index <= 20; index += 1) {
  ready.incidents.records.push({
    id: `incident-${index}`,
    projectId: `project-${((index - 1) % 3) + 1}`,
    organizationId: `organization-${((index - 1) % 3) + 1}`,
    relationship: "external-community",
    occurredAt: "2026-06-20T00:00:00Z",
    framework: index % 2 === 0 ? "Vite" : "Static HTML",
    category: "cascade-conflict",
    severity: "medium",
    synthetic: false,
    provenance: "external-project-observation",
    consent: {
      status: "recorded",
      recordedAt: "2026-06-21T00:00:00Z",
      researchUse: true,
      publicRedactedRecord: true,
    },
    redaction: { status: "reviewed", containsPersonalData: false },
    expectedDiagnosis: "Detect the observed cascade conflict.",
    allowedAmbiguity: "Equivalent selector wording is allowed.",
    source: {
      custodyRef: `private:g7/incidents/incident-${index}`,
      sha256: `sha256:${hex(200 + index, 64)}`,
    },
    review: {
      reviewer: `reviewer-${((index - 1) % 3) + 1}`,
      reviewedAt: "2026-06-22T00:00:00Z",
      status: "approved",
    },
  });
}
for (let index = 1; index <= 10; index += 1) {
  ready.interviews.records.push({
    id: `interview-${index}`,
    participantId: `participant-${index}`,
    organizationId: `organization-${index}`,
    relationship: "external-community",
    conductedAt: "2026-07-04T00:00:00Z",
    consent: {
      status: "recorded",
      recordedAt: "2026-07-03T00:00:00Z",
      researchUse: true,
      publicRedactedRecord: true,
    },
    redaction: { status: "reviewed", containsPersonalData: false },
    source: {
      custodyRef: `private:g7/interviews/interview-${index}`,
      sha256: `sha256:${hex(300 + index, 64)}`,
    },
    review: {
      reviewer: `reviewer-${((index - 1) % 3) + 1}`,
      reviewedAt: "2026-07-05T00:00:00Z",
      status: "approved",
    },
    findings: ["A reviewed external finding."],
    relatedIncidentIds: index <= 3 ? [`incident-${index}`] : [],
    relatedPilotIds: index <= 3 ? [`pilot-${index}`] : [],
  });
}
const readyStatus = validateExternalAdoptionBundle(ready, { root, verifyFiles: false });
assert.equal(readyStatus.result, "ready");
assert.deepEqual(readyStatus.missingCoverage, []);

const internalPilot = clone();
internalPilot.pilots.records.push({
  id: "pilot-internal",
  projectId: "project-internal",
  organizationId: "celiums-solutions",
  repositoryOwner: "celiumsai",
  relationship: "external-community",
  adapter: "html",
  tailwindProfile: "tailwind-v4-current",
  startedAt: "2026-07-01T00:00:00Z",
  completedAt: "2026-07-02T00:00:00Z",
  status: "completed",
  maintainerIds: ["maintainer-one"],
  consent: {
    status: "recorded",
    recordedAt: "2026-07-02T00:00:00Z",
    researchUse: true,
    publicRedactedRecord: true,
  },
  source: {
    url: "https://github.com/celiumsai/pliegocss",
    commit: "0000000000000000000000000000000000000000",
    sha256: `sha256:${"0".repeat(64)}`,
  },
  outcomes: {
    adoptionCompleted: true,
    rollbackVerified: true,
    browserEquivalenceVerified: true,
    blockingFailures: 0,
  },
  review: {
    reviewer: "reviewer-one",
    reviewedAt: "2026-07-02T00:00:00Z",
    status: "approved",
  },
});
assert.throws(
  () => validateExternalAdoptionBundle(internalPilot, { root, verifyFiles: false }),
  (error) =>
    error instanceof ExternalAdoptionError &&
    /excluded repository owner|excluded organization/u.test(error.message),
);

const syntheticIncident = clone();
syntheticIncident.incidents.records.push({
  id: "incident-synthetic",
  projectId: "project-external",
  organizationId: "organization-external",
  relationship: "external-community",
  occurredAt: "2026-07-01T00:00:00Z",
  framework: "Vite",
  category: "cascade-conflict",
  severity: "medium",
  synthetic: true,
  provenance: "external-project-observation",
  consent: {
    status: "recorded",
    recordedAt: "2026-07-02T00:00:00Z",
    researchUse: true,
    publicRedactedRecord: true,
  },
  redaction: { status: "reviewed", containsPersonalData: false },
  expectedDiagnosis: "Detect the cascade conflict.",
  allowedAmbiguity: "Equivalent selector wording is allowed.",
  source: {
    custodyRef: "private:g7/incidents/incident-synthetic",
    sha256: `sha256:${"1".repeat(64)}`,
  },
  review: {
    reviewer: "reviewer-one",
    reviewedAt: "2026-07-02T00:00:00Z",
    status: "approved",
  },
});
assert.throws(
  () => validateExternalAdoptionBundle(syntheticIncident, { root, verifyFiles: false }),
  (error) => error instanceof ExternalAdoptionError && /not a real external incident/u.test(error.message),
);

const unconsentedInterview = clone();
unconsentedInterview.interviews.records.push({
  id: "interview-one",
  participantId: "participant-one",
  organizationId: "organization-external",
  relationship: "external-community",
  conductedAt: "2026-07-01T00:00:00Z",
  consent: {
    status: "recorded",
    recordedAt: "2026-07-01T00:00:00Z",
    researchUse: true,
    publicRedactedRecord: false,
  },
  redaction: { status: "reviewed", containsPersonalData: false },
  source: {
    custodyRef: "private:g7/interviews/interview-one",
    sha256: `sha256:${"2".repeat(64)}`,
  },
  review: {
    reviewer: "reviewer-one",
    reviewedAt: "2026-07-02T00:00:00Z",
    status: "approved",
  },
  findings: ["A real finding."],
  relatedIncidentIds: [],
  relatedPilotIds: [],
});
assert.throws(
  () => validateExternalAdoptionBundle(unconsentedInterview, { root, verifyFiles: false }),
  (error) => error instanceof ExternalAdoptionError && /must permit research use/u.test(error.message),
);

process.stdout.write("external adoption authority contract: pass\n");
