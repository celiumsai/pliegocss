# Use typed ARIA and data variants

Status: implemented pre-0.1 contract

PliegoCSS can express application-specific ARIA and data attributes without opening the arbitrary
selector escape hatch. The compiler parses the attribute into semantic IR, emits one native CSS
attribute selector, classifies it for the strict compatibility profile, and rejects contradictory
tests of the same attribute.

## Forms

Use `aria-[name=value]:` when an ARIA state has an exact value:

```rust
use pliego_css::{Style, pc};

fn sortable_column() -> Style {
    pc!("aria-[sort=ascending]:font-semibold")
}
```

Use either presence or exact-value syntax for a data attribute:

```rust
use pliego_css::{Style, pc};

fn card() -> Style {
    pc!("data-[loading]:opacity-50 data-[density=compact]:p-2")
}
```

The generated selectors are native CSS:

```css
.pc_example[aria-sort=ascending] { font-weight: 600; }
.pc_example[data-loading] { opacity: 50%; }
.pc_example[data-density=compact] { padding: .5rem; }
```

The real generated class is derived from the complete semantic style; `pc_example` is used only to
make the selector shape readable here.

## Bounded grammar

Attribute names and values use the same closed fragment grammar:

- 1 to 64 ASCII bytes;
- start with `a` through `z`;
- contain only lowercase letters, digits, and single hyphens;
- end with a lowercase letter or digit;
- never contain `--`.

Values are intentionally unquoted in the current alpha. Uppercase text, whitespace, punctuation,
numeric-leading values, selector operators, and multiple `=` signs are rejected with `PCS012`.
This keeps generated selectors canonical and makes their compatibility classification mechanical.

ARIA presence syntax is not accepted because the supported typed form asserts an exact semantic
value. Data attributes support both presence and exact-value tests.

## Conflicts and identity

One condition may test an attribute only once. These fail closed:

```rust,compile_fail
pc!("aria-[sort=ascending]:aria-[sort=descending]:block");
pc!("data-[density=compact]:data-[density=comfortable]:block");
pc!("data-[loading]:data-loading:block");
```

Compiler-owned short forms remain available for common states. A short form and its configurable
equivalent produce the same semantic identity and CSS, for example:

```rust
pc!("aria-expanded:block");
pc!("aria-[expanded=true]:block");
```

Configurable attribute variants commute with viewport/container conditions, theme, interaction,
motion, contrast, and writing direction. Selector transforms still preserve their relative source
order because selector composition is not generally commutative.

## Compatibility boundary

These variants were introduced by policy 4 and remain allowed by compatibility policy schema 2 /
policy 7, including
`baseline-widely`. They are classified as `configurable-attribute-variants` and enforced by the
compiler's bounded attribute-selector grammar.

An arbitrary selector such as `[&[role=button]]:` remains `CMP003` under the strict profile. Use an
explicit `--targets none` build only when a downstream pipeline intentionally owns compatibility.

See the [syntax contract](../reference/syntax.md), [utility reference](../reference/utilities.md),
and [compatibility policy](../reference/compatibility-policy.md).
