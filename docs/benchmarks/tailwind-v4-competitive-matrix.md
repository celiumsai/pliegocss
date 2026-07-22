# PliegoCSS and Tailwind CSS 4.3.3

Status: **generated competitive matrix for release planning; not a universal performance or parity claim**

This document is generated from [`tailwind-v4-competitive-matrix.json`](./tailwind-v4-competitive-matrix.json).
The competitor and corpus authority is [Benchmark Authority v2](./tailwind-benchmark-authority-v2.md).
Edit those JSON contracts and regenerate; do not update this table by hand.

PliegoCSS is not positioned as “Tailwind in Rust.” Tailwind is the production utility-framework
baseline; PliegoCSS competes through typed semantics, policy, provenance, ownership, diagnostics and
receipts. Catalog breadth and a historical single-fixture timing are not a complete score.

## Active oracle

Primary lane: `tailwind-latest`. Oracle observation: `2026-07-22T15:25:28.117Z`.
Expiration: `2026-07-29T15:25:28.117Z`. Reset contract: **no-preflight**.

| Lane | Role | Registry selector | Version |
|---|---|---|---:|
| `tailwind-latest` | current-competitor | `latest` | `4.3.3` |
| `tailwind-v3-lts` | supported-historical-major | `v3-lts` | `3.4.19` |
| `tailwind-frozen-release` | historical-regression-control | `pinned` | `4.3.2` |

The `4.3.2` lane is historical regression control only. Current performance and payload claims
remain open until immutable schema-2 evidence is recorded from a clean tree with complete paired
samples, peak memory, gzip, Brotli and all three corpus sizes.

## Executive matrix

| Dimension | Area | Assessment | Tailwind | PliegoCSS | Release impact |
|---|---|---|---|---|---|
| `representative-fixture` | utility authoring | **matched** | The primary 4.3.3 lane and both historical lanes must emit every candidate in the complete five-view fixture before timing is comparable. | The shared medium corpus retains 44 class attributes, 302 occurrences, 90 unique concrete tokens, and all eight tracked variants. | keep-green |
| `catalog-breadth` | utility authoring | **tailwind-advantage** | Tailwind documents a broad production catalog across layout, typography, effects, filters, tables, transforms, interactivity, SVG, and accessibility. | The generated catalog is intentionally compact and covers the selected typed families plus arbitrary declarations. | non-blocking |
| `variants-responsive` | utility authoring | **matched** | Responsive and state variants are first-class authoring concepts. | The matched fixture proves sm/md/lg, hover, focus, focus-visible, placeholder, and disabled; additional typed dimensions include theme, motion, contrast, ARIA/data, direction, containers, and layers. | keep-green |
| `arbitrary-values-selectors` | escape hatches | **matched** | Arbitrary values, properties, and variants provide escape hatches within utility markup. | Parsed arbitrary values, one arbitrary declaration, custom properties, and bounded &[...] selector transforms preserve access to real CSS while rejecting unsafe/ambiguous shapes. | keep-green |
| `theme-configuration` | tokens and themes | **different-by-design** | CSS-first @theme variables define utility namespaces and can be shared as CSS. | Typed TOML and DTCG Resolver inputs produce versioned theme identity, a canonical token graph, and a Cargo build bridge. | keep-green |
| `source-detection` | content discovery | **different-by-design** | Scans source files as plain text for complete class tokens and supports explicit source registration. | Validates visible Rust macro literals or consumes explicit typed adapter manifests; it refuses to infer framework ownership from filenames/text alone. | keep-green |
| `preflight-reset` | base styles | **tailwind-advantage** | Ships Preflight as part of its normal full import. | Emits no reset; Benchmark Authority v2 fixes no-preflight as the only primary comparison contract. | non-blocking |
| `build-latency-fixture` | performance | **open-gap** | Benchmark Authority v2 pins latest, v3-lts, and a historical frozen lane but has no reviewed clean-tree v2 snapshot yet. | The paired harness records complete alternating latency and peak-working-set samples across micro, medium, and large corpora; legacy Gate B numbers remain historical only. | blocking |
| `payload-fixture` | performance | **open-gap** | The v2 harness measures raw, gzip, and Brotli CSS plus source-utility HTML for every lane and corpus. | The v2 harness measures the same compression formats after manifest-driven class rewriting and retains both separate-stream transfer totals. | blocking |
| `rust-compile-overhead` | performance | **tailwind-advantage** | Does not impose Rust procedural-macro compilation on non-Rust applications. | Typed Rust authoring adds a large cold Cargo cost in the frozen Gate A measurement, while incremental medians are much smaller. | non-blocking |
| `static-semantic-errors` | developer safety | **pliego-differentiator** | Complete-token extraction is productive but does not provide Pliego's typed property-slot conflict and theme-domain contract. | Rejects unknown utilities/tokens, invalid domains, conflicts, impossible conditions, malformed arbitrary values, and cross-branch conflicts with stable diagnostics. | keep-green |
| `standard-css-audit` | governance | **pliego-differentiator** | The compared core contract is utility generation, not a general integrity-bound CSS policy receipt. | Accepts normal CSS for bounded compatibility, budget, accessibility, finding, and receipt workflows without requiring utility migration. | blocking |
| `provenance-receipts` | governance | **pliego-differentiator** | The compared CLI emits CSS but does not expose Pliego's source/config/output decision graph and canonical build/verification receipts. | Versioned manifests, source maps, semantic/physical graphs, finding documents, and receipts integrity-bind controlled workflows. | blocking |
| `budgets-ownership-dead-css` | governance | **pliego-differentiator** | Automatic utility generation removes absent tokens, but the compared contract does not classify observed/unobserved/dead/unknown with ownership-backed route/package receipts. | Budgets, typed ownership, exact reachability/observation evidence, retention, and conservative whole-StyleId pruning are implemented. | blocking |
| `accessibility-policy` | governance | **pliego-differentiator** | Provides utilities and variants useful for accessible designs but does not certify their system-level correctness. | Static policy distinguishes verified, unverified, manual-required, violation, and exception states for five bounded guard categories. | blocking |
| `agent-repair` | agent safety | **pliego-differentiator** | The compared CLI has no equivalent closed proposal/authorization/change/verification receipt protocol. | Exact low-risk edits, budgets, external authorization, idempotence, rollback, fixed runners, and canonical verification receipts are bounded by schemas. | non-blocking |
| `migration-bridge` | adoption | **open-gap** | Has a mature installed ecosystem and direct v4 authoring path. | Can inventory bounded Tailwind/Sass/CSS Modules projects read-only, but reversible codemods and semantic migration outcomes are not implemented. | should |
| `ecosystem-frameworks` | adoption | **tailwind-advantage** | Has broad framework integrations, established documentation, and a production user ecosystem. | Has a pinned PliegoRS path, plain HTML evidence, nineteen published Rust crates, and source-distributed editor clients, but a much smaller production ecosystem. | non-blocking |
| `distribution` | adoption | **tailwind-advantage** | Published npm packages and documented install paths are available today. | The exact-version 0.1.0-rc.2 crate graph, CLI, and LSP are published on crates.io and replayed with Rust 1.85; editor clients remain source-distributed candidates. | non-blocking |
| `browser-compatibility` | platform support | **open-gap** | Upstream documents a modern-browser compatibility floor and v4.3 behavior. | Uses versioned targets and official-data policy. Browser/output certification v1 implements same-host computed, bounded 1/64-pixel geometry, and bounded PNG comparison against the current Tailwind lane across Chromium, Firefox, and WebKit; dirty local Windows x64 and Linux x64 diagnostics pass, while the clean hosted 9/9 artifact and complete CSS classification remain incomplete. | blocking |
| `responsive-diagnosis` | platform support | **open-gap** | Provides responsive authoring but not Pliego's proposed causal browser diagnosis product. | Current Chromium fixture checks validate known outputs; viewport-matrix failure localization is not implemented. | post-0.1 |
| `plugin-extensibility` | ecosystem | **different-by-design** | Supports custom styles, directives, and an ecosystem-oriented extension model. | Defers unrestricted plugins to protect deterministic policy, schemas, and receipts; extensions must eventually be restricted and sandboxed. | post-0.1 |

Status totals: `matched` 3, `pliego-differentiator` 6, `tailwind-advantage` 5, `different-by-design` 3, `open-gap` 5.

## Blocking

- `build-latency-fixture`: Record and review immutable schema-2 evidence from a clean source commit before making a current speed claim.
- `payload-fixture`: Freeze a clean-tree v2 snapshot and keep all raw, gzip, Brotli, HTML, CSS, and transfer values visible.
- `standard-css-audit`: Close R0 standard-CSS classification and generic transform/minify output gaps before 0.1.0.
- `provenance-receipts`: Converge the R0 unified artifact and obtain hosted cross-OS evidence without relabeling existing schemas.
- `budgets-ownership-dead-css`: Close generic-CSS identities, hosted evidence, and final release replay.
- `accessibility-policy`: Add reviewed precision/recall plus browser/manual evidence; retain the explicit no-WCAG-certification limit.
- `browser-compatibility`: Produce the unexpired clean-tree 9/9 Windows x64, Linux x64, and macOS ARM64 matrix, then close the supported CSS corpus while preserving explicit blocked/degraded decisions.

## Should

- `migration-bridge`: Deliver conservative reversible migration slices selected from representative applications, with visual and receipt evidence.

## Explicitly post-0.1

- `responsive-diagnosis`: Keep experimental until a bounded evidence schema and reviewed precision corpus exist.
- `plugin-extensibility`: Do not add unrestricted execution; design a versioned policy-pack/plugin boundary only after core release gates close.

## Historical evidence boundary

[Gate B](./pliego-gate-b.md) remains immutable historical evidence for Tailwind `4.3.2` on its
recorded host. It cannot represent `tailwind-latest`, v3-LTS, current paired latency, memory,
Brotli, or micro/medium/large behavior. No current competitive score is emitted from that snapshot.

## Verification

```console
pnpm check:benchmark-authority
pnpm check:benchmark-oracle
pnpm check:tailwind-matrix
node scripts/measure-benchmark-authority-v2.mjs --smoke
```
