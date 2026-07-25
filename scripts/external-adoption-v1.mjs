import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { isAbsolute, relative, resolve } from "node:path";

const DATE = /^\d{4}-\d{2}-\d{2}$/u;
const INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/u;
const DIGEST = /^sha256:[0-9a-f]{64}$/u;
const HEX_DIGEST = /^[0-9a-f]{64}$/u;
const ID = /^[a-z0-9]+(?:-[a-z0-9]+)*$/u;

export class ExternalAdoptionError extends Error {
  constructor(message) {
    super(message);
    this.name = "ExternalAdoptionError";
  }
}

function fail(message) {
  throw new ExternalAdoptionError(message);
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

function nonEmpty(value, role) {
  if (typeof value !== "string" || value.trim() === "") fail(`${role} must be a non-empty string`);
  return value;
}

function canonicalId(value, role) {
  if (typeof value !== "string" || !ID.test(value)) fail(`${role} is not a canonical ID`);
  return value;
}

function strings(value, role, { nonEmptyArray = true } = {}) {
  if (
    !Array.isArray(value) ||
    (nonEmptyArray && value.length === 0) ||
    value.some((entry) => typeof entry !== "string" || entry.trim() === "")
  ) {
    fail(`${role} must be ${nonEmptyArray ? "a non-empty " : "an "}array of non-empty strings`);
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

function digest(value, role) {
  if (typeof value !== "string" || !DIGEST.test(value)) fail(`${role} is not a canonical SHA-256 digest`);
  if (value === `sha256:${"0".repeat(64)}`) fail(`${role} cannot be the zero digest`);
}

function repositoryPath(root, value, role) {
  if (
    typeof value !== "string" ||
    value === "" ||
    isAbsolute(value) ||
    value.includes("\\") ||
    value.split("/").includes("..")
  ) {
    fail(`${role} must be a portable repository-relative path`);
  }
  const absolute = resolve(root, value);
  const child = relative(root, absolute);
  if (!child || child.startsWith("..") || isAbsolute(child)) fail(`${role} escapes the repository`);
  if (!existsSync(absolute)) fail(`${role} does not exist: ${value}`);
  return absolute;
}

function hashedFile(root, value, role, verifyFiles) {
  exactKeys(value, ["path", "sha256"], role);
  if (!HEX_DIGEST.test(value.sha256)) fail(`${role}.sha256 is not canonical`);
  if (!verifyFiles) return;
  const path = repositoryPath(root, value.path, `${role}.path`);
  const actual = createHash("sha256").update(readFileSync(path)).digest("hex");
  if (actual !== value.sha256) fail(`${role} hash drifted for ${value.path}`);
}

function consent(value, role, requiredStatus) {
  exactKeys(value, ["status", "recordedAt", "researchUse", "publicRedactedRecord"], role);
  if (value.status !== requiredStatus) fail(`${role}.status must be ${requiredStatus}`);
  instant(value.recordedAt, `${role}.recordedAt`);
  if (value.researchUse !== true || value.publicRedactedRecord !== true) {
    fail(`${role} must permit research use and the public redacted record`);
  }
}

function redaction(value, role) {
  exactKeys(value, ["status", "containsPersonalData"], role);
  if (value.status !== "reviewed" || value.containsPersonalData !== false) {
    fail(`${role} must be reviewed and contain no personal data`);
  }
}

function source(value, role) {
  exactKeys(value, ["custodyRef", "sha256"], role);
  nonEmpty(value.custodyRef, `${role}.custodyRef`);
  digest(value.sha256, `${role}.sha256`);
}

function review(value, role, requiredStatus) {
  exactKeys(value, ["reviewer", "reviewedAt", "status"], role);
  canonicalId(value.reviewer, `${role}.reviewer`);
  instant(value.reviewedAt, `${role}.reviewedAt`);
  if (value.status !== requiredStatus) fail(`${role}.status must be ${requiredStatus}`);
}

function relationship(value, authority, organizationId, role) {
  if (!authority.eligibility.allowedRelationships.includes(value)) {
    fail(`${role} relationship is not external`);
  }
  if (authority.eligibility.excludedOrganizationIds.includes(organizationId)) {
    fail(`${role} uses an excluded organization`);
  }
}

function documentHeader(value, kind, studyId, role) {
  exactKeys(value, ["schemaVersion", "kind", "studyId", "records"], role);
  if (value.schemaVersion !== 1 || value.kind !== kind || value.studyId !== studyId) {
    fail(`${role} header drifted`);
  }
  if (!Array.isArray(value.records)) fail(`${role}.records must be an array`);
}

function validateInterviews(value, authority) {
  documentHeader(
    value,
    "pliegocss-external-adoption-interviews",
    authority.studyId,
    "interviews",
  );
  const ids = new Set();
  const participants = new Set();
  const sourceDigests = new Set();
  for (const [index, entry] of value.records.entries()) {
    const role = `interviews.records[${index}]`;
    exactKeys(
      entry,
      [
        "id",
        "participantId",
        "organizationId",
        "relationship",
        "conductedAt",
        "consent",
        "redaction",
        "source",
        "review",
        "findings",
        "relatedIncidentIds",
        "relatedPilotIds",
      ],
      role,
    );
    canonicalId(entry.id, `${role}.id`);
    canonicalId(entry.participantId, `${role}.participantId`);
    canonicalId(entry.organizationId, `${role}.organizationId`);
    if (ids.has(entry.id)) fail(`duplicate interview ID ${entry.id}`);
    if (participants.has(entry.participantId)) fail(`duplicate interview participant ${entry.participantId}`);
    ids.add(entry.id);
    participants.add(entry.participantId);
    relationship(entry.relationship, authority, entry.organizationId, role);
    const conductedAt = instant(entry.conductedAt, `${role}.conductedAt`);
    consent(entry.consent, `${role}.consent`, authority.eligibility.requiredConsentStatus);
    if (Date.parse(entry.consent.recordedAt) > conductedAt) {
      fail(`${role}.consent was recorded after the interview`);
    }
    redaction(entry.redaction, `${role}.redaction`);
    source(entry.source, `${role}.source`);
    if (sourceDigests.has(entry.source.sha256)) fail(`${role} duplicates another interview source`);
    sourceDigests.add(entry.source.sha256);
    review(entry.review, `${role}.review`, authority.eligibility.requiredReviewStatus);
    if (entry.review.reviewer === entry.participantId) fail(`${role} is self-reviewed`);
    if (Date.parse(entry.review.reviewedAt) < conductedAt) fail(`${role}.review predates the interview`);
    strings(entry.findings, `${role}.findings`);
    strings(entry.relatedIncidentIds, `${role}.relatedIncidentIds`, { nonEmptyArray: false });
    strings(entry.relatedPilotIds, `${role}.relatedPilotIds`, { nonEmptyArray: false });
  }
  return { count: value.records.length, ids };
}

function validateIncidents(value, authority) {
  documentHeader(
    value,
    "pliegocss-external-adoption-incidents",
    authority.studyId,
    "incidents",
  );
  const ids = new Set();
  const sourceDigests = new Set();
  for (const [index, entry] of value.records.entries()) {
    const role = `incidents.records[${index}]`;
    exactKeys(
      entry,
      [
        "id",
        "projectId",
        "organizationId",
        "relationship",
        "occurredAt",
        "framework",
        "category",
        "severity",
        "synthetic",
        "provenance",
        "consent",
        "redaction",
        "expectedDiagnosis",
        "allowedAmbiguity",
        "source",
        "review",
      ],
      role,
    );
    canonicalId(entry.id, `${role}.id`);
    canonicalId(entry.projectId, `${role}.projectId`);
    canonicalId(entry.organizationId, `${role}.organizationId`);
    if (ids.has(entry.id)) fail(`duplicate incident ID ${entry.id}`);
    ids.add(entry.id);
    relationship(entry.relationship, authority, entry.organizationId, role);
    const occurredAt = instant(entry.occurredAt, `${role}.occurredAt`);
    nonEmpty(entry.framework, `${role}.framework`);
    canonicalId(entry.category, `${role}.category`);
    if (!["low", "medium", "high", "critical"].includes(entry.severity)) {
      fail(`${role}.severity is invalid`);
    }
    if (entry.synthetic !== false || entry.provenance !== "external-project-observation") {
      fail(`${role} is not a real external incident`);
    }
    consent(entry.consent, `${role}.consent`, authority.eligibility.requiredConsentStatus);
    redaction(entry.redaction, `${role}.redaction`);
    nonEmpty(entry.expectedDiagnosis, `${role}.expectedDiagnosis`);
    nonEmpty(entry.allowedAmbiguity, `${role}.allowedAmbiguity`);
    source(entry.source, `${role}.source`);
    if (sourceDigests.has(entry.source.sha256)) fail(`${role} duplicates another incident source`);
    sourceDigests.add(entry.source.sha256);
    review(entry.review, `${role}.review`, authority.eligibility.requiredReviewStatus);
    if (Date.parse(entry.review.reviewedAt) < occurredAt) fail(`${role}.review predates the incident`);
  }
  return { count: value.records.length, ids };
}

function validatePilots(value, authority) {
  documentHeader(value, "pliegocss-external-adoption-pilots", authority.studyId, "pilots");
  const ids = new Set();
  const projects = new Set();
  const organizations = new Set();
  const sourceDigests = new Set();
  for (const [index, entry] of value.records.entries()) {
    const role = `pilots.records[${index}]`;
    exactKeys(
      entry,
      [
        "id",
        "projectId",
        "organizationId",
        "repositoryOwner",
        "relationship",
        "adapter",
        "tailwindProfile",
        "startedAt",
        "completedAt",
        "status",
        "maintainerIds",
        "consent",
        "source",
        "outcomes",
        "review",
      ],
      role,
    );
    canonicalId(entry.id, `${role}.id`);
    canonicalId(entry.projectId, `${role}.projectId`);
    canonicalId(entry.organizationId, `${role}.organizationId`);
    canonicalId(entry.repositoryOwner, `${role}.repositoryOwner`);
    if (ids.has(entry.id)) fail(`duplicate pilot ID ${entry.id}`);
    if (projects.has(entry.projectId)) fail(`duplicate pilot project ${entry.projectId}`);
    if (authority.eligibility.excludedRepositoryOwners.includes(entry.repositoryOwner)) {
      fail(`${role} is owned by an excluded repository owner`);
    }
    ids.add(entry.id);
    projects.add(entry.projectId);
    organizations.add(entry.organizationId);
    relationship(entry.relationship, authority, entry.organizationId, role);
    if (!["html", "vite", "pliegors"].includes(entry.adapter)) fail(`${role}.adapter is unsupported`);
    if (!["tailwind-v3-lts", "tailwind-v4-current"].includes(entry.tailwindProfile)) {
      fail(`${role}.tailwindProfile is unsupported`);
    }
    const started = instant(entry.startedAt, `${role}.startedAt`);
    const completed = instant(entry.completedAt, `${role}.completedAt`);
    if (completed < started || entry.status !== "completed") fail(`${role} is not a completed pilot`);
    strings(entry.maintainerIds, `${role}.maintainerIds`);
    consent(entry.consent, `${role}.consent`, authority.eligibility.requiredConsentStatus);
    if (Date.parse(entry.consent.recordedAt) > started) {
      fail(`${role}.consent was recorded after the pilot started`);
    }
    exactKeys(entry.source, ["url", "commit", "sha256"], `${role}.source`);
    if (typeof entry.source.url !== "string" || !entry.source.url.startsWith("https://")) {
      fail(`${role}.source.url must use HTTPS`);
    }
    if (
      !/^[0-9a-f]{40}$/u.test(entry.source.commit) ||
      entry.source.commit === "0".repeat(40)
    ) {
      fail(`${role}.source.commit is not canonical`);
    }
    digest(entry.source.sha256, `${role}.source.sha256`);
    if (sourceDigests.has(entry.source.sha256)) fail(`${role} duplicates another pilot source`);
    sourceDigests.add(entry.source.sha256);
    let sourceUrl;
    try {
      sourceUrl = new URL(entry.source.url);
    } catch {
      fail(`${role}.source.url is invalid`);
    }
    if (sourceUrl.hostname === "github.com") {
      const sourceOwner = sourceUrl.pathname.split("/").filter(Boolean)[0]?.toLowerCase();
      if (sourceOwner !== entry.repositoryOwner) {
        fail(`${role}.repositoryOwner does not match the GitHub evidence URL`);
      }
    }
    exactKeys(
      entry.outcomes,
      [
        "adoptionCompleted",
        "rollbackVerified",
        "browserEquivalenceVerified",
        "blockingFailures",
      ],
      `${role}.outcomes`,
    );
    if (
      entry.outcomes.adoptionCompleted !== true ||
      entry.outcomes.rollbackVerified !== true ||
      entry.outcomes.browserEquivalenceVerified !== true ||
      entry.outcomes.blockingFailures !== 0
    ) {
      fail(`${role}.outcomes do not satisfy the pilot protocol`);
    }
    review(entry.review, `${role}.review`, authority.eligibility.requiredReviewStatus);
    if (entry.maintainerIds.includes(entry.review.reviewer)) fail(`${role} is self-reviewed`);
    if (Date.parse(entry.review.reviewedAt) < completed) fail(`${role}.review predates completion`);
  }
  return { count: value.records.length, ids, projects, organizations };
}

function validatePolicy(value, coexistence, { root, verifyFiles }) {
  exactKeys(
    value,
    [
      "schemaVersion",
      "kind",
      "status",
      "releaseLine",
      "frozenAt",
      "protocol",
      "supportedTailwindProfiles",
      "supportedAdapters",
      "migration",
      "verification",
      "supportTiers",
      "claimBoundary",
    ],
    "adapterSupportPolicy",
  );
  if (
    value.schemaVersion !== 1 ||
    value.kind !== "pliegocss-adapter-support-policy" ||
    value.status !== "frozen-for-0.1-promotion" ||
    value.releaseLine !== "0.1.x" ||
    !DATE.test(value.frozenAt)
  ) {
    fail("adapterSupportPolicy header drifted");
  }
  exactKeys(value.protocol, ["name", "authoritySchema", "migrationInventorySchema"], "adapterSupportPolicy.protocol");
  if (
    value.protocol.name !== "static-complete-class-group-coexistence" ||
    value.protocol.authoritySchema !== 1 ||
    value.protocol.migrationInventorySchema !== 1
  ) {
    fail("adapterSupportPolicy protocol drifted");
  }
  if (!Array.isArray(value.supportedTailwindProfiles) || value.supportedTailwindProfiles.length !== 2) {
    fail("adapterSupportPolicy must freeze exactly two Tailwind profiles");
  }
  const profiles = new Map();
  for (const [index, profile] of value.supportedTailwindProfiles.entries()) {
    exactKeys(profile, ["id", "certifiedVersion", "sourceContract"], `adapterSupportPolicy.supportedTailwindProfiles[${index}]`);
    profiles.set(profile.id, profile);
  }
  if (
    profiles.get("tailwind-v3-lts")?.certifiedVersion !== "3.4.19" ||
    profiles.get("tailwind-v4-current")?.certifiedVersion !== "4.3.3"
  ) {
    fail("adapterSupportPolicy Tailwind versions drifted");
  }
  if (!Array.isArray(value.supportedAdapters) || value.supportedAdapters.length !== 3) {
    fail("adapterSupportPolicy must freeze exactly three adapters");
  }
  const adapters = new Map();
  for (const [index, adapter] of value.supportedAdapters.entries()) {
    exactKeys(adapter, ["id", "supportTier", "inputContract", "version"], `adapterSupportPolicy.supportedAdapters[${index}]`);
    if (adapter.supportTier !== "certified") fail(`${adapter.id} is not certified`);
    adapters.set(adapter.id, adapter);
  }
  if (
    adapters.get("html")?.version !== null ||
    adapters.get("html")?.inputContract !== "complete-static-html-document" ||
    adapters.get("vite")?.version !== "8.1.5" ||
    adapters.get("vite")?.inputContract !== "vite-production-build" ||
    adapters.get("pliegors")?.version !== "pliego-dom=0.0.2;pliego-ssg=0.0.2" ||
    adapters.get("pliegors")?.inputContract !==
      "rendered-static-html-plus-pinned-framework-browser-replay"
  ) {
    fail("adapterSupportPolicy adapter versions drifted");
  }
  exactKeys(
    value.migration,
    [
      "mode",
      "tailwindRuntimeRetained",
      "applyUnit",
      "rollback",
      "dynamicClasses",
      "arbitraryConfigOrPluginExecution",
      "tailwindRemoval",
    ],
    "adapterSupportPolicy.migration",
  );
  if (
    value.migration.mode !== "coexistence" ||
    value.migration.tailwindRuntimeRetained !== true ||
    value.migration.applyUnit !== "complete-literal-class-group" ||
    value.migration.rollback !== "exact-source-bytes" ||
    value.migration.dynamicClasses !== "unsupported-fail-closed" ||
    value.migration.arbitraryConfigOrPluginExecution !== "unsupported-fail-closed" ||
    value.migration.tailwindRemoval !== "not-certified"
  ) {
    fail("adapterSupportPolicy migration boundary drifted");
  }
  exactKeys(
    value.verification,
    ["requiredHosts", "requiredComparisons", "maximumLayoutGeometryDeltaCssPx"],
    "adapterSupportPolicy.verification",
  );
  strings(value.verification.requiredHosts, "adapterSupportPolicy.verification.requiredHosts");
  strings(value.verification.requiredComparisons, "adapterSupportPolicy.verification.requiredComparisons");
  if (
    JSON.stringify(value.verification.requiredComparisons) !==
      JSON.stringify(["dom", "aria", "computed-style", "layout-geometry", "exact-rollback"]) ||
    value.verification.maximumLayoutGeometryDeltaCssPx !== 0.1
  ) {
    fail("adapterSupportPolicy geometry tolerance drifted");
  }
  exactKeys(value.supportTiers, ["certified", "best-effort", "unsupported"], "adapterSupportPolicy.supportTiers");
  for (const [key, description] of Object.entries(value.supportTiers)) {
    nonEmpty(description, `adapterSupportPolicy.supportTiers.${key}`);
  }
  nonEmpty(value.claimBoundary, "adapterSupportPolicy.claimBoundary");

  if (
    !coexistence ||
    coexistence.schemaVersion !== 1 ||
    coexistence.kind !== "pliegocss-adapter-coexistence-certification" ||
    coexistence.status !== "active-contract"
  ) {
    fail("adapter coexistence authority is unavailable or incompatible");
  }
  const certifiedProfiles = new Map(
    coexistence.tailwindProfiles.map((profile) => [profile.id, profile.version]),
  );
  for (const profile of value.supportedTailwindProfiles) {
    const certified = coexistence.tailwindProfiles.find((entry) => entry.id === profile.id);
    if (
      certifiedProfiles.get(profile.id) !== profile.certifiedVersion ||
      certified?.sourceContract !== profile.sourceContract
    ) {
      fail(`adapterSupportPolicy ${profile.id} does not match the G6 authority`);
    }
  }
  const certifiedAdapters = new Map(coexistence.adapters.map((adapter) => [adapter.id, adapter]));
  if (certifiedAdapters.get("vite")?.version !== adapters.get("vite")?.version) {
    fail("adapterSupportPolicy Vite version does not match the G6 authority");
  }
  if (
    JSON.stringify(coexistence.hosts.map((host) => host.id)) !==
      JSON.stringify(value.verification.requiredHosts) ||
    coexistence.comparison.maximumLayoutGeometryDeltaCssPx !==
      value.verification.maximumLayoutGeometryDeltaCssPx
  ) {
    fail("adapterSupportPolicy verification matrix does not match the G6 authority");
  }
  if (
    coexistence.migration.tailwindRuntimeRetained !== value.migration.tailwindRuntimeRetained ||
    coexistence.migration.dynamicClassPolicy !== "reject" ||
    coexistence.migration.rollback !== "restore-exact-source-bytes"
  ) {
    fail("adapterSupportPolicy migration semantics do not match the G6 authority");
  }
  if (verifyFiles) {
    const manifest = readFileSync(
      resolve(root, "integration-tests", "pliegors-dev-loop", "Cargo.toml"),
      "utf8",
    );
    for (const dependency of ["pliego-dom", "pliego-ssg"]) {
      if (!new RegExp(`^${dependency} = "=0\\.0\\.2"$`, "mu").test(manifest)) {
        fail(`adapterSupportPolicy PliegoRS pin drifted from ${dependency}`);
      }
    }
  }
}

function validateAuthority(authority, { root, verifyFiles }) {
  exactKeys(
    authority,
    [
      "schemaVersion",
      "status",
      "kind",
      "studyId",
      "evidence",
      "adapterSupportPolicy",
      "thresholds",
      "eligibility",
      "requiredCoverage",
      "claimBoundary",
    ],
    "authority",
  );
  if (
    authority.schemaVersion !== 1 ||
    authority.status !== "active-contract" ||
    authority.kind !== "pliegocss-external-adoption-authority" ||
    authority.studyId !== "g7-external-adoption-0.1"
  ) {
    fail("authority header drifted");
  }
  exactKeys(authority.evidence, ["interviews", "incidents", "pilots"], "authority.evidence");
  for (const name of ["interviews", "incidents", "pilots"]) {
    hashedFile(root, authority.evidence[name], `authority.evidence.${name}`, verifyFiles);
  }
  hashedFile(root, authority.adapterSupportPolicy, "authority.adapterSupportPolicy", verifyFiles);
  exactKeys(authority.thresholds, ["interviews", "incidents", "pilots"], "authority.thresholds");
  exactKeys(authority.thresholds.interviews, ["minimumQualified", "targetMaximum"], "authority.thresholds.interviews");
  exactKeys(authority.thresholds.incidents, ["minimumQualified"], "authority.thresholds.incidents");
  exactKeys(
    authority.thresholds.pilots,
    ["minimumQualified", "targetMaximum", "minimumIndependentProjects", "minimumIndependentOrganizations"],
    "authority.thresholds.pilots",
  );
  if (
    authority.thresholds.interviews.minimumQualified !== 10 ||
    authority.thresholds.interviews.targetMaximum !== 15 ||
    authority.thresholds.incidents.minimumQualified !== 20 ||
    authority.thresholds.pilots.minimumQualified !== 3 ||
    authority.thresholds.pilots.targetMaximum !== 5 ||
    authority.thresholds.pilots.minimumIndependentProjects !== 3 ||
    authority.thresholds.pilots.minimumIndependentOrganizations !== 3
  ) {
    fail("authority thresholds drifted");
  }
  exactKeys(
    authority.eligibility,
    [
      "allowedRelationships",
      "excludedOrganizationIds",
      "excludedRepositoryOwners",
      "requiredConsentStatus",
      "requiredReviewStatus",
      "rawEvidencePolicy",
    ],
    "authority.eligibility",
  );
  strings(authority.eligibility.allowedRelationships, "authority.eligibility.allowedRelationships");
  strings(authority.eligibility.excludedOrganizationIds, "authority.eligibility.excludedOrganizationIds");
  strings(authority.eligibility.excludedRepositoryOwners, "authority.eligibility.excludedRepositoryOwners");
  if (
    JSON.stringify(authority.eligibility.allowedRelationships) !==
      JSON.stringify(["external-community", "external-customer", "external-partner"]) ||
    !authority.eligibility.excludedOrganizationIds.includes("celiums-solutions") ||
    !authority.eligibility.excludedOrganizationIds.includes("pliegocss") ||
    !authority.eligibility.excludedRepositoryOwners.includes("celiumsai") ||
    authority.eligibility.requiredConsentStatus !== "recorded" ||
    authority.eligibility.requiredReviewStatus !== "approved" ||
    authority.eligibility.rawEvidencePolicy !== "private-custody-hash-public-redacted-record"
  ) {
    fail("authority eligibility boundary drifted");
  }
  const coverage = strings(authority.requiredCoverage, "authority.requiredCoverage");
  for (const required of [
    "ten-to-fifteen-interviews",
    "twenty-incidents",
    "three-to-five-pilots",
    "three-independent-projects",
    "three-independent-organizations",
    "frozen-adapter-support-policy",
  ]) {
    if (!coverage.includes(required)) fail(`authority.requiredCoverage is missing ${required}`);
  }
  nonEmpty(authority.claimBoundary, "authority.claimBoundary");
}

export function loadExternalAdoptionBundle(root) {
  const authority = JSON.parse(
    readFileSync(resolve(root, "benchmarks", "external-adoption-v1", "authority.json"), "utf8"),
  );
  const readReferenced = (entry) =>
    JSON.parse(readFileSync(repositoryPath(root, entry.path, entry.path), "utf8"));
  return {
    authority,
    interviews: readReferenced(authority.evidence.interviews),
    incidents: readReferenced(authority.evidence.incidents),
    pilots: readReferenced(authority.evidence.pilots),
    policy: readReferenced(authority.adapterSupportPolicy),
    coexistence: JSON.parse(
      readFileSync(resolve(root, "benchmarks", "adapter-coexistence-v1", "authority.json"), "utf8"),
    ),
  };
}

export function validateExternalAdoptionBundle(bundle, { root, verifyFiles = true } = {}) {
  object(bundle, "bundle");
  if (typeof root !== "string" || root === "") fail("root is required");
  validateAuthority(bundle.authority, { root, verifyFiles });
  validatePolicy(bundle.policy, bundle.coexistence, { root, verifyFiles });
  const interviews = validateInterviews(bundle.interviews, bundle.authority);
  const incidents = validateIncidents(bundle.incidents, bundle.authority);
  const pilots = validatePilots(bundle.pilots, bundle.authority);

  const interviewIds = interviews.ids;
  const incidentIds = incidents.ids;
  const pilotIds = pilots.ids;
  for (const [index, entry] of bundle.interviews.records.entries()) {
    for (const id of entry.relatedIncidentIds) {
      if (!incidentIds.has(id)) fail(`interviews.records[${index}] references unknown incident ${id}`);
    }
    for (const id of entry.relatedPilotIds) {
      if (!pilotIds.has(id)) fail(`interviews.records[${index}] references unknown pilot ${id}`);
    }
  }
  if (interviewIds.size !== interviews.count) fail("interview identity drifted");

  const thresholds = bundle.authority.thresholds;
  const coveredCoverage = [];
  if (interviews.count >= thresholds.interviews.minimumQualified) {
    coveredCoverage.push("ten-to-fifteen-interviews");
  }
  if (incidents.count >= thresholds.incidents.minimumQualified) {
    coveredCoverage.push("twenty-incidents");
  }
  if (pilots.count >= thresholds.pilots.minimumQualified) {
    coveredCoverage.push("three-to-five-pilots");
  }
  if (pilots.projects.size >= thresholds.pilots.minimumIndependentProjects) {
    coveredCoverage.push("three-independent-projects");
  }
  if (pilots.organizations.size >= thresholds.pilots.minimumIndependentOrganizations) {
    coveredCoverage.push("three-independent-organizations");
  }
  coveredCoverage.push("frozen-adapter-support-policy");
  const missingCoverage = bundle.authority.requiredCoverage.filter(
    (item) => !coveredCoverage.includes(item),
  );

  return {
    schemaVersion: 1,
    kind: "pliegocss-external-adoption-status",
    studyId: bundle.authority.studyId,
    result: missingCoverage.length === 0 ? "ready" : "blocked",
    counts: {
      qualifiedInterviews: interviews.count,
      qualifiedIncidents: incidents.count,
      qualifiedPilots: pilots.count,
      independentProjects: pilots.projects.size,
      independentOrganizations: pilots.organizations.size,
    },
    thresholds,
    coveredCoverage,
    missingCoverage,
    policy: {
      status: bundle.policy.status,
      releaseLine: bundle.policy.releaseLine,
    },
    claimBoundary: bundle.authority.claimBoundary,
  };
}
