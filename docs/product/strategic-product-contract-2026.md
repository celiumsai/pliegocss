# Strategic product contract: CSS, Rust, and AI

Status: **normative; adopted 2026-07-14**

This contract translates *PliegoCSS - Reporte estrategico de pain points y oportunidad de producto:
CSS nativo + Rust + agentes de IA* into release gates. The reviewed source is the 24-page PDF dated
14 July 2026 with SHA-256
`1177d2b35ae5336c07664d843f575a2f4937b9a026a9d21c4917b01b00f4822a`.

The report explicitly describes proposed capabilities, not existing implementation. Consequently,
this page distinguishes current evidence from required evidence and no unchecked row may be implied
by product copy. It is normative alongside the
[research requirements](./research-requirements.md) and `EXECUTION_PLAN_ULTRA.md`.

One source correction is recorded without changing the report's architectural conclusion: DTCG
2025.10 is a stable Final Community Group Report, not a preview draft. It is still not a W3C
Recommendation, so PliegoCSS keeps it behind a versioned adapter and retains an independent internal
model. See the [DTCG technical reports](https://www.designtokens.org/TR/2025.10/).

## Product category and non-negotiable decisions

PliegoCSS is the verifiable CSS compiler for humans and agents. It accepts and emits standard CSS,
applies explicit policy, explains decisions, and produces reproducible evidence. Compact utilities
and Rust macros remain optional authoring clients; they are not the category, the required input, or
the moat.

| Decision | Binding contract |
|---|---|
| Category | Compiler/verifier and auditable CSS control layer; never marketed as "Tailwind in Rust". |
| Input/output | Standard CSS first. Extensions are minimal, optional, typed, and lower to native CSS. |
| Backend | Compose a proven parser/transformer/minifier behind a Pliego-owned boundary; Lightning CSS is the initial backend. |
| Moat | Semantic IR, policy engine, provenance, explainability, agent contract, and receipts. Rust and speed are implementation requirements, not standalone differentiation. |
| Runtime | Zero styling runtime by default. No required JavaScript or WASM style engine. |
| Hosts | PliegoRS is first-class but not exclusive; generic repositories must receive value without migration. |
| Agents | Versioned deterministic contracts govern agents. No LLM is required or authoritative in the core. |
| Accessibility | Report verified, unverified, exception, and manual-test coverage. Never claim automatic WCAG certification. |
| Responsive | Use real browsers for layout/rendering evidence. Never implement a browser layout engine. |
| Adoption | `pliegocss audit` is the first-value wedge; compile, CI, migration, and typed manifests are layered adoption. |
| Claims | Only first-party reproducible benchmarks and measured diagnostic quality may support claims. |

Every proposed feature must answer: does this make CSS more verifiable, explainable, or safe for
humans and agents without creating new lock-in? Authoring convenience alone does not justify core
scope.

## Seven product pillars

| Pillar | Required outcome |
|---|---|
| Compile | Parse, normalize, target-transform, bundle, and minify native CSS with a proven backend. |
| Guard | Enforce compatibility, cascade/layer, specificity, accessibility, motion, forced-colors, duplication, deprecation, and budget policies. |
| Explain | Trace winning declarations, token provenance, component/artifact origin, and responsive evidence while stating static-analysis limits. |
| Tokens | Maintain a typed graph of tokens, aliases, derived values, themes, cycles, coverage, and declared contrast relationships. |
| Agent | Emit stable diagnostics, ranked suggestions, bounded plans, dry-runs, idempotent patches, and machine-readable results. |
| Bridge | Audit Sass, Tailwind, CSS Modules, PostCSS, and generic CSS incrementally; classify unsupported constructs instead of dropping them. |
| Receipt | Integrity-bind inputs, configuration, backend, targets, decisions, exceptions, patches, checks, and outputs. |

The operating loop is `Discover -> Model -> Check -> Explain -> Plan -> Apply -> Receipt`. `Apply`
always requires explicit authorization; the earlier steps are read-only and deterministic.

## Prioritized pain-point contract

| ID | Priority | Pain point | Acceptance contract | Current state on 2026-07-16 |
|---|---|---|---|---|
| S1 | P0 | Compatibility and platform pace | Versioned target/Baseline policy sourced from versioned official web-features data; every feature is preserved, transformed, given a fallback, degraded, experimental, or blocked with reason and data date. | **Partial:** policy schema 2/version 7 binds exact official datasets/query/result and standard-CSS audit classifies a bounded rule-feature set fail-closed; complete declaration/selector decisions, fallback/browser evidence, and the unified receipt remain missing. |
| S2 | P0 | AI-generated CSS governance | Stable schema/code/span/cause/evidence/constraints plus ranked suggestions with risk, scope, prerequisites, change budget, dry-run, idempotence, and receipt; human and agent use the same deterministic core. | **Partial — bounded repair apply and five verification kinds implemented:** source spans, canonical findings, human/JSON/SARIF audit output, catalog, and explain contracts exist. Proposal/plan/dry-run schema 1.0.0 admits only exact verified, unexcepted, low-risk edits; enforces source containment, prerequisites, non-overlap, portable paths, and file/edit/byte budgets; binds exact FindingDocument and before/after source snapshots, structured patch, and payload hashes; and recognizes complete `ready|already-applied` states. Explicit apply requires an external exact-plan selection token, revalidates under a persistent cooperative lock, and publishes sources plus Change Receipt 1.0.0 with rollback. Separate policy/Verification Receipt 1.4.0 with canonical 1.0.0/1.1.0/1.2.0/1.3.0 read support executes built-in `standard-css-audit`, `token-graph-integrity`, and identity-bound `css-budget-audit`, validates fixed-profile `test-suite-evidence` plus one pinned PliegoRS Chromium `browser-evidence` profile, covers every after source with source-specific checks, derives `passed|failed|blocked` without a policy/plan external-command surface, and publishes complete adjacent after-FindingDocuments with receipt-last rollback. A tracked 16-case synthetic corpus protects the repair authority boundary (one deterministic accepted control and 15 intended fail-closed rejections) with explicit provenance and claim limits. Hosted signer/approval protocol, Firefox/WebKit evidence, real-incident corpus evidence, and same-model turn-reduction trials remain missing. |
| S3 | P0/P1 | Cascade, specificity, and provenance | Semantic graph over rules, layers, scopes, tokens, sources, components, routes, and artifacts; `explain cascade` identifies the winner and specificity escalation, and declares when browser evidence is required. | **Partial — bounded static slice implemented:** `explain-cascade` schema 1 resolves one author stylesheet for a simple declared HTML element and closed direction-independent longhand set. It preserves exact declaration spans and shorthand origin, ranks top-level named layers/importance/specificity/source order, reports escalation, and returns `browser-required` with stable blockers rather than guessing across selectors, conditions, scopes, imports, nesting, animations, transitions, `all`, or `revert*`. Multiple sheets/origins, inline styles, complete selector/logical-property/custom-property/inheritance modeling, graph joins, and browser continuation remain missing. |
| S4 | P0 | Tokens, themes, and drift | Typed aliases, cycles, derived values, themes, coverage, contrast/focus/motion contracts, deprecation, and versioned DTCG exchange over a stable internal model. | **Partial:** the typed registry, DTCG 2025.10 format/Resolver bridges, and canonical `pliegocss-token-graph/1` preserve aliases, derived values, deprecation provenance, selections, and validated theme permutations. Generated control groups publish the graph and measure transitive use coverage; direct CLI selection binds the complete Resolver plus canonical context inputs. Bundle-plan schema 2 integrates exact Resolver/plan evidence and the complete graph with local gates green. Cargo build-macro selection passes the local full-workspace/MSRV/API and clean-package gates. Accessibility policy schema 1 now declares contrast pairs and resolves token endpoints against an optional canonical graph; hosted portability evidence remains missing. |
| S5 | P0 | Accessibility hidden in visual choices | Configurable contrast, focus visibility, motion, forced-colors, and input-modality guards with exact evidence, explicit exceptions, and manual-test status. | **Partial — bounded static policy implemented:** schema 1 / policy 1 requires all five checks and independently maps violation, unverified, and manual-required results to `fail|warn`; declared contrast pairs use literal colors or an optional canonical TokenGraph, while CSS AST checks cover exact motion-preference guards, explicit focus-outline suppression, forced-color opt-out, and same-rule hover/focus equivalence. Unproven cascade and cross-context relations fail closed as manual-required. Findings retain evidence, deterministic exceptions, and manual-required status. Corpus precision/recall, browser/manual validation, hosted runners, and native macOS/ARM64 evidence remain missing; this is not WCAG certification. |
| S6 | P0 | Budgets, duplication, and dead CSS | Budgets by route/package/layer for bytes, rules, selectors, specificity, and duplication; semantic fingerprints; typed ownership; conservative removal and allowlists; distinguish `unobserved` from `dead`. | **Partial:** budget schema 1 enforces exact-artifact file/layer metrics plus ownership-backed package and composed-route metrics over a canonically regenerated Asset Plan, including cross-bundle duplication, deltas, and reviewed exceptions. Ownership schema 1 binds exact plan bytes and resolves package/composed-route views. Usage analysis schemas 1/2 inventory the complete pre-pruning bundle-qualified StyleId universe, separate static reachability from scoped positive observation, derive `observed|unobserved|dead|unknown`, bind exact evidence, retain tombstones, and record report/pruning/policy disposition; sampled absence never proves deadness and contradictory evidence fails closed. The explicit retention sidecar is bundle-qualified and preserves reviewed dead StyleIds without relabeling them. Declaration/theme-token granularity, hosted evidence, and generic-CSS usage identities remain missing. |
| S7 | P1/P2 | Responsive/layout diagnosis | Real-browser viewport matrices, controlled perturbations, ranked causal properties, visual/focus/interaction evidence, and explicit confidence/limits. | **Missing:** current Chromium smoke validates known fixtures but is not a responsive failure localizer. |
| S8 | P1 | Fragile static extraction | Exact typed source manifests from PliegoRS/framework compilers plus a useful generic HTML/JS/template mode; never infer certainty from text scanning. | **Partial:** strict adapter-attested reachability, Asset Plan, and Project Index exist; a framework-neutral typed collector rejects unowned visible Rust macro sites. The PliegoRS product registry drives deterministic reachability, automatic shared/route/island/unreachable partitioning, and SSG composition. Rustc dep-info now proves CSS-source completeness across the fixture's exact native library, SSG binary, and wasm32 client targets, with before/after graph and byte stability; unbuilt feature/target permutations are not claimed. A plain-HTML reference contract maps exact line-oriented manifest origins to declared template slots and proves the result in Chrome without a styling runtime. A typed Vite/Astro adapter for dynamic template branches remains missing. |
| S9 | P1 | Migration and lock-in | Layered `audit-only -> CI guard -> compile -> tokens -> typed source manifests`; reversible codemods, classified gaps, and visual tests; no perfect-migration claim. | **Missing:** Tailwind is currently only a benchmark fixture. |
| S10 | P2 | Preprocessor logic | Add only limited typed abstractions that compile to stable CSS and have measured value; do not center the MVP on a new dialect. | **Partial and intentionally bounded:** typed macros/recipes exist; further syntax expansion is frozen behind R0 evidence. |

## Phase 0 evidence gate

The report treats market evidence as engineering input, not launch copy. Before the product category
is considered validated, PliegoCSS must have:

- 10-15 interviews across design-system teams, platform teams, agencies, and AI-heavy teams;
- 20 anonymized real CSS incidents spanning compatibility, cascade, tokens, responsive behavior,
  accessibility, and bloat;
- a diagnostic corpus built before the final UI or broad automatic-fix surface;
- audit-only trials that measure which findings lead to action;
- the five highest-signal rules converted into configurable gates;
- a reproducible competitive baseline against direct Lightning CSS work, Tailwind, UnoCSS, PostCSS,
  Sass output, CSS Modules, and the current PliegoCSS pipeline.

Provider benchmarks are context only. The corpus must record provenance, consent/redaction status,
expected diagnosis, allowed ambiguity, and stable content hashes. Public release claims remain
blocked until a reviewed evidence report exists.

## R0 / `0.1.0` release contract

The following are mandatory, not aspirational. Existing utility-first implementation is reusable
foundation but cannot substitute for an unchecked item.

| Gate | Required evidence | Current state |
|---|---|---|
| R0.1 Standard CSS compile/audit | Normal CSS input accepted without migration; proven backend; target transforms and minification; deterministic bytes. | **Partial:** `audit` accepts normal CSS, binds the proven backend/source hash, and emits deterministic findings/inventory; the generic CSS transform/minify output path and complete declaration/selector classification remain missing. |
| R0.2 Deterministic manifest and receipt | Source/config/output hashes, backend name/version, targets and compatibility-data date, source maps, counts, token graph, decisions, violations, exceptions, and receipt. | **Partial:** direct CSS/Asset Plan audit, exact-snapshot compile/watch, and `bundle --control` feed the closed schema-1 models from real analyzers. Generated groups emit canonical rule-level Source Map v3 sidecars, publish integrity-bound `pliego.tokens.json`, bind their complete output graph, and expose read-only `--check` where finite. Direct CLI DTCG input binds exact Resolver bytes, canonical selections, and the complete graph; bundle schema 2 applies exact plan/Resolver binding with local gates green. Full attribution, hosted runners, and macOS remain missing. |
| R0.3 Compatibility policy | Stable diagnostic codes and reproducible preserved/transformed/fallback/degraded/blocked decisions based on versioned official data. | **Expanded bounded slice:** exact official web-features 3.32.0 data drives eight rule families, five declaration/value features (`user-select`, `aspect-ratio`, `color()`, Oklab/OkLCh, `color-mix()`), the separately rejected variadic `color-mix()` feature, and five selector features (`:focus-visible`, `:has()`, `:is()`, `:where()`, selector-list `:not()`). The partial-coverage finding remains mandatory because the complete CSS value/selector surfaces and browser/fallback evidence remain open. |
| R0.4 Budgets | Bytes, rules, selectors, specificity, and semantic duplication, with route/package/layer attribution and regression deltas. | **Locally verified bounded slice:** schema 1 closes exact artifacts plus verified Asset Plan file/layer aggregation and ownership-backed package/composed-route aggregation with cross-bundle fingerprints. Usage analysis schemas 1/2 retain generated StyleId evidence. Generic CSS audit now emits context/selector/importance/declaration-bound identities; `generic-css-usage` joins only positive observations, keeps absence `unknown`, and can publish report/findings/manifest/receipt as a rollback-capable group. Route views remain outside Control Manifest partitions; generic declaration evidence does not prove deadness; hosted evidence and final release replay remain open. |
| R0.5 Token graph | Aliases, cycle rejection, themes, coverage, hashes, and typed provenance. | **Partial:** the canonical graph, DTCG format/Resolver bridges, aliases, derived values, deprecation provenance, cycle rejection, validated theme permutations, graph hashes, transitive coverage, integrity-bound control artifact, direct CLI selection, bundle-plan schema-2 selection, and Cargo build-macro selection are implemented. Canonical Token Usage now projects retained StyleId consumers, transitive dependencies, unused active tokens, and actual custom-property emission with a read-only query CLI. Local gates are green; hosted evidence remains missing. |
| R0.6 Basic accessibility guards | Declared contrast pairs and configurable motion/focus policies with verified/unverified/manual coverage. | **Locally verified bounded slice:** direct CSS and verified Asset Plans evaluate the closed schema-1 policy for contrast, motion, focus visibility, forced colors, and input modality; policy and optional TokenGraph bytes are integrity-bound, findings preserve verified/unverified/manual-required states and exceptions, and control token measurements record declared pair count. The machine quality corpus records 4 TP, 4 TN, 0 FP, 0 FN for decidable literal-sRGB contrast, four correct dynamic abstentions, and eight exact-code static guard cases; it is part of `verify:fast`. Browser/manual validation, hosted runners, native macOS/ARM64 evidence, and real-incident quality evidence remain open, so no WCAG certification or broad accuracy claim is made. |
| R0.7 Equivalent reports | Human, JSON, and SARIF outputs express the same findings and severity, with exact spans and stable schemas. | **Verified for `audit`:** human, canonical JSON, and SARIF 2.1.0 project the same schema-1 findings; SARIF embeds each complete finding and maps exact locations/fingerprints. Legacy process diagnostics outside `audit` remain a separate schema, not omitted audit findings. |
| R0.8 Audit bridges | At least read-only inventory for Sass, Tailwind, and CSS Modules, including unsupported/dynamic constructs and Preflight reliance. | **Partial — bounded project inventory with public evidence:** schema 1 binds exact source/consumer/auxiliary bytes and observations, the CLI accepts a declaration or bounded directory discovery, and the authored corpus plus three pinned MIT projects replay without executing original toolchains. The public role corpus reviews 119 files and measures 104 TP, 0 FP, and 0 FN for emitted file-role tuples; all 144 Bootstrap Sass edges resolve through closed relative file/partial/index/import-only rules. Configured load paths/importers, broader syntax, migration outcomes, reversible codemods, and representative scale remain open; the metric is not semantic migration accuracy. |
| R0.9 Determinism/performance | Byte identity on Windows/Linux/macOS; cold, incremental-change, no-op, peak memory, binary/package size, and supported-corpus precision/recall evidence. | **Partial:** strong local gates and CI matrix exist; hosted cross-OS and diagnostic-quality evidence are missing. |

`0.1.0` is blocked until R0.1-R0.7 and R0.9 are verified. R0.8 is a report-level **Should** and may
move only through a reviewed ADR with a concrete post-release version; it cannot silently disappear.

## Subsequent gates

- **R1 - Explain and agent loop:** extend the bounded schema-1 cascade foundation to full
  cascade/token explanation; typed PliegoRS source manifest; extend the implemented schema-1.0.0
  `plan` and `fix` boundary beyond its authorized rollback-capable change receipt with required-check
  execution and final verification receipts; add Sass/Tailwind/CSS Modules bridges and
  CI actions. A 16-case synthetic conformance corpus now protects the bounded authority contract but
  does not count as agent efficacy. Gate: the same agent/model resolves a separately reviewed frozen
  corpus in fewer turns without increasing violations.
- **R2 - Browser-assisted diagnostics:** real viewport matrices, responsive localization, visual
  diffs, focus/interaction tests, LSP/IDE, and route baselines. Gate: reproducible browser evidence
  reaches category-specific precision without alert fatigue.
- **R3 - Ecosystem and governance:** restricted/sandboxed plugin SDK, deterministic policy packs,
  stabilized DTCG adapter, and third-party integrations. Gate: extensions cannot weaken security,
  determinism, schema validation, or receipts.

Deferred by design: a required JS runtime, CSS-in-JS subsystem, component/UI framework, broad new
utility language, layout/raster engine, generative correction in the core, automated accessibility
certification, unversioned DTCG coupling, and an unrestricted plugin ecosystem.

## Central artifact contract

The long-term `pliego.css.manifest.json` must expose these independently versioned sections:

| Section | Minimum contents |
|---|---|
| `schemaVersion` | Exact wire contract and migration rules. |
| `sourceHash`, `configHash` | Canonical input and policy/config identities. |
| `backend` | Parser/transformer name and exact version. |
| `targets` | Profile, browser vector, official compatibility-data version/date. |
| `outputs` | File, byte count, SHA-256, and source-map identity. |
| `rules` | Counts by layer, route, component, state, and rule type. |
| `tokens` | Graph, aliases, derived values, themes, coverage, cycles, and hashes. |
| `decisions` | Preservations, transforms, fallbacks, degradations, experiments, and blocks. |
| `violations` | Stable code, severity, exact span, policy, evidence, exception, and coverage status. |
| `receipt` | Acyclic expectation for the adjacent receipt schema/file, required checks, and expected derived result. |

The current manifests remain valid numbered contracts. The unified artifact requires a new schema;
existing fields must not be relabeled to imply evidence they do not carry.

## Agent-ready contract

Every agent-facing diagnostic must include a stable code, versioned schema, exact span/source map,
cause, policy, evidence, ranked suggestions, risk, scope, prerequisites, deterministic flag, and
applicable exception. Plans add maximum files/rules/tokens, plan hash, diff, and required checks.
Receipts bind tool/schema versions, source/config hashes, targets/data date, plan/patch hashes,
changed counts, diagnostic totals, browser evidence status, output hash, and bytes.

Finding schema 1.0.0 now implements the shared diagnostic model and byte-frozen JSON/human vector.
The [control-artifact schema 1.0.0](../reference/control-artifacts-schema-1.md) implements the
manifest/receipt wire models, canonical encoder, strict decoder, integrity validation, and frozen
fixture. The [R0 manifest/receipt design](./r0-manifest-receipt-design.md) records the implemented
audit, exact-snapshot compile/watch, source-map, canonical TokenGraph, and bundle-build
projection/grouped-publication paths plus direct CLI DTCG selection. Bundle-plan schema-2 DTCG
selection has local gates green; Cargo selection passes the local workspace/MSRV/API and clean
package gates, while attribution and hosted-evidence gates remain open.
The first [`audit` slice](../reference/audit-command.md) now proves normal-CSS ingestion, backend
identity, source hashing, inventory evidence, and canonical syntax findings. It is deliberately not
counted as the complete R0 audit/policy surface.

The same patch applied twice must be a no-op. No fix is applied without explicit human or policy
authorization. Browser-required checks cannot be replaced by static confidence.

## Metrics and launch proof

Required measures are:

- cross-OS byte identity for the supported corpus;
- cold, incremental-change, and no-op latency, peak memory, binary/package size;
- precision, recall, false positives, exceptions, and time-to-resolution by diagnostic category;
- bytes/rules/duplication per route and pull-request delta;
- theme coverage, invalid aliases, cycles, and evaluated contrast pairs;
- agent violations, correction turns, cost, and idempotent-fix rate against a no-PliegoCSS control;
- time to first audit value, CI activation, and optional progression to compile mode.

Launch demos must prove the agent guardrail, typed source-manifest handling of a dynamic-class case,
and token blast-radius analysis. The launch narrative is: **AI can write CSS; PliegoCSS decides
whether that CSS belongs in your system.**
