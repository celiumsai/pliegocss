# ADR-0013: Model cascade layers in semantic IR

## Status

Accepted starting with compatibility policy schema 1 / policy version 6.

## Context

PliegoCSS cannot make source order choose between conflicting declarations, but applications still
need an explicit, standards-based precedence mechanism for foundations, components, utilities, and
intentional overrides. Treating `@layer` as arbitrary text would hide that precedence from
conflict analysis, StyleId identity, persisted semantic IR, provenance, and strict compatibility.

The host must also remain free to use unlayered CSS, its own layer graph, or no layers at all.
PliegoCSS must not inject a reset or silently move historical styles into a layer.

## Decision

Add cascade layer as a closed condition dimension with four compiler-owned names in fixed
low-to-high order:

```css
@layer pliego.base,pliego.components,pliego.utilities,pliego.overrides;
```

The authoring variants are `layer-base:`, `layer-components:`, `layer-utilities:`, and
`layer-overrides:`. At most one may appear in a condition. The unprefixed form remains unlayered,
preserves every earlier StyleId byte stream, and emits no layer order statement.

Assignments in different layers may target the same semantic slot because their winner is explicit
in the native cascade. Within one layer, the existing source-order-independent conflict rules still
apply. The compiler includes the selected layer in StyleId and semantic-IR identity, emits the fixed
order statement before layered rules, and exposes both layer statements and layer blocks in the
schema-5 physical trace.

Generated ring/shadow initializers are placed in `pliego.base` whenever a style uses a layer. This
keeps support declarations below the authored layered assignment. PliegoCSS still emits no reset or
base declarations implicitly.

## Consequences

- Layer precedence is inspectable in semantic IR, CSS, compatibility policy, and provenance.
- Reordering commuting variants does not change identity or output.
- Repeating or combining two layer variants is `PCS012`.
- Normal declarations follow the declared low-to-high order. Native CSS reverses layer priority for
  `!important`; PliegoCSS preserves that standards behavior.
- Unlayered author declarations outrank normal layered declarations. Mixing host unlayered CSS with
  PliegoCSS layers is therefore an explicit host-level cascade decision.
- A host may declare a broader top-level order before loading PliegoCSS output. Repeated identical
  PliegoCSS order statements are valid and do not reorder already established layers.
- Component scope remains a separate, unimplemented semantic dimension; layers do not simulate
  scoping or selector isolation.

## Rejected alternatives

### Put every utility in `pliego.utilities` by default

Rejected because it would silently change the precedence and bytes of all existing unprefixed
styles and could let host unlayered CSS override them unexpectedly.

### Accept arbitrary layer names

Rejected for the alpha because it would require a user-defined ordering graph, namespace and merge
rules, new resource limits, and stronger host-integration contracts.

### Resolve cross-layer declarations by source order

Rejected because native layers already provide explicit precedence and source order would restore
the ambiguity PliegoCSS is designed to eliminate.

See the [cascade layer guide](../how-to/cascade-layers.md) and
[compatibility policy schema 2](../reference/compatibility-policy.md).
