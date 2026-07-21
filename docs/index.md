# PliegoCSS documentation

PliegoCSS is an experimental standards-first CSS compiler and verifier for humans and agents. It
accepts compact typed Rust authoring today, but that is one frontend rather than the product
category: standard-CSS audit, policy, explainability, and deterministic receipts are the controlling
direction. The current workspace has compile-time syntax and conflict validation, typed semantic IR,
deterministic CSS, typed configurable themes, responsive and interactive variants, conditional
composition, and an explicit build CLI. Rust 1.85 is the minimum supported version.

Gate A and Gate B are closed as conditional GO decisions over their frozen fixtures. Those results
validate continued development; they do not declare the compiler production-ready or close the
PliegoRS integration and developer-experience phases. The current PliegoRS evidence is a pinned
local cross-repository SSR/SSG gate plus a resumability-markup seam with declarative, explicitly
owned route bundles. `pliego-cssc watch` only regenerates artifacts; composed with the pinned
`pliego dev`, the certified local path performs a PliegoRS-owned full-page browser reload. The CLI
  accepts an adapter-owned, exact-range reachability sidecar and projects it into
  opt-in manifest schema 4 without inferring framework semantics. Schema 5 additionally reconciles
  that semantic graph with exact rules, declarations, and UTF-8 byte ranges in the final CSS.
  Explicit bundle builds can also emit a framework-neutral asset load plan with exact CSS/manifest
  hashes and independent route/island bundle selections. Schema-5 builds can additionally emit one
  portable Project Index joining source sites to semantic, token, component, and physical CSS IDs.

## Getting started

- [Installation](./getting-started/installation.md)
- [First standard-CSS audit](./getting-started/first-audit.md)
- [First compile](./getting-started/first-compile.md)
- [PliegoRS + PliegoCSS from an empty project](./getting-started/pliegors.md)
- [Project structure](./getting-started/project-structure.md)
- [Editor setup for VS Code and Neovim](./getting-started/editor-setup.md)
- [Mental model](./learn/mental-model.md)
- [Themes and tokens](./learn/themes-and-tokens.md)
- [Configure custom breakpoints](./how-to/custom-breakpoints.md)
- [Use typed container queries](./how-to/container-queries.md)
- [Use typed ARIA and data variants](./how-to/attribute-variants.md)
- [Use typed writing modes](./how-to/writing-modes.md)
- [Use typed cascade layers](./how-to/cascade-layers.md)
- [Configure CSS budgets](./how-to/configure-budgets.md)
- [Configure package ownership and route composition](./how-to/configure-ownership.md)
- [Configure accessibility policies](./how-to/configure-accessibility-policies.md)
- [Audit CSS usage with reachability and observation evidence](./how-to/audit-css-usage.md)
- [PliegoRS development reload loop](./how-to/pliegors-dev-loop.md)
- [Theme troubleshooting](./troubleshooting/themes.md)
- [Ownership sidecar troubleshooting](./troubleshooting/ownership-sidecar.md)
- [Accessibility audit troubleshooting](./troubleshooting/accessibility-audit.md)
- [Usage-evidence troubleshooting](./troubleshooting/usage-evidence.md)
- [LSP troubleshooting](./troubleshooting/lsp.md)
- [PliegoRS integration boundary](./integrations/pliegors.md)

## Reference

- [Syntax contract](./reference/syntax.md)
- [Implemented utility reference](./reference/utilities.md)
- [Generated utility and seed-token catalog](./reference/generated-catalog.md)
- [Theme schema](./reference/theme-schema.md)
- [DTCG 2025.10 exchange bridge](./reference/dtcg-bridge.md)
- [Canonical token graph schema 1](./reference/token-graph-schema-1.md)
- [Token-usage report schema 1](./reference/token-usage-schema-1.md)
- [Critical-style evidence schema 1](./reference/critical-style-evidence-schema-1.md)
- [Command-line compiler](./reference/cli.md)
- [Language Server Protocol transport](./reference/lsp.md)
- [LSP diagnostic corpus schema 2](./reference/lsp-diagnostic-corpus.md)
- [Public API candidate](./reference/public-api.md)
- [Declarative bundle-plan schema](./reference/bundle-plan.md)
- [Asset load-plan schemas 1 and 2](./reference/asset-plan-schema.md)
- [Ownership sidecar schema 1](./reference/ownership-schema-1.md)
- [Project Index schemas 1 and 2](./reference/project-index-schema.md)
- [Provenance manifest schema 4](./reference/manifest-schema-4.md)
- [Physical provenance manifest schema 5](./reference/manifest-schema-5.md)
- [Reachability sidecar schema 1](./reference/reachability-schema.md)
- [Usage analysis schemas 1 and 2](./reference/usage-analysis-schema-1.md)
- [Usage observation schema 1](./reference/usage-observation-schema-1.md)
- [Usage retention sidecar schema 1](./reference/usage-retention-schema-1.md)
- [CLI diagnostic JSON schema](./reference/diagnostic-schema.md)
- [Canonical finding schema 1.0.0](./reference/finding-schema-1.md)
- [Control manifest and build receipt schema 1.0.0](./reference/control-artifacts-schema-1.md)
- [Canonical CSS Source Map v3 contract](./reference/css-source-maps.md)
- [Standard CSS audit command](./reference/audit-command.md)
- [Bounded standard-CSS classifier corpus](./reference/standard-css-classifier-corpus.json)
- [Bounded cascade explanation command](./reference/cascade-explain-command.md)
- [Repair proposal, plan, dry-run, and Change Receipt schemas 1.0.0](./reference/repair-plan-schema.md)
- [Repair check policy and Verification Receipt schemas 1.4.0](./reference/repair-verification-schema.md)
- [Migration inventory schema 1](./reference/migration-inventory-schema-1.md)
- [Migration project inventory schema 1](./reference/migration-project-inventory-schema-1.md)
- [Reversible migration plan schema 1](./reference/reversible-migration-plan-schema-1.md)
- [CSS budget policy schema 1](./reference/budget-policy.md)
- [Accessibility policy schema 1](./reference/accessibility-policy.md)
- [Standards and third-party provenance schema 1](./reference/standards-provenance.md)
- [StyleId binary format 2](./reference/style-id-format-v2.md)
- [Semantic IR binary format 2](./reference/semantic-ir-binary-v2.md)
- [Historical semantic IR binary format 1](./reference/semantic-ir-binary-v1.md)
- [Compatibility candidate contract](./reference/compatibility.md)
- [Compatibility policy schema 2](./reference/compatibility-policy.md)
- [Initial utility catalog (F0 plan)](./reference/initial-utility-catalog.md)
- [Glossary](./glossary.md)

## Concepts

- [Research-derived product requirements and alpha gates](./product/research-requirements.md)
- [Strategic CSS/Rust/AI product contract and R0-R3 gates](./product/strategic-product-contract-2026.md)
- [Product maturity map](./product/maturity-map.md)
- [Representative applications and selected gaps](./product/representative-applications-gap-map.md)
- [R0 unified manifest and receipt design](./product/r0-manifest-receipt-design.md)
- [Typed IR](./concepts/typed-ir.md)
- [Conflict model](./concepts/conflict-model.md)
- [CSS emission](./concepts/css-emission.md)
- [Composition and conditional styles](./concepts/composition.md)
- [Architecture decisions](./adr/README.md)

## Status and evidence

- [0.1.0-rc.1 hardening report](./product/hardening-report-0.1.0-rc.1.md)
- [0.1.0 release readiness](./product/release-readiness-0.1.0.md)
- [F0 verification](./status/f0-verification.md)
- [F1 parser and IR](./status/f1.md)
- [F2 semantic compiler](./status/f2.md)
- [F3 deterministic CSS pipeline](./status/f3.md)
- [F4 variants and composition](./status/f4.md)
- [F5 native PliegoRS integration](./status/f5.md)
- [F6 developer experience](./status/f6.md)
- [F7 measured optimization and reachability](./status/f7.md)
- [Reachability-pruning benchmark](./benchmarks/reachability-pruning.md)
- [Repair authority conformance corpus](./benchmarks/repair-corpus.md)
- [Migration bridge contract corpus](./benchmarks/migration-corpus.md)
- [Reviewed public migration role corpus](./benchmarks/migration-real-corpus.md)
- [Portability contract](./status/portability.md)
- [Packaging contract](./status/packaging.md)
- [Gate A status](./status/gate-a.md)
- [Gate A benchmark](./benchmarks/pliego-gate-a.md)
- [Gate B complete-fixture benchmark](./benchmarks/pliego-gate-b.md)
- [Competitive matrix against Tailwind CSS v4.3.2](./benchmarks/tailwind-v4-competitive-matrix.md)
- [Representative application 1: plain HTML audit-first](./benchmarks/representative-plain-html-audit.md)
- [Representative application 2: Vite/Tailwind inventory-first](./benchmarks/representative-vite-tailwind.md)
- [Representative application 3: CSS Modules consumer](./benchmarks/representative-css-modules.md)
- [Representative application 4: typed Rust controlled build](./benchmarks/representative-rust-control.md)
- [Representative application 5: framework-neutral multiroute bundles](./benchmarks/representative-framework-routes.md)
- [Adjacent media-query merge benchmark](./benchmarks/media-query-merging.md)
- [Rust check baseline](./benchmarks/rust-check-baseline.md)
- [Frozen machine snapshots](../benchmarks/evidence/README.md)
- [Browser validation](./benchmarks/browser-validation.md)
- [Hosted browser/OS evidence matrix](./benchmarks/hosted-browser-matrix.json)
- [PliegoRS development-loop latency and browser reload](./benchmarks/pliegors-dev-loop.md)
- [Migration inventory and plan adoption latency](./benchmarks/migration-adoption-latency.md)
- [WASM runtime overhead](./benchmarks/wasm-overhead.md)

## Contributing and releases

- [Brand system](../brand/BRANDBOOK.md)
- [Contributing](../CONTRIBUTING.md)
- [Governance](../GOVERNANCE.md)
- [Security](../SECURITY.md)
- [Support](../SUPPORT.md)
- [Community code of conduct](../CODE_OF_CONDUCT.md)
- [Trademark policy](../TRADEMARKS.md)
- [Property, parallel, and fuzz testing](./contributing/fuzzing.md)
- [Release process and checklist](./contributing/release-process.md)
- [Changelog](../CHANGELOG.md)
- [Apache-2.0 license](../LICENSE)

Each reference page's explicit status controls. Pages marked implemented describe behavior in the
current workspace; pages marked as a frozen contract remain non-integration specifications until
their listed implementation gates pass.
