# pliego-css-ir

`pliego-css-ir` contains the syntax and semantic data model shared by the PliegoCSS parser,
compiler, theme registry, procedural macros, and command-line compiler.

It defines:

- syntax nodes and structured `PCS` diagnostics;
- compact IDs, typed viewport/container/layer conditions, semantic slots, typed values, and
  assignments;
- `SemanticStyle` plus structural invariant validation;
- `StyleId` and the versioned CSS class-name encoding.

## Stability

This is a lockstep implementation crate, not the supported application facade. Its public items are
needed across package boundaries and may evolve with the compiler as one exact-version unit.
Applications should depend on `pliego-css`; direct tooling consumers must use the exact same version
of every PliegoCSS crate and validate incoming `SemanticStyle` values before compiling them. These
items are not covered by the application SemVer promise unless explicitly promoted.

The current workspace is pre-release; this README does not claim that `0.1.0` is published. The
typed-IR and compatibility references live under `docs/` in a release checkout.
