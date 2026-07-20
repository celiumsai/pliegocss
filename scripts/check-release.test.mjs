import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const script = resolve(root, "scripts", "check-release.mjs");

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
assert(!fast.gates.some((gate) => gate.id === "getting-started"));

const integration = list("integration");
assert.deepEqual(integration.includes, ["fast", "integration"]);
assert(integration.gates.some((gate) => gate.id === "plain-html"));
assert(integration.gates.some((gate) => gate.id === "getting-started"));
assert(integration.gates.some((gate) => gate.id === "pliegors-browser"));

const release = list("release");
assert.deepEqual(release.includes, ["fast", "integration", "release"]);
assert(release.gates.some((gate) => gate.id === "packages"));
assert(release.gates.some((gate) => gate.id === "benchmark-evidence"));
assert(release.gates.some((gate) => gate.id === "migration-real-corpus"));
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

process.stdout.write("release profile contract: pass\n");
