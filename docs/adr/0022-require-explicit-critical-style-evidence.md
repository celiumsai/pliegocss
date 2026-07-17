# ADR-0022: Require explicit initial-render evidence for critical CSS

## Status

Accepted for the evidence boundary; CSS projection remains in progress.

## Context

Application reachability proves that a style can participate in a route or island. It cannot prove
that the style is used above the fold, at a paint boundary, or in a particular viewport. Treating all
route CSS as critical would rename reachability as performance evidence and can increase HTML and
transfer cost. CSS coverage without route, viewport, timing, and build identity is equally ambiguous.

## Decision

Critical CSS selection requires a closed `pliegocss-critical-style-capture/1` sidecar. Each positive
capture names one exact route/path, browser profile, viewport, paint boundary, and bundle-qualified
StyleId set. The sidecar binds the canonical pre-selection usage-universe digest and exact
reachability digest. Producers must list known dynamic gaps explicitly.

The compiler verifies every captured style against the immutable retained selection and verifies
that its bundle belongs to the named Asset Plan route. Multiple captures for a route form a union;
absence is never interpreted as proof that a style is globally unused. Full route stylesheets remain
required as the correctness fallback.

The future projection emits whole StyleId rule sets and their directly required theme variables. It
does not slice declarations or copy arbitrary final-CSS byte ranges. Output will remain separate
from the framework-neutral Asset Plan because inlining, CSP, URL, and delivery policy belong to the
consumer.

## Consequences

- Critical selection is reproducible for one build and declared capture matrix.
- Stale build hashes, route drift, cross-route bundles, and pruned StyleIds fail closed.
- A sampled capture can improve first-render delivery without becoming a deletion claim.
- Capturing mobile and desktop states is explicit; the compiler does not claim universal coverage.
- Hosted browser capture and measured before/after paint evidence remain release gates.

## Rejected alternatives

### Mark every route style critical

Rejected because reachability is not initial-render use and can over-inline entire route bundles.

### Infer critical rules from selectors or source order

Rejected because neither proves DOM match, viewport state, or a paint boundary.

### Use unbound browser CSS coverage

Rejected because coverage without exact build and route identity becomes stale silently.

See the [critical-style evidence schema](../reference/critical-style-evidence-schema-1.md),
[Usage Analysis](../reference/usage-analysis-schema-1.md), and
[Asset Plan](../reference/asset-plan-schema.md).
