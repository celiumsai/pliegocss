# Repair authority conformance corpus

Status: **16-case synthetic boundary corpus implemented; real-incident and agent-turn evidence is
still missing**

The tracked `benchmarks/repair-corpus/cases.json` corpus protects the schema-1 repair authority
boundary from silent widening. It is engineered from the public repair contract; it is not user
research, a production incident set, a precision/recall sample, or evidence that an agent needs
fewer turns.

Run the gate from the repository root:

```console
pnpm check:repair-corpus
```

The Node harness validates the closed corpus shape, provenance declaration, sorted unique case IDs,
and explicit claim boundary. It then runs the data-driven Rust integration test and requires one
accepted control plus fifteen fail-closed rejections. The report includes the exact corpus SHA-256,
case categories, expected/actual outcome, and reason-match state.

The configured CI workspace matrix executes the Rust test on Windows, Linux, and macOS under Rust
1.85 and 1.96. Its format/evidence job also runs the Node wrapper under Rust 1.96. Configuration is
not hosted evidence: the matrix must still complete on the eventual remote release commit.

## Covered behavior

The accepted control proves that one verified, unexcepted, low-risk exact edit produces identical
canonical plans on repeated construction. Negative cases cover:

- inserted and removed byte budgets;
- reviewed exceptions, unverified findings, and medium-risk suggestions;
- missing prerequisites, unknown findings, and unknown suggestions;
- FindingDocument hash drift and source-set drift;
- non-UTF-8 input, unsafe paths, and source-range escape;
- overlapping edits and unknown proposal fields.

Every negative case must reach its intended contract layer. For example, the unknown-finding case
uses a syntactically valid `sha256:` fingerprint so it tests semantic resolution rather than being
rejected earlier as malformed input.

## Frozen provenance and claim boundary

The corpus declares:

| Field | Value |
|---|---|
| `kind` | `synthetic` |
| `source` | `engineered from the public repair contract` |
| `consent` | `not-applicable` |
| `redaction` | `not-required` |
| `claimBoundary` | `conformance-only-not-real-incident-or-agent-turn-evidence` |

Changing a case, expectation, provenance field, or claim boundary changes the tracked corpus hash
and requires review. The harness deliberately does not accept free-form commands, generated cases,
or an external expected-result override.

## Evidence still required

R1 remains open until the same fixed model/agent is evaluated against a reviewed frozen corpus and
the result demonstrates fewer turns without increasing post-repair violations. The strategic R0
research gate separately requires 20 anonymized real CSS incidents with provenance,
consent/redaction state, expected diagnosis, allowed ambiguity, and stable hashes. Those datasets
must not be synthesized or inferred from this conformance suite.
