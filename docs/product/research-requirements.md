# Research-derived product requirements

Status: **normative traceability baseline; alpha gates are not all closed**

This page tracks the original A1-A12 engineering research. The later
[strategic product contract](./strategic-product-contract-2026.md) converts the complete 14 July 2026
CSS/Rust/AI report into the controlling product and R0-R3 release contract. Both are normative
alongside the repository execution plan: a release claim must cite current code, a reproducible command,
and an artifact for every gate below. A roadmap checkbox or an indirect smoke test is not evidence.

## Product doctrine

PliegoCSS is a standards-first compiler/verifier and auditable CSS control layer, not a Tailwind
clone and not a proprietary browser runtime. Compact utility syntax is an optional typed frontend,
not the required input or product category. Its durable constraints are:

- compact, local authoring must remain possible;
- typed intent, conflicts, variants, tokens, scopes, layers, and source spans belong in canonical IR;
- semantically equivalent input must produce the same identity and native CSS artifact;
- every emitted rule and compatibility decision must be explainable from built artifacts;
- class, CSS, manifest, and project-index contracts must remain host-agnostic;
- PliegoRS receives the deepest adapter, but no Pliego runtime is required to consume output;
- reset/normalize behavior is opt-in and must never become a silent import side effect;
- raw CSS, typed custom properties, and explicitly governed literals remain escape hatches;
- migration tooling must classify unsupported input instead of silently dropping it.

## Evidence states

- **Verified**: the complete acceptance signal has a reproducible local gate and exact artifact.
- **Partial**: useful implementation exists, but at least one required dimension or environment is
  absent.
- **Missing**: no implementation-level acceptance evidence exists yet.

## Alpha gate traceability

| Gate | Current state | Evidence already present | Missing acceptance evidence |
|---|---|---|---|
| A1 Grammar/parser | Verified | `docs/status/f1.md`; parser spans, stable diagnostics, UI pass/fail corpus | Keep the golden corpus and ambiguity rejection green as grammar expands. |
| A2 Property/token model | Verified for the current alpha catalog | `docs/status/f2.md`; descriptor-driven property contract; typed domains for layout, spacing, color, typography, borders, and effects | Freeze the catalog/version only at the release candidate; DTCG interoperability is A7. |
| A3 Variants/scopes | Verified for the explicit 0.1.0 scope | `docs/status/f4.md`; responsive and typed container breakpoints, interaction, theme, motion, contrast, typed boolean and configurable ARIA/data-state/presence/value variants, `ltr`/`rtl`, typed native writing modes, fixed-order typed cascade layers, arbitrary selector subset, and conditional composition; ADR-0014 defers native component scope because the frozen browser vector predates `@scope` and no selector fallback is equivalent | Post-0.1 component scope and broader capability tiers require a new reviewed policy decision. |
| A4 Canonicalization/hash | Partial | StyleId format 2, semantic-IR binary format 2, property invariance, container-condition frozen vector, 40-process determinism gate | Clean cross-machine equivalence on the release commit and explicit authored-order equivalence corpus across every new IR dimension. |
| A5 Native CSS emitter | Partial | `docs/status/f3.md`; Lightning CSS parse/transform boundary; minified/pretty output; native `@container` and fixed-order `@layer` emission; schema-5 physical nesting/order trace; Browser/output certification v1 compares 52 computed properties, 1/64-pixel box/text-line geometry bounded to 0.1 CSS px, and bounded PNGs against current Tailwind across responsive/hover/focus and explicit reset modes; dirty local Windows x64 and Linux x64 Chromium/Firefox/WebKit diagnostics pass; ADR-0014 explicitly moves native component scope beyond 0.1.0 | Produce the clean hosted 9/9 Windows x64, Linux x64, and macOS ARM64 artifact; add a W3C-valid corpus and broader compatibility-policy fixtures. |
| A6 Manifest/source maps | Partial | Manifest schemas 3–5, graph schemas 1–2, exact UTF-8 spans/ranges, Project Index schema 1, canonical integrity-bound Source Map v3 for generated control groups, a revision-pinned PliegoRS SSG consumer validating exact source snapshots through bundle-qualified physical output, and a plain-HTML fixture binding exact line-input origins to manifest classes | Make browser-devtools and LSP integrations consume the shared artifacts, make the future non-Rust framework adapter emit Project Index/topology rather than only schema-3 style facts, and add a release-commit compatibility vector. |
| A7 DTCG token bridge | Partial | `pliego-css-config` implements the stable DTCG 2025.10 format bridge, bounded same-document Resolver sets/modifiers/contexts, canonical aliases/derived values, cycle rejection, metadata/deprecation provenance, validated theme permutations, and `pliegocss-token-graph/1`; controlled direct CLI builds bind exact Resolver bytes, canonical selections, and the complete integrity-bound graph; bundle-plan schema 2 integrates the same contract with local Windows/WSL E2E and package gates green; Cargo `theme!` selects the same active registry for macro identity, with local full-workspace/MSRV/API and clean-package gates green; accessibility policy schema 1 declares contrast pairs and resolves token endpoints through an optional canonical graph | Decide the optional external-reference policy if justified, complete supported-type validation for preserved namespaces, and reproduce graph/policy evidence on hosted release machines. |
| A8 Compatibility policy | Verified for the alpha catalog and bounded standard-CSS rule classifier | Policy schema 2 / policy 7; exact `web-features@3.32.0` and `baseline-browser-mapping@2.10.43` identities/hashes/query; frozen `baseline-widely`, fixed `modern`, unmanaged `none`; AST-backed audit decisions for layers, container query kinds, nesting, `@property`, `@scope`, and `@starting-style`; CMP001–CMP003 and unknown at-rules fail closed in strict mode; explicit partial-coverage finding; `pnpm check:compatibility`; ADR-0012/0013/0014 | Complete declaration-value and selector classification plus hosted Chrome/Firefox/WebKit execution remain A5/A12 evidence; component scope is explicitly `error` and deferred beyond 0.1.0, not silently supported. |
| A9 Host adapters | Partial | Standard class conversion; framework-neutral typed topology collector with fail-closed Rust inventory/site ownership; revision-pinned PliegoRS product registry and SSR/SSG fixture generating reachability plus shared/route/island/unreachable partitions and consuming Asset Plan/Project Index; rustc dep-info completeness across the exact native library, SSG binary, and wasm32 client targets; framework-neutral manifests; and a local Chrome plain-HTML contract fixture that maps exact line-oriented manifest origins to explicit template slots with zero styling runtime | Extend Cargo completeness evidence when feature/target support expands, and add at least one typed Vite/React or Astro adapter producing identical IR/artifacts and exact dynamic-branch coverage. |
| A10 Tailwind analyzer | Partial | Benchmark Authority v2 pins live 4.3.3, upstream v3-LTS 3.4.19, and historical 4.3.2 lanes; migration project schema 1 binds exact Tailwind CSS, config, plugin, and template files, resolves typed relative seams, inventories literal/dynamic template candidates plus closed unsupported config/plugin hooks, and has both an authored contract runner and a pinned MIT public-project role corpus without executing JavaScript. The public corpus measures the complete cross-tool role surface at 104 TP, 0 FP, and 0 FN over 119 reviewed files. | Add broader template/arbitrary-value/interpolation and source/safelist graphs, conflicts, semantic-observation gold labels, migration outcomes, and a larger representative corpus. |
| A11 Language tooling | Partial | Machine-readable diagnostics, catalog, explain, formatting replacements, manifests, shared Project Index schemas 1/2, an initial standard stdio LSP with full buffer sync/UTF-16/completion/hover/diagnostics/formatting plus integrity-verified source-to-final-CSS definition links, bounded cached per-literal reuse of compiler diagnostic schema 1, an unreleased VS Code client candidate with explicit executable/index configuration and a VSIX gate, and a pinned real VS Code 1.105.1 extension-host diagnostic/definition gate | Add another editor client, add debounce/cancellation, cover cross-literal `pcx!` semantics, and prove complete equality between CLI/editor errors over a frozen negative corpus and hosted matrix. |
| A12 Performance corpus | Partial | Benchmark Authority v2 micro/medium/large paired Tailwind lanes; Gate A/B historical context; Rust-check, watch, pruning, media merge, fuzz, parallel determinism, package-size gates; Browser/output certification v1 host evidence schema and 3×3 workflow | Content-addressed 1k/10k/100k graphs, 10-package monorepo, 50/500 scoped-file fixtures, cold/warm/incremental memory and p50/p95, hosted release-commit benchmark plus browser evidence. |

## Cross-cutting requirements not represented by one gate

The following claims remain release blockers even when their underlying syntax exists:

- accessibility release evidence beyond the implemented bounded static policy: reduced motion,
  forced colors, focus visibility, contrast, and input modality already preserve source evidence,
  deterministic exceptions, and independent `fail|warn` handling for violation, unverified, and
  manual-required states. Its local gate passes 30 control tests, nine CLI audit tests, one Asset
  Plan test, strict Clippy, and the dirty package gate; corpus precision/recall, browser/manual
  validation, hosted runners, and native macOS/ARM64 evidence remain release blockers, and the
  policy does not certify WCAG conformance;
- arbitrary-value budgets, inventory, token promotion suggestions, and deprecation diagnostics;
- deterministic explanation of the winning declaration across layer, scope, specificity, condition,
  and canonical order;
- package graphs and compilation units that do not depend on the current working directory;
- no silent global Preflight/reset and a documented embed/microfrontend boundary;
- production debugging from minified declaration back to authored source and token provenance;
- AI/tooling input through a versioned schema or IR API so generated styles cannot bypass validation.
- standard CSS audit without migration and versioned SARIF are implemented; bounded schema-1.0.0
  proposal/plan/dry-run is also implemented for exact verified, unexcepted, low-risk edits with
  budgets and idempotent source-state verification. Authorized exact-plan apply now publishes
  sources plus deterministic Change Receipt 1.0.0 with rollback, while truthfully leaving checks
  `not-run` and browser evidence `not-collected`; separate policy/Verification Receipt 1.4.0 with
  canonical 1.0.0/1.1.0/1.2.0/1.3.0 read support executes closed `standard-css-audit`,
  `token-graph-integrity`, identity-bound `css-budget-audit`, and canonical
  `test-suite-evidence` plus one pinned PliegoRS Chromium `browser-evidence` profile. The separate
  `run-tests` and `run-browser` boundaries have one fixed profile each; policy/plan cannot provide
  commands, scripts, URLs, selectors, or arguments. Every check publishes its complete canonical
  after-FindingDocument adjacent to the receipt with receipt-last rollback. Signed/hosted test
  attestation and Firefox/WebKit browser evidence remain blockers;
- route/package/layer budgets for bytes, rules, selectors, specificity, and semantic duplication;
  schema 1 closes one exact artifact plus verified Asset Plan file/layer aggregation, while ownership
  schema 1 binds exact plan bytes and resolves exclusive packages plus overlapping composed routes.
  CLI ingestion, input-ledger binding, and typed package/route budget projection pass locally;
  usage analysis schemas 1/2 now separate static reachability, scoped observation, usage verdict,
  generated whole-StyleId removal disposition, and hash-bound bundle-qualified retention. Generic
  CSS/token granularity and hosted release evidence remain open;
- evidence states distinguish verified, unverified, manual, unobserved, and dead without equating
  reachability to observation or sampled absence to deadness.

## Implementation order

The dependency order is deliberate:

1. Freeze the diagnostic and unified manifest/receipt schemas, then reuse the existing source,
   Project Index, compatibility, and deterministic-output foundations to
   implement standard-CSS `audit` and the complete R0 guard surface.
2. Close budgets, token graph, basic accessibility policies, equivalent human/JSON/SARIF reports,
   and cross-OS evidence before public `0.1.0`.
3. Build Sass/Tailwind/CSS Modules analyzers as read-only inventory before reviewed migrations (A10).
4. Finish verification of the implemented Cargo Resolver selection, then close declared token-policy
   relationships without making the external DTCG format the internal IR (A7).
5. Extend the implemented bounded single-stylesheet `explain-cascade` schema 1 to full cascade
   explanation, then extend the implemented authorized plan/fix/change-receipt and first built-in
   verification boundary with remaining check/evidence kinds; add a typed Vite/Astro adapter and LSP on the same
   Project Index (A6/A9/A11); keep Cargo source completeness synchronized with every supported
   PliegoRS feature/target permutation.
6. Add real-browser responsive/focus/interaction evidence, then a restricted plugin/policy ecosystem.
7. Scale the engineering corpus and reproduce release evidence on clean hosted machines (A12/A4/A5).
8. Incorporate interviews and real-incident research when organic usage provides meaningful inputs;
   use it to guide priorities and claims rather than as a prerequisite for preceding work.

`0.1.0` remains blocked by the R0 contract even when an A-row was previously scoped as alpha-complete.
Every row must be **Verified** or explicitly moved through a reviewed scope decision. Missing work
must never be represented as an implemented compatibility, accessibility, or agent-safety promise.
