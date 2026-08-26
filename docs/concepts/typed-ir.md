# Typed intermediate representation

The syntax parser preserves what the developer wrote. Semantic lowering converts it into a flat,
portable representation used for validation, hashing, CSS emission, manifests, and tooling.

## Portability

Syntax spans use native `usize` while slicing the decoded Rust string. Semantic spans use `u32` so a
manifest has the same shape on native server targets and `wasm32`.

The semantic IR contains no `syn`, `quote`, `proc_macro2`, trait objects, or public generic types.
Build-time parser dependencies remain outside application runtime code.

## Flat structure

```text
SemanticStyle
├── symbols
├── conditions
├── values
├── utilities
├── assignments
└── arbitrary properties
```

Compact IDs connect the tables. Utilities point to contiguous assignment ranges rather than owning a
heap allocation per utility.

## Conditions

A condition stores independent dimensions:

- viewport breakpoint;
- container breakpoint;
- cascade layer;
- theme mode;
- motion preference;
- contrast preference;
- pseudo-state bitset;
- ordered selector transforms.

This makes `md:hover:` and `hover:md:` identical after normalization. Ordered arbitrary selector
transforms remain separate because their order can change CSS semantics.

The cascade dimension is closed: unlayered, base, components, utilities, or overrides. Unlayered is
the backward-compatible default. Layered values map to namespaced native `pliego.*` layers and enter
StyleId, persisted IR, conflict analysis, and provenance instead of being treated as emitter text.

## Slots and footprints

The current IR defines 85 leaf slots. A `SlotSet(u128)` represents the complete footprint of a
utility. This enables constant-time overlap and refinement checks:

```text
p-4  -> top + right + bottom + left
px-2 -> right + left
```

Since the horizontal footprint is narrower, `p-4 px-2` is a valid refinement independent of source
order. `p-4 p-6` writes different values over the same footprint and is a conflict.

Effects that share a physical CSS property can remain semantically distinct. Ring and shadow, for
example, compose before the emitter produces `box-shadow`.

## Values

Values distinguish tokens, exact numbers, lengths, percentages, colors, keywords, and custom
properties. Floating point is not used for normalized identity because platform-dependent formatting
would undermine deterministic hashes.

## Style identity

`StyleId` is a 128-bit semantic identity. The final hash is computed from normalized content, not
symbol-table insertion order or original utility order. Equivalent authoring must therefore produce
one ID:

```rust,ignore
pc!("flex gap-4")
pc!("gap-4 flex")
```

Catalog lowering derives the ID from StyleId format 2: an explicit tagged and length-framed binary
stream over normalized semantic content and the active `ThemeId`. Multibyte fields are big-endian;
interned text is encoded by UTF-8 content; assignment records are sorted by their encoded bytes; and
spans, source spelling, table insertion positions, and Rust `Debug` representations are excluded.
SHA-256 compacts that stream and its first 128 bits, interpreted big-endian, become `StyleId`.

The CSS class still uses class-name format 1: `pc_` plus the complete ID in lowercase base 36.
StyleId format 2 and its candidate vectors are machine-enforced at public preview
`0.1.0-rc.3`, while final `0.1.0` promotion remains open. The complete byte grammar and migration boundary are in the
[StyleId format-2 reference](../reference/style-id-format-v2.md).

## Persistence is not identity

The StyleId stream cannot reconstruct semantic IR because it intentionally excludes spans and the
stored ID. Persistent compiler caches use a separate
[semantic IR binary format 2](../reference/semantic-ir-binary-v2.md). That envelope stores resolved
assignment records plus portable spans, declares the independent IR/StyleId/ThemeId versions, and
requires the active theme to decode and verify the stored identity.

The codec is implemented in `pliego-css-compiler`, not by deriving Serde on these Rust types. This
keeps table IDs and enum layout private, adds no dependency to the IR crate, and makes limits,
canonical order, unsafe text rejection, and migrations explicit.

Manifest schema 4 also avoids serializing internal IR. It projects each canonical assignment to a
versioned semantic-declaration ID and exposes only its direct token edge plus application ownership.
Conditions, slots, intern tables, values, and binary artifact bytes remain private. Manifest schema
5 keeps those semantic IDs unchanged and adds a separate physical graph over final CSS declaration
occurrences; it does not reinterpret semantic declarations as one-to-one CSS properties.

## Invariants

`SemanticStyle` validation checks table references, assignment ranges, condition references,
footprints, slot/value compatibility, and other structural guarantees before emission. Invalid IR
must fail before CSS generation.
