import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const path = resolve(root, "docs/product/release-readiness-0.1.0.json");
if (!existsSync(path)) throw new Error("release readiness document is missing");
const value = JSON.parse(readFileSync(path, "utf8"));
const fail = (message) => { throw new Error(`release readiness: ${message}`); };
if (value.schemaVersion !== 1 || value.targetVersion !== "0.1.0" || value.candidateVersion !== "0.1.0-rc.1") fail("header drifted");
if (value.repository !== "https://github.com/celiumsai/pliegocss") fail("repository drifted");
if (!Array.isArray(value.localGates) || value.localGates.length !== 8 || value.localGates.some((gate) => gate.status !== "passed")) fail("local gate accounting drifted");
if (!Array.isArray(value.externalBlockers) || value.externalBlockers.length !== 4) fail("external blocker inventory drifted");
const allowed = new Set(["blocked", "not-configured", "not-authorized", "pending-push"]);
if (value.externalBlockers.some((blocker) => !allowed.has(blocker.status))) fail("external blocker status is dishonest");
if (value.result !== "blocked") fail("0.1.0 must remain blocked while external blockers exist");
process.stdout.write(`${JSON.stringify({schemaVersion:1,candidate:value.candidateVersion,result:value.result,localPassed:value.localGates.length,externalBlocked:value.externalBlockers.length},null,2)}\n`);
