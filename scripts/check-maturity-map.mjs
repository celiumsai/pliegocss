import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const path = resolve(root, "docs/product/maturity-map.json");
const document = JSON.parse(readFileSync(path, "utf8"));

function fail(message) {
  throw new Error(`maturity map: ${message}`);
}

if (document.schemaVersion !== 1) fail("schemaVersion must be 1");
if (document.releaseTarget !== "0.1.0") fail("releaseTarget must be 0.1.0");
if (!/^\d{4}-\d{2}-\d{2}$/.test(document.statusDate ?? "")) {
  fail("statusDate must be an ISO date");
}
const maturities = new Set(["stable", "beta", "experimental"]);
if (Object.keys(document.levels ?? {}).sort().join(",") !== [...maturities].sort().join(",")) {
  fail("levels must define stable, beta, and experimental exactly");
}
if (!Array.isArray(document.capabilities) || document.capabilities.length === 0) {
  fail("capabilities must be non-empty");
}
const ids = new Set();
const counts = { stable: 0, beta: 0, experimental: 0 };
for (const [index, capability] of document.capabilities.entries()) {
  const label = `capabilities[${index}]`;
  if (!/^[a-z][a-z0-9-]*$/.test(capability.id ?? "")) fail(`${label}.id is not canonical`);
  if (ids.has(capability.id)) fail(`duplicate capability id ${capability.id}`);
  ids.add(capability.id);
  if (!maturities.has(capability.maturity)) fail(`${label}.maturity is unknown`);
  counts[capability.maturity] += 1;
  for (const field of ["title", "scope"]) {
    if (typeof capability[field] !== "string" || capability[field].trim() === "") {
      fail(`${label}.${field} must be non-empty`);
    }
  }
  for (const field of ["owners", "gates", "graduationCriteria", "limitations"]) {
    if (!Array.isArray(capability[field]) || capability[field].length === 0) {
      fail(`${label}.${field} must be a non-empty array`);
    }
    if (capability[field].some((value) => typeof value !== "string" || value.trim() === "")) {
      fail(`${label}.${field} entries must be non-empty strings`);
    }
  }
  if (!Array.isArray(capability.schemas)) fail(`${label}.schemas must be an array`);
  for (const owner of capability.owners) {
    if (!existsSync(resolve(root, owner))) fail(`${label}.owners references missing ${owner}`);
  }
}
const expected = { stable: 6, beta: 11, experimental: 4 };
for (const maturity of maturities) {
  if (counts[maturity] !== expected[maturity]) {
    fail(`expected ${expected[maturity]} ${maturity} capabilities, found ${counts[maturity]}`);
  }
}
const markdown = readFileSync(resolve(root, "docs/product/maturity-map.md"), "utf8");
for (const maturity of maturities) {
  if (!markdown.toLowerCase().includes(`## ${maturity}`)) {
    fail(`human map is missing the ${maturity} section`);
  }
}
process.stdout.write(`${JSON.stringify({ schemaVersion: 1, capabilities: document.capabilities.length, counts }, null, 2)}\n`);
