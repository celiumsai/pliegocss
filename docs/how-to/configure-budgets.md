# Configure CSS budgets without hiding regressions

Start with one exact CSS artifact and one ownership subject. A useful budget has two independent
boundaries:

- `maximum` is the long-term ceiling;
- `baseline + maxIncrease` is the short-term regression ceiling.

If current canonical CSS is 14,200 bytes, a policy can allow normal churn without allowing every
change to consume the long-term headroom:

```json
"bytes": {
  "maximum": 18000,
  "baseline": 14200,
  "maxIncrease": 300
}
```

The artifact fails above 14,500 bytes even though it remains below 18,000. Update `baseline` only
after reviewing the exact change and its policy finding.

## Choose attribution deliberately

- Use `file` for source/build artifacts whose path is the ownership boundary.
- Use `package` only with `--budget-subject package=NAME` from an adapter or command that knows the
  complete package artifact.
- With one CSS file, use `route` only with `--budget-subject route=/PATH` when that file contains the
  complete route CSS.
- With compiler-generated bundles, use `--asset-plan` for automatic file/layer subjects and add an
  explicit ownership schema 1 companion whenever the policy contains package or route subjects.
- The ownership sidecar supplies the total/exclusive bundle-to-package map and composes every route
  with its declared islands. It is the sole package/route authority in Asset Plan mode; manual
  `--budget-subject` values are rejected there.
- Use `layer` for named parsed cascade layers; PliegoCSS detects these automatically and combines
  repeated blocks with the same full layer name.

Do not label one route bundle as the total route. Ownership-backed Asset Plan mode adds every base
bundle selected by that route plus every bundle selected by its declared islands, deduplicates the
union by bundle ID, and detects duplicate declarations across bundle boundaries. Ownership schema 1
provides that adapter attestation without changing Asset Plan schema 1: it binds exact plan bytes and
covers every bundle/package and route/island relationship.

The implemented command for typed package and composed-route subjects is:

```console
pliego-cssc audit --asset-plan dist/assets/pliego.assets.json \
  --ownership pliego.ownership.json --targets baseline-widely \
  --budget-policy pliego.budgets.json
```

Ownership is explicit and never discovered. It is the sole package/route authority; manual
package/route subjects must not compete with it. Composed routes may overlap and are budget
observations, not Control Manifest partitions.

## Require total policy coverage

Every `budgets[]` definition must match one verified observed subject. PliegoCSS emits
`PCSS-BUDGET-199` and fails when even one file, layer, package, or route definition is stale or
misspelled; a partially matched policy never passes. Compare the finding's `declared-budgets`,
`matched-budgets`, and `observed-subjects` evidence before changing the policy.

## Keep specificity as a tuple

Specificity is `id,class-or-attribute,type`, for example `"0,3,1"`. Avoid scalar scores such as 31:
they erase CSS's lexicographic precedence. A permitted increase is another tuple:

```json
"specificity": {
  "maximum": "1,3,1",
  "baseline": "0,3,1",
  "maxIncrease": "0,1,0"
}
```

This regression boundary allows at most `0,4,1`; an ID selector still exceeds it because the first
component wins lexicographically.

## Treat duplication as a review signal

`semanticDuplicates` counts extra style rules with the same ordered normalized declarations and the
same ancestor condition, layer, and nesting context. It excludes selector identity so sibling rules
that could potentially be grouped are visible. It does not prove that rules are dead, unused, or
safe to remove.

Review selectors and cascade behavior before consolidating. Different conditions or layers have
different fingerprints and do not inflate the count.

## Use exceptions transparently

```json
"exceptions": [
  {
    "id": "reviewed-checkout-growth",
    "budget": "checkout-route",
    "metrics": ["rules", "bytes"],
    "justification": "temporary dual checkout retained through experiment EXP-42"
  }
]
```

An exception must point to an existing configured metric and cannot overlap another exception. It
changes an enforced error into `PCSS-BUDGET-102`, but retains the overage, policy hash, and
justification. Schema 1 intentionally has no clock-dependent expiry; external review automation can
track dates until receipts gain a deterministic evaluation-date contract.

Run the [audit example](../../examples/audit/README.md), then use the complete
[budget policy reference](../reference/budget-policy.md). The ownership workflow is in
[configure ownership](./configure-ownership.md).
