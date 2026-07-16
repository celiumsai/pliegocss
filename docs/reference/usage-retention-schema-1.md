# Usage retention sidecar schema 1

Status: **implemented explicit bundle input; complete dirty-explicit package replay green, clean
release evidence pending**

A retention sidecar preserves reviewed, structurally dead bundle StyleIds during explicit
reachability pruning. It is policy, not application or runtime evidence: retained styles remain
`staticReachability: unreachable` and `usageState: dead`.

```json
{
  "schemaVersion": 1,
  "universeSha256": "64-lowercase-hex-digits",
  "reachabilitySha256": "64-lowercase-hex-digits",
  "entries": [
    {
      "id": "external-email-renderer",
      "bundleId": "application",
      "styleId": "32-lowercase-hex-digits",
      "justification": "Consumed by the external mail renderer outside application routing."
    }
  ]
}
```

Explicit use:

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --manifest-version 5 \
  --reachability pliego.reachability.json \
  --prune-unreachable \
  --usage-report \
  --retention pliego.retention.json
```

`--retention` is a single-use `bundle` option. It requires `--prune-unreachable`, exact
`--reachability`, and `--usage-report`; it does not require observation input. The file is never
auto-discovered. It must already exist as a regular UTF-8 file inside the bundle-plan directory,
must not traverse a symbolic link or reparse point, and must not alias an input or generated output.

## Entry identity

The effective key is `(bundleId, StyleId)`. `id` is a stable review identifier, not a selector.
Entries are canonically ordered by bundle ID and StyleId; duplicate keys or IDs fail.

- `id` is lowercase ASCII kebab-case, starts and ends with an alphanumeric character, and is at
  most 128 bytes;
- `bundleId` follows the Asset Plan identifier contract;
- `styleId` is exactly 32 lowercase hexadecimal characters; and
- `justification` is required, contains no control characters, and is at most 4 KiB.

There are no wildcards, class names, selectors, source paths, routes, dates, or clock-driven expiry.
Those forms are ambiguous, unstable, or nondeterministic relative to the reviewed universe.

## Integrity binding

`universeSha256` identifies the canonical complete pre-pruning `(bundleId, StyleId)` universe used
by Usage Analysis. `reachabilitySha256` identifies the exact raw reachability bytes, including
otherwise insignificant textual changes. Both digests must match the current build.

Every entry must exist in the bound universe and all of its complete origins must be structurally
unreachable. A reachable, mixed-origin, unknown, or missing style fails closed. Retention cannot
repair stale reachability.

## Selection semantics

When the sidecar is valid, pruning selects:

```text
reachable bundle StyleIds union policy-retained bundle StyleIds
```

The policy is bundle-qualified. The same StyleId may be retained in one bundle and removed from
another. Selection always preserves the whole StyleId rule set and all compiler origins.

Usage Analysis schema 2 records retained entries with:

```json
{
  "selected": true,
  "staticReachability": "unreachable",
  "usageState": "dead",
  "retention": {
    "state": "retained",
    "entryId": "external-email-renderer",
    "justification": "Consumed by the external mail renderer outside application routing."
  },
  "removalDisposition": "policy-retained"
}
```

Every other schema-2 style carries:

```json
{ "retention": { "state": "not-retained" } }
```

The report root binds exact retention bytes as `{ "state": "bound", "inputBytes",
"inputSha256", "entries" }`, and `summary.removalPolicyRetained` counts the exact retained entries.

Observation remains independent. A positive observation of an entry proven unreachable in the
same snapshot is contradictory and aborts even if the policy retains it.

## Artifact versioning

Retention changes the output selection to `reachable-or-retained-style-ids`. That value belongs to
Usage Analysis schema 2 and, when requested, Asset Plan schema 2 and Project Index schema 2. The
ownership parser accepts the resulting Asset Plan only under that exact schema/selection pair.
Modes without retention continue emitting their established schema-1 bytes.

A retained bundle is not invented into route or island selections. If a style is actually owned by
an application route, the adapter must correct reachability instead of using retention.

## Validation and limits

- all objects are closed and all fields are required;
- digests are exactly 64 lowercase hexadecimal characters;
- the document is at most 16 MiB;
- at most 65,535 entries are accepted;
- arrays are canonical and duplicates fail; and
- canonical builder output is two-space pretty JSON with one final LF.

When `bundle --control` is active, the exact file enters the config ledger with role
`usage-retention`, participates in `configHash`, and remains separate from `usage-observation`.
`--check` reuses the explicit file and compares the complete expected output group without repair.

See [ADR-0019](../adr/0019-retain-dead-styles-by-explicit-policy.md),
[usage analysis](./usage-analysis-schema-1.md), and
[reachability schema 1](./reachability-schema.md).
