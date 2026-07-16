# Token graph schema 1

Status: **canonical Rust/JSON contract, control-file publication, direct plus bundle-plan schema-2
DTCG selection, and optional direct/Asset-Plan audit evidence implemented and locally verified**

`pliegocss-token-graph/1` is the internal semantic token contract. It preserves information that a
resolved `ThemeRegistry` intentionally cannot: aliases, derived values, deprecations, reference
edges, source provenance, resolver selections, and every validated theme permutation. The registry
remains the compiler's exact resolved view, so adding graph semantics does not change a `ThemeId`,
`StyleId`, class name, or CSS when the resolved values are unchanged.

The Rust types live in `pliego-css-config`:

```rust
use pliego_css_config::{parse_token_graph, TokenGraph};

fn round_trip(graph: &TokenGraph) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = graph.to_canonical_json()?;
    let decoded = parse_token_graph(&bytes)?;
    assert_eq!(&decoded, graph);
    Ok(())
}
```

## Document shape

The closed JSON object has these fields:

| Field | Meaning |
|---|---|
| `schemaVersion` | Numeric wire schema; exactly `1` |
| `graphVersion` | Semantic graph identity; exactly `pliegocss-token-graph/1` |
| `adapter` | Optional exact adapter `name`, `version`, and canonical source hash |
| `sources` | Canonically ordered source declarations and dependency edges |
| `themes` | Canonically ordered resolved themes or context permutations |
| `dtcgInventory` | Counts keyed by exact DTCG token type |

Each source records:

- a unique dotted `path` and unique JSON `pointer`;
- the effective source `tokenType`;
- `expression`: `literal`, `alias`, or `derived`;
- sorted unique `references`, retaining curly or JSON Pointer syntax, normalized target, and an
  optional property below the target value;
- effective `deprecated` state;
- an optional typed PliegoCSS `projection` containing namespace, token ID, and canonical name;
- the fully resolved semantic `resolvedValue`.

Each theme records a stable name, the canonical modifier `selections`, its 32-digit lowercase
`themeId`, its exact per-resolution DTCG inventory, and its complete sorted typed token set. A token
can identify the winning source declaration; that source must carry the same typed projection.
Seed or adapter-unrepresentable values may have no source.

## Identity boundary

`ThemeId` hashes the resolved typed registry used by the compiler. The graph hash hashes the full
canonical graph document. Therefore changing:

```text
action = literal #3366ff
```

to a semantically equivalent alias:

```text
action = {color.brand}  where color.brand resolves to #3366ff
```

preserves `ThemeId`, `StyleId`, class names, and CSS, while changing the graph hash and alias
inventory. DTCG adapters also bind the canonical semantic source document through
`adapter.sourceHash`, so overridden declarations remain part of graph identity. This is required:
authoring/provenance changes must remain auditable without producing runtime identity churn.

The graph hash excludes the retained-style set of one compilation. Token coverage is a separate
measurement. It starts with directly referenced typed tokens and follows winning alias/derived
edges transitively; pruning styles may change coverage without changing graph identity.

## Canonical encoding

`TokenGraph::to_canonical_json()` validates first, serializes compact UTF-8 JSON, and appends exactly
one LF. `parse_token_graph()` accepts only those exact canonical bytes. It rejects leading
whitespace, pretty-printed equivalents, missing LF, unknown fields, unsorted collections, invalid
identities, unresolved edges, cycles, and any semantic invariant violation.

Canonical order is part of the contract:

- source paths, pointers, references, theme names, and typed tokens are unique and sorted;
- theme selection maps and DTCG inventories use deterministic key order;
- token IDs are eight lowercase hexadecimal digits derived from the canonical token name;
- theme IDs are 32 lowercase hexadecimal digits;
- a non-literal source must have reference edges and a literal source must not.

## Defensive limits

Validation fails before canonical publication when any boundary is exceeded:

| Resource | Limit |
|---|---:|
| Sources | 65,536 |
| Themes/permutations | 256 |
| Tokens in one theme | 65,536 |
| Reference edges | 262,144 |
| Counted graph text | 48 MiB |
| Resolved-value nodes | 1,048,576 |
| Resolved-value depth | 64 |
| Canonical graph bytes | 64 MiB |

Cycle detection is iterative rather than recursive. Canonical serialization uses a bounded writer,
so an oversized graph does not first allocate an unbounded output buffer.

## Control measurements

`pliego-css-control::build_token_graph_measurements()` binds one canonical selection and active
registry to the graph. It verifies the exact `ThemeId`, every resolved typed token/value, and the
compiled reference set before reporting:

- graph version and SHA-256 identity;
- token, alias, derived, theme, deprecation, and DTCG type counts;
- transitive token coverage in basis points;
- exact adapter identity;
- zero cycles only after graph validation has rejected cyclic input.

Before publication, the control projection independently checks that token, theme, alias, derived,
deprecation, cycle, and DTCG-inventory measurements all describe the same canonical graph theme.
Coverage remains analyzer evidence because it depends on the retained compiled-use set rather than
the graph alone.

Contrast relationships do not live in the token graph and do not change graph identity. They live in
accessibility policy schema 1, where foreground/background endpoints can refer to graph colors and
optionally select one resolver permutation. `tokens.contrastPairs` is the number of relationships
declared by that policy. It is not multiplied by the number of matching themes or findings. Generated
compile/build/watch and bundle groups currently have no accessibility policy input, so their value
remains zero. `build_audit_token_graph_measurements()` accepts the independently evaluated declared
pair count; an audit with policy and graph can therefore report a nonzero value.

Controlled compile/build/watch and `bundle --control` publish the exact canonical bytes at the fixed
`pliego.tokens.json` path. The control output has role `token-graph`, media type
`application/json`, no source map, and a SHA-256 equal to `tokens.graphHash`. Generated CSS and style
manifests relate to that file, and the graph relates back to those outputs; the receipt binds the
complete relationship set. Its required `token-graph-integrity` check must pass and point to the
exact graph output reference; omission or evidence substitution fails schema validation.

`pliego-cssc audit --token-graph FILE` supplies the same canonical schema to either direct CSS or
Asset Plan mode and requires `--accessibility-policy FILE`. The input ledger records the supplied
file with role `token-graph`; with `--control-dir`, its canonical bytes are also published at fixed
`pliego.tokens.json`. A graph-bearing audit group therefore contains four artifacts instead of the
policy-only three. Audit does not synthesize a graph from CSS custom properties.

Audit measurements deliberately use `coverageBasisPoints: 0`. The graph was measured, but ordinary
CSS audit has no compiler-owned typed-token reference set from which to calculate use coverage. This
is distinct from `observation: unavailable`, where `coverageBasisPoints` is absent, graph identity is
absent, and every counter is zero. Audit zero coverage is not evidence that the application uses no
tokens.

TOML/seed selection constructs this artifact with `TokenGraph::from_registry`: one `default` theme,
empty selections, literal sources, and no DTCG adapter. The `--tokens FILE` path for
compile/build/watch instead publishes the resolver's complete graph, including every validated
permutation, while its active canonical `--token-input modifier=context` selection chooses the
registry used for CSS. Canonical selections also enter `configHash` independently of `ThemeId`.
This direct CLI path has passed local suite, watch, package, and frozen Windows/Linux Resolver
portability vectors in `scripts/check-portability.mjs`. Bundle-plan schema 2 applies the same
complete-graph rule to `kind = "dtcg-resolver"`: exact Resolver bytes are ledgered as
`token-resolver`, while exact plan bytes bind its context selection to `configHash`. That producer
has local gates green. The Cargo build macro now selects the same active registry but intentionally
does not embed the complete graph in its binary artifact; controlled CLI generation owns that
publication. Its local workspace/MSRV/API and clean-package gates pass; declared
contrast/accessibility policy integration is now implemented for the bounded audit surface. Product
gate R0.5 remains partial only where broader producer attribution and hosted cross-platform evidence
are still required; browser/manual accessibility evidence remains outside this graph contract.
