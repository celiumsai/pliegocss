# F0 verification report

Date: 2026-07-12

## Environment

- Rust 1.96.0.
- Cargo 1.96.0.
- Node 24.16.0.
- pnpm 11.7.0.
- Tailwind CSS and CLI 4.3.2.
- Windows 10.0.26200, x64.
- Intel Core Ultra 9 285H, 16 logical CPUs.

## Rust gates

```shell
cargo fmt --all
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

Result at F0 verification: 15 tests passed, zero failed, Clippy clean.

## Tailwind fresh-process baseline

```shell
pnpm baseline:measure
```

Result:

- four profiles measured;
- 30 development and 30 minified samples per profile after warmups;
- median, nearest-rank p95, min, max, and median absolute deviation recorded;
- one distinct output hash per profile and mode;
- source inputs unchanged during measurement;
- class-only scanner control matches the full fixture scan;
- CSS raw/gzip and HTML+CSS gzip recorded.

## Tailwind persistent baseline

```shell
pnpm baseline:measure-watch
```

Result:

- 100 content-only samples;
- 100 additional-known-utility samples;
- 50 new-utility samples;
- wall and engine latency recorded separately.

## Contracts

- `pc!` grammar frozen for the spike.
- `pcx!` syntax reserved for F4.
- 40 initial semantic utility families.
- semantic slots, footprints, refinements, and condition normalization documented.
- ten robustness mutations frozen.
- five fixture contracts and three viewports frozen.
- full and no-preflight reset profiles frozen.

## Work completed ahead of F1

- Syntax AST with byte spans.
- `PCS001` through `PCS012` code enum.
- Balanced tokenizer for brackets, parentheses, quotes, escapes, and real spaces.
- Syntax parsing for variants, negative, important, arbitrary properties, and arbitrary values.
- Compile-time `pc!` macro returning a static `Style` handle.
