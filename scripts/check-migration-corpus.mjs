import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { cargoTargetRoot, isolatedCargoEnvironment } from "./rust-target.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const corpusRoot = join(root, "integration-tests", "migration-corpus");
const manifest = JSON.parse(readFileSync(join(corpusRoot, "cases.json"), "utf8"));
const cargoEnvironment = isolatedCargoEnvironment(root);
const cargoTarget = cargoTargetRoot(root, cargoEnvironment);

function fail(message) {
  throw new Error(message);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: "utf8", ...options });
  if (result.error) fail(`cannot run ${command}: ${result.error.message}`);
  if (result.status !== 0) {
    fail(`${command} ${args.join(" ")} failed (${result.status}):\n${result.stderr}`);
  }
  return result;
}

function filesUnder(directory) {
  const output = [];
  for (const name of readdirSync(directory).sort()) {
    const path = join(directory, name);
    if (statSync(path).isDirectory()) output.push(...filesUnder(path));
    else output.push(path);
  }
  return output;
}

function snapshot(directory) {
  return Object.fromEntries(
    filesUnder(directory).map((path) => [
      relative(directory, path).replaceAll("\\", "/"),
      createHash("sha256").update(readFileSync(path)).digest("hex"),
    ]),
  );
}

if (manifest.schemaVersion !== 1 || manifest.classification !== "authored-contract") {
  fail("migration corpus manifest must be schema 1 and authored-contract");
}
if (!Array.isArray(manifest.cases) || manifest.cases.length < 3) {
  fail("migration corpus requires at least three cases");
}

const cargo = process.env.CARGO ?? "cargo";
run(cargo, ["build", "--locked", "--quiet", "-p", "pliego-cssc"], {
  cwd: root,
  env: cargoEnvironment,
});
const executable =
  process.env.PLIEGOCSS_BIN ??
  join(cargoTarget, "debug", process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc");
if (!existsSync(executable)) fail(`missing PliegoCSS CLI at ${executable}`);

const ids = new Set();
const results = [];
for (const entry of manifest.cases) {
  if (!entry.id || ids.has(entry.id)) fail(`duplicate or empty corpus id ${entry.id}`);
  ids.add(entry.id);
  const directory = resolve(corpusRoot, entry.directory);
  if (!directory.startsWith(`${corpusRoot}\\`) && !directory.startsWith(`${corpusRoot}/`)) {
    fail(`${entry.id}: directory escapes the corpus root`);
  }
  const before = snapshot(directory);
  const invocation = run(executable, ["migration-project-inventory", entry.declaration], {
    cwd: directory,
  });
  if (!invocation.stdout.endsWith("\n")) fail(`${entry.id}: stdout lacks trailing LF`);
  if (invocation.stderr !== "") fail(`${entry.id}: successful command wrote stderr`);
  const document = JSON.parse(invocation.stdout);
  for (const [key, expected] of Object.entries(entry.summary)) {
    if (document.summary?.[key] !== expected) {
      fail(`${entry.id}: summary.${key}=${document.summary?.[key]}, expected ${expected}`);
    }
  }
  for (const expected of entry.requiredDependencies ?? []) {
    const found = document.dependencies?.some((edge) =>
      Object.entries(expected).every(([key, value]) => edge[key] === value),
    );
    if (!found) fail(`${entry.id}: missing dependency ${JSON.stringify(expected)}`);
  }
  const after = snapshot(directory);
  if (JSON.stringify(after) !== JSON.stringify(before)) fail(`${entry.id}: fixture was mutated`);
  results.push({ id: entry.id, bytes: Buffer.byteLength(invocation.stdout), summary: entry.summary });
}

process.stdout.write(
  `${JSON.stringify({ schemaVersion: 1, classification: manifest.classification, cases: results }, null, 2)}\n`,
);
