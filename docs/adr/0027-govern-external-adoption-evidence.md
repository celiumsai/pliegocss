# ADR-0027: Govern G7 with redacted, hash-bound external evidence

- Status: Accepted for the G7 evidence architecture
- Date: 2026-07-25

## Context

Owned fixtures can prove deterministic behavior but cannot prove independent adoption. G7 requires
10–15 external interviews, 20 real incidents, and 3–5 pilots across at least three independent
projects. Raw interviews and private project snapshots may contain personal or proprietary data, so
committing them would be unsafe. Counting unaudited summaries would be unverifiable.

## Decision

G7 uses a schema-1 external-adoption authority with three hash-bound public ledgers. Every admitted
record is pseudonymous, consented, redacted, reviewed, and bound to its private source by SHA-256.
The source remains in private custody. Owned projects, Celiums/PliegoCSS organizations,
`celiumsai` repositories, synthetic incidents, duplicate identities, incomplete pilots, and
unreviewed records fail closed.

Recruitment uses a public, no-private-data cohort notice while consent and evidence intake happen
through private correspondence. A public issue or email is only an interest signal; it is not
consent and never counts as evidence. Interviews, incidents, and pilots all require an explicit
reviewed-redaction record.

The 0.1.x adapter support policy is frozen alongside the authority. It certifies only complete
literal class groups in static HTML, Vite 8.1.5, and pinned PliegoRS rendered output against
Tailwind CSS 3.4.19 and 4.3.3. Tailwind remains loaded during coexistence. Dynamic classes,
arbitrary configs/plugins, Tailwind removal, and other versions are outside the claim.

The authority validator succeeds for an honest blocked state. The promotion gate is a separate
command and fails until all external thresholds are satisfied.

## Consequences

- G7 progress is machine-countable without turning private research material into public source.
- The public cohort can recruit participants without becoming a shadow evidence ledger.
- A support-policy change invalidates the authority hash and requires explicit review.
- Fixtures and synthetic cases remain useful but can never inflate adoption counts.
- The repository cannot complete G7 by itself; qualified external participation is a real input.

## Verification

```console
pnpm check:external-adoption-authority
pnpm check:external-adoption
```
