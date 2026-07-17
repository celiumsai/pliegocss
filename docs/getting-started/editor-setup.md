# Editor setup

Status: local unreleased clients; no Marketplace or plugin-registry publication

PliegoCSS uses one native language server for both supported editor clients. The editor adapter is
small and never downloads a server: install `pliego-css-lsp` and `pliego-cssc` from the same exact
checkout, then configure both executable paths explicitly.

## Install matching native tools

From the pinned PliegoCSS checkout:

```console
cargo install --path crates/pliego-cssc --locked
cargo install --path crates/pliego-css-lsp --locked
pliego-cssc --version
pliego-css-lsp --version
```

Both commands must report the same version. While the workspace remains unpublished at `0.0.0`,
the checkout revision plus `Cargo.lock` is the distribution identity. Do not mix binaries from
different revisions merely because their experimental version strings match.

Choose one theme mode:

- `discover` finds one conventional `pliego.theme.toml` from the workspace/package boundary;
- `seed` opts into the deterministic built-in registry;
- `config` requires one exact TOML path.

Project Index navigation is optional. Generate `pliego.index.json` through the documented bundle
pipeline before configuring it; the server rejects stale source, manifest, Asset Plan, or CSS
bindings instead of navigating to unverified output.

## VS Code 1.105+

The candidate extension is local and unreleased. Contributors can build and package it from the
repository root:

```console
pnpm --filter pliegocss-vscode check
pnpm --filter pliegocss-vscode package
```

Install the generated `target/pliegocss-vscode-0.0.0.vsix` through VS Code's **Install from VSIX**
action. Configure workspace settings with absolute executable paths:

```json
{
  "pliegocss.server.path": "/exact/path/to/pliego-css-lsp",
  "pliegocss.compiler.path": "/exact/path/to/pliego-cssc",
  "pliegocss.theme.mode": "seed",
  "pliegocss.projectIndex.path": "target/site/assets/pliego.index.json"
}
```

For `config` mode, also set `pliegocss.theme.config`. Run **PliegoCSS: Restart Language Server**
after replacing a native binary, changing theme selection, or regenerating an external Project
Index. Only the first local workspace folder is supported; virtual and untrusted workspaces fail
closed.

## Neovim 0.12+

Add `editors/neovim` from the exact checkout to `runtimepath`, then configure the built-in LSP
client:

```lua
require("pliegocss").setup({
  server = "/exact/path/to/pliego-css-lsp",
  compiler = "/exact/path/to/pliego-cssc",
  root_dir = vim.fn.getcwd(),
  theme = { mode = "seed" },
  project_index = "target/site/assets/pliego.index.json",
})
```

For a custom theme use `theme = { mode = "config", path = "/exact/path/pliego.theme.toml" }`.
Omit `project_index` when physical Go to Definition is not generated. The module starts only for
file-backed Rust buffers and validates every configured file before launching the client.

## Verify the connection

Open a saved Rust file containing a visible utility literal, for example:

```rust,no_run
use pliego_css::pc;

fn card_class() -> String {
    pc!("flex gap-4").class_name()
}
```

Expected behavior:

1. completion inside the literal is catalog-backed;
2. hover shows compiler-emitted CSS;
3. changing `gap-4` to `unknown-thing` publishes `PCS001` at the exact utility range;
4. formatting returns a version-bound literal edit without writing the file itself;
5. Go to Definition is advertised only when a Project Index path was configured.

Repository gates use pinned real hosts:

```console
pnpm integration:vscode-host
pnpm integration:neovim
pnpm integration:lsp
```

The first two commands may download their pinned editor test runtimes. That download belongs to the
test harness; neither client downloads PliegoCSS executables in normal operation.

## Current boundary

The server supports full-document synchronization, not incremental text edits. Semantic checks are
debounced for 150 ms, stale direct compiler children are cancelled per URI/version, and current
typed diagnostic families are checked against CLI schema 1 through corpus schema 2. Hosted editor
evidence, multi-root workspaces, code actions, semantic tokens, signed packages, and Marketplace or
plugin-registry publication remain release work.

See [LSP troubleshooting](../troubleshooting/lsp.md) for startup, theme, diagnostics, and navigation
failures.
