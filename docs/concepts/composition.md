# Composition and conditional styles

PliegoCSS composition combines validated semantic assignments. It does not concatenate utility
strings at runtime and it does not use CSS source order to decide which value wins.

## Branch over base

`pcx!` accepts a base literal followed by one or more complete Rust conditional clauses:

```rust
use pliego_css::{Style, pcx};

fn button(selected: bool) -> Style {
    pcx!(
        "inline-flex rounded-md bg-surface text-ink",
        if selected {
            "bg-accent text-white"
        } else {
            "bg-transparent text-accent"
        },
    )
}
```

The macro parses and validates the base and every branch during Rust compilation. For each selected
branch, assignments that overlap base slots in the same normalized condition replace those slots.
Non-overlapping base assignments remain. In the example, the selected background and text colors
replace the base colors, while `inline-flex` and `rounded-md` survive.

This replacement is semantic. Reversing source fragments cannot change the result, and a normal
same-condition conflict is not silently converted into an override. Composition is the explicit
operation that grants the branch precedence over its base.

## Conditions and precedence

Composition compares assignments within canonical conditions. Built-in dimensions commute, so
`md:hover:` and `hover:md:` identify the same condition. A branch assignment only removes the base
slots it overlaps in that condition; base assignments under a different breakpoint, state, theme,
motion preference, contrast preference, or selector chain remain separate.

Chained conditions can combine:

- one fixed breakpoint: `sm`, `md`, or `lg`;
- one theme mode: `light` or `dark`;
- one motion preference and one contrast preference;
- distinct interactive states;
- typed ARIA/data attribute conditions and one inherited `ltr`/`rtl` direction;
- the `placeholder` transform or a validated arbitrary `[&...]` selector transform with exactly one
  leading anchor.

Arbitrary selector transforms are ordered because their composition may change meaning. Built-in
dimensions still normalize around them. Consequently, `md:hover:[&>svg]:opacity-100` and
`[&>svg]:hover:md:opacity-100` are equivalent, while two `[&...]` transforms retain authoring order.
Typed selectors use the same canonical selector strings, so `aria-expanded:block` and its exact
`[&[aria-expanded=true]]:block` semantic equivalent receive the same identity. Contradictory
direction or `data-state-*` families fail before identity generation.

The arbitrary-selector subset requires an unquoted `&` anchor and rejects top-level selector lists,
at-rules, declaration or block syntax, comments, line breaks, and unsafe `<` input. It is a bounded
compiler contract, not a general arbitrary CSS parser.

## Independent `if` and `match` clauses

One `pcx!` invocation may combine complete `if ... else ...` and `match` clauses:

```rust
# use pliego_css::{Style, pcx};
enum Kind {
    Primary,
    Ghost,
}

fn badge(kind: Kind, muted: bool) -> Style {
    pcx!(
        "inline-flex rounded-md",
        match kind {
            Kind::Primary => "bg-accent text-white",
            Kind::Ghost => "bg-transparent text-accent",
        },
        if muted { "opacity-50" } else { "opacity-100" },
    )
}
```

Every result arm must be a visible Rust string literal. This keeps the complete style set available
to the compiler and gives every Cartesian result a deterministic `StyleId`. `PCX003` compares every
pair of branches from different clauses by canonical condition and semantic slot. If two independently
selectable branches can write the same slot under the same condition, compilation fails instead of
choosing an implicit order. Model related state in one `match` when assignments must overlap.

Constructed strings, constants, match guards, arm attributes, and `else if` remain unsupported. One
invocation may produce at most 64 Cartesian results (`PCX004`); collapse correlated booleans into one
enum and `match` before that boundary.

## Runtime boundary

The public `Style` handle contains only a `StyleId`. `pcx!` expands to ordinary Rust `if`/`match`
selectors followed by an ID lookup over the compiled Cartesian results. Each clause expression is
evaluated exactly once, in source order. Runtime code chooses an identity; it does not carry semantic
IR, parse utility names, allocate a style registry, or emit CSS.

Reactive CSS values use custom properties instead of dynamic utility names:

```rust
let panel = pliego_css::pc!("w-(--panel-width) gap-(--panel-gap)");
```

The style identity records typed references to `--panel-width` and `--panel-gap`. The application
may update those CSS variables through normal HTML/CSS mechanisms without changing the class name or
invoking a PliegoCSS runtime compiler.

## CSS extraction today

Macro expansion and CSS extraction share the same parser, semantic lowering, composition, identity,
and emission logic. `pliego-css-source` discovers every clause and Cartesian composition through
Rust syntax trees, and `pliego-cssc --source` feeds those results through semantic composition:

```console
cargo run -p pliego-cssc -- compile \
  --source src/main.rs \
  --output pliego.css
```

Manual extraction remains available when a build adapter cannot provide Rust source. Pass the same
base once per result and aggregate the selected, non-conflicting clause branches into the branch
argument:

```console
cargo run -p pliego-cssc -- compile \
  --compose "inline-flex rounded-md bg-surface text-ink" "bg-accent text-white" \
  --compose "inline-flex rounded-md bg-surface text-ink" "bg-transparent text-accent" \
  --output pliego.css
```

`--compose` is not equivalent to joining both strings under `--style`: ordinary lowering would see
overlapping same-condition values as a conflict. It is also not automatic; omitting a possible
branch means its class has no emitted CSS rule.

## Accessible condition patterns

- Use `focus-visible:` for keyboard focus indicators without forcing the same visual treatment on
  every pointer interaction. Do not suppress the native outline unless independently verified
  replacement evidence exists.
- Keep `disabled:` styling paired with the element's real disabled semantics; the variant only
  selects `:disabled`.
- Put motion-dependent effects under `motion-safe:` and provide a usable base or `motion-reduce:`
  result for the same affected selector and motion family.
- Treat `contrast-more:` and `contrast-less:` as refinements. Content and controls must remain
  understandable when neither preference query matches.
- Pair functional hover behavior with keyboard-reachable focus behavior; a `hover:` variant alone
  does not establish input-modality equivalence.
- Treat forced-colors support as a user-agent behavior. Avoid `forced-color-adjust: none` unless the
  exact result has separate browser/manual evidence.
- Use `placeholder:` only for supplementary hints; placeholders do not replace labels.

These authoring patterns and the audit policy have different jobs. Variants describe when generated
CSS applies; they do not certify the rendered DOM. `pliego-cssc audit --accessibility-policy FILE`
can statically check declared contrast pairs, exact motion-preference guards, explicit
focus-outline suppression, forced-color adjustment, and same-rule hover/focus declaration
equivalence in either direct CSS or an Asset Plan. Relations that need cascade or cross-file proof
remain `manual-required`. A token-backed contrast relationship also needs explicit
`--token-graph FILE` evidence:

```console
pliego-cssc audit --input dist/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json \
  --token-graph dist/pliego.tokens.json --format json
```

Audit findings retain `verified`, `unverified`, or `manual-required` plus stable
`context.subject-id`; success means only that the configured bounded static gate passed. Composition
and static audit cannot prove focus order, DOM label relationships, replacement-indicator
visibility, keyboard behavior implemented in JavaScript, forced-colors rendering, or global WCAG
conformance. Use browser, assistive-technology, interaction, and human evidence for those boundaries.

## Current limits

- Reusable named compositions and richer multi-style composition APIs are not implemented.
- `pcx!` is intentionally capped at 64 Cartesian results per invocation.
- Recursive module-graph and Cargo-aware source discovery are not implemented; build adapters must
  pass each Rust source unit through `--source`.
- Gate B's complete fixture and transfer-size validation are closed as a conditional GO; the
  automated multi-browser matrix is still pending.

See the [syntax contract](../reference/syntax.md) for accepted forms and diagnostics, the
[command-line reference](../reference/cli.md) for artifact generation, and the
[conflict model](./conflict-model.md) for slot and condition rules.
