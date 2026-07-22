# pliego-css-lsp

`pliego-css-lsp` is the public-preview standard-LSP transport for PliegoCSS Rust literals. It keeps
open documents in memory, negotiates UTF-16 positions, publishes scanner, formatter, semantic, and
`pcx` diagnostics, and serves completion and hover from the same in-process compiler engine used by
the CLI and watch mode. It does not launch `pliego-cssc` or create temporary source files.

Print the installed package version without entering stdio mode:

```console
pliego-css-lsp --version
```

Run it over stdio from the project root:

```console
pliego-css-lsp --seed
```

Enable source-to-CSS navigation without repository inference:

```console
pliego-css-lsp --project-index target/site/assets/pliego.index.json --seed
```

Without `--seed`, the server discovers `pliego.theme.toml` at the initialized workspace root.
`--config` selects one exact theme file. `--pliego-cssc` and `PLIEGO_CSSC` remain accepted during
the 0.1 compatibility window, but are ignored: semantic analysis is always in-process.

The server requests full document synchronization and offers semantic features only inside
statically supported `pc!`/`pcx!` Rust string literals.
