# First compile

Status: implemented core; release hardening remains open

Add the local crate while the project is unreleased:

```toml
[dependencies]
pliego-css = { path = "<path-to-PliegoCSS>/crates/pliego-css" }
```

Then validate a utility list during Rust compilation:

<!-- docs-smoke:basic-rust:start -->
```rust
use pliego_css::{Style, pc};

fn card() -> Style {
    pc!(
        "flex flex-col gap-4 rounded-lg border border-line bg-surface p-6 \
         md:flex-row md:items-center hover:bg-surface-raised"
    )
}

fn main() {
    let class = card().class_name();
    assert!(class.starts_with("pc_"));
    println!("{class}");
}
```
<!-- docs-smoke:basic-rust:end -->

`pc!` validates grammar, catalog names, token domains, variants, negative forms, and semantic
conflicts while Rust compiles. It returns the same class identity for equivalent normalized styles.

Install the CLI from the same exact checkout as described in [Installation](./installation.md), then
run the corresponding extraction from the application directory:

```console
pliego-cssc compile \
  --style "flex flex-col gap-4 rounded-lg border border-line bg-surface p-6 md:flex-row md:items-center hover:bg-surface-raised" \
  --seed --theme --output pliego.css
```

For multiple styles, repeat `--style` or use `--input styles.txt`, with one utility string per line.
The CLI deduplicates equivalent canonical identity streams, fails on inconsistent or colliding
stream/ID mappings, orders output deterministically by stream bytes, and validates and minifies the
final CSS through Lightning CSS.

For a project source tree, extraction can use the same visible macro literals:

```console
pliego-cssc compile --source src --seed --theme --output pliego.css --manifest pliego.manifest.json
```

To publish a verifiable build group with a canonical source map, place CSS and manifest below one
existing root and enable control mode:

```console
mkdir -p dist
pliego-cssc compile --source src --seed --theme \
  --output dist/app.css --manifest dist/app.manifest.json \
  --control-dir dist
```

This additionally emits `dist/app.css.map`, `dist/pliego.tokens.json`, canonical findings, the
control manifest, and its receipt. The token graph is the canonical
`pliegocss-token-graph/1` projection of the active registry. The map and graph are integrity-bound
through the control manifest, and PliegoCSS does not inject a `sourceMappingURL` comment into CSS.
Repeat with `--check` to compare all seven artifacts without writing. See the
[source-map contract](../reference/css-source-maps.md) and
[token-graph contract](../reference/token-graph-schema-1.md).

The direct CLI DTCG Resolver surface selects one explicit document and context. Copy the tracked
`examples/product.resolver.json` fixture from the PliegoCSS checkout into this application as
`product.resolver.json`, or substitute the path to your own Resolver document. Add
`pliego-css-build` under `[build-dependencies]` as shown in [Installation](./installation.md), then
create:

```rust,no_run
// build.rs
fn main() {
    pliego_css_build::theme!(
        tokens = "product.resolver.json",
        inputs = {
            "appearance" => "dark",
        },
    );
}
```

```console
pliego-cssc compile --source src --tokens product.resolver.json \
  --token-input appearance=dark --theme \
  --output dist/app.css --manifest dist/app.manifest.json \
  --control-dir dist
```

`--tokens` conflicts with `--config` and `--seed`; JSON is never auto-discovered, and every
`--token-input modifier=context` requires `--tokens`. Inputs are repeatable for distinct modifiers.
Omitted modifiers use defaults; names and values converge after case folding. Duplicate modifiers,
unknown modifiers, and unknown contexts fail. The selected registry drives CSS, while the complete
all-permutation graph is published and the canonical selection map enters `configHash`. The
`build.rs` call above encodes only the selected registry for `pc!`/`pcx!`; repeat the same Resolver
and inputs in the CLI command so generated class identities and CSS agree. `inputs = {}` uses every
Resolver default. Macro input names and contexts are string literals, duplicates including
case-fold collisions fail closed, relative paths resolve from `CARGO_MANIFEST_DIR`, and each package
must call `theme!` only once. Bundle schema 2 remains the plan-owned equivalent for one-shot assets;
downstream tests freeze macro/CLI identity for the same Resolver selection.

The CLI defaults to the `modern` target contract (Chrome 111, Edge 111, Firefox 128, Safari 16.4)
and writes a schema-3 manifest when requested. The manifest reports StyleId format 2, class-name
format 1, and ThemeId format 1 as required top-level fields. It auto-discovers one
`pliego.theme.toml` from the working directory or supplied source package; select an exact file with
`--config`, prevent discovery with `--seed`, or explicitly select a Resolver document with
`--tokens`. Only TOML participates in conventional discovery. Source-directory scanning is
syntactic and includes every discovered Rust file, not only Cargo-reachable modules.

Schema 3 is the default. Once a framework adapter can identify exact component sites and
route/island ownership, opt into the complete semantic provenance graph with
`--manifest-version 4 --reachability pliego.reachability.json`. Without a pruning flag, this does
not change CSS bytes. See
[manifest schema 4](../reference/manifest-schema-4.md) and
[reachability schema 1](../reference/reachability-schema.md).

Use `--manifest-version 5` with the same sidecar when a debugger, editor, or asset tool also needs
exact final rule/declaration nodes and UTF-8 byte ranges. Schema 5 fails instead of emitting a partial
trace; with the same pruning setting it emits the same CSS as schema 4. Add
`--prune-unreachable` to either schema only when the adapter attests a complete application graph;
the compiler then removes whole StyleIds with no route- or island-reachable exact origin. Theme
variables remain global and unpruned. See [manifest schema 5](../reference/manifest-schema-5.md).

For a multi-bundle application, add `--asset-plan --project-index` to a schema-5 `bundle` command.
The fixed `pliego.index.json` lets adapters and editor tooling consume the same portable
source-to-output model instead of rescanning the repository. See
[Project Index schemas 1 and 2](../reference/project-index-schema.md).

`pcx!` additionally precompiles all statically enumerable combinations of its conditional clauses
and returns the selected `StyleId` at runtime. It does not generate CSS dynamically in the browser.

If the application has artifacts from the earlier format-1 candidate, rebuild Rust/SSR/SSG output
and regenerate CSS and manifests together. The class encoder is still format 1, but its StyleId input
changed, so old `pc_*` values cannot be mixed with new CSS. See the
[StyleId format-2 migration](../reference/style-id-format-v2.md#migration-from-the-format-1-candidate).

The complete checked Rust source lives in `examples/basic`. Run `pnpm check:getting-started` from
the PliegoCSS checkout to compile it with Rust 1.85, execute the documented CLI extraction both
from the literal style and from the Rust source, and prove that the macro class, manifest class,
and generated CSS converge exactly.
