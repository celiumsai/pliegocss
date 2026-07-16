# Configure custom breakpoints

Use a project theme when the seed `sm`, `md`, and `lg` breakpoints do not match the layout. Schema 1
can override a seed breakpoint or add a new responsive variant.

## 1. Enable the build bridge

Add the build-only dependency alongside the public facade:

```toml
[dependencies]
pliego-css = { path = "../PliegoCSS/crates/pliego-css" }

[build-dependencies]
pliego-css-build = { path = "../PliegoCSS/crates/pliego-css-build" }
```

Create `build.rs` in the package root:

```rust,no_run
fn main() {
    pliego_css_build::theme!("pliego.theme.toml");
}
```

## 2. Define the breakpoints

```toml
# pliego.theme.toml
schema = 1
extends = "seed"

[breakpoints]
md = "50rem"
tablet = "52rem"
content-wide = "72rem"
```

`md` keeps its seed breakpoint ID and receives a new minimum width. `tablet` and `content-wide` are
new named variants. Accepted units are `px`, `rem`, `em`, `ch`, and `vw`; each value must be finite
and greater than zero.

## 3. Use the variants in visible literals

```rust
use pliego_css::{Style, pc};

fn results() -> Style {
    pc!("grid grid-cols-1 gap-4 tablet:grid-cols-2 content-wide:grid-cols-3")
}
```

The macro reads the validated binary registry during Rust compilation. A misspelled breakpoint is
reported as an unknown variant before the application runs.

One utility condition may contain at most one breakpoint. For example,
`tablet:content-wide:grid` is rejected rather than producing contradictory media constraints.

## 4. Emit with the same registry

Custom breakpoints affect `ThemeId`, and `ThemeId` affects every `StyleId`. The CSS generation step
must load the same `pliego.theme.toml` registry used by the macro and call the registry-aware
lowering and emission APIs. Emitting a theme-scoped style under another registry fails with a style
identity mismatch.

Pass both the Rust source and theme configuration to the CLI:

```console
cargo run -p pliego-cssc -- compile --source src --config pliego.theme.toml --theme --output app.css
```

`--config` selects the active registry; `--theme` includes that registry's required CSS variables.
The source scanner handles visible `pc!` and reachable `pcx!` literal compositions. A `--source`
directory is scanned recursively for `.rs` files; this is deterministic traversal, not semantic
Cargo reachability analysis.

For the programmatic emission path, see [How token values reach CSS](../learn/themes-and-tokens.md#how-token-values-reach-css).
For the complete validation rules, see the [theme schema](../reference/theme-schema.md).
