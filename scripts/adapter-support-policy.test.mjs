import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { loadContract, repositoryRoot } from "./adapter-coexistence-v1.mjs";
import { validateSupportPolicy } from "./check-adapter-coexistence-authority.mjs";

const { authority } = loadContract();
const policy = JSON.parse(
  readFileSync(resolve(repositoryRoot, "docs", "product", "adapter-support-policy-0.1.x.json")),
);
const manifest = readFileSync(
  resolve(repositoryRoot, "integration-tests", "pliegors-smoke", "Cargo.toml"),
  "utf8",
);
const contract = JSON.parse(
  readFileSync(
    resolve(repositoryRoot, "integration-tests", "pliegors-smoke", "pliegors-contract.json"),
    "utf8",
  ),
);

const duplicateProfile = structuredClone(policy);
duplicateProfile.supportedTailwindProfiles.push(duplicateProfile.supportedTailwindProfiles[0]);
assert.throws(
  () => validateSupportPolicy(duplicateProfile, authority, manifest, contract),
  /profile inventory drifted|duplicate IDs/u,
);

const expandedClaim = structuredClone(policy);
expandedClaim.claimBoundary = "All Tailwind and framework configurations are certified.";
assert.throws(
  () => validateSupportPolicy(expandedClaim, authority, manifest, contract),
  /claim boundary drifted/u,
);

const staleRevision = structuredClone(contract);
staleRevision.revision = "0".repeat(40);
assert.throws(
  () => validateSupportPolicy(policy, authority, manifest, staleRevision),
  /source revision drifted/u,
);

process.stdout.write("adapter support policy fail-closed contract: pass\n");
