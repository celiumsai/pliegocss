# Language Server Protocol transport

`pliego-css-lsp` implements a bounded subset of the official
[LSP 3.18 specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/)
over framed JSON-RPC on stdin/stdout. Protocol transport and document coordinates remain adapter
concerns; parsing, semantic lowering, conditional analysis, catalog resolution, and physical CSS
emission come from the shared in-process `AnalysisHost`.

## Start the server

Install `pliego-css-lsp` from the same PliegoCSS compatibility unit used by the project:

```console
cargo install pliego-css-lsp --version '=0.1.0-rc.2' --locked
pliego-css-lsp --seed
```

Choose exactly one theme mode:

```console
# Deterministic built-in registry
pliego-css-lsp --seed

# Exact project theme; relative paths resolve from initialize.rootUri
pliego-css-lsp --config pliego.theme.toml

# No theme flag: discover initialize.rootUri/pliego.theme.toml, else seed
pliego-css-lsp

# Optional verified source-to-final-CSS navigation
pliego-css-lsp --project-index target/site/assets/pliego.index.json
```

The 0.1 options `--pliego-cssc PATH` and `PLIEGO_CSSC` remain syntactically accepted so existing
clients keep starting, but the path is ignored and excluded from cache identity. The LSP never
executes a compiler child.

## Implemented protocol surface

The initialize result selects `positionEncoding: "utf-16"`, requests full-document synchronization,
and advertises completion, hover, document formatting, and optionally definition.

| Direction | Method | Contract |
|---|---|---|
| client to server | `initialize`, `initialized`, `shutdown`, `exit` | Process lifecycle and one file-root workspace. |
| client to server | `textDocument/didOpen`, `didChange`, `didClose` | In-memory, monotonically versioned, full-document buffers. |
| server to client | `textDocument/publishDiagnostics` | Scanner, PCS semantic, shared `pcx`, and `FMT001` findings. |
| request | `textDocument/completion` | Compiler catalog examples and summaries with a whole-utility `textEdit`. |
| request | `textDocument/hover` | Shared catalog pattern, summary, and compiler-emitted CSS. |
| request | `textDocument/definition` | Verified Project Index source site to final CSS declaration links. |
| request | `textDocument/formatting` | Complete Rust literal-token edits from canonical decoded values. |

Input frames are capped at 16 MiB. Lines and characters are zero-based UTF-16 positions. The server
converts them from exact UTF-8 source bytes and rejects a position that splits a surrogate pair.
Cursor features fail closed for cooked literals containing escapes because decoded offsets cannot
be treated as Rust source offsets. Formatting remains safe because it replaces the complete token.

## Shared-engine consistency boundary

- Completion reads `utility_catalog()` directly; it does not deserialize catalog JSON.
- Hover resolves the accepted `StyleItem`, lowers it through `AnalysisHost`, and emits CSS with the
  host's exact theme.
- Every syntactically valid literal uses the same parser, semantic cache, and stable `Diagnostic`
  type as CLI and watch.
- Macro expansion, source scanning, and LSP diagnostics all send the same `PcxRequest` to the
  bounded conditional frontend. `PCX003` targets the original conflicting branch token; no
  synthetic source file exists.
- The host cache is theme-bound and capped. A theme ID change invalidates semantic and physical
  entries. Each document remains capped at 256 semantic checks and emits `PCL002` at the boundary.
- Full-buffer changes replace the pending job for that URI and restart a 150 ms debounce. Due work
  runs on a dedicated in-process worker. Version checks before and after analysis suppress stale
  publication without child-process cancellation.
- Definition consumes Project Index schema 1 or 2 and verifies adjacent source, Asset Plan,
  manifest, CSS hashes, and exact declaration ranges on each request.

The versioned twenty-case corpus freezes exact code, message, range, severity, suggestion, and
replacement equality for PCS001–PCS012, PSC001–PSC006, PCR001, and FMT001. The same session compares
CLI and LSP `PCX003`, exercises the 256-literal guard, passes a deliberately nonexistent legacy
compiler path, and reports `compilerProcesses: 0`.

## Current non-goals

This candidate does not yet provide incremental text changes, multi-root workspaces, code actions,
semantic tokens, signed editor packages, or a hosted multi-editor matrix. Full-document sync and a
single local root remain explicit prerelease limits.

Run the reproducible gate with:

```console
pnpm integration:lsp
```

The gate builds Rust 1.85 binaries, compares the LSP diagnostic corpus against CLI output, verifies
UTF-16 ranges, completion, hover, formatting, Project Index navigation, bounded diagnostics, current
version publication, clean shutdown, zero stderr, and zero LSP compiler processes.

## Editor clients

The VS Code manifest retains its historical compiler-path property as deprecated configuration, and
the Neovim table tolerates a legacy `compiler` member. Both clients ignore those values and launch
only the language server. `pliego-cssc` remains a separate build and CI tool.

Run their repository gates with:

```console
pnpm integration:vscode
pnpm integration:vscode-host
pnpm integration:neovim
```

See [Editor setup](../getting-started/editor-setup.md) and
[the diagnostic corpus](./lsp-diagnostic-corpus.md).
