# Standards and third-party provenance schema 1

Status: **machine-enforced offline**

[`standards/provenance.json`](../../standards/provenance.json) is the curated inventory for the
standards, CSS backend, compatibility datasets, and benchmark selected by this gate. It is not an
exhaustive dependency notice, SBOM, or legal audit. Run the deterministic gate with:

```console
pnpm check:attribution
```

The command performs no network requests. It rejects unknown fields, duplicate identities or URLs,
noncanonical order, invalid versions/licenses/HTTPS URLs, missing evidence, and drift from the
reviewed records. It then checks the exact local anchors: Cargo and pnpm locks, frozen compatibility
metadata, DTCG adapter constants, accessibility references, CI/release wiring, documentation links,
and the curated third-party notice.

## Closed schema

The root has exactly `schemaVersion` (`1`) and `sources`. Every source has exactly these fields:

| Field | Contract |
|---|---|
| `id` | Stable lowercase identifier; unique and sorted. |
| `name` | Human-readable upstream name. |
| `kind` | `software`, `dataset`, or `specification`. |
| `version` | Exact numeric or SemVer-like version; no ranges or moving tags. |
| `status` | Exact publication/package status. |
| `license` | Reviewed license identifier. |
| `url` | Canonical, credential-free HTTPS source; unique. |
| `relationship` | Why PliegoCSS records the source. |
| `usage` | `build-time`, `frozen-data`, `implemented-contract`, `audit-reference`, or `benchmark-only`. |
| `evidence[]` | Existing repository-relative files that prove the relationship. |
| `integrity[]` | Unique subject plus reviewed npm SRI, SHA-256 metadata, or Cargo checksum; empty only for versioned specifications. |

All objects are closed. Additive fields require a new schema version. Updating a source requires a
reviewed manifest/checker change and regeneration of every affected lock, golden, notice, and
benchmark artifact.

## Current boundary

| Source | Exact identity | Relationship |
|---|---|---|
| Lightning CSS | `1.0.0-alpha.71`, MPL-2.0 | Downstream build-time CSS backend; its exact Cargo checksum must match `Cargo.lock`. |
| `web-features` | `3.32.0`, Apache-2.0 | Frozen compatibility metadata records npm SRI and the declared SHA-256 of `data.json`. |
| `baseline-browser-mapping` | `2.10.43`, Apache-2.0 | Frozen metadata records package SRI and the declared fixed-query result SHA-256. |
| DTCG Format, Color, Resolver | `2025.10` | Stable **Final Community Group Reports** under the W3C Community FSA. They are not W3C Recommendations or W3C Standards. |
| WCAG | `2.2` Recommendation | Authoritative accessibility reference; a static PliegoCSS pass is not a WCAG conformance claim. |
| CSS Color Adjustment | Level 1 Candidate Recommendation Snapshot | Forced-colors reference. |
| Media Queries | Level 5 Working Draft | User-preference media-query reference; draft status is explicit. |
| Tailwind CSS and CLI | `4.3.2`, MIT | Performance/ergonomics baseline only; package SRI plus the full transitive pnpm lock SHA-256 are frozen. It is not a PliegoCSS runtime or backend dependency. |

The DTCG status comes from the official [Format](https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/),
[Color](https://www.w3.org/community/reports/design-tokens/CG-FINAL-color-20251028/), and
[Resolver](https://www.w3.org/community/reports/design-tokens/CG-FINAL-resolver-20251028/) reports.
The accessibility references are the dated [WCAG 2.2 Recommendation](https://www.w3.org/TR/2024/REC-WCAG22-20241212/),
[CSS Color Adjustment Level 1 snapshot](https://www.w3.org/TR/2025/CR-css-color-adjust-1-20251216/),
and [Media Queries Level 5 draft](https://www.w3.org/TR/2026/WD-mediaqueries-5-20260219/).
WCAG links to the W3C Document License 2023; the two CSS reports link to the distinct W3C Software
and Document License 2023. The manifest keeps those identifiers separate.

The compatibility golden vendors only identities, SRI values, and declared hashes. The gate does
not vendor, download, or rehash the upstream dataset/package bytes; reproducing those hashes is a
separate evidence-capture operation.

See [`THIRD_PARTY_NOTICES.md`](../../THIRD_PARTY_NOTICES.md) for the human-readable attribution.
