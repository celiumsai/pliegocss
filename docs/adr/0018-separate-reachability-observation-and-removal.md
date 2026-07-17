# ADR-0018: Separate reachability, observation, usage, and removal

- Status: Accepted and implemented
- Date: 2026-07-15

## Context

PliegoCSS can already prove that a complete `StyleId` has no origin reachable from the roots in an
adapter-attested reachability document. The opt-in `--prune-unreachable` path uses that proof to
exclude the complete rule set from generated output. It cannot prove that a browser, test, route,
viewport, theme, or interaction state exercised a style.

Those are different facts. Treating a statically reachable style as observed would invent runtime
evidence. Treating a style absent from sampled observation as dead would create unsafe deletions.
The existing terms also have narrower meanings elsewhere: Control Manifest uses
`measured|unavailable` for analyzer availability, accessibility findings use
`verified|unverified|manual-required`, and budget coverage uses an observed policy subject to mean a
matched measurement. None of those states is a usage verdict.

The product contract requires PliegoCSS to distinguish `unobserved` from `dead`, retain exact
evidence, and make removal conservative. Existing manifest, Asset Plan, Project Index, finding, and
control schemas must not be silently reinterpreted to claim evidence they do not carry.

## Decision

Add a separate, versioned usage-analysis contract over one complete bundle-qualified `StyleId`
universe. Every entry carries four independent axes:

1. `staticReachability`: `reachable`, `unreachable`, or `unknown`;
2. `observationState`: `observed`, `unobserved`, or `unavailable`;
3. `usageState`: `observed`, `unobserved`, `dead`, or `unknown`; and
4. `removalDisposition`: `retain`, `blocked`, `candidate`, or `removed`.

The analysis unit in schema 1 is exactly `(bundleId, StyleId)`. Selectors, individual
declarations, authored standard CSS, and theme custom properties are outside the removal unit.

The derivation is fail closed:

| Static evidence | Observation evidence | Usage verdict | Removal disposition |
|---|---|---|---|
| reachable | positive exact hit | observed | retain |
| reachable | collected, no hit | unobserved | retain |
| reachable | not collected | unknown | retain |
| unreachable under complete exact coverage | no positive hit | dead | candidate, or removed only by explicit pruning |
| unknown | positive exact hit | observed | retain |
| unknown | collected, no hit | unobserved | retain |
| unknown | not collected | unknown | blocked |

An exact positive observation that names an entry proven statically unreachable in the same bound
snapshot is contradictory evidence. Analysis fails instead of preferring either producer.

Observation is an explicit sidecar, never discovered. It binds the canonical universe hash and the
exact reachability digest, identifies its producer and declared coverage, records tested contexts
and unknown dynamic inputs, and lists only positive bundle-qualified hits. Absence from a sampled or
producer-attested snapshot yields `unobserved`, never `dead` by itself. Wall-clock time is metadata,
not a semantic input.

`dead` is a snapshot-scoped structural verdict: complete compiler origins are owned by the complete
adapter graph and every origin belongs only to components outside all route/island roots. A
`StyleId` shared by several origins is retained when any origin is reachable. A stale, unowned, or
partial origin fails the complete analysis rather than becoming `unobserved`.

`candidate` is report-only. `removed` means excluded from generated CSS by the already explicit
`--prune-unreachable` operation; it never means authored source was edited. Source mutation,
declaration-level disposition, per-token usage verdicts, and generic repair receipts remain outside
this schema. Theme-variable emission may be filtered downstream from the retained StyleId set
without changing this report's removal unit.

## Rejected alternatives

### Rename static reachability to observation

Rejected because route topology is not runtime, browser, test, or telemetry evidence.

### Infer deadness from absence in tests or telemetry

Rejected because finite routes, states, viewports, themes, authentication conditions, and dynamic
class construction cannot prove global absence.

### Extend manifest schema 5 or Project Index schema 1 in place

Rejected because those frozen contracts describe emitted provenance. A pruned manifest omits the
very complete `StyleId` universe required to explain removal.

### Make individual declarations or theme variables usage-analysis units

Rejected for schema 1 because shared identities, cascade behavior, fallbacks, and global theme
contracts require a different proof boundary. A later compiler optimization can omit
variable-backed tokens with no consumer among already retained complete styles without claiming a
per-token observation verdict here.

## Consequences

- Bundle analysis must retain an all-compiled universe even when the selected CSS output is pruned.
- Reports can state exactly what is reachable, what was actually observed, and what remains unknown.
- A no-observation run is still useful: it can prove structural deadness but cannot invent observed
  or unobserved runtime states.
- Observation producers must regenerate their sidecar after universe or reachability drift.
- Generated-output pruning stays explicit and reversible by rebuilding without the flag.
- The explicit retention policy in ADR-0019 can keep a structurally dead entry without changing its
  evidence or relabeling it as observed.

## Verification

The implemented gate covers closed-schema and defensive-limit tests, canonical byte identity,
universe and reachability hash drift rejection, positive-hit validation, contradictory-evidence
rejection, shared-origin OR reachability, all-unreachable deadness, no-evidence unknown states,
sampled-absence retention, exact selected/removed agreement, `--check` immutability, control
manifest/receipt binding, documentation, strict Clippy, Rust 1.85, and package replay.
