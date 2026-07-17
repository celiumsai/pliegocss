# Migration project inventory schema 1

Status: **explicit source/consumer/auxiliary snapshot, conservative dependencies, and CSS Modules
consumer linking implemented; transitive discovery, auxiliary semantic evaluation, and broader
consumer syntax remain open**

`MigrationProject` declares a closed set of Sass, Tailwind CSS v4 entry, and CSS Modules files. It
does not crawl the repository or infer source kind from filenames. Collection sorts the declaration
set canonically, rejects duplicate/cross-role paths and more than 4,096 combined declared files,
inventories every regular file twice, and publishes bytes only when both complete reads
agree.

The same closed input set can be checked into the project as schema-1 JSON and parsed with
`MigrationProject::from_json`:

```json
{
  "schemaVersion": 1,
  "sources": [
    { "sourceKind": "sass", "file": "src/legacy.scss" },
    { "sourceKind": "tailwind", "file": "src/app.css" },
    { "sourceKind": "css-modules", "file": "src/card.module.css" }
  ],
  "consumers": [
    { "consumerKind": "css-modules", "file": "src/Card.tsx" }
  ],
  "auxiliaries": [
    { "auxiliaryKind": "tailwind-config", "file": "tailwind.config.js" },
    { "auxiliaryKind": "tailwind-plugin", "file": "src/plugin.ts" },
    { "auxiliaryKind": "tailwind-template", "file": "src/index.html" }
  ]
}
```

The declaration is bounded to 1 MiB, rejects unknown fields, unsafe paths, kind/extension mismatch,
unsupported schema versions, and more than 4,096 files across all three roles. Parsing does not read declared files; collection
performs the same canonical duplicate, file-safety, two-pass, and dependency checks as the builder
API. Declaration order therefore does not affect snapshot bytes.

`MigrationProject::from_file` applies the same portable regular-file/no-link boundary to the
declaration itself. The CLI exposes that complete path and writes no file implicitly:

```console
pliego-cssc migration-project-inventory migration.project.json > migration.inventory.json
```

```rust,no_run
use pliego_css_source::{
    MigrationAuxiliaryKind, MigrationConsumerKind, MigrationProject, MigrationProjectAuxiliary,
    MigrationProjectConsumer, MigrationProjectSource, MigrationSourceKind,
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
    .consumer(MigrationProjectConsumer::new(
        MigrationConsumerKind::CssModules,
        "src/Card.tsx",
    ))
    .auxiliary(MigrationProjectAuxiliary::new(
        MigrationAuxiliaryKind::TailwindConfig,
        "tailwind.config.js",
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
    "consumers": 1,
    "auxiliaries": 3,
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
    "dynamicDependencies": 0,
    "consumerImports": 1,
    "staticConsumerUsages": 2,
    "dynamicConsumerUsages": 1,
    "tailwindConfigs": 1,
    "tailwindPlugins": 1,
    "tailwindTemplates": 1,
    "staticTemplateCandidates": 4,
    "dynamicTemplateCandidates": 1,
    "tailwindConfigKeys": 2,
    "tailwindPluginApis": 1
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
  ],
  "consumers": [
    {
      "consumerKind": "css-modules",
      "file": "src/Card.tsx",
      "sourceBytes": 80,
      "sourceSha256": "<64 lowercase hex characters>",
      "observations": []
    }
  ],
  "auxiliaries": [
    {
      "auxiliaryKind": "tailwind-config",
      "file": "tailwind.config.js",
      "sourceBytes": 42,
      "sourceSha256": "<64 lowercase hex characters>",
      "observations": []
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
| `tailwind-config`, `tailwind-plugin` | Legacy configuration and plugin ownership seams |
| `tailwind-source` | Template/discovery path or dynamic inline-source seam |
| `css-modules-composes` | local, global, and quoted `from` composition |
| `css-modules-import`, `css-modules-value` | ICSS import/value seams |

Resolution is intentionally narrower than Sass, PostCSS, bundler, or Node resolution:

- `resolved`: an explicit `./` or `../` specifier normalizes inside the project and names a
  declared source or exact Tailwind auxiliary of the expected kind;
- `local`: CSS Modules `composes` has no `from` and targets its containing source;
- `external`: a package, Sass built-in, URL, browser-absolute path, `global`, or relative Sass CSS
  import is owned outside this declared source graph;
- `unresolved`: static syntax is visible, but extensionless Sass lookup, query/fragment syntax, or
  another source-toolchain-specific rule would be required. Undeclared Tailwind auxiliaries plus
  glob/directory `@source` discovery remain here;
- `dynamic`: interpolation or syntax that cannot expose one safe static specifier.

An exact supported local path is an integrity claim: if its normalized target is absent from the
declared set, has the wrong source kind, or escapes the project root, collection fails. PliegoCSS
does not silently reinterpret that edge as external and does not guess Sass partials/index files.

## CSS Modules consumer boundary

Declared `js`, `jsx`, `ts`, `tsx`, `mjs`, and `cjs` consumers are read through the same bounded
regular-file/no-link boundary and inventoried twice. The scanner retains default and namespace
imports ending in `.module.css`, exact `binding.className` and `binding["class-name"]` accesses, and
marks computed brackets, binding escape, named/dynamic imports, or binding text inside template
literals as dynamic. Comments and ordinary quoted strings cannot create usage observations.

Every relative import target must normalize to a declared `css-modules` source; missing or mistyped
targets fail the complete snapshot. Package imports remain visible without a local target. This is a
lexical migration inventory, not JavaScript execution, TypeScript type analysis, bundler alias
resolution, destructuring semantics, or proof that an exported CSS class exists.

## Tailwind auxiliary boundary

`tailwind-config` and `tailwind-plugin` accept JavaScript/TypeScript module extensions. They pass
bounded lexical-state validation, then retain exact UTF-8 byte count and SHA-256.
The scanner retains the closed config-key set `content`, `theme`, `plugins`, `presets`, `safelist`,
and `corePlugins` only when an identifier is followed by `:` in code. Plugin modules retain
`addUtilities`, `matchUtilities`, `addComponents`, `addVariant`, and `matchVariant` only when the
identifier is followed by `(` in code. These observations carry exact spans and an `unsupported`
disposition: they expose migration ownership without claiming semantic normalization. Comments and
strings cannot create these observations.
`tailwind-template` accepts common HTML/component/template extensions and retains the same exact
identity. Its tag-aware lexical scanner extracts whitespace-separated literal candidates from exact
`class` and `className` attributes with byte spans. Expression, interpolation, or template-literal
values remain a single dynamic observation. HTML comments, attribute strings, and text outside tags
do not create candidates. This is not a complete parser for every accepted template language.

Relative `@config`, `@plugin`, and exact-file `@source` specifiers normalize from their containing
CSS file and resolve only when the target is declared with the required auxiliary kind. A declared
target with the wrong kind fails the snapshot; an undeclared target stays `unresolved`. Package
plugins stay `external`, inline sources stay `dynamic`, and glob/directory discovery stays
`unresolved`. No auxiliary is imported, executed, transpiled, or passed to Tailwind/Node.

## Security and consistency boundary

- every source, consumer, and auxiliary path is explicit, project-relative, UTF-8, kind-compatible,
  and duplicate-free across all roles;
- every component is inspected and symbolic links or Windows reparse points are rejected;
- the final file is opened with no-follow semantics, must remain regular, and is bounded to 16 MiB;
- malformed UTF-8, comments, strings, dependency targets, or per-file defensive limits fail the
  whole snapshot;
- all declared files are inventoried in canonical order and then inventoried again in that order;
- any byte or derived-inventory difference between passes fails instead of mixing revisions.

This is a confirmed declared-file snapshot, not an atomic filesystem transaction. It does not prove
that an intermediate revision never existed, crawl undeclared sources, reproduce toolchain-specific
resolution, discover undeclared templates/consumers, evaluate Tailwind configuration/plugins, or
reproduce arbitrary JavaScript/bundler semantics. Those surfaces and a real migration corpus remain
required before R0.8 can close.

The versioned `integration-tests/migration-project` fixture exercises all three source families in
one declaration, including exact resolved Sass/CSS/CSS Modules targets, local CSS Modules
composition, package/built-in external edges, extensionless Sass unresolved lookup, and an
unsupported Tailwind plugin seam. It is a representative cross-toolchain contract fixture, not yet
a corpus of real migrated applications. Its declared TSX consumer freezes two static CSS Modules
class usages and one computed dynamic usage. Its declared Tailwind config, plugin, and HTML
template freeze three exact auxiliary identities and three resolved relative seams while the
package plugin remains external.
