import { spawnSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { loadContract, readJson, repositoryRoot } from "./adapter-coexistence-v1.mjs";

const contract = loadContract();
const source = {
  commit: spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repositoryRoot,
    encoding: "utf8",
    windowsHide: true,
  }).stdout.trim(),
  gitTree: spawnSync("git", ["rev-parse", "HEAD^{tree}"], {
    cwd: repositoryRoot,
    encoding: "utf8",
    windowsHide: true,
  }).stdout.trim(),
  dirty: false,
};
const targetRoot = resolve(
  repositoryRoot,
  "target",
  "adapter-coexistence-certification",
  "matrix-contract-test",
);
rmSync(targetRoot, { recursive: true, force: true });

function evidence(host) {
  const count = contract.corpus.projects.length;
  return {
    schemaVersion: 1,
    kind: "pliegocss-adapter-coexistence-host-evidence",
    result: "pass",
    source: { ...source },
    authority: {
      sha256: contract.authoritySha256,
      corpusSha256: contract.corpusSha256,
    },
    host: {
      ...host,
      browserVersion: "Chromium contract test",
      playwright: "1.61.1",
    },
    summary: {
      projects: count,
      passed: count,
      domEquivalent: count,
      ariaEquivalent: count,
      computedStyleEquivalent: count,
      geometryEquivalent: count,
      exactRollbacks: count,
      adapters: Object.fromEntries(
        contract.authority.adapters.map((adapter) => [
          adapter.id,
          contract.corpus.projects.filter((project) => project.adapter === adapter.id).length,
        ]),
      ),
      tailwindProfiles: Object.fromEntries(
        contract.authority.tailwindProfiles.map((profile) => [
          profile.id,
          contract.corpus.projects.filter(
            (project) => project.tailwindProfile === profile.id,
          ).length,
        ]),
      ),
    },
    projects: contract.corpus.projects.map((project) => ({
      id: project.id,
      adapter: project.adapter,
      tailwindProfile: project.tailwindProfile,
      passed: true,
      source: { classGroups: 3 },
      tailwind: {
        version: contract.authority.tailwindProfiles.find(
          (profile) => profile.id === project.tailwindProfile,
        ).version,
        sourceInventory: { dynamic: 0, unsupported: 0, preflightReliance: "not-observed" },
        audit: { severities: { error: 0 }, codes: ["PCSS-AUDIT-000"] },
      },
      migration: { replacements: 3, tailwindRetained: true, rollback: { exact: true } },
      vite:
        project.adapter === "vite"
          ? {
              version: contract.authority.adapters.find((adapter) => adapter.id === "vite").version,
              baseline: { assets: [{ path: "index.html" }] },
              candidate: { assets: [{ path: "index.html" }] },
            }
          : null,
      equivalence: {
        domEquivalent: true,
        ariaEquivalent: true,
        computedStyleEquivalent: true,
        geometryEquivalent: true,
        maximumObservedGeometryDeltaCssPx: 0,
      },
    })),
  };
}

function pliegorsEvidence() {
  return {
    schema: "pliegocss/pliegors-browser-gate/2",
    passed: true,
    pliegocssSource: { ...source },
    pliegorsContract: readJson(
      resolve(repositoryRoot, "integration-tests", "pliegors-smoke", "pliegors-contract.json"),
    ),
    browser: { product: "Chrome contract test" },
    expected: { finalMinutes: 20 },
    final: {
      documentIdentity: true,
      nodeIdentity: { island: true, button: true, value: true },
      eventCount: 1,
      finalMinutes: 20,
    },
    relevantEvents: [],
    serverErrors: [],
  };
}

function writeCase(name, mutate = () => {}) {
  const root = join(targetRoot, name);
  mkdirSync(root, { recursive: true });
  const documents = contract.authority.hosts.map(evidence);
  const pliegors = pliegorsEvidence();
  mutate(documents, pliegors);
  for (const document of documents) {
    writeFileSync(join(root, `${document.host.id}.json`), `${JSON.stringify(document, null, 2)}\n`);
  }
  const pliegorsPath = join(root, "pliegors-framework.json");
  writeFileSync(pliegorsPath, `${JSON.stringify(pliegors, null, 2)}\n`);
  return { root, pliegorsPath };
}

function run({ root, pliegorsPath }) {
  return spawnSync(
    process.execPath,
    [
      resolve(repositoryRoot, "scripts", "check-adapter-coexistence-matrix.mjs"),
      `--evidence-root=${relative(repositoryRoot, root)}`,
      `--pliegors-evidence=${relative(repositoryRoot, pliegorsPath)}`,
      `--output=${relative(repositoryRoot, join(root, "matrix.json"))}`,
    ],
    { cwd: repositoryRoot, encoding: "utf8", windowsHide: true },
  );
}

const valid = run(writeCase("valid"));
if (valid.status !== 0) throw new Error(`valid matrix failed\n${valid.stdout}${valid.stderr}`);

for (const [name, mutate] of [
  ["missing-host", (documents) => documents.pop()],
  ["dirty-source", (documents) => {
    documents[0].source.dirty = true;
  }],
  ["aria-drift", (documents) => {
    documents[1].projects[0].equivalence.ariaEquivalent = false;
  }],
  ["audit-error", (documents) => {
    documents[2].projects[0].tailwind.audit.severities.error = 1;
  }],
  ["vite-evidence-missing", (documents) => {
    documents[0].projects.find((project) => project.adapter === "vite").vite = null;
  }],
  ["pliegors-dirty-source", (_documents, pliegors) => {
    pliegors.pliegocssSource.dirty = true;
  }],
]) {
  const result = run(writeCase(name, mutate));
  if (result.status === 0) throw new Error(`${name} matrix unexpectedly passed`);
}

rmSync(targetRoot, { recursive: true, force: true });
process.stdout.write("adapter coexistence matrix fail-closed contract: pass\n");
