# Conflict model

PliegoCSS does not use source order to resolve contradictory utilities. It compares semantic slots
inside a canonical condition.

## Assignment

Conceptually, every utility becomes one or more assignments:

```rust,ignore
struct Assignment {
    slot: Slot,
    value: SemanticValue,
    footprint: Footprint,
    condition: ConditionKey,
    source: SourceSpan,
}
```

## Rules

1. Same slot, condition, footprint, and value: deduplicate.
2. Same slot and condition with different values over the same footprint: error.
3. A narrower footprint may refine a broader one.
4. A narrower condition may override a broader one.
5. Mutually exclusive conditions do not conflict.
6. Different cascade layers establish explicit native precedence and do not conflict.
7. Textual order never changes the semantic result.

An arbitrary property uses its canonical property name as the effective conflict identity rather
than treating every `[property:value]` as one generic slot. Distinct names coexist. The same name and
value deduplicates; the same name with different values conflicts. Branch composition replaces only
the same-named property, and cross-clause `pcx!` analysis compares resolved names across the clauses'
independent IR tables.

## Examples

| Input | Result |
|---|---|
| `flex grid` | Error: two values for `display.mode` |
| `text-sm text-lg` | Error: two font sizes |
| `text-sm text-ink` | Valid: font size and color |
| `p-4 p-6` | Error: identical padding footprint |
| `p-4 px-2` | Valid: horizontal refinement |
| `px-2 p-4` | Same result as `p-4 px-2` |
| `hidden md:flex` | Valid: narrower responsive condition |
| `bg-surface hover:bg-accent` | Valid: narrower pseudo-state |
| `md:hover:bg-a hover:md:bg-b` | Error: same normalized condition |
| `shadow-sm ring-2` | Valid: effects compose semantically |
| `layer-components:flex layer-overrides:grid` | Valid: explicit cross-layer precedence |
| `layer-components:flex layer-components:grid` | Error: same slot and layer |
| `[mask-type:luminance] [text-wrap:balance]` | Valid: distinct arbitrary properties |
| `[mask-type:alpha] [mask-type:luminance]` | Error: two values for the same property |

## Conditions

A condition includes at most one viewport breakpoint, container breakpoint, theme mode, and cascade
layer, plus compatible user preferences and pseudo-states. Variant order is normalized, so spelling
does not influence output order. Layer precedence applies to normal declarations in the declared
`base < components < utilities < overrides` order; native CSS reverses that order for important
declarations.

Dependency lints are separate from conflicts. `flex-col` without a reachable `flex`, for example,
may be ineffective but is not two contradictory assignments.
