# Standard CSS audit command

Status: **implemented E1 ingestion, bounded rule-feature compatibility, artifact budgets, bounded
accessibility policy, ownership-backed Asset Plan package/composed-route aggregation, and
CSS/Asset-Plan audit control artifact publication/check; complete declaration/selector
compatibility and runtime accessibility evidence remain open**

`pliego-cssc audit` reads ordinary UTF-8 CSS without converting it to PliegoCSS utilities, changing
the source, or requiring PliegoRS. The target profile is mandatory so a compatibility statement can
never depend on an implicit browser audience:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely --format human
pliego-cssc audit --input src/app.css --targets modern --format json
pliego-cssc audit --input src/app.css --targets modern --format sarif
pliego-cssc audit --input src/app.css --targets none --format json
```

An optional closed budget policy adds file/layer limits plus explicit package/route attribution:

```console
pliego-cssc audit --input dist/home.css --targets baseline-widely \
  --budget-policy config/pliego.budgets.json \
  --budget-subject package=app --budget-subject route=/home --format json
```

See [budget policy schema 1](./budget-policy.md) for exact metric, delta, exception, and ownership
semantics. Policy coverage is total: every declared budget definition must match one verified
subject. Any unmatched definition, including one in an otherwise partially matched policy, emits
`PCSS-BUDGET-199` and fails the audit.

An optional closed accessibility policy applies the same checks to direct CSS or to every stylesheet
in an Asset Plan:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json --format json
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json \
  --token-graph dist/pliego.tokens.json --format json
```

`--token-graph` requires `--accessibility-policy`. It supplies exact canonical graph evidence for a
token endpoint or selected theme. Literal-to-literal relationships without `selections` remain
verifiable without a graph. The policy configures `fail|warn` independently for violations,
unverified results, and manual-required results across contrast, motion, focus visibility, forced
colors, and input modality. See [accessibility policy schema 1](./accessibility-policy.md).

For compiler-produced multi-bundle output, replace `--input` with the mutually exclusive Asset Plan
mode. Routes come from the integrity-checked plan rather than manual assertions:

```console
pliego-cssc audit --asset-plan dist/assets/pliego.assets.json --targets baseline-widely \
  --budget-policy config/pliego.budgets.json \
  --accessibility-policy config/pliego.accessibility.json --format json
```

The audit loads every adjacent CSS/manifest pair and requires the supplied plan bytes to equal a
canonical regeneration. File and layer policies can use that verified ledger directly. Because
Asset Plan schema 1 keeps islands separate and does not attest route-to-island occurrence, every
package or route policy additionally requires an explicit ownership sidecar; PliegoCSS never guesses
that composition.

Ownership schema 1 implements the explicit seam for typed package and composed-route budgets:

```console
pliego-cssc audit --asset-plan dist/assets/pliego.assets.json \
  --ownership config/pliego.ownership.json --targets baseline-widely \
  --budget-policy config/pliego.budgets.json --format json
```

The sidecar is never discovered. It binds the exact Asset Plan byte count and SHA-256, maps every
bundle to exactly one package, and lists islands for every route. The resulting route observation is
the stable deduplicated union of base route bundles plus those island bundles. Manual
`--budget-subject` values are rejected in Asset Plan mode, so the sidecar is the only package/route
authority.

`--input` must be a project-relative, portable logical path. Absolute paths, parent segments,
symlinks, non-files, non-UTF-8 input, and files over 16 MiB fail closed. The command does not write
unless `--control-dir` is explicitly selected.
`--format` defaults to `human`; `json` emits canonical finding schema 1.0.0 and `sarif` emits SARIF
2.1.0. Every SARIF result carries the standard `ruleId`, mapped `level`, exact location and partial
fingerprint plus the complete canonical finding under `properties.pliegoCssFinding`, so evidence,
verification, suggestions, exceptions, and source maps cannot drift. Invocation/tool failures still
use the separate legacy diagnostic envelope selected by `--diagnostic-format`.

## Canonical control group

For direct CSS or an Asset Plan, an existing output directory can receive the canonical audit group
without changing the selected stdout renderer:

```console
mkdir -p reports/pliegocss
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --control-dir reports/pliegocss --format human
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --control-dir reports/pliegocss --check
pliego-cssc audit --asset-plan dist/pliego.assets.json --targets baseline-widely \
  --control-dir reports/pliegocss
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json \
  --token-graph dist/pliego.tokens.json --control-dir reports/pliegocss
```

The first command publishes `pliego.css.findings.json`, `pliego.css.manifest.json`, and
`pliego.css.receipt.json` through the existing lock/rollback-capable group boundary. The second
recomputes the same bytes and exits nonzero on missing or changed files without modifying them.
When `--token-graph` is present, fixed `pliego.tokens.json` is published first and the complete audit
group has four artifacts; `--check` recomputes and compares all four. Policy-only audits retain the
three-artifact group.
The publication boundary rolls back ordinary write/rename failures; it is not a crash-atomic
filesystem transaction, so abrupt process or machine termination still requires a rerun.

The manifest binds a canonical one-source set, target/budget/accessibility config identity, exact
Lightning CSS backend and stages, frozen target/data snapshot, finding output, rule inventory,
compatibility decisions, findings, exceptions, and required receipt checks. Package/route/component/state/rule
ownership remains `unknown` unless a typed source attests it. A direct CSS audit has no typed token
source unless `--token-graph` is supplied, so omission produces `unavailable` with a reason rather
than zero measured tokens. A supplied graph produces measured token state and exact graph output.
Its `coverageBasisPoints: 0` records that standard-CSS audit has no compiler-owned typed-use set; it
does not mean unavailable and does not claim that the application uses no tokens. Failed syntax
likewise makes rule measurement `unavailable` and produces a valid failed receipt.

The input ledger records exact roles. Direct CSS and Asset Plan CSS use `source`; style manifests use
`style-manifest`; the plan uses `asset-plan`; budget policy uses `policy`; ownership uses
`ownership`; accessibility policy uses `accessibility-policy`; and an explicitly supplied graph uses
`token-graph`. The parsed budget policy is canonically serialized before its content contributes to
`configHash`. Ownership, accessibility-policy, and token-graph inputs contribute their exact selected
file bytes, so byte-only drift in those inputs changes `configHash`; this remains independent of the
terminal renderer.

Asset Plan mode first regenerates and verifies the canonical plan. Its input set then contains the
exact plan and every adjacent CSS/style-manifest pair, its rule metrics aggregate all bundles, and
the receipt requires `asset-plan-integrity`. Route budget observations are allowed to overlap through
shared bundles; `rules.byRoute` remains `unknown` until a non-overlapping ownership model can attest a
partition. Ownership schema 1 deliberately does not make that claim: its package map is exclusive,
but composed route views can still overlap, so `rules.byRoute` remains `unknown` even when ownership
is supplied. Package findings also do not populate `rules.byPackage`; that control projection waits
for a future schema with its own partition validation.

This audit slice is also exercised as an independent consumer of a compiler-produced Asset Plan.
Generated `bundle --control` and exact-snapshot compile/watch groups use the same schema and emit
integrity-bound Source Map v3 sidecars. Audit-only inputs do not invent a generated map.

## Bounded accessibility evidence

The accessibility projection is deterministic and static. Contrast is evaluated only for
foreground/background relationships declared by policy. A relationship without `selections` fans
out across graph themes when a graph is present, but `tokens.contrastPairs` counts the declared
relationships, not those theme evaluations. Exact unrounded WCAG 2.2 relative-luminance comparison
is available only for statically resolved, opaque, in-gamut sRGB colors; dynamic or incomplete color
evidence stays `unverified` or `manual-required` according to the analysis boundary.

CSS-AST checks cover motion scoped directly to exact `prefers-reduced-motion: no-preference`, active
motion under `reduce`, explicit outline suppression on `:focus`/`:focus-visible`, explicit
`forced-color-adjust`, and same-rule hover/focus declaration equivalence. Separate motion overrides,
positive focus declarations without computed-cascade evidence, cross-rule/file correlation,
nesting/scope/layer boundaries, dynamic values, and complex selectors preserve `manual-required`
rather than being reported as passes. CSS parse failure makes the four CSS-AST checks `unverified`
for that stylesheet.

Every non-summary accessibility observation carries stable `context.subject-id`. A reviewed
exception matches one exact check plus subject ID, changes the result to its `x02` code, retains evidence and
verification, and prevents only that observation from failing. An unmatched exception emits
`PCSS-A11Y-999`. The evaluator has no wall clock, so `expiresOn` is integrity-bound review metadata;
time-based expiry enforcement belongs to CI or a future versioned evaluation-date contract.

## Frozen compatibility evidence

The audit is offline and reproducible. Compatibility policy schema 2 / policy 7 records:

- `web-features@3.32.0`, its npm SRI, immutable tarball URL, and the SHA-256 of `data.json`;
- `baseline-browser-mapping@2.10.43`, its npm SRI and immutable tarball URL;
- the exact query `widelyAvailableOnDate=2026-07-14;includeDownstreamBrowsers=false`;
- the SHA-256 of that query's canonical browser vector;
- Chrome/Chrome Android/Edge 120, Firefox/Firefox Android 121, and Safari/iOS Safari 17.2.

No network, clock, Browserslist file, or host package changes an audit. Updating either dataset is a
reviewed policy-version change with a new golden artifact.

The first classifier version, `css-rule-features-1`, recognizes these AST-backed features:

- cascade layers;
- size, style, and scroll-state container queries;
- CSS nesting;
- registered custom properties with `@property`;
- `@scope`;
- `@starting-style`.

Each observed feature receives a canonical decision carrying the official feature ID, compat key,
Baseline status/dates, per-browser minimums, occurrence count, dataset identity, selected profile,
and an exact span for the first occurrence. `baseline-widely` accepts only status `high`; `modern`
compares the frozen per-browser support against its fixed native target vector; `none` records an
explicit unmanaged result. Rejected features include ranked, non-authoritative remediation paths.

Unknown at-rules and unrecognized `@container` feature names emit `PCSS-COMPAT-199`. They fail under
`baseline-widely`, warn under `modern` and `none`, and always remain `unverified`: successful parsing
is never browser-support proof.

## Findings and exit status

| Code | Meaning |
|---|---|
| `PCSS-AUDIT-000` | Complete source parsed and structurally inventoried. |
| `PCSS-SYNTAX-001` | Verified syntax failure with exact UTF-8 span. |
| `PCSS-COMPAT-001` | Explicit warning that the current classifier is bounded. |
| `PCSS-COMPAT-100` | Classified native feature allowed by the selected profile. |
| `PCSS-COMPAT-101` | Classified native feature rejected by the selected profile. |
| `PCSS-COMPAT-102` | Classified feature intentionally unmanaged under `none`. |
| `PCSS-COMPAT-199` | Parsed at-rule has no official classifier mapping. |
| `PCSS-BUDGET-100` | Absolute and regression boundaries pass. |
| `PCSS-BUDGET-101` | Budget exceeded without a reviewed exception. |
| `PCSS-BUDGET-102` | Budget exceeded under an explicit reviewed exception. |
| `PCSS-BUDGET-198` | Asset Plan budget measurement is unavailable because one integrity-bound CSS bundle failed syntax ingestion. |
| `PCSS-BUDGET-199` | One or more definitions in the explicit policy matched no verified observed subject; partial policy coverage also fails. |
| `PCSS-A11Y-000` | Summary of the configured static accessibility gate; never a global compliance result. |
| `PCSS-A11Y-100` / `101` / `102` / `108` | Declared contrast pass, violation, reviewed exception, or unverified/manual result. |
| `PCSS-A11Y-200` / `201` / `202` / `208` / `209` | Motion pass, violation, reviewed exception, unverified, or manual-required result. |
| `PCSS-A11Y-300` / `301` / `302` / `308` / `309` | Reserved focus pass, explicit suppression violation, reviewed exception, unverified, or manual-required result. Schema 1 does not emit `300` without computed-cascade proof. |
| `PCSS-A11Y-400` / `401` / `402` / `409` | Forced-colors pass, violation, reviewed exception, or unverified/manual-required result. |
| `PCSS-A11Y-500` / `501` / `502` / `508` / `509` | Input-modality pass, violation, reviewed exception, unverified, or manual-required result. |
| `PCSS-A11Y-999` | Configured accessibility exception matched no current non-pass observation. |

The process exits nonzero for syntax failures, enforced compatibility errors, budget errors, and
accessibility result classes configured as `fail`. Warnings and unmanaged findings do not fail the
process. `passed=true` therefore means only that the declared bounded static gates produced no
unexcepted enforced error; it is not a claim of complete CSS compatibility or WCAG conformance.

## Structural inventory

The inventory binds the logical file and full UTF-8 byte range, exact input SHA-256/size,
`lightningcss@1.0.0-alpha.71`, recursive rule count, style rules, selectors, style declarations,
`!important` declarations, maximum specificity, classifier version, unique classified features,
unclassified syntax groups, target profile, canonical minified bytes, semantic duplicate groups,
and additional duplicate occurrences.

The parser enables standard nesting, uses no error recovery, and does not transform or print the
stylesheet. Invalid CSS converts the backend's UTF-16 position to exact UTF-8 bytes and one-based
Unicode-scalar coordinates. Human and JSON views derive from the same finding document.

## Open R0 work

This slice does not yet classify the complete declaration-value or selector feature surface; the
mandatory `PCSS-COMPAT-001` warning prevents that gap from becoming false certainty. It also does
not yet:

- enforce cascade conflicts or project complete package/route/component/state/rule ownership into
  audit control groups;
- infer a typed token graph or token-use set from ordinary CSS/custom-property spelling; callers
  must supply a canonical graph explicitly when the accessibility policy needs it;
- infer foreground/background DOM pairing, text size/weight, focus order, keyboard operation,
  replacement-indicator visibility, forced-colors rendering, touch behavior, or JavaScript input
  equivalence;
- ingest directories/import graphs, Sass, Tailwind, or CSS Modules;
- certify WCAG conformance or replace browser/manual evidence.

Those remain explicit E1-E4 and R0 evidence/coverage gates in the execution plan.
