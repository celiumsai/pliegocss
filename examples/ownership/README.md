# Ownership schema 1 example

Status: **core-validated and locally replayed through `audit --ownership`**

[`pliego.ownership.json`](./pliego.ownership.json) binds the Asset Plan generated from the existing
PliegoRS smoke fixture. It demonstrates all schema-1 relationships without inventing discovery:

- all five bundles have exactly one package;
- `route:home` has no islands;
- `route:visit` may render `island:visit-counter`;
- the `unreachable` bundle remains owned by `experiments` even though no route selects it; and
- shared CSS is deduplicated in the composed `/visit` route view.

## Regenerate the bound plan

From the repository root:

POSIX:

```sh
mkdir -p target/ownership-example target/ownership-registry
```

PowerShell:

```powershell
New-Item -ItemType Directory -Path target/ownership-example,target/ownership-registry -Force | Out-Null
```

Generate reachability and the bundle plan from the pinned PliegoRS product registry, then compile it:

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

The collector requires the revision-pinned PliegoRS checkout documented by the integration gate.
Delete the generated hidden plan after use; it is a build artifact, not checked-in configuration.

The checked example currently records an Asset Plan length of `2808` bytes and SHA-256
`44f80aea91a016d298a814da551165fb488c66341b14f09feacf179809a554c8`. These values are exact,
not permanent. A compiler, fixture, manifest, CSS, or topology change can legitimately replace
them.

Verify or refresh on POSIX:

```sh
wc -c < target/ownership-example/pliego.assets.json
sha256sum target/ownership-example/pliego.assets.json
```

Or in PowerShell:

```powershell
$plan = Get-Item target/ownership-example/pliego.assets.json
$plan.Length
(Get-FileHash -Algorithm SHA256 $plan.FullName).Hash.ToLowerInvariant()
```

Update both `assetPlanBytes` and `assetPlanSha256` together. Never replace only the digest to silence
a failure: first verify the Asset Plan against all adjacent CSS and manifest files.

Production adapters should call `pliego_css_ownership::build_ownership_document` with
`BundlePackageInput` and `RouteCompositionInput`. The builder derives these exact plan fields,
canonicalizes record order, appends one LF, and validates its own output; the commands above remain
useful for reviewing or refreshing this checked example.

## Audit the composition

The checked example is replayable without a budget policy:

```console
cargo run -p pliego-cssc -- audit \
  --asset-plan target/ownership-example/pliego.assets.json \
  --ownership examples/ownership/pliego.ownership.json \
  --targets none
```

This exact plan/sidecar pair passed locally in Debian WSL2. Add
`--budget-policy config/pliego.budgets.json --format json` to enforce typed package and composed-route
subjects from a project policy; no policy is auto-discovered.

The expected bundle views are:

| Subject | Unique bundles in Asset Plan order |
|---|---|
| package `app-shell` | `shared` |
| package `counter-feature` | `island-visit-counter` |
| package `experiments` | `unreachable` |
| package `site-pages` | `route-home`, `route-visit` |
| route `/` | `shared`, `route-home` |
| route `/visit` | `shared`, `island-visit-counter`, `route-visit` |

Routes overlap through `shared`; they are budget observations, not Control Manifest partitions.

See the [ownership reference](../../docs/reference/ownership-schema-1.md) and
[configuration guide](../../docs/how-to/configure-ownership.md).
