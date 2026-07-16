# ADR-0003: Conflicts do not depend on source order

Status: accepted for the F0 spike

## Decision

Contradictory assignments in the same condition are compile errors. Refinements by narrower footprint
or condition are allowed. Textual order never selects the winner.

## Consequences

- `flex grid` fails instead of silently choosing one.
- `p-4 px-2` and `px-2 p-4` are equivalent.
- Conditions must be canonicalized before conflict analysis.
