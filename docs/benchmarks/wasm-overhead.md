# WASM runtime overhead

Date: 2026-07-13

Status: paired micro-fixture passes

Two same-shape `cdylib` crates were built for `wasm32-unknown-unknown` with size optimization, fat
LTO, embedded bitcode and aborting panics. Both export one function returning the same low 64 bits
of a 128-bit style identity:

- control: the identity is a plain Rust constant;
- styled: the identity comes from `const STYLE: Style = pc!("...")`.

| Artifact | Raw | Gzip level 9 |
|---|---:|---:|
| Plain control | 424 B | 362 B |
| `pc!` style | 424 B | 361 B |
| Delta | 0 B | -1 B |

The one-byte gzip difference is compression noise; raw output is identical in size. This confirms
the current boundary: parsing, semantic lowering, diagnostics and CSS emission remain host-side,
while the WASM-visible `Style` handle is only its 128-bit `StyleId`.

This micro-fixture does not replace the later paired PliegoRS application measurement. Framework
integration, view code, resumability, route manifests and real component counts may affect the final
artifact independently of the style handle.

Run locally with:

```console
pnpm baseline:measure-wasm
```

Machine-local details are written to `benchmarks/results/wasm-overhead.json` and ignored by Git.
