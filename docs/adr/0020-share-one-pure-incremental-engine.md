# ADR-0020: Share one pure incremental compiler engine

- Status: Implemented; owner acceptance pending
- Date: 2026-07-22

## Context

The semantic compiler was already deterministic and I/O-free, but adapters did not share one
complete execution boundary. CLI code owned physical deduplication and fragment caching, procedural
macros and source scanning duplicated `pcx` conflict/cartesian logic, and the LSP launched
`pliego-cssc` once per uncached literal or synthetic conditional document. That made editor latency,
cancellation, cache identity, and semantic parity depend on process orchestration.

Adding a new crate would also change the exact nineteen-crate publication unit during an RC.

## Decision

Use `pliego-css-compiler` as the pure shared engine and add:

- `CompileRequest → CompileResult` as the complete compile API;
- `AnalysisHost` for theme-bound semantic and physical caches;
- `PhysicalRulePlanner` as the explicit semantic-to-physical boundary; and
- `PcxRequest → PcxAnalysis` as the shared bounded conditional frontend.

The engine performs no filesystem, environment, process, optimizer, publication, or protocol I/O.
CLI and watch retain artifact/provenance/Lightning CSS concerns. Macros retain Rust syntax and
runtime selection. LSP retains JSON-RPC, UTF-16 coordinates, document versions, and Project Index
navigation.

`StyleId` remains semantic identity. Physical grouping is planner policy and may evolve without
changing the ID contract.

The legacy LSP compiler-path option remains accepted during `0.1.x` but is ignored. It is not part
of cache identity and cannot cause a child process to run.

## Rejected alternatives

### Add `pliego-css-engine` as a twentieth crate

Rejected for G2 because it broadens the locked publication graph without adding a necessary
ownership boundary.

### Keep CLI subprocesses behind an LSP cache

Rejected because process startup, temporary source mapping, stdout schemas, and PID cancellation
remain divergent execution semantics even when cached.

### Share only helper functions

Rejected because helpers do not establish one request/result boundary or one cache owner.

## Consequences

- CLI, watch, and LSP execute one compiler engine.
- Macro, scanner, and editor conditional results share one bounded algorithm.
- LSP semantic work has zero compiler child processes and zero temporary Rust files.
- Publication and protocol failure domains stay outside the compiler.
- Engine Rust APIs remain exact-version advanced surfaces until explicitly promoted for 1.0.

## Verification

`pnpm check:engine-boundary` freezes ownership and rejects old process/duplication markers.
Compiler and LSP unit tests cover deterministic results, cache reuse, theme invalidation, `pcx`
composition/conflicts, source ranges, and configuration identity. `pnpm integration:lsp` passes a
nonexistent legacy compiler path, compares the twenty-case CLI/LSP corpus and `PCX003`, verifies
stale-version suppression, and reports `compilerProcesses: 0`.
