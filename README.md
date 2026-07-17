# PliegoCSS

PliegoCSS is an experimental standards-first CSS compiler and verifier built in Rust for humans and
agents. Its product direction is normal CSS input and output plus compatibility policy, provenance,
tokens, accessibility guardrails, budgets, explainability, and deterministic receipts. The compact
Rust utility syntax implemented today is one optional typed frontend: it parses visible literals into
semantic IR so Rust can reject unknown tokens, invalid domains, contradictory variants, and style
conflicts during compilation.

The browser still receives static standards-compliant CSS. PliegoCSS does not replace the browser's
cascade, layout, or paint engines and does not ship a styling runtime in WebAssembly.

```rust
use pliego_css::{Style, pc};

fn button() -> Style {
    pc!(
        "inline-flex items-center gap-2 rounded-lg bg-accent px-4 py-2 \
         text-sm font-semibold text-white hover:bg-accent-strong \
         focus-visible:ring-2 disabled:opacity-50"
    )
}
```

## Current status

The workspace is unreleased at `0.0.0`; syntax and crate boundaries may still change.

- The [strategic product contract](./docs/product/strategic-product-contract-2026.md) supersedes the
  original "utility-first framework" category and defines the Phase 0 and R0 gates that now block
  `0.1.0`. Standard-CSS audit now has strict ingestion, bounded official compatibility decisions,
  artifact budget schema 1, verified Asset Plan file/layer aggregation, ownership-backed package and
  composed-route aggregation, and canonical audit
  findings/manifest/receipt publication for direct CSS or Asset Plan with drift-only `--check`, plus
  an opt-in receipt over the complete `bundle --control` output graph, and exact-snapshot
  compile/build/watch control groups with seven-artifact grouped publication. Generated control
  groups emit canonical Source Map v3 sidecars and `pliego.tokens.json`, integrity-bind both through
  the manifest/receipt, and measure actual registry-use coverage. The token artifact uses
  `pliegocss-token-graph/1`. The explicit DTCG surface for
  compile/build/check/inspect/watch selects contexts and publishes the complete resolver graph;
  its local test/package gates pass. Bundle-plan schema-2 selection has passed Windows/WSL E2E,
  full-workspace, and package gates. Cargo build-macro DTCG selection passes the full Debian WSL2
  workspace, Clippy, Rust 1.85, and downstream public-API gates; the post-change Windows package
  gate ran extracted DTCG and legacy-TOML consumers with identical output. Commit `9714b09` then
  passed the complete clean Debian WSL2 package gate with the same exact consumer output. Complete
  CSS classification, corpus precision/recall, and hosted evidence remain incomplete. Ownership
  schema 1 now has a green public parser/builder and explicit audit integration for exact Asset Plan
  binding, exclusive bundle/package ownership, and overlapping composed route views; the core tests,
  Asset Plan CLI E2E, and checked example replay pass locally in Debian WSL2. Audit findings have
  equivalent human, JSON, and SARIF 2.1.0 views.
- A bounded accessibility policy schema 1 / policy 1 is implemented for direct CSS and verified
  Asset Plans. It declares contrast pairs against literal colors or an optional canonical token
  graph and configures `fail|warn` independently for violation, unverified, and manual-required
  results across contrast, motion, focus visibility, forced colors, and input modality. The local
  development gate passed 30 control tests, nine CLI audit tests, one Asset Plan test, strict
  Clippy, and the dirty package gate. This is static policy evidence, not WCAG certification:
  browser/manual validation, diagnostic-corpus precision/recall, hosted runners, and native
  macOS/ARM64 evidence remain open.
- A bounded normal-CSS `explain-cascade` command now resolves direct author declarations for a
  closed longhand set when a simple HTML element, top-level named layers, importance, specificity,
  and source order are sufficient. Schema 1 preserves exact declaration spans and shorthand
  origins, reports specificity escalation, and returns `browser-required` with stable blockers for
  potentially matching dynamic/conditional/nested/scoped/imported/runtime constructs. This is a
  single-stylesheet static slice, not full browser cascade or computed-style emulation.
- `pliego-css-source` and `pliego-cssc` now emit canonical migration inventory schema 1 for closed,
  explicitly typed Sass, Tailwind CSS v4, CSS Modules, consumer, and auxiliary sets. The snapshot
  binds exact bytes/hash/spans, resolves only declared conservative edges, records template and
  CSS Modules usage seams (including one-level aliases and simple destructuring), and never executes
  Sass, JavaScript, plugins, configs, or imports. A bounded no-follow library crawler can derive an
  uncollected declaration from evidence-backed file roles, and the existing project-inventory CLI
  accepts that directory mode. A network-gated corpus pins Bootstrap, Tailwind's Vite playground,
  and Next.js basic CSS Modules with verified MIT bytes; its 119 reviewed files yield 104/104 exact
  file-role tuples with zero false positives or negatives. Bootstrap's 144 Sass import seams also
  resolve through bounded relative file/partial/index/import-only rules. Those results remain
  bounded: configured load paths/importers, broader syntax, migration outcomes, and representative
  scale are open, and no perfect-migration claim is made.
- `pliego-cssc plan`, `fix --dry-run`, and explicitly authorized `fix --apply` now implement the
  bounded agent-repair boundary. Closed
  schema-1.0.0 proposals accept only exact edits tied to verified, unexcepted, low-risk finding
  suggestions and enforce source-range containment, prerequisites, non-overlap, UTF-8, and explicit
  file/edit/byte budgets. Plans bind the exact FindingDocument, before/after source snapshots,
  structured edits, deterministic patch, and canonical payload hash; dry-run verification reports
  only complete `ready` or `already-applied` states. Apply requires the external exact-plan token,
  revalidates under a persistent cooperative lock, and publishes source bytes plus Change Receipt
  1.0.0 with staged rollback. The receipt truthfully reports `checks-pending`, `not-run`, and
  `not-collected`. A separate `pliego-css-agent verify` boundary now executes closed in-process
  `standard-css-audit`, `token-graph-integrity`, and identity-bound `css-budget-audit` checks, and
  validates `test-suite-evidence` from the explicit fixed-profile `run-tests` boundary and
  `browser-evidence` from the explicit pinned PliegoRS/Chromium `run-browser` boundary. Verification
  Receipt 1.4.0 derives `passed|failed|blocked` and publishes one complete adjacent
  after-FindingDocument per check; canonical 1.0.0/1.1.0/1.2.0/1.3.0 evidence remains readable under
  its original kind limits and no plan/policy can supply a command. Hosted signatures and the
  Firefox/WebKit browser matrix remain open.
- F0 and F1 are closed.
- Gate A is closed as a conditional GO over the matched core fixture: 30/30 deterministic fresh
  processes, a Chromium computed-style smoke, and documented build/payload measurements.
- Gate B is closed as a conditional GO over all 44 complete-fixture class attributes, including the
  tracked responsive and interactive variants.
- The typed theme registry, schema-1 TOML configuration, build-time `theme!` bridge, theme-scoped
  format-2 `StyleId`, multi-clause `pcx!`, deterministic emitter, Lightning CSS boundary,
  typed inline-size container queries, bounded configurable ARIA/data variants, native writing-mode
  utilities, typed fixed-order cascade layers, source-aware CLI,
  versioned Baseline compatibility policy,
  deterministic `pretty|minified` output, default
  schema-3 manifests, opt-in schema-4 semantic ownership graphs, opt-in schema-5 physical CSS
  traces, opt-in pruning of unreachable StyleId rule sets from an explicit reachability sidecar,
  framework-neutral schema-1 asset load plans, the closed ownership schema-1 core and audit seam,
  plus a portable source-to-output Project Index for
  bundle outputs, and an exact-snapshot watch loop
  with per-file Rust scan and theme-scoped semantic-IR caching are implemented. The CLI
  also exposes generated catalog JSON, style explanations, read-only formatting diagnostics,
  opt-in collision-safe Rust literal fixes, and a
  structured JSON error envelope as process-level editor contracts. Its read-only CSS audit accepts
  optional versioned budgets for canonical bytes, rules, selectors, specificity, and semantic
  duplication with explicit ownership, deltas, and reviewed exceptions. Declarative schema-1/schema-2
  bundle plans compile explicit source partitions from one snapshot. An initial standard stdio LSP
  transport now provides completion, hover, syntax/format diagnostics, and whole-literal formatting;
  an unreleased VS Code client packages that transport without embedded binaries. A real extension
  host gate, additional editors, Project Index navigation, and semantic diagnostic parity remain open.
- `pliego-css-config` implements the versioned DTCG 2025.10 format bridge, a bounded same-document
  Resolver profile, and the canonical token graph with aliases, derived values, deprecations,
  provenance, cycle rejection, and validated theme permutations. Direct CLI `--tokens` selection
  and bundle-plan schema-2 DTCG selection have passed their local gates. The Cargo build-macro
  bridge now selects the same Resolver context for `pc!`/`pcx!`; its local workspace, MSRV, public
  API, and clean-package gates pass. Accessibility policy schema 1 now declares contrast
  relationships and resolves token endpoints through an optional canonical graph; hosted
  portability evidence and the remaining R0 release gates keep R0.5 partial.
- The documented two-process development loop is locally certified against the pinned PliegoRS
  CLI: a `pc!` edit reached real Chromium with matching class and computed padding, while invalid
  input retained the last valid page and the tested unchanged CSS/manifest publication group did not
  advance reload. On Debian WSL2 with native Linux binaries and target directory, 20 edits measured
  232.4 ms CSS p50 / 284.5 ms p95 and 2.008 s site/SSE convergence p50 / 2.458 s p95. Chromium
  separately confirmed the resulting full-page reload; this is not CSS-only HMR or a browser-latency
  measurement.
- A local Rust 1.85 cross-project gate renders PliegoRS SSR markup and builds two deterministic SSG
  routes from five adapter-derived bundles under schema-5 tracing and unreachable-rule pruning. A
  validated PliegoRS product registry drives collection, automatic source partitioning, and SSG
  composition without inferring ownership from filenames. The gate verifies
  the compiler-produced Asset Plan and Project Index, exact source/CSS/manifest hashes, source-site
  backlinks, and final physical references; combines route and rendered-island selections; excludes
  a deliberately unreachable bundle from deployment; and checks class identity, route isolation,
  the PliegoRS build ledger, and one resumable-island SSR contract.
- PliegoCSS now accepts a strict framework-neutral reachability sidecar for component, route, and
  island provenance. `--prune-unreachable` can remove whole unreachable StyleId rule sets while
  retaining shared styles and complete provenance, but it does not prune theme variables or derive
  application ownership. `bundle --asset-plan` projects that explicit topology into separate route
  and island bundle selections with exact CSS/manifest integrity metadata; schema-5
  `--project-index` binds source sites to semantic declarations, tokens, components, and final CSS.
  `--usage-report` independently retains the complete pre-pruning bundle/StyleId universe and keeps
  static reachability, scoped positive observation, usage verdict, output disposition, and explicit
  retention policy separate. A reviewed `--retention` sidecar can preserve only exact structurally
  dead bundle StyleIds; Usage Analysis, Asset Plan, and Project Index schema 2 then share one
  `reachable-or-retained-style-ids` selection without rewriting the `dead` verdict.
  A local Chromium/CDP gate now proves that the pinned resumable island advances state without
  replacing its SSR document, island, button, or bound-text element. The same fixture explicitly
  preloads only its 560-byte shared theme bundle and proves that the applied stylesheet reuses one
  network request. Rustc dep-info now closes CSS-source discovery for the fixture's exact site-lib,
  site-ssg, and wasm-client targets; unbuilt feature/target permutations, a generalized measured
  preload policy, hosted multi-browser evidence, and activation of the selected API contract in a
  real `0.1.0` release remain open.
- A frozen [bundle portability vector](./docs/status/portability.md) covers CWD independence, Unicode/CRLF provenance, read-only
  checks, symlink/junction escape, and case-insensitive filesystem aliases. CI is configured for Ubuntu, Windows, and
  macOS on Rust 1.85/1.96 plus Node 22.13; hosted green-run evidence remains pending because this
  checkout has no configured remote.
- The publishable boundary candidate is now fifteen exact-version crates in six dependency waves.
  The independent cascade core sits in wave 1 and the bounded repair core in wave 5 so neither
  pushes the already-tight build or CLI archives over the fixed 61,440-byte ceiling. The latest
  exact clean `0905ded` replay packaged and compiled all fifteen extracted archives and passed its
  registry-shaped Rust 1.85 consumer, including the new direct agent dependency:
  `pliego-css-agent` was 18,639 bytes, `pliego-css-build` 59,907 bytes, `pliego-cssc` 61,132 bytes,
  and `pliego-css-control` 61,071 bytes. Nothing has been uploaded, and repository/registry
  ownership gates remain open.
- Clean-commit local snapshots freeze Gate A, Gate B, and paired Rust-check evidence for `c47239c`;
  hosted multi-OS performance evidence remains open.

These gates validate continued development; they are not production-readiness claims. See the
[execution plan](./EXECUTION_PLAN_ULTRA.md), [roadmap](./ROADMAP.md), and
[documentation index](./docs/index.md). Release changes and the non-publishing checklist live in
[`CHANGELOG.md`](./CHANGELOG.md) and the [release process](./docs/contributing/release-process.md).

The exact compiler-owned utility/token surface is generated by `pliego-cssc catalog` and versioned
in [`generated-catalog.md`](./docs/reference/generated-catalog.md); CI rejects documentation drift.
Persisted identity, class, theme-binary, schema, and portable-bundle vectors are tracked by the
[compatibility candidate contract](./docs/reference/compatibility.md). They prevent accidental drift
without claiming that the current `0.0.0` Rust API is already a public SemVer freeze.
The exact tagged StyleId byte stream, SHA-256 truncation, class relationship, and migration boundary
are specified in the [StyleId format-2 reference](./docs/reference/style-id-format-v2.md).
The deliberately small application/build surface selected for `0.1.x` is documented and exercised
by the downstream [public API candidate contract](./docs/reference/public-api.md).

## Requirements

Rust 1.85 is the minimum supported Rust version. This repository pins Rust 1.96 for contributor
tooling. The Rust crates do not require Node.js. Node.js 22.13 or newer and pnpm are used only by
this repository's verification, packaging, benchmark, and integration harnesses.

The crates are not published yet, so use local path dependencies:

```toml
[dependencies]
pliego-css = { path = "../PliegoCSS/crates/pliego-css" }
```

The compile-checked seed example lives in [`examples/basic`](./examples/basic). Run it with:

```console
cargo run -p pliego-css-basic-example
```

## Build CSS

`pliego-cssc` can consume explicit styles, line-oriented input files, or visible `pc!`/`pcx!`
literals from explicitly supplied Rust source files:

```console
cargo run -p pliego-cssc -- compile --source examples/basic/src --seed --theme --output pliego.css --manifest pliego.manifest.json
```

Applications with a framework-produced reachability sidecar can opt into manifest schema 4 without
changing CSS bytes:

```console
pliego-cssc compile --source src --seed \
  --output dist/pliego.css --manifest dist/pliego.manifest.json \
  --manifest-version 4 --reachability pliego.reachability.json
```

The graph uses exact source ranges and fails rather than inferring missing ownership. When a trusted
consumer also needs an exact trace into the final Lightning CSS artifact, select schema 5:

```console
pliego-cssc compile --source src --seed \
  --output dist/pliego.css --manifest dist/pliego.manifest.json \
  --manifest-version 5 --reachability pliego.reachability.json
```

Schema 5 adds graph schema 2 with exact UTF-8 ranges for physical rules and declarations,
many-to-many semantic contributions, generated-output markers, and complete fail-closed coverage.
With the same pruning setting it remains byte-identical to schema 4; without
`--prune-unreachable`, schemas 3, 4, and 5 retain their established identical CSS bytes.

To emit only StyleIds with at least one exact origin owned by a route- or island-reachable
component, add the explicit pruning flag:

```console
pliego-cssc compile --source src --seed \
  --output dist/pliego.css --manifest dist/pliego.manifest.json \
  --manifest-version 5 --reachability pliego.reachability.json \
  --prune-unreachable
```

The compiler still validates every compiled origin against the sidecar. A retained StyleId keeps
all of its origins, including origins owned by components outside the root set. `--theme` continues
to emit the registry's supported custom properties in full; this flag does not claim token-CSS
pruning. See [manifest schema 4](./docs/reference/manifest-schema-4.md),
[manifest schema 5](./docs/reference/manifest-schema-5.md), and
[reachability schema 1](./docs/reference/reachability-schema.md). The
[controlled pruning benchmark](./docs/benchmarks/reachability-pruning.md) freezes one synthetic
raw/gzip result without extrapolating it to production applications.

`build` is an alias for `compile`. The current scanner parses Rust with `syn`; `--source` accepts a
`.rs` file or recursively scans a directory. Directory traversal is deterministic but is not Cargo
reachability analysis: every discovered Rust file is in scope. Direct `--style`, `--compose`, and
`--input` remain available. `--config`, `--seed`, and `--tokens` are mutually exclusive. When none
is supplied, the CLI discovers a unique `pliego.theme.toml` from the working directory and supplied
input/source package roots; JSON is never discovered. Multiple TOML matches are rejected instead of
choosing silently.

The compile/build/check/inspect/watch DTCG surface is explicit:

```console
pliego-cssc compile --style "flex bg-brand" --tokens examples/product.resolver.json \
  --token-input appearance=dark --theme --output pliego.css
```

`--tokens` conflicts with `--config`/`--seed`; each repeated `--token-input modifier=context`
requires it, and JSON is never auto-discovered. Defaults and case-insensitive spellings converge on
canonical selections; duplicate or unknown inputs fail. Controlled output publishes every validated
permutation, and `configHash` binds the canonical selections separately from `ThemeId`. For a Cargo
or PliegoRS package, configure `theme!(tokens = ..., inputs = {...})` in `build.rs` and repeat the
same Resolver and inputs as CLI `--tokens`/`--token-input`; this keeps macro-generated identities and
CSS on the same selected registry. The build artifact contains only that selected registry, while a
controlled CLI build publishes the complete Resolver graph. Bundle schema 2 remains the plan-owned
equivalent for one-shot assets.

Other implemented commands:

```console
cargo run -p pliego-cssc -- check --source examples/basic/src --seed
cargo run -p pliego-cssc -- inspect --source examples/basic/src --seed
cargo run -p pliego-cssc -- watch --input benchmarks/pliego-gate-a/styles.txt --output pliego.css --seed --theme
```

To publish CSS, its canonical Source Map v3 sidecar, specialized manifest, canonical token graph,
canonical findings, control manifest, and receipt
from one exact source/config snapshot, place both generated outputs under one existing control root:

```console
mkdir target/readme-control
cargo run -p pliego-cssc -- compile \
  --source examples/basic/src --seed --theme \
  --output target/readme-control/app.css \
  --manifest target/readme-control/app.manifest.json \
  --control-dir target/readme-control
```

Repeat the same command with `--check` for read-only drift detection across all seven artifacts.
`watch` accepts the same `--control-dir` contract without `--check` and retains the last complete
valid group after an invalid snapshot. The map is emitted as `app.css.map`; the graph uses the fixed
`pliego.tokens.json` filename. Both are discovered and verified through the control manifest. CSS
does not receive a `sourceMappingURL` comment. See the [CLI reference](./docs/reference/cli.md),
[source-map contract](./docs/reference/css-source-maps.md), and
[token-graph contract](./docs/reference/token-graph-schema-1.md).

For several explicitly owned assets, a bundle plan compiles every named CSS/manifest pair before
publishing the output group. Frozen schema 1 accepts seed/config themes; schema 2 retains them and
adds an explicit DTCG Resolver. Per-bundle manifests default to schema 3 and accept the same
schema-4/schema-5 sidecar and pruning flags:

```console
mkdir target/readme-bundles
cargo run -p pliego-cssc -- bundle --plan pliego.bundles.toml --output-dir target/readme-bundles
```

Schema 2 keeps DTCG selection inside the plan—there are no bundle `--tokens` flags:

```toml
schema = 2
targets = "modern"
format = "minified"

[theme]
kind = "dtcg-resolver"
path = "examples/product.resolver.json"

[theme.inputs]
appearance = "dark"

[bundles.application]
sources = ["src/styles.rs"]
emit-theme = true
```

The Resolver bytes are recorded as `token-resolver`, controlled output carries the complete graph,
and exact plan bytes bind the selection to `configHash`. Textually different plans remain distinct
snapshots even if they resolve to the same ThemeId/CSS. Invalid schema, Resolver, or selection input
fails before publication. The local Windows/WSL E2E, workspace, Rust 1.85, and package gates pass;
hosted/macOS evidence remains a release boundary.

With manifest schema 4 or 5 plus reachability, the boolean `--asset-plan` option adds the fixed
`target/readme-bundles/pliego.assets.json` output to the same check/publication group:

First generate `target/pliego.reachability.json` from the adapter's typed
`ApplicationTopology::collect(...)`; the PliegoRS fixture collector is the executable example and
also generates its bundle plan from one validated `ProductRegistry`. The following generic command
consumes explicit plan and reachability bytes:

```console
cargo run -p pliego-cssc -- bundle \
  --plan pliego.bundles.toml \
  --output-dir target/readme-bundles \
  --manifest-version 5 \
  --reachability target/pliego.reachability.json \
  --prune-unreachable \
  --asset-plan \
  --project-index \
  --usage-report \
  --control
```

The load plan records `all-compiled`, `reachable-style-ids`, or the schema-2
`reachable-or-retained-style-ids` mode, exact CSS/manifest byte counts and SHA-256 hashes, and
separate route/island selections. `pliego.index.json` adds exact source document
hashes and source-site links through StyleId, semantic declarations, tokens, components, and
bundle-qualified physical declarations. `--control` adds one shared `pliego.tokens.json`, canonical
findings, control manifest, and receipt to that same rollback-capable group; its manifest hashes
every output and relates CSS/style manifests to the token graph, the Asset Plan to all pairs, and the
Project Index to the Asset Plan. `--check`
recomputes the entire group without writing. `pliego.usage.json` independently retains the complete
pre-pruning `(bundleId, StyleId)` universe and distinguishes static reachability, optional scoped
observation, `observed|unobserved|dead|unknown` usage, and report/pruning disposition. A reachable
style is not relabeled as observed, and absence in sampled observation never proves deadness. This
report can also bind a non-empty `--retention pliego.retention.json` sidecar during explicit
pruning. Retained entries stay `unreachable + dead`, are emitted whole only in their named bundle,
and record `policy-retained`; all other dead entries remain removed. The three selected artifacts
upgrade together to schema 2, and the exact policy enters controlled `configHash` as
`usage-retention`. The compiler command remains deterministic explicit partitioning: it does not
rewrite a supplied plan, inspect a product registry, generate URLs, or select preload policy. The
PliegoRS adapter now generates both reachability and an explicit plan from its product registry
before invoking that framework-neutral command; it does not claim exhaustive discovery of
unregistered Cargo modules. See the
[bundle-plan reference](./docs/reference/bundle-plan.md) and
[asset load-plan schema](./docs/reference/asset-plan-schema.md), plus
[Project Index schemas 1 and 2](./docs/reference/project-index-schema.md),
[usage analysis schemas 1 and 2](./docs/reference/usage-analysis-schema-1.md), and the
[retention sidecar](./docs/reference/usage-retention-schema-1.md).

Ownership schema 1 binds one exact Asset Plan to total/exclusive package ownership and total
route-to-island composition. It resolves package bundle groups and composed route views in canonical
plan order; route views may overlap and are not Control Manifest partitions. The implemented audit
seam is explicit and never discovered:

```console
cargo run -p pliego-cssc -- audit \
  --asset-plan target/readme-bundles/pliego.assets.json \
  --ownership examples/ownership/pliego.ownership.json \
  --targets none
```

The 12 public core contract tests and the Asset Plan ownership CLI E2E are green, and the command
above passed against the checked example in Debian WSL2. Add an explicit `--budget-policy` to enforce
its typed package/composed-route subjects. This is local evidence, not a hosted or release claim.
See [ownership schema 1](./docs/reference/ownership-schema-1.md).

The default target contract remains `modern`: Chrome 111, Edge 111, Firefox 128, and Safari 16.4.
New applications can select strict `--targets baseline-widely`, frozen at Chrome/Edge 120, Firefox
121, and Safari/iOS Safari 17.2. That profile rejects unclassified arbitrary CSS instead of silently
claiming compatibility. Pass `--targets none` to hand browser lowering explicitly to a downstream
pipeline. Inspect the exact schema-1 decision artifact with
`pliego-cssc compatibility --targets baseline-widely`. See the
[compatibility policy](./docs/reference/compatibility-policy.md) and
[CLI reference](./docs/reference/cli.md).

## Custom themes

Custom tokens and breakpoints are configured at build time. Add the bridge:

```toml
[dependencies]
pliego-css = { path = "../PliegoCSS/crates/pliego-css" }

[build-dependencies]
pliego-css-build = { path = "../PliegoCSS/crates/pliego-css-build" }
```

Create `build.rs`:

```rust,no_run
fn main() {
    pliego_css_build::theme!("pliego.theme.toml");
}
```

Or select one DTCG Resolver permutation:

```rust,no_run
fn main() {
    pliego_css_build::theme!(
        tokens = "product.resolver.json",
        inputs = {
            "appearance" => "dark",
        },
    );
}
```

Use `inputs = {}` for every declared Resolver default. Resolver paths are relative to the package's
`CARGO_MANIFEST_DIR`; input names and contexts must be string literals, and duplicate modifiers
including case-fold collisions fail closed. Call `theme!` once per package, then invoke
`pliego-cssc` with the same Resolver and inputs when producing CSS.

For the TOML form, define a schema-1 theme:

```toml
schema = 1
extends = "seed"

[tokens.color]
brand = "oklch(56% 0.18 255)"

[tokens.spacing]
gutter = "1.75rem"

[breakpoints]
tablet = "52rem"
```

Compile the custom-theme example and its matching CSS with the same configuration:

```console
cargo check -p pliego-css-custom-theme-example
cargo run -p pliego-cssc -- compile --source examples/custom-theme/src --config examples/custom-theme/pliego.theme.toml --theme --output custom-theme.css
```

The TOML or Resolver parser runs only during the build. A canonical
`pliego-css-theme-v1-{theme_id}.bin` registry is written under Cargo's `OUT_DIR`, and the
application runtime retains only `StyleId`. Changing the registry changes `ThemeId` and therefore
the generated class identities. A DTCG build writes only the selected registry to this artifact;
complete graph publication remains a controlled CLI responsibility.

Read [Installation](./docs/getting-started/installation.md),
[Themes and tokens](./docs/learn/themes-and-tokens.md), and the exact
[theme schema](./docs/reference/theme-schema.md).

## Verification and benchmarks

```console
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm install
pnpm baseline:measure
pnpm baseline:measure-pliego
pnpm baseline:measure-pliego-b
pnpm baseline:measure-media
pnpm baseline:measure-rust
pnpm baseline:measure-wasm
pnpm baseline:check-diagnostics
pnpm check:api
pnpm check:properties
pnpm check:determinism
pnpm check:manifest
pnpm check:trace
pnpm check:media
pnpm check:pruning
pnpm check:fuzz
pnpm check:evidence
pnpm check:portability
pnpm check:cascade
pnpm check:agent
pnpm check:packages
pnpm integration:plain-html
pnpm integration:pliegors
pnpm integration:pliegors-browser
pnpm integration:pliegors-dev
```

`pnpm check:fuzz` additionally requires `nightly-2026-06-26`, `rust-src`, and cargo-fuzz 0.13.2.
The exact setup, the parallel-lock helper overrides, and restricted-Windows workarounds are in the
[property, parallel, and fuzz testing guide](./docs/contributing/fuzzing.md).

The benchmark reports record reset-contract limitations and raw/gzip tradeoffs; do not reduce them
to a universal "faster and smaller" claim. Start with the
[Gate A report](./docs/benchmarks/pliego-gate-a.md) and
[Gate B report](./docs/benchmarks/pliego-gate-b.md) and the targeted
[media-query merge](./docs/benchmarks/media-query-merging.md) and
[reachability-pruning](./docs/benchmarks/reachability-pruning.md) reports. Exact Gate A, Gate B, and
Rust-check machine JSON is indexed under [`benchmarks/evidence`](./benchmarks/evidence/README.md);
the targeted change gates print their deterministic JSON directly. The PliegoRS integration commands
expect `PliegoCSS` and `pliegors` to be sibling directories, unless `PLIEGORS_ROOT` names a clean
checkout. The SSG gate covers server-rendered `view!`/builder markup, deterministic assets,
schema-5 pruning, asset-plan-driven route/island bundle selection, deliberate dead-bundle exclusion,
and resumability markup. The browser gate regenerates that site, runs the product island event in
local Chromium, and requires document/island/control/value object identity, state 15→20, stable
classes, one matching state event, one shared-CSS preload reused by its stylesheet request, and a
real Rust/WASM client startup with exactly one module/WASM load, and a clean console. The bounded
fixture currently measures 31,423 raw / 12,584 gzip-9 WASM bytes; this is not a production budget.
The development gate composes both watchers and requires 20 coherent,
  single-generation edits plus closed negative cases. The PliegoRS adapter automatically derives its
  registered application reachability and source partition. Rustc dep-info now proves every macro
  site across the exact site-lib/site-ssg/wasm-client source graph is owned; unsupported Cargo
  feature/target permutations remain unclaimed. Schema-4/schema-5 manifests
  can import the generated reachability sidecar and the asset plan can project verified ownership
  without claiming universal preload policy or token-variable pruning.

`pnpm integration:plain-html` is independent of the sibling PliegoRS checkout. It compiles explicit
line-oriented utility inputs, binds exact manifest origins to declared template slots, and checks the
rendered external CSS in local Chrome. The page has no scripts or WASM styling runtime. This proves
the bounded [plain HTML contract](./docs/integrations/plain-html.md), not dynamic JavaScript
reachability or a published Vite/Astro adapter.

## License

Apache-2.0.
