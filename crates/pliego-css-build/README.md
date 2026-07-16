# pliego-css-build

Supported build-script bridge for custom PliegoCSS themes:

```rust
fn main() {
    pliego_css_build::theme!("pliego.theme.toml");
}
```

For a DTCG Resolver 2025.10 document, select one permutation directly in the same single build
script call:

```rust
fn main() {
    pliego_css_build::theme!(
        tokens = "product.resolver.json",
        inputs = {
            "appearance" => "dark",
            "density" => "compact",
        },
    );
}
```

Use `inputs = {}` to select all Resolver defaults. Modifier and context matching is ASCII
case-insensitive. Unknown inputs, invalid contexts, and exact or case-folded duplicate modifier
names fail the build. The selected registry—not every permutation—is encoded into the canonical
binary artifact.

Call `theme!` once per package. The macro tracks the exact theme or Resolver file, validates it into
`OUT_DIR`, and exports its path and `ThemeId` to `pc!`/`pcx!`. Invoke `pliego-cssc` with the same
theme source and Resolver inputs for matching CSS.

The non-default `artifacts` feature exposes the bounded manifest/trace/Asset Plan, finding 1.0.0,
standard-CSS audit, compatibility, and budget contracts used by `pliego-cssc`. See
`docs/reference/audit-command.md`, `docs/reference/finding-schema-1.md`, and
`docs/reference/budget-policy.md` in a release checkout.

## Stability

Both `theme!` forms, `THEME_PATH_ENV`, and `THEME_ID_ENV` are the candidate bridge. `artifacts`
remains an exact-version tooling contract. Packages develop in lockstep; this pre-release workspace
does not claim that `0.1.0` is published.
