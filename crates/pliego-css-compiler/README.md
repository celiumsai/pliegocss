# pliego-css-compiler

`pliego-css-compiler` lowers parsed PliegoCSS utilities into typed semantic IR, detects conflicts,
derives theme-scoped `StyleId` values, and emits deterministic CSS.

The hidden incremental cache reuses raw fragments by canonical semantic stream. The build-artifact
layer can also reuse final output for byte-identical raw CSS; changed raw CSS still receives
whole-artifact optimization, so cache reuse does not alter CSS bytes or cascade order.

```rust
use pliego_css_compiler::{emit_css, lower_style};
use pliego_css_parser::parse_style_list;

let syntax = parse_style_list("flex gap-4 hover:bg-accent")?;
let style = lower_style(&syntax)?;
let css = emit_css(&style)?;
assert!(css.contains(".pc_"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

Registry-aware variants accept one shared `pliego-css-theme::ThemeRegistry`. The crate also owns the
utility catalog, conditional composition analysis, typed container and fixed-order cascade-layer
conditions, StyleId format 2 encoding, theme CSS emission, and semantic IR binary format 2. Callers
composing mutable or untrusted semantic IR must use `try_compose_style_override` or
`try_compose_style_override_with_theme`; these validate both inputs before indexing intern tables and
return structural invariant errors. The infallible composition wrappers are for compiler-produced,
prevalidated IR. The IR
artifact is a canonical, theme-aware persistence envelope
with resolved assignment records and portable spans; it is separate from the one-way identity stream
and adds no Serde dependency.

`emit_theme` emits the complete variable-backed registry surface. `emit_used_theme` accepts the
retained styles for one artifact; `emit_theme_references` accepts the application-wide retained
token set used by grouped bundle emission. Both pruned paths emit only directly referenced
variable-backed tokens and return `:root{}` when none are needed. Callers must establish the
retention proof first and must
account separately for authored CSS that consumes custom properties through `var(...)`.

## Stability

This is a lockstep implementation and tooling crate, not the supported application facade. Its
public functions exist so `pliego-css-macros`, `pliego-cssc`, and build tooling can share one compiler
contract. Direct consumers must pin exact matching PliegoCSS package versions; advanced composition,
catalog, and identity APIs are not covered by the application SemVer promise unless explicitly
promoted by a later release contract.

The current workspace is pre-release and this README does not claim that `0.1.0` is published. The
CSS emission, StyleId, and semantic-IR binary references live under `docs/` in a release checkout.
