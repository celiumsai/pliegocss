# R0 unified manifest and receipt design

Status: **schema-1 contract, direct-CSS/Asset-Plan audit, optional audit TokenGraph evidence, bounded
accessibility policy, exact-snapshot compile/watch, generated Source Map v3, and bundle-build
generation/publication implemented; full attribution, runtime/browser accessibility evidence, and
hosted cross-OS integration remain open**

The strategic report requires one inspectable `pliego.css.manifest.json` plus receipts for builds and
authorized changes. Existing style manifests 3-5, Asset Plan 1, and Project Index 1 remain valid
specialized artifacts. They are not renamed, mutated, or treated as if they already contain policy,
violations, backend identity, config hashes, or receipts.

ADR-0016 reserves a new document kind and semantic-versioned schema. The implementation must use the
types and ordering below before `pliegocss audit` can claim R0.

## Unified control manifest

Fixed filename: `pliego.css.manifest.json`

```json
{
  "documentKind": "pliego-css-control-manifest",
  "schemaVersion": "1.0.0",
  "tool": {"name": "pliegocss", "version": "0.x"},
  "inputs": {},
  "backend": {},
  "targets": {},
  "outputs": [],
  "rules": {},
  "tokens": {},
  "decisions": [],
  "violations": {},
  "receipt": {}
}
```

All object keys and collection orders are canonical. Paths are portable logical paths. Hashes use
`sha256:<64 lowercase hex>`. No wall-clock timestamp participates in semantic identity; evidence
dates identify external data snapshots, not build time.

| Section | Required schema-1 content |
|---|---|
| `documentKind` / `schemaVersion` | Exact discriminator and semantic wire version. Unknown major versions fail closed. |
| `tool` | PliegoCSS version plus versions of every contract that affects interpretation. |
| `inputs` | Merkle-like canonical `sourceHash`, `configHash`, source-root identity, policy/config files with individual hashes, typed source-manifest/Project Index hashes, and declared adapter versions. Files sort by logical path. |
| `backend` | Backend name, exact version, enabled stages, and deterministic configuration identity. It must distinguish PliegoCSS work from Lightning CSS work. |
| `targets` | Policy profile, explicit browser vector, official compatibility dataset source/version/date, reset/scope policy, and capability decisions version. |
| `outputs` | Logical file, role, media type, bytes, SHA-256, source-map hash/reference, and any specialized manifest/Asset Plan/Project Index relationship. |
| `rules` | Explicit measured/unavailable observation state; when measured: total bytes, rules, selectors, declarations, maximum specificity, semantic duplicates, and counts by layer, package, route, component, state, and rule type. Unknown attribution is explicit, never silently assigned. |
| `tokens` | Explicit measured/unavailable observation state; when measured: internal graph version/hash, tokens, aliases, derived values, themes, coverage, cycles, declared contrast pairs, deprecations, and DTCG adapter/version/inventory where used. Unavailable never means zero measured tokens. |
| `decisions` | Stable ID, feature/policy, action (`preserved`, `transformed`, `fallback`, `degraded`, `experimental`, `blocked`), reason, source/evidence, affected outputs, and deterministic decision fingerprint. |
| `violations` | Finding schema version, canonical findings document file/hash/counts by code/severity/verification, active exception IDs, and whether human/JSON/SARIF parity passed. Findings may be referenced rather than duplicated. |
| `receipt` | Acyclic receipt expectation: receipt schema/file, canonical required-check IDs, and expected derived result. It never contains a receipt hash or manifest hash. |

The control manifest hashes every output but cannot hash itself. Its embedded receipt expectation
contains no receipt hash and no manifest hash; the adjacent receipt closes the final edge by hashing
the complete manifest. A consumer may hash the receipt bytes externally, but that hash is not
inserted back into either document.

## Build receipt

Fixed default filename: `pliego.css.receipt.json`

```json
{
  "documentKind": "pliego-css-build-receipt",
  "schemaVersion": "1.0.0",
  "tool": {"name": "pliegocss", "version": "0.x"},
  "inputs": {"sourceHash": "sha256:...", "configHash": "sha256:..."},
  "targets": {"profile": "baseline-widely", "dataVersion": "...", "dataDate": "..."},
  "manifest": {"file": "pliego.css.manifest.json", "bytes": 0, "sha256": "sha256:..."},
  "outputs": [],
  "decisions": {},
  "findings": {},
  "checks": [],
  "result": "passed"
}
```

The receipt binds the exact manifest bytes, every output, backend, targets, compatibility-data
identity, decision summary, finding-document hash/counts, exceptions, and checks. Each check records
stable ID, `passed|failed|not-run`, evidence kind, evidence artifact hash when present, and whether it
was required. Browser-required checks cannot be replaced by a static `passed` claim.

`result` is derived: it is `passed` only when every required check passed, no unexcepted error
finding remains, every referenced hash verifies, and all configured budgets pass. A receipt is an
integrity and evidence artifact, not a cryptographic signature; signature support, if added later,
wraps a hash computed over the canonical receipt bytes without changing these semantics.

## Change plan and repair receipt boundary

R1 introduces separate `pliego.css.plan.json` and `pliego.css.change-receipt.json` documents. A plan
contains its hash, finding fingerprints, ranked selected suggestions, exact proposed diff, maximum
files/rules/tokens/bytes changed, risk, prerequisites, and required checks. Applying it requires
explicit authorization and exact input/plan hash agreement.

The change receipt binds plan hash, patch hash, changed counts, before/after source/config hashes,
checks, browser evidence, findings before/after, and output/manifest hashes. Applying the same plan to
the already-updated source must be a no-op, not a second patch.

The implemented bounded slice freezes proposal/plan/dry-run/Change Receipt schema 1.0.0, allows only
verified unexcepted low-risk finding suggestions, enforces exact ranges and change budgets, binds the
original FindingDocument plus source before/after bytes and hashes, and distinguishes complete
`ready` from `already-applied`. `planSha256` and `patch.sha256` detect drift but the plan remains
`dry-run-only`; apply requires the external literal `sha256:<planSha256>`, which selects one exact
plan without authenticating identity. Publication revalidates under a persistent cooperative lock
and stages sources plus receipt as one rollback-capable group. The Change Receipt is deliberately
change-only: `checks-pending`, every required check `not-run`, and browser evidence `not-collected`.
Separate check-policy/Verification Receipt schema 1.4.0 (with canonical
1.0.0/1.1.0/1.2.0/1.3.0 read
support) executes the closed in-process `standard-css-audit`, `token-graph-integrity`, and
identity-bound `css-budget-audit` kinds, and validates canonical `test-suite-evidence` produced by
the explicit fixed-profile Cargo runner plus canonical `browser-evidence` produced by one pinned
PliegoRS Chromium/CDP runner. It derives `passed|failed|blocked` without a policy/plan program/argv,
script, URL, selector, or assertion surface and publishes each complete
after-FindingDocument beside the receipt through a create-if-absent, receipt-last group. Hosted
runner authentication and the Firefox/WebKit matrix remain design requirements rather than
implemented claims.

## Hashing and publication rules

1. Canonicalize and hash source/config/policy/adapter inputs.
2. Run backend, semantic checks, budgets, and required browser evidence.
3. Serialize output artifacts and canonical finding documents.
4. Serialize the control manifest with the embedded receipt summary but no self-hash.
5. Serialize the adjacent receipt containing the exact manifest bytes/hash.
6. Verify every edge from in-memory bytes.
7. Publish the complete output/manifest/receipt group with the existing rollback-capable boundary.

`--check` recomputes all bytes and exits nonzero on drift without writing. A failed build retains the
last valid group; it never pairs new CSS with an old manifest or receipt.

## Implemented schema-1 slice

`pliego-css-control` owns the fixed discriminators and filenames, typed sections, strict
`deny_unknown_fields` decoding, deterministic pretty-JSON serialization, portable-path and SHA-256
validation, canonical collection order, decision-fingerprint verification, an acyclic manifest to
receipt integrity edge, derived receipt results, and the rule that a passed browser check requires
an integrity-bound evidence artifact. Tests freeze complete manifest and receipt byte hashes and cover unknown
fields, noncanonical order, decision tampering, manifest tampering, invalid result derivation, and
unevidenced browser claims.

Direct `audit --input FILE.css --control-dir DIR` now feeds the contract from the real standard-CSS
analyzer and frozen compatibility policy, derives compatibility decisions and finding/check
summaries, publishes canonical findings/manifest/receipt as one rollback-capable group, and supports
`--check` without writes. Syntax failure produces an honest failed receipt; unavailable rules or
tokens carry reasons and cannot carry invented measurements.

Optional `--accessibility-policy FILE` now applies the same deterministic policy to direct CSS. Its
exact bytes enter the config ledger as `accessibility-policy`; optional `--token-graph FILE` requires
that policy, enters as `token-graph`, and supplies canonical evidence for token-backed or
theme-selected contrast relationships. A budget policy retains role `policy`. Accessibility
findings preserve verified/unverified/manual-required boundaries and stable `context.subject-id` for
reviewed exceptions.

`audit --asset-plan FILE.json --control-dir DIR` extends that evidence boundary to the canonically
regenerated plan plus every adjacent CSS/style-manifest pair. It aggregates rule metrics across
bundles, hashes each exact input, relates the finding output to the Asset Plan, and requires a passed
`asset-plan-integrity` receipt check. Route budget observations may overlap through shared bundles,
so they are not misrepresented as a partition of `rules.byRoute`. The Asset Plan has input role
`asset-plan`; the optional accessibility policy evaluates every integrity-bound stylesheet in that
plan.

When `--ownership FILE` is supplied explicitly, its exact bytes use the `ownership` input role and
participate in `configHash`. It can drive exclusive package and overlapping composed-route budget
findings, but neither view is copied into the current Control Manifest partition fields:
`rules.byPackage` and `rules.byRoute` remain `unknown` until a future schema separately validates
their required algebra.

The no-graph direct-CSS control group and a separately invoked audit over an existing
compiler-produced Asset Plan have frozen findings/manifest/receipt hashes that match byte-for-byte
on local Windows x64 and a Linux x64 binary under Debian WSL2. Their `--check` paths are also proven
read-only on both. Supplying `--token-graph` publishes exact canonical `pliego.tokens.json` as a
fourth audit artifact and requires `token-graph-integrity`; policy-only audit preserves the original
three-artifact group.
Generated direct compile and watch now publish seven artifacts after adding
`pliego.tokens.json`; their graph-bearing hashes supersede the earlier six-artifact vector and are
frozen and green locally on Windows and Debian WSL2 Linux x64. Hosted runners and macOS remain open.

`bundle --control` closes the first generated-build group. It snapshots the exact plan, optional
theme configuration or Resolver, reachability document, and Rust sources; audits generated CSS in memory; then
publishes CSS, Source Map v3 sidecars, style manifests, one shared TokenGraph, Asset Plan, optional
Project Index, findings, control manifest, and receipt through one rollback-capable boundary. Output
relationships preserve the specialized
artifact graph, and bundle `--check` is read-only for both ordinary and control-artifact drift. The
one-bundle manifest-5 portability vector with Project Index now contains nine artifacts; its new
graph-bearing hashes are frozen and green on local Windows x64 and Debian WSL2 Linux x64. Hosted
runners and macOS remain open.

Direct compile/build control mode snapshots line inputs, expanded Rust sources, the resolved TOML
theme or explicit DTCG Resolver, and reachability once, then compiles and audits only those bytes.
The Resolver enters the config ledger as `token-resolver`. Inline styles and compositions become
canonical virtual source bytes. Compiler settings, target policy, theme selection/identity,
canonical Resolver selections, formatting, manifest version, and pruning contribute separately to
configHash; selection identity remains bound even when two permutations share a ThemeId and CSS.
CSS, canonical source map, specialized manifest, `pliego.tokens.json`, findings, control manifest,
and receipt publish as one rollback-capable
group under a common control root; `--check` is read-only. Watch uses the same builder for every
confirmed snapshot and retains the last valid complete group after invalid input.

Generated compile/watch and bundle control manifests publish a bounded
`pliegocss-token-graph/1`. Its hash covers exact canonical `pliego.tokens.json` bytes; coverage starts
with retained token references, follows winning alias/derived edges transitively, and unions compiled
assets for bundles. The schema and DTCG Resolver retain aliases, derived values, deprecations,
provenance, selections, and validated theme permutations.

Audit-side graph measurement has no retained compiler-use set. It counts the union of declared
projected tokens across themes and records `coverageBasisPoints: 0`; that is measured graph evidence,
not `unavailable` and not a claim that the application uses zero tokens. Its `contrastPairs` value
counts accessibility-policy declarations, not theme fan-out. Without explicit `--token-graph`, audit
keeps token observation unavailable rather than reconstructing a graph from arbitrary CSS.

TOML/seed CLI input projects one literal `default` theme. Explicit `--tokens FILE` instead publishes
the complete Resolver graph and uses `--token-input` only to select the active permutation; exact
Resolver bytes and canonical selections are integrity-bound. Bundle-plan schema 2 carries the same
complete graph through exact plan and `token-resolver` bytes with local Windows/WSL E2E, workspace,
Rust 1.85, rollback/read-only, and package gates green. Exact plan
bytes—including selection spellings—participate in `configHash`, so textually distinct bundle plans
do not promise semantic convergence even when they resolve to the same ThemeId/CSS. Cargo
build/macro DTCG selection now writes only the selected registry for macro identity; the matching
controlled CLI invocation must repeat the Resolver/inputs and owns complete-graph publication. Its
local workspace/MSRV/API and clean-package gates pass. Direct CSS and reopened Asset Plan audit cannot
reconstruct a typed graph, but may now ingest and republish an explicitly supplied canonical graph.
Declared accessibility relationships and graph-backed contrast are implemented for this bounded
surface. R0.5 remains partial where complete producer attribution and hosted evidence are still
required.

R0.6 now has a deterministic static slice: declared opaque in-gamut sRGB contrast, exact
motion-preference guards, explicit focus-outline suppression, forced-color adjustment, and
same-rule hover/focus declaration equivalence. Unproven cascade and cross-context relations remain
manual-required. The policy independently maps violation, unverified, and manual-required
results to `fail|warn`; reviewed exceptions remain visible. This does not infer DOM color pairing,
text size/weight, focus order, replacement-indicator visibility, keyboard JavaScript behavior,
forced-colors rendering, or WCAG conformance. Runtime, browser, assistive-technology, and manual
evidence therefore remain R0.6 gates rather than being relabeled as static success.

Generated compile/watch and bundle control groups emit one canonical Source Map v3 per CSS artifact.
Every CSS `sourceMap` reference resolves to an exact `css-source-map` output in the same manifest;
the receipt binds both. Segments map final class-selector starts to a deterministic primary authored
origin using UTF-16 line/column units. The numbered style manifest retains all origins, and schema 5
retains exact declaration-level physical lineage. CSS has no injected discovery comment, so its
bytes and ordinary compile/watch behavior remain unchanged.

## Open implementation gates

- Feed a separately validated package partition plus component/state/rule attribution rather than
  audit-mode `unknown`. Ownership schema 1 route compositions are overlapping budget views and must
  not populate the current partition-valued `rules.byRoute`; route memberships need a future
  versioned control shape.
- Add browser/manual evidence for accessibility properties that static CSS cannot prove; never treat
  `PCSS-A11Y-000` as WCAG certification.
- Run all frozen control vectors on hosted Windows/Linux/macOS runners.
