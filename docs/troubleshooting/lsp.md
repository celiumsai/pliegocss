# LSP troubleshooting

Use this page when completion, diagnostics, formatting, or Project Index navigation is missing.
The current clients deliberately fail closed; they do not search the network or replace binaries.

## The server does not start

1. Run both configured executables directly with `--version`.
2. Confirm they come from the same checkout revision and lockfile.
3. Use absolute paths. Neovim requires readable executable files; VS Code resolves its process from
   the first local workspace folder.
4. Confirm the buffer is a saved Rust file. Untitled/virtual buffers are unsupported.
5. Restart the client after changing executable paths.

An unavailable compiler produces `PCL001`; it is not converted into a guessed style diagnostic.
Capture the complete message before replacing paths or theme settings.

## Diagnostics disagree with the CLI

Reproduce from the editor workspace root with the same theme selection:

```console
pliego-cssc --diagnostic-format json check --source src --seed
```

Replace `--seed` with the exact configured TOML or DTCG selection when appropriate. Current corpus
schema 2 requires CLI/LSP equality for code, message, range, severity, category, suggestion, and
typed replacement across PCS, PSC, PCR001, FMT001, and PCX003. `PCL001` and `PCL002` are operational
fallbacks tested through fault injection.

If only an escaped cooked Rust literal has a wider range, that is intentional: decoded offsets
cannot always be mapped safely through escapes, so the server selects the complete literal token.

## Theme-backed features fail

- `discover`: verify that exactly one `pliego.theme.toml` is discoverable within the package
  boundary.
- `seed`: confirm the editor and CLI both opt into seed explicitly.
- `config`: use one readable exact path; relative-path roots differ between editor processes.

Restart the language server after editing external theme files. Semantic cache entries are scoped to
the server process and selected registry.

## Go to Definition is absent or rejected

Definition is advertised only when `pliego.index.json` is configured. The server then revalidates:

- the current Rust source bytes and source-site range;
- Project Index, Asset Plan, manifest, and CSS schemas;
- byte lengths and SHA-256 digests;
- schema-5 physical declaration coverage and target ranges.

Regenerate the entire verified artifact group. Editing only the JSON path or CSS file creates a
mixed snapshot and must continue to fail.

## Diagnostics arrive twice or appear stale

Local parser/scanner/format diagnostics publish immediately; compiler-backed diagnostics arrive
after the 150 ms debounce. A newer full-document version cancels the stale direct compiler child and
its result is neither cached nor published. If an editor plugin sends incremental changes instead of
the negotiated full-document synchronization, the server rejects that request.

## A large file reports PCL002

One document is capped at 256 semantic utility-literal checks. `PCL002` with
`semantic diagnostic literal limit exceeded` means the server stopped before launching an unbounded
number of compiler processes. Split generated or unusually large Rust source units; do not suppress
the finding and assume the remaining literals were validated.

## Collect reproducible evidence

From the PliegoCSS checkout run:

```console
pnpm integration:lsp
pnpm integration:vscode-host
pnpm integration:neovim
```

Report the editor version, OS, exact PliegoCSS commit, both binary paths/versions, theme mode, and
whether Project Index navigation was enabled. Do not attach proprietary source or theme documents
without redaction and authorization.
