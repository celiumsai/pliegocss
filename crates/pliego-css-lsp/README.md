# pliego-css-lsp

`pliego-css-lsp` is the unreleased standard-LSP transport for PliegoCSS Rust literals. It keeps
open documents in memory, negotiates UTF-16 positions, publishes scanner and `FMT001` diagnostics,
returns collision-safe whole-literal formatting edits, and delegates completion/hover metadata to
the versioned `pliego-cssc` catalog/explain JSON contracts. With an explicit generated Project
Index, Go to Definition follows verified source sites and manifests to final CSS declarations.

Run it over stdio from the project root:

```console
pliego-css-lsp --pliego-cssc /exact/path/to/pliego-cssc --seed
```

Enable source-to-CSS navigation without repository inference:

```console
pliego-css-lsp --project-index target/site/assets/pliego.index.json --seed
```

Without `--seed`, the delegated compiler uses normal conventional theme discovery. `--config`
selects one exact theme file. The server currently requests full document synchronization and only
offers semantic features inside statically supported `pc!`/`pcx!` Rust string literals.
