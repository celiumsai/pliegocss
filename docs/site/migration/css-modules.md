<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/migration/css-modules/","category":"Migration","eyebrow":"CSS Modules migration","order":21}
-->

# Preserve component ownership.

Discover module exports and usage while keeping application-owned component topology separate from CSS semantics.

## Inventory exported classes {#inventory}

The scanner identifies bounded module class definitions and their source locations without assuming a particular bundler naming strategy.

```console
pliego-cssc migration-inventory css-modules src/Card.module.css
```

## Let the application own reachability {#ownership}

Route and component reachability comes from the framework or product index, not from filename heuristics inside the CSS compiler.

## Mixed adoption is first-class {#mixed}

A project can retain modules for component styles, audit their emitted CSS, and introduce typed identities only for new or shared surfaces.
