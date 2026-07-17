# Token-usage report schema 1

Status: **implemented as an opt-in bundle artifact; exact clean package replay green at commit
`af9c2c5`, hosted release evidence pending**

`bundle --usage-report` emits two adjacent, read-only reports:

```text
OUTPUT_DIR/pliego.usage.json
OUTPUT_DIR/pliego.token-usage.json
```

Usage Analysis classifies complete bundle-qualified StyleIds. Token Usage projects the active
canonical Token Graph through that same retained StyleId selection. It does not scan generated CSS.

## Query

```console
pliego-css-tokens explain \
  --report dist/assets/pliego.token-usage.json \
  --token color.accent

pliego-css-tokens explain \
  --report dist/assets/pliego.token-usage.json \
  --token color.accent \
  --format json
```

The default text format is for humans; `--format json` returns the exact canonical report node.
Kinds and names are exact and case-sensitive. Missing tokens, malformed/canonicality-invalid
reports, duplicate options, unknown formats, and unreadable files fail without changing artifacts.

## Top-level contract

Every object is closed and the document is bounded to 16 MiB.

| Field | Contract |
|---|---|
| `schemaVersion` | Integer `1`. |
| `reportKind` | `pliegocss-token-usage/1`. |
| `graphVersion` | Version copied from the canonical graph. |
| `graphSha256` | SHA-256 of its canonical graph bytes. |
| `theme` | Exact resolved theme `name`, sorted `selections`, and 128-bit `themeId`. |
| `summary` | Counts for `tokens`, `direct`, `dependencies`, `unused`, and `emittedCustomProperties`. |
| `tokens` | Strictly ordered active resolved token nodes. |

Each token node contains:

| Field | Contract |
|---|---|
| `id` | `token:<kind>:<8-hex-tokenId>`. |
| `kind`, `tokenId`, `name` | Exact typed identity and resolved canonical name. |
| `source` | Winning graph-source path, or `null` for a registry literal. |
| `expression` | Winning source expression classification. |
| `deprecated` | Winning-source deprecation state. |
| `status` | `direct`, `dependency`, or `unused`. |
| `customProperty` | Compiler-owned property name, or `null` for literal-backed kinds. |
| `emitted` | True only when this exact build emitted that direct token's custom property. |
| `consumers` | Sorted unique direct `(bundleId, StyleId)` consumers. |
| `requiredBy` | Sorted direct token node IDs whose transitive source path needs this token. |

Status precedence is `direct` over `dependency` over `unused`. A direct node can therefore have both
consumers and `requiredBy`. Dependency nodes have no direct consumers and at least one `requiredBy`;
unused nodes have neither.

## Evidence boundary

“Unused” means unreachable from retained semantic StyleIds in this exact build and active resolver
selection. It is not runtime observation and is not permission to delete a design token. Authored
CSS `var(...)` consumers, other applications, other theme selections, and dynamic external systems
are outside the proof. The report remains inventory-only when used outside that exact build context.

When pruning is enabled, theme custom properties are selected from direct typed references across
the entire bundle group, not only the bundle that emits `:root`. Dependencies are resolved into the
direct token's value and are not separately emitted unless they also have a direct retained consumer.

## Canonical validation

The parser rejects unknown fields, wrong versions, invalid IDs/hashes, duplicate or unsorted tokens
and consumers, dangling `requiredBy`, inconsistent status arrays, and incorrect summary counts. It
then reserializes and requires byte-for-byte canonical pretty JSON with one final LF.

See [ADR-0021](../adr/0021-project-token-usage-from-retained-semantic-styles.md), the
[Token Graph schema](./token-graph-schema-1.md), and
[Usage Analysis schemas 1 and 2](./usage-analysis-schema-1.md).
