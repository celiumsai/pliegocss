# PliegoCSS for VS Code

This unreleased extension launches an external `pliego-css-lsp` for file-backed Rust documents. It
does not download or embed native executables, and editor semantics do not require `pliego-cssc`.

Configure `pliegocss.server.path`, select `discover`, `seed`, or `config` theme mode, and optionally
set `pliegocss.projectIndex.path` for verified source-to-CSS navigation. The historical
`pliegocss.compiler.path` setting is deprecated and ignored.

Build and test from the repository root:

```console
pnpm integration:vscode
pnpm integration:vscode-host
```

The real-host gate uses VS Code 1.105.1, opens a Rust buffer, verifies Project Index definition, and
observes shared-engine `PCS001` and `PCX003` diagnostics.
