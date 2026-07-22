<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/migration/","category":"Migration","eyebrow":"Controlled migration","order":5}
-->

# Adopt without surrendering the exit.

Inventory first, generate bounded plans, preserve originals, and detect drift immediately before publication.

## Read-only discovery {#inventory}

Tailwind, Sass, CSS Modules, and standard CSS can be inventoried before any replacement plan exists.

```console
pliego-cssc migration-project-plan ./migration-declaration.json
```

## Reversible replacements {#reversible}

Plans bind source and destination hashes. Apply and rollback use adjacent locks, pre-publication revalidation, backups, and group compensation.

## No speculative codemods {#limits}

Unsupported or context-dependent constructs stay in the inventory. The migration layer does not infer framework semantics it cannot prove.
