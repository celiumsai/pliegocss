<!-- SPDX-License-Identifier: Apache-2.0 -->

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brand/pliegocss-symbol-reversed.svg">
    <img src="brand/pliegocss-symbol.svg" alt="PliegoCSS logo" width="112" height="112">
  </picture>
</p>

<h1 align="center">PliegoCSS</h1>

<p align="center"><strong>Compile confidence into CSS.</strong></p>

<p align="center">
  Standards-first CSS analysis, typed Rust authoring, deterministic artifacts,
  and controlled migration without a styling runtime.
</p>

<p align="center">
  <a href="https://pliegocss.dev/">Website</a> &middot;
  <a href="https://pliegocss.dev/docs/">Documentation</a> &middot;
  <a href="https://pliegocss.dev/changelog/">Changelog</a> &middot;
  <a href="https://pliegocss.dev/security/">Security</a>
</p>

<p align="center">
  <a href="https://github.com/celiumsai/pliegocss/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/celiumsai/pliegocss/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/celiumsai/pliegocss/actions/workflows/codeql.yml"><img alt="CodeQL" src="https://github.com/celiumsai/pliegocss/actions/workflows/codeql.yml/badge.svg"></a>
  <a href="https://crates.io/crates/pliego-cssc"><img alt="crates.io" src="https://img.shields.io/crates/v/pliego-cssc.svg"></a>
  <a href="https://docs.rs/pliego-css"><img alt="docs.rs" src="https://img.shields.io/docsrs/pliego-css"></a>
  <a href="https://github.com/celiumsai/pliegocss/releases"><img alt="GitHub prerelease" src="https://img.shields.io/github/v/release/celiumsai/pliegocss?include_prereleases"></a>
  <a href="LICENSE"><img alt="Apache-2.0" src="https://img.shields.io/crates/l/pliego-cssc.svg"></a>
  <a href="https://doc.rust-lang.org/stable/releases.html"><img alt="rustc 1.85+" src="https://img.shields.io/badge/rustc-1.85%2B-b7410e.svg"></a>
  <img alt="Public preview" src="https://img.shields.io/badge/status-public_preview-315bff.svg">
  <img alt="Public repository" src="https://img.shields.io/badge/repository-public-0a1120.svg">
</p>

> [!IMPORTANT]
> `0.1.0-rc.2` is public-preview software distributed from the public
> canonical repository.
> The nineteen PliegoCSS crates form one exact-version compatibility unit and
> are published on crates.io and replayed from the registry with Rust 1.85
> after the complete hosted CI and CodeQL matrices passed. `0.1.0` remains blocked by the
> [machine readiness record](./docs/product/release-readiness-0.1.0.md).
> Core formats and selected APIs are frozen candidates; beta and experimental
> capabilities may still change.

## Why PliegoCSS?

CSS tooling often asks teams to choose between ordinary CSS, typed authoring, static analysis, and
reproducible build evidence. PliegoCSS puts those workflows behind one Rust implementation while
keeping the browser contract simple: the output is static standards-compliant CSS with no styling
runtime or WebAssembly requirement.

PliegoCSS is designed around four principles:

- **Standards first** — normal CSS remains a first-class input and output.
- **Fail closed** — unsupported or ambiguous analysis is reported instead of guessed.
- **Deterministic artifacts** — CSS, manifests, findings, source maps, token graphs, and receipts are
  hash-bound and reproducible.
- **Incremental adoption** — audit an existing project first; typed Rust styles and reversible
  migration tools are optional layers.

## What PliegoCSS does

| Area | Current capability |
|---|---|
| Standard CSS | Audit, compatibility findings, budgets, accessibility guards, transform/minify |
| Typed Rust | Compile-time checked `pc!` and `pcx!` literals with stable style identities |
| Tokens and themes | TOML themes, DTCG 2025.10 resolver inputs, aliases, provenance, token graphs |
| Build evidence | Canonical manifests, findings, Source Map v3, control receipts, read-only drift checks |
| Adoption | Sass/Tailwind/CSS Modules inventory and bounded reversible Tailwind projections |
| Usage | Generated StyleId analysis and conservative generic-CSS declaration observations |
| Tooling | CLI, stdio LSP, VS Code client candidate, Neovim client candidate |
| Automation | Bounded repair plans, exact edits, authorization, rollback, verification receipts |

The exact maturity and claim boundaries are maintained in the
[product maturity map](./docs/product/maturity-map.md). Competitive coverage is tracked honestly in
the [Tailwind CSS v4.3.2 matrix](./docs/benchmarks/tailwind-v4-competitive-matrix.md).

## Quick start

### Requirements

- Rust **1.85 or newer**
- Git
- Node.js and pnpm are required only for repository development and full verification

Install the public-preview CLI from crates.io:

```console
cargo install pliego-cssc --version '=0.1.0-rc.2' --locked
```

Or clone the repository and build locally:

```console
git clone https://github.com/celiumsai/pliegocss
cd pliegocss
cargo build --locked -p pliego-cssc
```

### 1. Audit ordinary CSS

```console
pliego-cssc audit \
  --input app.css \
  --targets baseline-widely \
  --format human
```

Machine-readable output is available as canonical JSON or SARIF:

```console
pliego-cssc audit --input app.css --targets baseline-widely --format json
pliego-cssc audit --input app.css --targets baseline-widely --format sarif
```

### 2. Transform CSS without changing the authored source

```console
pliego-cssc transform-css \
  --input app.css \
  --output dist/app.css \
  --targets modern \
  --format minified
```

Add `--check` in CI to detect output drift without writing files.

### 3. Compile typed Rust styles

```rust
use pliego_css::{pc, Style};

fn button() -> Style {
    pc!(
        "inline-flex items-center gap-2 rounded-lg bg-accent px-4 py-2 \
         text-sm font-semibold text-white hover:bg-accent-strong \
         focus-visible:ring-2 disabled:opacity-50"
    )
}
```

Compile visible literals from a Rust source tree:

```console
pliego-cssc compile \
  --source src \
  --seed \
  --theme \
  --output dist/app.css \
  --manifest dist/app.manifest.json
```

Unknown utilities, invalid domains, conflicting declarations, and contradictory variants fail during
compilation rather than becoming silent runtime behavior.

## CLI workflows

### Controlled artifact publication

A controlled compile publishes CSS and its evidence as one rollback-capable group:

```console
mkdir -p dist/control
pliego-cssc compile \
  --source src \
  --seed \
  --theme \
  --output dist/control/app.css \
  --manifest dist/control/app.manifest.json \
  --control-dir dist/control
```

The group can include:

- generated CSS;
- Source Map v3;
- specialized manifest;
- canonical token graph;
- findings document;
- control manifest;
- build receipt.

Repeat the command with `--check` for read-only verification.

### DTCG resolver input

```console
pliego-cssc compile \
  --style "flex bg-brand" \
  --tokens examples/product.resolver.json \
  --token-input appearance=dark \
  --theme \
  --output dist/app.css
```

Resolver paths and selections are explicit. JSON token files are never silently discovered.

### Bundle plans

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/bundles \
  --control
```

Bundle plans provide explicit source ownership, route/island partitions, optional reachability
pruning, Asset Plans, and Project Index output.

### Conservative migration inventory

```console
pliego-cssc migration-project-inventory ./existing-project
pliego-cssc migration-project-plan migration-project.json
```

Migration proposals remain `automatic: false`. Apply operations require exact before-bytes and hashes,
and rollback refuses to overwrite drifted files. See the
[reversible migration contract](./docs/reference/reversible-migration-plan-schema-1.md).

### Generic CSS usage evidence

```console
pliego-cssc generic-css-usage \
  --findings pliego.css.findings.json \
  --observed observed-identities.json \
  --scope browser-session-1 \
  --output pliego.css.generic-usage.json
```

Only positive evidence becomes `observed`; missing evidence remains `unknown` and is never relabeled
as dead CSS.

## Architecture

```text
ordinary CSS ───────────────┐
Rust pc!/pcx! literals ─────┼─> parser / source inventory
TOML or DTCG tokens ────────┘            │
                                          v
                                semantic IR + policy
                                          │
                    ┌─────────────────────┼─────────────────────┐
                    v                     v                     v
               static CSS          findings/SARIF      manifests/receipts
```

The workspace is split into small crates with explicit boundaries:

- `pliego-css` — application-facing typed API and macros;
- `pliego-cssc` — command-line interface;
- `pliego-css-build` — compilation, CSS audit, findings, and artifacts;
- `pliego-css-config` / `pliego-css-theme` — themes, DTCG, policy, and token graphs;
- `pliego-css-control` — canonical control manifests and receipts;
- `pliego-css-source` — source and migration inventory;
- `pliego-css-io` / `pliego-css-publication` — bounded I/O and grouped publication;
- `pliego-css-lsp` — bounded LSP 3.18 server;
- `pliego-css-agent` — repair and verification boundary.

All nineteen publishable crates share one exact version and are packaged in dependency order.

## Evidence and compatibility

The release candidate is exercised by hosted CI on:

- Ubuntu with Rust 1.85 and 1.96;
- Windows with Rust 1.85 and 1.96;
- macOS with Rust 1.85 and 1.96;
- bounded fuzz, package extraction/replay, WASM checks, rustdoc, Clippy, and deterministic artifacts.

Local Chrome and Edge computed-style gates cover selected standard-CSS and migration projections.
Firefox and Safari/WebKit behavior is not claimed where no corresponding browser replay exists.
Likewise, the accessibility analyzer provides bounded static evidence and does **not** claim WCAG
certification.

## Documentation

| Topic | Link |
|---|---|
| Documentation index | [`docs/index.md`](./docs/index.md) |
| First compile | [`docs/getting-started/first-compile.md`](./docs/getting-started/first-compile.md) |
| CLI reference | [`docs/reference/cli.md`](./docs/reference/cli.md) |
| Public API candidate | [`docs/reference/public-api.md`](./docs/reference/public-api.md) |
| Compatibility policy | [`docs/reference/compatibility.md`](./docs/reference/compatibility.md) |
| Themes and tokens | [`docs/learn/themes-and-tokens.md`](./docs/learn/themes-and-tokens.md) |
| Maturity map | [`docs/product/maturity-map.md`](./docs/product/maturity-map.md) |
| Release readiness | [`docs/product/release-readiness-0.1.0.md`](./docs/product/release-readiness-0.1.0.md) |
| Release process | [`docs/contributing/release-process.md`](./docs/contributing/release-process.md) |
| Changelog | [`CHANGELOG.md`](./CHANGELOG.md) |

## Development

```console
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
node scripts/check-release.mjs fast
```

The full hosted matrix and release profile add package replay, deterministic concurrency, fuzzing,
portability vectors, editor integrations, and optional external corpora.

## Project boundaries

PliegoCSS does not:

- replace browser cascade, layout, or paint engines;
- ship a styling runtime or require WASM in the browser;
- infer application ownership from filenames;
- execute Tailwind plugins, Sass importers, or project JavaScript during migration inventory;
- treat missing runtime observations as proof that CSS is dead;
- claim complete Tailwind catalog parity, complete browser support, or WCAG certification.

## License

Licensed under the [Apache License 2.0](./LICENSE).

Copyright © Celiums Solutions LLC.
