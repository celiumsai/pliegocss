# Canonical finding schema 1.0.0

Status: **implemented model plus byte-frozen JSON/human and audit SARIF 2.1.0 projections, including
bounded accessibility-policy findings; legacy process-diagnostic migration remains open**

The canonical finding is the semantic source for every PliegoCSS diagnostic surface. Human text,
JSON, and SARIF output must project the same code, severity, source, cause, evidence,
verification boundary, suggestions, and exception. A renderer may change presentation but cannot
invent or omit a finding.

This is distinct from the historical [CLI diagnostic envelope](./diagnostic-schema.md). That schema
describes process failures from current `pliego-cssc` commands. Finding schema 1.0.0 is the richer
audit/policy contract required by R0; both remain versioned until existing commands migrate.

The Rust model is exposed by `pliego_css_build::artifacts` behind the `artifacts` feature:

```rust
use pliego_css_build::artifacts::{
    Finding, FindingCause, FindingDocument, FindingSeverity, FindingTool, FindingVerification,
    parse_finding_document,
};

let cause = FindingCause::new(
    "compatibility.baseline",
    "fallback-required",
    "the selected target vector does not prove native support",
)?;
let finding = Finding::new(
    "PCSS-COMPAT-004",
    "compatibility",
    FindingSeverity::Warning,
    "feature requires a documented fallback",
    FindingVerification::Unverified,
    cause,
)?;
let document = FindingDocument::new(
    FindingTool::new("pliegocss", "0.0.0")?,
    "audit",
    vec![finding],
)?;
let json = document.to_json_pretty()?;
let reparsed = parse_finding_document(json.as_bytes())?;
assert_eq!(reparsed, document);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Document

| Field | Contract |
|---|---|
| `schemaVersion` | Exact semantic version `1.0.0`. Any field/type/meaning change requires a compatible minor addition or breaking major version. Schema 1 is closed, so unknown fields currently fail. |
| `tool` | Required lowercase-kebab tool name and nonempty version. |
| `command` | Required lowercase-kebab operation such as `audit` or `check`. |
| `findings` | Zero to 65,536 canonical findings. Source-backed findings sort by logical path and UTF-8 byte range, followed by code and fingerprint; source-less findings sort after them. Exact duplicate fingerprints fail. |

JSON is pretty-printed deterministically with one trailing newline. The complete byte vector lives in
the `finding_contract` integration target. Documents larger than 16 MiB fail before deserialization.

## Finding

| Field | Contract |
|---|---|
| `fingerprint` | `sha256:` plus 64 lowercase hex digits derived from schema, code, category, severity, verification, source/source-map, cause, sorted context, and sorted evidence. Message wording, suggestions, and reviewed exceptions do not change instance identity. Tampering fails parse. |
| `code` | Stable 3-64 character uppercase ASCII code using letters, digits, and single hyphens; for example `PCSS-A11Y-101`. Message text is never the API. |
| `category` | Lowercase kebab identifier such as `accessibility`, `compatibility`, `cascade`, `token`, or `budget`. |
| `severity` | `info`, `warning`, or `error`. The selected policy owns severity; renderers do not. |
| `message` | Human-readable summary without control characters. |
| `verification` | `verified`, `unverified`, or `manual-required`. Automated accessibility output must never collapse these into a WCAG-compliance claim. |
| `deterministic` | Required `true` in schema 1.0.0. Nondeterministic model output cannot enter this contract. |
| `source` | Exact authored location or null. Paths are portable, relative, `/`-separated logical paths. Byte ranges are zero-based, half-open UTF-8 ranges. Line/column coordinates are one-based Unicode-scalar positions and are either all present or all null. |
| `cause` | Required `policy`, semantic `rule`, and explanation. This is why the finding exists, not a repetition of its message. |
| `context` | Deterministically sorted string map for dimensions such as theme, selector, route, component, or target profile. Accessibility observations use the exact key `subject-id` for reviewed exception identity. |
| `evidence` | Sorted, unique typed facts with `kind`, `name`, string `value`, optional `unit`, and deterministic `source`. Up to 1,024 entries. Browser traces must identify a browser source rather than masquerading as static proof. |
| `suggestions` | Up to 256 ranked suggestions. Ranks are unique and contiguous from one. Every suggestion has stable ID, message, `low|medium|high` risk, scope, and sorted prerequisites. A suggestion is not authorization to write. |
| `exception` | Optional reviewed ID, justification, and optional canonical `YYYY-MM-DD` expiry date. Exceptions do not erase the finding or evidence. |

`source.sourceMap`, when present, links the authored range to one exact generated logical file and
half-open UTF-8 byte range. It does not replace the authored span.

## Fingerprint boundary

The fingerprint groups the same semantic finding even if prose improves or ranked remediation
changes. The complete finding document still receives its own SHA-256 in the future manifest and
receipt, so message, suggestion, or exception changes remain integrity-visible.

Fingerprint input uses canonical JSON field order and the sorted map/vector contracts above. A
consumer must call `parse_finding_document`; trusting the stored fingerprint without recomputing it
is unsupported.

## Human, JSON, and SARIF parity

`FindingDocument::to_human` renders directly from the validated model. It includes severity/code,
message, source, policy/rule cause, verification, all evidence, ranked suggestion risk, and any
exception. Schema 1 therefore has executable proof that JSON and human output share semantic input.

`pliego-cssc audit --format sarif` emits SARIF 2.1.0 from the same `FindingDocument`. Its `ruleId`,
`level`, message, byte/line location, generated source-map related location, and partial fingerprint
derive from the finding. `properties.pliegoCssFinding` contains the complete canonical finding, so a
regression test compares it semantically with JSON. SARIF-only findings, omitted fields, and
severity rewriting are forbidden.

The `$schema` URI is the official
[OASIS SARIF 2.1.0 Plus Errata 01 schema](https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json).

## Accessibility projection

`pliego-cssc audit --accessibility-policy FILE` projects the bounded static accessibility analyzer
through this unchanged schema in human, JSON, and SARIF. The stable code families are:

| Family | Contract |
|---|---|
| `PCSS-A11Y-000` | Summary of the configured static gate only. |
| `PCSS-A11Y-100` / `101` / `102` / `108` | Declared contrast pass, violation, reviewed exception, or unverified/manual result. |
| `PCSS-A11Y-200` / `201` / `202` / `208` / `209` | Motion pass, violation, reviewed exception, unverified, or manual-required result. |
| `PCSS-A11Y-300` / `301` / `302` / `308` / `309` | Reserved focus pass, explicit suppression violation, reviewed exception, unverified, or manual-required result. Schema 1 does not emit `300` without computed-cascade proof. |
| `PCSS-A11Y-400` / `401` / `402` / `409` | Forced-colors pass, violation, reviewed exception, or unverified/manual-required result. |
| `PCSS-A11Y-500` / `501` / `502` / `508` / `509` | Input-modality pass, violation, reviewed exception, unverified, or manual-required result. |
| `PCSS-A11Y-999` | A configured exception matched no current non-pass observation. |

Every non-summary accessibility observation carries `context.subject-id` as a canonical prefixed
SHA-256. Contrast subjects are stable over check, pair ID, and canonical resolver selections. CSS
subjects are stable over check, logical stylesheet, canonical selector, and check family. Policy
authors copy that exact value into an exception's `subjectId`; alternate spellings such as
`a11y-subject-id` are not part of the contract.

Contrast findings point to the relevant pair ID inside the accessibility-policy file. CSS-AST
findings point to the canonical selector range in the audited stylesheet. `verified` covers only a
proved pass or violation inside the declared static boundary; unresolved evidence stays
`unverified`, and runtime/dynamic evidence stays `manual-required`. A reviewed matching exception
changes the finding to the check's `x02` code, attaches the review record, preserves evidence and
verification, and prevents only that observation from failing. It never erases the finding.

## Limits and non-claims

- This contract does not apply a fix, define a change budget, or authorize writes. Plans and receipts
  are separate versioned artifacts.
- `verified` means verified inside the recorded analysis boundary, not globally correct CSS or WCAG
  certification.
- A missing source is allowed for process/tool findings but is insufficient for a source policy that
  promises exact spans.
- Logical paths deliberately reject absolute Windows/POSIX paths, backslashes, drive prefixes,
  empty segments, `.` and `..` so identical projects can produce identical documents cross-OS.
