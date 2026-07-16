# Glossary

## Accessibility policy

A closed, versioned input that selects enforcement for contrast, motion, focus visibility, forced
colors, and input modality. It configures bounded static analysis and reviewed exceptions; it does
not represent WCAG certification or browser evidence.

## Alias

A token whose complete value comes from another token reference. PliegoCSS retains the reference
edge in `TokenGraph` even though the compiler consumes the resolved value.

## Condition

The normalized context in which an assignment applies: breakpoint, theme, user preference, and
pseudo-state. `md:hover:` and `hover:md:` describe the same condition.

## Declared contrast pair

An explicit policy relationship between one foreground and one background color, expressed as CSS
literals or canonical `color.<kebab-name>` token endpoints. PliegoCSS evaluates the relationship but
does not infer that the two colors are paired in the rendered DOM.

## Footprint

The set of semantic leaf slots touched by a utility. `p-4` touches four padding slots; `px-2`
touches only left and right.

## IR

The semantic intermediate representation produced after parsing utilities. The compiler operates on
the IR rather than on class-name strings.

## Derived token

A composite token or token property that contains one or more references but is not a complete-value
alias. Coverage follows those dependencies transitively.

## Graph hash

The SHA-256 identity of canonical `pliegocss-token-graph/1` bytes. It covers adapter source identity,
aliases, derived values, provenance, deprecations, and all validated themes; it does not replace
`ThemeId` or depend on which styles one compilation retained.

## Modifier and context

A DTCG Resolver modifier names one selection dimension such as `appearance`; each context is one
allowed value such as `light` or `dark`. One complete map of modifier contexts selects a resolution.

## Ownership sidecar

An explicit schema-1 audit input that binds exact Asset Plan bytes to a total, exclusive
bundle-to-package map and a total route-to-island composition map. It is never discovered and does
not infer relationships from Cargo metadata, filenames, or runtime behavior.

## Package ownership

Accounting responsibility for one complete Asset Plan bundle. Ownership schema 1 requires every
bundle to have exactly one package, so package budget views partition the bundle ledger. This is not
proof that a Cargo crate produced the CSS and cannot divide one bundle between packages.

## Reachability

Knowledge that a style can appear in a rendered component, route, or island. A framework adapter
supplies this structural information through a neutral sidecar; PliegoCSS validates exact source
sites and does not infer ownership from names or paths. The framework-neutral typed collector now
generates canonical sidecar bytes from adapter-supplied topology and rejects unowned visible Rust
macro sites. Wiring it to the product PliegoRS route/Cargo graph remains pending.

## Route composition

The adapter-attested union of island types that may render on one Asset Plan route. A composed route
budget merges the route's base bundles with every referenced island's bundles and deduplicates by
bundle ID. Route views may overlap and are not Control Manifest partitions.

## Semantic declaration

One canonical assignment in the typed IR. It can emit one or more physical CSS declarations, so a
schema-4 semantic declaration is not a one-to-one postprocessor rule trace.

## Physical declaration

One declaration occurrence in the exact final Lightning CSS serialization. Manifest schema 5 gives
it an artifact-local ordinal ID, exact UTF-8 ranges, a generated-support marker, its owning rule,
and every semantic or synthetic producer. It is not a stable identity across different CSS digests.

## Physical rule

One qualified rule or `@media` wrapper in the exact final stylesheet. Manifest schema 5 numbers
physical rules in depth-first preorder, records exact whole/header UTF-8 ranges, and links nested
rules to their media parent. Its ID is local to one digest-bound CSS artifact.

## Slot

A semantic destination such as `display.mode`, `padding.left`, or `typography.color`. Conflict
detection works on slots, not raw CSS property names.

## Style ID

A deterministic identifier derived from normalized style semantics, the active theme, and the
versioned identity format. SSR and client-resume flows must retain the same ID independently.

## Subject ID

A stable `sha256:` identity for one accessibility observation. Reviewed exceptions bind to its
exact check and subject ID rather than message text or source offsets.

## Theme ID

The deterministic identity of one fully resolved typed `ThemeRegistry`. Two token documents with
different aliases or provenance may share a ThemeId when all compiler-visible values are equal.

## Token

A named design-system value such as `space.4`, `color.surface`, or `radius.lg`. Themes define tokens;
utilities reference them.

## Token graph

The canonical build-time structure that retains token sources, reference edges, projections,
deprecations, resolver selections, and resolved theme views before the compiler consumes a flat
registry.

## Utility

A compact authoring instruction such as `flex`, `gap-4`, or `hover:bg-accent`. A utility expands to
one or more semantic assignments.

## Variant

A condition prefix such as `md:`, `hover:`, or `dark:`. Variants narrow when an assignment applies.

## Verification state

The evidence boundary on a finding: `verified` means the configured static rule was decided inside
its declared scope, `unverified` means available evidence was insufficient, and `manual-required`
means runtime, browser, DOM, interaction, or human evidence is necessary. None is a global
accessibility-compliance result.
