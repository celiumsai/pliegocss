import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, resolve } from "node:path";
import {
  ROOT,
  assertChildPath,
  digest,
  exactKeys,
  fail,
  fileSha256,
  readJson,
  repositoryVersion,
  safeFilename,
  validateBinary,
  validateCycloneDx,
  validatePackageManifest,
  validatePolicy,
  validateSource,
} from "./repository-distribution.mjs";

function parse(args) {
  const value = { allowDirty: false };
  for (const arg of args) {
    if (arg === "--allow-dirty") {
      value.allowDirty = true;
      continue;
    }
    const match = /^--([a-z-]+)=(.+)$/u.exec(arg);
    if (!match || Object.hasOwn(value, match[1])) fail(`invalid option: ${arg}`);
    value[match[1]] = match[2];
  }
  return value;
}

function run(command, args, { cwd = ROOT, capture = false } = {}) {
  const executable =
    process.platform === "win32" && command === "pnpm"
      ? process.env.ComSpec ?? "cmd.exe"
      : command;
  const commandArgs =
    process.platform === "win32" && command === "pnpm"
      ? ["/d", "/s", "/c", command, ...args]
      : args;
  const result = spawnSync(executable, commandArgs, {
    cwd,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit",
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    const details = capture ? `${result.stdout ?? ""}${result.stderr ?? ""}`.trim() : "";
    fail(
      `${command} ${args.join(" ")} failed: ${result.error?.message ?? (details || result.status)}`,
    );
  }
  return result.stdout?.trim() ?? "";
}

function safeOutput(path) {
  return assertChildPath(resolve(ROOT, "target"), path, "distribution output");
}

function artifact(path, kind, extra = {}) {
  return {
    kind,
    file: basename(path),
    bytes: statSync(path).size,
    sha256: fileSha256(path),
    ...extra,
  };
}

const options = parse(process.argv.slice(2));
if (!options.input || !options.output) fail("--input and --output are required");
const inputRoot = resolve(ROOT, options.input);
const outputRoot = safeOutput(resolve(ROOT, options.output));
const dirty = run("git", ["status", "--porcelain=v1"], { capture: true });
if (dirty && !options.allowDirty) fail("distribution build requires a clean worktree");
const version = repositoryVersion();
const source = {
  commit: options["source-commit"] ?? run("git", ["rev-parse", "HEAD"], { capture: true }),
  gitTree:
    options["git-tree"] ??
    run("git", ["show", "-s", "--format=%T", "HEAD"], { capture: true }),
  tag: options.tag ?? null,
};
validateSource(source);
if (source.tag !== null && source.tag !== `v${version}`) {
  fail(`source tag ${source.tag} does not match package version ${version}`);
}
const policy = validatePolicy();
validatePackageManifest(undefined, policy);

rmSync(outputRoot, { recursive: true, force: true });
const releaseRoot = resolve(outputRoot, "release-assets");
const stageRoot = resolve(outputRoot, "stage");
mkdirSync(releaseRoot, { recursive: true });
mkdirSync(stageRoot, { recursive: true });

const nativeInputs = [];
for (const target of policy.targets) {
  const root = resolve(inputRoot, `native-${target.id}`);
  const receiptPath = resolve(root, "native.json");
  if (!existsSync(receiptPath)) fail(`native receipt missing for ${target.id}`);
  const receipt = readJson(receiptPath, `${target.id} native receipt`);
  exactKeys(
    receipt,
    ["schemaVersion", "kind", "version", "source", "target", "binaries", "sboms"],
    `${target.id} native receipt`,
  );
  if (
    receipt.schemaVersion !== 1 ||
    receipt.kind !== "pliegocss-native-distribution-input" ||
    receipt.version !== version ||
    receipt.target.id !== target.id
  ) {
    fail(`${target.id} native receipt identity drifted`);
  }
  exactKeys(receipt.target, ["id", "os", "arch", "format"], `${target.id} receipt target`);
  const expectedTarget = {
    id: target.id,
    os: target.os,
    arch: target.arch,
    format: target.format,
  };
  if (JSON.stringify(receipt.target) !== JSON.stringify(expectedTarget)) {
    fail(`${target.id} native receipt target drifted`);
  }
  validateSource(receipt.source);
  if (JSON.stringify(receipt.source) !== JSON.stringify(source)) {
    fail(`${target.id} native input is not bound to the aggregate source`);
  }
  if (!Array.isArray(receipt.binaries) || receipt.binaries.length !== policy.binaries.length) {
    fail(`${target.id} native receipt binary inventory drifted`);
  }
  for (const [index, binary] of receipt.binaries.entries()) {
    exactKeys(binary, ["name", "file", "bytes", "sha256"], `${target.id} binary receipt`);
    const expectedName = policy.binaries[index];
    const expectedFile = `${expectedName}${target.extension}`;
    if (
      binary.name !== expectedName ||
      binary.file !== expectedFile ||
      !Number.isSafeInteger(binary.bytes) ||
      binary.bytes <= 0
    ) {
      fail(`${target.id} binary receipt inventory drifted`);
    }
    safeFilename(binary.file, `${target.id} binary file`);
    digest(binary.sha256, `${target.id}/${binary.file}.sha256`);
    const path = resolve(root, binary.file);
    const details = validateBinary(path, target);
    if (details.sha256 !== binary.sha256 || details.bytes !== binary.bytes) {
      fail(`${target.id}/${binary.file} drifted after native staging`);
    }
  }
  if (!Array.isArray(receipt.sboms) || receipt.sboms.length !== policy.binaries.length) {
    fail(`${target.id} native receipt SBOM inventory drifted`);
  }
  for (const [index, sbom] of receipt.sboms.entries()) {
    exactKeys(sbom, ["binary", "file", "sha256"], `${target.id} SBOM receipt`);
    const expectedBinary = policy.binaries[index];
    if (
      sbom.binary !== expectedBinary ||
      sbom.file !== `${expectedBinary}.cdx.json`
    ) {
      fail(`${target.id} SBOM receipt inventory drifted`);
    }
    safeFilename(sbom.file, `${target.id} SBOM file`);
    digest(sbom.sha256, `${target.id}/${sbom.file}.sha256`);
    const path = resolve(root, sbom.file);
    validateCycloneDx(path, sbom.binary, version);
    if (fileSha256(path) !== sbom.sha256) {
      fail(`${target.id}/${sbom.file} drifted after native staging`);
    }
  }
  nativeInputs.push({ target, root, receipt });
}

const packageStage = resolve(stageRoot, "pnpm-package");
mkdirSync(resolve(packageStage, "bin"), { recursive: true });
mkdirSync(resolve(packageStage, "vendor"), { recursive: true });
mkdirSync(resolve(packageStage, "SBOM"), { recursive: true });
for (const file of ["package.json", "README.md"]) {
  copyFileSync(resolve(ROOT, "packages", "cli", file), resolve(packageStage, file));
}
copyFileSync(resolve(ROOT, "LICENSE"), resolve(packageStage, "LICENSE"));
for (const file of readdirSync(resolve(ROOT, "packages", "cli", "bin"))) {
  copyFileSync(
    resolve(ROOT, "packages", "cli", "bin", file),
    resolve(packageStage, "bin", file),
  );
}
for (const { target, root, receipt } of nativeInputs) {
  const vendor = resolve(packageStage, "vendor", target.id);
  mkdirSync(vendor, { recursive: true });
  for (const binary of receipt.binaries) {
    const destination = resolve(vendor, binary.file);
    copyFileSync(resolve(root, binary.file), destination);
    if (target.os !== "win32") chmodSync(destination, 0o755);
  }
  for (const sbom of receipt.sboms) {
    copyFileSync(
      resolve(root, sbom.file),
      resolve(packageStage, "SBOM", `${target.id}-${sbom.file}`),
    );
  }
}
run("pnpm", ["pack", "--pack-destination", releaseRoot], { cwd: packageStage });
const packed = readdirSync(releaseRoot).filter((file) => file.endsWith(".tgz"));
if (packed.length !== 1) fail(`pnpm pack produced ${packed.length} tarballs`);
const pnpmAsset = resolve(releaseRoot, `pliegocss-pnpm-${version}.tgz`);
const originalPnpmAsset = resolve(releaseRoot, packed[0]);
if (originalPnpmAsset !== pnpmAsset) {
  copyFileSync(originalPnpmAsset, pnpmAsset);
  rmSync(originalPnpmAsset);
}

const artifacts = [];
for (const { target, root, receipt } of nativeInputs) {
  const nativeStage = resolve(stageRoot, target.id);
  mkdirSync(nativeStage, { recursive: true });
  for (const file of ["LICENSE", "native.json", ...receipt.binaries.map((x) => x.file), ...receipt.sboms.map((x) => x.file)]) {
    copyFileSync(resolve(root, file), resolve(nativeStage, file));
  }
  for (const binary of receipt.binaries) {
    if (target.os !== "win32") chmodSync(resolve(nativeStage, binary.file), 0o755);
  }
  const archive = resolve(
    releaseRoot,
    `pliegocss-${version}-${target.id}.tar.gz`,
  );
  run("tar", ["-czf", archive, "-C", nativeStage, "."]);
  artifacts.push(
    artifact(archive, "native-archive", {
      target: target.id,
      binaries: receipt.binaries,
      sboms: receipt.sboms,
    }),
  );
  for (const sbom of receipt.sboms) {
    const sbomAsset = resolve(releaseRoot, `${target.id}-${sbom.file}`);
    copyFileSync(resolve(root, sbom.file), sbomAsset);
    artifacts.push(
      artifact(sbomAsset, "sbom", { target: target.id, binary: sbom.binary }),
    );
  }
}
artifacts.push(
  artifact(pnpmAsset, "pnpm-package", {
    target: "universal-declared-hosts",
    package: policy.package.name,
    lifecycleScripts: false,
    dependencies: 0,
    registryPublication: false,
  }),
);
artifacts.sort((left, right) => left.file.localeCompare(right.file, "en"));

const manifest = {
  schemaVersion: 1,
  kind: "pliegocss-repository-distribution",
  version,
  repository: policy.repository,
  source,
  packageManager: {
    name: "pnpm",
    version: policy.package.packageManagerVersion,
    installSource: "verified-local-github-release-asset",
    npmjsPublished: false,
    lifecycleScripts: false,
  },
  immutableReleaseRequired: true,
  supportedTargets: policy.targets.map(({ id, os, arch }) => ({ id, os, arch })),
  artifacts,
  claimBoundary: policy.claimBoundary,
};
const manifestPath = resolve(
  releaseRoot,
  `pliegocss-distribution-${version}.json`,
);
writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

const checksumFiles = [...artifacts.map((entry) => entry.file), basename(manifestPath)].sort();
const checksumText = `${checksumFiles
  .map((file) => `${fileSha256(resolve(releaseRoot, file))}  ${file}`)
  .join("\n")}\n`;
writeFileSync(resolve(releaseRoot, "SHA256SUMS"), checksumText);

process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 1,
      version,
      source,
      releaseRoot,
      artifacts: artifacts.length + 2,
      pnpmPackage: basename(pnpmAsset),
      worktree: dirty ? "dirty-explicitly-allowed" : "clean",
    },
    null,
    2,
  )}\n`,
);
