<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/migration/tailwind/","category":"Migration","eyebrow":"Tailwind migration","order":19}
-->

# Inventory Tailwind before translating it.

Recover the real utility surface, classify supported roles, and keep unsupported framework behavior visible.

## Scan without replacement {#inventory}

The inventory recognizes bounded class-bearing surfaces and reports candidates with source ranges. It does not rewrite template semantics during discovery.

```console
pliego-cssc migration-inventory tailwind src/App.tsx
```

## Generate a hash-bound project plan {#plan}

A declaration selects roots and destinations. The plan records exact source and output hashes before any replacement can be applied.

```console
pliego-cssc migration-project-plan migration.json
```

## Preserve unsupported behavior {#gaps}

Plugins, generated variants, arbitrary selectors, and runtime class construction remain explicit migration findings unless a project-owned adapter proves them.
