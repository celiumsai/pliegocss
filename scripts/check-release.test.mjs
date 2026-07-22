import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { targetDirectoryForRustcIdentity } from "./rust-target.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const script = resolve(root, "scripts", "check-release.mjs");
const source = readFileSync(script, "utf8");
const pliegorsDevLoopSource = readFileSync(
  resolve(root, "scripts", "check-pliegors-dev-loop.mjs"),
  "utf8",
);
const pliegorsDevLoopManifest = readFileSync(
  resolve(root, "integration-tests", "pliegors-dev-loop", "Cargo.toml"),
  "utf8",
);

function list(profile) {
  const result = spawnSync(process.execPath, [script, "--list", profile], {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
  });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

const fast = list("fast");
assert.equal(fast.schemaVersion, 1);
assert.equal(fast.profile, "fast");
assert.deepEqual(fast.includes, ["fast"]);
assert(fast.gates.some((gate) => gate.id === "rust-clippy"));
assert(fast.gates.some((gate) => gate.id === "standards-provenance"));
assert(fast.gates.some((gate) => gate.id === "brand-system"));
assert(fast.gates.some((gate) => gate.id === "release-authority-contract-tests"));
assert(fast.gates.some((gate) => gate.id === "release-readiness-contract"));
assert(fast.gates.some((gate) => gate.id === "hosted-browser-matrix-contract"));
assert(fast.gates.some((gate) => gate.id === "document-authority"));
assert(fast.gates.some((gate) => gate.id === "site-markdown-docs"));
assert(fast.gates.some((gate) => gate.id === "benchmark-authority-v2"));
assert(fast.gates.some((gate) => gate.id === "browser-output-authority"));
assert(!fast.gates.some((gate) => gate.id === "release-readiness"));
assert(!fast.gates.some((gate) => gate.id === "getting-started"));

const integration = list("integration");
assert.deepEqual(integration.includes, ["fast", "integration"]);
assert(integration.gates.some((gate) => gate.id === "plain-html"));
assert(integration.gates.some((gate) => gate.id === "getting-started"));
assert(integration.gates.some((gate) => gate.id === "pliegors-browser"));
assert(integration.gates.some((gate) => gate.id === "benchmark-authority-smoke"));

const release = list("release");
assert.deepEqual(release.includes, ["fast", "integration", "release"]);
assert(release.gates.some((gate) => gate.id === "packages"));
assert(release.gates.some((gate) => gate.id === "benchmark-evidence"));
assert(release.gates.some((gate) => gate.id === "media-query-merge"));
assert(release.gates.some((gate) => gate.id === "reachability-pruning"));
assert(release.gates.some((gate) => gate.id === "supply-chain"));
assert(release.gates.some((gate) => gate.id === "site"));
assert(release.gates.some((gate) => gate.id === "site-deployment"));
assert(release.gates.some((gate) => gate.id === "migration-real-corpus"));
assert(release.gates.some((gate) => gate.id === "benchmark-oracle-live"));
assert.deepEqual(
  release.gates.find((gate) => gate.id === "release-readiness")?.command,
  ["node", "scripts/check-release-readiness.mjs", "--profile=release"],
);
assert.deepEqual(
  release.gates.find((gate) => gate.id === "hosted-browser-matrix")?.command,
  ["node", "scripts/check-hosted-browser-matrix.mjs", "--profile=release"],
);
assert.equal(
  new Set(release.gates.map((gate) => gate.id)).size,
  release.gates.length,
);

const invalid = spawnSync(process.execPath, [script, "--list", "unknown"], {
  cwd: root,
  encoding: "utf8",
  windowsHide: true,
});
assert.notEqual(invalid.status, 0);
assert.match(invalid.stderr, /unknown verification profile/u);
assert.match(
  source,
  /process\.platform === "win32" && gate\.command === "pnpm"/u,
  "Windows must invoke pnpm command shims through cmd.exe",
);
const targetBase = resolve(root, "target", "isolation-contract");
const targetEnvironment = { CARGO_TARGET_DIR: targetBase };
const stableTarget = targetDirectoryForRustcIdentity(
  root,
  targetEnvironment,
  "release: 1.85.0\nhost: x86_64-pc-windows-msvc\ncommit-hash: aaaa",
);
assert.equal(
  stableTarget,
  targetDirectoryForRustcIdentity(
    root,
    targetEnvironment,
    "release: 1.85.0\nhost: x86_64-pc-windows-msvc\ncommit-hash: aaaa",
  ),
  "one exact toolchain identity must map to one deterministic target",
);
assert.notEqual(
  stableTarget,
  targetDirectoryForRustcIdentity(
    root,
    targetEnvironment,
    "release: 1.96.0\nhost: x86_64-pc-windows-msvc\ncommit-hash: bbbb",
  ),
  "different Rust toolchains must never share build artifacts",
);
assert.match(stableTarget, /toolchains/u);
assert.equal(
  release.gates.find((gate) => gate.id === "properties")?.rustToolchain,
  "1.85.0",
  "the MSRV property gate must select its own target namespace",
);
for (const dependency of ["pliego-dom", "pliego-ssg"]) {
  assert.match(
    pliegorsDevLoopManifest,
    new RegExp(`^${dependency} = "=0\\.0\\.2"$`, "mu"),
    `${dependency} must exercise the exact published PliegoRS dependency`,
  );
}
assert.match(
  pliegorsDevLoopSource,
  /join\(PLIEGORS_ROOT, "crates", crate\)/u,
  "the development-loop gate must source its Cargo override from PLIEGORS_ROOT",
);
assert.match(
  pliegorsDevLoopSource,
  /join\(cargoOverrideDirectory, "config\.toml"\)/u,
  "the development-loop gate must materialize a local Cargo path override",
);
assert.doesNotMatch(
  pliegorsDevLoopSource,
  /\.\.\/\.\.\/\.\.\/pliegors\/crates/u,
  "the development-loop gate must not require a stale relative PliegoRS checkout",
);

process.stdout.write("release profile contract: pass\n");
