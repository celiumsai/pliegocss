# Ownership sidecar schema 1

Status: **implemented and locally replayed through the public core and `audit --ownership`**

Ownership schema 1 is an explicit, audit-only input that binds organizational package ownership and
route-to-island composition to one exact supported Asset Plan schema 1 or 2 document. Ownership
itself remains schema 1: it does not change bundle output, infer application structure, or replace
the reachability and retention inputs that determined the Asset Plan.

The CLI seam is explicit; `pliego.ownership.json` is never discovered:

```console
pliego-cssc audit \
  --asset-plan dist/assets/pliego.assets.json \
  --ownership pliego.ownership.json \
  --budget-policy pliego.budgets.json \
  --targets baseline-widely
```

The public `pliego-css-ownership` parser/builder and its 12 closed-contract tests are implemented.
The Asset Plan CLI E2E gate and an explicit audit of the checked example also pass in Debian WSL2.
That is checkout-local evidence; it is not a hosted-CI or release claim.

## Complete shape

All fields are required. The byte count and digest below bind the exact schema-1 Asset Plan generated
by the repository's PliegoRS smoke fixture at the time this example was refreshed. Binding a schema-2
plan uses the same ownership shape and changes only these exact byte-integrity values as required by
the bound plan.

```json
{
  "schemaVersion": 1,
  "ownershipCoverage": "adapter-attested-complete",
  "assetPlanBytes": 2808,
  "assetPlanSha256": "44f80aea91a016d298a814da551165fb488c66341b14f09feacf179809a554c8",
  "bundlePackages": [
    {
      "bundleId": "island-visit-counter",
      "packageId": "counter-feature"
    },
    {
      "bundleId": "route-home",
      "packageId": "site-pages"
    },
    {
      "bundleId": "route-visit",
      "packageId": "site-pages"
    },
    {
      "bundleId": "shared",
      "packageId": "app-shell"
    },
    {
      "bundleId": "unreachable",
      "packageId": "experiments"
    }
  ],
  "routeCompositions": [
    {
      "routeId": "route:home",
      "islandIds": []
    },
    {
      "routeId": "route:visit",
      "islandIds": [
        "island:visit-counter"
      ]
    }
  ]
}
```

Schema 1 objects are closed. A decoder must reject unknown or missing fields, unsupported versions,
duplicate records, noncanonical identifiers, dangling references, incomplete coverage, and values
outside its defensive limits.

## Fields

| Field | Contract |
|---|---|
| `schemaVersion` | Must be `1`. |
| `ownershipCoverage` | Must be `adapter-attested-complete`. The adapter attests both complete bundle-to-package ownership and complete route-to-island composition for the bound application snapshot. |
| `assetPlanBytes` | Exact byte length of the supported Asset Plan schema 1 or 2 file, including its final LF newline. |
| `assetPlanSha256` | Exactly 64 lowercase hexadecimal characters: SHA-256 of those exact Asset Plan schema 1 or 2 bytes. |
| `bundlePackages` | Total and exclusive mapping from every bound Asset Plan bundle to one package budget subject. |
| `routeCompositions` | Total mapping from every bound Asset Plan route to the island types that may render on it. |

Each `bundlePackages[]` object has exactly:

| Field | Contract |
|---|---|
| `bundleId` | Exact `bundles[].id` from the bound Asset Plan. |
| `packageId` | Stable package budget subject: 1–128 ASCII letters, digits, hyphens, or underscores. It is an accounting identity, not an inferred Cargo package. |

Each `routeCompositions[]` object has exactly:

| Field | Contract |
|---|---|
| `routeId` | Exact namespaced `routes[].id` from the bound Asset Plan, such as `route:visit`. |
| `islandIds` | Unique exact namespaced `islands[].id` references from the same plan. An empty array is required when the route has no islands. |

## Exact Asset Plan binding

The audit consumer reads the selected Asset Plan as bounded raw bytes, parses the supported closed
schema-specific structure, and proves every adjacent CSS/manifest pair through byte-identical
canonical regeneration before trusting ownership. It then reads the explicitly selected sidecar as
bounded raw bytes and requires its `assetPlanBytes` and `assetPlanSha256` to equal the retained plan
bytes before resolving any package, route, or island reference.

The Asset Plan parser accepts exactly these schema/selection pairs; each line below is a minimal
excerpt, not a complete closed plan:

```json
{ "schemaVersion": 1, "ruleSelection": "all-compiled" }
{ "schemaVersion": 1, "ruleSelection": "reachable-style-ids" }
{ "schemaVersion": 2, "ruleSelection": "reachable-or-retained-style-ids" }
```

Schema 1 accepts only `all-compiled` or `reachable-style-ids`; schema 2 accepts only
`reachable-or-retained-style-ids`. Schema 1 with the retained selection, schema 2 with either
schema-1 selection, and every other version/selection combination fail closed. A schema-2 Asset Plan
does not require an Ownership schema bump because
the ownership field set and aggregation model are unchanged; exact raw-byte binding prevents a
schema-1 ownership snapshot from being silently reused for a different schema-2 plan.

A semantically equivalent but byte-different Asset Plan of either supported schema is a different
snapshot and requires a regenerated ownership sidecar. The sidecar hashes the plan, not itself. The
audit control input ledger separately records the exact ownership bytes under the `ownership` role
and includes them in `configHash`.

## Total and exclusive bundle ownership

Let `P` be the set of `bundles[].id` values in the verified Asset Plan and `O` the multiset of
`bundlePackages[].bundleId` values. Validation requires:

```text
set(O) = P
count(bundleId in O) = 1 for every bundleId in P
```

Consequently, a bundle cannot be unowned, owned by two packages, or named only by the sidecar. A
package may own several bundles. Every package represented by schema 1 owns at least one bundle.
Bundles selected by no route or island still require one package owner and participate in that
package's budget.

This is accounting ownership of complete CSS artifacts. Schema 1 cannot split one bundle between
packages, infer package identity from a path, or prove that a Cargo crate produced the bundle.

## Total route composition

The set of `routeCompositions[].routeId` values must equal the set of bound Asset Plan route IDs.
Every island reference must resolve in that same plan. A route with no island still has one record
with `islandIds: []`.

One island may be listed by several routes, and an island may be listed by none. `islandIds` is the
adapter-attested union of island **types** that may render on the route for the bound snapshot. It
does not record instance count, DOM order, conditional frequency, preload policy, or runtime
observation. If an adapter cannot produce that complete union, it must not emit
`ownershipCoverage: "adapter-attested-complete"`.

## Budget projection

Package and route observations are deterministic views over complete Asset Plan bundles:

```text
packageBundles(package) =
  every plan bundle whose unique bundlePackages entry names package

routeBundles(route) =
  stableDeduplicate(
    route.bundles
    + each referenced island.bundles
  )
```

`stableDeduplicate` preserves the canonical top-level Asset Plan bundle order. A global/theme bundle
selected by both a route and one of its islands is measured once. Cross-bundle semantic duplicate
fingerprints are still merged across the resulting unique set.

Package views partition the bundle ledger because ownership is total and exclusive. Route views do
not: shared bundles and shared islands can make routes overlap. Therefore route budget observations
must **not** be published as `Control Manifest rules.byRoute` partitions. That field remains
`unknown` until a future non-overlapping partition contract exists. Budget findings may still name
the exact overlapping route subject and its evidence.

Control Manifest package projection is also deferred in schema 1 even though the resolved package
views are exclusive. `rules.byPackage` remains `unknown` until the control API carries this separately
validated partition without weakening its sum-to-total invariant.

With an ownership sidecar, its typed package and route subjects are the authority. Manual
`--budget-subject package=...` or `route=...` assertions must not silently override or augment them.
Direct `--input` audits retain their existing explicit-subject behavior.

## Canonical order

Canonical ownership documents use this order:

1. root fields in the schema order shown above;
2. `bundlePackages` sorted by `bundleId`;
3. `routeCompositions` sorted by `routeId`; and
4. every `islandIds` array sorted by exact island ID.

Equivalent input array order may be canonicalized by a producer, but duplicates are errors rather
than values to deduplicate. Canonical serialization uses two-space pretty JSON and exactly one final
LF newline. Consumers parse object members by name and must not rely on textual member order.

## Public core and limits

The public `pliego-css-ownership` crate exposes:

- `parse_asset_plan(&[u8]) -> Result<AssetPlan, OwnershipError>` for the closed structural Asset
  Plan schema 1 or 2, including strict schema/selection pairing;
- `parse_ownership(&[u8], &AssetPlan) -> Result<Ownership, OwnershipError>` for exact binding and
  resolved views;
- `BundlePackageInput::new` and `RouteCompositionInput::new` as adapter-facing input DTOs;
- `build_ownership_document(&AssetPlan, &[BundlePackageInput], &[RouteCompositionInput])` to emit
  canonical, validated schema-1 bytes with one final LF;
- canonical plan-order bundle, route, and island accessors; and
- package bundle groups plus composed route IDs, paths, island IDs, and bundle groups.

Parsing a plan does not prove adjacent artifact provenance; the audit caller must still regenerate
and compare the plan before trusting the parsed value.

Both documents are bounded to 16 MiB. Ownership allows at most 65,535 combined mapping records,
composition records, and declared island references, plus a separate cap of 65,535 resolved
composed-route bundle memberships. Asset Plan bundle, route, island, and reference limits remain
those of the supported Asset Plan schema. Limit and integer overflow failures occur before
publishing findings or control artifacts.

## Trust boundary and non-goals

PliegoCSS can verify exact bytes, graph closure, total coverage, exclusive package ownership, and
deterministic aggregation. It cannot independently prove that the adapter described the real
application. This document is build metadata, not authorization.

Schema 1 deliberately does not:

- discover `pliego.ownership.json`, Cargo metadata, package roots, routes, or islands;
- replace reachability schema 1, authorize retention, or change Asset Plan bundle selection;
- infer route composition from island names, source paths, HTML, or runtime telemetry;
- classify an unreferenced bundle as safe to delete, `dead`, or `unobserved`;
- create an island budget subject, preload directive, URL, cache policy, or deployment partition;
- assign partial declarations within a bundle to different packages; or
- turn overlapping routes into non-overlapping Control Manifest partitions.

See [Asset load plan schemas 1 and 2](./asset-plan-schema.md),
[CSS budget policy schema 1](./budget-policy.md), and
[reachability sidecar schema 1](./reachability-schema.md), plus
[usage retention schema 1](./usage-retention-schema-1.md),
[ADR-0017](../adr/0017-bind-budget-ownership-to-asset-plan.md), and
[ADR-0019](../adr/0019-retain-dead-styles-by-explicit-policy.md).
