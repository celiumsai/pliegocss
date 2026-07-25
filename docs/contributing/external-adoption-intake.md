# External adoption intake and evidence custody

Status: **G7 operating protocol**

This workflow collects external evidence without publishing personal data or proprietary project
source. It implements
[`external-adoption-v1/authority.json`](../../benchmarks/external-adoption-v1/authority.json) and
the [0.1.x adapter support policy](../reference/adapter-support-policy-0.1.x.md).

## Before any interview or pilot

1. Confirm that the participant controls or maintains a project outside Celiums Solutions,
   PliegoCSS, and the `celiumsai` repository owner.
2. Explain the research use, the public redacted record, and private evidence custody.
3. Record affirmative consent for both research use and publication of the redacted record.
4. Assign pseudonymous participant, organization, project, and evidence IDs. Do not put names,
   email addresses, access tokens, private repository URLs, or raw source in the public ledger.
5. Store the raw evidence in approved private custody and calculate its SHA-256 digest.

Consent may be withdrawn before promotion. Remove the public record, rotate the ledger hash in the
authority, and re-run the gate. A deleted or withdrawn record stops counting immediately.

## Interview record

One reviewed participant may satisfy one interview. Capture:

- time, external relationship, organization pseudonym, consent and redaction review;
- stable digest and private custody reference for the source notes/transcript;
- concise findings, linked incident IDs, and linked pilot IDs;
- a second-party reviewer ID, review time, and `approved` status.

Ten qualified participants are the minimum; fifteen is the planned sampling target. Duplicate
participant IDs fail the authority.

## Incident record

An incident must have occurred in an external project. Capture framework, category, severity,
provenance, expected diagnosis, allowed ambiguity, consent/redaction state, raw-evidence digest, and
review. Set `synthetic` to `false` and provenance to `external-project-observation`.

Synthetic fixtures remain useful for conformance but never count toward the required 20 incidents.

## Pilot record

A qualified pilot must:

- use static HTML, Vite 8.1.5, or the pinned PliegoRS rendered-output contract;
- use the certified Tailwind 3.4.19 or 4.3.3 profile;
- finish the bounded adoption protocol;
- pass DOM/ARIA/computed-style/layout comparison;
- prove exact rollback;
- end with zero blocking failures;
- bind an HTTPS evidence/repository reference, exact 40-character source commit, and SHA-256 digest;
- receive approved review and consent for the public redacted record.

G7 requires three to five completed pilots, at least three unique projects, and at least three
independent organizations.

## Publishing a reviewed record

1. Add only the redacted record to `interviews.json`, `incidents.json`, or `pilots.json`.
2. Recompute that ledger's SHA-256 and update `authority.json`.
3. Run `pnpm check:external-adoption-authority`.
4. Run `node scripts/render-external-adoption.mjs --write`.
5. Review the diff for personal data and unsupported claims.
6. Run `pnpm check:external-adoption`. It must remain blocked until all thresholds are real.

Raw private material belongs under the private custody system, never in the Git worktree.
