# pliego-css-parser

`pliego-css-parser` parses the utility grammar used by PliegoCSS style literals. It preserves exact
byte spans and produces structured `pliego-css-ir::Diagnostic` errors; catalog lookup, token
resolution, and conflict analysis happen later in `pliego-css-compiler`.

```rust
use pliego_css_parser::{format_style_list, parse_style_list};

let syntax = parse_style_list("hover:bg-accent   p-4")?;
assert_eq!(format_style_list(&syntax), "hover:bg-accent p-4");
# Ok::<(), pliego_css_ir::Diagnostic>(())
```

The crate also exposes operand and named-candidate parsers used by compiler tooling.

## Stability

This is a lockstep implementation crate. Applications should use `pliego-css::{pc, pcx}` instead of
calling the parser directly. Tooling that does consume it must pin exact matching PliegoCSS package
versions; AST details and staged parser helpers are not covered by the application SemVer promise
unless explicitly promoted.

The current workspace is pre-release and this README does not claim that `0.1.0` is published. The
grammar reference lives at `docs/reference/syntax.md` in a release checkout.
