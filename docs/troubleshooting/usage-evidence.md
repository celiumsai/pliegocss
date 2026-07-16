# Troubleshooting usage evidence

## `invalid usage analysis`

One of the closed invariants failed: malformed/unknown fields, invalid identity, duplicate origin,
noncanonical generated state, selection mismatch, or a defensive limit. Regenerate with the same
PliegoCSS version; do not edit generated state fields manually.

## `usage origin has no component`

The reachability sidecar claims complete coverage but omitted or moved an exact source range. Rebuild
the sidecar from the same source snapshot. Paths and half-open ranges must match exact UTF-8 bytes;
containment or filename-prefix matching is never used.

## Observation hash mismatch

`universeSha256` or `reachabilitySha256` no longer matches. CSS identities, theme/configuration,
source provenance, bundle assignment, or application topology changed after observation. Recollect
the snapshot; changing the hash by hand would detach the evidence.

## Observation references a style outside the universe

The producer emitted a stale bundle/StyleId pair or used an identity from another bundle. StyleIds
are bundle-qualified in this schema. Regenerate instrumentation from the current report.

## `invalid usage retention`

The sidecar is not closed/canonicalizable: it may be empty, over a defensive limit, contain an
unknown or missing field, use an invalid digest/ID/StyleId, repeat an `id` or `(bundleId, StyleId)`,
or omit a non-empty justification. Rebuild it with `pliego-css-usage`; do not add wildcards,
selectors, class names, source paths, dates, or expiry fields.

## Retention hash mismatch

`usage retention universeSha256 does not match current universe` means the complete pre-pruning
bundle/style/origin inventory changed. `usage retention reachabilitySha256 does not match exact
reachability input` means the raw reachability bytes changed. Regenerate and re-review the policy
against the current report; editing only the digest would detach the approval from its evidence.

## Retention references an unknown or non-dead style

An unknown-style error means the exact `(bundleId, StyleId)` is absent from the bound universe.
`requires a structurally unreachable style` means the target is reachable, mixed-origin, or lacks
the required complete structural proof. Fix application topology when the style belongs to a real
route/island; retention is only for reviewed styles that remain structurally unreachable.

## `usage observation contradicts unreachable style`

The same bound snapshot says a style was positively observed and that all of its exact origins are
outside every route/island root. Treat this as stale or incomplete adapter evidence. PliegoCSS fails
closed and leaves the previous output group untouched.

## Many entries are `unobserved`

This is not a deletion list. Check declared routes, authentication, feature flags, themes,
viewports, browsers, interaction states, locales, SSR/island hydration, and dynamic inputs. Keep the
entries unless complete structural reachability independently proves them dead.

## Every entry is `unknown/blocked`

The report was generated without exact `--reachability`. Supply a complete adapter sidecar and
manifest schema 4/5. Do not replace missing application evidence with filename or selector guesses.

## A dead StyleId still appears in output

Without `--prune-unreachable`, `candidate` is report-only and all compiled styles remain selected.
Add the explicit pruning flag only after review. Theme custom properties remain global and are not
pruned by either usage-analysis schema.

With `--retention`, a reviewed entry intentionally remains in output as
`removalDisposition: policy-retained`. It must still read `staticReachability: unreachable` and
`usageState: dead`; policy does not rewrite those facts. Check the exact bundle ID because the same
StyleId in another bundle is a distinct entry.

## `--retention` is rejected before compilation

The option is bundle-only and single-use. It requires `--usage-report`, `--prune-unreachable`, and
exact `--reachability`. The sidecar must be an existing regular UTF-8 file inside the bundle-plan
directory, cannot traverse a symbolic link/reparse point, and cannot alias another input or output.

## A shared StyleId remains reachable

This is expected when any exact origin belongs to a route/island root. PliegoCSS removes a whole
StyleId only when every origin in that bundle occurrence is unreachable; it never discards the dead
origin of a retained shared identity independently.

## `--check` reports drift

`--check` is intentionally read-only. Restore the reviewed artifact or rerun the command without
`--check` after validating all input/hash changes. A failing check must not repair or partially
publish files.

See [usage analysis schemas 1 and 2](../reference/usage-analysis-schema-1.md),
[usage retention schema 1](../reference/usage-retention-schema-1.md),
[ADR-0018](../adr/0018-separate-reachability-observation-and-removal.md), and
[ADR-0019](../adr/0019-retain-dead-styles-by-explicit-policy.md).
