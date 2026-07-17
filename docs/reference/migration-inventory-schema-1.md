# Migration inventory schema 1

Status: **Rust producer and fail-closed CLI implemented for bounded single-file Sass, Tailwind CSS
v4 entry CSS, and CSS Modules inventory; project graphs remain open**

`pliego-css-source` exposes a read-only migration bridge that accepts an explicit source kind,
portable logical path, and exact UTF-8 source bytes. It never runs Sass, Tailwind, PostCSS,
JavaScript, plugins, configs, or project imports.

```rust
use pliego_css_source::{
    MigrationSourceKind, inventory_migration_source,
};

let inventory = inventory_migration_source(
    MigrationSourceKind::Tailwind,
    "src/app.css",
    "@import \"tailwindcss\";\n@source inline(\"bg-red-{100..900..100}\");\n",
)?;
assert_eq!(inventory.dynamic_count(), 1);
assert!(inventory.as_bytes().ends_with(b"\n"));
# Ok::<(), pliego_css_source::MigrationInventoryError>(())
```

The output is canonical two-space JSON with one trailing LF:

```json
{
  "schemaVersion": 1,
  "sourceKind": "tailwind",
  "file": "src/app.css",
  "sourceBytes": 72,
  "sourceSha256": "<64 lowercase hex characters>",
  "preflightReliance": "implicit",
  "summary": {
    "constructs": 2,
    "dynamic": 1,
    "unsupported": 0
  },
  "constructs": [
    {
      "kind": "tailwind-import",
      "byteStart": 0,
      "byteEnd": 22,
      "disposition": "static",
      "syntax": "@import \"tailwindcss\";"
    }
  ]
}
```

The byte count in this illustrative document is not a compatibility vector; real output always
derives it and the SHA-256 from the exact caller-provided bytes.

## CLI

```console
pliego-cssc migration-inventory sass src/legacy.scss
pliego-cssc migration-inventory tailwind src/app.css > app.inventory.json
pliego-cssc migration-inventory css-modules src/card.module.css
```

The CLI accepts one explicit source, never performs discovery, and writes the canonical document to
stdout. It has no output-file mutation surface. Inputs must be portable project-relative paths, and
symlink/reparse-point inputs are rejected.

## Dispositions

| Value | Meaning |
|---|---|
| `static` | The lexical construct and bounded prelude were observed with an exact source range. This is not a claim that PliegoCSS can transform it. |
| `dynamic` | Control flow, interpolation, mixin/function execution, or inline candidate generation prevents a complete static expansion. |
| `unsupported` | The inventory preserves the construct, but the bridge has no safe semantic normalization for it. |

Unknown or dynamic syntax is never silently relabeled as supported. The producer inventories what
it can prove from the supplied source and leaves execution semantics to the original toolchain.

## Covered constructs

### Tailwind CSS v4 entry CSS

- `@import "tailwindcss"` and `@import "tailwindcss/preflight"`;
- `@source`, including dynamic `inline(...)` candidate generation;
- `@theme`, `@utility`, `@variant`, `@custom-variant`, and `@reference`;
- unsupported `@apply`, `@plugin`, and `@config` seams.

`preflightReliance` is `not-observed`, `implicit`, `explicit`, or `mixed`. A full
`@import "tailwindcss"` is implicit reliance; the dedicated preflight import is explicit. This
classification does not inspect generated CSS or prove that later tooling did not remove Preflight.

### Sass/SCSS

- `@use`, `@forward`, and legacy unsupported `@import`;
- variables, mixins, functions, includes, extends, and control directives;
- `#{...}` interpolation, including interpolation inside strings.

The producer records variable tokens rather than pretending to evaluate scope or distinguish every
definition from every reference. Mixins, functions, includes, extends, controls, and interpolation
are dynamic because the bridge does not execute Sass.

### CSS Modules

- `:local`, `:global`, `:export`, and `composes:`;
- unsupported ICSS `:import` and `@value` extensions.

The first slice does not resolve composition targets, loader conventions, JS imports, or generated
class mappings.

## Validation and limits

- source is UTF-8 by API construction, contains no NUL, and is at most 16 MiB;
- logical paths are portable, project-relative, and kind-compatible (`.scss`/`.sass`, `.css`, or
  `.module.css`, compared case-insensitively);
- strings and block comments must terminate;
- comments and strings cannot create false at-rule/property matches; Sass interpolation is still
  detected inside non-comment strings;
- each recorded prelude is at most 4 KiB and the document contains at most 65,535 constructs;
- constructs are sorted by exact byte range and kind, then deduplicated;
- the exact source byte count and SHA-256 are always recorded.

This single-file output is lexical inventory, not a full language parser, dependency graph,
migration plan, codemod, or compatibility proof. The separate declared-project schema derives
conservative dependency observations from these constructs without changing this per-file document.
R0.8 remains partial until the project-input surface inventories utilities and arbitrary values from
templates, config/plugin graphs, source-toolchain-specific Sass resolution, CSS Modules/JavaScript
composition consumers, and classified unsupported/dynamic constructs across complete real-project
fixtures.
