# DTCG 2025.10 format and resolver bridge

Status: **format bridge, bounded same-document resolver, canonical graph, direct and bundle-plan
selection locally verified; Cargo build-macro selection passes local workspace/MSRV/API and clean
package gates**

`pliego-css-config` adapts Design Tokens Community Group 2025.10 documents into the canonical
PliegoCSS token graph and a resolved `ThemeRegistry`. DTCG 2025.10 is a stable Final Community
Group Report intended for implementation; it is not a W3C Recommendation. DTCG remains an adapter
boundary rather than PliegoCSS's internal IR.

The direct-format API imports or exports one resolved token document:

```rust
use pliego_css_config::{export_dtcg, parse_dtcg_path};

let imported = parse_dtcg_path("design.tokens.json")?;
let registry = imported.registry();
let graph = imported.graph();
let inventory = imported.report();

let exported = export_dtcg(registry)?;
let json = exported.to_json_pretty()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The resolver API validates every context permutation before returning:

```rust
use std::collections::BTreeMap;
use pliego_css_config::parse_dtcg_resolver_path;

let resolver = parse_dtcg_resolver_path("product.resolver.json")?;
let selected = resolver.resolve(&BTreeMap::from([
    ("appearance".to_owned(), "dark".to_owned()),
]))?;

let registry = selected.registry();
let selections = selected.selections();
let complete_graph = selected.graph();
# Ok::<(), Box<dyn std::error::Error>>(())
```

The direct CLI exposes the same resolver boundary to `compile`/`build`, `check`, `inspect`, and
`watch`:

```console
pliego-cssc compile --style "flex bg-brand" --tokens examples/product.resolver.json \
  --token-input appearance=dark --theme --output app.css
```

`--tokens FILE` is explicit and mutually exclusive with `--config` and `--seed`; the CLI never
auto-discovers a JSON token document. `--token-input modifier=context` is repeatable and is valid
only with `--tokens`. Omitted modifiers use declared defaults. Modifier names and context values
are case-insensitive, then converge on the resolver's canonical selection map. Repeating a
modifier—including a spelling that collides after case folding—fails instead of selecting a winner;
unknown modifiers, unknown contexts, and missing required non-default inputs also fail closed.

The active permutation supplies the compiler registry. Its canonical selection map participates in
the controlled build's `configHash` independently of `ThemeId`, so two selections remain auditable
even if they resolve to the same typed registry. Controlled compile/build/watch publishes the
resolver's complete graph, including every validated permutation, rather than projecting only the
selected branch. This surface is implemented and tested for direct CLI authoring.

Bundle plan schema 2 connects the same Resolver without adding bundle flags:

```toml
schema = 2
targets = "modern"
format = "minified"

[theme]
kind = "dtcg-resolver"
path = "product.resolver.json"

[theme.inputs]
appearance = "dark"

[bundles.application]
sources = ["src/styles.rs"]
emit-theme = true
```

Schema 1 remains frozen to seed/config themes. Schema 2 loads the exact Resolver once, shares its
selected registry across all bundles, and publishes the complete graph in controlled output. The
Resolver is ledgered as `token-resolver`; exact plan bytes already bind the selection to
`configHash`. Consequently textually distinct plans do not converge merely because their canonical
selections, ThemeId, or CSS happen to match. Invalid selection or Resolver input fails before group
publication. This bundle integration passes its local Windows/WSL E2E, workspace, Rust 1.85, and
package gates; hosted/macOS evidence remains a release boundary.

The Cargo `theme!` build macro selects the matching registry for `pc!`/`pcx!`:

```rust,no_run
fn main() {
    pliego_css_build::theme!(
        tokens = "product.resolver.json",
        inputs = {
            "appearance" => "dark",
        },
    );
}
```

Relative Resolver paths use `CARGO_MANIFEST_DIR`; `inputs = {}` selects declared defaults. Modifier
names and contexts are string literals, and ordered entries preserve exact and case-fold duplicate
rejection instead of silently overwriting a key. Call `theme!` once per package. The build artifact
contains only the selected `ThemeRegistry`, which is sufficient for macro identity; it is not a token
graph artifact. Invoke the CLI with the same Resolver plus `--token-input` selections for CSS. A
controlled CLI build then publishes the complete graph with every validated permutation. The Cargo
bridge passes the full Debian WSL2 workspace, Clippy, Rust 1.85, and public-API gates; the Windows
dirty package gate runs extracted DTCG and legacy-TOML consumers with identical output. Its clean
feature-commit package replay remains open. R0.5 remains partial because declared
contrast/focus/motion/forced-colors policy relationships and hosted evidence are open.

## DTCG format input

The path loaders accept only regular, non-link UTF-8 files and reject directories, symbolic links,
Windows reparse points, and documents larger than 16 MiB. This is a fail-closed source-input
contract: repositories must materialize the Resolver/token file rather than route it through a
symlink. The format parser accepts up to 64 nested group levels and 65,536 tokens.
It:

- resolves curly-brace aliases and same-document JSON Pointer references, including references into
  composite token properties;
- inherits `$type` and `$deprecated` from parent groups;
- accepts group `$root` tokens;
- detects reference cycles and declared/referenced type mismatches;
- validates every value projected into the PliegoCSS registry;
- preserves the semantic JSON value and unknown extension metadata in `DtcgTheme`;
- rejects malformed `$extensions` and canonical-name collisions instead of selecting a winner;
- merges projected definitions over the seed registry, matching theme-schema 1;
- records literal, alias, derived, deprecation, reference-edge, projection, and resolved-value data
  in `DtcgTheme::graph()` before exposing the flat registry view.

Top-level namespaces recognized without an extension hint are `color`, `spacing`, `font-family`,
`font-size`, `font-weight`, `line-height`, `letter-spacing`, `radius`, `shadow`, `z-index`, and
`breakpoints`. A group or token may instead declare the same logical namespace in
`$extensions.io.celiums.pliego-css.kind`. Nested path segments normalize into one kebab-case
PliegoCSS name; a namespace-root `$root` token must provide the extension field `name`.

| PliegoCSS namespace | Required DTCG type |
|---|---|
| `color` | `color` |
| `spacing`, `font-size`, `letter-spacing`, `radius`, `breakpoints` | `dimension` (`px` or `rem`) |
| `font-family` | `fontFamily` |
| `font-weight` | `fontWeight` |
| `line-height` | `number` or `dimension` |
| `shadow` | `shadow` |
| `z-index` | integral `number` in the signed 32-bit range |

Color components, alpha, font-weight ranges, dimensions, shadow members, and canonical registry
domains are validated before a definition enters the registry. Standard tokens outside recognized
PliegoCSS namespaces remain in the preserved document and appear in
`DtcgReport::preserved_tokens()`. Preservation is not a claim that PliegoCSS validated every DTCG
composite type.

`DtcgReport::aliases()` counts imported tokens containing any reference, including composite or
property-derived values. `TokenGraph::aliases()` counts complete-value aliases only;
`TokenGraph::derived_values()` counts the latter composite/property category separately.

## Resolver profile

`DTCG_RESOLVER_PROFILE` is `2025.10/same-document-1`. It implements the Resolver 2025.10 minimum
same-document reference requirement with a closed, bounded profile:

- `version` must be exactly `2025.10` and `resolutionOrder` must be non-empty;
- named and inline sets compose `sources` in array order;
- modifiers use a non-empty `contexts` map whose values are source arrays; this profile rejects a
  single-context modifier because it should be a set;
- contexts may use empty source arrays;
- later sources and later resolution-order items replace earlier token declarations;
- same-document source references may target named sets or bundled token documents under `$defs`;
- keys next to `$ref` are shallow overrides: nested objects and arrays replace rather than merge;
- only `resolutionOrder` may reference a modifier, and nothing may reference
  `resolutionOrder` itself;
- input modifier names and context values are case-insensitive; document collisions after case
  folding, unknown inputs, invalid values, and missing non-default inputs fail closed;
- aliases resolve only after the selected sources have been ordered and flattened;
- every possible context permutation is resolved at load time, so an invalid non-default branch
  rejects the whole document;
- the product of modifier contexts may not exceed 256; reference nesting may not exceed 64;
- expansion, reference visits, materialized sources, retained permutation bytes, and combined graph
  sources have independent fail-closed budgets, preventing a small reference DAG from expanding
  exponentially in memory;
- JSON Pointer processing validates RFC 6901 `~0`/`~1` escapes and URI-fragment percent encoding.

Filesystem and network references are rejected without I/O. This is an explicit profile boundary,
not a claim that Resolver 2025.10 forbids those optional transports.

`DtcgResolver::graph()` contains every validated permutation. `DtcgResolver::resolve()` returns the
selected registry, that same complete graph, and the canonical selection map after defaults and
case folding. Consumers must use `DtcgTheme::selections()` rather than the raw input map when
looking up the selected graph theme. `resolve_entries()` accepts repeated adapter inputs without
first collapsing exact duplicate keys.

The resolver adapter records a SHA-256 of the canonical semantic resolver document in the graph.
This keeps declarations, ordering, and metadata integrity-bound even when a later source overrides
their resolved value. Individual graph themes carry their own DTCG inventory; the graph-level
inventory is the per-type maximum across all permutations.

## Export and lossless profile

`export_dtcg` emits standard DTCG tokens whenever a registry value has a faithful representation.
The root extension `io.celiums.pliego-css` records profile version 1, DTCG format `2025.10`, seed
inheritance, the exact `ThemeId`, and an inventory. Each projected token carries its exact CSS value
in extension metadata.

CSS permits values the DTCG token types cannot represent faithfully, including several units,
functions, keywords, and current shadow forms. Those definitions are never replaced by invented
standard values. They are stored losslessly in `unmappedTokens`, listed by
`DtcgReport::unmapped_tokens()`, and restored during profile import. Export completes only after
self-import proves the exact `ThemeId`.

`DtcgTheme::to_json_pretty()` produces deterministic pretty JSON with one trailing newline. The
canonical internal graph has a separate compact wire contract described in
[Token graph schema 1](./token-graph-schema-1.md).

## Deliberate fail-closed limits and remaining R0 work

The 0.1 format profile still rejects group `$extends`. The resolver does not load external files or
URLs, and neither CLI nor Cargo discovers JSON token files. The controlled compile/build/watch path
publishes the complete selected resolver graph as integrity-bound `pliego.tokens.json`; its local
suite, watch, portability, and package gates pass. Schema-2 bundle integration passes its local
Windows/WSL E2E, workspace, Rust 1.85, and package gates. Cargo `theme!` selection is implemented
with local workspace/MSRV/API and clean-package gates passing. Declared contrast relationships and
accessibility policy checks plus hosted evidence remain open, so R0.5 and the 0.1.0 MVP are not
closed.

## Verification

Run:

```text
cargo test -p pliego-css-config --all-features
cargo test -p pliego-css-build --all-features
cargo test -p pliego-css-control --all-features
cargo test -p pliego-cssc --test dtcg_selection
cargo clippy -p pliego-css-config --all-features --all-targets -- -D warnings
```

The resolver suite covers defaults, case-insensitive inputs, post-order alias resolution,
last-wins composition, named and inline items, bundled `$defs`, set references, shallow overrides,
cycles in non-default branches, closed input validation, case-fold collisions, forbidden/external
references, the 256-permutation cap, and path loading. The separate CLI selection gate covers
defaults, explicit contexts, case folding, duplicate/unknown inputs, complete graph publication,
and canonical-selection `configHash` binding; that gate is green in the local workspace and package
verification recorded for this checkout. Bundle schema-2 tests additionally prove schema-1
rejection, default/explicit selection, exact `token-resolver` input evidence, complete graph
publication, read-only `--check`, and last-valid-group retention; those local gates are green.
Build-macro tests cover default and explicit contexts, case folding, duplicate preservation,
invalid Resolver/selection failure, selected-registry artifact identity, repeated artifact stability,
macro grammar, and the downstream public-API Cargo bridge. Packaging status records the clean
feature-commit evidence and exact archive sizes.

Primary references:

- [DTCG 2025.10 technical reports](https://www.designtokens.org/TR/2025.10/)
- [Format module](https://www.designtokens.org/TR/2025.10/format/)
- [Color module](https://www.designtokens.org/TR/2025.10/color/)
- [Resolver module](https://www.designtokens.org/TR/2025.10/resolver/)
