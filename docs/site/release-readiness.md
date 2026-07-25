<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/release-readiness/","category":"Evidence","eyebrow":"Release readiness","order":30}
-->

# Release truth bound to one exact candidate.

The current 1.0 promotion state is blocked; no historical report can override these source-bound gates.

## Candidate identity {#candidate}

Target 0.1.0; candidate 0.1.0-rc.2; tag v0.1.0-rc.2; commit 064dcbce96a3a5cc97a940d07566c008bb5e2d3e; Git tree 010a64f5d0bbd142a1d015fda4ccf573cc6e4133; observed 2026-07-22T14:10:00Z; expires 2026-07-29T14:10:00Z.

## Readiness dimensions {#dimensions}

Technical: blocked; Operational: blocked; Adoption: blocked; Authorized: blocked; Promotion: blocked.

## Current blockers {#blockers}

g0-corrections-on-candidate (pending): G0 corrections are not part of the exact RC.2 candidate. They require a newly named candidate plus hosted replay before they can become release evidence. browser-release-matrix (blocked): Every required hosted browser lane is not configured for the exact candidate source; local or historical browser runs have no promotion authority. adapter-coexistence-matrix (blocked): RC.2 predates the G6 adapter authority. It has no clean exact-source 21-project HTML/Vite/PliegoRS matrix, Tailwind v3/v4 source/output audit, DOM/ARIA/style equivalence, exact rollback, or pinned PliegoRS framework replay. registry-replay-refresh (pending): RC.2 was previously replayed from crates.io, but schema 3 requires a fresh, unexpired, artifact-hashed replay for the exact promotion candidate. production-deployment-refresh (pending): The production site has historical deployment evidence, but a current exact-source edge and browser replay with hashed artifacts is missing. signed-binary-distribution (blocked): RC.2 predates the repository-only G5 distribution contract. It has no exact-source native archives, checksums, CycloneDX SBOMs, GitHub/Sigstore attestations, immutable GitHub Release, or verified pnpm package asset; npmjs publication is forbidden. external-adoption-evidence (pending): RC.2 predates the G7 authority. Recruitment is open, but the required external interviews, real incidents, and pilots have not been recorded as consented, redacted, reviewed, hash-bound evidence. final-promotion-authorization (not-authorized): Mario has not explicitly authorized publishing final 0.1.0 for this exact source commit and Git tree.
