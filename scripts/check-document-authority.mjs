import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const fail = (message) => { throw new Error(`document authority: ${message}`); };
const authority = JSON.parse(
  readFileSync(resolve(root, "docs/product/document-authority.json"), "utf8"),
);
const exact = (value, keys, role) => {
  if (!value || typeof value !== "object" || Array.isArray(value)) fail(`${role} must be an object`);
  if (JSON.stringify(Object.keys(value).sort()) !== JSON.stringify([...keys].sort())) {
    fail(`${role} fields drifted`);
  }
};
exact(
  authority,
  ["schemaVersion", "kind", "currentState", "siteProjection", "normative", "historical", "rules"],
  "root",
);
if (authority.schemaVersion !== 1 || authority.kind !== "pliegocss-document-authority") {
  fail("header drifted");
}
exact(authority.currentState, ["machine", "human", "generatedBy"], "currentState");
exact(authority.siteProjection, ["source", "route"], "siteProjection");
const requiredCurrent = {
  machine: "docs/product/release-readiness-0.1.0.json",
  human: "docs/product/release-readiness-0.1.0.md",
  generatedBy: "scripts/render-release-readiness.mjs",
};
if (JSON.stringify(authority.currentState) !== JSON.stringify(requiredCurrent)) {
  fail("current state is not singular and canonical");
}
if (
  authority.siteProjection.source !== "docs/site/release-readiness.md" ||
  authority.siteProjection.route !== "/docs/release-readiness/"
) {
  fail("site projection drifted");
}
for (const path of [
  ...Object.values(authority.currentState),
  authority.siteProjection.source,
  ...authority.normative,
]) {
  if (!existsSync(resolve(root, path))) fail(`missing governed document ${path}`);
}
for (const entry of authority.historical) {
  exact(entry, ["path", "supersededBy"], `historical.${entry.path}`);
  if (!entry.path.startsWith("docs/archive/")) fail(`${entry.path} is not archived`);
  if (!existsSync(resolve(root, entry.path))) fail(`missing archive ${entry.path}`);
  if (!existsSync(resolve(root, entry.supersededBy))) fail(`missing successor ${entry.supersededBy}`);
  const bytes = readFileSync(resolve(root, entry.path), "utf8");
  if (!bytes.includes("historical") && !bytes.includes("archiveStatus")) {
    fail(`${entry.path} lacks an archive boundary`);
  }
}
for (const stale of [
  "docs/product/hardening-report-0.1.0-rc.2.md",
  "docs/product/next-gap-2026-07-20.md",
  "docs/product/next-gap-2026-07-20.json",
]) {
  if (existsSync(resolve(root, stale))) fail(`stale current-looking path still exists: ${stale}`);
}
const readinessFiles = readdirSync(resolve(root, "docs/product"))
  .filter((name) => /^release-readiness-.+\.json$/u.test(name));
if (JSON.stringify(readinessFiles) !== JSON.stringify(["release-readiness-0.1.0.json"])) {
  fail("more than one current readiness machine document exists");
}
const render = spawnSync(
  process.execPath,
  [resolve(root, authority.currentState.generatedBy), "--check"],
  { cwd: root, encoding: "utf8", windowsHide: true },
);
if (render.error || render.status !== 0) {
  fail(`generated current page drifted: ${render.error?.message ?? render.stderr.trim()}`);
}
process.stdout.write(
  `${JSON.stringify({schemaVersion: 1, currentState: authority.currentState.machine, historical: authority.historical.length, siteRoute: authority.siteProjection.route}, null, 2)}\n`,
);
