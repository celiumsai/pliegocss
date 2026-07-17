# Migration project inventory schema 1

Status: **explicit confirmed source snapshot implemented; dependency edges, templates, configs,
plugins, and composition consumers remain open**

`MigrationProject` declares a closed set of Sass, Tailwind CSS v4 entry, and CSS Modules files. It
does not crawl the repository or infer source kind from filenames. Collection sorts the declaration
set canonically, rejects duplicate paths and more than 4,096 sources, inventories every regular
file twice, and publishes bytes only when both complete reads agree.

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
# Ok::<(), Box<dyn std::error::Error>>(() )
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
    "unsupported": 1
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
  ]
}
```

The values above are illustrative, not compatibility vectors. Every source entry is the exact
single-file schema-1 document without its trailing LF. Sources are ordered by portable path and
source kind; summary counts are derived only from those entries.

## Security and consistency boundary

- every path is explicit, project-relative, UTF-8, kind-compatible, and duplicate-free;
- every component is inspected and symbolic links or Windows reparse points are rejected;
- the final file is opened with no-follow semantics, must remain regular, and is bounded to 16 MiB;
- malformed UTF-8, comments, strings, or per-file defensive limits fail the whole snapshot;
- all sources are inventoried in one canonical order and then inventoried again in that same order;
- any byte or derived-inventory difference between passes fails instead of mixing revisions.

This is a confirmed declared-file snapshot, not an atomic filesystem transaction. It does not prove
that an intermediate revision never existed, execute original tooling, resolve imports, discover
templates or JavaScript consumers, evaluate Tailwind configuration/plugins, or infer Sass/CSS
Modules composition graphs. Those edges remain required before R0.8 can close.
