<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/typed-styles/","category":"Core","eyebrow":"Typed styles","order":2}
-->

# Types at author time. CSS at run time.

Stable 128-bit identities connect Rust source, emitted classes, manifests, and physical declarations.

## One literal, one stable identity {#pc}

pc! accepts one visible string literal. The macro parses semantics, rejects conflicting slots, and returns a sealed Style.

```rust
const CARD: Style = pc!(
  "grid gap-4 rounded-xl border border-line p-6 shadow-md"
);
```

## Visible state spaces {#pcx}

pcx! compiles every visible branch combination and selects one identity at runtime. Independent clauses that can collide are rejected.

```rust
let style = pcx!(
  "block",
  if active { "opacity-100" } else { "opacity-50" }
);
```

## Follow the physical effect {#lineage}

Manifest schema 5 joins semantic declarations to final rules, conditions, byte ranges, and synthesized effects such as box-shadow.
