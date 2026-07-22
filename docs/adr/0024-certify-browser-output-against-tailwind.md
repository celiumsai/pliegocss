# ADR-0024: Certify browser output against Tailwind

Date: 2026-07-22

Status: Accepted and implemented; release-candidate acceptance evidence remains source-bound

## Context

Compiler tests and a Chromium smoke do not prove that PliegoCSS and the current Tailwind lane
produce equivalent browser behavior. A release claim needs explicit reset ownership, computed-style
coverage, visual evidence, multiple engines, both supported CPU architectures, and artifacts bound
to one clean source identity.

## Decision

Browser/output certification v1 is the authority for the shared medium fixture. It compiles the
same DOM and theme with PliegoCSS and Benchmark Authority v2's `tailwind-latest` lane, then compares
52 frozen computed properties, 1/64-pixel border/text geometry, and bounded perceptual PNG output.

Every host executes five viewport/state scenarios in two modes: `no-reset`, where neither engine
receives a reset or Tailwind Preflight, and `shared-reset`, where the same hash-bound layered reset
is prepended to both outputs. Colors normalize to 8-bit sRGB, equivalent flex-end serialization is
canonicalized, and computed lengths quantize to 0.25 CSS px. Geometry is recorded at 1/64 CSS px
and fails above 0.1 CSS px delta. Each scenario records its maximum observed geometry delta and the
host summary records the recomputed maximum; the aggregator rejects absent, invalid, over-budget,
or inconsistent values. PNG comparison uses threshold 0.2 and a maximum mismatch ratio of 0.005
while retaining the original and diff hashes.

The hosted matrix is the 3×3 cross-product of Chromium, Firefox, and WebKit on Windows x64, Linux
x64, and macOS ARM64. Host evidence must be clean, unexpired, tied to one commit and Git tree, and
complete before aggregation passes. The common source identity must equal the aggregator checkout;
nine mutually consistent documents from another commit are rejected.

## Consequences

- Local Windows runs are diagnostic even when all three engines pass.
- WebKit is evidence for the Playwright WebKit engine; it is not renamed into a Safari claim.
- The tolerance is a reviewed antialias/subpixel budget, not an exact-PNG assertion; matching box
  and text-line geometry within 0.1 CSS px is independently mandatory. The 0.5% ceiling was
  calibrated from a dirty local Linux WebKit container diagnostic whose glyph-edge mismatch was
  0.4545022%, with zero computed differences and a maximum measured geometry delta of 0.09375 CSS px.
- Passing covers only the frozen shared utility intent. It does not prove general Tailwind
  compatibility, browser identity across engines, or a complete CSS implementation.
- `0.1.0` and later LTS promotion remain blocked until the clean hosted 9/9 artifact exists for the
  release source identity.

## Authority

- `benchmarks/browser-output-certification-v1/authority.json`
- `scripts/browser-output-certification.mjs`
- `.github/workflows/browser-output-certification.yml`
- `scripts/check-browser-output-matrix.mjs`
