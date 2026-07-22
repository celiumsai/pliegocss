# Shared compiler engine

Status: implemented G2 architecture; exact-version advanced API during `0.1.x`

PliegoCSS exposes one I/O-free engine in `pliego-css-compiler`. CLI, watch, procedural macros, Rust
source scanning, and LSP transport adapt their own inputs and outputs around this boundary instead
of reimplementing semantic or physical decisions.

## Primary contract

```rust,no_run
use pliego_css_compiler::{AnalysisHost, CompileInput, CompileRequest};

let mut host = AnalysisHost::default();
let result = host.compile(&CompileRequest {
    inputs: vec![CompileInput::literal("flex gap-4")],
    include_theme: false,
    physical_trace: false,
})?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`CompileRequest → CompileResult` is the sole complete compile operation. It owns no filesystem,
environment, process, publication, target optimizer, or protocol behavior. Those remain adapter
boundaries.

## Semantic identity versus physical planning

`StyleId` identifies normalized semantic IR under one exact theme and stays independent from the
physical emission strategy. `PhysicalRulePlanner` consumes validated semantic styles, collision-
checks canonical identity streams, deduplicates them, emits deterministic fragments, and prunes its
cache after each complete snapshot. Later atomic, grouped, hybrid, or shared-chunk planners may
change physical rules without changing semantic identity.

Target-specific Lightning CSS optimization, provenance, reachability selection, manifests,
publication locks, rollback, and receipts remain outside the planner.

## Incremental host

`AnalysisHost` owns:

- one exact `ThemeRegistry`;
- a bounded literal-to-semantic cache, including stable failures;
- one `PhysicalRulePlanner` and its fragment cache;
- cumulative cache statistics for adapter observability.

Changing the theme ID invalidates every theme-bound cache. CLI watch retains one host across
snapshots. The LSP retains one host per active theme/config identity and performs document version
checks around bounded work.

## Shared conditional frontend

`PcxRequest` contains a base literal and all visible branch literals grouped by independent clause.
`AnalysisHost::analyze_pcx` performs the shared operation in this order:

1. parse and lower base and branches through the semantic cache;
2. reject cross-clause slot overlap as `PCX003`;
3. reject more than 64 combinations as `PCX004`;
4. compose every bounded cartesian selection in stable order.

The procedural macro owns Rust expression syntax and runtime selectors. The source scanner owns Rust
byte ranges and provenance. The LSP owns UTF-16 projection. None owns a second conditional semantic
algorithm.

## Stability

These engine types are packaged because exact-version sibling tools need them, but remain an
advanced implementation API in `0.1.x`. The application API remains `pliego_css::{pc, pcx, Style,
StyleId}`. Promotion to a 1.0 library contract requires downstream compile fixtures, semver policy,
and an explicit public-API decision.
