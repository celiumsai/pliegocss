<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/themes/","category":"Core","eyebrow":"Themes and tokens","order":13}
-->

# Resolve tokens before styles.

TOML themes and DTCG Resolver documents feed the same typed registry, identity, graph, and emitted variables.

## Extend the seed {#toml}

A project theme can override or add typed colors, spacing, radii, typography, and breakpoints without changing utility grammar.

```text
schema = 1
extends = "seed"

[tokens.color]
brand = "oklch(62% 0.18 250)"
```

## Select DTCG contexts explicitly {#dtcg}

Resolver inputs are never auto-discovered. Modifier selections participate in configuration identity even when two contexts resolve to equal values.

```console
pliego-cssc compile --tokens product.resolver.json \
  --token-input appearance=dark --style "bg-brand"
```

## Publish the complete Token Graph {#graph}

Controlled output binds selected CSS to canonical token provenance, aliases, permutations, and reachability-aware usage evidence.
