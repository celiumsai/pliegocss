# pliego-css

`pliego-css` is the supported application facade for PliegoCSS, a utility-first CSS compiler built
for Rust applications and PliegoRS. The `pc!` and `pcx!` macros validate visible style literals while
Rust compiles and return a compact `Style` handle containing only a semantic `StyleId`.

```rust
use pliego_css::{Style, pc, pcx};

const CARD: Style = pc!("rounded-lg bg-surface p-6");

fn button(active: bool) -> Style {
    pcx!(
        "rounded-md",
        if active { "bg-accent text-white" } else { "bg-transparent text-accent" },
    )
}
```

`Style` implements `Display` and converts into `String`, so renderers can use it directly as a class
attribute. Macro expansion does not write a stylesheet; run `pliego-cssc` against the same visible
sources to emit the matching static CSS.

Custom themes are configured from `build.rs` with the supported `pliego-css-build::theme!` entrypoint.

## Stability

The supported candidate application surface is `Style`, `StyleId`, `pc!`, and `pcx!`. The repository
is still at its pre-release workspace version and this README is not an announcement that `0.1.0`
has been published. The lower-level parser, IR, compiler, theme, and proc-macro crates are lockstep
implementation packages.

Installation and syntax references live under `docs/getting-started` and `docs/reference` in a
release checkout. Repository reachability remains a release gate.
