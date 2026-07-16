# Usage analysis schemas 1 and 2

Status: **implemented opt-in bundle report and policy-retention model; exact clean package replay
green at commit `8f83036`, registry and hosted release evidence pending**

`bundle --usage-report` emits one deterministic, read-only artifact:

```text
OUTPUT_DIR/pliego.usage.json
```

The report inventories the complete pre-pruning `(bundleId, StyleId)` universe. It deliberately
does not reuse the post-pruning Project Index as its source: a selected-only manifest cannot explain
which complete StyleIds were excluded.

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --manifest-version 5 \
  --reachability pliego.reachability.json \
  --usage-report
```

Add `--prune-unreachable` to select only reachable StyleIds while retaining their tombstone evidence
in the report. Add `--observations pliego.observations.json` only when a producer has collected the
explicit [observation sidecar](./usage-observation-schema-1.md). Observation input requires both
`--usage-report` and exact `--reachability` evidence and is never discovered.

Add `--retention pliego.retention.json` only to a pruning build that must preserve reviewed,
structurally dead StyleIds for an external consumer. It requires `--usage-report`,
`--prune-unreachable`, exact `--reachability`, and a non-empty closed
[retention sidecar](./usage-retention-schema-1.md). This switches the report and selected Asset
Plan/Project Index artifacts to schema 2; it does not rewrite reachability or usage evidence.

## Separate evidence, verdict, policy, and output state

Neither schema treats static reachability as runtime observation. Retention is an explicit policy
overlay, not a fifth kind of evidence:

| Field | Values | Meaning |
|---|---|---|
| `staticReachability` | `reachable`, `unreachable`, `unknown` | Whether exact origins connect to any route/island root in a complete reachability graph. |
| `observationState` | `observed`, `unobserved`, `unavailable` | Whether the bound observation snapshot contains a positive hit, contains no hit, or was not collected. |
| `usageState` | `observed`, `unobserved`, `dead`, `unknown` | Derived snapshot-scoped verdict. |
| `removalDisposition` | `retain`, `blocked`, `candidate`, `removed`, `policy-retained` | What generated-output handling occurred or is justified. |

Derivation is deterministic:

| Static | Observation | Usage | Removal without pruning | Removal with pruning |
|---|---|---|---|---|
| reachable | observed | observed | retain | retain |
| reachable | unobserved | unobserved | retain | retain |
| reachable | unavailable | unknown | retain | retain |
| unreachable | unobserved or unavailable | dead | candidate | removed |
| unknown | observed | observed | retain | pruning is unavailable |
| unknown | unobserved | unobserved | retain | pruning is unavailable |
| unknown | unavailable | unknown | blocked | pruning is unavailable |

`unobserved` means only “no positive hit inside the declared observation scope.” It never means
unused, dead, or safe to remove. `dead` requires complete exact static evidence for every origin of
the bundle-qualified StyleId. A positive observation of an entry proven unreachable in the same
bound universe is contradictory evidence and fails before any output group is published.

Under schema 2, a valid retention grant changes only the last column for its exact
`(bundleId, StyleId)`: the entry remains `unreachable` and `dead`, is selected, and records
`policy-retained`. All other unreachable entries remain removed. Observation still cannot select
output, and contradictory positive observation still fails even when policy would retain the entry.

## Top-level contract

All objects are closed; unknown or missing fields fail.

| Field | Contract |
|---|---|
| `schemaVersion` | `1` for `all-compiled` or `reachable-style-ids`; `2` only for `reachable-or-retained-style-ids`. |
| `stateModelVersion` | Equal to `schemaVersion`: `1` or `2`. |
| `analysisUnit` | `bundle-style-id`. Individual selectors/declarations are not removal units. |
| `originCoverage` | `compiler-verified-complete`; every entry has one or more exact source origins. |
| `ruleSelection` | `all-compiled`, `reachable-style-ids`, or `reachable-or-retained-style-ids`, with the strict schema pairing above. |
| `universeSha256` | SHA-256 of the canonical all-compiled universe defined below. |
| `application` | Exact reachability binding or `{ "state": "unavailable" }`. |
| `observation` | Exact observation binding and declared scope or `{ "state": "unavailable" }`. |
| `retention` | Schema 2 only: exact retention input binding with `state`, byte count, SHA-256, and entry count. Omitted in schema 1. |
| `summary` | Complete counts for every state and disposition. |
| `styles` | Canonically ordered bundle-qualified StyleId entries. |

When reachability is present, `application` is:

```json
{
  "state": "bound",
  "coverage": "adapter-attested-complete",
  "inputBytes": 814,
  "inputSha256": "64-lowercase-hex-digits"
}
```

When observation is present, its binding repeats the parsed coverage, producer, canonicalized
contexts and unknown dynamic inputs, plus the exact raw input byte count and SHA-256. Whitespace
drift therefore changes the report and, under `--control`, changes `configHash`.

Schema 2 additionally records:

```json
{
  "retention": {
    "state": "bound",
    "inputBytes": 412,
    "inputSha256": "64-lowercase-hex-digits",
    "entries": 1
  }
}
```

`summary.removalPolicyRetained` is present only in schema 2. Schema 1 omits both the root
`retention` field and every retention-specific summary/style field, preserving its established
shape and bytes for identical inputs.

## Style entries

Each entry has exactly:

| Field | Contract |
|---|---|
| `bundleId` | Valid Asset Plan bundle identifier. The same StyleId in another bundle is another entry. |
| `styleId`, `className` | Theme-scoped semantic identity and derived native class. |
| `selected` | Whether that complete StyleId is present in the generated bundle CSS/manifest. |
| four state fields | The axes and derivation above. |
| `retention` | Schema 2 only: `{ "state": "not-retained" }` or the exact retained entry ID and justification. Omitted in schema 1. |
| `origins` | Complete canonical origin list with per-origin reachability evidence. |

Each origin preserves canonical semantic `source`, portable `file`, exact half-open UTF-8 byte
range, `macroKind`, and `reason`. With application evidence it also records sorted `componentIds`,
`routeIds`, and `islandIds`. An origin is reachable when at least one owning component is referenced
by any route or island. Style reachability is OR-combined across all origins. Thus a shared StyleId
with one reachable and one unreachable origin is retained as a whole and both origins remain visible.

Without reachability, every origin and entry is `staticReachability: unknown`; component/root arrays
are empty. The report remains useful as an inventory but cannot justify pruning.

`className` is validated as the native lowercase base-36 class derived from the exact 128-bit
`StyleId`; producers cannot supply an unrelated `pc_*` spelling.

## Universe identity

`universeSha256` is independent of selected/pruned state, reachability, and observations. The
producer sorts styles by `(bundleId, styleId)`, sorts exact origins by portable file/range and
provenance, then hashes compact UTF-8 JSON with this shape:

```json
{
  "schemaVersion": 1,
  "analysisUnit": "bundle-style-id",
  "styles": [
    {
      "bundleId": "application",
      "styleId": "32-lowercase-hex-digits",
      "className": "pc_...",
      "origins": [
        {
          "source": "p-4",
          "file": "src/card.rs",
          "byteStart": 120,
          "byteEnd": 131,
          "macroKind": "pc",
          "reason": "visible-literal"
        }
      ]
    }
  ]
}
```

This digest lets an observation or retention producer bind the same all-compiled identities even
when final CSS uses reachable-only or reachable-plus-policy selection.

## Selection and conservative removal

For `all-compiled`, every entry must have `selected: true`. Statically unreachable entries are
`candidate`; the report does not mutate output or source.

For `reachable-style-ids`, every reachable entry must be selected and every unreachable entry must
be unselected. Any mismatch fails. `removed` means excluded from generated CSS by the explicit
`--prune-unreachable` request. It does not mean a Rust macro, ordinary CSS declaration, selector, or
token was edited or deleted. Rebuilding without the flag restores the complete generated rule set.

For `reachable-or-retained-style-ids`, the selected set is exactly the reachable StyleIds union the
bundle-qualified retention entries. A retained entry must be structurally unreachable; reachable,
mixed-origin, unknown, absent, stale-universe, or stale-reachability targets fail closed. Retained
entries stay `usageState: dead`, receive `removalDisposition: policy-retained`, and preserve every
origin and complete rule in their original bundle. The same StyleId in another bundle remains a
different entry.

Neither schema removes individual declarations, raw authored CSS, theme custom properties, or token
graph nodes. They do not infer dynamic class construction, route coverage, test quality, or browser
state completeness.

## Canonicalization, limits, and publication

- analysis, observation, and retention documents: maximum 16 MiB each;
- maximum 65,535 style entries and 65,535 aggregate origins;
- maximum 65,535 aggregate observation-context labels and 65,535 aggregate application labels per
  origin;
- paths use the reachability portable-path contract;
- style IDs are exactly 32 lowercase hexadecimal characters;
- arrays are sorted and duplicates are rejected;
- canonical output is two-space pretty JSON with one final LF;
- `--check` compares exact expected bytes and never repairs drift;
- publication is part of the same rollback-capable bundle output group; and
- `bundle --control` records role `usage-analysis`, binds its exact bytes in the manifest/receipt,
  and records optional observation/retention inputs with roles `usage-observation` and
  `usage-retention`; both exact inputs participate in `configHash`.

The Rust `parse_usage_analysis` entrypoint validates the closed structure, derived state, canonical
arrays, and self-contained universe digest; a digest alone cannot authenticate an external file.
`verify_usage_analysis` additionally accepts the exact compiler universe and reachability,
observation, and retention bytes, rederives the canonical report, and requires byte-for-byte
equality. Evidence-aware consumers should use the verifier.

See [ADR-0018](../adr/0018-separate-reachability-observation-and-removal.md), the
[reachability contract](./reachability-schema.md), and the
[usage-audit how-to](../how-to/audit-css-usage.md).
