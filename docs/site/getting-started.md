<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/getting-started/","category":"Start","eyebrow":"Getting started","order":1}
-->

# Start with the CSS you already have.

Audit first, compile when it adds value, and keep the browser contract ordinary.

## Install the release candidate {#install}

The 0.1.0-rc.3 compatibility unit is under exact-source verification. Use this command after all nineteen crates are published and replayed with Rust 1.85.

```console
cargo install pliego-cssc --version '=0.1.0-rc.3' --locked
```

## Audit ordinary CSS {#audit}

Start without changing authoring. The analyzer emits human, canonical JSON, or SARIF findings and fails closed on unknown surfaces.

```console
pliego-cssc audit --input app.css \
  --targets baseline-widely --format human
```

## Add typed identities selectively {#compile}

Rust literals are validated at compile time and compile to static standards-compliant CSS. There is no styling runtime.

```rust
use pliego_css::{pc, Style};

const BUTTON: Style = pc!(
  "inline-flex gap-2 rounded-lg bg-accent px-4 py-2"
);
```
