# Project structure

A seed-theme package needs only its normal Cargo files and the `pliego-css` dependency. A package
with custom tokens or breakpoints adds one build script and either a TOML configuration or an
explicit DTCG Resolver.

```text
my-app/
├── Cargo.toml
├── build.rs
├── pliego.theme.toml
├── pliego.bundles.toml
├── styles.txt
├── src/
│   └── main.rs
└── static/
    └── app.css
```

## `Cargo.toml`

```toml
[package]
name = "my-app"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[dependencies]
pliego-css = { path = "<path-to-PliegoCSS>/crates/pliego-css" }

[build-dependencies]
pliego-css-build = { path = "<path-to-PliegoCSS>/crates/pliego-css-build" }
```

`pliego-css-build` belongs under `[build-dependencies]`, not normal runtime dependencies. It owns
TOML/Resolver parsing and the canonical artifact handoff to the procedural macros.

## `build.rs`

```rust,no_run
fn main() {
    pliego_css_build::theme!("pliego.theme.toml");
}
```

The path is resolved relative to the package root. The script tells Cargo to rerun when the file
changes, validates it, and writes `pliego-css-theme-v1-{theme_id}.bin` atomically under `OUT_DIR`.
The `v1` segment is the canonical binary format version. `OUT_DIR` and its artifact are build
outputs; do not copy or commit them.

For DTCG, replace that one call with:

```rust,no_run
fn main() {
    pliego_css_build::theme!(
        tokens = "product.resolver.json",
        inputs = {
            "appearance" => "dark",
        },
    );
}
```

`inputs = {}` uses Resolver defaults. Input entries are literals and duplicate modifiers fail
closed after case folding. Keep one `theme!` call per package. The binary contains only the selected
registry; invoke the CLI with the same Resolver and inputs to generate matching CSS and the complete
controlled token graph.

## `pliego.theme.toml`

```toml
schema = 1
extends = "seed"

[tokens.color]
brand = "#3366ff"

[tokens.spacing]
gutter = "1.375rem"

[breakpoints]
tablet = "52rem"
```

Keep this file in source control. Its normalized content determines `ThemeId`, which participates in
every class identity produced by `pc!` and `pcx!` for this package.

## `src/main.rs`

```rust
use pliego_css::{Style, pc};

fn shell() -> Style {
    pc!("bg-brand p-gutter tablet:p-8")
}

fn main() {
    println!("{}", shell());
}
```

The utility text must remain visible to the procedural macro. The runtime `Style` contains its ID,
not the utility string, registry, parser, or generated CSS.

## `styles.txt` and `static/app.css`

`styles.txt` is an optional explicit input contract for `pliego-cssc`: one complete style list per
line, with full-line comments beginning with `#`. The CLI can instead scan visible macro literals
from explicitly named Rust files.

For the custom registry shown above, install `pliego-cssc` from the exact PliegoCSS checkout, then
compile from the application root. Create the parent output directory first:

```console
mkdir static
pliego-cssc compile --source src --config pliego.theme.toml --theme --output static/app.css
```

The output directory must already exist. `--config` loads the same registry as `build.rs`, while
`--theme` includes its CSS variables. If `--config` is omitted, the CLI discovers a unique
`pliego.theme.toml` from the working directory and supplied input/source package roots; use
`--seed` to select the built-in registry explicitly. The static scanner parses each supplied file
with `syn`, finds visible `pc!` literals and every statically enumerable `pcx!` composition, and
records their byte ranges in the schema-3 manifest.

Those exact logical paths and byte ranges are the join key for optional
[reachability schema 1](../reference/reachability-schema.md). With an adapter-produced sidecar,
manifest schema 4 projects component, route, island, semantic-declaration, and token ownership
without guessing from the directory layout. Manifest schema 5 uses the same sidecar and adds an
exact, fail-closed projection from those semantic declarations into the final CSS artifact.

A source directory is traversed recursively and includes every discovered `.rs` file; this is not
semantic Cargo reachability analysis. Pass individual files for a narrower scope, or use `--input`,
`--style`, and `--compose` for explicit additions. See the
[composition guide](../concepts/composition.md) for conditional semantics.

The schema-3 manifest records `styleIdFormatVersion`, `classNameFormatVersion`,
`themeIdFormatVersion`, `themeId`, the compatibility-target contract, every style's origins, and
`cssSha256` plus `cssBytes` for binding it to the emitted stylesheet. The current identity-format
values are 2, 1, and 1. Default schema 3 does not model Cargo modules, components, routes, or
islands; opt-in schema 4 imports explicit application ownership and still does not infer Cargo
reachability. Opt-in schema 5 retains that graph and adds physical rule/declaration provenance.

## Optional `pliego.bundles.toml`

Applications that own more than one stylesheet can version their explicit source partition:

```toml
schema = 1
targets = "modern"
format = "minified"

[theme]
kind = "config"
path = "pliego.theme.toml"

[bundles.global]
sources = ["src/styles/global.rs"]
emit-theme = true

[bundles.account]
sources = ["src/routes/account.rs"]
emit-theme = false
```

```console
pliego-cssc bundle --plan pliego.bundles.toml --output-dir static
```

The schema-1 plan resolves its source and theme paths relative to its own directory. By default it
emits one CSS file and one schema-3 manifest per bundle after compiling the complete plan from one
snapshot; explicit schema 4 adds the semantic reachability graph and schema 5 adds its physical CSS
trace. A schema-5 build with explicit reachability may add `--asset-plan --project-index` to emit
the portable load and source-to-output contracts beside those pairs. It does not derive route
ownership or remove stale assets; see the exact
[bundle-plan schema](../reference/bundle-plan.md) and
[Project Index schemas 1 and 2](../reference/project-index-schema.md).

## What ships

- Rust compilation receives the binary registry through build-time environment variables.
- The application runtime receives ID-only `Style` values.
- The browser receives the static `app.css` file.
- The TOML parser, theme registry construction, semantic compiler, and CSS generation do not run in
  the browser.

Start with [Installation](./installation.md), then consult the exact
[theme schema](../reference/theme-schema.md).
