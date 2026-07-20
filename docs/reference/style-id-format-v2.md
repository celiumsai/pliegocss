# StyleId binary format 2

Status: **implemented and protected by exact candidate vectors at workspace version 0.0.0**

This document specifies the canonical byte stream used by
`STYLE_ID_FORMAT_VERSION = 2`, the derivation of the 128-bit `StyleId`, and its relationship to a
CSS class. It is an identity contract for normalized semantic IR; it is not a serialization format
for reconstructing the original utility source or a validation substitute for `SemanticStyle`.

## Normative conventions

- Every multibyte integer is unsigned and big-endian unless its type is explicitly signed.
- `u8`, `u16`, `u32`, `u128`, `i32`, and `i64` have their normal fixed widths.
- `text` is `u32 byte_length || UTF-8 bytes`. The length counts bytes, not Unicode scalar values.
- `count8` is a `u8`; `count32` is a big-endian `u32`.
- `||` means byte concatenation. Numeric tags below are single bytes.
- A `ThemeId` and final `StyleId` are encoded as complete 16-byte big-endian values.
- Encoding starts only after `SemanticStyle::validate()` succeeds. Invalid references, unrepresented
  state/slot bits, or lengths outside their fixed-width fields are errors; they are not truncated.

Source spans, source spelling, table insertion positions, the previously assigned `style.id`, and
unused table entries are not encoded. Interned arbitrary values, custom-property names, arbitrary
property names, and selectors are resolved and encoded as their UTF-8 content.

## Top-level stream

The complete stream is:

```text
"pliego-style-id\0"             16 literal bytes
0x0002                          StyleId format, u16
0x01                            theme section tag
theme_id_format                 u16; currently 0x0001
theme_id                        u128
0x02                            assignments section tag
assignment_count                count32
repeat assignment_count times:
    0x03                        assignment entry tag
    assignment_byte_length      count32
    assignment_bytes
```

The encoder builds every `assignment_bytes` record first, sorts the records lexicographically by
their raw bytes, and removes exact duplicate records. The resulting order and count enter the stream.
Authoring order, assignment-table order, and duplicate provenance therefore do not affect identity.

The explicit theme-format field prevents the same 128 theme bits from being interpreted under an
unreported ThemeId contract. The current stream embeds `THEME_ID_FORMAT_VERSION = 2` and the
theme binary format.

## Assignment record

Every assignment record has exactly five tagged fields in this order:

```text
0x11 utility_tag [arbitrary_property_name]
0x12 slot_count:u8 slot_tag...
0x13 semantic_value
0x14 negative:u8 important:u8
0x15 condition
```

`negative` and `important` are `0` or `1`. Slot tags are emitted in canonical `SlotSet` iteration
order. A bit present in the set without a defined tag is an error.

For utility tag 60 (`ArbitraryProperty`), `arbitrary_property_name` is a `text` field immediately
after the utility tag. Other utility tags have no utility payload.

### Utility tags

| Tags | Semantic utility names in tag order |
|---|---|
| 1–10 | `Display`, `Visibility`, `Position`, `Inset`, `ZIndex`, `Overflow`, `Width`, `MinWidth`, `MaxWidth`, `Height` |
| 11–20 | `MinHeight`, `MaxHeight`, `AspectRatio`, `Margin`, `Padding`, `Gap`, `FlexDirection`, `FlexWrap`, `FlexGrow`, `FlexShrink` |
| 21–30 | `FlexBasis`, `AlignItems`, `AlignContent`, `AlignSelf`, `JustifyContent`, `JustifyItems`, `JustifySelf`, `GridTemplateColumns`, `GridTemplateRows`, `GridColumn` |
| 31–40 | `GridRow`, `BackgroundColor`, `BackgroundImage`, `TextColor`, `FontFamily`, `FontSize`, `FontWeight`, `LineHeight`, `LetterSpacing`, `FontSmoothing` |
| 41–50 | `TextAlign`, `TextDecorationLine`, `BorderWidth`, `BorderColor`, `BorderStyle`, `BorderRadius`, `OutlineWidth`, `OutlineColor`, `OutlineOffset`, `OutlineStyle` |
| 51–60 | `Opacity`, `BoxShadow`, `RingWidth`, `RingColor`, `Transform`, `TransitionProperty`, `Resize`, `Cursor`, `PointerEvents`, `ArbitraryProperty` |
| 61 | `ContainerType` |
| 62 | `WritingMode` |

### Slot tags

| Tags | Semantic slot names in tag order |
|---|---|
| 1–10 | `DisplayMode`, `Visibility`, `PositionMode`, `InsetTop`, `InsetRight`, `InsetBottom`, `InsetLeft`, `ZIndex`, `OverflowX`, `OverflowY` |
| 11–20 | `Width`, `MinWidth`, `MaxWidth`, `Height`, `MinHeight`, `MaxHeight`, `AspectRatio`, `MarginTop`, `MarginRight`, `MarginBottom` |
| 21–30 | `MarginLeft`, `PaddingTop`, `PaddingRight`, `PaddingBottom`, `PaddingLeft`, `GapRow`, `GapColumn`, `FlexDirection`, `FlexWrap`, `FlexGrow` |
| 31–40 | `FlexShrink`, `FlexBasis`, `AlignItems`, `AlignContent`, `AlignSelf`, `JustifyContent`, `JustifyItems`, `JustifySelf`, `GridTemplateColumns`, `GridTemplateRows` |
| 41–50 | `GridColumn`, `GridRow`, `BackgroundColor`, `BackgroundImage`, `TextColor`, `FontFamily`, `FontSize`, `FontWeight`, `LineHeight`, `LetterSpacing` |
| 51–60 | `FontSmoothing`, `TextAlign`, `TextDecorationLine`, `BorderTopWidth`, `BorderRightWidth`, `BorderBottomWidth`, `BorderLeftWidth`, `BorderTopColor`, `BorderRightColor`, `BorderBottomColor` |
| 61–70 | `BorderLeftColor`, `BorderTopStyle`, `BorderRightStyle`, `BorderBottomStyle`, `BorderLeftStyle`, `BorderTopLeftRadius`, `BorderTopRightRadius`, `BorderBottomRightRadius`, `BorderBottomLeftRadius`, `OutlineWidth` |
| 71–80 | `OutlineColor`, `OutlineOffset`, `OutlineStyle`, `Opacity`, `BoxShadow`, `RingWidth`, `RingColor`, `Transform`, `TransitionProperty`, `Resize` |
| 81–83 | `Cursor`, `PointerEvents`, `ArbitraryProperty` |
| 84 | `ContainerType` |
| 85 | `WritingMode` |

## Semantic values

The byte after record tag `0x13` selects one value form:

| Tag | Form | Payload |
|---:|---|---|
| 1 | `Keyword` | `keyword_tag:u8` |
| 2 | `Token` | `token_kind_tag:u8 || token_id:u32` |
| 3 | `Integer` | `value:i32` |
| 4 | `Number` | `coefficient:i64 || decimal_scale:u8` |
| 5 | `Length` | `coefficient:i64 || decimal_scale:u8 || length_unit_tag:u8` |
| 6 | `Percentage` | `basis_points:u16` |
| 7 | `Color` | one color payload below |
| 8 | `Fraction` | `numerator:u16 || denominator:u16` |
| 9 | `Arbitrary` | `text` |
| 10 | `CustomProperty` | `name:text || value_kind_tag:u8` |

`CssNumber` is already canonicalized before encoding; the encoder records its signed base-10
coefficient and scale without formatting it as text.

Color payloads are:

| Tag | Color | Payload |
|---:|---|---|
| 1 | transparent | none |
| 2 | current color | none |
| 3 | token color | `token_id:u32 || has_alpha:u8 || [basis_points:u16]` |
| 4 | sRGBA | `red:u8 || green:u8 || blue:u8 || alpha:u8` |

`has_alpha` is `0` or `1`; the basis-point field exists only when it is `1`.

### Value tag ledgers

- Token kinds 1–10: `Spacing`, `Color`, `FontFamily`, `FontSize`, `FontWeight`, `LineHeight`,
  `LetterSpacing`, `Radius`, `Shadow`, `ZIndex`.
- Length units 1–9: `Px`, `Rem`, `Em`, `Percent`, `Ch`, `Vw`, `Vh`, `Dvw`, `Dvh`.
- Custom-property value kinds 1–13: `Any`, `Keyword`, `Token`, `Integer`, `Number`, `Length`,
  `Percentage`, `Color`, `Fraction`, `Shadow`, `Image`, `GridTemplate`, `Transform`.
- Keyword tags 1–63: `None`, `Auto`, `Normal`, `Block`, `Inline`, `InlineBlock`, `Flex`,
  `InlineFlex`, `Grid`, `InlineGrid`, `Visible`, `Hidden`, `Collapse`, `Static`, `Relative`,
  `Absolute`, `Fixed`, `Sticky`, `Clip`, `Scroll`, `Row`, `RowReverse`, `Column`,
  `ColumnReverse`, `Wrap`, `WrapReverse`, `NoWrap`, `Start`, `End`, `Center`, `SpaceBetween`,
  `SpaceAround`, `SpaceEvenly`, `Stretch`, `Baseline`, `MinContent`, `MaxContent`, `FitContent`,
  `Content`, `Left`, `Right`, `Justify`, `Bold`, `Underline`, `Overline`, `LineThrough`, `Solid`,
  `Dashed`, `Dotted`, `Double`, `Pointer`, `Default`, `Wait`, `NotAllowed`, `Text`, `Move`,
  `Antialiased`, `Colors`, `Vertical`, `InlineSize`, `HorizontalTb`, `VerticalLr`, `VerticalRl`.

## Condition encoding

The payload after record tag `0x15` is:

```text
has_breakpoint:u8
[breakpoint_id:u16]
[0x16 container_breakpoint_id:u16]
[0x17 cascade_layer_tag:u8]
theme_mode_tag:u8
motion_preference_tag:u8
contrast_preference_tag:u8
state_count:u8
state_tag...
selector_count:u32
selector:text...
```

`has_breakpoint` is `0` or `1`; the viewport breakpoint ID exists only when it is `1`. The disjoint
`0x16` field exists only for a typed `cq-<breakpoint>` condition and carries its active-theme
breakpoint ID. The disjoint `0x17` field exists only for an explicit typed cascade layer; its tag is
1 `Base`, 2 `Components`, 3 `Utilities`, or 4 `Overrides`. Unlayered conditions omit the marker.
Existing records without those optional dimensions remain byte-identical. Selectors preserve their
normalized semantic order because selector-transform order can change CSS behavior. Pseudo states
use a fixed order, regardless of authoring order.

| Dimension | Tags |
|---|---|
| Theme mode | 1 `Any`, 2 `Light`, 3 `Dark` |
| Motion preference | 1 `Any`, 2 `Safe`, 3 `Reduce` |
| Contrast preference | 1 `Any`, 2 `More`, 3 `Less` |
| Pseudo state | 1 `Hover`, 2 `Focus`, 3 `FocusVisible`, 4 `Active`, 5 `Disabled` |
| Cascade layer after marker `0x17` | 1 `Base`, 2 `Components`, 3 `Utilities`, 4 `Overrides` |

## Hash and compact ID

Derivation is normative:

1. compute SHA-256 over the complete canonical stream;
2. take digest bytes 0 through 15, preserving their order;
3. interpret those 16 bytes as one big-endian `u128`;
4. if the result is zero, use one because zero represents an unresolved style.

The 32-character lowercase hexadecimal form shown in JSON is that complete `u128`, padded with
leading zeroes. A 128-bit truncation can map distinct streams to the same number. During CLI
aggregation PliegoCSS retains canonical streams, rejects one stream mapped to multiple IDs, and
rejects distinct streams mapped to one ID instead of silently merging their CSS.

SHA-256 here provides deterministic compaction. A StyleId is not authentication, authorization,
signature, or proof that two untrusted inputs are semantically equal. The digest cannot recover the
original stream or source.

## CSS class format

`CLASS_NAME_FORMAT_VERSION = 1` encodes the complete `StyleId` as unpadded lowercase base 36 and
prefixes it with `pc_`. Zero would encode as `pc_0`, although format-2 derivation reserves zero and
remaps it to one.

The class algorithm did not change when StyleId format 2 was introduced, so its format version stays
1. Its input did change: every format-1 candidate StyleId and corresponding `pc_*` class must be
treated as obsolete.

Current frozen examples are:

| Theme/style | StyleId | Class |
|---|---|---|
| Seed theme, `flex gap-4` | `70cb04ef9bf9621f5826351f1778f68e` | `pc_6oe73ec16rbb7ublcoa3bpzf2` |
| Minimal compatibility theme, `flex gap-gutter tablet:grid` | `e0b572e3fdfbf091d9a2ddb126278634` | `pc_dax2y1pql4op1rjk97yv9e88k` |
| Seed theme, `container-inline cq-sm:grid` | `11893074aaaf88f7b356ad6d53a913e2` | `pc_11dgqogrs5aq1d9ocwyulg6w2` |

## Document versions

Artifacts that expose identity now report all three related versions at the top level:

```json
{
  "styleIdFormatVersion": 2,
  "classNameFormatVersion": 1,
  "themeIdFormatVersion": 2
}
```

These fields are required in CSS manifest schemas 3, 4, and 5, inspection schema 2, catalog schema 3,
and explain schema 2. They let a consumer reject mixed identity contracts before interpreting a
`styleId` or class. Structured diagnostics remain schema 1, the theme configuration remains schema
1, the theme identity and binary remain format 1, declarative bundle plans use schemas 1/2,
the reachability sidecar is schema 1, nested manifest graphs are schemas 1/2, and the PliegoRS
source-surface contract remains schema 2.

## Migration from the format-1 candidate

There is no hash-to-hash conversion. Migration requires the normalized semantic inputs and active
theme:

1. recompile all crates containing `pc!` or `pcx!` and rebuild SSR/SSG markup;
2. regenerate every CSS file, schema-3/schema-4/schema-5 manifest, catalog, explanation cache, and bundle output;
3. invalidate StyleId- and class-keyed caches or indexes;
4. publish markup, CSS, and manifests as one coordinated application revision; and
5. reject any artifact whose three format fields do not match the consuming compiler revision.

Do not mix format-1 classes with format-2 CSS. An unchanged ThemeId does not make that combination
valid because the StyleId stream version is itself part of the hash input.

## Change control

Changing an assigned tag, existing field meaning/order/width, endianness, domain bytes, text framing,
sorting/deduplication rule, SHA-256 truncation rule, or zero handling requires a new
`STYLE_ID_FORMAT_VERSION` and migration notes. During the unpublished candidate, append-only tags and
disjoint optional markers may extend format 2 only when every previously valid stream and frozen ID
remains byte-identical; the semantic-IR persistence envelope still increments its own version because
it has a decoder. Changing only the `pc_` base-36 algorithm requires a
new `CLASS_NAME_FORMAT_VERSION`. Changing ThemeId derivation or its binary representation follows
the separate theme format contract.

See the [compatibility candidate contract](./compatibility.md), [typed IR](../concepts/typed-ir.md),
and [CLI document schemas](./cli.md).
