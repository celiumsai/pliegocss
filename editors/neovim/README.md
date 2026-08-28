# PliegoCSS for Neovim

This unreleased client uses Neovim's built-in LSP client and one external `pliego-css-lsp`. It does
not download or embed PliegoCSS executables, and editor semantics do not require `pliego-cssc`.

Add this directory to `runtimepath`, then configure:

```lua
require("pliegocss").setup({
  server = "/exact/path/to/pliego-css-lsp",
  root_dir = vim.fn.getcwd(),
  theme = { mode = "seed" }, -- or discover; config requires path
  project_index = "target/site/assets/pliego.index.json", -- optional
})
```

Only file-backed Rust buffers are started by the `FileType` autocmd. Invalid server/config paths,
theme modes, and missing optional artifacts fail before a client starts. A legacy `compiler` key is
accepted as an unused table member for 0.1 configuration compatibility.

Run the pinned real-host gate from the repository root:

```console
pnpm integration:neovim
```

The headless host opens a real Rust buffer, follows Project Index definition, and observes
shared-engine `PCS001` and cross-clause `PCX003` diagnostics.
