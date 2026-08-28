import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const document = JSON.parse(readFileSync(resolve(root, "docs/product/representative-applications-gap-map.json"), "utf8"));
function fail(message) { throw new Error(`representative gap map: ${message}`); }
if (document.schemaVersion !== 1 || document.selectedNext !== "classification-corpus") fail("header drifted");
if (!Array.isArray(document.applications) || document.applications.length !== 5) fail("exactly five applications required");
if (!Array.isArray(document.priorities) || document.priorities.length !== 4) fail("exactly four priorities required");
const ids = new Set();
for (const app of document.applications) {
  if (ids.has(app.id) || app.status !== "passed") fail(`invalid application ${app.id}`);
  ids.add(app.id);
  if (!app.gate.startsWith("integration:representative:")) fail(`invalid gate ${app.gate}`);
  if (!Array.isArray(app.proven) || !app.proven.length || !Array.isArray(app.gaps) || !app.gaps.length) fail(`${app.id} lacks evidence/gaps`);
}
for (const [index, priority] of document.priorities.entries()) {
  if (priority.rank !== index + 1) fail("priority ranks must be contiguous");
  if (!priority.exposedBy.every((id) => ids.has(id))) fail(`${priority.id} references unknown app`);
  if (!priority.reason || !["blocking", "should"].includes(priority.releaseImpact)) fail(`${priority.id} is incomplete`);
}
if (new Set(document.priorities.map((priority) => priority.id)).size !== document.priorities.length) {
  fail("priority IDs must be unique");
}
if (
  document.priorities[0]?.id !== document.selectedNext ||
  document.priorities[0]?.releaseImpact !== "blocking"
) {
  fail("selected next work must be the highest-ranked blocking priority");
}
for (const name of [
  "representative-plain-html-audit.md", "representative-vite-tailwind.md",
  "representative-css-modules.md", "representative-rust-control.md", "representative-framework-routes.md",
]) if (!existsSync(resolve(root, "docs/benchmarks", name))) fail(`missing ${name}`);
process.stdout.write(`${JSON.stringify({ schemaVersion: 1, applications: 5, priorities: 4, selectedNext: document.selectedNext }, null, 2)}\n`);
