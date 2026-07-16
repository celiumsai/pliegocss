# Theme troubleshooting

Theme failures are intended to stop the Cargo build before mismatched classes or unsafe CSS reach an
application. Start with the complete error after `PliegoCSS theme configuration failed:` or
`PliegoCSS theme error:`.

## Cargo cannot find the theme file

Typical error:

```text
failed to read `.../pliego.theme.toml`: The system cannot find the file specified
```

`theme!("pliego.theme.toml")` resolves relative paths against `CARGO_MANIFEST_DIR`, the directory
containing the package's `Cargo.toml`. Confirm the filename and keep `build.rs` in that package root.
An absolute path is accepted but makes the project machine-specific.

The DTCG form resolves its Resolver path against the same directory:

```rust,no_run
pliego_css_build::theme!(tokens = "product.resolver.json", inputs = {});
```

DTCG path loaders require a regular, non-link UTF-8 file of at most 16 MiB. Directories, symbolic
links, and Windows reparse points fail closed. Materialize the Resolver in the package instead of
linking it from another tree.

## Unsupported schema or base theme

Schema 1 requires:

```toml
schema = 1
extends = "seed"
```

Other schema numbers and other `extends` values are rejected. Both fields are required, and unknown
fields are errors rather than ignored configuration.

## Invalid or duplicate name

Names accept ASCII letters, digits, hyphens, underscores, and whitespace. They normalize to
lowercase kebab case. These declarations collide:

```toml
[tokens.color]
brand_blue = "#36f"
brand-blue = "#25e"
```

Choose one canonical name. Characters such as `/`, `.`, quotes inside a key, or non-ASCII letters
are rejected by the schema's name validation.

## Unsafe token value

Token values cannot be empty and cannot contain controls, braces, semicolons, CSS comment
delimiters, or `!important`. Define only one CSS value, not a declaration or rule:

```toml
# valid
[tokens.shadow]
card = "0 12px 32px #0002"

# invalid: attempts to add another declaration
broken = "0 1px black; color: red"
```

Passing the declaration-safety check is only the first layer. The registry then applies a bounded
validator for the selected namespace, so cross-domain definitions such as a color containing `1rem`,
spacing containing `red`, or an unknown font-weight spelling fail before macro expansion. Length,
color, weight, line-height, letter-spacing, shadow, family, and z-index namespaces do not share one
untyped acceptance path.

This validator is conservative rather than a complete CSS grammar. Complex supported function forms
must still be valid CSS, and the CLI validates the complete emitted artifact with Lightning CSS.

## Invalid breakpoint

A breakpoint must be a finite positive length using `px`, `rem`, `em`, `ch`, or `vw`:

```toml
[breakpoints]
tablet = "52rem"
```

Values such as `0rem`, `768`, `50%`, `calc(...)`, uppercase units, or an entire media expression are
not accepted by schema 1. Names reserved by another condition dimension —for example `hover`,
`focus`, `dark`, `motion-safe`, or `placeholder`— are also rejected so a breakpoint cannot shadow
the built-in variant.

## Token ID or breakpoint collision

The registry checks duplicate `(namespace, name)` pairs, token ID collisions, duplicate breakpoint
names, and duplicate breakpoint IDs. These are fatal because silently choosing a definition would
make output nondeterministic.

Rename the new definition. Do not edit persisted IDs or the generated binary artifact.

## Theme artifact cannot be read or decoded

`pc!` reports the artifact path from `PLIEGO_CSS_THEME_PATH`. The decoder rejects oversized,
truncated, wrong-version, non-UTF-8, trailing-data, invalid-registry, and identity-mismatched files.

The artifact belongs to Cargo's `OUT_DIR` and is written atomically by `pliego-css-build`. Do not
modify it or set `PLIEGO_CSS_THEME_PATH` manually. If an interrupted or external process has damaged
the target directory, remove the affected local build output with Cargo's normal clean workflow and
rebuild; do not delete source configuration.

## `PLIEGO_CSS_THEME_ID` is malformed or does not match

The value must contain exactly 32 hexadecimal characters. `pliego-css-build` exports it together
with the artifact path. A mismatch means the environment and binary do not describe the same
registry.

Remove manual environment overrides and let the package's own `build.rs` provide both values. Cargo
tracks the exact TOML or Resolver source through `rerun-if-changed`, so editing it should regenerate
a new content-addressed artifact.

## Style identity does not match the active theme

The registry-aware emitter recomputes `StyleId`. This error occurs when semantic IR lowered under
Theme A is passed to emission under Theme B, or when the style carries a stale/foreign identity.

Load one `ThemeRegistry` and pass the same instance to `lower_style_with_theme`, composition, and
`emit_css_with_theme`. Do not mix the seed wrappers with custom-theme IR.

## A custom utility compiles but its CSS is missing

`build.rs` makes custom tokens and breakpoints visible to `pc!`; it does not itself write the
application stylesheet. Run the CLI with the same configuration and explicit source files:

```console
cargo run -p pliego-cssc -- compile --source src --config pliego.theme.toml --theme --output app.css
```

`--config` selects the custom registry and `--theme` emits its variables. A source directory scans
every discovered `.rs` file recursively; pass individual files if that scope is too broad. Styles
can also be added through `--style`, `--compose`, or the line-oriented `--input` described in the
[CLI reference](../reference/cli.md).

## The seed theme is selected unexpectedly

Procedural macros deliberately fall back to `ThemeRegistry::seed()` when
`PLIEGO_CSS_THEME_PATH` is absent. For a custom theme, verify all three pieces are present in the same
package:

1. `pliego-css-build` under `[build-dependencies]`;
2. a root `build.rs` calling `pliego_css_build::theme!(...)`;
3. a valid TOML theme or DTCG Resolver at that path.

If no custom theme is intended, the seed fallback is the correct behavior and no build script is
needed.

The CLI has a separate selection contract. `--config`, `--seed`, and `--tokens` are mutually
exclusive; none silently wins. With no explicit selector it discovers `pliego.theme.toml` from the
current directory and the nearest containing Cargo package of each `--input` or `--source`; JSON is
never discovered. Macro expansion inherits neither TOML discovery nor CLI DTCG selection, so a CLI
command can use a registry while a package without the matching `build.rs` still compiles macros
against the seed. For DTCG, call
`theme!(tokens = "product.resolver.json", inputs = { "appearance" => "dark" })` once in that
package and repeat the same Resolver/inputs as CLI `--tokens`/`--token-input`.

## A DTCG build disagrees with generated CSS

The Cargo artifact contains only the registry selected by `build.rs`; controlled CLI output uses
that registry for CSS but publishes the Resolver's complete graph. A different file, omitted input,
or different context can therefore change `ThemeId` and every class identity. Keep one `theme!` call
per package and mirror its exact Resolver plus selections in the CSS command. Use `inputs = {}` only
when both sides should use every declared default.

Modifier and context entries in the macro must be string literals. Unknown modifiers/contexts,
missing required inputs, exact duplicates, and duplicates that differ only by ASCII case fail the
build instead of selecting a winner.

## Multiple theme configurations match

When supplied inputs belong to different packages, or the current directory contributes a second
theme, discovery may find more than one canonical `pliego.theme.toml`. The CLI refuses to choose one:

```text
multiple `pliego.theme.toml` files match the supplied sources
```

Select the intended registry with `--config path/to/pliego.theme.toml`, split the build by package,
or use `--seed` only when the built-in registry is intentional.

## Output aliases a theme or input

The CLI rejects `--output` and `--manifest` paths that normalize or canonicalize to each other or to
an input, Rust source, explicit configuration, or resolved auto-discovered theme. Choose separate
artifact paths. This validation is deliberate: overwriting a theme or source would change the next
build's identity and could destroy project input.

See the [theme schema](../reference/theme-schema.md) for the exact contract and
[Project structure](../getting-started/project-structure.md) for placement.
