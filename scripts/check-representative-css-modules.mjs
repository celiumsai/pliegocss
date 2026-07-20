import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const fixture = join(root, "integration-tests", "representative", "css-modules-consumer");
const executable = join(root, "target", "debug", process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc");
function fail(message, detail) { throw new Error(detail === undefined ? message : `${message}\n${JSON.stringify(detail, null, 2)}`); }
function run(command, args, cwd = root) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8", windowsHide: true });
  if (result.error) throw result.error;
  if (result.status !== 0) fail(`command failed: ${command} ${args.join(" ")}`, result);
  return result;
}
function sha256(bytes) { return createHash("sha256").update(bytes).digest("hex"); }
function snapshot() {
  const paths = ["Card.tsx", "styles/Card.module.css", "styles/base.module.css"];
  return Object.fromEntries(paths.map((name) => [name, sha256(readFileSync(join(fixture, name)))]));
}
for (const name of ["Card.tsx", "styles/Card.module.css", "styles/base.module.css"]) if (!existsSync(join(fixture, name))) fail(`missing ${name}`);
run("cargo", ["build", "--quiet", "--locked", "-p", "pliego-cssc"]);
const before = snapshot();
const first = run(executable, ["migration-project-inventory", "."], fixture);
const second = run(executable, ["migration-project-inventory", "."], fixture);
if (first.stdout !== second.stdout || first.stderr !== "" || second.stderr !== "") fail("inventory output drifted");
if (JSON.stringify(before) !== JSON.stringify(snapshot())) fail("inventory mutated CSS Modules application");
const inventory = JSON.parse(first.stdout);
for (const [field, expected] of Object.entries({ sources: 2, consumers: 1, cssModulesSources: 2, consumerImports: 1, staticConsumerUsages: 4, dynamicConsumerUsages: 1, consumerAliases: 1, consumerDestructures: 1 })) {
  if (inventory.summary?.[field] !== expected) fail(`summary.${field} drifted`, { actual: inventory.summary?.[field], expected });
}
const composition = inventory.dependencies?.find((edge) => edge.kind === "css-modules-composes");
if (composition?.resolution !== "resolved" || composition.target !== "styles/base.module.css") fail("CSS Modules composition did not resolve", composition);
const consumer = inventory.consumers?.[0];
const kinds = new Set(consumer?.observations?.map((observation) => observation.kind));
for (const kind of ["import", "binding-alias", "destructured-class", "class-usage"]) if (!kinds.has(kind)) fail(`missing consumer observation ${kind}`);
process.stdout.write(`${JSON.stringify({
  schema: "pliegocss/representative-application/1",
  id: "css-modules-consumer",
  passed: true,
  application: { files: Object.keys(before).length, snapshot: before },
  inventory: { bytes: Buffer.byteLength(first.stdout), sha256: sha256(Buffer.from(first.stdout)), summary: inventory.summary, resolvedComposition: composition },
  evidenceLimits: [
    "This gate proves bounded CSS Modules source/consumer inventory and one relative composition edge.",
    "It does not execute TypeScript, a bundler, or CSS Modules transformation and does not prove exported class existence."
  ]
}, null, 2)}\n`);
