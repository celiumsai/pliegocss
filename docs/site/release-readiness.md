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

g0-corrections-on-candidate (pending): G0 corrections exist only in the later uncommitted worktree. They require a reviewed commit/tree and hosted replay before they can become release evidence. browser-release-matrix (blocked): Every required hosted browser lane is not configured for the exact candidate source; local or historical browser runs have no promotion authority. registry-replay-refresh (pending): RC.2 was previously replayed from crates.io, but schema 3 requires a fresh, unexpired, artifact-hashed replay for the exact promotion candidate. production-deployment-refresh (pending): The production site has historical deployment evidence, but a current exact-source edge and browser replay with hashed artifacts is missing. signed-binary-distribution (blocked): Signed native binaries, checksums, SBOMs, attestations, and the npm launcher required by the LTS distribution boundary do not exist yet. external-adoption-evidence (pending): The required external interviews, real incidents, and pilots have not been recorded as reviewed evidence. final-promotion-authorization (not-authorized): Mario has not explicitly authorized publishing final 0.1.0 for this exact source commit and Git tree.
