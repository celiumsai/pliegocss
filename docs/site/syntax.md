<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/syntax/","category":"Core","eyebrow":"Utility syntax","order":10}
-->

# A finite language with visible edges.

Utilities lower into semantic slots, conditions, and typed values before any CSS serialization occurs.

## Utility plus typed value {#shape}

Spacing, color, typography, layout, effects, and interactivity draw from one validated theme registry. Arbitrary escape hatches remain policy-visible.

```text
flex items-center gap-4 rounded-lg bg-surface px-4 py-2 text-sm
```

## Conditions are structure {#conditions}

Responsive, state, attribute, container, layer, and media variants become normalized condition paths. Their order is not incidental string decoration.

```text
md:grid-cols-2 hover:bg-accent-strong aria-[expanded=true]:ring-2
```

## Importance follows the physical effect {#important}

A trailing exclamation mark marks the semantic contribution important. Synthesized effects such as the composed box-shadow preserve that authority in final CSS and lineage.

```text
ring-2! shadow-md!
```
