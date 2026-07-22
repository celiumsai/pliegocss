import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { basename, isAbsolute, relative, resolve } from "node:path";

export const ROOT = resolve(import.meta.dirname, "..");
export const POLICY_PATH = resolve(
  ROOT,
  "distribution",
  "repository-distribution-v1.json",
);
export const PACKAGE_PATH = resolve(ROOT, "packages", "cli", "package.json");

const COMMIT = /^[0-9a-f]{40}$/u;
const DIGEST = /^[0-9a-f]{64}$/u;
const RELEASE_TAG = /^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/u;

export function fail(message) {
  throw new Error(`repository distribution: ${message}`);
}

export function readJson(path, role) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    fail(`${role} is not valid JSON: ${error.message}`);
  }
}

export function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

export function fileSha256(path) {
  return sha256(readFileSync(path));
}

export function exactKeys(value, expected, role) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    fail(`${role} must be an object`);
  }
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    fail(`${role} fields drifted: ${actual.join(", ")}`);
  }
}

export function repositoryVersion() {
  const manifest = readFileSync(resolve(ROOT, "Cargo.toml"), "utf8");
  const match = /^version = "([^"]+)"$/mu.exec(manifest);
  if (!match) fail("workspace package version is missing");
  return match[1];
}

export function validateSource(source, { allowNullTag = true } = {}) {
  exactKeys(source, ["commit", "gitTree", "tag"], "source");
  if (!COMMIT.test(source.commit) || !COMMIT.test(source.gitTree)) {
    fail("source commit and Git tree must be full lowercase object IDs");
  }
  if (
    source.tag !== null &&
    (typeof source.tag !== "string" || !RELEASE_TAG.test(source.tag))
  ) {
    fail("source tag must be a canonical release tag or null");
  }
  if (!allowNullTag && source.tag === null) fail("release source tag is required");
}

export function validatePolicy(policy = readJson(POLICY_PATH, "policy")) {
  exactKeys(
    policy,
    [
      "schemaVersion",
      "kind",
      "repository",
      "releaseAssetBase",
      "package",
      "release",
      "binaries",
      "targets",
      "claimBoundary",
    ],
    "policy",
  );
  if (
    policy.schemaVersion !== 1 ||
    policy.kind !== "pliegocss-repository-distribution-policy" ||
    policy.repository !== "https://github.com/celiumsai/pliegocss" ||
    policy.releaseAssetBase !==
      "https://github.com/celiumsai/pliegocss/releases/download"
  ) {
    fail("policy identity drifted");
  }
  exactKeys(
    policy.package,
    [
      "name",
      "format",
      "registryPublication",
      "distributionChannel",
      "packageManager",
      "packageManagerVersion",
      "lifecycleScripts",
      "dependencies",
    ],
    "policy.package",
  );
  if (
    policy.package.name !== "@pliegocss/cli" ||
    policy.package.format !== "npm-tarball" ||
    policy.package.registryPublication !== "forbidden" ||
    policy.package.distributionChannel !== "github-release-asset" ||
    policy.package.packageManager !== "pnpm" ||
    policy.package.packageManagerVersion !== "11.7.0" ||
    policy.package.lifecycleScripts !== "forbidden" ||
    policy.package.dependencies !== "forbidden"
  ) {
    fail("repository-only pnpm package policy drifted");
  }
  exactKeys(
    policy.release,
    [
      "draftFirst",
      "immutableRequiredBeforePublication",
      "checksums",
      "sbom",
      "sbomTool",
      "sbomToolVersion",
      "provenance",
    ],
    "policy.release",
  );
  if (
    policy.release.draftFirst !== true ||
    policy.release.immutableRequiredBeforePublication !== true ||
    policy.release.checksums !== "sha256" ||
    policy.release.sbom !== "cyclonedx-json-1.5" ||
    policy.release.sbomTool !== "cargo-cyclonedx" ||
    policy.release.sbomToolVersion !== "0.5.9" ||
    policy.release.provenance !== "github-sigstore-attestation"
  ) {
    fail("release security policy drifted");
  }
  if (
    JSON.stringify(policy.binaries) !==
    JSON.stringify(["pliego-cssc", "pliego-css-lsp"])
  ) {
    fail("binary boundary drifted");
  }
  if (!Array.isArray(policy.targets) || policy.targets.length !== 3) {
    fail("exactly three native targets are required");
  }
  const expectedTargets = [
    {
      id: "x86_64-pc-windows-msvc",
      os: "win32",
      arch: "x64",
      runner: "windows-latest",
      extension: ".exe",
      format: "pe",
    },
    {
      id: "x86_64-unknown-linux-gnu",
      os: "linux",
      arch: "x64",
      runner: "ubuntu-24.04",
      extension: "",
      format: "elf",
    },
    {
      id: "aarch64-apple-darwin",
      os: "darwin",
      arch: "arm64",
      runner: "macos-15",
      extension: "",
      format: "mach-o",
    },
  ];
  const targetIds = new Set();
  const hostPairs = new Set();
  for (const target of policy.targets) {
    exactKeys(
      target,
      ["id", "os", "arch", "runner", "extension", "format"],
      `policy.targets.${target.id ?? "unknown"}`,
    );
    if (!/^[A-Za-z0-9_-]+$/u.test(target.id ?? "") || targetIds.has(target.id)) {
      fail(`target ID is invalid or duplicated: ${target.id}`);
    }
    targetIds.add(target.id);
    const pair = `${target.os}:${target.arch}`;
    if (hostPairs.has(pair)) fail(`host pair is duplicated: ${pair}`);
    hostPairs.add(pair);
    if (!new Set(["pe", "elf", "mach-o"]).has(target.format)) {
      fail(`unsupported binary format for ${target.id}`);
    }
  }
  if (JSON.stringify(policy.targets) !== JSON.stringify(expectedTargets)) {
    fail("native target matrix drifted");
  }
  if (typeof policy.claimBoundary !== "string" || !policy.claimBoundary) {
    fail("claim boundary is missing");
  }
  return policy;
}

export function validatePackageManifest(
  manifest = readJson(PACKAGE_PATH, "pnpm package"),
  policy = validatePolicy(),
  { packed = false } = {},
) {
  const forbiddenLifecycle = new Set([
    "preinstall",
    "install",
    "postinstall",
    "prepare",
    "prepublish",
    "prepublishOnly",
  ]);
  if (
    manifest.name !== policy.package.name ||
    manifest.version !== repositoryVersion() ||
    manifest.private !== true ||
    (!packed && manifest.packageManager !== `pnpm@${policy.package.packageManagerVersion}`)
  ) {
    fail("pnpm package identity, version, or private boundary drifted");
  }
  if (
    manifest.engines?.pnpm !== ">=11.7.0 <12" ||
    (packed &&
      Object.hasOwn(manifest, "packageManager") &&
      manifest.packageManager !== `pnpm@${policy.package.packageManagerVersion}`)
  ) {
    fail("pnpm package manager recommendation drifted");
  }
  for (const dependencyField of [
    "dependencies",
    "devDependencies",
    "optionalDependencies",
    "peerDependencies",
  ]) {
    if (
      Object.hasOwn(manifest, dependencyField) &&
      Object.keys(manifest[dependencyField] ?? {}).length > 0
    ) {
      fail(`pnpm package must not contain ${dependencyField}`);
    }
  }
  for (const name of Object.keys(manifest.scripts ?? {})) {
    if (forbiddenLifecycle.has(name)) fail(`forbidden lifecycle script: ${name}`);
  }
  if (Object.keys(manifest.scripts ?? {}).length > 0) {
    fail("release pnpm package must not contain scripts");
  }
  if (
    JSON.stringify(manifest.bin) !==
    JSON.stringify({
      "pliego-cssc": "bin/pliego-cssc.mjs",
      "pliego-css-lsp": "bin/pliego-css-lsp.mjs",
    })
  ) {
    fail("pnpm binary launcher boundary drifted");
  }
  if (manifest.repository?.url !== "git+https://github.com/celiumsai/pliegocss.git") {
    fail("pnpm package repository must be the canonical GitHub repository");
  }
  if (Object.hasOwn(manifest.publishConfig ?? {}, "registry")) {
    fail("pnpm package must not configure an npm registry");
  }
  return manifest;
}

export function validateCycloneDx(path, binaryName, version) {
  const document = readJson(path, `${binaryName} SBOM`);
  if (
    document.bomFormat !== "CycloneDX" ||
    document.specVersion !== "1.5" ||
    document.version !== 1
  ) {
    fail(`${binaryName} SBOM must be CycloneDX JSON 1.5 document version 1`);
  }
  const component = document.metadata?.component;
  if (
    !component ||
    component.name !== binaryName ||
    component.version !== version
  ) {
    fail(`${binaryName} SBOM metadata does not identify ${binaryName}@${version}`);
  }
  if (!Array.isArray(document.components)) {
    fail(`${binaryName} SBOM components must be an array`);
  }
  return document;
}

export function validateBinary(path, target) {
  if (!existsSync(path)) fail(`native binary is missing: ${path}`);
  const bytes = readFileSync(path);
  if (bytes.byteLength < 64 * 1024) {
    fail(`${target.id} binary is implausibly small: ${bytes.byteLength} bytes`);
  }
  const magic = bytes.subarray(0, 4).toString("hex");
  const valid =
    (target.format === "pe" && magic.startsWith("4d5a")) ||
    (target.format === "elf" && magic === "7f454c46") ||
    (target.format === "mach-o" && new Set(["cffaedfe", "feedfacf"]).has(magic));
  if (!valid) fail(`${target.id} binary has invalid ${target.format} magic ${magic}`);
  return { bytes: bytes.byteLength, sha256: sha256(bytes) };
}

export function assertChildPath(parent, child, role) {
  const root = resolve(parent);
  const candidate = resolve(child);
  const local = relative(root, candidate);
  if (!local || local.startsWith("..") || isAbsolute(local)) {
    fail(`${role} must be a child of ${parent}`);
  }
  return candidate;
}

export function safeFilename(value, role) {
  if (
    typeof value !== "string" ||
    value === "." ||
    value === ".." ||
    basename(value) !== value ||
    !/^[A-Za-z0-9@._-]+$/u.test(value)
  ) {
    fail(`${role} must be a portable filename without path segments`);
  }
  return value;
}

export function digest(value, role) {
  if (typeof value !== "string" || !DIGEST.test(value)) {
    fail(`${role} must be 64 lowercase SHA-256 digits`);
  }
  return value;
}
