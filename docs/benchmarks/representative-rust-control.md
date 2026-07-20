# Representative application 4: typed Rust controlled build

Status: **passing local representative application gate**

This standalone Rust 1.85 application exercises the selected typed API with two `pc!` styles and a
`pcx!` branch. It then compiles the visible source through the CLI's controlled mode and verifies the
complete seven-artifact group with `--check`.

## Current evidence

- Standalone `cargo +1.85.0 check --locked` passes.
- Four semantic styles: two direct styles and both reachable `pcx!` branches.
- Provenance contains only `pc` and `pcx` macro kinds.
- CSS: 1,315 bytes, SHA-256
  `14e959cffd34a373b68daaf6d7a0c2b3ef5bfec1ffa5a8cd55ff36491264efc6`.
- Controlled outputs include CSS, Source Map v3, schema-3 style manifest, token graph, findings,
  control manifest, and receipt.
- A second `--check` run leaves all seven byte counts and hashes unchanged.

This gate does not measure a clean Cargo build and does not render the application in a browser. Its
scope is the typed application/API → source scan → deterministic controlled artifact path.

## Reproduction

```console
pnpm integration:representative:rust-control
```

This is application **4 of 5** required before selecting implementation gaps.
