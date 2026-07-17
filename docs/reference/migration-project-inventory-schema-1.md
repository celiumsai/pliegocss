# Migration project inventory schema 1

Status: **explicit confirmed source snapshot and conservative declared-source dependency edges
implemented; templates, configs, plugins, transitive discovery, and composition consumers remain
open**

`MigrationProject` declares a closed set of Sass, Tailwind CSS v4 entry, and CSS Modules files. It
does not crawl the repository or infer source kind from filenames. Collection sorts the declaration
set canonically, rejects duplicate paths and more than 4,096 sources, inventories every regular
file twice, and publishes bytes only when both complete reads agree.

The same closed input set can be checked into the project as schema-1 JSON and parsed with
`MigrationProject::from_json`:

```json
{
  "schemaVersion": 1,
  "sources": [
    { "sourceKind": "sass", "file": "src/legacy.scss" },
    { "sourceKind": "tailwind", "file": "src/app.css" },
    { "sourceKind": "css-modules", "file": "src/card.module.css" }
  ]
}
```

The declaration is bounded to 1 MiB, rejects unknown fields, unsafe paths, kind/extension mismatch,
unsupported schema versions, and more than 4,096 entries. Parsing does not read sources; collection
performs the same canonical duplicate, file-safety, two-pass, and dependency checks as the builder
API. Declaration order therefore does not affect snapshot bytes.

`MigrationProject::from_file` applies the same portable regular-file/no-link boundary to the
declaration itself. The CLI exposes that complete path and writes no file implicitly:

```console
pliego-cssc migration-project-inventory migration.project.json > migration.inventory.json
```

```rust,no_run
use pliego_css_source::{
    MigrationProject, MigrationProjectSource, MigrationSourceKind,
};

let snapshot = MigrationProject::new()
    .source(MigrationProjectSource::new(
        MigrationSourceKind::Sass,
        "src/legacy.scss",
    ))
    .source(MigrationProjectSource::new(
        MigrationSourceKind::Tailwind,
        "src/app.css",
    ))
    .source(MigrationProjectSource::new(
        MigrationSourceKind::CssModules,
        "src/card.module.css",
    ))
    .collect()?;
std::fs::write("migration.inventory.json", snapshot.as_bytes())?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The canonical document uses two-space JSON and one trailing LF:

```json
{
  "schemaVersion": 1,
  "summary": {
    "sources": 3,
    "sassSources": 1,
    "tailwindSources": 1,
    "cssModulesSources": 1,
    "constructs": 8,
    "dynamic": 2,
    "unsupported": 1,
    "dependencies": 3,
    "resolvedDependencies": 1,
    "externalDependencies": 1,
    "unresolvedDependencies": 1,
    "dynamicDependencies": 0
  },
  "sources": [
    {
      "schemaVersion": 1,
      "sourceKind": "tailwind",
      "file": "src/app.css",
      "sourceBytes": 23,
      "sourceSha256": "<64 lowercase hex characters>",
      "preflightReliance": "implicit",
      "summary": {
        "constructs": 1,
        "dynamic": 0,
        "unsupported": 0
      },
      "constructs": []
    }
  ],
  "dependencies": [
    {
      "from": "src/app.css",
      "kind": "tailwind-reference",
      "byteStart": 0,
      "byteEnd": 25,
      "specifier": "./theme.css",
      "resolution": "resolved",
      "target": "src/theme.css"
    }
  ]
}
```

The values above are illustrative, not compatibility vectors. Every source entry is the exact
single-file schema-1 document without its trailing LF. Sources are ordered by portable path and
source kind. Dependencies are ordered by containing source and exact byte range; summary counts are
derived from those canonical observations. `resolvedDependencies` includes both `resolved` edges
and `local` CSS Modules composition.

## Dependency resolution boundary

The project layer observes these bounded constructs:

| Kind | Observed syntax |
| --- | --- |
| `sass-use`, `sass-forward`, `sass-import` | Sass module/import seams; every quoted legacy import-list specifier is retained |
| `tailwind-import`, `tailwind-reference` | CSS imports and Tailwind references |
| `css-modules-composes` | local, global, and quoted `from` composition |
| `css-modules-import`, `css-modules-value` | ICSS import/value seams |

Resolution is intentionally narrower than Sass, PostCSS, bundler, or Node resolution:

- `resolved`: an explicit `./` or `../` specifier includes a supported extension, normalizes inside
  the project, and names a declared source of the expected kind;
- `local`: CSS Modules `composes` has no `from` and targets its containing source;
- `external`: a package, Sass built-in, URL, browser-absolute path, `global`, or relative Sass CSS
  import is owned outside this declared source graph;
- `unresolved`: static syntax is visible, but extensionless Sass lookup, query/fragment syntax, or
  another source-toolchain-specific rule would be required;
- `dynamic`: interpolation or syntax that cannot expose one safe static specifier.

An exact supported local path is an integrity claim: if its normalized target is absent from the
declared set, has the wrong source kind, or escapes the project root, collection fails. PliegoCSS
does not silently reinterpret that edge as external and does not guess Sass partials/index files.

## Security and consistency boundary

- every path is explicit, project-relative, UTF-8, kind-compatible, and duplicate-free;
- every component is inspected and symbolic links or Windows reparse points are rejected;
- the final file is opened with no-follow semantics, must remain regular, and is bounded to 16 MiB;
- malformed UTF-8, comments, strings, dependency targets, or per-file defensive limits fail the
  whole snapshot;
- all sources are inventoried in one canonical order and then inventoried again in that same order;
- any byte or derived-inventory difference between passes fails instead of mixing revisions.

This is a confirmed declared-file snapshot, not an atomic filesystem transaction. It does not prove
that an intermediate revision never existed, crawl undeclared sources, reproduce toolchain-specific
resolution, discover templates or JavaScript consumers, evaluate Tailwind configuration/plugins, or
identify downstream CSS Modules composition consumers. Those surfaces and real-project fixtures
remain required before R0.8 can close.
