# ADR-0019: Retain structurally dead styles only by explicit policy

- Status: Accepted and implemented
- Date: 2026-07-15

## Context

ADR-0018 separates structural reachability, runtime observation, usage verdict, and removal. Under
that model a complete `StyleId` can be structurally `dead` yet still need publication for a consumer
outside the application topology: an external renderer, embedded document, third-party integration,
or migration boundary.

Changing reachability to hide that exception would corrupt application evidence. Treating an
observation hit as a retention instruction would let sampled runtime data control deletion. Keeping
all dead styles would make pruning ineffective. The exception therefore needs its own explicit,
reviewable policy axis.

Before this decision, Asset Plan schema 1 and Project Index schema 1 permitted only `all-compiled`
and `reachable-style-ids`. Reusing the latter for output that also contains policy-retained dead
styles would make those artifacts false.

## Decision

Introduce a closed retention sidecar over exact bundle-qualified entries:

```json
{
  "schemaVersion": 1,
  "universeSha256": "64-lowercase-hex-digits",
  "reachabilitySha256": "64-lowercase-hex-digits",
  "entries": [
    {
      "id": "external-email-renderer",
      "bundleId": "application",
      "styleId": "32-lowercase-hex-digits",
      "justification": "Consumed by the external mail renderer outside application routing."
    }
  ]
}
```

The document is explicit and never auto-discovered. It binds the canonical pre-pruning universe
and the exact reachability bytes. Each entry must exist in that universe and be structurally
`unreachable` across every complete origin. Reachable, mixed-origin, unknown, duplicate, stale, or
missing entries fail the build. Identifiers are stable lowercase kebab-case and justifications are
required bounded text. Wildcards, selectors, class names, routes, dates, and clock-driven expiry are
not accepted.

Retention is policy, not evidence. Usage Analysis schema 2 adds:

- `retention: not-retained | { state: "retained", entryId, justification }` per entry;
- `removalDisposition: policy-retained`; and
- `ruleSelection: reachable-or-retained-style-ids` when pruning uses the policy.

A retained entry keeps `staticReachability: unreachable` and `usageState: dead`; it is selected and
its removal disposition becomes `policy-retained`. Observation never selects output. A positive
observation that contradicts the same unreachable snapshot remains a hard error even when policy
retains the style.

Compilation uses one derived selection for both emitted output and analysis:

```text
selected = reachable StyleIds union policy-retained StyleIds
```

Selection is bundle-qualified. Retaining a shared StyleId in one bundle does not retain it in
another. Retention preserves the entire StyleId rule set and every origin; it never selects an
individual declaration, authored standard CSS rule, or theme variable.

Asset Plan schema 2, Project Index schema 2, and ownership parsing add the same
`reachable-or-retained-style-ids` selection. Modes without retention continue emitting their
existing schema-1 bytes. A bundle retained only for an external consumer remains in the artifact
ledger but is not invented into any route or island selection.

The CLI surface is `bundle --retention FILE`. It requires `--prune-unreachable`, exact
`--reachability`, and `--usage-report`. In controlled builds the raw sidecar is recorded as
`usage-retention`, affects the exact configuration ledger, and participates in the same atomic
publication/check group.

## Rejected alternatives

### Mark retained styles reachable

Rejected because policy cannot rewrite adapter-owned topology.

### Use observation hits as an allowlist

Rejected because observation is evidence about an exercised snapshot, not authority to publish or
remove CSS.

### Keep Asset Plan and Project Index schema 1

Rejected because `reachable-style-ids` would no longer describe the emitted bundle contents.

### Allow selectors, classes, or wildcards

Rejected because they are unstable or ambiguous relative to the exact bundle-qualified StyleId
universe and can silently retain more CSS than reviewed.

## Consequences

- Exceptions are visible, deterministic, hash-bound, and reviewable.
- A retention entry cannot conceal stale reachability or contradictory runtime evidence.
- Asset Plan, Project Index, ownership, control, and usage artifacts agree on one selection.
- The new schema versions are additive; existing no-retention output remains byte-identical.
- Removing an exception is reversible by rebuilding from the same inputs without that entry.

## Verification

The implemented gate covers canonical builder/parser tests, closed objects and defensive
limits, universe/reachability drift rejection, unknown/reachable/mixed-origin rejection,
bundle-qualified selection, full-StyleId preservation, unchanged `dead` verdict, contradictory
observation rejection, schema-2 Asset Plan/Project Index/ownership round trips, atomic failure,
read-only `--check`, control binding, documentation, strict Clippy, Rust 1.85, and the fixed 60 KiB
package gate.
