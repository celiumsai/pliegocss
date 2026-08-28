import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  exactKeys,
  extractStaticClassGroups,
  loadContract,
  profileById,
  renderProjectDocument,
  repositoryRoot,
  sha256,
} from "./adapter-coexistence-v1.mjs";

function fail(message) {
  throw new Error(`adapter coexistence authority: ${message}`);
}

export function uniqueMap(values, role) {
  if (!Array.isArray(values)) fail(`${role} must be an array`);
  const entries = new Map(values.map((value) => [value.id, value]));
  if (entries.size !== values.length) fail(`${role} contains duplicate IDs`);
  return entries;
}

const { authority, corpus } = loadContract();
exactKeys(
  authority,
  [
    "schemaVersion",
    "status",
    "kind",
    "corpus",
    "sharedReset",
    "tailwindProfiles",
    "adapters",
    "hosts",
    "migration",
    "comparison",
    "requiredCoverage",
    "claimBoundary",
  ],
  "authority",
);
if (
  authority.schemaVersion !== 1 ||
  authority.status !== "active-contract" ||
  authority.kind !== "pliegocss-adapter-coexistence-certification"
) {
  fail("header drifted");
}
if (corpus.schemaVersion !== 1 || corpus.kind !== "pliegocss-adapter-coexistence-corpus") {
  fail("corpus header drifted");
}
if (!Array.isArray(corpus.projects) || corpus.projects.length < authority.corpus.minimumProjects) {
  fail(`requires at least ${authority.corpus.minimumProjects} projects`);
}
if (authority.corpus.minimumProjects < 20) fail("minimum project floor cannot be below 20");

export function validateSupportPolicy(supportPolicy, authority, pliegorsManifest, pliegorsContract) {
  exactKeys(
    supportPolicy,
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
    "adapter support policy",
  );
  if (
    supportPolicy.schemaVersion !== 1 ||
    supportPolicy.kind !== "pliegocss-adapter-support-policy" ||
    supportPolicy.status !== "frozen-for-0.1-promotion" ||
    supportPolicy.releaseLine !== "0.1.x" ||
    !/^\d{4}-\d{2}-\d{2}$/u.test(supportPolicy.frozenAt)
  ) {
    fail("adapter support policy header drifted");
  }
  exactKeys(
    supportPolicy.protocol,
    ["name", "authoritySchema", "migrationInventorySchema"],
    "adapter support policy protocol",
  );
  if (
    supportPolicy.protocol.name !== "static-complete-class-group-coexistence" ||
    supportPolicy.protocol.authoritySchema !== 1 ||
    supportPolicy.protocol.migrationInventorySchema !== 1
  ) {
    fail("adapter support policy protocol drifted");
  }

  if (supportPolicy.supportedTailwindProfiles.length !== authority.tailwindProfiles.length) {
    fail("adapter support policy profile inventory drifted");
  }
  for (const [index, profile] of supportPolicy.supportedTailwindProfiles.entries()) {
    exactKeys(
      profile,
      ["id", "certifiedVersion", "sourceContract"],
      `adapter support policy profile ${index}`,
    );
  }
  const policyProfiles = uniqueMap(
    supportPolicy.supportedTailwindProfiles,
    "adapter support policy profiles",
  );
  if (
    policyProfiles.size !== authority.tailwindProfiles.length ||
    authority.tailwindProfiles.some((profile) => {
      const policy = policyProfiles.get(profile.id);
      return (
        policy?.certifiedVersion !== profile.version ||
        policy?.sourceContract !== profile.sourceContract
      );
    })
  ) {
    fail("adapter support policy profiles do not match the coexistence authority");
  }

  if (supportPolicy.supportedAdapters.length !== authority.adapters.length) {
    fail("adapter support policy adapter inventory drifted");
  }
  for (const [index, adapter] of supportPolicy.supportedAdapters.entries()) {
    exactKeys(
      adapter,
      ["id", "supportTier", "inputContract", "version"],
      `adapter support policy adapter ${index}`,
    );
    if (adapter.supportTier !== "certified") fail(`${adapter.id} is not certified`);
  }
  const policyAdapters = uniqueMap(
    supportPolicy.supportedAdapters,
    "adapter support policy adapters",
  );
  if (
    policyAdapters.size !== authority.adapters.length ||
    policyAdapters.get("html")?.version !== null ||
    policyAdapters.get("html")?.inputContract !== "complete-static-html-document" ||
    policyAdapters.get("vite")?.version !==
      authority.adapters.find((adapter) => adapter.id === "vite")?.version ||
    policyAdapters.get("vite")?.inputContract !== "vite-production-build" ||
    policyAdapters.get("pliegors")?.version !==
      "source-revision=abb8653e75da4a5cb4dd9b51200114fbb1e760c7;pliego-dom=0.4.0-beta.1;pliego-ssg=0.4.0-beta.1" ||
    policyAdapters.get("pliegors")?.inputContract !==
      "rendered-static-html-plus-pinned-framework-browser-replay"
  ) {
    fail("adapter support policy adapters drifted");
  }

  exactKeys(
    supportPolicy.migration,
    [
      "mode",
      "tailwindRuntimeRetained",
      "applyUnit",
      "rollback",
      "dynamicClasses",
      "arbitraryConfigOrPluginExecution",
      "tailwindRemoval",
    ],
    "adapter support policy migration",
  );
  if (
    supportPolicy.migration.mode !== "coexistence" ||
    supportPolicy.migration.tailwindRuntimeRetained !==
      authority.migration.tailwindRuntimeRetained ||
    supportPolicy.migration.applyUnit !== "complete-literal-class-group" ||
    supportPolicy.migration.rollback !== "exact-source-bytes" ||
    supportPolicy.migration.dynamicClasses !== "unsupported-fail-closed" ||
    supportPolicy.migration.arbitraryConfigOrPluginExecution !== "unsupported-fail-closed" ||
    supportPolicy.migration.tailwindRemoval !== "not-certified" ||
    authority.migration.dynamicClassPolicy !== "reject" ||
    authority.migration.rollback !== "restore-exact-source-bytes"
  ) {
    fail("adapter support policy migration boundary drifted");
  }

  exactKeys(
    supportPolicy.verification,
    ["requiredHosts", "requiredComparisons", "maximumLayoutGeometryDeltaCssPx"],
    "adapter support policy verification",
  );
  if (
    JSON.stringify(supportPolicy.verification.requiredHosts) !==
      JSON.stringify(authority.hosts.map((host) => host.id)) ||
    JSON.stringify(supportPolicy.verification.requiredComparisons) !==
      JSON.stringify(["dom", "aria", "computed-style", "layout-geometry", "exact-rollback"]) ||
    supportPolicy.verification.maximumLayoutGeometryDeltaCssPx !==
      authority.comparison.maximumLayoutGeometryDeltaCssPx
  ) {
    fail("adapter support policy verification matrix drifted");
  }
  exactKeys(
    supportPolicy.supportTiers,
    ["certified", "best-effort", "unsupported"],
    "adapter support policy tiers",
  );
  if (
    Object.values(supportPolicy.supportTiers).some(
      (description) => typeof description !== "string" || description.trim() === "",
    ) ||
    typeof supportPolicy.claimBoundary !== "string" ||
    supportPolicy.claimBoundary.trim() === ""
  ) {
    fail("adapter support policy descriptions are incomplete");
  }
  for (const dependency of ["pliego-dom", "pliego-ssg"]) {
    if (
      !new RegExp(`^${dependency} = \\{ version = "=0\\.4\\.0-beta\\.1",`, "mu").test(
        pliegorsManifest,
      )
    ) {
      fail(`adapter support policy PliegoRS pin drifted from ${dependency}`);
    }
  }
  if (pliegorsContract.revision !== "abb8653e75da4a5cb4dd9b51200114fbb1e760c7") {
    fail("adapter support policy PliegoRS source revision drifted");
  }
  if (
    supportPolicy.claimBoundary !==
    "The 0.1.x promoted adapter claim covers only literal complete class groups in static HTML, Vite 8.1.5 production builds, and pinned PliegoRS rendered output, coexisting with Tailwind CSS 3.4.19 or 4.3.3 without Preflight. It does not certify dynamic class construction, arbitrary Tailwind configuration or plugin execution, unsupported utilities, Tailwind removal, or other framework versions."
  ) {
    fail("adapter support policy claim boundary drifted");
  }
}

const supportPolicy = JSON.parse(
  readFileSync(resolve(repositoryRoot, "docs", "product", "adapter-support-policy-0.1.x.json"), "utf8"),
);
const pliegorsManifest = readFileSync(
  resolve(repositoryRoot, "integration-tests", "pliegors-smoke", "Cargo.toml"),
  "utf8",
);
const pliegorsContract = JSON.parse(
  readFileSync(
    resolve(repositoryRoot, "integration-tests", "pliegors-smoke", "pliegors-contract.json"),
    "utf8",
  ),
);
validateSupportPolicy(supportPolicy, authority, pliegorsManifest, pliegorsContract);

const ids = new Set();
const documentHashes = new Set();
const adapterCounts = Object.fromEntries(authority.adapters.map((adapter) => [adapter.id, 0]));
const profileCounts = Object.fromEntries(authority.tailwindProfiles.map((profile) => [profile.id, 0]));
let classGroups = 0;
for (const project of corpus.projects) {
  exactKeys(
    project,
    ["id", "adapter", "tailwindProfile", "title", "viewport", "body"],
    `project ${project.id ?? "unknown"}`,
  );
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(project.id) || ids.has(project.id)) {
    fail(`invalid or duplicate project id ${JSON.stringify(project.id)}`);
  }
  ids.add(project.id);
  if (!(project.adapter in adapterCounts)) fail(`${project.id} uses an unknown adapter`);
  profileById(authority, project.tailwindProfile);
  if (!(project.tailwindProfile in profileCounts)) fail(`${project.id} uses an unknown profile`);
  exactKeys(project.viewport, ["width", "height"], `${project.id}.viewport`);
  if (
    !Number.isInteger(project.viewport.width) ||
    !Number.isInteger(project.viewport.height) ||
    project.viewport.width < 320 ||
    project.viewport.height < 480
  ) {
    fail(`${project.id} viewport is outside the bounded browser range`);
  }
  const document = renderProjectDocument(project);
  const groups = extractStaticClassGroups(document, project.id);
  if (groups.groups.length < 3) fail(`${project.id} is not a meaningful class-bearing project`);
  if (!/\b(?:aria-[a-z-]+|role)=/u.test(project.body)) {
    fail(`${project.id} must expose explicit accessible semantics`);
  }
  if (project.adapter === "pliegors" && !project.body.includes("data-pliegors-")) {
    fail(`${project.id} is missing a PliegoRS rendered-output marker`);
  }
  const documentHash = sha256(Buffer.from(document));
  if (documentHashes.has(documentHash)) fail(`${project.id} duplicates another project document`);
  documentHashes.add(documentHash);
  adapterCounts[project.adapter] += 1;
  profileCounts[project.tailwindProfile] += 1;
  classGroups += groups.groups.length;
}

for (const adapter of authority.adapters) {
  if (adapterCounts[adapter.id] < adapter.minimumProjects) {
    fail(`${adapter.id} requires ${adapter.minimumProjects} projects, found ${adapterCounts[adapter.id]}`);
  }
}
for (const profile of authority.tailwindProfiles) {
  if (profileCounts[profile.id] < 7) fail(`${profile.id} requires at least seven projects`);
  const installed = JSON.parse(
    readFileSync(resolve(repositoryRoot, "node_modules", profile.packageAlias, "package.json"), "utf8"),
  );
  if (installed.name !== profile.packageName || installed.version !== profile.version) {
    fail(
      `${profile.id} expected ${profile.packageName}@${profile.version}, found ${installed.name}@${installed.version}`,
    );
  }
}
const viteAdapter = authority.adapters.find((adapter) => adapter.id === "vite");
const vite = JSON.parse(readFileSync(resolve(repositoryRoot, "node_modules", "vite", "package.json"), "utf8"));
if (vite.version !== viteAdapter.version) fail(`Vite expected ${viteAdapter.version}, found ${vite.version}`);

const resetBytes = readFileSync(resolve(repositoryRoot, authority.sharedReset.path));
if (sha256(resetBytes) !== authority.sharedReset.sha256) fail("shared reset hash drifted");

const required = new Set(authority.requiredCoverage);
for (const item of [
  "at-least-20-projects",
  "html",
  "vite-production-build",
  "pliegors-rendered-output",
  "pinned-pliegors-framework-browser",
  "tailwind-v3-lts",
  "tailwind-v4-current",
  "tailwind-source-inventory",
  "tailwind-output-audit",
  "reversible-group-apply",
  "exact-byte-rollback",
  "dom-equivalence",
  "aria-equivalence",
  "computed-style-equivalence",
  "layout-geometry-equivalence",
]) {
  if (!required.has(item)) fail(`required coverage is missing ${item}`);
}

process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 1,
      passed: true,
      projects: corpus.projects.length,
      uniqueDocuments: documentHashes.size,
      classGroups,
      adapters: adapterCounts,
      tailwindProfiles: profileCounts,
    },
    null,
    2,
  )}\n`,
);
