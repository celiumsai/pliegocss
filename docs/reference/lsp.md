# Language Server Protocol transport

`pliego-css-lsp` is the initial standard-LSP transport for PliegoCSS Rust literals. It implements a
bounded subset of the official [LSP 3.18 specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/)
over framed JSON-RPC on stdin/stdout. The transport owns protocol and document-coordinate concerns;
it does not fork the compiler's utility catalog or explanation semantics.

## Start the server

Install or build `pliego-css-lsp` and `pliego-cssc` from the exact same PliegoCSS version, then start
the server from the project root:

```console
pliego-css-lsp --pliego-cssc /exact/path/to/pliego-cssc --seed
```

`PLIEGO_CSSC` is the default compiler selector when `--pliego-cssc` is omitted. Choose exactly one
theme mode:

```console
# Deterministic built-in seed theme
pliego-css-lsp --seed

# Exact project theme
pliego-css-lsp --config pliego.theme.toml

# No theme flag: conventional discovery from the initialize root
pliego-css-lsp

# Optional verified source-to-final-CSS navigation
pliego-css-lsp --project-index target/site/assets/pliego.index.json
```

The server runs delegated compiler commands from the `initialize.rootUri` file directory. Compiler
startup failures and unsupported catalog/explain schema versions fail the affected request instead
of returning guessed metadata.

## Implemented protocol surface

The initialize result selects `positionEncoding: "utf-16"`, requests full-document synchronization,
and advertises completion, hover, document formatting, and optionally definition when a Project
Index is configured. The current closed method set is:

| Direction | Method | Contract |
|---|---|---|
| client to server | `initialize`, `initialized`, `shutdown`, `exit` | Process lifecycle and one file-root workspace. |
| client to server | `textDocument/didOpen`, `didChange`, `didClose` | In-memory, monotonically versioned, full-document buffers. |
| server to client | `textDocument/publishDiagnostics` | Rust parse/scanner diagnostics, PCS syntax diagnostics, and `FMT001` drift. |
| request | `textDocument/completion` | Catalog schema 3 examples and summaries, with a whole-utility `textEdit`. |
| request | `textDocument/hover` | Explain schema 2 pattern, summary, and emitted CSS. |
| request | `textDocument/definition` | Exact Project Index source site to integrity-bound manifest and final CSS declaration links. Advertised only with `--project-index`. |
| request | `textDocument/formatting` | Complete Rust literal-token edits produced from canonical decoded values. |

Input frames are capped at 16 MiB. LSP lines and characters are zero-based UTF-16 positions; the
server converts them from exact UTF-8 source bytes and rejects a position that splits a surrogate
pair. This is intentionally different from CLI diagnostic columns, which count Unicode scalar
values.

Completion and hover operate only inside statically supported `pc!` and `pcx!` string literals. Raw
strings and ordinary strings without escapes have an exact source-to-decoded mapping. A cooked
literal containing escapes fails closed for cursor features because a decoded offset cannot safely
be treated as a Rust source offset. Document formatting remains safe in that case because it
replaces the complete literal token with a newly escaped ordinary Rust string.

## Consistency boundary

- Completion invokes `pliego-cssc catalog --format json` once and requires catalog schema 3.
- Hover invokes `pliego-cssc explain --style ... --format json` and requires explain schema 2.
- Open-buffer syntax and formatting use the same `pliego-css-source` scanner and
  `pliego-css-parser` canonical formatter as the CLI.
- Formatting returns edits only; it never writes a source file. The editor remains responsible for
  applying the version-bound edit set.
- Definition consumes Project Index schema 1 or 2 instead of scanning repository structure. It
  verifies the Asset Plan, source snapshot, bundle ownership, manifest/CSS hashes, schema-5 physical
  contract, and exact declaration ranges on every request. Any stale or mixed artifact fails the
  request closed.

The current diagnostic pass covers Rust parsing, macro extraction, PliegoCSS syntax, and canonical
formatting. Theme-aware semantic/compiler diagnostics are not yet evaluated on every keystroke, so
this candidate does not yet prove complete CLI/editor diagnostic equality.

## Current non-goals

This candidate does not yet provide incremental document changes, workspace folders, code actions,
semantic tokens, cancellation, background/debounced compiler checks, a real extension-host gate,
another editor client, or a hosted multi-editor matrix. Those remain release gates; the existence
of the stdio server and initial VS Code package does not close the full F6 editor-tooling task.

Run the reproducible local process gate with:

```console
pnpm integration:lsp
```

The gate builds both binaries with Rust 1.85, performs one framed stdio session, and asserts UTF-16
initialization, `FMT001` publication, whole-literal formatting, exact completion replacement,
compiler-backed hover CSS, clean shutdown, and zero stderr.
The same session configures a synthetic integrity-bound Project Index and requires Go to Definition
to select the exact physical declaration in its verified stylesheet.

## VS Code client candidate

The unreleased client under `editors/vscode` follows the official VS Code language-client pattern
and selects only file-backed Rust documents. Configure `pliegocss.server.path` and
`pliegocss.compiler.path` to same-version external binaries; the extension does not download or
update either executable. Theme mode is explicitly `discover`, `seed`, or `config`. Optional
`pliegocss.projectIndex.path` forwards one exact workspace-relative or absolute index path and is
empty by default.

The client runs in the workspace extension host, uses the first local workspace folder as process
working directory, restarts on configuration changes or through **PliegoCSS: Restart Language
Server**, and declares virtual and untrusted workspaces unsupported. Build and inspect its VSIX with:

```console
pnpm integration:vscode
```

That gate type-checks the client, tests fail-closed argument construction, bundles its production
entry, verifies the exact four-file extension payload, packages a VSIX below 256 KiB, and confirms
that no native server is embedded. The VSIX hash is evidence for one run, not a reproducibility
claim because the packaging tool may encode timestamps.
