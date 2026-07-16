# ADR-0017: Bind budget ownership to exact Asset Plan bytes

- Status: Accepted; core and CLI implemented, release verification open
- Date: 2026-07-15

## Context

Asset Plan schema 1 already integrity-binds every emitted CSS/manifest pair and preserves separate
route and island bundle selections. Audit can regenerate that plan byte for byte and measure each
bundle, but it cannot infer which deployable package owns a bundle or which islands may render on a
route. The existing `--budget-subject package=...` assertion therefore attributes the complete plan
to every named package, and route budgets include only base route bundles.

Filenames, Cargo package names, route names, and source paths are not reliable ownership evidence.
Inferring from them would turn missing adapter knowledge into apparently verified measurements.

Budget membership and Control Manifest attribution also have different algebra:

- package ownership can be a total, exclusive partition of emitted bundles;
- route views intentionally overlap because shared/theme bundles and islands may appear on several
  routes; and
- `rules.byPackage` / `rules.byRoute` in Control Manifest schema 1 are partitions whose values must
  sum exactly to the total rule count.

Publishing overlapping route views as a control partition would be false evidence.

## Decision

Add an explicit, closed `pliego.ownership.json` schema 1 consumed only through
`audit --asset-plan ... --ownership ...`. There is no discovery by filename or directory.

The sidecar binds the exact Asset Plan bytes with `assetPlanBytes` and `assetPlanSha256`, requires
`ownershipCoverage: "adapter-attested-complete"`, assigns every plan bundle to exactly one
`packageId`, and supplies exactly one island composition for every plan route. Route and island IDs
are the namespaced IDs already present in Asset Plan schema 1.

Audit validates ownership only after it has regenerated the Asset Plan from adjacent CSS and
manifest bytes and proved byte equality. Package metrics merge each exclusively owned bundle once.
Route metrics merge the union of base-route bundles and every composed island's bundles once, in
the plan's canonical bundle order. Cross-bundle semantic duplication remains measurable because the
merge operates on full metric inventories rather than adding precomputed totals.

An ownership sidecar participates in the audit configuration ledger with role `ownership`; changing
any byte changes `configHash`. Asset Plan audit rejects competing manual package/route authorities.
Direct-CSS `--budget-subject` remains a separate explicit assertion.

Schema 1 does not project route membership into Control Manifest `rules.byRoute`, because routes are
overlapping budget views rather than a partition. Package projection is also deferred until the
control API can carry a separately validated partition without breaking existing Rust struct
literals or the fixed package-size boundary. Unknown attribution remains explicit.

## Rejected alternatives

### Infer package ownership from paths or Cargo metadata

Rejected because one emitted bundle may combine multiple source locations and a deployment package
need not equal a Cargo crate. CWD-dependent discovery would also break deterministic replay.

### Add ownership fields to Asset Plan schema 1

Rejected because Asset Plan schema 1 is already frozen and generated from compiler/reachability
inputs. Route-to-island composition and deployment package ownership belong to an adapter snapshot,
not to the compiler-owned plan.

### Count route and island bundles independently

Rejected because shared/theme bundles would be counted repeatedly. Deduplication must occur before
metric merge.

### Treat route views as Control Manifest partitions

Rejected because overlapping routes cannot sum to one whole-plan total. A future schema may model
memberships explicitly, but schema 1 must not weaken its partition invariant.

## Consequences

- Adapters must emit one additional deterministic, integrity-bound document.
- Package budgets become exact partitions of emitted bundles instead of whole-plan aliases.
- Route budgets include adapter-attested island composition without double counting.
- Stale, partial, ambiguous, or dangling ownership fails before budget/control publication.
- The new parser/resolver lives in a dedicated publishable crate so the nearly full CLI and control
  archives do not absorb the entire contract.
- The publishable workspace grows from eleven to twelve exact-version crates across six dependency
  waves. The ownership contract has no workspace-crate dependencies and joins wave 1; accounting for
  the existing optional `pliego-css-control` dependency places control in wave 5 and the CLI alone
  in wave 6.

## Verification

The implementation gate requires schema mutation tests, byte/hash mismatch tests, total/exclusive
bundle coverage, total route coverage, dangling route/island rejection, shared-bundle deduplication,
package/route budget observations, ledger/hash drift, compatibility of commands without ownership,
strict Clippy, Rust 1.85, public-API checks, and the complete twelve-package replay.

The 11 core contract tests, Asset Plan ownership CLI E2E, exact-byte `configHash` assertion, checked
example replay, complete workspace, strict Clippy, Rust 1.85, downstream public-API, portability,
and dirty-explicit twelve-package gates pass locally in Debian WSL2. The exact clean-commit package
replay also passes on `6d9fe01`; hosted and registry evidence remain open and are not implied by
this implementation status.
