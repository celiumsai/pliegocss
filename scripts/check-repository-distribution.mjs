import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, isAbsolute, relative, resolve } from "node:path";
import {
  ROOT,
  fail,
  repositoryVersion,
  safeFilename,
  validatePackageManifest,
  validatePolicy,
} from "./repository-distribution.mjs";
import { resolveBinary, selectTarget } from "../packages/cli/bin/launcher.mjs";

function run(script, args) {
  const result = spawnSync(process.execPath, [resolve(ROOT, "scripts", script), ...args], {
    cwd: ROOT,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    fail(`${script} failed: ${result.error?.message ?? `${result.stdout}${result.stderr}`.trim()}`);
  }
  return result.stdout.trim();
}

const policy = validatePolicy();
validatePackageManifest(undefined, policy);
const workflow = readFileSync(resolve(ROOT, ".github", "workflows", "distribution.yml"), "utf8");
for (const required of [
  "cargo +1.96.0 install cargo-cyclonedx --version '=0.5.9' --locked",
  "pnpm/action-setup@0ebf47130e4866e96fce0953f49152a61190b271",
  "version: 11.7.0",
  "actions/attest@f7c74d28b9d84cb8768d0b8ca14a4bac6ef463e6",
  "merge-multiple: false",
  "repos/${GITHUB_REPOSITORY}/immutable-releases",
  "--draft",
]) {
  assert(workflow.includes(required), `distribution workflow is missing: ${required}`);
}
assert(!/\b(?:npm|pnpm) publish\b/u.test(workflow), "distribution workflow must not publish to npmjs");
assert.throws(() => safeFilename("../escape.tgz", "fixture artifact"), /portable filename/u);
assert.throws(
  () =>
    validatePackageManifest(
      {
        ...structuredClone(validatePackageManifest(undefined, policy)),
        scripts: { postinstall: "node fetch-binary.mjs" },
      },
      policy,
    ),
  /forbidden lifecycle script/u,
);
assert.equal(selectTarget("win32", "x64").id, "x86_64-pc-windows-msvc");
assert.equal(selectTarget("linux", "x64").id, "x86_64-unknown-linux-gnu");
assert.equal(selectTarget("darwin", "arm64").id, "aarch64-apple-darwin");
assert.throws(() => selectTarget("linux", "arm64"), /no repository binary/u);
assert.match(
  resolveBinary("package", "pliego-cssc", "linux", "x64").replaceAll("\\", "/"),
  /\/package\/vendor\/x86_64-unknown-linux-gnu\/pliego-cssc$/u,
);
assert.throws(
  () => resolveBinary("/package", "unknown", "linux", "x64"),
  /unknown PliegoCSS repository binary/u,
);

const temporaryRoot = mkdtempSync(resolve(tmpdir(), "pliegocss-distribution-contract-"));
const fixtureInput = resolve(ROOT, "target", `distribution-contract-${basename(temporaryRoot)}`, "native");
const fixtureOutput = resolve(ROOT, "target", `distribution-contract-${basename(temporaryRoot)}`, "bundle");
const version = repositoryVersion();
const sourceCommit = "1".repeat(40);
const gitTree = "2".repeat(40);
try {
  for (const target of policy.targets) {
    const source = resolve(temporaryRoot, target.id);
    mkdirSync(source, { recursive: true });
    const magic = {
      pe: Buffer.from([0x4d, 0x5a, 0x90, 0x00]),
      elf: Buffer.from([0x7f, 0x45, 0x4c, 0x46]),
      "mach-o": Buffer.from([0xcf, 0xfa, 0xed, 0xfe]),
    }[target.format];
    for (const binary of policy.binaries) {
      const bytes = Buffer.alloc(64 * 1024, 0x5a);
      magic.copy(bytes, 0);
      writeFileSync(resolve(source, `${binary}${target.extension}`), bytes);
      const sbom = {
        bomFormat: "CycloneDX",
        specVersion: "1.5",
        serialNumber: `urn:uuid:00000000-0000-4000-8000-${binary === "pliego-cssc" ? "000000000001" : "000000000002"}`,
        version: 1,
        metadata: {
          component: { type: "application", name: binary, version },
        },
        components: [],
      };
      writeFileSync(resolve(source, `${binary}.cdx.json`), `${JSON.stringify(sbom)}\n`);
    }
    run("prepare-native-distribution.mjs", [
      `--target=${target.id}`,
      `--binary-dir=${relative(ROOT, source)}`,
      `--cli-sbom=${relative(ROOT, resolve(source, "pliego-cssc.cdx.json"))}`,
      `--lsp-sbom=${relative(ROOT, resolve(source, "pliego-css-lsp.cdx.json"))}`,
      `--output=${relative(ROOT, resolve(fixtureInput, `native-${target.id}`))}`,
      `--source-commit=${sourceCommit}`,
      `--git-tree=${gitTree}`,
    ]);
  }
  run("build-repository-distribution.mjs", [
    `--input=${relative(ROOT, fixtureInput)}`,
    `--output=${relative(ROOT, fixtureOutput)}`,
    `--source-commit=${sourceCommit}`,
    `--git-tree=${gitTree}`,
    "--allow-dirty",
  ]);
  const result = JSON.parse(
    run("check-distribution-bundle.mjs", [
      "--",
      `--root=${relative(ROOT, resolve(fixtureOutput, "release-assets"))}`,
    ]),
  );
  assert.equal(result.result, "passed");
  assert.equal(result.targets, 3);
  assert.equal(result.pnpmPackage, `pliegocss-pnpm-${version}.tgz`);
  assert.equal(result.npmjsPublished, false);
  assert.equal(result.lifecycleScripts, false);
} finally {
  const contractRoot = resolve(fixtureInput, "..");
  if (!contractRoot.startsWith(resolve(ROOT, "target"))) {
    fail(`refusing to remove unsafe contract path: ${contractRoot}`);
  }
  rmSync(contractRoot, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
  const local = relative(resolve(tmpdir()), temporaryRoot);
  if (!local || local.startsWith("..") || isAbsolute(local)) {
    fail(`refusing to remove unsafe fixture path: ${temporaryRoot}`);
  }
  rmSync(temporaryRoot, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
}

process.stdout.write(
  `${JSON.stringify({
    schemaVersion: 1,
    result: "passed",
    policy: "repository-only",
    packageManager: "pnpm@11.7.0",
    npmjsPublished: false,
    lifecycleScripts: false,
    targets: policy.targets.map((target) => target.id),
  }, null, 2)}\n`,
);
