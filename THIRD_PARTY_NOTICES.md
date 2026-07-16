# Third-party notices

PliegoCSS is licensed under Apache-2.0. This curated notice covers the external standards, CSS
backend, compatibility datasets, and benchmark selected by the schema-1 provenance gate. It is not
an exhaustive dependency notice, SBOM, or legal audit. Other Cargo/npm dependencies remain governed
by their package licenses and lockfiles. Nothing here replaces upstream license texts or grants
additional rights.

## Build-time software

### Lightning CSS 1.0.0-alpha.71

- Relationship: downstream CSS backend used at build time.
- License: MPL-2.0.
- Source: https://github.com/parcel-bundler/lightningcss
- Locked evidence: `Cargo.lock`, checksum
  `cb6314c2f0590ac93c86099b98bb7ba8abcf759bfd89604ffca906472bb54937`.

## Frozen compatibility data

### web-features 3.32.0

- Relationship: frozen WebDX compatibility dataset; no floating network lookup.
- License: Apache-2.0.
- Source: https://github.com/web-platform-dx/web-features
- Frozen metadata: npm SRI plus the declared `data.json` SHA-256 in
  `standards/provenance.json`; dataset bytes are not vendored or rehashed by this gate.

### baseline-browser-mapping 2.10.43

- Relationship: frozen browser mapping for the dated Baseline query; no floating network lookup.
- License: Apache-2.0.
- Source: https://github.com/web-platform-dx/baseline-browser-mapping
- Frozen metadata: npm SRI plus the declared fixed-query result SHA-256 in
  `standards/provenance.json`; package/result bytes are not vendored or rehashed by this gate.

## Implemented exchange reports

The following Design Tokens Community Group 2025.10 documents are **Final Community Group
Reports**, published under the W3C Community Final Specification Agreement. They are not W3C
Recommendations and are not on the W3C Standards Track.

- Design Tokens Format Module 2025.10 — W3C-Community-Final-Specification-Agreement —
  https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/
- Design Tokens Color Module 2025.10 — W3C-Community-Final-Specification-Agreement —
  https://www.w3.org/community/reports/design-tokens/CG-FINAL-color-20251028/
- Design Tokens Resolver Module 2025.10 — W3C-Community-Final-Specification-Agreement —
  https://www.w3.org/community/reports/design-tokens/CG-FINAL-resolver-20251028/

## Accessibility and CSS references

These versioned W3C documents are referenced by PliegoCSS policy and documentation. They are not
bundled software dependencies.

- Web Content Accessibility Guidelines (WCAG) 2.2 — W3C Recommendation —
  `W3C-Document-License-2023` —
  https://www.w3.org/TR/2024/REC-WCAG22-20241212/
- CSS Color Adjustment Module Level 1 — W3C Candidate Recommendation Snapshot —
  `W3C-Software-and-Document-License-2023` —
  https://www.w3.org/TR/2025/CR-css-color-adjust-1-20251216/
- Media Queries Level 5 — W3C Working Draft —
  `W3C-Software-and-Document-License-2023` —
  https://www.w3.org/TR/2026/WD-mediaqueries-5-20260219/

## Benchmark-only software

### Tailwind CSS and CLI 4.3.2

- Relationship: frozen performance and ergonomics baseline only; not a PliegoCSS runtime/backend
  dependency.
- License: MIT.
- Source: https://github.com/tailwindlabs/tailwindcss
- Locked evidence: `tailwindcss@4.3.2`, `@tailwindcss/cli@4.3.2`, and the complete transitive
  `pnpm-lock.yaml` graph at SHA-256
  `c4f9b91705418524bd91178fd417e3b02585dfce613c2a56e6cef25a6c8c1049`.
