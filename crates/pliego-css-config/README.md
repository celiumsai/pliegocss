# pliego-css-config

`pliego-css-config` parses and validates both the schema-1 TOML theme adapter and the DTCG 2025.10
format/Resolver adapter used by PliegoCSS build and command-line tooling. The TOML form can extend
the seed registry with typed tokens and breakpoints; the DTCG form selects one validated Resolver
permutation while preserving the complete canonical token graph for controlled CLI output.

```toml
schema = 1
extends = "seed"

[tokens.color]
brand = "oklch(56% 0.18 255)"

[tokens.spacing]
gutter = "1.75rem"

[breakpoints]
tablet = "52rem"
```

`parse_str` and `parse_path` return a validated `pliego-css-theme::ThemeRegistry` or a structured
`ConfigError`.

The crate also exposes a DTCG 2025.10 format bridge through `parse_dtcg_str`, `parse_dtcg_path`, and
`export_dtcg`, plus the bounded same-document Resolver profile through
`parse_dtcg_resolver_str/path`. It retains aliases, derived values, deprecations, provenance, and
all validated context permutations in `pliegocss-token-graph/1`, then supplies the exact resolved
`ThemeRegistry` expected by the compiler. Path loaders accept only regular, non-link UTF-8 files up
to 16 MiB and reject symlinks/reparse points. Resolver references remain same-document only; the CLI
and Cargo build macro select explicit Resolver files without discovery. See `docs/reference/dtcg-bridge.md` and
`docs/reference/token-graph-schema-1.md` in a release checkout.

Budget policy schema 1 is exposed through `parse_budget_policy` and `evaluate_budget_policy`. It
validates file/package/route/layer subjects, canonicalizes policy order, evaluates bytes, rules,
selectors, specificity, and semantic-duplication limits, records regression deltas, and preserves
reviewed exceptions without turning them into passing measurements. The CLI/build audit adapter owns
the finding projection; this crate owns the backend-independent policy contract and its optional
`css-analysis` feature owns canonical AST measurement. See
`docs/reference/budget-policy.md` in a release checkout.

## Stability

This is a lockstep implementation and advanced tooling crate. Applications should normally use
`pliego-css-build::theme!`; the CLI also consumes this parser internally. Direct consumers must pin
exact matching PliegoCSS package versions. The document schema is versioned independently from the
Rust API, which is not covered by the application SemVer promise unless explicitly promoted.

The current workspace is pre-release and this README does not claim that `0.1.0` is published. The
theme and DTCG references live under `docs/reference/` in a release checkout.
