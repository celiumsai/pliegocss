# Troubleshoot ownership sidecars

Status: **implemented fail-closed parser and CLI integration**

Ownership failures are intentionally fail-closed. Do not bypass a stale or incomplete sidecar with
manual package/route subjects: regenerate it from the adapter snapshot that produced the Asset Plan.

## The ownership file is ignored or `--ownership` is unknown

`pliego.ownership.json` has no discovery behavior. The implemented interface requires both:

```console
pliego-cssc audit --asset-plan PATH --ownership PATH
```

If a binary reports `unknown audit option --ownership`, it predates this contract. Use a revision
that contains the ownership crate and CLI integration; do not assume the filename is consumed
automatically.

## Asset Plan byte count or digest differs

The sidecar binds raw Asset Plan bytes, including the final newline. Common causes include:

- recompiling CSS or manifests;
- changing pruning, targets, format, theme, topology, or bundle order;
- regenerating with a different compiler revision;
- formatting or line-ending conversion after generation; or
- pointing `--asset-plan` at another output directory.

Regenerate the ownership file from the selected plan. Do not edit `assetPlanBytes` or
`assetPlanSha256` until the plan's own adjacent CSS/manifest integrity and canonical regeneration
also pass.

## A bundle is missing

`bundlePackages` covers the complete top-level Asset Plan bundle ledger, not only bundles selected
by a route. Include fully pruned and unreferenced bundles. An omitted `dead` bundle is still an
incomplete ownership contract.

## A bundle has more than one package

Package ownership is exclusive. Shared use by several routes or components does not create shared
accounting ownership. Choose one accountable `packageId`, or change the declarative bundle plan so
each package receives a separate complete artifact.

Do not duplicate one `bundleId` under several packages and do not attempt percentage ownership;
schema 1 has no partial-bundle allocation.

## A bundle, route, or island is unknown

References are exact and case-sensitive:

- `bundleId` matches `bundles[].id`;
- `routeId` matches the namespaced Asset Plan ID such as `route:visit`; and
- `islandIds[]` matches namespaced IDs such as `island:visit-counter`.

Raw reachability IDs such as `visit` or `visit-counter` are not the Asset Plan IDs. A mismatch often
means the ownership sidecar and plan were produced from different application snapshots.

## A route with no islands is rejected

Coverage is total. Keep the route record and use an empty list:

```json
{
  "routeId": "route:home",
  "islandIds": []
}
```

Omitting the route does not mean “no islands”; it means the adapter did not attest the route.

## A conditional island is absent from the route total

`islandIds` is the union of island types that may render, not a sample of one request. Include a
conditionally rendered island whenever the bound build can place it on the route. Runtime frequency
and instance count do not change CSS bundle membership.

If the adapter cannot enumerate the complete union, it cannot truthfully emit
`ownershipCoverage: "adapter-attested-complete"`. Keep the budget unavailable rather than producing
a falsely small route total.

## A route budget is larger than expected

The route observation is:

```text
base route bundles ∪ bundles for every declared island
```

Bundle IDs are deduplicated before metrics are merged, so a global/theme bundle referenced by both
the route and an island is counted once. The remaining increase is normally the island's distinct
CSS or cross-bundle duplicate evidence. Inspect the Asset Plan's separate route/island selections
and the ownership composition before raising the limit.

## Route totals do not add up to the project total

This is expected. Routes are overlapping views: shared bundles and shared islands can contribute to
several route subjects. Route observations are valid budget subjects but are not a partition and
must not be emitted as `Control Manifest rules.byRoute` ownership. Use exclusive package views or
the complete Asset Plan ledger for non-overlapping accounting.

## One or more package/route budgets match no subject

Every policy subject must match a verified observation. The package subject ID must equal
`packageId` exactly, while route policies use the Asset Plan route path rather than its namespaced
`routeId`. Also verify that the package owns at least one bundle and that the audit received the
ownership file explicitly. `PCSS-BUDGET-199` fails even when other policy definitions matched. Do
not restore matching by adding `--budget-subject package=...`; that would create a second authority.

## Reordering changed review bytes

Canonical producers sort `bundlePackages` by `bundleId`, `routeCompositions` by `routeId`, and each
`islandIds` list by island ID. Reorder the source into canonical form before review. Duplicates remain
errors and must not be silently removed.

The sidecar binds the exact Asset Plan, not the textual order of semantically equivalent ownership
records. Audit/control ledgers may still hash the exact ownership input bytes for reproducibility.

## Security boundary

Hashes detect drift relative to the selected sidecar; they do not authenticate an attacker-controlled
Asset Plan and ownership file replaced together. Treat both as build inputs from one trusted
pipeline. Ownership metadata is not route authorization, package registry proof, dead-code proof,
or runtime telemetry.

See [ownership schema 1](../reference/ownership-schema-1.md) for the complete closed contract.
