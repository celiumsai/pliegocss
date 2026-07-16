# ADR-0006: Keep CSS post-processing outside the macro graph

Status: accepted after Gate A

## Context

`pc!` needs the parser, semantic lowering, conflicts, and `StyleId` derivation while Rust compiles.
It does not need a standards CSS parser or browser-target transforms. When Lightning CSS was an
optional dependency of `pliego-css-compiler`, Cargo feature unification enabled it for the compiler
instance used by the procedural macro whenever the CLI was built in the same workspace.

That widened the host dependency graph, increased macro build work, and caused generated `trybuild`
executables to be blocked by Windows Application Control.

## Decision

- `pliego-css-compiler` owns semantic lowering and deterministic raw CSS emission.
- `pliego-css-macros` depends only on that lightweight compiler path.
- `pliego-cssc` owns Lightning CSS parsing, minification, and future browser-target transforms.
- CSS post-processing must not be exposed as a compiler feature that Cargo can unify into the macro
  dependency graph.

## Consequences

- Compile-time diagnostics remain standards-independent and fast enough for incremental Rust work.
- The CLI is the production artifact boundary and must always validate the combined stylesheet.
- Browser targets and prefixes can evolve without rebuilding the procedural macro dependency graph.
- Any future shared post-processing API belongs in a separate crate that the macro never depends on.
