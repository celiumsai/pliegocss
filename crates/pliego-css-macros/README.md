# pliego-css-macros

`pliego-css-macros` implements the internal `pc_id!` and `pcx_id!` procedural expansions used by
the public declarative `pc!` and `pcx!` wrappers in the `pliego-css` facade. Literals are parsed,
lowered, conflict-checked, and assigned a theme-scoped `StyleId` while Rust compiles.

```rust
use pliego_css::{pc, pcx};

fn main() {
    let base = pc!("flex gap-4");
    let selected = pcx!(
        "rounded-md",
        if true { "bg-accent" } else { "bg-surface" },
    );

    assert!(!base.is_empty());
    assert!(!selected.is_empty());
}
```

## Stability

This is a lockstep implementation crate. Its internal macros return identity values to hygienic
declarative wrappers owned by `pliego-css`; the wrappers therefore survive a Cargo dependency
rename without reading the consumer manifest. Do not depend on this crate directly. Macro syntax and
supported behavior are documented as part of the facade contract, while implementation details may
evolve before the public `0.1.0` freeze.

The current workspace is pre-release and this README does not claim that `0.1.0` is published. The
composition guide lives at `docs/concepts/composition.md` in a release checkout.
