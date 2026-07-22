<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/tooling/lsp/","category":"Tooling","eyebrow":"Language server","order":23}
-->

# Compiler feedback without editor subprocesses.

The LSP uses the same in-process `AnalysisHost` as CLI and watch without launching `pliego-cssc` or writing synthetic source files.

## Diagnostics, completion, and hover {#features}

Visible `pc!` and `pcx!` literals receive shared diagnostics, catalog-backed completion, and compiler-emitted hover CSS under the exact active theme.

## Suppress stale document work {#cancellation}

Document versions are debounced for 150 ms, and version checks around bounded in-process analysis prevent obsolete results from being published.

## Bound analysis {#limits}

Each document is capped at 256 semantic checks, the shared cache is bounded and theme-invalidated, and excessive input emits `PCL002`.
