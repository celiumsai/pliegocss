# CSS emission

PliegoCSS emits standard CSS from validated semantic IR. The public build path is:

```text
pc! literal -> syntax AST -> SemanticStyle -> StyleId -> CSS class and rules
```

Syntax and semantic failures happen before emission. The emitter accepts only a structurally valid
`SemanticStyle` with a resolved identity; unknown tokens, broken table references, unsupported
semantic utilities, unknown breakpoints, and an identity derived under another theme are errors
rather than silent fallbacks.

## Class identity

Each style uses one class derived from the complete 128-bit `StyleId`. The current encoding is
lowercase base 36 with a `pc_` prefix:

```text
pc_<semantic-id>
```

Equivalent normalized input therefore targets the same class. Source order and source spans are not
part of semantic identity. Class-name format 1 is CSS-safe and unchanged; StyleId format 2 supplies
its input through an explicit tagged binary stream and SHA-256 truncation. The current vectors are
machine-enforced candidate contracts at `0.0.0`, not yet a published SemVer guarantee. Moving from
the earlier format-1 candidate changes the class even though the base-36 algorithm remains format 1.

## Rules and conditions

Assignments are grouped by normalized condition. The same class may produce one base rule and
additional conditional rules:

```rust,ignore
pc!("flex gap-4 hover:bg-accent md:grid")
```

Conceptually emits:

```css
.pc_<id>{display:flex;gap:1rem}
.pc_<id>:hover{background-color:var(--color-accent)}
@media (min-width:48rem){.pc_<id>{display:grid}}
```

The actual output is minified. Pseudo-states become selector suffixes; light and dark modes use
`[data-theme=...]` ancestors; breakpoints, reduced-motion preferences, and contrast preferences use
media queries. Condition ordering comes from normalized semantic dimensions, so commuting forms such
as `md:hover:` and `hover:md:` emit the same rule structure.

`container-inline` establishes `container-type:inline-size`. A `cq-<breakpoint>:` condition reuses
the active theme's breakpoint scale as a minimum container width. When viewport and container
dimensions coexist, native output nests `@media { @container { ... } }`; commuting spellings retain
one StyleId and byte-identical output.

An explicit `layer-*:` condition emits the fixed order statement
`@layer pliego.base,pliego.components,pliego.utilities,pliego.overrides;` and wraps its rule in the
selected namespaced layer. Layer is the outer condition wrapper, followed by media and then
container. Unprefixed styles remain byte-identical, unlayered CSS and emit no order statement.
Generated ring/shadow initializers enter `pliego.base` when a style uses layers, so normal authored
assignments remain above their support declarations.

After aggregation, the top-level/media-nested topology emitted by PliegoCSS combines exactly equal
adjacent media-query siblings while preserving every qualified rule in place. A non-media sibling or
a different parsed query is a hard boundary; the optimizer never moves rules to seek a match. It
treats `@container`, `@supports`, and `@layer` as hard boundaries and does not traverse them while
merging media siblings. This removes repeated
wrappers without changing cascade order. See the
[measured contract](../benchmarks/media-query-merging.md).

## Declarations

The emitter covers the currently documented utility families:

- display, width and height constraints;
- inline-size container establishment and typed container breakpoints;
- flex direction, wrapping, alignment, and justification;
- grid templates and column spans;
- gap, padding, and margin footprints;
- background and text colors;
- font family, size, weight, and line height;
- border width/color, radius, opacity, shadow, and ring;
- arbitrary CSS property declarations, resolved by property name.

Complete footprints use CSS shorthands. Narrower semantic footprints use the corresponding longhand,
which preserves refinements such as `p-4 px-2` independently of authoring order. Negative values are
emitted as `calc(<value>*-1)` only after the semantic compiler confirms the family and value are
negatable.

Ring and shadow assignments compose through `--pc-ring-*` and `--pc-shadow` variables into one
`box-shadow` declaration. Distinct arbitrary property names coexist deterministically; repeating the
same name and value deduplicates, while two values for the same name conflict.

## Active theme

Every lowering and emission operation uses one `ThemeRegistry`. Seed wrappers delegate to the same
registry-aware implementation as the explicit `*_with_theme` APIs. Token values and breakpoint widths
resolve from that active registry, and `ThemeId` participates in `StyleId`, so two registries cannot
normally reuse a class for different declarations. `ThemeId` is itself a compact 128-bit identity,
so this is collision resistance with a loud batch guard, not a mathematical uniqueness proof.

`emit_theme(&registry)` writes the custom properties required by the current emitter. Color tokens
other than direct keywords and font-family tokens use those variables; spacing, radii, type scale,
weights, line heights, letter spacing, and shadows resolve directly into utility declarations. The
schema-1 TOML and DTCG Resolver build bridges validate or select one canonical registry during the
Cargo build; no theme parser or styling runtime enters browser WASM.

## Determinism boundary

Within one compiler version, canonical assignments and ordered condition groups make emission
deterministic for equivalent semantic input. `pliego-cssc` aggregates styles by canonical StyleId
format-2 stream, rejects inconsistent or colliding stream/ID mappings, orders rules by stream bytes,
combines adjacent equal media wrappers, and passes the resulting artifact through Lightning CSS for
standards parsing and minification. It
uses the explicit `baseline-widely`, `modern`, or `none` target contract rather than host browser
configuration. The strict profile is defined by
[compatibility policy schema 2](../reference/compatibility-policy.md).
Default schema-3 manifests retain every normalized origin, report the
StyleId/class/ThemeId format versions, and bind the metadata to the exact CSS through `cssSha256`
and `cssBytes`. Opt-in schema 4 adds compiler-verified origin ownership plus an adapter-attested
application graph, canonical semantic declarations, and direct token references without changing
the CSS artifact. Opt-in schema 5 preserves that exact semantic graph and adds a fail-closed graph-2
projection of final qualified/media/container/layer rules and layer-order statements, declaration
occurrences, generated support output, and exact half-open UTF-8 byte ranges after Lightning CSS
serialization.

Compilation and postprocessing finish before output staging. CSS and manifest are each written to a
sibling temporary file and synchronized. Advisory per-destination locks exclude cooperating writers;
existing destinations then move to sibling backups before the prepared files are renamed. A reported
failure triggers rollback in the same process, and rollback failures identify the paths that could not
be restored. The two paths still cannot be committed as one filesystem transaction or made
crash-atomic. Consumers must verify the manifest digest and byte length before accepting the pair.

## What is intentionally not complete

The current pipeline does not yet:

- deduplicate declarations shared by different style identities;
- validate behavior in a real browser matrix;
- split CSS by route or island;
- emit critical CSS or prune individual declarations/theme variables;
- emit a reset/preflight layer;
- approve the current machine-enforced StyleId format-2 candidate as part of a public release.

Static source scanning only covers the explicitly supplied Rust files or directory trees. It does not
resolve Cargo dependencies, generated code, PliegoRS routes, or island reachability. Schemas 4/5 can
import those application relationships from an exact, framework-owned sidecar.
`--prune-unreachable` can then remove complete StyleId rule sets with no reachable exact origin, but
the adapter remains responsible for application completeness and `--theme` remains unpruned.
