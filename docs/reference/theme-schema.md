# TOML theme configuration schema 1

Status: implemented schema version 1

This document specifies only the schema-1 `pliego.theme.toml` adapter. Cargo also accepts an
explicit DTCG Resolver JSON document through
`theme!(tokens = FILE, inputs = { "modifier" => "context" })`; that separate bridge is documented
in [DTCG 2025.10 bridge](./dtcg-bridge.md).

Schema-1 PliegoCSS theme files are TOML documents read during the Cargo build. Version 1 requires
both top-level fields:

```toml
schema = 1
extends = "seed"

[tokens.color]
brand = "#3366ff"
accent = "oklch(62% .21 29)"

[tokens.spacing]
gutter = "1.375rem"

[tokens.font-family]
display = "Inter, ui-sans-serif, sans-serif"

[breakpoints]
tablet = "52rem"
wide = "80rem"
```

Unknown top-level fields and unknown token tables are errors. `schema = 1` is the only supported
schema and `extends = "seed"` is the only supported base. Every table is optional, but the two
top-level fields are not.

## Token tables

The tables map token names to CSS value strings.

| TOML table | Semantic namespace | Example |
|---|---|---|
| `[tokens.color]` | Color | `brand = "#3366ff"` |
| `[tokens.spacing]` | Spacing and the current size families | `gutter = "1.375rem"` |
| `[tokens.font-family]` | Font family | `display = "Inter, sans-serif"` |
| `[tokens.font-size]` | Font size | `hero = "3.5rem"` |
| `[tokens.font-weight]` | Font weight | `book = "450"` |
| `[tokens.line-height]` | Line height | `copy = "1.65"` |
| `[tokens.letter-spacing]` | Letter spacing | `caps = ".08em"` |
| `[tokens.radius]` | Border radius | `card = ".875rem"` |
| `[tokens.shadow]` | Box shadow | `card = "0 12px 32px #0002"` |

There is no schema-1 table for z-index or arbitrary namespaces.

Definitions are merged over the seed registry. A canonical name already present in the seed
replaces that value while preserving its token ID. A new name adds a token to that namespace. The
schema does not remove seed definitions.

## Names

Names are trimmed and normalized to lowercase kebab case:

- ASCII letters and digits are retained;
- hyphens, underscores, and ASCII whitespace become one `-` separator;
- other characters are rejected;
- a name cannot be empty or end in a separator;
- two source names that normalize to the same name are rejected.

For example, `Brand_Blue` and `brand blue` both normalize to `brand-blue`, so declaring both in the
same table is an error.

Token IDs are deterministic within their namespace. If two distinct names produce the same ID, the
registry reports a fatal collision instead of selecting one silently.

## Values

All values are trimmed and must be non-empty. Schema 1 rejects control characters, `{`, `}`, `;`,
CSS comment delimiters, and `!important` in any casing. This prevents a token value from escaping
its declaration when the theme is emitted.

After those declaration-safety checks, `ThemeRegistry` applies a conservative validator for each
semantic namespace:

- spacing, font-size, and radius values accept zero, non-negative CSS lengths or percentages, and
  balanced `calc`, `clamp`, `min`, `max`, or `var` function forms;
- colors accept canonical 3/4/6/8-digit hexadecimal values, alphabetic CSS keywords, or the supported
  color-function forms; the seed names `transparent` and `current` retain their exact semantic values;
- font weights accept CSS weight keywords or integers from 1 through 1000;
- line height, letter spacing, shadow, font family, and the internal z-index namespace each have a
  bounded domain check rather than accepting an arbitrary cross-domain fragment.

The registry therefore rejects obvious domain errors such as `color = "1rem"`, `spacing = "red"`,
or an unknown weight spelling before macro expansion. These checks are intentionally narrower than a
complete CSS grammar: complex function bodies are structurally bounded but the CLI remains responsible
for passing the complete emitted artifact through Lightning CSS. A custom registry accepted by a macro
is typed and declaration-safe, but that alone is not a full browser-grammar proof.

## Breakpoints

`[breakpoints]` maps variant names to positive CSS lengths:

```toml
[breakpoints]
sm = "42rem"       # overrides the seed value and preserves its ID
tablet = "52rem"   # adds a new responsive variant
```

Accepted units are `px`, `rem`, `em`, `ch`, and `vw`. Units are lowercase and a value must be a
finite number greater than zero. Percentages, unitless values, media-query expressions, and values
such as `0rem` are rejected.

New breakpoint IDs are assigned deterministically after the seed IDs. Duplicate names or IDs are
fatal registry errors. A custom breakpoint cannot use a built-in condition name such as `hover`,
`focus`, `dark`, `motion-safe`, or `placeholder`; configuration rejects the collision instead of
changing the meaning of that variant. Seed breakpoint names such as `sm` may be overridden.

## Build artifact and identity

`pliego-css-build` converts the registry selected from this TOML schema or one explicit DTCG
Resolver context to a canonical, content-addressed binary file in Cargo's `OUT_DIR`. Its filename
contains both the binary-format version and the 32-character
lowercase `ThemeId`, so a format migration cannot alias an older artifact. The binary has a
magic header, format version, size limits, UTF-8 validation, registry validation, and a stored-ID
check when decoded.

ThemeId format 2 hashes an explicitly domain-separated and length-framed canonical stream with
SHA-256. The first 16 digest bytes, interpreted big-endian, form the `ThemeId` (with all-zero remapped
to one). Token and breakpoint records include explicit section/record tags, counts, fixed-width IDs,
and UTF-8 byte lengths. Input definitions are sorted before encoding, so order does not affect the
stream; any canonical value change does. Because the stored identity semantics changed, the current
theme binary is format 2. Format-1 binaries are rejected explicitly and must be regenerated.

The decoder rejects artifacts larger than 16 MiB, more than 65,536 entries in either the token or
breakpoint list, and names or values larger than 1 MiB each. It also rejects truncation, unknown
token-kind tags, trailing data, unsupported format versions, and records whose order or encoding is
not the canonical representation of the decoded registry. If a content-addressed filename already
exists with different bytes, the build reports an artifact conflict instead of overwriting it.

The build script exports:

- `PLIEGO_CSS_THEME_PATH`: path to the canonical binary artifact;
- `PLIEGO_CSS_THEME_ID`: expected 128-bit identity as 32 lowercase hexadecimal characters.

`pc!` and `pcx!` decode the artifact while Rust compiles. Missing theme environment variables select
the seed registry. A corrupt artifact, malformed identity, or identity mismatch fails compilation.
Neither the TOML/DTCG parser nor the registry is included in the application's runtime.

`ThemeId` is part of `StyleId` derivation. Identical utility semantics under two different theme
registries therefore receive different classes, and the CSS emitter rejects a style identity that
does not match its active theme.

See [Themes and tokens](../learn/themes-and-tokens.md) for the model and
[Theme troubleshooting](../troubleshooting/themes.md) for diagnostics. The separate
[DTCG 2025.10 bridge](./dtcg-bridge.md) exchanges token documents through a versioned adapter; it
does not change this TOML schema or make DTCG the internal theme representation. Cargo can select
one Resolver context through `theme!(tokens = FILE, inputs = {...})`; its artifact contains only the
resolved registry, while controlled CLI generation owns complete-graph publication.
