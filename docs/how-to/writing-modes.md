# Use typed writing modes

Status: implemented pre-0.1 contract

PliegoCSS represents CSS `writing-mode` as one conflict-checked semantic slot. Use it when text and
layout must flow horizontally or vertically:

```rust
use pliego_css::{Style, pc};

fn vertical_label() -> Style {
    pc!("writing-vertical-rl")
}
```

## Utilities

| Utility | Native CSS | Meaning |
|---|---|---|
| `writing-horizontal` | `writing-mode: horizontal-tb` | Horizontal inline flow, blocks from top to bottom. |
| `writing-vertical-lr` | `writing-mode: vertical-lr` | Vertical inline flow, blocks from left to right. |
| `writing-vertical-rl` | `writing-mode: vertical-rl` | Vertical inline flow, blocks from right to left. |

Only these three interoperable modes are typed in the current alpha. Sideways and deprecated SVG
values are not silently accepted through this family.

## Writing mode is not direction

`ltr:` and `rtl:` are selector variants. They match the inherited writing direction through native
`:dir()` and conditionally apply a utility. Writing-mode utilities set the element's block-flow and
inline-axis orientation. They can be combined because they represent different semantics:

```rust
use pliego_css::{Style, pc};

fn rtl_vertical_label() -> Style {
    pc!("rtl:writing-vertical-rl")
}
```

This emits a generated class scoped by `:dir(rtl)` whose declaration is
`writing-mode:vertical-rl`. PliegoCSS does not infer language, text direction, or document locale.

## Conflicts and conditions

Two writing modes in the same normalized condition are `PCS005` because source order never chooses
a winner:

```rust,compile_fail
pc!("writing-horizontal writing-vertical-rl");
```

Different conditions are valid:

```rust
use pliego_css::{Style, pc};

fn responsive_label() -> Style {
    pc!("writing-horizontal md:writing-vertical-rl")
}
```

Writing-mode utilities compose with viewport/container queries, themes, interaction, motion,
contrast, direction, and typed attribute variants. Their utility, slot, and keyword values are
persisted in semantic IR binary format 2 with append-only tags, so older format-2 streams retain
their exact bytes and identities.

## Compatibility boundary

The three utilities are classified as `writing-mode-utilities` by compatibility policy schema 2 /
policy 7 and are allowed under `baseline-widely`. An arbitrary declaration such as
`[writing-mode:sideways-rl]` remains `CMP002` under that strict profile.

See the [utility reference](../reference/utilities.md), [typed IR](../concepts/typed-ir.md), and
[compatibility policy](../reference/compatibility-policy.md).
