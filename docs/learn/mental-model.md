# Mental model

PliegoCSS's current Rust frontend keeps a compact utility-first workflow. This is an optional typed
authoring path inside the broader standards-first compiler/verifier, not the required product input:

```rust,ignore
pc!("flex items-center gap-4")
```

The `pc!` procedural macro parses this literal during Rust compilation. Each utility is resolved
into semantic assignments:

```text
flex         -> display.mode = flex
items-center -> alignment.items = center
gap-4        -> layout.gap = token(space.4)
```

The macro compiler validates conflicts, normalizes variants, encodes an explicit theme-scoped
StyleId format-2 byte stream, and derives the compact `StyleId` from the first 128 bits of its
SHA-256 digest in big-endian order.
The separate CSS pipeline scans only the explicitly supplied styles, inputs, or Rust source trees,
deduplicates equivalent rules, and emits static standards-compliant CSS. It does not currently
derive Cargo module, route, or island reachability.

`pcx!` applies the same model to conditional composition. Every statically enumerable clause
combination is validated and assigned a class during compilation; runtime code evaluates each
clause once and selects one precompiled `StyleId`. The implementation bounds the Cartesian product
at 64 combinations and rejects conflicts that can occur across clauses.

PliegoCSS does not replace CSS or the browser's rendering engine. Normal CSS remains available for
interoperability and the long tail of the platform.

## Four layers

```text
compact utility syntax
        ↓
semantic typed IR
        ↓
validation and optimization
        ↓
static browser CSS
```

The public syntax optimizes authoring speed. The IR provides correctness. The browser remains the
runtime.

Theme identity and output compatibility are explicit boundaries. `ThemeId` participates in every
`StyleId`; the CLI defaults to its versioned `modern` browser targets; and a default schema-3 manifest can
bind generated CSS to its theme, identity-format versions, target contract, source origins, SHA-256
digest, and byte length. Class-name format 1 still uses `pc_` plus lowercase base 36, but format-1
candidate classes changed because StyleId format 2 changed its input. These contracts improve build
coherence. Opt-in schema 4 can validate explicit application reachability from a framework sidecar,
and schema 5 can trace that semantic graph into exact final CSS ranges. Neither is a substitute for
browser resumability tests or automatic reachability analysis. See the
[StyleId format-2 reference](../reference/style-id-format-v2.md).
