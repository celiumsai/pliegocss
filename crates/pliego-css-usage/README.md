# pliego-css-usage

Closed contracts for PliegoCSS usage analysis, runtime-observation sidecars, and explicit retention
policy. The crate classifies every compiler origin against exact application reachability evidence,
keeps `observed`, `unobserved`, `dead`, and `unknown` distinct, and never treats sampled absence as
proof of removability.

Use `build_usage_observation` and `build_usage_retention` in adapter tooling. Use
`prepare_usage_analysis` to derive one immutable bundle-qualified `UsageSelection` before emission,
then serialize the matching report with `PreparedUsageAnalysis::build_analysis`. Schema 1 covers
all-compiled or reachable-only output. Schema 2 covers the reachable union plus exact reviewed dead
StyleIds; retained entries remain structurally `dead` and are marked `policy-retained`.

`parse_usage_analysis` proves the closed report shape and self-contained derived invariants. It
cannot authenticate external files represented only by digests. Consumers that possess the exact
compiler universe and sidecar bytes use `verify_usage_analysis`, which rederives the canonical
report and requires byte-for-byte equality.

The crate reexports its exact `AssetRuleSelection` input and enables the narrow
`pliego-css-build/usage-artifacts` feature; observation/retention producer tooling does not pull
Lightning CSS.

`bundle --usage-report` also publishes `pliego.token-usage.json`: a canonical projection of the
active Token Graph over retained StyleIds with `direct`, `dependency`, and `unused` states. Query an
existing report without compilation using `pliego-css-tokens explain --report FILE --token
KIND.NAME [--format text|json]`.

Every sidecar is closed, bounded, canonical, and bound to the complete pre-pruning universe plus
the exact reachability bytes. Selection always operates on a whole `(bundleId, StyleId)` and never
edits authored Rust, individual declarations, or theme variables.

## Stability

Exact-version pre-release tooling; `0.1.0` is not published.
