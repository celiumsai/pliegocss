# PliegoCSS for Neovim

This unreleased client uses Neovim's built-in LSP client and external, same-version
`pliego-css-lsp` and `pliego-cssc` binaries. It does not download or embed PliegoCSS executables.

Add this directory to `runtimepath`, then configure explicit paths:

```lua
require("pliegocss").setup({
  server = "/exact/path/to/pliego-css-lsp",
  compiler = "/exact/path/to/pliego-cssc",
  root_dir = vim.fn.getcwd(),
  theme = { mode = "seed" }, -- or discover; config requires path
  project_index = "target/site/assets/pliego.index.json", -- optional
})
```

Only file-backed Rust buffers are started by the `FileType` autocmd. Invalid paths, theme modes,
and missing optional artifacts fail before a client starts. The project root determines compiler
discovery and relative configuration behavior.

Run the pinned real-host gate from the repository root:

```console
pnpm integration:neovim
```

The first run downloads the official Neovim 0.12.4 archive into an ignored cache and verifies its
frozen SHA-256 before extraction. The headless host opens a real Rust buffer, follows Project Index
definition, and observes compiler-backed `PCS001` and cross-clause `PCX003` diagnostics. The test
runtime download is gate infrastructure; the client module never downloads an editor or server.
