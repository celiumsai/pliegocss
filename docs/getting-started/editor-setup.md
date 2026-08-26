# Editor setup

Status: native crates published; editor adapters remain local candidates

PliegoCSS uses one native language server for both supported editor clients. Editor semantics run
inside `pliego-css-lsp`; the server does not launch `pliego-cssc`. Install the CLI separately when
the project also needs build, watch, audit, or artifact commands.

## Install the native tools

```console
cargo install pliego-css-lsp --version '=0.1.0-rc.3' --locked
cargo install pliego-cssc --version '=0.1.0-rc.3' --locked
pliego-css-lsp --version
pliego-cssc --version
```

Both packages belong to one exact compatibility unit. The CLI is not an LSP runtime dependency.
Choose `discover`, `seed`, or one exact `config` theme. Project Index navigation is optional and
fails closed when source, manifest, Asset Plan, or CSS bindings are stale.

## VS Code 1.105+

Build the unreleased client from the repository root:

```console
pnpm --filter pliegocss-vscode check
pnpm --filter pliegocss-vscode package
```

Install `target/pliegocss-vscode-0.0.0.vsix`, then configure:

```json
{
  "pliegocss.server.path": "/exact/path/to/pliego-css-lsp",
  "pliegocss.theme.mode": "seed",
  "pliegocss.projectIndex.path": "target/site/assets/pliego.index.json"
}
```

`pliegocss.compiler.path` is a deprecated 0.1 compatibility setting. Existing configurations may
retain it, but the server ignores the forwarded path. For `config` mode also set
`pliegocss.theme.config`. Restart the language server after changing theme selection or replacing a
Project Index.

## Neovim 0.12+

Add `editors/neovim` from the exact checkout to `runtimepath`:

```lua
require("pliegocss").setup({
  server = "/exact/path/to/pliego-css-lsp",
  root_dir = vim.fn.getcwd(),
  theme = { mode = "seed" },
  project_index = "target/site/assets/pliego.index.json",
})
```

The optional legacy `compiler` key is accepted by the current client but no longer supplies editor
semantics. For a custom theme use `theme = { mode = "config", path = "/exact/path/pliego.theme.toml" }`.

## Verify the connection

Open a saved Rust file containing:

```rust,no_run
use pliego_css::pc;

fn card_class() -> String {
    pc!("flex gap-4").class_name()
}
```

Expected behavior:

1. completion comes from the compiler catalog;
2. hover includes compiler-emitted CSS;
3. `unknown-thing` publishes `PCS001` at the exact utility range;
4. formatting returns a version-bound literal edit without writing the file;
5. Go to Definition appears only with a verified Project Index.

Repository gates:

```console
pnpm integration:lsp
pnpm integration:vscode-host
pnpm integration:neovim
```

The server supports full-document synchronization and a single local root. Hosted editor evidence,
multi-root workspaces, code actions, semantic tokens, signed packages, and registry publication
remain release work. See [LSP troubleshooting](../troubleshooting/lsp.md).
