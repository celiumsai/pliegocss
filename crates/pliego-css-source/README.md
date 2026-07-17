# pliego-css-source

Static Rust source scanning for the PliegoCSS build pipeline.

The crate parses Rust with `syn`; it never searches source text with regular
expressions. It extracts visible string literals from `pc!(...)`, every clause
of `pcx!(base, clause, ...)`, and every reachable Cartesian composition in
deterministic order. One invocation is capped at 64 compositions, matching the
procedural macro's expansion boundary.

```rust
use pliego_css_source::{InvocationKind, scan_source};

let report = scan_source(r#"let style = pc!("flex gap-4");"#)?;
let InvocationKind::Pc(call) = &report.invocations[0].kind else {
    unreachable!();
};
assert_eq!(call.style.value, "flex gap-4");
# Ok::<(), pliego_css_source::SourceParseError>(())
```

Macro definitions are intentionally not scanned: their bodies are templates,
not reachable invocations. Macro calls nested inside the token trees of other
macros are scanned recursively.

## Application collector

Framework adapters can construct an `ApplicationTopology` from their typed route, component, and
island graph, then collect canonical reachability schema 1 from the same project snapshot:

```rust,no_run
use pliego_css_source::{
    ApplicationComponent, ApplicationRoute, ApplicationTopology,
};

let topology = ApplicationTopology::new()
    .source_root("src")
    .component(
        ApplicationComponent::new("app::home")
            .source_unit("src/styles/home.rs"),
    )
    .route(ApplicationRoute::new("home", "/").component("app::home"));
let reachability = topology.collect(".")?;
std::fs::write("target/pliego.reachability.json", reachability.as_bytes())?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The collector recursively inventories bounded `.rs` inputs with portable paths, parses every unit
with the existing Rust scanner, rejects malformed or unowned `pc!`/`pcx!` calls, and fails on stale
exact-site declarations. `source_unit` is an explicit adapter attestation that every discovered
PliegoCSS call in that unit belongs to the component; `site` is the narrower exact-range seam. It
does not infer routes, components, islands, Cargo reachability, or bundle boundaries from filenames.

## Migration source inventory

Exact-version tooling can conservatively inventory one explicit Sass, Tailwind CSS v4 entry, or CSS
Modules source without executing its original toolchain:

```rust
use pliego_css_source::{MigrationSourceKind, inventory_migration_source};

let inventory = inventory_migration_source(
    MigrationSourceKind::CssModules,
    "src/button.module.css",
    ":local(.button) { composes: reset from global; }",
)?;
assert_eq!(inventory.constructs().len(), 2);
# Ok::<(), pliego_css_source::MigrationInventoryError>(())
```

Schema 1 binds the exact path, byte count, SHA-256, construct ranges, dynamic/unsupported counts,
and Tailwind Preflight reliance. `static` means only that a lexical prelude was inventoried; it does
not authorize transformation. `pliego-cssc migration-inventory` exposes the producer through a
read-only stdout contract.

For multi-file tooling, `MigrationProject` accepts only explicit typed source declarations, sorts
them canonically, rejects duplicate paths, and inventories the complete set twice before emitting a
schema-1 project snapshot. The project snapshot derives conservative Sass, CSS import/reference,
and CSS Modules/ICSS dependency observations. Exact supported local targets must exist in the
declared set with the expected kind; package and built-in references stay external, while dynamic or
toolchain-specific resolution remains visible without guessing. Explicit Tailwind config, plugin,
and template auxiliaries are retained by exact bytes/hash; relative `@config`, `@plugin`, and
exact-file `@source` seams link only to the declared auxiliary kind. Configuration and plugin
modules are lexically validated but never executed. Declared JS/TS CSS Modules consumers can be linked
to exact sources and inventory static property/bracket class usage plus conservative dynamic binding
usage without executing a bundler.
Declared templates additionally inventory exact literal `class`/`className` candidates and retain
expression/interpolation attributes as dynamic observations; this is tag-aware lexical inspection,
not arbitrary template-language execution.
Config/plugin modules also retain a closed set of recognized config-key and registration-API seams
as unsupported observations. They are evidence for migration planning, not executable semantics.
`MigrationProject::from_json` accepts the same closed declaration as bounded schema-1 JSON so it can
be reviewed and checked into a project without introducing repository crawling or source execution;
`MigrationProject::from_file` adds the bounded regular-file and no-link path boundary used by
`pliego-cssc migration-project-inventory`.

## Stability

This is an exact-version tooling and adapter crate used by `pliego-cssc`. The application-topology
collector bridge is selected in `docs/reference/public-api.md`; scanner DTOs/functions and
composition projections remain outside the minimal application surface. Direct adapter/tooling
consumers must pin the matching PliegoCSS package version until an RC activates the promise.

The current workspace is pre-release and this README does not claim that `0.1.0` is published. The
source-discovery contract lives at `docs/reference/cli.md`; migration inventory schema 1 lives at
`docs/reference/migration-inventory-schema-1.md` in a release checkout.
