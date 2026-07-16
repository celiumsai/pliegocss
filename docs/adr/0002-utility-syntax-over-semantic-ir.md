# ADR-0002: Utility syntax over semantic IR

Status: accepted

## Decision

The primary authoring API is compact utility syntax parsed by Rust macros. Parsed utilities become a
typed semantic IR before validation or emission.

## Consequences

- Authoring remains close to Tailwind's speed.
- Compiler internals do not depend on string order.
- Parser, diagnostics, editor tooling, and documentation share one utility catalog.
