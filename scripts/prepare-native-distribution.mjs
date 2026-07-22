import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import {
  ROOT,
  assertChildPath,
  exactKeys,
  fail,
  fileSha256,
  repositoryVersion,
  validateBinary,
  validateCycloneDx,
  validatePolicy,
  validateSource,
} from "./repository-distribution.mjs";

function options(args) {
  const parsed = {};
  for (const arg of args) {
    const match = /^--([a-z-]+)=(.+)$/u.exec(arg);
    if (!match || Object.hasOwn(parsed, match[1])) fail(`invalid option: ${arg}`);
    parsed[match[1]] = match[2];
  }
  return parsed;
}

function git(args) {
  const result = spawnSync("git", args, {
    cwd: ROOT,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    fail(`git ${args.join(" ")} failed: ${result.error?.message ?? result.stderr.trim()}`);
  }
  return result.stdout.trim();
}

const input = options(process.argv.slice(2));
const required = ["target", "binary-dir", "cli-sbom", "lsp-sbom", "output"];
for (const name of required) {
  if (!input[name]) fail(`missing --${name}`);
}
const policy = validatePolicy();
const target = policy.targets.find((entry) => entry.id === input.target);
if (!target) fail(`unknown native target: ${input.target}`);
const version = repositoryVersion();
const source = {
  commit: input["source-commit"] ?? git(["rev-parse", "HEAD"]),
  gitTree: input["git-tree"] ?? git(["show", "-s", "--format=%T", "HEAD"]),
  tag: input.tag ?? null,
};
validateSource(source);
if (source.tag !== null && source.tag !== `v${version}`) {
  fail(`source tag ${source.tag} does not match package version ${version}`);
}

const binaryDirectory = resolve(ROOT, input["binary-dir"]);
const output = assertChildPath(
  resolve(ROOT, "target"),
  resolve(ROOT, input.output),
  "native output",
);
rmSync(output, { recursive: true, force: true });
mkdirSync(output, { recursive: true });

const binaries = [];
for (const binaryName of policy.binaries) {
  const filename = `${binaryName}${target.extension}`;
  const sourcePath = resolve(binaryDirectory, filename);
  const details = validateBinary(sourcePath, target);
  const destination = resolve(output, filename);
  copyFileSync(sourcePath, destination);
  if (target.os !== "win32") chmodSync(destination, 0o755);
  binaries.push({ name: binaryName, file: filename, ...details });
}

const sbomInputs = {
  "pliego-cssc": resolve(ROOT, input["cli-sbom"]),
  "pliego-css-lsp": resolve(ROOT, input["lsp-sbom"]),
};
const sboms = [];
for (const binaryName of policy.binaries) {
  const sourcePath = sbomInputs[binaryName];
  if (!existsSync(sourcePath)) fail(`SBOM is missing: ${sourcePath}`);
  validateCycloneDx(sourcePath, binaryName, version);
  const filename = `${binaryName}.cdx.json`;
  const destination = resolve(output, filename);
  copyFileSync(sourcePath, destination);
  sboms.push({ binary: binaryName, file: filename, sha256: fileSha256(destination) });
}

copyFileSync(resolve(ROOT, "LICENSE"), resolve(output, "LICENSE"));
const receipt = {
  schemaVersion: 1,
  kind: "pliegocss-native-distribution-input",
  version,
  source,
  target: {
    id: target.id,
    os: target.os,
    arch: target.arch,
    format: target.format,
  },
  binaries,
  sboms,
};
exactKeys(
  receipt,
  ["schemaVersion", "kind", "version", "source", "target", "binaries", "sboms"],
  "native receipt",
);
writeFileSync(resolve(output, "native.json"), `${JSON.stringify(receipt, null, 2)}\n`);
process.stdout.write(
  `${JSON.stringify({ target: target.id, output, files: binaries.length + sboms.length + 2 })}\n`,
);
