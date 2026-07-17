# ADR-0021: Project token usage from retained semantic styles

## Status

Accepted.

## Context

The canonical Token Graph describes every token in the selected theme, while Usage Analysis proves
which complete `(bundleId, StyleId)` units were retained. Neither artifact alone can answer whether
a token has a direct retained consumer, is needed only through an alias/derived dependency, or is
unused. Inferring that answer from emitted CSS or custom-property spelling would lose typed identity
and would confuse declaration with use.

Theme-variable pruning also spans a bundle group. A shared theme bundle may need a variable whose
only semantic consumer lives in a route or island bundle; pruning against only the theme bundle's
local styles is incorrect.

## Decision

`bundle --usage-report` additionally emits canonical `pliego.token-usage.json`. Its universe is the
active resolution of the exact canonical Token Graph. Its direct consumers are the retained
bundle-qualified StyleIds from the same immutable selection used for CSS emission. Transitive
references in the winning token sources produce dependency status; all remaining active tokens are
unused.

Statuses are `direct`, `dependency`, and `unused`. A direct token may also be required by another
direct token, so `requiredBy` is explanatory and does not change status precedence. `emitted` means
that this build actually emitted the token's compiler-owned CSS custom property; `customProperty`
only identifies the spelling that would be used. Literal-backed tokens and dependency-only tokens
are not reported as emitted.

Under `--prune-unreachable`, all retained semantic styles across every bundle form one token-reference
set. Any bundle configured with `emit-theme = true` emits that application-wide set. Without pruning,
theme emission remains the complete variable-backed registry surface.

The read-only `pliego-css-tokens explain` command queries an existing report by exact
`KIND.NAME`. It does not compile, scan sources, mutate policy, or infer runtime evidence.

## Consequences

- Token answers are reproducible from the same typed identities and selection as generated CSS.
- A route-only consumer cannot be dropped from a shared pruned theme.
- Unused status is relative to one active theme resolution and one retained build snapshot.
- The report does not authorize token deletion: external authored `var(...)`, other resolver
  selections, and unbuilt applications remain outside its proof boundary.
- Changing the report shape, status derivation, or query identity requires a schema version change.

## Rejected alternatives

### Scan final CSS for custom-property names

Rejected because CSS spelling cannot recover typed TokenIds, aliases, or exact StyleId consumers.

### Emit one report per bundle

Rejected because dependencies and global theme emission are application-wide, while consumers
already carry their bundle identity.

### Treat dependency as direct use

Rejected because a resolved alias dependency has no direct retained StyleId consumer.

See the [token-usage report schema](../reference/token-usage-schema-1.md),
[Usage Analysis](../reference/usage-analysis-schema-1.md), and
[canonical Token Graph](../reference/token-graph-schema-1.md).
