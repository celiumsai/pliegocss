<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/migration/sass/","category":"Migration","eyebrow":"Sass migration","order":20}
-->

# Separate syntax conversion from semantic adoption.

Inventory variables, nesting, mixins, imports, and generated surfaces without pretending every Sass abstraction is a utility.

## Map the source surface {#inventory}

The inventory records statically visible Sass roles and unresolved dynamic behavior so the team can choose what stays authored CSS.

```console
pliego-cssc migration-inventory sass styles/main.scss
```

## Ordinary CSS is a valid destination {#css-first}

Use the standard-CSS transformer for syntax and compatibility work. Adopt typed PliegoCSS styles only where identity and provenance pay for themselves.

## Keep originals and rollback evidence {#rollback}

Applied replacements preserve source backups and hash-bound rollback records. A changed destination aborts rather than overwriting concurrent edits.
