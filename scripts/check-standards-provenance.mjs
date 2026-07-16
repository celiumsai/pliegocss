import { createHash } from "node:crypto";
import { existsSync, lstatSync, readFileSync, realpathSync } from "node:fs";
import { dirname, isAbsolute, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const realRoot = realpathSync(root);
const attributionCommand = "node ./scripts/check-standards-provenance.mjs";
const reviewedFileDigests = Object.freeze({
  "THIRD_PARTY_NOTICES.md": "00c94fd9f07291966d5ffa252c6cf01d86da7a6ebecc8d526c7024f2cd36ce60",
  "docs/reference/standards-provenance.md": "2a48b22dd41fe7cdc33e2f7f075a1af261fdf5bfdba1f0ef6daa202151bf74fd",
});
const sourceKeys = [
  "id",
  "name",
  "kind",
  "version",
  "status",
  "license",
  "url",
  "relationship",
  "usage",
  "evidence",
  "integrity",
];
const expectedSources = JSON.parse(`[
  {"id":"baseline-browser-mapping","name":"baseline-browser-mapping","kind":"dataset","version":"2.10.43","status":"versioned-package","license":"Apache-2.0","url":"https://github.com/web-platform-dx/baseline-browser-mapping","relationship":"compatibility-dataset","usage":"frozen-data","evidence":["integration-tests/compatibility-policy/expected.baseline-widely.json"],"integrity":[{"subject":"baseline-browser-mapping@2.10.43","value":"sha512-AjYpR78kDWAY3Efj+cDTFH9t9SCoL7OoTp1BOb0mQV7S+6CiLwnWM3FyxhJtdPufDFKzmCSFoUncKjWgJEZTCQ=="},{"subject":"widelyAvailableOnDate=2026-07-14;includeDownstreamBrowsers=false","value":"sha256:05b26b78a9c0f3ad82ce4565ef058ac34fac7ebae45fc260daf3343ffece53fd"}]},
  {"id":"css-color-adjust-1","name":"CSS Color Adjustment Module Level 1","kind":"specification","version":"1","status":"w3c-candidate-recommendation-snapshot","license":"W3C-Software-and-Document-License-2023","url":"https://www.w3.org/TR/2025/CR-css-color-adjust-1-20251216/","relationship":"css-specification","usage":"audit-reference","evidence":["docs/reference/accessibility-policy.md"],"integrity":[]},
  {"id":"dtcg-color-2025.10","name":"Design Tokens Color Module 2025.10","kind":"specification","version":"2025.10","status":"final-community-group-report","license":"W3C-Community-Final-Specification-Agreement","url":"https://www.w3.org/community/reports/design-tokens/CG-FINAL-color-20251028/","relationship":"exchange-specification","usage":"implemented-contract","evidence":["crates/pliego-css-config/src/dtcg.rs","docs/reference/dtcg-bridge.md"],"integrity":[]},
  {"id":"dtcg-format-2025.10","name":"Design Tokens Format Module 2025.10","kind":"specification","version":"2025.10","status":"final-community-group-report","license":"W3C-Community-Final-Specification-Agreement","url":"https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/","relationship":"exchange-specification","usage":"implemented-contract","evidence":["crates/pliego-css-config/src/dtcg.rs","docs/reference/dtcg-bridge.md"],"integrity":[]},
  {"id":"dtcg-resolver-2025.10","name":"Design Tokens Resolver Module 2025.10","kind":"specification","version":"2025.10","status":"final-community-group-report","license":"W3C-Community-Final-Specification-Agreement","url":"https://www.w3.org/community/reports/design-tokens/CG-FINAL-resolver-20251028/","relationship":"exchange-specification","usage":"implemented-contract","evidence":["crates/pliego-css-config/src/dtcg_resolver.rs","docs/reference/dtcg-bridge.md"],"integrity":[]},
  {"id":"lightningcss","name":"Lightning CSS","kind":"software","version":"1.0.0-alpha.71","status":"versioned-crate","license":"MPL-2.0","url":"https://github.com/parcel-bundler/lightningcss","relationship":"css-backend","usage":"build-time","evidence":["Cargo.lock"],"integrity":[{"subject":"lightningcss@1.0.0-alpha.71","value":"cargo-checksum:cb6314c2f0590ac93c86099b98bb7ba8abcf759bfd89604ffca906472bb54937"}]},
  {"id":"mediaqueries-5","name":"Media Queries Level 5","kind":"specification","version":"5","status":"w3c-working-draft","license":"W3C-Software-and-Document-License-2023","url":"https://www.w3.org/TR/2026/WD-mediaqueries-5-20260219/","relationship":"css-specification","usage":"audit-reference","evidence":["docs/reference/accessibility-policy.md"],"integrity":[]},
  {"id":"tailwindcss","name":"Tailwind CSS and CLI","kind":"software","version":"4.3.2","status":"versioned-package","license":"MIT","url":"https://github.com/tailwindlabs/tailwindcss","relationship":"benchmark-baseline","usage":"benchmark-only","evidence":["benchmarks/tailwind-v4/input.css","package.json","pnpm-lock.yaml"],"integrity":[{"subject":"@tailwindcss/cli@4.3.2","value":"sha512-Fzt+HrIZHDlkRYKdLMBeufaroaPvwCBG70sMLdmurdeadNMO/LxbmT8Sbb+P83ep0iAlAImettb7Y+rO+37rXw=="},{"subject":"pnpm-lock.yaml","value":"sha256:c4f9b91705418524bd91178fd417e3b02585dfce613c2a56e6cef25a6c8c1049"},{"subject":"tailwindcss@4.3.2","value":"sha512-WtctNNSH8A9jlMIqxzuYumOHU5uGZyRv0Q5svQl+oEPy5w84YpBxdb7MdqyiSPQge5jTJ6zFQLq0PFygdccSBA=="}]},
  {"id":"wcag-2.2","name":"Web Content Accessibility Guidelines (WCAG) 2.2","kind":"specification","version":"2.2","status":"w3c-recommendation","license":"W3C-Document-License-2023","url":"https://www.w3.org/TR/2024/REC-WCAG22-20241212/","relationship":"accessibility-reference","usage":"audit-reference","evidence":["docs/reference/accessibility-policy.md"],"integrity":[]},
  {"id":"web-features","name":"web-features","kind":"dataset","version":"3.32.0","status":"versioned-package","license":"Apache-2.0","url":"https://github.com/web-platform-dx/web-features","relationship":"compatibility-dataset","usage":"frozen-data","evidence":["integration-tests/compatibility-policy/expected.baseline-widely.json"],"integrity":[{"subject":"web-features@3.32.0","value":"sha512-PQBbTofqV8FtMP65oT9tLPjbN4FSB2dRdNxLM0A9j4bNifVpFhEP/ATXSMMJAqPPWb/pgUOh6B+98yzfNEVbNw=="},{"subject":"web-features@3.32.0/data.json","value":"sha256:58bc2056041c93e313c3a58658a8120f857cf4888d0592e6e4a9b0484748441e"}]}
]`);

function fail(message) {
  throw new Error(message);
}

function assert(condition, message) {
  if (!condition) fail(message);
}

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function exactKeys(value, expected, label) {
  assert(isObject(value), `${label} must be an object`);
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  assert(JSON.stringify(actual) === JSON.stringify(wanted), `${label} has unknown or missing fields`);
}

function unique(values, label) {
  assert(new Set(values).size === values.length, `${label} contains duplicates`);
}

function sorted(values, label) {
  const expected = [...values].sort();
  assert(JSON.stringify(values) === JSON.stringify(expected), `${label} must be sorted`);
}

function repositoryFile(relativePath, label) {
  assert(
    typeof relativePath === "string" && relativePath.length > 0 &&
      !isAbsolute(relativePath) && !relativePath.includes("\\"),
    `${label} path is invalid`,
  );
  const parts = relativePath.split("/");
  assert(
    parts.every((part) => part.length > 0 && part !== "." && part !== ".."),
    `${label} path is not canonical`,
  );
  const path = resolve(root, relativePath);
  assert(path.startsWith(`${root}${sep}`) && existsSync(path), `${label} is missing: ${relativePath}`);
  let cursor = root;
  for (const part of parts) {
    cursor = join(cursor, part);
    const metadata = lstatSync(cursor);
    assert(
      !metadata.isSymbolicLink(),
      `${label} cannot traverse a symlink or junction: ${relativePath}`,
    );
  }
  assert(lstatSync(path).isFile(), `${label} must be a regular file: ${relativePath}`);
  const realPath = realpathSync(path);
  assert(
    realPath.startsWith(`${realRoot}${sep}`),
    `${label} resolves outside the repository: ${relativePath}`,
  );
  return path;
}

function sourceById(manifest, id) {
  return manifest.sources.find((source) => source.id === id);
}

function integrityValue(source, subject) {
  const item = source.integrity.find((entry) => entry.subject === subject);
  assert(item, `source ${source.id} lacks integrity for ${subject}`);
  return item.value;
}

function verifyReviewedFileDigest(relativePath, bytes) {
  const expected = reviewedFileDigests[relativePath];
  assert(expected, `no reviewed SHA-256 is registered for ${relativePath}`);
  const actual = createHash("sha256").update(bytes).digest("hex");
  assert(actual === expected, `${relativePath} reviewed SHA-256 drifted`);
}

function readReviewedFile(relativePath, label) {
  const bytes = readFileSync(repositoryFile(relativePath, label));
  verifyReviewedFileDigest(relativePath, bytes);
  return bytes.toString("utf8");
}

function verifyPnpmLockIntegrity(bytes, source) {
  const actual = `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
  assert(
    actual === integrityValue(source, "pnpm-lock.yaml"),
    "pnpm-lock.yaml SHA-256 drifted",
  );
}

function lockSection(lock, name) {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const matches = [...lock.matchAll(new RegExp(`^${escaped}:\\n`, "gm"))];
  assert(matches.length === 1, `pnpm-lock.yaml must contain exactly one ${name} section`);
  const tail = lock.slice(matches[0].index + matches[0][0].length);
  const next = tail.search(/^[a-z][a-zA-Z0-9-]*:\n/m);
  return next === -1 ? tail : tail.slice(0, next);
}

function sectionEntries(section) {
  const entries = [];
  let current;
  for (const line of section.split("\n")) {
    const header = /^  (\S.*?):(?: (.*))?$/.exec(line);
    if (header) {
      if (current) entries.push({ name: current.name, text: current.lines.join("\n").trimEnd() });
      let name = header[1];
      if (name.startsWith("'") && name.endsWith("'")) name = name.slice(1, -1);
      current = { name, lines: [line] };
    } else if (current) {
      current.lines.push(line);
    }
  }
  if (current) entries.push({ name: current.name, text: current.lines.join("\n").trimEnd() });
  return entries;
}

function oneEntry(entries, name, section) {
  const matches = entries.filter((entry) => entry.name === name);
  assert(matches.length === 1, `${section} must contain exactly one ${name} entry`);
  return matches[0].text;
}

function tailwindLockContract(source) {
  const version = source.version;
  const cli = `@tailwindcss/cli@${version}`;
  const core = `tailwindcss@${version}`;
  return {
    cli,
    core,
    importer: `  .:\n    devDependencies:\n      '@tailwindcss/cli':\n        specifier: ${version}\n        version: ${version}\n      tailwindcss:\n        specifier: ${version}\n        version: ${version}`,
    cliPackage: `  '${cli}':\n    resolution: {integrity: ${integrityValue(source, cli)}}\n    hasBin: true`,
    corePackage: `  ${core}:\n    resolution: {integrity: ${integrityValue(source, core)}}`,
    cliSnapshot: `  '${cli}':\n    dependencies:\n      '@parcel/watcher': 2.5.1\n      '@tailwindcss/node': ${version}\n      '@tailwindcss/oxide': ${version}\n      enhanced-resolve: 5.21.6\n      mri: 1.2.0\n      picocolors: 1.1.1\n      tailwindcss: ${version}`,
    coreSnapshot: `  ${core}: {}`,
    dependencies: [
      "@parcel/watcher@2.5.1",
      `@tailwindcss/node@${version}`,
      `@tailwindcss/oxide@${version}`,
      "enhanced-resolve@5.21.6",
      "mri@1.2.0",
      "picocolors@1.1.1",
      core,
    ],
  };
}

function validateManifest(manifest) {
  exactKeys(manifest, ["schemaVersion", "sources"], "manifest");
  assert(manifest.schemaVersion === 1, "schemaVersion must be 1");
  assert(Array.isArray(manifest.sources) && manifest.sources.length > 0, "sources must be non-empty");

  for (const [index, source] of manifest.sources.entries()) {
    const label = `sources[${index}]`;
    exactKeys(source, sourceKeys, label);
    assert(/^[a-z0-9]+(?:[.-][a-z0-9]+)*$/.test(source.id), `${label}.id is invalid`);
    assert(typeof source.name === "string" && source.name.trim() === source.name && source.name.length > 0, `${label}.name is invalid`);
    assert(["software", "dataset", "specification"].includes(source.kind), `${label}.kind is invalid`);
    assert(/^\d+(?:\.\d+)*(?:-[0-9A-Za-z]+(?:\.[0-9A-Za-z]+)*)?$/.test(source.version), `${label}.version is invalid`);
    assert([
      "versioned-crate",
      "versioned-package",
      "final-community-group-report",
      "w3c-recommendation",
      "w3c-candidate-recommendation-snapshot",
      "w3c-working-draft",
    ].includes(source.status), `${label}.status is invalid`);
    assert(["Apache-2.0", "MIT", "MPL-2.0", "W3C-Document-License-2023", "W3C-Software-and-Document-License-2023", "W3C-Community-Final-Specification-Agreement"].includes(source.license), `${label}.license is invalid`);
    assert([
      "css-backend",
      "compatibility-dataset",
      "exchange-specification",
      "accessibility-reference",
      "css-specification",
      "benchmark-baseline",
    ].includes(source.relationship), `${label}.relationship is invalid`);
    assert(["build-time", "frozen-data", "implemented-contract", "audit-reference", "benchmark-only"].includes(source.usage), `${label}.usage is invalid`);

    let url;
    try {
      url = new URL(source.url);
    } catch {
      fail(`${label}.url is invalid`);
    }
    assert(url.protocol === "https:" && !url.username && !url.password && !url.search && !url.hash, `${label}.url must be canonical HTTPS`);

    assert(Array.isArray(source.evidence) && source.evidence.length > 0, `${label}.evidence must be non-empty`);
    unique(source.evidence, `${label}.evidence`);
    sorted(source.evidence, `${label}.evidence`);
    for (const evidence of source.evidence) {
      repositoryFile(evidence, `${label}.evidence`);
    }

    assert(Array.isArray(source.integrity), `${label}.integrity must be an array`);
    for (const [integrityIndex, integrity] of source.integrity.entries()) {
      exactKeys(integrity, ["subject", "value"], `${label}.integrity[${integrityIndex}]`);
      assert(typeof integrity.subject === "string" && integrity.subject.length > 0, `${label}.integrity subject is invalid`);
      assert(/^(?:sha512-[A-Za-z0-9+/]+={0,2}|sha256:[0-9a-f]{64}|cargo-checksum:[0-9a-f]{64})$/.test(integrity.value), `${label}.integrity value is invalid`);
    }
    unique(source.integrity.map((entry) => entry.subject), `${label}.integrity subjects`);
    sorted(source.integrity.map((entry) => entry.subject), `${label}.integrity subjects`);
    assert(source.kind === "specification" || source.integrity.length > 0, `${label}.integrity cannot be empty`);
    assert(source.kind !== "specification" || source.integrity.length === 0, `${label}.specification integrity must be empty`);
  }

  unique(manifest.sources.map((source) => source.id), "source ids");
  unique(manifest.sources.map((source) => source.url), "source URLs");
  sorted(manifest.sources.map((source) => source.id), "source ids");

  assert(manifest.sources.length === expectedSources.length, "reviewed source count drifted");
  for (const expected of expectedSources) {
    const actual = sourceById(manifest, expected.id);
    assert(actual, `reviewed source is missing: ${expected.id}`);
    for (const key of sourceKeys) {
      assert(JSON.stringify(actual[key]) === JSON.stringify(expected[key]), `source ${expected.id} ${key} drifted`);
    }
  }
}

function verifyCargoLock(manifest) {
  const source = sourceById(manifest, "lightningcss");
  const lock = readFileSync(resolve(root, "Cargo.lock"), "utf8").replaceAll("\r\n", "\n");
  const packages = lock.split(/\n(?=\[\[package\]\]\n)/).filter((block) => /^\[\[package\]\]\nname = "lightningcss"$/m.test(block));
  assert(packages.length === 1, "Cargo.lock must contain exactly one lightningcss package");
  const version = packages[0].match(/^version = "([^"]+)"$/m)?.[1];
  const registry = packages[0].match(/^source = "([^"]+)"$/m)?.[1];
  const checksum = packages[0].match(/^checksum = "([0-9a-f]+)"$/m)?.[1];
  assert(version === source.version, "Lightning CSS version drifted from Cargo.lock");
  assert(registry === "registry+https://github.com/rust-lang/crates.io-index", "Lightning CSS registry source drifted from Cargo.lock");
  assert(integrityValue(source, `lightningcss@${source.version}`) === `cargo-checksum:${checksum}`, "Lightning CSS checksum drifted from Cargo.lock");
}

function verifyCompatibilityGolden(manifest) {
  const golden = JSON.parse(readFileSync(resolve(root, "integration-tests", "compatibility-policy", "expected.baseline-widely.json"), "utf8"));
  const web = sourceById(manifest, "web-features");
  const webData = golden.compatibilityData?.webFeatures;
  assert(webData?.name === "web-features", "web-features name drifted from compatibility golden");
  assert(webData?.version === web.version, "web-features version drifted from compatibility golden");
  assert(webData.artifact === `https://registry.npmjs.org/web-features/-/web-features-${web.version}.tgz`, "web-features artifact URL drifted from compatibility golden");
  assert(webData.content === "data.json", "web-features content path drifted from compatibility golden");
  assert(webData.integrity === integrityValue(web, `web-features@${web.version}`), "web-features SRI drifted from compatibility golden");
  assert(`sha256:${webData.contentSha256}` === integrityValue(web, `web-features@${web.version}/data.json`), "web-features data hash drifted from compatibility golden");

  const mapping = sourceById(manifest, "baseline-browser-mapping");
  const mappingData = golden.compatibilityData?.baselineBrowserMapping;
  assert(mappingData?.name === "baseline-browser-mapping", "baseline-browser-mapping name drifted from compatibility golden");
  assert(mappingData?.version === mapping.version, "baseline-browser-mapping version drifted from compatibility golden");
  assert(mappingData.artifact === `https://registry.npmjs.org/baseline-browser-mapping/-/baseline-browser-mapping-${mapping.version}.tgz`, "baseline-browser-mapping artifact URL drifted from compatibility golden");
  assert(mappingData.integrity === integrityValue(mapping, `baseline-browser-mapping@${mapping.version}`), "baseline-browser-mapping SRI drifted from compatibility golden");
  assert(`sha256:${mappingData.resultSha256}` === integrityValue(mapping, mappingData.query), "baseline-browser-mapping result hash drifted from compatibility golden");
}

function verifyDtcg(manifest) {
  const format = readFileSync(resolve(root, "crates", "pliego-css-config", "src", "dtcg.rs"), "utf8");
  const resolver = readFileSync(resolve(root, "crates", "pliego-css-config", "src", "dtcg_resolver.rs"), "utf8");
  const bridge = readFileSync(resolve(root, "docs", "reference", "dtcg-bridge.md"), "utf8");
  const version = sourceById(manifest, "dtcg-format-2025.10").version;
  assert(format.includes(`pub const DTCG_FORMAT_VERSION: &str = "${version}";`), "DTCG format constant drifted");
  assert(resolver.includes(`pub const DTCG_RESOLVER_PROFILE: &str = "${version}/same-document-1";`), "DTCG resolver profile drifted");
  assert(bridge.includes("stable Final Community") && bridge.includes("not a W3C Recommendation"), "DTCG publication status documentation drifted");
  for (const id of ["dtcg-format-2025.10", "dtcg-color-2025.10", "dtcg-resolver-2025.10"]) {
    assert(sourceById(manifest, id).status === "final-community-group-report", `${id} must remain a Final Community Group Report`);
  }
}

function verifyAccessibilityReferences(manifest, candidate) {
  const reference = candidate ?? readFileSync(
    repositoryFile("docs/reference/accessibility-policy.md", "accessibility policy reference"),
    "utf8",
  );
  for (const [id, fragment] of [
    ["wcag-2.2", "#contrast-minimum"],
    ["css-color-adjust-1", "#forced-color-adjust-prop"],
    ["mediaqueries-5", "#prefers-reduced-motion"],
  ]) {
    const url = `${sourceById(manifest, id).url}${fragment}`;
    assert(reference.includes(url), `accessibility reference is missing dated source ${url}`);
  }
}

function verifyAttributionScript(packageJson) {
  assert(
    packageJson.scripts?.["check:attribution"] === attributionCommand,
    "package.json check:attribution command drifted",
  );
}

function verifyTailwindLock(lock, source) {
  const contract = tailwindLockContract(source);
  assert(lock.startsWith("lockfileVersion: '9.0'\n"), "pnpm lockfile version drifted");
  const importers = lockSection(lock, "importers");
  const packages = sectionEntries(lockSection(lock, "packages"));
  const snapshots = sectionEntries(lockSection(lock, "snapshots"));
  const importerEntries = sectionEntries(importers);

  assert(oneEntry(importerEntries, ".", "importers") === contract.importer, "Tailwind root importer drifted");
  assert((importers.match(/^\s+'?@tailwindcss\/cli'?:$/gm) ?? []).length === 1, "Tailwind CLI must have exactly one importer edge");
  assert((importers.match(/^\s+tailwindcss:$/gm) ?? []).length === 1, "tailwindcss must have exactly one importer edge");

  assert(oneEntry(packages, contract.cli, "packages") === contract.cliPackage, "Tailwind CLI package snapshot drifted");
  assert(oneEntry(packages, contract.core, "packages") === contract.corePackage, "tailwindcss package snapshot drifted");
  assert(oneEntry(snapshots, contract.cli, "snapshots") === contract.cliSnapshot, "Tailwind CLI dependency graph drifted");
  assert(oneEntry(snapshots, contract.core, "snapshots") === contract.coreSnapshot, "tailwindcss runtime snapshot drifted");

  for (const [prefix, expected] of [["@tailwindcss/cli@", contract.cli], ["tailwindcss@", contract.core]]) {
    for (const [section, entries] of [["packages", packages], ["snapshots", snapshots]]) {
      const versions = entries.filter((entry) => entry.name.startsWith(prefix)).map((entry) => entry.name);
      assert(versions.length === 1 && versions[0] === expected, `${section} contains orphan or multiple ${prefix} entries`);
    }
  }
  for (const dependency of contract.dependencies) {
    oneEntry(packages, dependency, "packages");
    oneEntry(snapshots, dependency, "snapshots");
  }
}

function verifyTailwind(manifest) {
  const source = sourceById(manifest, "tailwindcss");
  assert(source.usage === "benchmark-only", "Tailwind must remain benchmark-only");
  const packageJson = JSON.parse(readFileSync(resolve(root, "package.json"), "utf8"));
  verifyAttributionScript(packageJson);
  assert(packageJson.devDependencies?.tailwindcss === source.version, "tailwindcss package version drifted");
  assert(packageJson.devDependencies?.["@tailwindcss/cli"] === source.version, "Tailwind CLI package version drifted");
  const lockBytes = readFileSync(repositoryFile("pnpm-lock.yaml", "pnpm lockfile"));
  verifyPnpmLockIntegrity(lockBytes, source);
  const lock = lockBytes.toString("utf8").replaceAll("\r\n", "\n");
  verifyTailwindLock(lock, source);
}

function exactCount(value, needle) {
  return value.split(needle).length - 1;
}

function verifyDocumentationIndex(index) {
  const link = "- [Standards and third-party provenance schema 1](./reference/standards-provenance.md)";
  assert(exactCount(index, link) === 1, "docs/index.md provenance link drifted");
}

function verifyCiWiring(ci) {
  const jobsMarker = "jobs:\n";
  assert(exactCount(ci, jobsMarker) === 1, "CI jobs boundary drifted");
  const header = ci.slice(0, ci.indexOf(jobsMarker));
  const expectedHeader = `name: CI

on:
  push:
  pull_request:

permissions:
  contents: read

`;
  assert(header === expectedHeader, "CI workflow name, triggers, or global permissions drifted");
  const job = /\n  provenance:\n([\s\S]*?)(?=\n  [a-zA-Z0-9_-]+:\n|$)/.exec(ci)?.[0];
  assert(job, "CI provenance job is missing");
  assert(exactCount(job, "node-version: 22.13.0") === 1, "CI provenance job must use Node 22.13.0");
  assert(
    exactCount(job, "run: node scripts/check-standards-provenance.mjs") === 1,
    "CI provenance command drifted",
  );
  const expected = `
  provenance:
    name: Standards provenance
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4
      - uses: actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020 # v4
        with:
          node-version: 22.13.0
      - name: Check standards provenance and attribution
        run: node scripts/check-standards-provenance.mjs
`;
  assert(job === expected, "CI provenance job block drifted");
}

function verifyReleaseWiring(release) {
  const marker = "## 2. Run local quality gates\n";
  assert(exactCount(release, marker) === 1, "release quality-gates section drifted");
  const tail = release.slice(release.indexOf(marker) + marker.length);
  const next = tail.search(/^## /m);
  const qualityGates = next === -1 ? tail : tail.slice(0, next);
  assert(
    (qualityGates.match(/^pnpm check:attribution$/gm) ?? []).length === 1 &&
      (release.match(/^pnpm check:attribution$/gm) ?? []).length === 1,
    "release quality-gates checklist must contain exactly one pnpm check:attribution command",
  );
}

function verifyRepositoryWiring() {
  readReviewedFile(
    "docs/reference/standards-provenance.md",
    "standards provenance reference",
  );
  const packageJson = JSON.parse(readFileSync(repositoryFile("package.json", "package manifest"), "utf8"));
  verifyAttributionScript(packageJson);
  verifyDocumentationIndex(
    readFileSync(repositoryFile("docs/index.md", "documentation index"), "utf8"),
  );
  verifyCiWiring(readFileSync(repositoryFile(".github/workflows/ci.yml", "CI workflow"), "utf8"));
  verifyReleaseWiring(
    readFileSync(
      repositoryFile("docs/contributing/release-process.md", "release process"),
      "utf8",
    ),
  );
}

function markdownLevelTwoSection(document, heading) {
  const marker = `${heading}\n`;
  assert(exactCount(document, marker) === 1, `THIRD_PARTY_NOTICES.md section drifted: ${heading}`);
  const tail = document.slice(document.indexOf(marker) + marker.length);
  const next = tail.search(/^## /m);
  return next === -1 ? tail : tail.slice(0, next);
}

function verifyNoticeContent(manifest, notices) {
  const firstSection = notices.search(/^## /m);
  assert(firstSection > 0, "THIRD_PARTY_NOTICES.md lacks scoped sections");
  const preamble = notices.slice(0, firstSection).replace(/\s+/g, " ");
  assert(
    preamble.includes("It is not an exhaustive dependency notice, SBOM, or legal audit.") &&
      preamble.includes("Other Cargo/npm dependencies remain governed by their package licenses and lockfiles."),
    "THIRD_PARTY_NOTICES.md scope and non-SBOM disclaimer drifted",
  );

  const sectionBySource = {
    "baseline-browser-mapping": "## Frozen compatibility data",
    "css-color-adjust-1": "## Accessibility and CSS references",
    "dtcg-color-2025.10": "## Implemented exchange reports",
    "dtcg-format-2025.10": "## Implemented exchange reports",
    "dtcg-resolver-2025.10": "## Implemented exchange reports",
    lightningcss: "## Build-time software",
    "mediaqueries-5": "## Accessibility and CSS references",
    tailwindcss: "## Benchmark-only software",
    "wcag-2.2": "## Accessibility and CSS references",
    "web-features": "## Frozen compatibility data",
  };
  exactKeys(sectionBySource, manifest.sources.map((source) => source.id), "notice source/section mapping");
  for (const source of manifest.sources) {
    const section = markdownLevelTwoSection(notices, sectionBySource[source.id]);
    for (const value of [source.name, source.version, source.license, source.url]) {
      assert(section.includes(value), `THIRD_PARTY_NOTICES.md lacks scoped ${source.id} attribution: ${value}`);
    }
  }
  assert(notices.includes("not W3C\nRecommendations") || notices.includes("not W3C Recommendations"), "DTCG non-Recommendation notice is missing");
}

function verifyNotices(manifest) {
  const notices = readReviewedFile("THIRD_PARTY_NOTICES.md", "curated third-party notice");
  verifyNoticeContent(manifest, notices);
}

function verifyClosedSchemaRegressions() {
  const base = () => structuredClone({ schemaVersion: 1, sources: expectedSources });
  const cases = [
    ["unknown or missing fields", (value) => { value.extra = true; }],
    ["duplicates", (value) => { value.sources.push(structuredClone(value.sources[0])); }],
    ["url", (value) => { value.sources[0].url = "http://example.com/source"; }],
    ["license", (value) => { value.sources[0].license = "unreviewed"; }],
    ["version", (value) => { value.sources[0].version = "latest"; }],
    ["not canonical", (value) => { value.sources[0].evidence = ["integration-tests/../Cargo.lock"]; }],
    ["regular file", (value) => { value.sources[0].evidence = ["docs"]; }],
    ["drifted", (value) => { value.sources[0].version = "2.10.44"; }],
  ];
  for (const [expectedMessage, mutate] of cases) {
    const candidate = base();
    mutate(candidate);
    let message = "";
    try {
      validateManifest(candidate);
    } catch (error) {
      message = error instanceof Error ? error.message : String(error);
    }
    assert(message.includes(expectedMessage), `closed-schema regression was accepted: ${expectedMessage}`);
  }
}

function expectRejection(label, expectedMessage, operation) {
  let message = "";
  try {
    operation();
  } catch (error) {
    message = error instanceof Error ? error.message : String(error);
  }
  assert(
    message.includes(expectedMessage),
    `${label} mutation was accepted or rejected unexpectedly: ${message || "<no error>"}`,
  );
}

function verifyIntegrationRegressions(manifest) {
  const packageJson = JSON.parse(readFileSync(resolve(root, "package.json"), "utf8"));
  const noOp = structuredClone(packageJson);
  noOp.scripts["check:attribution"] = "node -e \"process.exit(0)\"";
  expectRejection("no-op attribution script", "command drifted", () => verifyAttributionScript(noOp));

  const source = sourceById(manifest, "tailwindcss");
  const contract = tailwindLockContract(source);
  const lock = readFileSync(resolve(root, "pnpm-lock.yaml"), "utf8").replaceAll("\r\n", "\n");
  const importerDrift = lock.replace("        specifier: 4.3.2", "        specifier: 4.3.1");
  expectRejection("Tailwind importer drift", "root importer drifted", () => verifyTailwindLock(importerDrift, source));

  const duplicatePackage = lock.replace(contract.cliPackage, `${contract.cliPackage}\n\n${contract.cliPackage}`);
  expectRejection("duplicate Tailwind package", "exactly one", () => verifyTailwindLock(duplicatePackage, source));

  const orphanSnapshot = lock.replace(`\n${contract.coreSnapshot}\n`, "\n");
  expectRejection("orphan Tailwind package", "exactly one", () => verifyTailwindLock(orphanSnapshot, source));

  const graphDrift = lock.replace(
    contract.cliSnapshot,
    contract.cliSnapshot.replace(`tailwindcss: ${source.version}`, "tailwindcss: 4.3.1"),
  );
  expectRejection("Tailwind dependency graph drift", "dependency graph drifted", () => verifyTailwindLock(graphDrift, source));

  const transitiveDrift = lock.replace("      lightningcss: 1.32.0", "      lightningcss: 1.31.0");
  expectRejection("Tailwind transitive lock drift", "SHA-256 drifted", () => {
    verifyPnpmLockIntegrity(Buffer.from(transitiveDrift), source);
  });

  const index = readFileSync(resolve(root, "docs", "index.md"), "utf8");
  expectRejection("documentation index link", "link drifted", () => {
    verifyDocumentationIndex(index.replace(
      "- [Standards and third-party provenance schema 1](./reference/standards-provenance.md)",
      "",
    ));
  });

  const ci = readFileSync(resolve(root, ".github", "workflows", "ci.yml"), "utf8");
  expectRejection("CI workflow_dispatch-only trigger", "triggers", () => {
    verifyCiWiring(ci.replace("  push:\n  pull_request:\n", "  workflow_dispatch:\n"));
  });
  expectRejection("CI path-filtered trigger", "triggers", () => {
    verifyCiWiring(ci.replace("  push:\n", "  push:\n    paths:\n      - 'scripts/**'\n"));
  });
  expectRejection("CI Node version", "Node 22.13.0", () => {
    verifyCiWiring(ci.replace("node-version: 22.13.0", "node-version: 22.12.0"));
  });
  expectRejection("disabled CI provenance job", "job block drifted", () => {
    verifyCiWiring(ci.replace(
      "    name: Standards provenance",
      "    name: Standards provenance\n    if: false",
    ));
  });

  const release = readFileSync(resolve(root, "docs", "contributing", "release-process.md"), "utf8");
  expectRejection("release attribution command", "quality-gates checklist", () => {
    verifyReleaseWiring(release.replace("pnpm check:attribution", "pnpm check:attribution-noop"));
  });

  const accessibility = readFileSync(
    resolve(root, "docs", "reference", "accessibility-policy.md"),
    "utf8",
  );
  const datedWcag = sourceById(manifest, "wcag-2.2").url;
  expectRejection("moving W3C accessibility alias", "dated source", () => {
    verifyAccessibilityReferences(
      manifest,
      accessibility.replace(datedWcag, "https://www.w3.org/TR/WCAG22/"),
    );
  });

  const referencePath = "docs/reference/standards-provenance.md";
  const referenceBytes = readFileSync(resolve(root, ...referencePath.split("/")));
  expectRejection("reviewed provenance reference", "reviewed SHA-256 drifted", () => {
    verifyReviewedFileDigest(referencePath, Buffer.concat([referenceBytes, Buffer.from("\n")]));
  });

  const notices = readFileSync(resolve(root, "THIRD_PARTY_NOTICES.md"), "utf8");
  const lightningUrl = sourceById(manifest, "lightningcss").url;
  const misplacedNotice = notices
    .replace("# Third-party notices\n", `# Third-party notices\n\n${lightningUrl}\n`)
    .replace(`- Source: ${lightningUrl}`, "- Source: moved outside this section");
  expectRejection("notice section association", "scoped lightningcss attribution", () => {
    verifyNoticeContent(manifest, misplacedNotice);
  });
  expectRejection("notice non-SBOM scope", "scope and non-SBOM disclaimer", () => {
    verifyNoticeContent(
      manifest,
      notices.replace("It is not\nan exhaustive dependency notice, SBOM, or legal audit.", ""),
    );
  });
}

function main() {
  const manifest = JSON.parse(
    readFileSync(repositoryFile("standards/provenance.json", "standards provenance manifest"), "utf8"),
  );
  validateManifest(manifest);
  verifyClosedSchemaRegressions();
  verifyIntegrationRegressions(manifest);
  verifyCargoLock(manifest);
  verifyCompatibilityGolden(manifest);
  verifyDtcg(manifest);
  verifyAccessibilityReferences(manifest);
  verifyTailwind(manifest);
  verifyNotices(manifest);
  verifyRepositoryWiring();
  console.log("standards provenance gate: pass");
}

try {
  main();
} catch (error) {
  console.error(`standards provenance gate: fail: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
