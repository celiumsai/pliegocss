# ADR-0001: Compile to standard CSS

Status: accepted

## Decision

PliegoCSS is an authoring and build-time compiler. It emits static standards-compliant CSS and does
not replace browser cascade, layout, or paint.

## Consequences

- Browsers execute native CSS without a PliegoCSS WASM runtime.
- CSS platform knowledge and interoperability remain relevant.
- Building a browser engine is outside project scope.
