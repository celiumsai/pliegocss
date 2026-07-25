import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { AuthorityError, validateReleaseReadiness } from "./release-authority.mjs";
import {
  loadExternalAdoptionBundle,
  validateExternalAdoptionBundle,
} from "./external-adoption-v1.mjs";

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
  const externalAdoptionBundle = loadExternalAdoptionBundle(root);
  const externalAdoption = validateExternalAdoptionBundle(externalAdoptionBundle, { root });
  const adoptionCheck = value.checks.find((check) => check.id === "external-adoption-evidence");
  if (
    JSON.stringify(adoptionCheck.requiredCoverage) !==
    JSON.stringify(externalAdoptionBundle.authority.requiredCoverage)
  ) {
    throw new AuthorityError(
      "external-adoption-evidence coverage drifted from the G7 authority",
    );
  }
  if (adoptionCheck.status === "passed" && externalAdoption.result !== "ready") {
    throw new AuthorityError(
      `external-adoption-evidence is passed while G7 is blocked by ${externalAdoption.missingCoverage.join(", ")}`,
    );
  }
  if (adoptionCheck.status === "passed") {
    const requiredArtifacts = new Set([
      "benchmarks/external-adoption-v1/authority.json",
      ...Object.values(externalAdoptionBundle.authority.evidence).map((entry) => entry.path),
      externalAdoptionBundle.authority.adapterSupportPolicy.path,
    ]);
    const actualArtifacts = new Set(adoptionCheck.artifacts.map((artifact) => artifact.path));
    for (const path of requiredArtifacts) {
      if (!actualArtifacts.has(path)) {
        throw new AuthorityError(
          `external-adoption-evidence passed without ${path}`,
        );
      }
    }
    if (
      !adoptionCheck.references.some(
        (reference) => reference.kind === "external-adoption-authority",
      )
    ) {
      throw new AuthorityError(
        "external-adoption-evidence passed without its authority reference",
      );
    }
  }
  process.stdout.write(`${JSON.stringify({ ...summary, externalAdoption }, null, 2)}\n`);
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
