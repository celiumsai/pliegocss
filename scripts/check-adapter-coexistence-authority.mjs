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
