import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { basename, resolve } from "node:path";
import {
  ROOT,
  digest,
  exactKeys,
  fail,
  fileSha256,
  readJson,
  repositoryVersion,
  safeFilename,
  validateCycloneDx,
  validatePackageManifest,
  validatePolicy,
  validateSource,
} from "./repository-distribution.mjs";

function parse(args) {
  const parsed = {};
  for (const arg of args) {
    if (arg === "--") continue;
    const match = /^--([a-z-]+)=(.+)$/u.exec(arg);
    if (!match || Object.hasOwn(parsed, match[1])) fail(`invalid option: ${arg}`);
    parsed[match[1]] = match[2];
  }
  return parsed;
}

function tar(args) {
  const result = spawnSync("tar", args, {
    cwd: ROOT,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    fail(`tar ${args.join(" ")} failed: ${result.error?.message ?? result.stderr.trim()}`);
  }
  return result.stdout;
}

const options = parse(process.argv.slice(2));
if (!options.root) fail("--root is required");
const bundleRoot = resolve(ROOT, options.root);
const version = repositoryVersion();
const policy = validatePolicy();
const manifestPath = resolve(bundleRoot, `pliegocss-distribution-${version}.json`);
if (!existsSync(manifestPath)) fail(`distribution manifest is missing: ${manifestPath}`);
const manifest = readJson(manifestPath, "distribution manifest");
exactKeys(
  manifest,
  [
    "schemaVersion",
    "kind",
    "version",
    "repository",
    "source",
    "packageManager",
    "immutableReleaseRequired",
    "supportedTargets",
    "artifacts",
    "claimBoundary",
  ],
  "distribution manifest",
);
if (
  manifest.schemaVersion !== 1 ||
  manifest.kind !== "pliegocss-repository-distribution" ||
  manifest.version !== version ||
  manifest.repository !== policy.repository ||
  manifest.immutableReleaseRequired !== true ||
  manifest.claimBoundary !== policy.claimBoundary
) {
  fail("distribution manifest identity drifted");
}
validateSource(manifest.source);
exactKeys(
  manifest.packageManager,
  ["name", "version", "installSource", "npmjsPublished", "lifecycleScripts"],
  "distribution package manager",
);
if (
  manifest.packageManager.name !== "pnpm" ||
  manifest.packageManager.version !== policy.package.packageManagerVersion ||
  manifest.packageManager.installSource !== "verified-local-github-release-asset" ||
  manifest.packageManager.npmjsPublished !== false ||
  manifest.packageManager.lifecycleScripts !== false
) {
  fail("distribution package-manager boundary drifted");
}
const expectedTargets = policy.targets.map(({ id, os, arch }) => ({ id, os, arch }));
if (JSON.stringify(manifest.supportedTargets) !== JSON.stringify(expectedTargets)) {
  fail("supported target inventory drifted");
}
if (!Array.isArray(manifest.artifacts) || manifest.artifacts.length !== 10) {
  fail("distribution must contain three archives, six SBOMs, and one pnpm package");
}
const expectedArtifacts = new Map();
for (const target of policy.targets) {
  expectedArtifacts.set(`pliegocss-${version}-${target.id}.tar.gz`, {
    kind: "native-archive",
    target: target.id,
  });
  for (const binary of policy.binaries) {
    expectedArtifacts.set(`${target.id}-${binary}.cdx.json`, {
      kind: "sbom",
      target: target.id,
      binary,
    });
  }
}
expectedArtifacts.set(`pliegocss-pnpm-${version}.tgz`, {
  kind: "pnpm-package",
  target: "universal-declared-hosts",
});
const artifactNames = new Set();
for (const artifact of manifest.artifacts) {
  if (!artifact || typeof artifact !== "object" || Array.isArray(artifact)) {
    fail("distribution artifact must be an object");
  }
  safeFilename(artifact.file, "distribution artifact file");
  const expected = expectedArtifacts.get(artifact.file);
  if (!expected || artifact.kind !== expected.kind || artifact.target !== expected.target) {
    fail(`${artifact.file} type or target drifted`);
  }
  if (artifact.kind === "native-archive") {
    exactKeys(
      artifact,
      ["kind", "file", "bytes", "sha256", "target", "binaries", "sboms"],
      `${artifact.file} manifest entry`,
    );
    if (
      !Array.isArray(artifact.binaries) ||
      !Array.isArray(artifact.sboms) ||
      artifact.binaries.length !== policy.binaries.length ||
      artifact.sboms.length !== policy.binaries.length
    ) {
      fail(`${artifact.file} receipt inventory drifted`);
    }
  } else if (artifact.kind === "sbom") {
    exactKeys(
      artifact,
      ["kind", "file", "bytes", "sha256", "target", "binary"],
      `${artifact.file} manifest entry`,
    );
    if (artifact.binary !== expected.binary) fail(`${artifact.file} binary drifted`);
  } else {
    exactKeys(
      artifact,
      [
        "kind",
        "file",
        "bytes",
        "sha256",
        "target",
        "package",
        "lifecycleScripts",
        "dependencies",
        "registryPublication",
      ],
      `${artifact.file} manifest entry`,
    );
    if (
      artifact.package !== policy.package.name ||
      artifact.lifecycleScripts !== false ||
      artifact.dependencies !== 0 ||
      artifact.registryPublication !== false
    ) {
      fail("pnpm package release boundary drifted");
    }
  }
  if (artifactNames.has(artifact.file)) fail(`duplicate artifact ${artifact.file}`);
  artifactNames.add(artifact.file);
  if (!Number.isSafeInteger(artifact.bytes) || artifact.bytes <= 0) {
    fail(`${artifact.file}.bytes must be a positive safe integer`);
  }
  digest(artifact.sha256, `${artifact.file}.sha256`);
  const path = resolve(bundleRoot, artifact.file);
  if (!existsSync(path) || !statSync(path).isFile()) fail(`artifact missing: ${artifact.file}`);
  if (fileSha256(path) !== artifact.sha256 || statSync(path).size !== artifact.bytes) {
    fail(`artifact hash or size drifted: ${artifact.file}`);
  }
  if (artifact.kind === "sbom") {
    validateCycloneDx(path, artifact.binary, version);
  }
}
const expectedArtifactNames = new Set(expectedArtifacts.keys());
if (
  artifactNames.size !== expectedArtifactNames.size ||
  [...expectedArtifactNames].some((file) => !artifactNames.has(file))
) {
  fail("distribution artifact filename inventory drifted");
}
for (const target of policy.targets) {
  const archiveName = `pliegocss-${version}-${target.id}.tar.gz`;
  if (!artifactNames.has(archiveName)) fail(`native archive missing for ${target.id}`);
  const archiveFiles = tar(["-tzf", resolve(bundleRoot, archiveName)])
    .replaceAll("\\", "/")
    .split(/\r?\n/u)
    .map((line) => line.replace(/^\.\//u, ""))
    .filter((line) => line && !line.endsWith("/"));
  const expectedArchiveFiles = new Set([
    ...policy.binaries.map((binary) => `${binary}${target.extension}`),
    ...policy.binaries.map((binary) => `${binary}.cdx.json`),
    "native.json",
    "LICENSE",
  ]);
  if (
    archiveFiles.length !== expectedArchiveFiles.size ||
    archiveFiles.some((file) => !expectedArchiveFiles.has(file))
  ) {
    fail(`${archiveName} file inventory drifted`);
  }
}
const packageName = `pliegocss-pnpm-${version}.tgz`;
if (!artifactNames.has(packageName)) fail("pnpm package artifact is missing");
const packagePath = resolve(bundleRoot, packageName);
const packageFiles = tar(["-tzf", packagePath])
  .replaceAll("\\", "/")
  .split(/\r?\n/u)
  .filter((line) => line && !line.endsWith("/"));
const expectedPackageFiles = new Set([
  "package/package.json",
  "package/bin/pliego-cssc.mjs",
  "package/bin/pliego-css-lsp.mjs",
  "package/bin/launcher.mjs",
  "package/LICENSE",
  "package/README.md",
]);
for (const target of policy.targets) {
  for (const binary of policy.binaries) {
    expectedPackageFiles.add(`package/vendor/${target.id}/${binary}${target.extension}`);
    expectedPackageFiles.add(`package/SBOM/${target.id}-${binary}.cdx.json`);
  }
}
if (
  packageFiles.length !== expectedPackageFiles.size ||
  packageFiles.some((file) => !expectedPackageFiles.has(file))
) {
  fail("pnpm package file inventory drifted");
}
const packedManifest = JSON.parse(tar(["-xOzf", packagePath, "package/package.json"]));
validatePackageManifest(packedManifest, policy, { packed: true });

const checksumsPath = resolve(bundleRoot, "SHA256SUMS");
if (!existsSync(checksumsPath)) fail("SHA256SUMS is missing");
const checksumLines = readFileSync(checksumsPath, "utf8").trimEnd().split(/\r?\n/u);
const checksums = new Map();
for (const line of checksumLines) {
  const match = /^([0-9a-f]{64})  ([A-Za-z0-9@._-]+)$/u.exec(line);
  if (!match || checksums.has(match[2])) fail(`invalid checksum line: ${line}`);
  checksums.set(match[2], match[1]);
}
const expectedChecksums = new Set([...artifactNames, basename(manifestPath)]);
if (
  checksums.size !== expectedChecksums.size ||
  [...expectedChecksums].some((file) => !checksums.has(file))
) {
  fail("SHA256SUMS inventory drifted");
}
for (const [file, expected] of checksums) {
  if (fileSha256(resolve(bundleRoot, file)) !== expected) fail(`checksum mismatch for ${file}`);
}
const allowedFiles = new Set([...expectedChecksums, "SHA256SUMS"]);
const actualFiles = readdirSync(bundleRoot).filter((file) => statSync(resolve(bundleRoot, file)).isFile());
if (actualFiles.some((file) => !allowedFiles.has(file)) || actualFiles.length !== allowedFiles.size) {
  fail("release asset directory contains an undeclared file");
}

process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 1,
      result: "passed",
      version,
      source: manifest.source,
      targets: policy.targets.length,
      nativeArchives: 3,
      sboms: 6,
      pnpmPackage: packageName,
      npmjsPublished: false,
      lifecycleScripts: false,
    },
    null,
    2,
  )}\n`,
);
