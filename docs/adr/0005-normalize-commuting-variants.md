# ADR-0005: Normalize commuting variants only

Status: accepted for the F0 spike

## Context

Breakpoint, theme, preference, and pseudo-state variants usually describe independent condition
dimensions. Arbitrary selector transforms, however, can be order-sensitive.

## Decision

Built-in condition variants are normalized by semantic dimension, so `md:hover:` and `hover:md:`
describe the same condition. Selector transforms preserve source order and remain separate from the
condition key.

## Consequences

- Equivalent built-in conditions cannot evade conflict analysis through spelling order.
- Future selector composition remains standards-correct.
- The IR distinguishes conditions from ordered selector transforms.
