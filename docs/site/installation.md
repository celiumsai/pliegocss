<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/installation/","category":"Start","eyebrow":"Installation","order":7}
-->

# Choose an adoption surface.

Install the CLI for any project, add the Rust crates only when typed authoring belongs in the application.

## CLI-only adoption {#cli}

The command-line compiler can audit, transform, explain, bundle, and migrate standard CSS without adding a runtime dependency to the application.

```console
cargo install pliego-cssc --version '=0.1.0-rc.2' --locked
pliego-cssc --version
```

## Typed Rust authoring {#rust}

Add pliego-css and its macro crate to applications that want compile-time style identities. The emitted result remains ordinary CSS.

```console
[dependencies]
pliego-css = "=0.1.0-rc.2"
```

## Pin the exact release identity {#pin}

Before the public release, use the signed project tag plus Cargo.lock. Do not track a moving branch in a production build.
