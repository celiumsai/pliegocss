<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/variants/","category":"Core","eyebrow":"Variants and conditions","order":12}
-->

# Conditions without string-order folklore.

Every variant joins a typed condition path with deterministic normalization and specificity behavior.

## Responsive paths {#responsive}

Theme breakpoints are validated once and emitted as ordered media conditions. Assignments at different breakpoints do not conflict.

```text
grid-cols-1 md:grid-cols-2 lg:grid-cols-3
```

## State and attributes {#state}

Pseudo-class and bounded attribute variants stay in semantic IR so policy and physical lineage can explain their selectors.

```text
hover:bg-accent focus-visible:ring-2 data-[state=open]:opacity-100
```

## Cascade layers {#layers}

Fixed layer variants model author-controlled precedence without allowing arbitrary source-order dependence to leak into identity.

```text
layer-components:bg-surface layer-utilities:p-4
```
