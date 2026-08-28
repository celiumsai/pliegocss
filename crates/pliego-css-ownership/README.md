# pliego-css-ownership

Closed schema-1 contracts for the PliegoCSS Asset Plan ownership sidecar. The crate validates total
and exclusive bundle-to-package ownership, total route compositions, island references, and an exact
byte-count/SHA-256 binding to the Asset Plan. Resolved package and route views expose deduplicated
bundle IDs in the canonical order of that plan.

Adapters can use `build_ownership_document` with `BundlePackageInput` and
`RouteCompositionInput` to emit deterministic two-space pretty JSON. The producer binds the exact
plan bytes and reuses the parser as its final validation gate.

Parsing an Asset Plan proves only that its closed schema and references are internally valid. A CLI
or adapter must regenerate the plan from its adjacent CSS and manifest inputs and compare the exact
bytes before treating the plan, or an ownership sidecar bound to it, as trusted.

Route compositions are overlapping deployment views. They are not partitions and do not attribute
Control Manifest measurements to routes.

## Stability

Prepared as exact-version public-preview tooling for `0.1.0-rc.3`; final `0.1.0` is not published.
