# Semantic IR binary format 2

Status: **machine-enforced prerelease contract at `0.1.0-rc.2`**

Format 2 is the current persisted, provenance-bearing semantic IR envelope exposed by
`pliego-css-compiler`. It is distinct from [StyleId format 2](./style-id-format-v2.md): StyleId is a
one-way identity stream, while this artifact reconstructs validated IR and retains each assignment's
portable source span.

Format 2 adds the typed container-query condition dimension, `ContainerType` utility/slot, and
`InlineSize` keyword. It replaces format 1; current decoders reject format-1 bytes instead of
guessing a migration. The later writing-mode extension appends `WritingMode`, `HorizontalTb`,
`VerticalLr`, and `VerticalRl` tags without changing any earlier format-2 byte stream. The typed
cascade-layer extension adds the disjoint optional condition marker `0x17`; unlayered records remain
byte-identical.

## Public codec boundary

```rust
use pliego_css_compiler::{decode_style_ir, encode_style_ir, lower_style, IR_BINARY_FORMAT_VERSION};
use pliego_css_parser::parse_style_list;

let syntax = parse_style_list("container-inline layer-components:md:cq-sm:hover:grid")?;
let style = lower_style(&syntax)?;
let bytes = encode_style_ir(&style)?;
let decoded = decode_style_ir(&bytes)?;
assert_eq!(decoded, style);
assert_eq!(encode_style_ir(&decoded)?, bytes);
assert_eq!(IR_BINARY_FORMAT_VERSION, 2);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Configured-theme consumers must use the explicit-theme functions. Decoding under a different
`ThemeId`, or with missing viewport/container breakpoint IDs, fails closed.

## Header

All multibyte integers are unsigned big-endian.

| Offset | Width | Field | Format-2 value |
|---:|---:|---|---|
| 0 | 8 | magic | `PLGCIR\0\0` |
| 8 | 2 | semantic IR format | `2` |
| 10 | 2 | StyleId format | `2` |
| 12 | 2 | ThemeId format | `2` |
| 14 | 16 | active `ThemeId` | complete `u128` |
| 30 | 16 | stored `StyleId` | complete `u128` |
| 46 | 32 | payload SHA-256 | digest of bytes from offset 78 through EOF |
| 78 | 4 | assignment-record count | `u32` |

The decoder accepts only the current explicit versions. The payload digest detects corruption; it
is not a signature or MAC.

## Assignment records

Each record is:

```text
semantic_length:u32
semantic_record:[u8; semantic_length]
source_start:u32
source_end:u32
```

The semantic record reuses the canonical StyleId assignment projection:

1. `0x11` utility family and any resolved arbitrary-property name;
2. `0x12` sorted semantic slots;
3. `0x13` typed value;
4. `0x14` negative and important booleans;
5. `0x15` resolved condition dimensions, state set, and ordered selector strings.

Format 2 recognizes utility tags 61 `ContainerType` and 62 `WritingMode`; slot tags 84
`ContainerType` and 85 `WritingMode`; keyword tag 60 `InlineSize` and append-only tags 61
`HorizontalTb`, 62 `VerticalLr`, and 63 `VerticalRl`; plus the optional condition field below. Tag
60 remains the payload-bearing `ArbitraryProperty` utility; existing tag meanings did not move.

## Optional condition fields

The condition payload begins with the existing viewport breakpoint presence byte and optional ID.
It then contains disjoint optional fields before theme/motion/contrast:

```text
has_viewport_breakpoint:u8
[viewport_breakpoint_id:u16]
[0x16 container_breakpoint_id:u16]
[0x17 cascade_layer_tag:u8]
theme_mode_tag:u8
motion_preference_tag:u8
contrast_preference_tag:u8
state_count:u8 state_tag...
selector_count:u32 selector:text...
```

`0x16` is present exactly when the condition has a typed `cq-<breakpoint>` dimension. The ID is
resolved against the active theme on decode. Absence preserves the exact semantic record used by
pre-container StyleId format-2 inputs. `0x17` is present exactly for a typed layer and carries 1
`Base`, 2 `Components`, 3 `Utilities`, or 4 `Overrides`. Unlayered conditions omit it, preserving
every pre-layer record. Unknown or zero layer tags fail closed.

## Canonical order and validation

Records sort lexicographically by `(semantic_record, source_start, source_end)`. Exact wire
duplicates, semantic duplicates with different spans, conflicting assignments, and noncanonical
cascade order are rejected. Decoding reconstructs intern tables, validates all text and references,
restores compiler cascade order, re-derives StyleId, re-encodes, and requires byte equality.

Decoded CSS text targets a standalone external stylesheet. The codec is not an HTML `<style>`
serializer and does not escape HTML end tags.

## Defensive limits

- complete artifact: 16 MiB;
- assignment records: 65,535;
- selector transforms per condition: 65,535;
- selector occurrences across the artifact: 65,535;
- one resolved UTF-8 field: 1 MiB.

All length arithmetic is checked before allocation. Truncation, trailing bytes, invalid UTF-8,
unknown tags, unsafe CSS text, invalid booleans/numbers, bad spans, missing theme references, digest
drift, StyleId drift, and noncanonical re-encoding fail loudly.

## Frozen format-2 vector

The inherited golden source remains 309 bytes because it has no container dimension. Its header now
declares format 2 and the complete artifact SHA-256 is
`337865f8e7b5b32fe58f67442537ad1d2926d94164d1692863a2616dddc85023`.

The container contract additionally freezes `container-inline cq-sm:grid` to StyleId
`11893074aaaf88f7b356ad6d53a913e2` and class `pc_11dgqogrs5aq1d9ocwyulg6w2`, plus round-trip and
nested-condition tests. Layered conditions have separate round-trip and canonical-order tests while
the inherited unlayered golden bytes remain exact.

## Migration from format 1

There is no byte-level upgrade function. Reparse the original source under the same theme, lower it
with the current compiler, encode format 2, and replace the cache entry atomically. Treat
`UnsupportedVersion` as a cache miss. Cache keys must include compiler revision,
`IR_BINARY_FORMAT_VERSION`, `ThemeId`, and `StyleId`.

## Change control

Any incompatible header, tag, framing, ordering, provenance, or validation change requires a new
`IR_BINARY_FORMAT_VERSION` and migration notes. Golden bytes, length, digest, round-trip behavior,
container references, cascade-layer markers, and the corruption matrix are executable release
evidence.
