# Installation

PliegoCSS `0.1.0-rc.2` is a public-preview, exact-version compatibility unit.
The nineteen crates are published together and must not be mixed across
PliegoCSS versions.

## Requirements

- Rust 1.85 or newer is the minimum supported Rust version (MSRV).
- Cargo is required for the procedural macro and build-script integration.
- A browser receives the generated CSS; no styling runtime is required in WebAssembly.

The PliegoCSS repository pins Rust 1.96 with `rust-toolchain.toml` for contributor tooling. That pin
does not raise the crate MSRV above Rust 1.85.

Node.js 22.13 or newer and pnpm are needed for this repository's verification, packaging, benchmark,
and integration harnesses. They are not dependencies of the Rust crates. Node is also required when
using the optional repository-hosted pnpm launcher described below; that launcher never downloads a
binary during installation or execution.

Registry consumers should use exact versions for the compatibility unit:

```toml
[dependencies]
pliego-css = "=0.1.0-rc.2"

[build-dependencies]
pliego-css-build = "=0.1.0-rc.2"
```

Install the CLI with the same exact candidate:

```console
cargo install pliego-cssc --version '=0.1.0-rc.2' --locked
```

## Repository-hosted pnpm package

The G5 Node distribution is an npm-format package, but PliegoCSS does **not** publish it to npmjs.
It is built as one universal `.tgz` containing the declared native CLI/LSP binaries and is attached
only to an immutable GitHub Release. The package has zero dependencies and no lifecycle scripts.

`0.1.0-rc.2` predates this contract and does not carry that asset. For a later candidate whose
release page contains the G5 bundle, download and verify the exact asset before asking pnpm to install
the local file:

```console
gh release download <tag> --repo celiumsai/pliegocss --pattern 'pliegocss-pnpm-*.tgz'
gh release verify-asset <tag> ./pliegocss-pnpm-<version>.tgz --repo celiumsai/pliegocss
gh attestation verify ./pliegocss-pnpm-<version>.tgz --repo celiumsai/pliegocss
pnpm add --save-dev --save-exact ./pliegocss-pnpm-<version>.tgz
pnpm exec pliego-cssc --version
pnpm exec pliego-css-lsp --version
```

Do not install from a branch, a moving Git reference, npmjs, or an unverified remote tarball. Commit
the resulting `pnpm-lock.yaml`; pnpm is the supported and recommended package manager for this path.
Windows x64, Linux x64 GNU, and macOS arm64 are the declared binary hosts. Use Cargo from an exact
version or checkout on any other Rust target.

## Install the command-line compiler

From an exact PliegoCSS checkout, install the workspace binary with its lockfile:

```console
cargo install --path crates/pliego-cssc --locked
pliego-cssc --version
```

The installed command can validate an application without writing artifacts:

```console
pliego-cssc check --source src
```

For a custom registry, add `--config pliego.theme.toml`; use `--seed` when the
built-in registry is intentional. A source checkout is identified by its
revision and `Cargo.lock`; a registry installation is identified by the exact
prerelease version.

Current PliegoRS can delegate the same read-only validation without linking the compiler:

```console
pliego css check --seed
```

The wrapper invokes the separately installed `pliego-cssc`, defaults to `--source src`, and forwards
additional check options. Use `--config pliego.theme.toml` instead of `--seed` for a custom registry.

## Seed theme setup

Add the public facade to the application:

```toml
[dependencies]
pliego-css = "=0.1.0-rc.2"
```

Use a visible string literal so the macro can validate it during compilation:

```rust
use pliego_css::{Style, pc};

fn card() -> Style {
    pc!("flex items-center gap-4 rounded-lg bg-surface p-6")
}

fn main() {
    println!("{}", card());
}
```

Equivalent normalized utility lists produce the same class. `Style` implements `Display` and holds
only the resulting 128-bit `StyleId`.

Check the package:

```console
cargo check
```

No `build.rs` is required for the seed registry.

## Custom theme setup

Add the build bridge when the package has a custom `pliego.theme.toml` or DTCG Resolver:

```toml
[dependencies]
pliego-css = "=0.1.0-rc.2"

[build-dependencies]
pliego-css-build = "=0.1.0-rc.2"
```

Create `build.rs`:

```rust,no_run
fn main() {
    pliego_css_build::theme!("pliego.theme.toml");
}
```

Create the minimum valid theme:

```toml
schema = 1
extends = "seed"
```

The build dependency parses TOML and produces the canonical registry artifact. Neither
`pliego-css-config`, serde, nor the TOML parser enters the application runtime dependency graph.
Continue with [Themes and tokens](../learn/themes-and-tokens.md) before adding custom values.

For a DTCG Resolver, keep the same dependency and select one context in that single package-level
`build.rs` call:

```rust,no_run
fn main() {
    pliego_css_build::theme!(
        tokens = "product.resolver.json",
        inputs = {
            "appearance" => "dark",
        },
    );
}
```

The path is relative to `CARGO_MANIFEST_DIR`. Use `inputs = {}` for Resolver defaults. Input names
and contexts are string literals; unknown or exact/case-fold duplicate modifiers fail the build.
Call `theme!` once per package.

## Generate a CSS artifact

After installing the exact CLI candidate, run the explicit extraction from the application
directory:

```console
pliego-cssc compile \
  --style "flex items-center gap-4 rounded-lg bg-surface p-6" \
  --seed --theme \
  --output pliego.css
```

On PowerShell, place the command on one line or replace the shell continuations appropriately.
Repeat `--style` for additional lists, pass `--input styles.txt` with one list per non-comment line,
or scan a Rust file explicitly:

```console
pliego-cssc compile --source src --seed --theme --output pliego.css
```

For a custom theme, provide the same configuration used by `build.rs`:

```console
pliego-cssc compile --source src --config pliego.theme.toml --theme --output pliego.css
```

`--seed`, `--config`, or `--tokens` selects the registry; the three options are mutually exclusive.
`--theme` only prepends the selected registry's CSS variables. `--config` selects an exact TOML
file, while `--tokens` selects an exact DTCG Resolver JSON and never participates in discovery. If
none is present, the CLI auto-discovers a unique `pliego.theme.toml` from the working directory and
the nearest package roots of supplied `--input`/`--source` paths. Discovery stops at a `Cargo.toml`
boundary; multiple matches fail closed. Pass `--seed` when the built-in registry is intentional.
`--source` accepts one `.rs` file or recursively scans a directory.
Directory traversal includes every Rust file found; it is not Cargo reachability analysis.

For DTCG source extraction, configure the package with the Resolver form of `theme!` above and pass
the same file plus the same modifier/context pairs to CLI `--tokens` and `--token-input`. The build
artifact contains only the selected registry for `pc!`/`pcx!`; a controlled CLI build publishes the
complete Resolver graph for audit. The TOML form remains unchanged, and both build forms are covered
by downstream identity and package-consumer tests.

See the [CLI reference](../reference/cli.md) for compile, composition, manifest, and watch options.

## Verify this repository

```console
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm check:api
pnpm check:manifest
pnpm integration:plain-html
```

`integration:plain-html` needs local Chrome or Chromium. It compiles explicit line-oriented styles,
binds their manifest classes to exact HTML template slots, and proves computed CSS with no script or
WASM styling runtime. See the [plain HTML contract](../integrations/plain-html.md).

When the `pliegors` repository is a sibling of `PliegoCSS`, run the Rust 1.85 cross-project gate:

```console
pnpm integration:pliegors
pnpm integration:pliegors-browser
pnpm integration:pliegors-dev
```

The first command renders PliegoRS SSR markup, builds two deterministic SSG routes, publishes five
explicit schema-5/pruned bundle pairs plus `pliego.assets.json` and `pliego.index.json`, and verifies
exact source snapshots, source-site lineage, class identity, route/island selection, build-ledger
hashes, dead-bundle exclusion, and the SSR markup contract of one resumable island. The second
regenerates and serves that site, launches local headless Chrome, executes the island event, and
requires the same SSR document/island/button/bound-text element objects, state 15→20, stable classes,
one matching state event, one shared-CSS preload reused by its stylesheet, and no browser/server
errors. It also builds and executes the fixture's optional Rust/WASM client; install the exact
`wasm-bindgen` version required by the pinned PliegoRS checkout and add `wasm32-unknown-unknown` to
Rust 1.85 before running it. The third starts `pliego-cssc watch` and `pliego dev`,
performs 20 edits with one
stable SSE generation each, and checks invalid/unchanged publication groups. A controlled Chromium
reload replay remains separate from that automated development-server gate. The SSG command covers
the product registry and derived partitions, but does not prove exhaustive Cargo-module discovery or
certify a production asset pipeline.

An application upgrading from the earlier StyleId format-1 candidate must recompile its macros and
SSR/SSG output, regenerate all CSS/manifests/bundles, invalidate class-keyed caches, and deploy the
new markup and assets together. Class-name format 1 did not change, but its encoded StyleId did. See
the [format-2 migration contract](../reference/style-id-format-v2.md#migration-from-the-format-1-candidate).

The checked minimal source is under `examples/basic`. See
[Project structure](./project-structure.md) for the files used by a custom-theme package.
