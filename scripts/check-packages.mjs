import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "..");
const EXPECTED_ORDER = [
  "pliego-css-ir",
  "pliego-css-cascade",
  "pliego-css-ownership",
  "pliego-css-source",
  "pliego-css-parser",
  "pliego-css-theme",
  "pliego-css-config",
  "pliego-css-compiler",
  "pliego-css-build",
  "pliego-css-macros",
  "pliego-css-agent",
  "pliego-css-usage",
  "pliego-css-control",
  "pliego-css",
  "pliego-cssc",
];
const EXCLUDED_EXAMPLES = [
  "pliego-css-basic-example",
  "pliego-css-custom-theme-example",
];
const REQUIRED_ROOT_FILES = ["CHANGELOG.md", "LICENSE", "README.md"];
const PUBLIC_API_FIXTURE = join(ROOT, "integration-tests", "public-api-smoke");
const PUBLIC_API_FIXTURE_FILES = [
  "Cargo.lock",
  "build.rs",
  "build-toml.rs",
  "pliego.theme.toml",
  "product.resolver.json",
  "src/main.rs",
];
const PACKAGE_TOOLCHAIN = "1.96.0";
const EXPECTED_REPOSITORY = "https://github.com/celiums/pliegocss";
const EXPECTED_MSRV = "1.85";
const MAX_COMPRESSED_ARCHIVE_BYTES = 60 * 1024;
const EXPECTED_LICENSE_SHA256 =
  "8ada45cd9f843acf64e4722ae262c622a2b3b3007c7310ef36ac1061a30f6adb";

function fail(message) {
  throw new Error(message);
}

function run(command, args, { capture = false, cwd = ROOT } = {}) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit",
  });
  if (result.error) {
    fail(`cannot run ${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const details = capture
      ? `\n${result.stdout ?? ""}${result.stderr ?? ""}`.trimEnd()
      : "";
    fail(`${command} ${args.join(" ")} failed with status ${result.status}${details}`);
  }
  if (capture && result.stderr) {
    process.stderr.write(result.stderr);
  }
  return result.stdout ?? "";
}

function runPackageCargo(args, options = {}) {
  return run("cargo", [`+${PACKAGE_TOOLCHAIN}`, ...args], options);
}

function dependencyContract(dependency) {
  return JSON.stringify({
    name: dependency.name,
    rename: dependency.rename ?? null,
    req: dependency.req,
    kind: dependency.kind ?? null,
    target: dependency.target ?? null,
    optional: dependency.optional,
    usesDefaultFeatures: dependency.uses_default_features,
    features: [...dependency.features].sort(),
  });
}

function resolvedDependencyName(dependency) {
  return (dependency.rename ?? dependency.name).replaceAll("-", "_");
}

function assertNormalizedRegistryDependency(manifest, packageName, dependency, version) {
  const tableKind =
    dependency.kind === "build"
      ? "build-dependencies"
      : dependency.kind === "dev"
        ? "dev-dependencies"
        : "dependencies";
  const dependencyKey = dependency.rename ?? dependency.name;
  const header = `[${tableKind}.${dependencyKey}]`;
  const headerStart = manifest.indexOf(`${header}\n`);
  if (headerStart < 0) {
    fail(`${packageName} archive is missing normalized table ${header}`);
  }
  const bodyStart = headerStart + header.length + 1;
  const nextTable = manifest.indexOf("\n[", bodyStart);
  const body = manifest.slice(bodyStart, nextTable < 0 ? manifest.length : nextTable);
  if (!new RegExp(`^version = ${JSON.stringify(`=${version}`)}$`, "mu").test(body)) {
    fail(`${packageName} -> ${dependency.name} archive requirement is not =${version}`);
  }
  if (/^path\s*=/mu.test(body)) {
    fail(`${packageName} -> ${dependency.name} retained a path in the packaged manifest`);
  }
  if (
    dependency.rename !== null &&
    !new RegExp(`^package = ${JSON.stringify(dependency.name)}$`, "mu").test(body)
  ) {
    fail(`${packageName} -> ${dependency.name} archive lost its dependency rename`);
  }
}

function safelyRemoveTemporaryTree(path) {
  const temporaryRoot = resolve(tmpdir());
  const candidate = resolve(path);
  const childPath = relative(temporaryRoot, candidate);
  if (
    !childPath ||
    childPath.startsWith("..") ||
    isAbsolute(childPath) ||
    !basename(candidate).startsWith("pliego-package-verify-")
  ) {
    fail(`refusing to remove unsafe temporary path: ${candidate}`);
  }
  rmSync(candidate, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
}

function canonical(path) {
  const physical = realpathSync.native(path);
  return process.platform === "win32" ? physical.toLocaleLowerCase("en-US") : physical;
}

function resolveMetadataPath(manifestPath, metadataPath) {
  return resolve(dirname(manifestPath), metadataPath);
}

function isPublishable(pkg) {
  return pkg.publish === null || pkg.publish.length > 0;
}

function portableArchiveKey(file, packageName) {
  if (
    !file ||
    isAbsolute(file) ||
    file.includes("\\") ||
    file.startsWith("/") ||
    /^[a-z]:/iu.test(file) ||
    file !== file.normalize("NFC")
  ) {
    fail(`${packageName} package contains a non-portable path: ${JSON.stringify(file)}`);
  }
  const segments = file.split("/");
  for (const segment of segments) {
    const stem = segment.split(".", 1)[0].toLocaleLowerCase("en-US");
    if (
      !segment ||
      segment === "." ||
      segment === ".." ||
      segment.endsWith(".") ||
      segment.endsWith(" ") ||
      segment.includes(":") ||
      /[<>"|?*]/u.test(segment) ||
      /[\u0000-\u001f\u007f]/u.test(segment) ||
      /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])$/u.test(stem)
    ) {
      fail(`${packageName} package contains an unsafe path segment: ${JSON.stringify(file)}`);
    }
  }
  return segments.map((segment) => segment.toLocaleLowerCase("en-US")).join("/");
}

const options = new Set(process.argv.slice(2));
const allowDirty = options.delete("--allow-dirty");
if (options.size > 0) {
  fail(`unknown option(s): ${[...options].join(", ")}`);
}

for (const file of REQUIRED_ROOT_FILES) {
  if (!existsSync(join(ROOT, file))) {
    fail(`required release file is missing: ${file}`);
  }
}
const licenseSha256 = createHash("sha256").update(readFileSync(join(ROOT, "LICENSE"))).digest("hex");
if (licenseSha256 !== EXPECTED_LICENSE_SHA256) {
  fail(`LICENSE is not the frozen Apache-2.0 text; found SHA-256 ${licenseSha256}`);
}

const cargoVersion = runPackageCargo(["--version"], { capture: true }).trim();
const cargoMatch = /^cargo (\d+)\.(\d+)\./u.exec(cargoVersion);
if (!cargoMatch) {
  fail(`cannot parse Cargo version: ${cargoVersion}`);
}
const cargoMajor = Number(cargoMatch[1]);
const cargoMinor = Number(cargoMatch[2]);
if (cargoMajor !== 1 || cargoMinor !== 96) {
  fail(`package workspace verification requires the pinned Cargo 1.96 toolchain; found ${cargoVersion}`);
}

const dirty = run("git", ["status", "--porcelain=v1"], { capture: true }).trim();
if (dirty && !allowDirty) {
  fail("package verification requires a clean worktree; commit changes or pass --allow-dirty");
}
const trackedFiles = run("git", ["ls-files", "-z"], { capture: true })
  .split("\0")
  .filter(Boolean);
const portableTrackedFiles = trackedFiles.map((file) => portableArchiveKey(file, "repository"));
if (new Set(portableTrackedFiles).size !== portableTrackedFiles.length) {
  fail("tracked repository paths collide on a portable filesystem");
}

const metadata = JSON.parse(
  runPackageCargo(["metadata", "--no-deps", "--format-version", "1", "--locked"], {
    capture: true,
  }),
);
if (canonical(metadata.workspace_root) !== canonical(ROOT)) {
  fail(`unexpected workspace root: ${metadata.workspace_root}`);
}

const workspacePackages = new Map(metadata.packages.map((pkg) => [pkg.name, pkg]));
const publishable = metadata.packages.filter(isPublishable);
const actualNames = publishable.map((pkg) => pkg.name).sort();
const expectedNames = [...EXPECTED_ORDER].sort();
if (JSON.stringify(actualNames) !== JSON.stringify(expectedNames)) {
  fail(
    `publishable package boundary changed; expected ${expectedNames.join(", ")}, found ${actualNames.join(", ")}`,
  );
}

const versions = new Set(publishable.map((pkg) => pkg.version));
if (versions.size !== 1) {
  fail(`publishable packages must share one version; found ${[...versions].join(", ")}`);
}
const [version] = versions;
const orderIndex = new Map(EXPECTED_ORDER.map((name, index) => [name, index]));
const rootLicense = canonical(join(ROOT, "LICENSE"));

for (const name of EXPECTED_ORDER) {
  const pkg = workspacePackages.get(name);
  if (!pkg) {
    fail(`package missing from Cargo metadata: ${name}`);
  }
  if (!pkg.description?.trim()) {
    fail(`${name} is missing a package description`);
  }
  if (pkg.repository !== EXPECTED_REPOSITORY) {
    fail(`${name} repository must be ${EXPECTED_REPOSITORY}`);
  }
  if (pkg.rust_version !== EXPECTED_MSRV) {
    fail(`${name} rust-version must be ${EXPECTED_MSRV}`);
  }
  if (pkg.license !== null) {
    fail(`${name} must use the packaged Apache license file instead of duplicate license metadata`);
  }
  if (
    !pkg.license_file ||
    canonical(resolveMetadataPath(pkg.manifest_path, pkg.license_file)) !== rootLicense
  ) {
    fail(`${name} must inherit the workspace LICENSE file`);
  }
  if (!pkg.readme || !existsSync(resolveMetadataPath(pkg.manifest_path, pkg.readme))) {
    fail(`${name} must reference an existing README`);
  }
  if (pkg.keywords.length === 0 || pkg.keywords.length > 5) {
    fail(`${name} must expose one through five registry keywords`);
  }
  if (pkg.categories.length === 0) {
    fail(`${name} must expose at least one registry category`);
  }

  const listArgs = ["package", "--list", "-p", name, "--locked"];
  if (allowDirty) {
    listArgs.push("--allow-dirty");
  }
  const files = runPackageCargo(listArgs, { capture: true })
    .split(/\r?\n/u)
    .filter(Boolean);
  for (const required of ["Cargo.toml", "Cargo.toml.orig", "LICENSE", "README.md"]) {
    if (!files.includes(required)) {
      fail(`${name} package is missing ${required}`);
    }
  }
  if (!files.some((file) => file.startsWith("src/"))) {
    fail(`${name} package contains no source files`);
  }
  const portablePaths = files.map((file) => portableArchiveKey(file, name));
  if (new Set(portablePaths).size !== portablePaths.length) {
    fail(`${name} package list contains paths that collide on a portable filesystem`);
  }
}

const publishableNames = new Set(EXPECTED_ORDER);
for (const pkg of metadata.packages) {
  for (const dependency of pkg.dependencies) {
    const internal = publishableNames.has(dependency.name);
    const hasPath = typeof dependency.path === "string";
    if (!internal) {
      if (hasPath) {
        fail(`${pkg.name} has an external path dependency: ${dependency.name}`);
      }
      continue;
    }
    if (!hasPath) {
      fail(`${pkg.name} -> ${dependency.name} must retain its local workspace path`);
    }
    const target = workspacePackages.get(dependency.name);
    if (!target || !isPublishable(target)) {
      fail(`${pkg.name} has an unknown internal dependency: ${dependency.name}`);
    }
    const requirement = `=${target.version}`;
    if (dependency.req !== requirement) {
      fail(`${pkg.name} -> ${dependency.name} must use exact requirement ${requirement}`);
    }
    if (isPublishable(pkg) && orderIndex.get(dependency.name) >= orderIndex.get(pkg.name)) {
      fail(`${dependency.name} must precede ${pkg.name} in the publication order`);
    }
    const targetRoot = canonical(dirname(target.manifest_path));
    if (canonical(dependency.path) !== targetRoot) {
      fail(`${pkg.name} dependency path does not resolve to ${dependency.name}`);
    }
  }
}

const packageStarted = Date.now();
const packageArgs = ["package", "--workspace"];
for (const name of EXCLUDED_EXAMPLES) {
  packageArgs.push("--exclude", name);
}
packageArgs.push("--locked", "--no-verify");
if (allowDirty) {
  packageArgs.push("--allow-dirty");
}
runPackageCargo(packageArgs);

const archiveRecords = EXPECTED_ORDER.map((name) => {
  const archive = join(metadata.target_directory, "package", `${name}-${version}.crate`);
  if (!existsSync(archive)) {
    fail(`Cargo did not create ${relative(ROOT, archive)}`);
  }
  const info = statSync(archive);
  if (info.size === 0 || info.mtimeMs < packageStarted - 2_000) {
    fail(`package archive is empty or stale: ${relative(ROOT, archive)}`);
  }
  if (info.size > MAX_COMPRESSED_ARCHIVE_BYTES) {
    fail(
      `${name} archive is ${info.size} bytes; maximum compressed archive size is ` +
        `${MAX_COMPRESSED_ARCHIVE_BYTES} bytes (60 KiB)`,
    );
  }
  return {
    name,
    path: archive,
    bytes: info.size,
    sha256: createHash("sha256").update(readFileSync(archive)).digest("hex"),
  };
});

const expectedInternalContracts = new Map(
  EXPECTED_ORDER.map((name) => [
    name,
    workspacePackages
      .get(name)
      .dependencies.filter((dependency) => publishableNames.has(dependency.name))
      .map(dependencyContract)
      .sort(),
  ]),
);
let downstreamEvidence;
const verificationRoot = mkdtempSync(join(tmpdir(), "pliego-package-verify-"));
try {
  const packagesRoot = join(verificationRoot, "packages");
  mkdirSync(packagesRoot);
  const extractedRoots = new Map();
  for (const archive of archiveRecords) {
    run("tar", ["-xzf", archive.path, "-C", packagesRoot], { cwd: verificationRoot });
    const packageRoot = join(packagesRoot, `${archive.name}-${version}`);
    if (!existsSync(join(packageRoot, "Cargo.toml"))) {
      fail(`${archive.name} archive did not extract to its canonical package root`);
    }
    extractedRoots.set(archive.name, packageRoot);
  }

  const memberPaths = EXPECTED_ORDER.map((name) =>
    relative(verificationRoot, extractedRoots.get(name)).replaceAll("\\", "/"),
  );
  const verificationManifest = [
    "[workspace]",
    'resolver = "2"',
    "members = [",
    ...memberPaths.map((path) => `  ${JSON.stringify(path)},`),
    "]",
    "",
    "[patch.crates-io]",
    ...EXPECTED_ORDER.map(
      (name) =>
        `${JSON.stringify(name)} = { path = ${JSON.stringify(
          relative(verificationRoot, extractedRoots.get(name)).replaceAll("\\", "/"),
        )} }`,
    ),
    "",
  ].join("\n");
  writeFileSync(join(verificationRoot, "Cargo.toml"), verificationManifest, "utf8");
  copyFileSync(join(ROOT, "Cargo.lock"), join(verificationRoot, "Cargo.lock"));

  const packagedMetadata = JSON.parse(
    runPackageCargo(["metadata", "--format-version", "1", "--all-features", "--locked"], {
      cwd: verificationRoot,
      capture: true,
    }),
  );
  if (canonical(packagedMetadata.workspace_root) !== canonical(verificationRoot)) {
    fail(`archive verification escaped its temporary workspace: ${packagedMetadata.workspace_root}`);
  }

  const packagedById = new Map(packagedMetadata.packages.map((pkg) => [pkg.id, pkg]));
  const internalPackages = packagedMetadata.packages.filter((pkg) =>
    publishableNames.has(pkg.name),
  );
  if (internalPackages.length !== EXPECTED_ORDER.length) {
    fail(
      `archive graph resolved ${internalPackages.length} internal package copies; expected ${EXPECTED_ORDER.length}`,
    );
  }
  const internalByName = new Map();
  for (const pkg of internalPackages) {
    if (internalByName.has(pkg.name)) {
      fail(`archive graph resolved more than one copy of ${pkg.name}`);
    }
    internalByName.set(pkg.name, pkg);
  }

  const workspaceMemberIds = new Set(packagedMetadata.workspace_members);
  const actualWorkspaceNames = [...workspaceMemberIds]
    .map((id) => packagedById.get(id)?.name)
    .sort();
  if (JSON.stringify(actualWorkspaceNames) !== JSON.stringify(expectedNames)) {
    fail(
      `archive workspace boundary changed; expected ${expectedNames.join(", ")}, found ${actualWorkspaceNames.join(", ")}`,
    );
  }

  for (const name of EXPECTED_ORDER) {
    const pkg = internalByName.get(name);
    if (!pkg || pkg.version !== version || pkg.source !== null) {
      fail(`${name} did not resolve to the extracted ${version} archive`);
    }
    if (canonical(dirname(pkg.manifest_path)) !== canonical(extractedRoots.get(name))) {
      fail(`${name} resolved outside its extracted archive`);
    }
    const internalDependencies = pkg.dependencies.filter((dependency) =>
      publishableNames.has(dependency.name),
    );
    const packagedManifest = readFileSync(join(extractedRoots.get(name), "Cargo.toml"), "utf8");
    for (const dependency of workspacePackages
      .get(name)
      .dependencies.filter((item) => publishableNames.has(item.name))) {
      assertNormalizedRegistryDependency(packagedManifest, name, dependency, version);
    }
    const actualContracts = internalDependencies.map(dependencyContract).sort();
    const expectedContracts = expectedInternalContracts.get(name);
    if (JSON.stringify(actualContracts) !== JSON.stringify(expectedContracts)) {
      fail(`${name} packaged internal dependency contract differs from the source manifest`);
    }
  }

  const internalNamesById = new Map(
    internalPackages.map((pkg) => [pkg.id, pkg.name]),
  );
  const resolutionNodes = new Map(
    packagedMetadata.resolve.nodes.map((node) => [node.id, node]),
  );
  for (const name of EXPECTED_ORDER) {
    const pkg = internalByName.get(name);
    const node = resolutionNodes.get(pkg.id);
    if (!node) {
      fail(`archive graph has no resolution node for ${name}`);
    }
    const actualEdges = node.deps
      .filter((dependency) => internalNamesById.has(dependency.pkg))
      .map((dependency) => ({
        name: dependency.name,
        package: internalNamesById.get(dependency.pkg),
        kinds: dependency.dep_kinds
          .map((kind) => ({ kind: kind.kind ?? null, target: kind.target ?? null }))
          .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right), "en")),
      }))
      .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right), "en"));
    const expectedEdges = workspacePackages
      .get(name)
      .dependencies.filter((dependency) => publishableNames.has(dependency.name))
      .map((dependency) => ({
        name: resolvedDependencyName(dependency),
        package: dependency.name,
        kinds: [{ kind: dependency.kind ?? null, target: dependency.target ?? null }],
      }))
      .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right), "en"));
    if (JSON.stringify(actualEdges) !== JSON.stringify(expectedEdges)) {
      fail(`${name} resolved internal archive graph differs from the source workspace`);
    }
  }

  runPackageCargo(
    ["check", "--release", "--workspace", "--all-targets", "--all-features", "--locked"],
    { cwd: verificationRoot },
  );

  const downstreamRoot = join(verificationRoot, "downstream");
  const downstreamSource = join(downstreamRoot, "src");
  mkdirSync(downstreamSource, { recursive: true });
  for (const fixtureFile of PUBLIC_API_FIXTURE_FILES) {
    const source = join(PUBLIC_API_FIXTURE, fixtureFile);
    if (!existsSync(source) || !statSync(source).isFile()) {
      fail(`public API fixture is missing ${fixtureFile}`);
    }
    copyFileSync(source, join(downstreamRoot, fixtureFile));
  }

  const downstreamRoots = [
    "pliego-css",
    "pliego-css-agent",
    "pliego-css-build",
    "pliego-css-ownership",
    "pliego-css-source",
    "pliego-css-usage",
  ];
  const downstreamInternalNames = new Set(downstreamRoots);
  const pendingInternalNames = [...downstreamRoots];
  while (pendingInternalNames.length > 0) {
    const name = pendingInternalNames.pop();
    for (const dependency of workspacePackages
      .get(name)
      .dependencies.filter((item) => publishableNames.has(item.name) && !item.optional)) {
      if (!downstreamInternalNames.has(dependency.name)) {
        downstreamInternalNames.add(dependency.name);
        pendingInternalNames.push(dependency.name);
      }
    }
  }
  const expectedDownstreamNames = [...downstreamInternalNames].sort();
  const downstreamManifest = [
    "[package]",
    'name = "pliego-css-public-api-smoke"',
    'version = "0.0.0"',
    'edition = "2024"',
    `rust-version = ${JSON.stringify(EXPECTED_MSRV)}`,
    "publish = false",
    "",
    "[dependencies]",
    `css = { package = "pliego-css", version = ${JSON.stringify(`=${version}`)} }`,
    `pliego-css-agent = ${JSON.stringify(`=${version}`)}`,
    `pliego-css-ownership = ${JSON.stringify(`=${version}`)}`,
    `pliego-css-source = ${JSON.stringify(`=${version}`)}`,
    `pliego-css-usage = ${JSON.stringify(`=${version}`)}`,
    "",
    "[build-dependencies]",
    `pliego-css-build = ${JSON.stringify(`=${version}`)}`,
    "",
    "[patch.crates-io]",
    ...expectedDownstreamNames.map(
      (name) =>
        `${JSON.stringify(name)} = { path = ${JSON.stringify(
          relative(downstreamRoot, extractedRoots.get(name)).replaceAll("\\", "/"),
        )} }`,
    ),
    "",
    "[workspace]",
    'resolver = "2"',
    "",
  ].join("\n");
  writeFileSync(join(downstreamRoot, "Cargo.toml"), downstreamManifest, "utf8");

  const downstreamMetadata = JSON.parse(
    run(
      "cargo",
      ["+1.85.0", "metadata", "--format-version", "1", "--all-features", "--locked"],
      { cwd: downstreamRoot, capture: true },
    ),
  );
  if (canonical(downstreamMetadata.workspace_root) !== canonical(downstreamRoot)) {
    fail(
      `downstream verification escaped its temporary workspace: ${downstreamMetadata.workspace_root}`,
    );
  }
  const downstreamConsumer = downstreamMetadata.packages.find(
    (pkg) => pkg.name === "pliego-css-public-api-smoke",
  );
  if (
    !downstreamConsumer ||
    downstreamMetadata.workspace_members.length !== 1 ||
    downstreamMetadata.workspace_members[0] !== downstreamConsumer.id
  ) {
    fail("downstream workspace must contain only the package consumer");
  }

  const downstreamInternalPackages = downstreamMetadata.packages.filter((pkg) =>
    publishableNames.has(pkg.name),
  );
  const actualDownstreamNames = downstreamInternalPackages.map((pkg) => pkg.name).sort();
  if (JSON.stringify(actualDownstreamNames) !== JSON.stringify(expectedDownstreamNames)) {
    fail(
      `downstream archive closure changed; expected ${expectedDownstreamNames.join(", ")}, found ${actualDownstreamNames.join(", ")}`,
    );
  }
  const downstreamInternalByName = new Map();
  for (const pkg of downstreamInternalPackages) {
    if (downstreamInternalByName.has(pkg.name)) {
      fail(`downstream graph resolved more than one copy of ${pkg.name}`);
    }
    downstreamInternalByName.set(pkg.name, pkg);
    if (pkg.version !== version || pkg.source !== null) {
      fail(`${pkg.name} did not resolve to the extracted downstream ${version} archive`);
    }
    if (canonical(dirname(pkg.manifest_path)) !== canonical(extractedRoots.get(pkg.name))) {
      fail(`${pkg.name} downstream resolution escaped its extracted archive`);
    }
  }

  const downstreamInternalNamesById = new Map(
    downstreamInternalPackages.map((pkg) => [pkg.id, pkg.name]),
  );
  const downstreamResolutionNodes = new Map(
    downstreamMetadata.resolve.nodes.map((node) => [node.id, node]),
  );
  for (const name of expectedDownstreamNames) {
    const pkg = downstreamInternalByName.get(name);
    const node = downstreamResolutionNodes.get(pkg.id);
    if (!node) {
      fail(`downstream graph has no resolution node for ${name}`);
    }
    const actualEdges = node.deps
      .filter((dependency) => downstreamInternalNamesById.has(dependency.pkg))
      .map((dependency) => ({
        name: dependency.name,
        package: downstreamInternalNamesById.get(dependency.pkg),
        kinds: dependency.dep_kinds
          .map((kind) => ({ kind: kind.kind ?? null, target: kind.target ?? null }))
          .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right), "en")),
      }))
      .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right), "en"));
    const declaredEdges = workspacePackages
      .get(name)
      .dependencies.filter((dependency) => downstreamInternalNames.has(dependency.name))
      .map((dependency) => ({
        name: resolvedDependencyName(dependency),
        package: dependency.name,
        kinds: [{ kind: dependency.kind ?? null, target: dependency.target ?? null }],
        optional: dependency.optional,
      }))
      .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right), "en"));
    const edgeContract = (edge) => ({
      name: edge.name,
      package: edge.package,
      kinds: edge.kinds,
    });
    const declaredContracts = declaredEdges.map(edgeContract);
    const requiredContracts = declaredEdges.filter((edge) => !edge.optional).map(edgeContract);
    const hasEdge = (edges, expected) =>
      edges.some((edge) => JSON.stringify(edge) === JSON.stringify(expected));
    if (
      actualEdges.some((edge) => !hasEdge(declaredContracts, edge)) ||
      requiredContracts.some((edge) => !hasEdge(actualEdges, edge))
    ) {
      fail(`${name} downstream archive graph differs from the normalized package graph`);
    }
  }

  const consumerNode = downstreamResolutionNodes.get(downstreamConsumer.id);
  if (!consumerNode) {
    fail("downstream graph has no resolution node for the package consumer");
  }
  const actualConsumerEdges = consumerNode.deps
    .filter((dependency) => downstreamInternalNamesById.has(dependency.pkg))
    .map((dependency) => ({
      name: dependency.name,
      package: downstreamInternalNamesById.get(dependency.pkg),
      kinds: dependency.dep_kinds
        .map((kind) => ({ kind: kind.kind ?? null, target: kind.target ?? null }))
        .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right), "en")),
    }))
    .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right), "en"));
  const expectedConsumerEdges = [
    {
      name: "css",
      package: "pliego-css",
      kinds: [{ kind: null, target: null }],
    },
    {
      name: "pliego_css_agent",
      package: "pliego-css-agent",
      kinds: [{ kind: null, target: null }],
    },
    {
      name: "pliego_css_build",
      package: "pliego-css-build",
      kinds: [{ kind: "build", target: null }],
    },
    {
      name: "pliego_css_ownership",
      package: "pliego-css-ownership",
      kinds: [{ kind: null, target: null }],
    },
    {
      name: "pliego_css_source",
      package: "pliego-css-source",
      kinds: [{ kind: null, target: null }],
    },
    {
      name: "pliego_css_usage",
      package: "pliego-css-usage",
      kinds: [{ kind: null, target: null }],
    },
  ].sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right), "en"));
  if (JSON.stringify(actualConsumerEdges) !== JSON.stringify(expectedConsumerEdges)) {
    fail("downstream consumer did not resolve its exact normal/build package boundary");
  }

  run(
    "cargo",
    ["+1.85.0", "check", "--release", "--all-targets", "--all-features", "--locked"],
    { cwd: downstreamRoot },
  );
  const dtcgDownstreamRun = run(
    "cargo",
    ["+1.85.0", "run", "--release", "--quiet", "--locked"],
    { cwd: downstreamRoot, capture: true },
  ).trim();
  if (!/^[0-9a-f]{32}\tpc_[0-9a-z]+$/u.test(dtcgDownstreamRun)) {
    fail(
      `downstream DTCG public API fixture produced an unexpected line: ${JSON.stringify(dtcgDownstreamRun)}`,
    );
  }

  copyFileSync(join(downstreamRoot, "build-toml.rs"), join(downstreamRoot, "build.rs"));
  const tomlDownstreamRun = run(
    "cargo",
    ["+1.85.0", "run", "--release", "--quiet", "--locked"],
    { cwd: downstreamRoot, capture: true },
  ).trim();
  if (!/^[0-9a-f]{32}\tpc_[0-9a-z]+$/u.test(tomlDownstreamRun)) {
    fail(
      `downstream TOML public API fixture produced an unexpected line: ${JSON.stringify(tomlDownstreamRun)}`,
    );
  }
  if (tomlDownstreamRun !== dtcgDownstreamRun) {
    fail("downstream TOML and DTCG build macros did not converge on the same selected registry");
  }
  downstreamEvidence = {
    rust: "1.85",
    fixture: "integration-tests/public-api-smoke",
    facadeDependency: "css (package pliego-css)",
    packages: expectedDownstreamNames,
    output: dtcgDownstreamRun,
    dtcgOutput: dtcgDownstreamRun,
    tomlOutput: tomlDownstreamRun,
  };
} finally {
  safelyRemoveTemporaryTree(verificationRoot);
}

const archives = archiveRecords.map(({ name, bytes, sha256 }) => ({ name, bytes, sha256 }));

process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 1,
      cargo: cargoVersion,
      version,
      publicationOrder: EXPECTED_ORDER,
      maxCompressedArchiveBytes: MAX_COMPRESSED_ARCHIVE_BYTES,
      archives,
      downstream: downstreamEvidence,
      worktree: dirty ? "dirty-explicitly-allowed" : "clean",
      published: false,
    },
    null,
    2,
  )}\n`,
);
