# pliego-css-theme

`pliego-css-theme` provides the canonical token and breakpoint registry used by every PliegoCSS
compiler stage. Registries are validated, sorted deterministically, assigned a `ThemeId`, and can be
encoded into the bounded versioned binary artifact consumed during macro expansion.

```rust
use pliego_css_ir::TokenKind;
use pliego_css_theme::ThemeRegistry;

let theme = ThemeRegistry::seed();
let accent = theme
    .token_by_name(TokenKind::Color, "accent")
    .expect("seed theme contains accent");
assert!(!accent.value.is_empty());
```

Applications normally define custom tokens in `pliego.theme.toml` and use the supported
`pliego-css-build::theme!` bridge rather than constructing registries directly.

## Stability

This is a lockstep implementation and advanced tooling crate. Its persisted format versions are
explicit contracts, while its Rust construction types are not covered by the application SemVer
promise unless explicitly promoted. Direct consumers must use exact matching PliegoCSS package
versions.

The current workspace is pre-release and this README does not claim that `0.1.0` is published. The
theme schema lives at `docs/reference/theme-schema.md` in a release checkout.
