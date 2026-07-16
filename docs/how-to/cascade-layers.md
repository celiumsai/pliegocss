# Use typed cascade layers

Cascade layers let an application state precedence explicitly while keeping PliegoCSS conflict
checks deterministic. PliegoCSS exposes four closed variants:

| Variant | Native layer | Normal-declaration precedence |
|---|---|---:|
| `layer-base:` | `pliego.base` | 1, lowest |
| `layer-components:` | `pliego.components` | 2 |
| `layer-utilities:` | `pliego.utilities` | 3 |
| `layer-overrides:` | `pliego.overrides` | 4, highest |

```rust
let card = pliego_css::pc!(
    "layer-components:block layer-components:bg-surface layer-utilities:p-4 layer-overrides:hover:bg-accent"
);
```

Any layered style emits the fixed native order before its layer blocks:

```css
@layer pliego.base,pliego.components,pliego.utilities,pliego.overrides;
```

Variants commute with other typed dimensions. These spellings have the same StyleId and CSS:

```rust
let a = pliego_css::pc!("layer-components:md:hover:grid");
let b = pliego_css::pc!("hover:md:layer-components:grid");
assert_eq!(a.id(), b.id());
```

## Conflict behavior

Within one layer and normalized condition, semantic conflicts still fail:

```rust,ignore
pc!("layer-components:flex layer-components:grid") // PCS005
pc!("layer-base:layer-utilities:flex")              // PCS012
```

Across layers, assigning the same slot is valid because the layer order names the winner:

```rust
let display = pliego_css::pc!("layer-components:flex layer-overrides:grid");
```

For normal declarations, `pliego.overrides` wins. CSS reverses layer priority for `!important`, so
important declarations in earlier layers outrank important declarations in later layers. PliegoCSS
does not conceal or rewrite that native rule.

## Host boundary

Unprefixed utilities remain unlayered and emit no `@layer` statement:

```rust
let legacy = pliego_css::pc!("flex p-4");
```

Normal unlayered author CSS outranks normal layered author CSS. Choose either a consistently layered
application policy or account for that native precedence when combining host stylesheets.

PliegoCSS does not inject a reset, invent application layer names, or isolate selectors. Native
component scope is explicitly deferred beyond `0.1.0` by
[ADR-0014](../adr/0014-defer-native-component-scope.md). A host that needs a larger layer graph may
establish it before loading PliegoCSS CSS:

```css
@layer vendor, pliego.base, pliego.components, pliego.utilities, pliego.overrides, app;
```

The generated `pliego.*` statement can repeat safely; CSS keeps the first established order.

## Compatibility and provenance

Typed layers are allowed by compatibility policy 7 under `baseline-widely`. Layer choice participates
in StyleId format 2 and semantic IR binary format 2 through an append-only optional marker, so all
pre-layer identities remain byte-identical. Manifest schema 5 records order statements as
`layer-order` physical rules and blocks as `layer` physical rules, including nested media/container
relationships and exact final-CSS ranges.

See [ADR-0013](../adr/0013-model-cascade-layers-in-semantic-ir.md), the
[syntax contract](../reference/syntax.md), and the
[compatibility policy](../reference/compatibility-policy.md).
