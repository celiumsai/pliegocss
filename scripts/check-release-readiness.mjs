import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { AuthorityError, validateReleaseReadiness } from "./release-authority.mjs";

const root = resolve(import.meta.dirname, "..");
const args = process.argv.slice(2);
const enforceRelease =
  (args.length === 1 && args[0] === "--profile=release") ||
  (args.length === 2 && args[0] === "--profile" && args[1] === "release");
if (
  !enforceRelease &&
  !(
    args.length === 0 ||
    (args.length === 1 && args[0] === "--validate")
  )
) {
  throw new Error(`release readiness: unknown mode ${args.join(" ")}`);
}

try {
  const value = JSON.parse(
    readFileSync(resolve(root, "docs/product/release-readiness-0.1.0.json"), "utf8"),
  );
  const summary = validateReleaseReadiness(value, { root });
  process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
  if (enforceRelease && summary.result !== "ready") {
    process.stderr.write(
      `release readiness: promotion blocked by ${summary.blockers.join(", ")}\n`,
    );
    process.exitCode = 1;
  }
} catch (error) {
  if (error instanceof AuthorityError) {
    throw new Error(`release readiness: ${error.message}`);
  }
  throw error;
}
