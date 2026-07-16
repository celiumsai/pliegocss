# ADR-0012: Freeze Baseline compatibility as a versioned policy

## Status

Accepted starting with compatibility policy schema 1 / policy version 1. The current wire contract
is schema 2 / policy version 7.

## Context

PliegoCSS needs a browser-support claim that is useful to applications and reproducible across
machines. A floating browser query makes output depend on time, installed data, environment, or
network state. Conversely, passing every parsed CSS literal to a postprocessor can silently emit
syntax outside the claimed support matrix because parsing is not a compatibility proof.

The project also must interoperate with host applications without imposing a reset or pretending
that framework-specific component scope exists in native CSS output.

## Decision

Add explicit `baseline-widely`, `modern`, and `none` compatibility profiles. Policy schema 1 and
policy version 1 froze the WebDX Baseline Widely Available browser mapping observed on 2026-07-14:
Chrome/Edge 120, Firefox 121, and Safari/iOS Safari 17.2.

`pliego-cssc compatibility --targets PROFILE` emits the complete deterministic decision artifact.
Every feature receives a capability tier and `allow`, `transform`, `warn`, or `error` action.
Lightning CSS receives the exact frozen browser vector; it does not define PliegoCSS semantics.

Policy version 2 classified typed ARIA/data and direction selectors. Policy version 3 classifies
typed inline-size container establishment and `cq-<theme-breakpoint>` conditions as native CSS,
without changing the Baseline browser snapshot. Policy version 4 classifies configurable
`aria-[name=value]`, `data-[name]`, and `data-[name=value]` variants after the compiler proves their
bounded canonical attribute-selector grammar.

Policy version 5 classifies `writing-horizontal`, `writing-vertical-lr`, and
`writing-vertical-rl` as compiler-validated native properties without changing the Baseline
browser snapshot.

Policy version 6 classifies the fixed `pliego.base`, `pliego.components`, `pliego.utilities`, and
`pliego.overrides` cascade-layer graph as compiler-validated native CSS. Layer choice is represented
in semantic IR; unprefixed styles remain unlayered and no reset is introduced.

Policy version 7 moves the wire contract to schema 2 and records the exact
`web-features@3.32.0` and `baseline-browser-mapping@2.10.43` package artifacts, npm integrity,
content/result hashes, capture date, and fixed-date mapping query. This is a deliberate schema
change because strict schema-1 consumers reject unknown provenance fields. It does not change the
browser vector or earlier compiler feature actions.

The strict Baseline profile fails closed on arbitrary values, properties, and selectors because the
compiler cannot prove their compatibility from typed IR. `--targets none` is the explicit unmanaged
escape hatch. `modern` remains the default only to preserve existing candidate output.

Reset profile `none` and scope profile `standard-class` are explicit. PliegoCSS emits no implicit
reset, and the host owns stylesheet placement. Explicit typed layers do not create selector
isolation. Component scope remains an error until represented and verified as a semantic IR
dimension.

## Consequences

- Identical profile and source inputs produce the same target vector and policy bytes over time.
- Strict compatibility errors occur before grouped output publication and preserve CMP codes in
  human and JSON diagnostics.
- Updating the moving WebDX mapping requires a policy version and reviewed evidence; it cannot
  arrive through dependency or host-data drift.
- Arbitrary CSS remains available through `modern` for candidate compatibility and through `none`
  as an explicit handoff, but it cannot inherit a Baseline guarantee silently.
- Host frameworks do not receive hidden reset, scope, or cascade assumptions.
- Lightning CSS transformations remain replaceable implementation machinery behind a PliegoCSS
  policy contract.

## Rejected alternatives

### Resolve Baseline or Browserslist during each build

Rejected because output would drift with time, data packages, environment, or network access.

### Treat successful parsing as compatibility

Rejected because a parser accepting syntax does not prove support or a safe transform for the
selected browsers.

### Reject arbitrary CSS in every profile

Rejected because interoperability and downstream-owned compatibility are product requirements.
The strict guarantee rejects it; `none` makes the transfer of responsibility explicit.

### Inject a reset by default

Rejected because resets affect global cascade, payload, rendering, and framework ownership. They
must be explicit application imports.

See [compatibility policy schema 2](../reference/compatibility-policy.md).
