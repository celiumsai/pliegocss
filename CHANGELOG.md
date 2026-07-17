# Changelog

All notable user-facing changes to PliegoCSS are recorded here. The project is still unreleased at
`0.0.0`; entries under **Unreleased** describe the candidate workspace, not a published stability
promise.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and releases will use
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) together with PliegoCSS's stricter
pre-1.0 compatibility policy.

## [Unreleased]

### Added

- An initial `pliego-css-lsp` stdio server implementing a bounded LSP 3.18 subset for full Rust
  document synchronization, UTF-16 positions, scanner/parser/format diagnostics, catalog-backed
  completion, explain-backed hover, and collision-safe whole-literal formatting edits. Cursor
  features fail closed for cooked Rust strings with escapes. Optional Project Index schema 1/2
  navigation verifies the source snapshot, Asset Plan, manifest/CSS integrity, and schema-5 physical
  declaration ranges before returning source-to-final-CSS definition links. Bounded per-literal
  semantic diagnostics now reuse `pliego-cssc check` schema 1 and project exact compiler ranges into
  UTF-16. A 150 ms per-document debounce moves compiler work off the protocol loop and rejects stale
  results by document version. A per-URI version registry now forcefully terminates and reaps the
  direct compiler child when its snapshot becomes stale, while concurrent stdout/stderr readers
  prevent bounded process pipes from blocking cancellation. Multi-clause `pcx!` buffers are
  projected to one bounded literal-only synthetic Rust source so same-version `check --source` owns
  `PCX003` semantics; findings map back to the exact original branch token. Current CLI/LSP
  diagnostic families have exact parity through corpus schema 2 and operational fault injection.
  The exclusive `pliego-css-lsp --version` process option lets editor setup verify the installed
  native server without entering stdio mode.
- An unreleased VS Code client candidate that connects file-backed Rust documents to explicit
  external `pliego-css-lsp` and `pliego-cssc` binaries. It supports discover/seed/config theme
  modes, optional Project Index navigation, fail-closed machine-overridable paths, configuration
  restart, and a bounded VSIX package gate without embedding or downloading native executables. A
  pinned VS Code 1.105.1 extension-host gate opens a real Rust document, follows the physical CSS
  definition, edits the buffer, and observes exact compiler-backed `PCS001` and cross-clause
  `PCX003` diagnostics.
- An unreleased 3,466-byte Neovim client candidate using only the editor's built-in LSP API and
  explicit external binaries. Its closed Lua/README payload has no download surface. Pinned official
  Neovim 0.12.4 Windows and Linux hosts verify Project Index definition plus exact `PCS001` and
  cross-clause `PCX003` diagnostics; downloaded editor archives are hash-verified gate fixtures,
  not client behavior.
- LSP diagnostic corpus schema 2 with twenty frozen parser, semantic, scanner, Rust-parse, and
  formatting cases covering PCS001–PCS012, PSC001–PSC006, PCR001, and FMT001. The same process gate
  covers cross-clause PCX003 and fault-injected PCL001/PCL002. Windows and Debian require exact
  code/message/range/severity/suggestion/replacement equality with CLI diagnostic schema 1. Parser
  failures now project precise, including zero-width, spans into UTF-16 when source mapping is exact.
  Rust parse failures are typed as PCR001 in both CLI and LSP, and scanner codes survive context
  prepended by `syn`.
- Bounded typed migration discovery through `discover_migration_project`. The library walks one
  project-relative root in canonical order, ignores only `.git`/`node_modules`/`target`, rejects
  link-like or non-regular paths, caps traversal at 32 levels, 65,536 entries, and 256 MiB of
  candidate files, and selects only
  evidence-backed Sass, CSS Modules, Tailwind, consumer, template, config, plugin, and exact-source
  roles. `migration-project-inventory DIRECTORY` exposes the same path through the existing CLI and
  performs normal two-pass collection; configured load paths/importers and broader semantic
  precision/recall remain open.
- File-qualified migration discovery and collection failures, so malformed source, consumer, or
  auxiliary syntax identifies the exact project-relative candidate instead of returning only the
  underlying lexical error.
- Deterministic Sass-relative resolution for extensionless files, partials, index files, and legacy
  import-only files. Relative lookup does not require `./`, import-only candidates precede normal
  candidates for `@import`, and ambiguous declared candidates fail closed. The pinned Bootstrap
  corpus now resolves all 144 observed Sass dependencies without load paths or importer execution.
- A network-gated reviewed public migration corpus with exact MIT license digests and immutable
  revision pins for Bootstrap Sass, Tailwind's Vite playground, and Next.js basic CSS Modules. Its
  Debian WSL2 gate compares emitted file-role tuples against 119 reviewed files: 104 true positives,
  zero false positives, and zero false negatives. The metric is explicitly limited to file-role
  discovery and is not a semantic migration, codemod, or general project-accuracy claim.

- A versioned three-case authored migration bridge corpus and read-only CLI runner covering resolved
  Sass modules, Tailwind-owned auxiliaries/template candidates/config-plugin seams, and CSS Modules
  consumer usage. The runner verifies selected summary values, required edges, clean process output,
  and fixture immutability. It is a contract corpus, not real-project precision/recall evidence.

- Read-only `pliego-cssc migration-inventory` for one explicit Sass, Tailwind CSS v4 entry, or CSS
  Modules source. The command emits canonical schema-1 JSON only to stdout and rejects unsafe paths,
  kind/extension mismatch, symlink-like inputs, malformed lexical state, and bounded-limit
  violations; it never executes the original source toolchain or claims transformation support.
- Explicit `MigrationProject` snapshots for up to 4,096 declared typed sources. Collection rejects
  duplicates and unsafe files, uses canonical path/kind order, inventories the complete set twice,
  and emits one nested schema-1 document only when both passes agree. Canonical dependency
  observations cover Sass module/import seams, CSS import/reference, and CSS Modules/ICSS
  composition; exact supported local targets fail closed unless declared with the expected kind,
  while external, unresolved, local, and dynamic edges remain explicit. The read-only
  `pliego-cssc migration-project-inventory` command loads a closed declaration and emits the
  canonical project snapshot only to stdout. Configured load paths/importers, transitive dependency
  crawling, broader toolchain semantics, and migration outcomes remain open.
- Bounded closed schema-1 JSON declarations through `MigrationProject::from_json`, allowing the
  typed source set to be reviewed and checked into a project before canonical collection. Unknown
  fields, unsupported versions, unsafe/kind-incompatible paths, oversized documents, and source
  count overflow fail before any source read. `MigrationProject::from_file` adds bounded regular-file
  loading with symlink/reparse-point rejection.
- A versioned cross-toolchain migration-project fixture covering Sass, Tailwind CSS v4, and CSS
  Modules together, with exact assertions for resolved, local, external, unresolved, and unsupported
  seams through the public CLI.
- Project dependency observations for Tailwind `@config`, `@plugin`, and `@source`. Package-owned
  plugins are external, declared relative config/plugin/exact-template paths resolve by auxiliary
  kind, undeclared or glob/directory discovery remains unresolved, and inline sources remain
  dynamic; PliegoCSS records these seams without executing or pretending to understand their
  JavaScript/toolchain semantics.
- Declared CSS Modules consumers for JavaScript and TypeScript module extensions. Project snapshots
  bind default/namespace ESM, TypeScript import-equals, and `const|let|var` CommonJS
  `.module.css` imports to declared CSS Modules sources, retain exact dot and
  quoted-bracket class usage, classify computed/binding-escape/template usage as dynamic, and reject
  missing or mistyped local targets without executing JavaScript, TypeScript, or bundlers.
- Explicit one-level CSS Modules binding aliases for `const|let|var alias = binding`. The alias
  declaration has its own typed observation and subsequent dot/quoted-bracket usages inherit the
  exact source target without turning the declaration into a false dynamic class usage.
- Typed CSS Modules destructuring observations for exact shorthand and renamed members. Computed,
  rest, default, nested, empty, and otherwise ambiguous patterns emit one fail-closed dynamic seam
  for the complete declaration; no partial static classes or duplicate binding usages are invented.
- Multi-role migration declarations, allowing one physical JS/TS component to be inventoried as
  both a CSS Modules consumer and Tailwind template. Duplicates remain closed within each role and
  every role entry counts toward the existing 4,096-entry bound.
- Explicit Tailwind config, plugin, and template auxiliaries in migration project declarations.
  Every auxiliary is read twice through the bounded no-link file boundary and retained by exact
  bytes/SHA-256. Relative `@config`, `@plugin`, and exact-file `@source` seams resolve only to the
  declared auxiliary kind; glob/directory discovery remains unresolved. Configuration and plugins
  are lexically validated but never executed.
- Conservative Tailwind template observations for literal `class`/`className` attributes inside
  markup tags. Exact whitespace-separated candidates retain byte spans; expressions and
  interpolation remain one dynamic observation. HTML comments, unrelated attribute strings, and
  text outside tags do not create candidates.
- Closed lexical seams for common Tailwind config keys and plugin registration APIs. Exact
  identifier spans are retained as unsupported observations only when followed by the required
  object-key `:` or call `(` delimiter in code; comments and strings are excluded.
- Closed post-change verification through the dedicated `pliego-css-agent verify` executable,
  repair-check policy and Verification Receipt schemas 1.2.0 with canonical 1.0.0/1.1.0 read
  support. Built-in `standard-css-audit`, `token-graph-integrity`, and `css-budget-audit` kinds
  revalidate every changed source under the repair lock, execute normal CSS/token-graph/budget
  validation in process, and derive `passed|failed|blocked`. Budget checks bind exact canonical
  policy bytes plus optional package/route subjects; supplementary inputs are re-read under the lock.
  Policy and plan content have no program, arguments, shell, or package-script surface;
  browser-required policy is honestly blocked until evidence exists. Each check publishes its
  complete canonical `FindingDocument` beside the receipt through create-if-absent staging; the
  receipt is linked last and failures roll back newly linked evidence.
- Explicitly authorized bounded repair application through `pliego-cssc fix --apply`. Apply requires
  `--authorize sha256:<planSha256>` plus a project-relative receipt destination, revalidates exact
  source bytes under a persistent cooperative lock, and publishes source transitions with Change
  Receipt schema 1.0.0 as one staged, rollback-capable group. Repeated same-plan application is a
  no-op that preserves the existing receipt. The authorization is an exact-plan selection guard, not
  identity authentication; the change-only receipt records `checks-pending`, required checks as
  `not-run`, and browser evidence as `not-collected` rather than claiming verification.
- Closed repair proposal, plan, and dry-run schemas 1.0.0 through the new `pliego-css-agent` crate,
  `pliego-cssc plan`, and `pliego-cssc fix --dry-run`. The boundary accepts only exact UTF-8 edits
  linked to verified, unexcepted, low-risk findings; enforces range containment, prerequisites,
  non-overlap, portable paths, and explicit change budgets; and binds exact FindingDocument/source
  snapshots, before/after hashes, a deterministic byte-edit patch, and the plan payload hash.
  Dry-run verification is idempotence-aware and deliberately performs no mutation, authorization,
  required-check execution, Change Receipt publication, or browser claim.
- Bounded normal-CSS cascade explanation through `pliego-cssc explain-cascade` and the dedicated
  `pliego-css-cascade` core. Schema 1 resolves a closed longhand set for one simple HTML element and
  author stylesheet using top-level named layers, importance, specificity, and source order; it
  preserves exact declaration spans/shorthand origins and returns stable `browser-required`
  blockers instead of guessing across dynamic or unsupported context. Full browser/computed-style
  cascade remains outside this slice.
- Explicit stylesheet preload support across the pinned PliegoRS SSG seam. `Head` accepts only
  unique preload URLs that also name an applied stylesheet and emits them before scripts/styles.
  The PliegoCSS fixture selects exactly one theme-bearing shared bundle (`shared.css`, 560 bytes)
  per route; local Chromium observes one preload element, one applied stylesheet, and one network
  request. This is a bounded delivery contract, not a general performance-improvement claim.
- A reproducible local Chromium/CDP gate for the pinned PliegoRS SSG fixture. The gate generates and
  serves the site, executes the resumable `visit-counter` event, and proves that the same document,
  island, button, and bound-text DOM objects survive while state advances from 15 to 20. It also
  requires stable PliegoCSS classes, one typed state event, and no browser/server errors. Hosted and
  multi-browser evidence remain release work.
- A bounded framework-neutral typed application collector in `pliego-css-source`. Adapters provide
  Rust roots plus component/route/island topology and explicit source-unit or exact-site ownership;
  collection rejects unowned `pc!`/`pcx!` invocations, stale sites, unsafe paths, symlinks, and
  defensive limit violations before emitting canonical reachability schema 1. The PliegoRS smoke
  now generates its sidecar, pins the current clean framework contract, and publishes SSG through
  verified `pliego build` / Build Report 2.0.0.
- A validated PliegoRS `ProductRegistry` for components, routes, islands, route-to-island occurrence,
  and declaration-site Rust source capture. The fixture adapter derives canonical reachability and
  `shared`/route/island/unreachable bundle-plan partitions from one registry snapshot and verifies
  both generated outputs twice before compilation. This closes the duplicated manual topology/plan
  seam without claiming exhaustive discovery of unregistered Cargo modules.
- Accessibility Policy schema 1 for direct CSS and byte-regenerated Asset Plans, with declared
  token contrast pairs, conservative reduced-motion/focus/forced-colors/input-modality analysis,
  verified/unverified/manual-required findings, evidence-bound exceptions, exact policy/graph
  ledger inputs, and deterministic human/JSON/SARIF parity. The local 30-test control suite,
  workspace, Clippy, Rust 1.85, public-API, and exact-commit package gates pass; this is not WCAG
  certification and still requires browser/manual and hosted evidence.
- A closed standards-provenance manifest and offline attribution gate for the frozen compatibility
  datasets, Lightning CSS backend, Tailwind benchmark baseline, DTCG reports, and accessibility CSS
  specifications. Exact versions, locks, evidence hashes, licenses/notices, CI triggers, and release
  checklist integration are verified without treating the manifest as an SBOM.
- The publishable `pliego-css-usage` crate and opt-in `bundle --usage-report` over the complete
  pre-pruning `(bundleId, StyleId)` universe. Closed usage-analysis schemas 1/2 keep static
  reachability, positive observation, derived usage, removal disposition, and policy separate;
  exact origins remain explainable after pruning, sampled absence never proves deadness, and stale
  or contradictory evidence fails before grouped publication. The evidence-aware verifier
  rederives reports from exact compiler origins and sidecar bytes; aggregate labels are bounded,
  valid multiline utility sources are preserved, origin collection is logarithmic, and the narrow
  `usage-artifacts` feature avoids pulling Lightning CSS into adapter tooling.
- Explicit `bundle --retention FILE` for reviewed structurally dead StyleIds outside application
  topology. The closed schema-1 sidecar binds exact universe/reachability bytes and selects only the
  reachable union plus bundle-qualified grants. Retained entries stay `dead` and become
  `policy-retained`; Usage Analysis, Asset Plan, and Project Index schema 2 plus ownership parsing
  agree on `reachable-or-retained-style-ids`, while no-retention schema-1 bytes remain stable.
- Package-boundary compaction moved reusable Lightning CSS optimization and catalog rendering into
  `pliego-css-build`, removed the CLI's direct backend dependency, and expanded the exact-version
  boundary to thirteen crates without raising the fixed 60 KiB compressed archive ceiling. The
  complete dirty-explicit replay and the exact clean commit `8f83036` both pass extracted-archive,
  all-feature release compilation, and Rust 1.85 downstream gates; no registry upload occurred.
- Explicit DTCG Resolver selection for CLI compile/build/check/inspect/watch through
  `--tokens FILE` and repeatable `--token-input modifier=context`: no JSON discovery, strict
  config/seed conflicts, canonical default/case-fold convergence, duplicate/unknown-input failure,
  complete graph publication, and canonical-selection `configHash` binding. Local CLI, watch,
  package, and frozen direct-Resolver Windows/Linux portability vectors pass. Bundle-plan schema-2
  integration also passes its Windows/WSL E2E, full-workspace, and package gates.
- Cargo `theme!` DTCG selection with
  `theme!(tokens = "product.resolver.json", inputs = { "appearance" => "dark" })`, preserving the
  existing TOML form. Relative Resolver paths use `CARGO_MANIFEST_DIR`; empty inputs select defaults;
  literal ordered entries preserve duplicate and case-fold collision rejection; and exactly the
  selected registry is encoded for `pc!`/`pcx!`. Applications must repeat the same Resolver and
  inputs in the CLI so CSS identity agrees; controlled CLI output remains the producer of the
  complete token graph. The full Debian WSL2 workspace, Clippy, Rust 1.85, and downstream public-API
  gates pass. The post-change Windows package gate ran the extracted DTCG and legacy-TOML consumers
  with identical output; commit `9714b09` passed the complete clean Debian WSL2 package gate with
  the same exact output and every archive below the fixed ceiling.
- Additive bundle-plan schema 2 for an explicit `[theme] kind = "dtcg-resolver"`, relative `path`,
  and optional `[theme.inputs]`, while schema 1 stays frozen to seed/config. The design adds no
  bundle flags, reads one bounded Resolver for all bundles, fails closed before publication, ledgers
  exact Resolver bytes as `token-resolver`, publishes the complete graph, and binds exact plan bytes
  to `configHash`. Schema compatibility, rollback, read-only check, Windows/WSL E2E, full-workspace,
  Rust 1.85, and package gates pass.
- Canonical Source Map v3 sidecars for controlled compile/build/watch and `bundle --control`.
  Rule-level segments map final class selectors to exact authored snapshots with UTF-16 positions;
  CSS/map relationships, bytes, and hashes fail closed in the control manifest and receipt. Inline
  CLI styles use a deterministic virtual source, multi-origin styles retain complete provenance in
  their specialized manifest, and ordinary CSS bytes remain unchanged.
- CSS budget policy schema 1 and optional `audit --budget-policy`, with canonicalized file/package/
  route/layer attribution; canonical minified bytes, recursive rules, selectors, exact specificity,
  and context-aware declaration-duplication metrics; absolute and baseline-delta limits; reviewed
  exceptions; policy hashes; exact findings; and total fail-closed policy coverage when any declared
  subject remains unobserved.
- Closed ownership schema 1 plus the public `pliego-css-ownership` parser/builder, binding one exact
  Asset Plan byte length and SHA-256 to total/exclusive bundle-to-package ownership and total
  route-to-island composition. Explicit `audit --asset-plan PLAN --ownership FILE` derives package
  budgets and stable-deduplicated base-plus-island route views; Asset Plan mode rejects competing
  manual `--budget-subject` assertions, never discovers the sidecar, and never projects overlapping
  routes as Control Manifest partitions. This milestone expanded the publishable boundary to twelve
  exact-version crates in six dependency waves; its dirty replay and commit `6d9fe01` clean replay
  pass. The later usage-contract work expands the current boundary to thirteen. Nothing has been
  uploaded.
- Portable publication and input/output collision keys now ASCII-case-fold on every host, so a plan
  accepted on Linux cannot overwrite a case-only input alias when replayed on Windows. The frozen
  portability vector proves rejection before mutation.
- Read-only `audit --asset-plan` aggregation that regenerates the canonical plan from every adjacent
  CSS/manifest pair before measuring file/layer subjects and cross-bundle semantic duplicates.
  Package and route policies require the exact-bound ownership companion rather than inferred or
  whole-ledger manual attribution.
- SARIF 2.1.0 output for `audit`, projected from the same canonical finding document as human and
  JSON output. Every result embeds its complete schema-1 finding and retains exact locations,
  severity, fingerprint, source map, evidence, suggestions, and exceptions.
- First standards-first `pliego-cssc audit` slice for normal project-relative CSS, with no migration
  or writes: strict Lightning CSS ingestion, canonical human/JSON finding output, exact source/hash
  evidence, versioned backend identity, structural inventory, maximum-specificity and `!important`
  metrics, mandatory explicit target profile, bounded AST-backed compatibility decisions, unknown
  at-rule fail-closed behavior, explicit partial-coverage status, deterministic golden vector,
  defensive limits, and nonzero syntax/compatibility-failure exit.
- Canonical finding schema 1.0.0 in `pliego-css-build::artifacts`, including stable codes,
  policy/rule causes, portable authored/generated spans, deterministic context/evidence,
  verified/unverified/manual boundaries, ranked suggestions with risk/scope/prerequisites, reviewed
  exceptions, SHA-256 anti-tampering fingerprints, strict parsing, and byte-frozen JSON/human output
  from one semantic model.
- ADR-0016 and the implemented R0 central manifest/build-receipt contract, preserving style
  manifests 3–5 while avoiding a receipt self-hash cycle. Direct audits and generated control groups
  populate it from real analyzers and publish through rollback-capable boundaries.
- Canonical `pliegocss-token-graph/1` with aliases, derived values, deprecations, provenance, cycle
  rejection, validated theme permutations, transitive coverage, bounded canonical JSON, and fixed
  `pliego.tokens.json` publication in seven-artifact compile/build/watch groups and controlled
  bundles. TOML/seed projects the active registry; DTCG CLI selection publishes the complete
  Resolver graph, bundle schema 2 applies the same locally verified graph contract, and the Cargo
  build macro now encodes the selected Resolver registry for macro identity.
- A lossless DTCG 2025.10 format bridge and bounded same-document Resolver profile in
  `pliego-css-config`, with typed projection, aliases and JSON Pointers, sets/modifiers/contexts,
  last-wins permutations, cycle/type/domain validation, inherited deprecation, metadata, exact
  ThemeId round-trip, and inventory for CSS-native values without a faithful standard representation.
- A normative strategic product contract that rebases PliegoCSS as a standards-first compiler and
  verifier, maps every CSS/Rust/AI report pain point to current evidence, and makes the report's
  Phase 0 and R0 release requirements explicit instead of treating them as future product copy.
- Compatibility policy schema 2 / current policy version 7 through
  `pliego-cssc compatibility --targets baseline-widely|modern|none`, including deterministic browser,
  feature, reset, and scope decisions plus a byte-frozen external gate. The policy integrity-binds
  `web-features@3.32.0`, `baseline-browser-mapping@2.10.43`, their immutable package artifacts and
  the fixed-date mapping query/result.
- A strict `baseline-widely` profile frozen to the 2026-07-14 WebDX mapping: Chrome/Edge 120,
  Firefox 121, and Safari/iOS Safari 17.2. It preserves typed CSS but rejects arbitrary values,
  properties, and selectors with structured `CMP001`–`CMP003` diagnostics before publication.
- Typed boolean ARIA, data presence/data-state, and `ltr`/`rtl` variants with canonical native
  selectors, contradiction checks, strict-Baseline classification, and policy version 2 decisions.
- Typed `container-inline`/`container-normal` utilities and `cq-<theme-breakpoint>:` conditions with
  canonical IR, native nested `@container` output, physical provenance, and policy version 3.
- Configurable typed `aria-[name=value]`, `data-[name]`, and `data-[name=value]` variants with
  bounded canonical selectors, contradiction detection, short-form identity equivalence, strict
  Baseline classification, and policy version 4.
- Typed `writing-horizontal`, `writing-vertical-lr`, and `writing-vertical-rl` utilities with one
  conflict-checked semantic slot, native CSS output, append-only format-2 tags, strict Baseline
  classification, and policy version 5.
- Typed `layer-base`, `layer-components`, `layer-utilities`, and `layer-overrides` variants with a
  fixed namespaced native order, cross-layer semantic precedence, append-only format-2 identity,
  schema-5 physical tracing, strict Baseline classification, and policy version 6.
- Opt-in Project Index schema 1 through `bundle --project-index`. It requires schema 5,
  reachability, and `--asset-plan`; emits fixed `pliego.index.json`; integrity-binds source,
  CSS/manifest, and asset-plan bytes; and maps exact source sites to StyleId, semantic declarations,
  tokens, adapter-attested components, and bundle-qualified physical declarations.
- One shared `pliego-css-build` Project Index generator, strict portable/case-collision path
  validation, deterministic path- and site-addressed IDs, grouped publication, byte-exact
  `--check`, CLI integration coverage, schema reference, and ADR-0011.
- `pc!` and `pcx!` macros with compile-time grammar, typed-domain, conflict, and conditional
  composition validation.
- Typed semantic IR, deterministic StyleId/class generation, configurable theme artifacts, and
  deterministic CSS emission through Lightning CSS.
- `pliego-cssc` compile, check, inspect, watch, bundle, catalog, explain, and formatting commands.
- Added a lockfile-pinned PliegoRS development fixture and executable two-process gate for
  `pliego-cssc watch` plus `pliego dev`. Twenty alternating `pc!` edits verify semantic padding,
  exact served/published CSS hashes, stale-class removal, one stable SSE generation per edit,
  source-cache hits, and p50/p95 latency. Negative cases retain the last valid site and prove the
  tested unchanged CSS/manifest publication group preserves hashes/mtimes without advancing reload.
- Pinned the PliegoRS CLI/dev-server source in the cross-repository contract. PliegoRS now ignores
  PliegoCSS's reserved publication lock/temporary/backup files so coordination artifacts cannot
  cause scans, lock-sharing errors, or redundant site rebuilds.
- Structured diagnostics, schema-versioned manifests, declarative bundle plans, rollback-capable
  grouped publication, and a source-aware Rust scanner.
- Upgraded the local PliegoRS SSR/SSG gate to consume compiler-generated Asset Plan and Project
  Index artifacts from a schema-5/graph-2 build with explicit reachability and pruning. Its closed
  SSG consumer verifies exact source snapshots, canonical site/backlink mappings, bundle-qualified
  physical references, and every manifest/CSS pair. Negative vectors reject unsafe filenames and a
  detached Project Index. A rendered island contributes its separate bundle, while an unreferenced
  dead bundle remains in build inventory but is absent from the deployed site and PliegoRS ledger.
- Generated utility documentation, benchmark fixtures, WASM checks, and a three-OS portability
  contract configured in CI.
- A Rust 1.85 downstream public-API and identity smoke that binds macro output to standalone CLI CSS,
  manifests, catalog, inspection, and explanation documents from an unrelated working directory.
- Package verification now compiles and runs that public facade/build surface using registry-shaped
  exact dependencies patched exclusively to the ten extracted `.crate` archives and records each
  archive's exact SHA-256.
- Benchmark evidence harnesses now reject dirty snapshots, serialize concurrent runs, remove
  build-affecting Cargo/Rust overrides, use isolated Cargo homes, and bind Rust-check measurements
  to versioned fixture lockfiles.
- Froze clean-commit Gate A, Gate B, and paired Rust-check evidence for `c47239c`, including exact
  host/security metadata and every recorded timing sample.
- Added a CI evidence-integrity gate that resolves each recorded source commit, verifies harness
  and tracked-input hashes, recomputes statistics/comparisons, validates every paired Rust sample,
  and freezes the approved snapshot set by complete-file SHA-256.
- Added canonical semantic-IR binary format 2 with explicit identity/theme versions, portable spans,
  typed container-query conditions, defensive limits, theme-aware decode validation, canonical
  re-encoding, and frozen vectors. Format-1 cache artifacts require source-based regeneration.
- Hardened arbitrary CSS and selector validation against comment truncation, raw CSS string-line
  controls, nested braces, escaped `!important`, selector-scope escape, and expansion attacks; the
  supported boundary remains a standalone external stylesheet, not raw HTML inline CSS.
- Enforced the documented 60 KiB (61,440-byte) compressed-size budget for every packaged crate.
- Added opt-in provenance manifest schema 4 with a canonical graph from route/island to explicit
  component ownership, canonical semantic declarations, and directly referenced theme tokens.
- Added strict reachability sidecar schema 1 with exact portable source ranges, defensive limits,
  canonical ordering, dangling-reference rejection, compiler-verified origin coverage, and an
  explicit adapter-owned application-completeness attestation.
- Added explicit `--prune-unreachable` support to compile/build/watch/bundle with manifest schema 4
  or 5. Route and island components form one union root set; a StyleId is emitted when any exact
  origin is reachable, retained shared styles keep every origin, and per-bundle filtering never
  derives or changes the declared partition. Theme custom-property emission remains unpruned.
- Compile, watch, and bundle preserve schema 3 by default; with pruning disabled, schema-4
  reachability-only changes leave CSS and identity bytes unchanged and can republish only the
  manifest.
- Added opt-in manifest schema 5 and graph schema 2 with exact final UTF-8 ranges for qualified/media
  rules and declarations, many-to-many semantic contribution edges, generated-declaration markers,
  a synthetic theme producer, and compiler-verified complete physical coverage.
- Physical tracing carries canonical lineage through the existing emitter and reconciles it against
  the exact final Lightning CSS AST and serialization. Unsupported, partial, ambiguous, or
  out-of-budget traces fail before artifact publication; schemas 3/4 and their CSS bytes are
  unchanged.
- Added boolean `bundle --asset-plan`. It requires manifest schema 4 or 5 plus reachability and writes
  deterministic `OUTPUT_DIR/pliego.assets.json` schema 1 with common build identity,
  `all-compiled`/`reachable-style-ids`, exact adjacent CSS/manifest byte counts and SHA-256 digests,
  separate route/island mappings, and the application-global theme bundle. It emits portable
  filenames, not deployment URLs, HTML links, or preload policy.
- Asset plans participate in the existing rollback-capable grouped publication and complete
  `bundle --check` comparison. This does not claim crash-atomic multi-file replacement.
- Kept compiler integration tests in workspace/CI verification while excluding them from the
  published compiler archive, preserving the enforced 61,440-byte compressed package ceiling.
- Hardened CSS/manifest publication against manifest-only interleaving, output symlinks, Rust-source
  destinations, collisions with lock/temporary/backup coordination names, and lock-identity drift
  while a cooperating writer temporarily stages the previous output as a backup.
- Added a cascade-preserving pass that merges exactly equal adjacent media-query siblings, including
  a deterministic raw/gzip adoption gate and schema-5 physical-trace coverage.
- Added a deterministic unreachable-rule pruning benchmark with repeated schema-5 artifacts, closed
  graph checks, frozen raw/gzip measurements, and an explicit synthetic-payload scope boundary.
- Added cross-pipeline property contracts, a schema-5 determinism/publication gate with 40 workers
  in five eight-process cohorts plus a deterministic lock probe, and two pinned cargo-fuzz targets
  with bounded CI plus a scheduled soak.
- Moved the 53 CLI unit tests out of published `src/main.rs` while preserving their private-code
  harness and every black-box suite. The registry archive no longer pays for test-only source, and
  the fixed 60 KiB ceiling remains unchanged.

### Compatibility

- Kept `modern` as the compile default to preserve candidate CSS, made `none` the explicit unmanaged
  handoff, and defined reset `none` plus standard-class scope without hidden global CSS.
- Added machine-enforced candidate vectors for representative StyleId/class values, theme identity
  and binary format, JSON schemas, browser target configuration, and portable bundle bytes.
- Froze manifest schema 4, nested graph schema 1, declaration ID format 1, and reachability schema 1
  behind explicit producer opt-in; schema 3 bytes remain the default compatibility vector.
- Added the explicit schema-5/graph-2 compatibility vector with physical rule/declaration ID format
  1; projecting physical-only fields and edges yields the exact schema-4 graph.
- Added asset load-plan schema 1 as an explicit producer/consumer candidate contract; future wire
  changes require a new schema rather than inferred optional fields.
- Preserved bundle-plan schema 1 unchanged for seed/config themes and assigned schema 2 to the
  additive DTCG Resolver shape. Schema-1 consumers must reject schema-2-only theme fields.
- Kept component scope fail-closed in every 0.1.0 profile: native `@scope` is outside the frozen
  browser vector, and ancestor-attribute selector prefixing is not treated as equivalent scoping.
- Replaced the format-1 `Debug`-derived StyleId record with the explicit format-2 tagged binary
  stream. The stream is length-framed, big-endian, and UTF-8 aware; StyleId uses the first 128 bits
  of SHA-256 in big-endian order. The format change invalidates previously generated StyleIds and
  classes and requires regenerating their artifacts.
- Added fail-closed detection when distinct canonical identity streams produce the same compact
  StyleId during CLI aggregation.
- Bumped the CSS manifest to schema 3, inspection JSON to schema 2, catalog JSON to schema 3, and
  explain JSON to schema 2. Each now reports `styleIdFormatVersion`, `classNameFormatVersion`, and
  `themeIdFormatVersion` at the top level.
- Defined Rust 1.85 as the candidate MSRV and documented the intended `0.1.x` compatibility rules.
- Selected the minimal application surface (`pc!`, `pcx!`, `Style`, `StyleId`) and the unit-returning
  `theme!` bridge; internal compiler/tooling crates remain exact-version implementation APIs.
- Made `pc!` and `pcx!` resolve Cargo dependency renames of the `pliego-css` facade.
- Made single-value CLI options reject duplicates consistently across every command.

### Migration

- Recompile Rust macros and SSR/SSG output, regenerate CSS, manifests, catalogs, bundle assets, and
  any `pliego.assets.json` load plans,
  invalidate class-keyed caches, and deploy markup and CSS as one coordinated revision when moving
  from candidate StyleId format 1 to format 2. There is no conversion from a format-1 hash to a
  format-2 hash.
- The lowercase base-36 class encoder remains format 1, but its input StyleId changed, so existing
  `pc_*` strings must not be retained or mixed with regenerated assets.

### Documentation verification

- Added a Rust 1.85 getting-started gate that keeps the marked first-compile snippet identical to
  the checked `examples/basic` package and proves macro, manifest, literal CLI, source scanner, and
  generated CSS identity. This closes the first executable tutorial only; the full snippet corpus
  and human onboarding remain release blockers.

### Incremental emission

- Watch now retains emitted raw CSS fragments by canonical semantic stream, reuses unchanged
  fragments across source-only revisions, invalidates through theme-scoped identity, and prunes
  styles absent from the complete snapshot. Final Lightning CSS optimization still processes the
  complete artifact and remains an explicit F6 optimization target.

### Known release blockers

- Obtain hosted Windows/Linux/macOS and Cloudflare evidence, complete multi-browser verification,
  activate the selected API contract in an RC, and complete human documentation onboarding.
- Complete declared contrast/focus/motion/forced-colors policies, full attribution, and the remaining
  R0 receipt evidence before closing R0.5 or the MVP.
