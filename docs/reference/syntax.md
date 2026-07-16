# Syntax contract

Status: implemented pre-0.1 contract

The parser-owned canonical printer is exposed through `pliego-cssc fmt --style`. It changes only
top-level whitespace; semantic validity still belongs to `pliego-cssc check` and the active theme.
The `--input` layer additionally owns the documented line endings, blank-line, and `#` comment
contract for line-oriented files.

The primary API parses a Rust string literal at compile time:

```rust,ignore
pc!("flex items-center gap-4")
```

## Grammar

```ebnf
pc_call             = "pc!", "(", rust_string, [","], ")" ;
style_list          = wsp*, item, { wsp+, item }, wsp* ;
item                = { variant, ":" }, ["-"], candidate, ["!"] ;
variant             = named_variant | configurable_attribute_variant | arbitrary_variant ;
named_variant       = ident ;
configurable_attribute_variant
                    = "aria-[", attr_fragment, "=", attr_fragment, "]"
                    | "data-[", attr_fragment, ["=", attr_fragment], "]" ;
arbitrary_variant   = "[", selector_format, "]" ;
candidate           = named_candidate | arbitrary_property ;
named_candidate     = utility_key, ["-", operand], ["/", modifier] ;
operand             = bare_operand | arbitrary_value | custom_var ;
arbitrary_value     = "[", css_value, "]" ;
custom_var          = "(", [type_hint, ":"], "--", custom_ident, ")" ;
arbitrary_property  = "[", css_property, ":", css_value, "]" ;
ident               = lower, { lower | digit | "-" } ;
attr_fragment       = lower
                    | lower, { lower | digit | "-" }, (lower | digit) ;
```

Utility keys and operands are resolved by longest match against the typed catalog. The parser does
not split every hyphen blindly.

## Whitespace and arbitrary values

Whitespace separates utilities only at bracket, parenthesis, and string depth zero. Real spaces are
valid inside arbitrary values:

```rust,ignore
pc!("grid-cols-[1fr 300px]")
pc!("bg-[oklch(62% 0.2 25)]")
pc!(r#"before:content-[\"Hello world\"]"#)
```

PliegoCSS does not translate underscores to spaces. This deliberately avoids ambiguity in URLs,
identifiers, and content. Raw Rust strings are recommended when embedded quotes are needed.

An arbitrary property contains exactly one declaration. Braces are rejected at every nesting level;
top-level semicolons, bang markers, and at-rules are also rejected:

```rust,ignore
pc!("[mask-type:luminance]")
```

## Variables

Custom properties use parentheses. A type hint resolves ambiguous families:

```rust,ignore
pc!("fill-(--brand)")
pc!("text-(color:--label)")
```

## Negative utilities

The minus sign appears after variants and is available only to negatable families:

```rust,ignore
pc!("md:-mt-4")
```

`-p-4` is invalid because padding is not negatable. `mt-[-1rem]` is rejected with a fix to
`-mt-[1rem]`; a minus inside a larger expression such as `calc()` remains part of that expression.

## Modifiers

Slash modifiers are family-specific. Named colors accept percentage alpha, and font-size entries
accept a line-height modifier. A slash on any other catalog family is an error; it is never ignored
silently. For example, `bg-accent/50` and `text-sm/6` are valid, while `p-4/50`, `opacity-50/x`,
and `font-semibold/x` fail semantic validation.

## Important

Important uses a suffix:

```rust,ignore
pc!("hover:bg-accent/50!")
```

It exists only as an interoperability escape for external CSS. It never resolves conflicts inside
one PliegoCSS style, is recorded in the manifest, and may produce a lint. Prefix `!bg-accent` and
embedded `!important` are invalid.

## Variants

The MVP built-ins are:

- `hover`, `focus`, `focus-visible`, `active`, `disabled`;
- `light`, `dark`;
- `motion-safe`, `motion-reduce`;
- `contrast-more`, `contrast-less`;
- boolean ARIA states such as `aria-expanded`, `aria-disabled`, and `aria-selected`;
- data presence states such as `data-loading`, plus enumerated `data-state-open`/`closed` variants;
- configurable exact ARIA values through `aria-[name=value]`;
- configurable data presence/value tests through `data-[name]` and `data-[name=value]`;
- inherited writing direction through `ltr` and `rtl`;
- cascade precedence through `layer-base`, `layer-components`, `layer-utilities`, and
  `layer-overrides`;
- theme-defined viewport breakpoints and `cq-<breakpoint>` container conditions.

Known commuting variants become a structured condition and normalize by dimension. Thus `md:hover:`
and `hover:md:` have the same condition and conflict if they assign different values to one slot.

Selector transformations use `[&...]:`; exactly one leading `&` anchors the generated class. They
retain source order because their composition can change meaning:

```rust,ignore
pc!("[&>svg]:block")
pc!("[&[data-state=open]]:bg-accent")
pc!("[&>svg]:[& path]:block")
```

Built-in condition dimensions still commute around an arbitrary selector, so
`md:hover:[&>svg]:block` and `[&>svg]:hover:md:block` normalize to the same style. Two arbitrary
selector transforms do not commute and are composed in authoring order.

The supported selector subset must start with exactly one unquoted `&`. It rejects extra or
non-leading anchors, crossed delimiters, top-level selector lists, at-rules, declaration/block
syntax, comments, line breaks, and unsafe `<` input. Commas remain valid inside functional selectors
such as `&:is(.primary,.secondary)` and inside quoted attribute values.

Duplicate, contradictory, and redundant variants are rejected. Examples include `hover:hover:`,
`dark:light:`, `motion-safe:motion-reduce:`, `rtl:ltr:`, two `data-state-*` values, two
`layer-*` values, and two breakpoints in one condition.

Typed selector variants emit native selectors and are accepted by strict `baseline-widely`:

```rust,ignore
pc!("rtl:aria-expanded:data-state-open:hover:bg-accent")
pc!("aria-[sort=ascending]:data-[density=compact]:data-[loading]:block")
```

ARIA variants test the exact boolean value `true`. `data-active`, `data-checked`, `data-disabled`,
`data-loading`, and `data-selected` test attribute presence. The `data-state-*` variants test one of
`active`, `checked`, `closed`, `inactive`, `open`, or `unchecked`. `ltr` and `rtl` use `:dir()` so
inherited direction is respected without assuming a particular framework root.

The four `layer-*` variants select one compiler-owned native cascade layer. Their fixed normal
precedence is `pliego.base`, `pliego.components`, `pliego.utilities`, then `pliego.overrides`.
Layer is a semantic condition dimension: assignments in different layers may target the same slot,
while conflicts inside one layer still fail. Unprefixed items remain unlayered.

Configurable attribute names and values are bounded to 1–64 ASCII bytes. They start with a
lowercase letter, contain only lowercase letters, digits, and single hyphens, end in a letter or
digit, and cannot contain `--`. ARIA requires an exact value; data attributes allow presence or an
exact value. Uppercase, whitespace, quoted/operator syntax, numeric-leading fragments, and multiple
`=` signs fail with `PCS012`. Testing the same canonical attribute twice is also `PCS012`, even when
one spelling is a compiler-owned short form. See the
[typed attribute guide](../how-to/attribute-variants.md).

## Conditional styles

`pcx!` uses Rust-like `if` and `match` syntax. All style branches remain literals visible to the
compiler:

```rust,ignore
pcx!(
    "inline-flex rounded-md bg-surface",
    if selected { "bg-accent text-on-accent" }
    else { "text-primary" },
)

pcx!(
    "rounded-md",
    match variant {
        Variant::Primary => "bg-accent text-on-accent",
        Variant::Ghost => "bg-transparent text-accent",
    },
    if muted { "opacity-50" } else { "opacity-100" },
)
```

Base and one selected branch from every clause compose. Arms of the same conditional are mutually
exclusive and may override one base slot. Independent clauses that may run together and assign
different values to one slot are rejected as ambiguous. The fix is to express the state space in one
`match`.

Constructed strings and unknown constants are rejected in F0/F1. Conditions are evaluated once per
reactive recomputation.

Each invocation accepts one or more complete `if ... else ...` or `match` clauses. Match guards, arm
attributes, constructed strings, constants, and `else if` are rejected. `PCX003` compares all branch
pairs from different clauses by canonical condition and semantic slot; any overlap is rejected, even
if the current values happen to be equal, because independently authored clauses must not establish
precedence implicitly. A selected branch may still override overlapping base slots semantically.

The macro precompiles every Cartesian result and caps one invocation at 64 results (`PCX004`). At
runtime, clause expressions are evaluated exactly once in source order and select only a `StyleId`.

## Diagnostic contract

| Code | Meaning |
|---|---|
| `PCS001` | Unknown utility, with nearest suggestion |
| `PCS002` | Malformed item or empty variant |
| `PCS003` | Unknown variant or token |
| `PCS004` | Value outside the utility domain |
| `PCS005` | Semantic conflict, with both source spans |
| `PCS006` | Negative form unsupported or non-canonical |
| `PCS007` | Unbalanced brackets, parentheses, quotes, or escapes |
| `PCS008` | Invalid or incompatible arbitrary CSS value |
| `PCS009` | Important marker misplaced or duplicated |
| `PCS010` | Ambiguous namespace; type hint required |
| `PCS011` | Arbitrary property contains multiple declarations or a block |
| `PCS012` | Redundant or impossible variant chain |
| `PCX001` | Conditional branch is not a visible literal |
| `PCX002` | `if` condition is not boolean |
| `PCX003` | Independent clauses may conflict at runtime |
| `PCX004` | Conditional Cartesian expansion exceeds 64 results |

## Normalization

Normalized semantic IR, not source text, determines the `StyleId`. These pairs therefore produce the
same style:

```rust,ignore
pc!("flex gap-4")
pc!("gap-4 flex")

pc!("p-4 px-2")
pc!("px-2 p-4")
```

An empty literal is rejected; use `Style::EMPTY`. Comments inside the string are not supported.
