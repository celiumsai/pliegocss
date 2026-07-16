# Configure package ownership and route composition

Status: **implemented for the public producer/parser and explicit audit integration**

Use an ownership sidecar when an adapter knows both of these facts for one exact Asset Plan:

- which package is accountable for every complete CSS bundle; and
- which island types may render on every declared route.

Do not hand-author this information from filenames when the framework adapter can generate it from
its build graph. PliegoCSS validates the supplied contract but does not discover it.

## 1. Generate the Asset Plan first

Ownership binds exact plan bytes, so produce and verify the schema-1 Asset Plan before writing the
sidecar. For the repository fixture:

POSIX:

```sh
mkdir -p target/ownership-example target/ownership-registry
```

PowerShell:

```powershell
New-Item -ItemType Directory -Path target/ownership-example,target/ownership-registry -Force | Out-Null
```

Then generate the plan:

This example expects generated reachability and plan bytes from the pinned PliegoRS product
registry; do not hand-edit exact site ranges or source partitions.

```console
cargo +1.85 run --locked \
  --manifest-path integration-tests/pliegors-smoke/Cargo.toml \
  --bin pliegocss-pliegors-collector -- \
  integration-tests/pliegors-smoke \
  target/ownership-registry/pliego.reachability.json \
  integration-tests/pliegors-smoke/.pliegocss-bundles.generated.toml

cargo run -p pliego-cssc -- bundle \
  --plan integration-tests/pliegors-smoke/.pliegocss-bundles.generated.toml \
  --output-dir target/ownership-example \
  --manifest-version 5 \
  --reachability target/ownership-registry/pliego.reachability.json \
  --prune-unreachable \
  --asset-plan
```

The exact file is `target/ownership-example/pliego.assets.json`. Do not format, copy through a text
transform, or normalize its line endings before hashing it.

## 2. Record exact bytes and SHA-256

POSIX:

```sh
wc -c < target/ownership-example/pliego.assets.json
sha256sum target/ownership-example/pliego.assets.json
```

PowerShell:

```powershell
$plan = Get-Item target/ownership-example/pliego.assets.json
$plan.Length
(Get-FileHash -Algorithm SHA256 $plan.FullName).Hash.ToLowerInvariant()
```

Copy those values into `assetPlanBytes` and `assetPlanSha256`. Recompute both after any plan byte
changes, even when its route and bundle IDs appear unchanged.

## 3. Assign every bundle exactly once

Read every `bundles[].id` from the verified plan and create one `bundlePackages` record for each.
Package IDs are budget subject IDs, not inferred Cargo package names:

```json
"bundlePackages": [
  { "bundleId": "counter", "packageId": "counter-feature" },
  { "bundleId": "dead", "packageId": "experiments" },
  { "bundleId": "global", "packageId": "app-shell" },
  { "bundleId": "home", "packageId": "site-pages" },
  { "bundleId": "visit", "packageId": "site-pages" }
]
```

Do not omit an unreferenced or fully pruned bundle. Package accounting covers the complete build
ledger, not only route-selected deployment assets. Do not repeat a shared bundle under several
packages; choose one accountable package or split the bundle explicitly in the bundle plan.

## 4. Declare every route's possible islands

Create one `routeCompositions` record for every exact namespaced `routes[].id`. Reference only exact
namespaced IDs from `islands[].id`:

```json
"routeCompositions": [
  { "routeId": "route:home", "islandIds": [] },
  {
    "routeId": "route:visit",
    "islandIds": ["island:visit-counter"]
  }
]
```

For conditional rendering, list the union of all island types that may appear on the route in the
bound application snapshot. CSS is loaded once per bundle, so island instance counts are irrelevant
to this budget contract. If the adapter cannot know the complete union, stop rather than claiming
`ownershipCoverage: "adapter-attested-complete"`.

## 5. Generate canonical bytes from Rust

Rust adapters should use the public producer API instead of hand-serializing JSON:

```rust
use std::fs;

use pliego_css_ownership::{
    BundlePackageInput, RouteCompositionInput, build_ownership_document, parse_asset_plan,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan_bytes = fs::read("target/ownership-example/pliego.assets.json")?;
    let plan = parse_asset_plan(&plan_bytes)?;
    let mappings = [
        BundlePackageInput::new("counter", "counter-feature"),
        BundlePackageInput::new("dead", "experiments"),
        BundlePackageInput::new("global", "app-shell"),
        BundlePackageInput::new("home", "site-pages"),
        BundlePackageInput::new("visit", "site-pages"),
    ];
    let visit_islands = ["island:visit-counter"];
    let compositions = [
        RouteCompositionInput::new("route:home", &[]),
        RouteCompositionInput::new("route:visit", &visit_islands),
    ];
    let ownership = build_ownership_document(&plan, &mappings, &compositions)?;
    fs::write("pliego.ownership.json", ownership)?;
    Ok(())
}
```

The builder derives the exact plan length/hash, sorts mappings and compositions, sorts island IDs,
serializes two-space JSON with one final LF, and parses the result again through the same consumer
contract. It rejects incomplete, duplicate, or dangling adapter data rather than emitting a partial
document.

## 6. Audit explicitly

The ownership file is never searched from the working directory, Asset Plan directory, or package
roots. Select both files explicitly:

```console
cargo run -p pliego-cssc -- audit \
  --asset-plan target/ownership-example/pliego.assets.json \
  --ownership examples/ownership/pliego.ownership.json \
  --targets none
```

The core contract tests and the Asset Plan ownership CLI E2E gate are green in the local Debian WSL2
development environment. The checked example also passes an explicit audit. These results establish
the current workspace behavior; they do not claim hosted-CI or release evidence.

After creating the policy in the next step, add
`--budget-policy config/pliego.budgets.json --format json` to enforce its typed subjects.

Ownership mode must use the sidecar as the only package/route authority. Do not add manual
`--budget-subject package=...` or `route=...` values. Direct CSS `--input` mode keeps its existing
explicit subject contract.

## 7. Budget the typed subjects

Package policies use the exact `packageId`:

```json
{
  "id": "site-pages-package",
  "subject": { "kind": "package", "id": "site-pages" },
  "limits": {
    "bytes": { "maximum": 12000, "baseline": 10800, "maxIncrease": 300 }
  }
}
```

Route policies continue to use the Asset Plan route path, not its namespaced node ID:

```json
{
  "id": "visit-route",
  "subject": { "kind": "route", "id": "/visit" },
  "limits": {
    "bytes": { "maximum": 18000, "baseline": 16400, "maxIncrease": 400 }
  }
}
```

The `/visit` measurement is the unique union of its base bundles and the bundles selected by
`island:visit-counter`. A shared global/theme bundle is counted once. Package `experiments` still
measures its owned `dead` bundle even though no route selects it.

Route observations are overlapping views. A shared bundle may contribute to `/` and `/visit`, so
do not sum route findings into a project total and do not interpret them as Control Manifest
partitions.

## Review checklist

- Asset Plan byte count and SHA-256 were computed from raw generated bytes.
- Every plan bundle appears once and only once in `bundlePackages`.
- Every plan route appears once and only once in `routeCompositions`.
- Every route/island ID is copied exactly, including its namespace.
- Conditional routes list every possible island type.
- Package IDs match budget policy subjects exactly.
- Sidecar arrays are in canonical order.
- No manual package/route subject competes with the sidecar.

See the complete [ownership schema 1 reference](../reference/ownership-schema-1.md),
[budget guide](./configure-budgets.md), and
[ownership troubleshooting guide](../troubleshooting/ownership-sidecar.md).
