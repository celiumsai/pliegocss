# PliegoCSS for VS Code

This unreleased extension connects file-backed Rust documents to an externally installed
`pliego-css-lsp` process. It provides the completion, hover, diagnostics, formatting, and optional
source-to-CSS navigation capabilities advertised by that server. The extension never downloads or
updates native executables.

Configure `pliegocss.server.path` and `pliegocss.compiler.path` to exact same-version binaries. Set
`pliegocss.theme.mode` to `discover`, `seed`, or `config`; config mode also requires
`pliegocss.theme.config`. Use **PliegoCSS: Restart Language Server** after changing external files.
Set `pliegocss.projectIndex.path` to a generated `pliego.index.json` to enable verified Go to
Definition from a Rust utility literal to its final physical CSS declarations. An empty value keeps
navigation disabled.

Only the first local workspace folder is currently used as the native process working directory.
Virtual and untrusted workspaces are explicitly unsupported.

`pnpm integration:vscode-host` launches the development extension in pinned VS Code 1.105.1 and
exercises real diagnostics plus Project Index definition. Its first run downloads that editor test
runtime; this is test infrastructure and does not change the extension's no-server-download policy.
