import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const matrix = JSON.parse(readFileSync(resolve(root, "docs/benchmarks/hosted-browser-matrix.json"), "utf8"));
const currentOs = process.platform === "win32" ? "windows" : process.platform;
function fail(message) { throw new Error(`hosted browser matrix: ${message}`); }
if (matrix.schemaVersion !== 1 || matrix.hosts?.length !== 5) fail("header or host inventory drifted");
const allowed = new Set(["available", "not-configured", "passed", "failed"]);
for (const host of matrix.hosts) {
  if (!allowed.has(host.status)) fail(`invalid status for ${host.id}`);
  if (host.status === "passed" && host.automation !== "passed") fail(`${host.id} claims pass without automation`);
  if (host.status === "available" && (!host.executable || !["executable-discovery-only", "local-computed-style-smoke"].includes(host.claim))) fail(`${host.id} availability is not bounded`);
  if (host.status === "not-configured" && host.claim !== "none") fail(`${host.id} claims evidence while not configured`);
  if (host.executable && currentOs === host.os && !existsSync(resolve(host.executable))) fail(`${host.id} executable is absent: ${host.executable}`);
}
if (matrix.historicalEvidence?.status !== "historical-manual" || matrix.historicalEvidence.ciReplay !== "not-configured") fail("historical evidence boundary drifted");
process.stdout.write(`${JSON.stringify({schemaVersion:1,available:matrix.hosts.filter((host)=>host.status==='available').map((host)=>host.id),passed:matrix.hosts.filter((host)=>host.status==='passed').map((host)=>host.id),notConfigured:matrix.hosts.filter((host)=>host.status==='not-configured').map((host)=>host.id)},null,2)}\n`);
