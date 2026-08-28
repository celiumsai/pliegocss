import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync, statSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import {
  loadContract,
  repositoryRoot,
  sha256,
} from "./adapter-coexistence-v1.mjs";

function fail(message) {
  throw new Error(`adapter coexistence matrix: ${message}`);
}

function parseOptions(arguments_) {
  const options = {
    evidenceRoot: null,
    pliegorsEvidence: null,
    output: null,
    requireClean: false,
  };
  for (const argument of arguments_) {
    if (argument === "--") continue;
    if (argument === "--require-clean") {
      options.requireClean = true;
    } else if (argument.startsWith("--evidence-root=")) {
      options.evidenceRoot = resolve(repositoryRoot, argument.slice(16));
    } else if (argument.startsWith("--pliegors-evidence=")) {
      options.pliegorsEvidence = resolve(repositoryRoot, argument.slice(20));
    } else if (argument.startsWith("--output=")) {
      options.output = resolve(repositoryRoot, argument.slice(9));
    } else {
      fail(
        "usage: node scripts/check-adapter-coexistence-matrix.mjs --evidence-root=PATH --pliegors-evidence=PATH [--output=PATH] [--require-clean]",
      );
    }
  }
  if (!options.evidenceRoot) fail("--evidence-root is required");
  if (!options.pliegorsEvidence) fail("--pliegors-evidence is required");
  options.output ??= resolve(
    repositoryRoot,
    "target",
    "adapter-coexistence-certification",
    "adapter-coexistence-matrix.json",
  );
  for (const [role, path] of Object.entries({
    "evidence root": options.evidenceRoot,
    "PliegoRS evidence": options.pliegorsEvidence,
    output: options.output,
  })) {
    const child = relative(repositoryRoot, path);
    if (!child || child === ".." || child.startsWith(`..${sep}`) || isAbsolute(child)) {
      fail(`${role} must be inside the repository`);
    }
  }
  if (!options.output.endsWith(".json")) fail("--output must be a JSON path");
  if (!options.pliegorsEvidence.endsWith(".json")) fail("--pliegors-evidence must be a JSON path");
  return options;
}

function filesBelow(directory) {
  return readdirSync(directory, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name, "en"))
    .flatMap((entry) => {
      const path = join(directory, entry.name);
      return entry.isDirectory() ? filesBelow(path) : [path];
    });
}

const options = parseOptions(process.argv.slice(2));
if (
  options.requireClean &&
  execFileSync("git", ["status", "--porcelain=v1", "-z", "--untracked-files=all"], {
    cwd: repositoryRoot,
    encoding: "utf8",
  }).length !== 0
) {
  fail("--require-clean found a dirty matrix checkout");
}
if (!statSync(options.evidenceRoot).isDirectory()) fail("evidence root is not a directory");
const contract = loadContract();
const evidence = filesBelow(options.evidenceRoot)
  .filter((path) => path.endsWith(".json"))
  .map((path) => ({ path, value: JSON.parse(readFileSync(path, "utf8")) }))
  .filter(({ value }) => value.kind === "pliegocss-adapter-coexistence-host-evidence");
const expectedHosts = contract.authority.hosts.map((host) => host.id).sort();
const actualHosts = evidence.map(({ value }) => value.host?.id).sort();
if (JSON.stringify(actualHosts) !== JSON.stringify(expectedHosts)) {
  fail(`host inventory drifted: expected ${expectedHosts.join(", ")}, found ${actualHosts.join(", ")}`);
}

const expectedProjects = contract.corpus.projects.map((project) => project.id);
const projectContract = new Map(contract.corpus.projects.map((project) => [project.id, project]));
let sourceIdentity = null;
const hosts = [];
for (const { path, value } of evidence) {
  if (
    value.schemaVersion !== 1 ||
    value.result !== "pass" ||
    value.kind !== "pliegocss-adapter-coexistence-host-evidence"
  ) {
    fail(`${path} header or result drifted`);
  }
  const expectedHost = contract.authority.hosts.find((host) => host.id === value.host.id);
  for (const key of ["os", "arch", "browser"]) {
    if (value.host[key] !== expectedHost[key]) fail(`${value.host.id} ${key} drifted`);
  }
  if (!value.host.browserVersion || !value.host.playwright) {
    fail(`${value.host.id} browser/tooling identity is incomplete`);
  }
  if (
    value.authority?.sha256 !== contract.authoritySha256 ||
    value.authority?.corpusSha256 !== contract.corpusSha256
  ) {
    fail(`${value.host.id} authority identity drifted`);
  }
  if (value.source?.dirty !== false || !/^[0-9a-f]{40}$/u.test(value.source?.commit ?? "")) {
    fail(`${value.host.id} source is dirty or incomplete`);
  }
  const identity = `${value.source.commit}:${value.source.gitTree}`;
  sourceIdentity ??= identity;
  if (identity !== sourceIdentity) fail(`${value.host.id} is detached from the matrix source`);
  if (
    value.summary?.projects !== expectedProjects.length ||
    value.summary?.passed !== expectedProjects.length ||
    value.summary?.domEquivalent !== expectedProjects.length ||
    value.summary?.ariaEquivalent !== expectedProjects.length ||
    value.summary?.computedStyleEquivalent !== expectedProjects.length ||
    value.summary?.geometryEquivalent !== expectedProjects.length ||
    value.summary?.exactRollbacks !== expectedProjects.length ||
    JSON.stringify(value.summary?.adapters) !==
      JSON.stringify(
        Object.fromEntries(
          contract.authority.adapters.map((adapter) => [
            adapter.id,
            contract.corpus.projects.filter((project) => project.adapter === adapter.id).length,
          ]),
        ),
      ) ||
    JSON.stringify(value.summary?.tailwindProfiles) !==
      JSON.stringify(
        Object.fromEntries(
          contract.authority.tailwindProfiles.map((profile) => [
            profile.id,
            contract.corpus.projects.filter(
              (project) => project.tailwindProfile === profile.id,
            ).length,
          ]),
        ),
      )
  ) {
    fail(`${value.host.id} summary is incomplete`);
  }
  if (JSON.stringify(value.projects?.map((project) => project.id)) !== JSON.stringify(expectedProjects)) {
    fail(`${value.host.id} project order or coverage drifted`);
  }
  for (const project of value.projects) {
    const expectedProject = projectContract.get(project.id);
    const expectedProfile = contract.authority.tailwindProfiles.find(
      (profile) => profile.id === expectedProject.tailwindProfile,
    );
    if (
      project.passed !== true ||
      project.adapter !== expectedProject.adapter ||
      project.tailwindProfile !== expectedProject.tailwindProfile ||
      project.tailwind?.version !== expectedProfile.version ||
      project.migration?.tailwindRetained !== true ||
      project.migration?.rollback?.exact !== true ||
      !Number.isInteger(project.source?.classGroups) ||
      project.source.classGroups < 1 ||
      project.migration?.replacements !== project.source.classGroups ||
      project.equivalence?.domEquivalent !== true ||
      project.equivalence?.ariaEquivalent !== true ||
      project.equivalence?.computedStyleEquivalent !== true ||
      project.equivalence?.geometryEquivalent !== true ||
      project.tailwind?.sourceInventory?.dynamic !== 0 ||
      project.tailwind?.sourceInventory?.unsupported !== 0 ||
      project.tailwind?.sourceInventory?.preflightReliance !== "not-observed" ||
      project.tailwind?.audit?.severities?.error !== 0 ||
      !project.tailwind?.audit?.codes?.includes("PCSS-AUDIT-000")
    ) {
      fail(`${value.host.id}/${project.id} did not close every required gate`);
    }
    if (
      expectedProject.adapter === "vite" &&
      (project.vite?.version !==
        contract.authority.adapters.find((adapter) => adapter.id === "vite").version ||
        !project.vite?.baseline?.assets?.length ||
        !project.vite?.candidate?.assets?.length)
    ) {
      fail(`${value.host.id}/${project.id} has no complete Vite production-build evidence`);
    }
    if (
      !Number.isFinite(project.equivalence.maximumObservedGeometryDeltaCssPx) ||
      project.equivalence.maximumObservedGeometryDeltaCssPx >
      contract.authority.comparison.maximumLayoutGeometryDeltaCssPx
    ) {
      fail(`${value.host.id}/${project.id} exceeded the geometry bound`);
    }
  }
  hosts.push({
    id: value.host.id,
    os: value.host.os,
    arch: value.host.arch,
    browser: value.host.browser,
    browserVersion: value.host.browserVersion,
    evidenceSha256: sha256(readFileSync(path)),
    projects: value.summary.projects,
    passed: true,
  });
}
hosts.sort((left, right) => left.id.localeCompare(right.id, "en"));

const [commit, gitTree] = sourceIdentity.split(":");
const checkoutCommit = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: repositoryRoot,
  encoding: "utf8",
}).trim();
const checkoutTree = execFileSync("git", ["rev-parse", "HEAD^{tree}"], {
  cwd: repositoryRoot,
  encoding: "utf8",
}).trim();
if (commit !== checkoutCommit || gitTree !== checkoutTree) {
  fail("host evidence does not belong to the matrix checkout");
}

const pliegorsEvidenceBytes = readFileSync(options.pliegorsEvidence);
const pliegors = JSON.parse(pliegorsEvidenceBytes.toString("utf8"));
const expectedPliegorsContract = JSON.parse(
  readFileSync(resolve(repositoryRoot, "integration-tests", "pliegors-smoke", "pliegors-contract.json"), "utf8"),
);
if (
  pliegors.schema !== "pliegocss/pliegors-browser-gate/2" ||
  pliegors.passed !== true ||
  JSON.stringify(pliegors.pliegorsContract) !== JSON.stringify(expectedPliegorsContract) ||
  pliegors.pliegocssSource?.dirty !== false ||
  pliegors.pliegocssSource?.commit !== commit ||
  pliegors.pliegocssSource?.gitTree !== gitTree ||
  !pliegors.browser?.product ||
  pliegors.final?.documentIdentity !== true ||
  pliegors.final?.nodeIdentity?.island !== true ||
  pliegors.final?.nodeIdentity?.button !== true ||
  pliegors.final?.nodeIdentity?.value !== true ||
  pliegors.final?.eventCount !== 1 ||
  pliegors.final?.finalMinutes !== pliegors.expected?.finalMinutes ||
  (pliegors.relevantEvents?.length ?? -1) !== 0 ||
  (pliegors.serverErrors?.length ?? -1) !== 0
) {
  fail("PliegoRS framework/browser evidence is incomplete or detached from the matrix source");
}
const matrix = {
  schemaVersion: 1,
  kind: "pliegocss-adapter-coexistence-matrix",
  result: "pass",
  source: { commit, gitTree },
  authority: {
    sha256: contract.authoritySha256,
    corpusSha256: contract.corpusSha256,
  },
  coverage: {
    hosts: hosts.length,
    projectsPerHost: expectedProjects.length,
    totalProjectReplays: hosts.length * expectedProjects.length,
    adapters: Object.fromEntries(
      contract.authority.adapters.map((adapter) => [
        adapter.id,
        contract.corpus.projects.filter((project) => project.adapter === adapter.id).length,
      ]),
    ),
    tailwindProfiles: Object.fromEntries(
      contract.authority.tailwindProfiles.map((profile) => [
        profile.id,
        contract.corpus.projects.filter((project) => project.tailwindProfile === profile.id).length,
      ]),
    ),
  },
  hosts,
  pliegorsFramework: {
    revision: pliegors.pliegorsContract.revision,
    sourceSha256: pliegors.pliegorsContract.sourceSha256,
    browser: pliegors.browser.product,
    evidenceSha256: sha256(pliegorsEvidenceBytes),
    passed: true,
  },
  claimBoundary: contract.authority.claimBoundary,
};
mkdirSync(dirname(options.output), { recursive: true });
writeFileSync(options.output, `${JSON.stringify(matrix, null, 2)}\n`);
process.stdout.write(
  `${JSON.stringify(
    {
      passed: true,
      hosts: hosts.length,
      projectsPerHost: expectedProjects.length,
      totalProjectReplays: matrix.coverage.totalProjectReplays,
      output: relative(repositoryRoot, options.output).replaceAll("\\", "/"),
    },
    null,
    2,
  )}\n`,
);
