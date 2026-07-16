# Repair proposal, plan, dry-run, and Change Receipt schemas 1.0.0

Status: **implemented bounded apply boundary**. `pliego-cssc plan` converts an agent-authored
proposal into a deterministic, integrity-bound plan. `pliego-cssc fix --dry-run` verifies that plan
without mutation. `pliego-cssc fix --apply` requires an external exact-plan authorization, publishes
the complete source transition plus an adjacent Change Receipt with rollback, and makes no claim
that required checks or browser evidence have run. The separate `pliego-css-agent verify` command can
subsequently execute the closed built-in check policy documented below without rewriting this receipt.

## Commands

```console
pliego-cssc plan \
  --findings findings.json \
  --proposal proposal.json \
  --source-root . \
  --format json > pliego.css.plan.json

pliego-cssc fix \
  --plan pliego.css.plan.json \
  --findings findings.json \
  --source-root . \
  --dry-run \
  --format json

pliego-cssc fix \
  --plan pliego.css.plan.json \
  --findings findings.json \
  --source-root . \
  --apply \
  --authorize sha256:<planSha256> \
  --receipt pliego.css.change-receipt.json \
  --format json
```

`plan` writes its result to stdout. JSON is the default; `--format text` prints the bounded summary
and exact byte-edit patch. `fix` requires exactly one of `--dry-run` or `--apply`; text is its default
and JSON is available explicitly. Apply additionally requires both `--authorize` and `--receipt`.
Unknown, duplicate, conflicting, or incomplete options fail closed.

All three input documents are closed JSON objects: unknown fields are errors. Documents and patch
text are bounded to 16 MiB. Source paths are portable project-relative paths; absolute paths,
prefixes, empty segments, `.`/`..`, backslashes, control characters, symbolic links, junctions,
reparse points, and escapes from `--source-root` are rejected. Each source must be a regular UTF-8
file of at most 16 MiB.

The receipt destination is relative to the process working directory. Its parent directories must
already exist, must not traverse link-like components, and the final path cannot alias the plan,
FindingDocument, a repair source, or the persistent `.pliegocss-repair.lock` coordination file.

## Repair proposal

The proposal is the only agent-authored input. It names exact edits rather than free-form
instructions:

```json
{
  "schemaVersion": "1.0.0",
  "findingDocumentSha256": "<64 lowercase hex characters>",
  "changeBudget": {
    "maxFiles": 1,
    "maxEdits": 1,
    "maxInsertedBytes": 4,
    "maxRemovedBytes": 4
  },
  "requiredChecks": ["audit"],
  "edits": [
    {
      "findingFingerprint": "<FindingDocument fingerprint>",
      "suggestionId": "replace-color",
      "file": "src/app.css",
      "byteStart": 17,
      "byteEnd": 21,
      "replacement": "#111"
    }
  ]
}
```

| Field | Contract |
|---|---|
| `schemaVersion` | Exactly `1.0.0`. |
| `findingDocumentSha256` | SHA-256 of the exact `--findings` bytes, including formatting and final newline. |
| `changeBudget` | Non-zero file/edit limits, each at most 65,535; inserted/removed limits at most 64 MiB. |
| `requiredChecks` | Non-empty dotted lowercase kebab identifiers, sorted and unique. Every selected suggestion prerequisite must appear here. |
| `edits` | Between 1 and 65,535 exact UTF-8 byte edits in canonical file/range/fingerprint order. |

`byteStart` is inclusive and `byteEnd` is exclusive. Edits in one file may not overlap. Each
replacement is at most 1 MiB and cannot contain NUL. The plan builder requires each range to fall
completely inside the exact source range attached to its finding.

## Authority boundary

An edit is admitted only when all of these statements are true:

- its fingerprint resolves to exactly one finding in the bound FindingDocument;
- the finding is `verified` and has no active exception;
- `suggestionId` resolves to a ranked suggestion whose risk is exactly `low`;
- proposal file and range are contained by that finding's exact source span;
- the current bytes at that range are valid UTF-8 and remain inside the bound source snapshot;
- all suggestion prerequisites are present in `requiredChecks`;
- the total files, edits, inserted bytes, and removed bytes stay within `changeBudget`.

Medium/high-risk, unverified, excepted, ambiguous, out-of-range, overlapping, stale, or
over-budget edits fail the complete plan. Schema 1.0.0 does not partially accept a proposal.

## Repair plan

The generated plan is a closed canonical document:

| Field | Contract |
|---|---|
| `schemaVersion` | Exactly `1.0.0`. |
| `planSha256` | SHA-256 of the canonical compact JSON payload containing every other plan field, in schema order. |
| `mode` | Exactly `dry-run-only`: the plan carries no embedded permission to mutate. An external exact-plan token is still required by apply. |
| `tool` | Validated tool `name` and `version`. |
| `findingDocument` | Portable logical `file`, exact byte count, and exact SHA-256. |
| `changeBudget` | The accepted proposal limits. |
| `changeSummary` | Derived `files`, `edits`, `insertedBytes`, and `removedBytes`; never trusted from the proposal. |
| `risk` | Exactly `low`. |
| `requiredChecks` | Canonical required-check set inherited from the proposal. |
| `sources` | One before/after byte count and SHA-256 transition per edited file, sorted by path. |
| `edits` | Canonical exact edits enriched with finding code, suggestion rank/scope, removed bytes, and content hashes. |
| `patch` | Exact deterministic projection of `edits`, with format, text, and SHA-256. |

Each planned edit contains:

```json
{
  "findingFingerprint": "<fingerprint>",
  "findingCode": "A11Y001",
  "suggestionId": "replace-color",
  "suggestionRank": 1,
  "suggestionScope": "declaration",
  "file": "src/app.css",
  "byteStart": 17,
  "byteEnd": 21,
  "removed": "#777",
  "removedSha256": "<sha256>",
  "replacement": "#111",
  "replacementSha256": "<sha256>"
}
```

`sources` exactly covers the edited files. Its after hash is derived by applying the ordered edits
in memory to the bound before snapshot. Parsing a plan re-derives summaries, source coverage, patch
text, patch hash, and `planSha256`; any mismatch is an error.

### Patch format

`patch.format` is `pliegocss-byte-edits/1`. It is an exact, deterministic review projection that
contains logical path, byte range, finding/suggestion identity, JSON-escaped removed bytes, and
JSON-escaped replacement bytes. It is not a unified diff and must not be passed to `patch` or `git
apply`. The structured `edits` array is authoritative.

`patch.sha256` binds the exact UTF-8 patch text. `planSha256` binds the complete canonical payload,
including that patch. These hashes provide drift and integrity detection, not authentication: a
party able to rewrite the plan can recompute them. The plan itself remains `dry-run-only`; apply
requires the external literal `sha256:<planSha256>`. That token prevents accidental or ambiguous
plan selection, but it is not a signature, identity assertion, trusted issuer, or human approval
protocol.

## Dry-run report

`fix --dry-run` reparses the exact FindingDocument named and hashed by the plan, reads exactly the
listed source files, and returns:

```json
{
  "schemaVersion": "1.0.0",
  "planSha256": "<sha256>",
  "state": "ready",
  "files": 1,
  "edits": 1,
  "patchSha256": "<sha256>"
}
```

| State | Meaning |
|---|---|
| `ready` | Every current source equals its complete before snapshot, and applying edits in memory reproduces every after hash. |
| `already-applied` | Every current source equals its complete after snapshot. This is idempotence recognition, not proof of who applied it. |

A source matching neither snapshot is stale. A mix of before and after snapshots is partially
applied. Both are errors; there is no successful `stale` or mixed report state. Missing or extra
source snapshots, FindingDocument path/byte/hash drift, invalid UTF-8, and plan drift also fail.

The dry run does not execute `requiredChecks`. It also does not claim browser evidence,
computed-style evidence, successful deployment, or semantic correctness beyond the verified
FindingDocument and exact byte contracts.

## Authorized apply and Change Receipt

`fix --apply` repeats all dry-run verification and accepts only the literal authorization
`sha256:<planSha256>`. A wrong token, stale/mixed source state, drifted FindingDocument, unsafe path,
or destination collision fails before publication. Plan content cannot supply a command or grant
its own authority.

The publisher acquires `.pliegocss-repair.lock` under `--source-root`, then re-inspects and re-reads
every source while holding that persistent cooperative lock. It stages exact after bytes beside each
destination, preserves source permissions, backs up existing destinations, and publishes sources and
receipt as one rollback-capable group. If a later publication step fails, earlier destinations are
restored. This is grouped publication with rollback, not a claim of a filesystem-wide transaction
against writers that ignore the lock.

The adjacent closed Change Receipt is schema `1.0.0`:

```json
{
  "schemaVersion": "1.0.0",
  "receiptSha256": "<sha256 of the canonical receipt payload>",
  "planSha256": "<authorized plan hash>",
  "patchSha256": "<bound patch hash>",
  "authorization": "sha256:<planSha256>",
  "state": "applied",
  "result": "checks-pending",
  "findingDocument": {
    "file": "findings.json",
    "bytes": 1234,
    "sha256": "<sha256>"
  },
  "changeSummary": {
    "files": 1,
    "edits": 1,
    "insertedBytes": 4,
    "removedBytes": 4
  },
  "sources": [
    {
      "file": "src/app.css",
      "beforeBytes": 25,
      "beforeSha256": "<sha256>",
      "afterBytes": 25,
      "afterSha256": "<sha256>"
    }
  ],
  "checks": [{ "id": "audit", "status": "not-run" }],
  "browserEvidence": "not-collected"
}
```

| Field | Contract |
|---|---|
| `receiptSha256` | SHA-256 of the canonical compact payload excluding the self-hash. Parsing re-derives it. |
| `planSha256` / `patchSha256` | Exact authorized plan and deterministic patch identities. |
| `authorization` | Exactly `sha256:<planSha256>`; selection guard only, not authentication. |
| `state` | `applied` when before bytes were replaced; `already-applied` when every source already matched the complete after state. |
| `result` | Exactly `checks-pending` in schema 1.0.0. It is not `passed`. |
| `findingDocument` / `sources` / `changeSummary` | Exact identities and counts inherited from the verified plan. |
| `checks` | Complete canonical `requiredChecks`, each exactly `not-run`. |
| `browserEvidence` | Exactly `not-collected`. |

Receipt JSON uses deterministic two-space formatting with one final LF. Parsing rejects alternate
whitespace or key formatting even when the semantic payload and self-hash would otherwise validate.

When an existing receipt for the same plan is found and every source is already applied, the exact
receipt bytes are preserved and no second patch occurs. A receipt for a different plan fails. If the
same receipt exists but sources returned to the before state, apply fails rather than silently
reusing or overwriting evidence.

## Explicit non-goals of schema 1.0.0

- no cryptographic signer or human approval protocol;
- no command execution from proposal or plan content;
- no best-effort repair, fuzzy range recovery, or partial success;
- no check execution inside apply or mutation of the Change Receipt after publication;
- no generated browser evidence;
- no inference that a low-risk edit is globally safe outside its bounded finding and checks.

Those omissions are release gates, not implied behavior. The separate
[repair verification boundary](./repair-verification-schema.md) executes schema-1
closed checks and validates fixed Rust/PliegoRS Chromium evidence to derive
`passed|failed|blocked` in a new artifact without rewriting this change-only receipt. Hosted,
signed, and multi-browser evidence remain open.
