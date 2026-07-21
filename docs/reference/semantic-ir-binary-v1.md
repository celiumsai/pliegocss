# Semantic IR binary format 1

Status: **historical, machine-enforced compatibility input at `0.1.0-rc.2`**

Historical format. The current codec is
[semantic IR binary format 2](./semantic-ir-binary-v2.md); it rejects format-1 bytes and requires
source-based regeneration.

This document specifies the persisted, provenance-bearing semantic IR envelope exposed by
`pliego-css-compiler`. It is distinct from [StyleId format 2](./style-id-format-v2.md): the identity
stream answers whether two normalized styles have the same semantics, while this artifact can be
decoded back into validated IR and retains each assignment's portable source span.

The format is intended for exact-revision build tools, caches, and future hot-reload transport. It
is not browser state, a public application DTO, or an authentication format.

## Public codec boundary

```rust
use pliego_css_compiler::{decode_style_ir, encode_style_ir, lower_style, IR_BINARY_FORMAT_VERSION};
use pliego_css_parser::parse_style_list;

let syntax = parse_style_list("md:hover:bg-accent p-4")?;
let style = lower_style(&syntax)?;
let bytes = encode_style_ir(&style)?;
let decoded = decode_style_ir(&bytes)?;
assert_eq!(decoded.id, style.id);
assert_eq!(encode_style_ir(&decoded)?, bytes);
assert_eq!(IR_BINARY_FORMAT_VERSION, 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Seed-theme wrappers are also available for repository tooling. Consumers with a configured theme
must use the explicit-theme functions; decoding under a different `ThemeId` fails.

The codec returns and accepts owned byte buffers; filesystem layout, atomic writes, cache eviction,
and transport are caller responsibilities. Cache keys should include the PliegoCSS package revision,
`IR_BINARY_FORMAT_VERSION`, active `ThemeId`, and `StyleId`. Treat any decode error as a cache miss
and rebuild from source; do not retry by guessing versions or deserializing `SemanticStyle` fields.

The encoder accepts compiler-normalized IR, such as the result of `lower_style`. Calling
`SemanticStyle::validate()` proves structural safety but does not normalize cascade order; manually
reordered assignments are rejected as `NonCanonical` instead of being silently reinterpreted.

## Header

All multibyte integers are unsigned big-endian unless a field explicitly says otherwise.

| Offset | Width | Field | Format-1 value |
|---:|---:|---|---|
| 0 | 8 | magic | `PLGCIR\0\0` |
| 8 | 2 | semantic IR format | `1` |
| 10 | 2 | StyleId format | `2` |
| 12 | 2 | ThemeId format | `1` |
| 14 | 16 | active `ThemeId` | complete `u128` |
| 30 | 16 | stored `StyleId` | complete `u128` |
| 46 | 32 | payload SHA-256 | digest of bytes from offset 78 through EOF |
| 78 | 4 | assignment-record count | `u32` |

The decoder accepts only the current explicit versions. It never guesses an older layout or treats
one format number as covering another format.

The payload digest covers the record count, resolved semantic bytes, and every source span. StyleId
cannot cover this role because provenance is intentionally absent from semantic identity. The digest
detects accidental corruption; it is not a signature or MAC.

## Assignment records

Each assignment is encoded as:

```text
semantic_length:u32
semantic_record:[u8; semantic_length]
source_start:u32
source_end:u32
```

`semantic_record` is the assignment payload already defined by StyleId format 2:

1. `0x11` utility family, resolving an arbitrary property to its canonical UTF-8 name;
2. `0x12` sorted semantic slots;
3. `0x13` typed value, resolving arbitrary values and custom properties by UTF-8 content;
4. `0x14` canonical negative and important booleans;
5. `0x15` resolved condition dimensions, state set, and ordered selector strings.

No condition, selector, arbitrary-value, custom-property, or arbitrary-property table index enters
the artifact. Rust enum discriminants, field order, `Debug` output, pointer width, and insertion
order are not wire inputs.

The two source fields are the half-open byte span from semantic IR. They are outside
`semantic_record`, so changing only provenance changes artifact bytes but not StyleId.

## Canonical order

Records are sorted lexicographically by `(semantic_record, source_start, source_end)`. Exact wire
duplicates are invalid, and distinct spans do not make a repeated semantic assignment valid. The
encoder also rejects same-condition conflicts that compiler lowering would never produce; input must
be conflict-free, deduplicated, and compiler-normalized. A decoder reconstructs intern tables from
resolved content, validates the result, restores the compiler's canonical cascade order, re-derives
StyleId under the supplied theme, re-encodes it, and requires byte-for-byte equality with the input.
Valid wire records in a different order are rejected as noncanonical. Wire identity order is never
exposed as emitter order; the `p-4 px-2` shorthand/refinement case is frozen explicitly by tests.

This projection deliberately ignores unused intern-table entries. The decoded Rust vectors need not
preserve an implementation-specific insertion order; the executable semantics, spans, emitted CSS,
identity, and re-encoded bytes are preserved.

## Defensive limits

Format 1 rejects input before unbounded allocation:

- complete artifact: at most 16 MiB;
- assignment records: at most 65,535;
- selector transforms in one condition: at most 65,535;
- selector-transform occurrences across the complete artifact: at most 65,535;
- one resolved UTF-8 field: at most 1 MiB;
- one semantic record: bounded by the complete artifact and checked length arithmetic.

Record count is checked against the bytes required even for minimum framing before allocation, and a
semantic payload is checked together with its trailing span before it is copied. The encoder applies
matching table/count ceilings before structural validation. Truncation, trailing bytes, invalid
UTF-8, unknown tags, invalid booleans or numeric constructors, empty spans, malformed references,
and size/count overflows fail loudly.

## Trust boundary

Decoded bytes are untrusted. Success requires all of the following:

- header magic and every independent format version match;
- stored ThemeId equals the supplied registry;
- the SHA-256 of the complete record payload equals the stored digest;
- token and breakpoint IDs exist in that registry and match their typed namespaces;
- arbitrary values, custom properties, selectors, and arbitrary property names pass the same
  safety rules as compiler lowering;
- `SemanticStyle::validate()` accepts all structural invariants;
- assignments are conflict-free, deduplicated, and in canonical compiler cascade order;
- recomputed StyleId equals the stored StyleId;
- canonical re-encoding equals every input byte.

The artifact has no signature or MAC. These checks prevent malformed or mixed compiler artifacts;
they do not establish who produced the bytes.

The text checks target a standalone external stylesheet, which is PliegoCSS's production artifact
boundary. Arbitrary CSS values may legitimately contain `<`, for example in an SVG data URL. Do not
concatenate decoded or emitted CSS directly inside an HTML `<style>` element: HTML recognizes a
`</style>` end tag independently of CSS quoting. An inline integration must use an explicit
HTML-context serializer or escaping layer; the IR codec does not provide one.

## Frozen format-1 vector

The compiler test vector lowers:

```text
block md:dark:hover:[&>svg]:bg-accent/20 -mt-[2rem]! [mask-type:luminance] text-(color:--label)
```

Its complete artifact is 309 bytes and has SHA-256
`1322021c15d1ffd053fcb7829a1a610ad3df9e4ccdbca74d01ccb38c869623de`. The test freezes the full
618-character hexadecimal artifact, not only this summary digest. Separate tests freeze every
inverse enum tag, all catalog examples, manual numeric/color variants, cascade preservation,
theme binding, defensive limits, malformed framing, unsafe text, and corruption with a recomputed
payload digest.

## Change policy

Any change to header fields, tag meanings, numeric widths, length framing, record ordering, included
provenance, or validation semantics requires incrementing `IR_BINARY_FORMAT_VERSION` and adding
migration notes. A StyleId or ThemeId format change is also reported independently in the header.

Format-1 golden bytes, length, SHA-256, round-trip behavior, and the corruption matrix are frozen by
compiler tests. Updating that vector is a compatibility decision, not routine snapshot churn.
