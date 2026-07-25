import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  loadExternalAdoptionBundle,
  validateExternalAdoptionBundle,
} from "./external-adoption-v1.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const allowed = new Set(["--validate", "--require-promotion-ready"]);
const args = process.argv.slice(2);
if (args.some((arg) => !allowed.has(arg))) {
  process.stderr.write(
    "usage: node scripts/check-external-adoption.mjs [--validate] [--require-promotion-ready]\n",
  );
  process.exit(2);
}

try {
  const status = validateExternalAdoptionBundle(loadExternalAdoptionBundle(root), { root });
  process.stdout.write(`${JSON.stringify(status, null, 2)}\n`);
  if (args.includes("--require-promotion-ready") && status.result !== "ready") {
    process.stderr.write(
      `external adoption is blocked: ${status.missingCoverage.join(", ")}\n`,
    );
    process.exitCode = 1;
  }
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
}
