import { spawnSync } from "node:child_process";
import { performance } from "node:perf_hooks";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const binary = resolve(root, process.platform === "win32" ? "target/release/pliego-cssc.exe" : "target/release/pliego-cssc");
const fixture = resolve(root, "integration-tests/representative/vite-tailwind-inventory");
const cases = [
  ["inventory", ["migration-project-inventory", "."]],
  ["plan", ["migration-project-plan", "."]],
];
const result = { schemaVersion: 1, fixture: "vite-tailwind-inventory", runs: 20, measurements: {} };
for (const [name, args] of cases) {
  const values = [];
  for (let index = 0; index < result.runs + 2; index++) {
    const start = performance.now();
    const child = spawnSync(binary, args, { cwd: fixture, encoding: "utf8" });
    const elapsed = performance.now() - start;
    if (child.status !== 0) throw new Error(`${name} failed: ${child.stderr}`);
    if (index >= 2) values.push(elapsed);
  }
  values.sort((a, b) => a - b);
  result.measurements[name] = {
    medianMs: Number(values[Math.floor(values.length / 2)].toFixed(3)),
    p95Ms: Number(values[Math.ceil(values.length * 0.95) - 1].toFixed(3)),
    maxMs: Number(values.at(-1).toFixed(3)),
  };
}
if (result.measurements.plan.p95Ms > 250 || result.measurements.inventory.p95Ms > 250) throw new Error(`migration adoption latency exceeded 250ms: ${JSON.stringify(result)}`);
process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
