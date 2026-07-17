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
| server to client | `textDocument/publishDiagnostics` | Rust parse/scanner diagnostics, PCS syntax diagnostics, compiler-backed per-literal semantic diagnostics, and `FMT001` drift. |
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
- Each syntactically valid visible literal is checked through same-version
  `pliego-cssc --diagnostic-format json check --style ...`. Schema-1 code, severity, message,
  suggestion, replacement, and decoded style range are validated before projection into LSP
  UTF-16 coordinates. Cooked strings with escapes use the safe whole-literal range.
- Semantic results are cached by exact decoded literal, capped at 4,096 entries, and each document
  is capped at 256 semantic literal checks. Exceeding that boundary emits `PCL002` rather than
  starting an unbounded number of compiler processes.
- Full-buffer changes replace the pending semantic job for that URI and restart a 150 ms debounce.
  Due jobs run on a dedicated worker instead of the JSON-RPC thread. A per-URI version registry
  terminates and reaps the direct compiler child when its snapshot becomes stale; cancelled work is
  neither cached nor published. Stdout and stderr are drained concurrently so a full process pipe
  cannot prevent cancellation.
- Multi-clause `pcx!` invocations are projected into one bounded synthetic Rust source containing
  only visible utility literals. The worker delegates that document to same-version
  `pliego-cssc check --source`, accepts only compiler `PCX003` cross-clause findings, and maps the
  synthetic branch range back to the original complete Rust literal token. The temporary source is
  created exclusively and removed after the compiler exits; it is never written into the project.
- Formatting returns edits only; it never writes a source file. The editor remains responsible for
  applying the version-bound edit set.
- Definition consumes Project Index schema 1 or 2 instead of scanning repository structure. It
  verifies the Asset Plan, source snapshot, bundle ownership, manifest/CSS hashes, schema-5 physical
  contract, and exact declaration ranges on every request. Any stale or mixed artifact fails the
  request closed.

The current diagnostic pass covers Rust parsing, macro extraction, PliegoCSS syntax, canonical
formatting, and theme-aware compiler validation of every bounded individual literal. Local findings
publish immediately; uncached compiler checks are debounced and run serially in the background.
Cancellation is scoped to the direct `pliego-cssc` child for one URI/version; PliegoCSS does not
claim arbitrary descendant process-tree termination for a replacement compiler executable.
Compiler-backed `PCX003`
parity now covers semantic overlaps and exact duplicates across independently selectable clauses.
The versioned seven-case negative corpus freezes code, message, and exact range equality for
representative parser, compiler, scanner, formatting, and composition failures. Broader code,
severity, suggestion, replacement, and configuration coverage remains necessary before claiming
complete CLI/editor diagnostic equality.

## Current non-goals

This candidate does not yet provide incremental document changes, workspace folders, code actions,
semantic tokens, or a hosted multi-editor matrix. Those remain
release gates; the existence
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
It then sends a burst of full-buffer versions ending in an unknown utility, requires the protocol
to answer a formatting request before semantic publication, and accepts exactly one version-10
`PCS001` result with the compiler's message and exact UTF-16 range. No stale semantic result may
survive the version check. The gate then starts a 30-second blocking compiler proxy, replaces that
document version, proves the stale PID exits, and requires a fresh compiler result within five
seconds. A subsequent version 13 contains an exact semantic duplicate across two
independent `pcx!` clauses; the gate requires one compiler `PCX003` mapped to the complete literal
token in the second clause. Finally, diagnostic corpus schema 1 applies seven additional buffer
versions and compares each frozen code, message, and byte/UTF-16 range against the CLI or shared
formatter contract. See [the corpus contract](./lsp-diagnostic-corpus.md).

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

Run the real extension-host gate with:

```console
pnpm integration:vscode-host
```

The first run downloads the exact VS Code 1.105.1 test runtime through the official Microsoft test
harness; later runs reuse the ignored local cache. The gate builds external Rust 1.85 server and
compiler binaries, loads the development extension in a real workspace host, follows Go to
Definition to the exact physical CSS range, edits the Rust buffer, and requires the compiler-backed
`PCS001` diagnostic followed by the exact cross-clause `PCX003` diagnostic. The extension itself
still downloads no server or compiler.

## Neovim client candidate

The unreleased module under `editors/neovim` uses Neovim's built-in LSP client. It requires explicit
external server/compiler paths and accepts the same discover/seed/config theme modes plus an optional
Project Index. Its `FileType` autocmd starts only for Rust buffers; invalid paths or modes fail before
spawning a server. The 3,466-byte client payload is exactly `README.md` plus
`lua/pliegocss/init.lua` and contains no download surface.

Run the real second-editor gate with:

```console
pnpm integration:neovim
```

The gate pins Neovim 0.12.4 and verifies the official archive SHA-256 before extraction into an
ignored test cache. Windows x64 uses `nvim-win64.zip`; Debian x86-64 uses
`nvim-linux-x86_64.tar.gz`. In both hosts Neovim opens a file-backed Rust buffer, starts the external
PliegoCSS binaries, follows Project Index definition to the physical CSS range, then observes exact
`PCS001` and cross-clause `PCX003` diagnostics. Runtime download belongs only to the test harness;
the Lua client never downloads Neovim or PliegoCSS binaries.
