# Utility reference

Status: implemented F2 catalog

This page documents the utility strings accepted by the current semantic compiler. It is an
implementation reference, not the larger proposed catalog. A style is a whitespace-separated list
inside `pc!("...")` or one `--style`/input line passed to `pliego-cssc`.

```rust
use pliego_css::{Style, pc};

fn button() -> Style {
    pc!("inline-flex items-center gap-2 rounded-lg bg-accent px-4 py-2 text-sm font-semibold hover:bg-accent-strong")
}
```

`pc!` validates the literal and returns its semantic class identity. It does not write a CSS file.
For the exact compiler-owned surface and example CSS, see the
[generated seed catalog](./generated-catalog.md); the CLI can also render JSON or a custom-theme
catalog through [`pliego-cssc catalog`](./cli.md#catalog).

## Fixed utilities

| Utility | CSS semantics |
|---|---|
| `block`, `inline-block`, `flex`, `inline-flex`, `grid`, `hidden` | `display` |
| `flex-row`, `flex-col` | `flex-direction` |
| `flex-wrap` | `flex-wrap: wrap` |
| `flex-1` | grow `1`, shrink `1`, basis `0%` |
| `items-start`, `items-center`, `items-end` | `align-items` |
| `justify-start`, `justify-center`, `justify-end`, `justify-between` | `justify-content` |
| `border` | solid `1px` border on all sides |
| `border-b` | solid `1px` bottom border |
| `tracking-tight` | `letter-spacing: -.025em` |
| `cursor-pointer`, `cursor-not-allowed` | `cursor` |
| `antialiased` | Platform font-smoothing declarations. |
| `transition-colors` | Color transition properties, duration, and timing function. |
| `aspect-square` | `aspect-ratio: 1 / 1` |
| `outline-none` | `outline-style: none` |
| `outline-2`, `outline-offset-2`, `outline-accent` | Width, offset, and seed accent color for outlines. |
| `resize-y` | `resize: vertical` |
| `writing-horizontal`, `writing-vertical-lr`, `writing-vertical-rl` | Native `writing-mode` values. |

## Parameterized families

`{space}` is one of `0`, `1`, `2`, `3`, `4`, `5`, `6`, `8`, `12`, `28`, or `40`.
It resolves to `n * .25rem`, except `0`, which emits `0`. Arbitrary values and custom properties are
accepted only where the table says so.

| Family | Accepted operand | Notes |
|---|---|---|
| `grid-cols-{value}` | integer `1..12`, `[value]`, `(--var)` | Integers emit `repeat(n,minmax(0,1fr))`. |
| `col-span-{n}` | integer `1..12` | Emits `span n/span n`. |
| `gap-{space}` | spacing token, `[value]`, `(--var)` | Sets row and column gap. |
| `p-{space}` | spacing token, `[value]`, `(--var)` | All padding sides. |
| `px-{space}` | spacing token, `[value]`, `(--var)` | Left and right padding. |
| `py-{space}` | spacing token, `[value]`, `(--var)` | Top and bottom padding. |
| `pb-{space}` | spacing token, `[value]`, `(--var)` | Bottom padding. |
| `m-{value}` | spacing token, `auto`, `[value]`, `(--var)` | All margin sides. |
| `mx-{value}` | spacing token, `auto`, `[value]`, `(--var)` | Left and right margin. |
| `mt-{value}` | spacing token, `auto`, `[value]`, `(--var)` | Top margin. |
| `w-{size}`, `h-{size}` | size token, `auto`, `[value]`, `(--var)` | Width or height. |
| `max-w-{size}`, `min-w-{size}`, `min-h-{size}` | size token, `auto`, `[value]`, `(--var)` | One CSS size constraint. |
| `bg-{color}` | color token, `[value]`, `(--var)` | Named colors accept `/0..100` alpha. |
| `text-{size}` | font-size token | The `/line` modifier accepts a line-height token, `[value]`, or `(--var)`. |
| `text-{color}` | color token or `(color:--var)` | Named colors accept `/0..100` alpha. |
| `font-{value}` | font-family or font-weight token | Bare catalog tokens only. |
| `leading-{value}` | line-height token, `[value]`, `(--var)` | Sets `line-height`. |
| `border-{value}` | non-negative integer pixels or color token | Widths add solid style. |
| `rounded-{value}` | radius token, `[value]`, `(--var)` | All four corners. |
| `shadow-{value}` | shadow token, `[value]`, `(--var)` | Composes through `--pc-shadow`. |
| `ring-{value}` | non-negative integer pixels or color token | Named ring colors accept `/0..100` alpha. |
| `opacity-{n}` | integer percentage `0..100` | Emits a percentage value accepted by CSS opacity. |
| `[property:value]` | one arbitrary declaration | Emits the property and value unchanged after parsing. |

The implemented size tokens are the spacing tokens plus `full`, `screen`, `prose`, and `6xl`.
Their special values are `100%`, viewport width/height, `65ch`, and `72rem`, respectively. Every size
family currently accepts this complete set, even when a combination is not a useful design choice.

## Seed token values

| Namespace | Names and current output |
|---|---|
| Font family | `sans` -> `var(--font-sans)`; `mono` -> `var(--font-mono)` |
| Font size | `xs` `.75rem`; `sm` `.875rem`; `base` `1rem`; `lg` `1.125rem`; `xl` `1.25rem`; `2xl` `1.5rem`; `3xl` `1.875rem` |
| Font weight | `normal` `400`; `medium` `500`; `semibold` `600`; `bold` `700` |
| Line height | `tight` `1.25`; `normal` `1.5`; `6` `1.5rem` |
| Radius | `sm` `.25rem`; `md` `.375rem`; `lg` `.5rem`; `xl` `.75rem`; `2xl` `1rem`; `full` `9999px` |
| Shadow | `sm` `0 1px 2px #0000000d`; `md` `0 4px 6px -1px #0000001a` |

Font-size utilities also emit a current default line height: `xs` -> `1rem`, `sm` -> `1.25rem`,
`base` -> `1.5rem`, `lg`/`xl` -> `1.75rem`, `2xl` -> `2rem`, and `3xl` -> `2.25rem`. A `text-{size}/{line}`
modifier adds an explicit semantic line-height assignment.

The color catalog is `transparent`, `current`, `white`, `canvas`, `surface`, `surface-raised`, `ink`,
`muted`, `accent`, `accent-strong`, and `line`. `transparent` and `current` emit CSS keywords, while
`white` emits `#fff`. The other colors and both font families require the seed variables emitted by
the CLI's `--theme` option, unless the application defines matching variables itself.

Numeric border and ring widths are literal pixels: `border-1`/`ring-1` emit `1px`, `border-2` emits
`2px`, and `border-4` emits `4px`. They deliberately do not resolve through the spacing namespace,
so overriding spacing token `4` cannot change a border width.

## Arbitrary values and custom properties

Arbitrary values preserve spaces and do not translate underscores:

```rust
fn main() {
    let layout = pliego_css::pc!("grid-cols-[1fr 300px] gap-[clamp(1rem,2vw,2rem)]");
    let painted = pliego_css::pc!("bg-[oklch(62% 0.2 25)] [mask-type:luminance]");
    println!("{} {}", layout.class_name(), painted.class_name());
}
```

Braces are rejected at every nesting level in arbitrary values and properties. Top-level semicolons,
bang markers, and at-rules are also rejected; `!` remains available only as the PliegoCSS suffix.
Distinct property names coexist in the same condition, and their authoring order does not change
`StyleId` or emitted CSS. Repeating the same property with the same value deduplicates; assigning two
values to the same property in one condition is a conflict.

Branch composition also resolves arbitrary properties by name. A branch replaces the base value of
the same property while retaining other properties. Independent `pcx!` clauses establish a
cross-clause conflict only when they can assign the same property name under the same structural
condition.

Custom properties use parentheses:

```rust
fn main() {
    let layout = pliego_css::pc!("gap-(--card-gap) w-(--card-width) grid-cols-(--card-layout)");
    let color = pliego_css::pc!("bg-(--card-bg) text-(color:--card-text)");
    println!("{} {}", layout.class_name(), color.class_name());
}
```

Accepted explicit hints are `color`, `length`, `number`, and `percentage`, but a hint must also match
the selected utility's semantic domain. `text-(--value)` is rejected as ambiguous; the current
catalog supports only the explicit `color` form for `text-*`. `border-*` and `ring-*` diagnose custom
properties as ambiguous but do not yet implement either hinted form.

Alpha modifiers are supported only for named color tokens. Do not attach an alpha modifier to an
arbitrary or custom color; arbitrary colors are rejected with a modifier, while a custom-color
modifier is currently accepted but not applied.

## Variants

| Dimension | Variants | Emission |
|---|---|---|
| State | `hover`, `focus`, `focus-visible`, `active`, `disabled` | Pseudo-class suffix |
| Pseudo-element | `placeholder` | `::placeholder` selector |
| Theme | `light`, `dark` | `[data-theme=light]` or `[data-theme=dark]` ancestor |
| Breakpoint | `sm`, `md`, `lg` | `min-width: 40rem`, `48rem`, `64rem` |
| Motion | `motion-safe`, `motion-reduce` | `prefers-reduced-motion` media query |
| Contrast | `contrast-more`, `contrast-less` | `prefers-contrast` media query |
| ARIA boolean | `aria-{busy,checked,disabled,expanded,hidden,invalid,pressed,readonly,required,selected}` | Exact `[aria-*=true]` selector |
| Data attribute | `data-{active,checked,disabled,loading,selected}`; enumerated `data-state-*` | Presence or exact `[data-state=value]` selector |
| Configurable ARIA | `aria-[name=value]` | Bounded exact `[aria-name=value]` selector |
| Configurable data | `data-[name]`, `data-[name=value]` | Bounded presence or exact `[data-name...]` selector |
| Direction | `ltr`, `rtl` | Native `:dir(ltr)` or `:dir(rtl)` selector |
| Cascade layer | `layer-base`, `layer-components`, `layer-utilities`, `layer-overrides` | Namespaced native `@layer pliego.*` block |
| Selector transform | `[&>svg]`, `[&[data-state=open]]` | Replaces `&` with the generated class selector. |

Dimensions commute and normalize, so `md:hover:flex` and `hover:md:flex` have the same condition.
Multiple distinct states form a compound selector. Repeating a state, combining two breakpoints, or
combining contradictory theme, motion, or contrast values is an error. Arbitrary selectors combine
with those commuting dimensions, while multiple selector transforms preserve source order:

```rust
fn main() {
    let icon = pliego_css::pc!("hover:[&>svg]:opacity-100");
    let open = pliego_css::pc!("[&[data-state=open]]:bg-accent");
    println!("{} {}", icon.class_name(), open.class_name());
}
```

Every arbitrary selector must start with exactly one unquoted `&`. This keeps every transform scoped
to the generated class and bounds selector expansion when transforms are chained. The compiler
rejects extra or non-leading anchors, crossed delimiters, top-level selector lists, at-rules,
blocks/declarations, comments, line breaks, and `<`. Functional selector lists such as
`[&:is(.primary,.secondary)]:` are accepted because the comma is nested and the generated class
remains the selector subject.

Typed ARIA, data, and direction variants are classified semantic selector transforms. Their named
form is allowed by the strict Baseline profile, exact duplicates fail, and mutually exclusive
direction or tests of the same ARIA/data attribute cannot be combined. Configurable names and values
are lowercase ASCII fragments of at most 64 bytes; see the
[typed attribute guide](../how-to/attribute-variants.md). An arbitrary selector outside the
classified set remains `CMP003` under `baseline-widely`.

Writing mode is a utility rather than a selector dimension. `writing-horizontal`,
`writing-vertical-lr`, and `writing-vertical-rl` assign one conflict-checked `writing-mode` slot.
They can be conditional through `rtl:`, breakpoints, container queries, or other variants, but two
values under the same normalized condition are `PCS005`. See the
[writing-mode guide](../how-to/writing-modes.md).

Cascade layers are a condition dimension with a closed low-to-high order: `pliego.base`,
`pliego.components`, `pliego.utilities`, and `pliego.overrides`. Same-slot assignments in different
layers are valid; the native layer order resolves normal declarations. Same-layer conflicts retain
the normal `PCS005` rules, and two layer variants in one condition are `PCS012`. Unprefixed
utilities remain unlayered. See the [cascade layer guide](../how-to/cascade-layers.md).

## Conflicts, refinements, and duplicates

PliegoCSS compares semantic property slots within the same normalized condition:

```rust
fn main() {
    let same = pliego_css::pc!("flex flex");            // exact duplicate: deduplicated
    let refined = pliego_css::pc!("p-4 px-2");         // valid: narrower refinement
    let conditional = pliego_css::pc!("flex md:grid"); // valid: different conditions
    println!("{} {} {}", same.class_name(), refined.class_name(), conditional.class_name());
    // let conflict = pliego_css::pc!("flex grid"); // PCS005: both assign display
    // let conflict = pliego_css::pc!("p-4 p-6");   // PCS005: same footprint
}
```

Source order does not choose a winner. Proper subset footprints are refinements and normalize
independently of order: `p-4 px-2` and `px-2 p-4` share one `StyleId`. Overlapping footprints that
are not equal or proper subsets must not assign the same slot.

## Negative and important forms

The leading minus appears after variants. The current public catalog allows it only for margin
families and `tracking-tight`:

```rust
fn main() {
    let shifted = pliego_css::pc!("-mt-4 md:-mx-[1.5rem] -tracking-tight");
    println!("{}", shifted.class_name());
}
```

`auto` cannot be negated. Use `-mt-[1rem]`, not `mt-[-1rem]`. Padding, gap, size, color, border,
ring, opacity, arbitrary properties, and the other fixed utilities reject the negative form.

Important is a suffix on one item:

```rust
fn main() {
    let interoperable = pliego_css::pc!("bg-accent hover:bg-accent-strong!");
    println!("{}", interoperable.class_name());
}
```

It emits `!important` on that assignment, but it does not make semantic conflicts source-ordered.
A non-important narrow refinement cannot override an important shorthand; either mark the refinement
important too or remove important from the shorthand. Prefix `!utility` and embedded `!important`
are invalid.

Native CSS reverses cascade-layer priority for important declarations. Therefore an important
declaration in `pliego.base` outranks an important declaration in `pliego.overrides`; PliegoCSS does
not rewrite this standards behavior.

## Class identity and CSS output

Each normalized style receives one 128-bit `StyleId`. `Style::class_name()` and the emitter encode
all of those bits as lowercase base 36 with a `pc_` prefix, for example `pc_da6tdj270pn0xz4jflsvgaai9`.
Equivalent normalized styles have the same class; source text and source spans are excluded from the
identity.

StyleId format 2 hashes an explicit tagged, length-framed, big-endian byte stream with SHA-256 and
uses the first 128 digest bits in big-endian order. UTF-8 content, not intern-table positions or Rust
`Debug` output, enters the stream. The class encoder remains format 1, but candidate format-1 class
strings changed because their StyleId input changed. See the exact
[StyleId format-2 contract](./style-id-format-v2.md). These vectors are machine-enforced at workspace
version `0.0.0`; the public SemVer freeze remains open. `Style::EMPTY.class_name()` returns an empty
string and does not represent an emitted rule.

Run the checked Rust example and compile its CSS from the workspace root:

```console
cargo run -p pliego-css-basic-example
cargo run -p pliego-cssc -- compile --style "flex flex-col gap-4 rounded-lg border border-line bg-surface p-6 md:flex-row md:items-center hover:bg-surface-raised" --theme --output pliego.css
```

## Current limits

- The implemented catalog is intentionally narrower than CSS or Tailwind. Typed custom tokens and
  breakpoints extend the seed but do not add new utility families.
- `pc!` accepts one visible Rust string literal. `pcx!` accepts a visible literal base and complete
  `if ... else ...` or `match ...` clauses, with at most 64 reachable Cartesian results. Constructed
  utility strings and constants are not extraction inputs.
- Procedural macros validate and return ID-only `Style` values; they do not write a stylesheet. The
  CLI can scan explicitly supplied Rust files or directories, but it does not resolve Cargo modules,
  dependencies, generated code, components, routes, or islands semantically.
- Lightning CSS validates and prints the combined CLI artifact in deterministic `minified` or
  `pretty` form, but the macro alone does not validate arbitrary CSS values against a full CSS grammar.
- There is no reset/preflight layer, automatic route splitting, critical CSS, or automatic
  reachability collection. Default schema 3 carries identity-format versions, style origins, and CSS
  integrity. Opt-in schemas 4/5 import exact component/route/island ownership; adding
  `--prune-unreachable` removes complete StyleId rule sets that have no reachable exact origin while
  retaining all origins of a shared emitted style. Direct token graph nodes then describe only
  emitted styles. With the same explicit pruning flag, `--theme` emits only variable-backed tokens
  referenced by retained styles; without pruning it emits the complete supported block. Authored
  `var(...)` consumers outside PliegoCSS semantic styles are not discovered.
- Watch mode uses native Windows/Linux filesystem events with authoritative exact-byte fallback snapshots of
  line-oriented inputs, Rust source trees, and theme configuration, then caches unchanged Rust
  syntax reports, theme-scoped semantic IR, raw CSS fragments, and byte-identical final output.
  Browser notification from PliegoCSS remains outside the compiler. The CLI already exposes catalog JSON for completion data,
  `explain --format json` for hover data, and read-only `fmt --source --check` diagnostics; PliegoRS
  development uses its existing SSE reload channel.
- The older [initial catalog](./initial-utility-catalog.md) records the F0 plan; this page supersedes
  it for implemented behavior.
