<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/editor-setup/","category":"Start","eyebrow":"Editor setup","order":9}
-->

# Put the compiler engine in the feedback loop.

Use `pliego-css-lsp` for diagnostics, completion, hover evidence, and catalog-aware authoring without launching the build CLI.

## VS Code extension {#vscode}

The maintained extension launches `pliego-css-lsp`, discovers the project theme, and renders diagnostics in Rust source.

```console
pnpm --filter pliegocss-vscode package
```

## Any LSP client {#generic}

Launch the server over stdio from the same workspace used by CI. The protocol stays editor-neutral.

```console
pliego-css-lsp --seed
```

## Bound in-process work {#safety}

Document versions debounce for 150 ms, stale results are suppressed, semantic caches are bounded, and each document stops at 256 checks with `PCL002`.
