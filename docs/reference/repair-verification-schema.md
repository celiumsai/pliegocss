# Repair check policy and Verification Receipt schemas 1.4.0

Status: **implemented bounded built-in verification plus fixed-profile Rust and browser evidence**. The
`pliego-css-agent verify` process reads a canonical Change Receipt, executes only in-process check
kinds compiled into PliegoCSS, validates exact evidence from the separate runner, and publishes a
canonical Verification Receipt. Schema 1.4 supports `standard-css-audit`,
`token-graph-integrity`, `css-budget-audit`, `test-suite-evidence`, and `browser-evidence`. Neither a plan nor a policy
can provide a program, shell, package script, build script, environment override, or argument
vector. Canonical 1.0.0, 1.1.0, 1.2.0, and 1.3.0 documents remain readable under their original kind limits.

## Command

```console
pliego-css-agent verify \
  --change-receipt pliego.css.change-receipt.json \
  --check-policy pliego.css.check-policy.json \
  --source-root . \
  --receipt pliego.css.verification-receipt.json \
  --format json
```

The explicit test boundary runs one compiled-in profile and publishes its canonical schema-1
evidence before the check policy is finalized:

```console
pliego-css-agent run-tests \
  --change-receipt pliego.css.change-receipt.json \
  --source-root . \
  --check-id tests.workspace \
  --evidence pliego.css.test-evidence.json
```

`run-tests` executes exactly `cargo test --workspace --all-targets --locked --offline`, forces an
isolated `target/pliego-css-agent-tests` target directory, and accepts no command or argument
extension. It publishes valid `passed` or `failed` evidence. Invocation/path/process failures exit
`2` without evidence; a completed nonzero Cargo exit publishes failed evidence and exits `1`.

The explicit browser boundary likewise exposes one compiled-in profile:

```console
pliego-css-agent run-browser \
  --change-receipt pliego.css.change-receipt.json \
  --source-root . \
  --check-id browser.pliegors \
  --evidence pliego.css.browser-evidence.json
```

`run-browser` invokes exactly `node scripts/check-pliegors-browser.mjs` in its private closed-report
mode. It accepts no script, URL, selector, assertion, browser argument, or shell extension. The
profile rebuilds the pinned PliegoRS fixture, launches local headless Chromium through CDP, checks
the visit-counter replay, and publishes valid `passed` or completed `failed` evidence. Early build,
launch, protocol, path, drift, or malformed-output failures exit `2` without evidence.

All artifact paths are project-relative and reject `.`, `..`, absolute paths, non-UTF-8 components,
symbolic links, junctions, reparse points, and aliases with changed sources or supplementary inputs.
The resolved `--source-root` must be a regular non-link directory. Documents, individual sources,
and supplementary budget/test/browser artifacts are capped at 16 MiB.

Exit status is `0` only for `result: passed`, `1` for a valid published `failed` or `blocked` receipt,
and `2` for invalid invocation, drift, unsafe paths, collision, or tool failure. JSON is emitted to
stdout and the required receipt destination; text is the default projection. Every executed check
also publishes its complete canonical `FindingDocument` beside the receipt.

## Check policy

The closed canonical policy is deterministic two-space JSON with one final LF:

```json
{
  "schemaVersion": "1.4.0",
  "checks": [
    {
      "id": "audit",
      "kind": "standard-css-audit",
      "source": "src/app.css",
      "compatibilityProfile": "baseline-widely"
    }
  ],
  "browserEvidence": "not-required"
}
```

| Field | Contract |
|---|---|
| `schemaVersion` | `1.4.0` for new evidence; canonical legacy `1.0.0`, `1.1.0`, `1.2.0`, and `1.3.0` remain readable under their original kind limits. |
| `checks` | 1–256 definitions sorted uniquely by dotted lowercase-kebab `id`; IDs must exactly equal the Change Receipt required-check set. |
| `kind` | `standard-css-audit`, `token-graph-integrity`, `css-budget-audit`, `test-suite-evidence`, or `browser-evidence`. No external-command kind exists. |
| `source` | Portable logical path naming one exact changed source in the Change Receipt. Every changed source must be covered by at least one definition. |
| `compatibilityProfile` | Standard CSS and CSS budget audits require `modern|baseline-widely`; token-graph, test-suite, and browser evidence require `none`. |
| `budgetPolicy` | Required only by `css-budget-audit`: exact portable file, nonzero byte count, and lowercase SHA-256 identity. |
| `budgetSubjects` | Optional sorted unique `package|route` ownership subjects for `css-budget-audit`, maximum 256. File and layer subjects are measured automatically. |
| `testEvidence` | Required only by `test-suite-evidence`: exact canonical runner-evidence file, nonzero byte count, and lowercase SHA-256 identity. |
| `workspaceManifest` | Required only by `test-suite-evidence`; must identify exact nonempty root `Cargo.toml`. |
| `lockfile` | Required only by `test-suite-evidence`; must identify exact nonempty root `Cargo.lock`. |
| `browserEvidenceFile` | Required only by `browser-evidence`: exact canonical runner-evidence file, nonzero byte count, and lowercase SHA-256 identity. |
| `browserProfileInputs` | Required only by `browser-evidence`: the exact ordered eight-file profile input set, including Cargo manifests, the pinned PliegoRS contract, both gate scripts, and the contract verifier. |
| `browserEvidence` | `not-required` or `required`. `not-required` forbids a browser check; `required` remains blocked without one and can pass only through exact evidence. |

Unknown fields, unknown kinds, unknown profiles, incomplete source coverage, duplicate IDs, alternate
formatting, and a check set different from the Change Receipt fail closed before execution.

### Token-graph integrity

One changed canonical `pliegocss-token-graph/1` artifact uses:

```json
{
  "id": "token-graph.integrity",
  "kind": "token-graph-integrity",
  "source": "dist/pliego.tokens.json",
  "compatibilityProfile": "none"
}
```

The check invokes the closed `parse_token_graph` core directly. Exact canonical bytes and valid
relationships emit verified informational finding `PCSS-TOKEN-000`; malformed, unknown,
noncanonical, inconsistent, or cyclic graphs emit verified error `PCSS-TOKEN-001` and make the check
`failed`. Both outcomes publish a complete adjacent FindingDocument. No resolver, filesystem import,
network access, or external process is invoked.

### CSS budget audit

Schema 1.2 binds the changed CSS and its policy separately:

```json
{
  "id": "budget",
  "kind": "css-budget-audit",
  "source": "src/app.css",
  "compatibilityProfile": "baseline-widely",
  "budgetPolicy": {
    "file": "config/css-budgets.json",
    "bytes": 742,
    "sha256": "<64 lowercase hexadecimal characters>"
  },
  "budgetSubjects": [
    { "kind": "package", "id": "app" },
    { "kind": "route", "id": "/home" }
  ]
}
```

The policy must be canonical CSS budget policy schema 1 bytes below `--source-root`. Its identity is
part of the canonical check policy and is repeated in the check result. The verifier measures the
entire CSS source plus detected layers, attributes the same artifact metrics to explicitly declared
package/route subjects, and reuses `PCSS-BUDGET-100|101|102|199` semantics from `pliego-cssc audit`.
An enforced limit or incomplete policy coverage makes the check `failed`; reviewed exceptions remain
visible evidence without failing the check. Missing, extra, drifted, noncanonical, linked, or aliased
policy inputs are tool errors, not budget passes.

### Fixed Rust test-suite evidence

`test-suite-evidence` is additive: it cannot replace source-specific audit coverage. Its `source`
anchors the finding to one changed file, while the evidence binds the entire Change Receipt hash.
The policy must bind three exact inputs:

```json
{
  "id": "tests.workspace",
  "kind": "test-suite-evidence",
  "source": "src/app.css",
  "compatibilityProfile": "none",
  "testEvidence": {
    "file": "pliego.css.test-evidence.json",
    "bytes": 731,
    "sha256": "<64 lowercase hexadecimal characters>"
  },
  "workspaceManifest": {
    "file": "Cargo.toml",
    "bytes": 1489,
    "sha256": "<64 lowercase hexadecimal characters>"
  },
  "lockfile": {
    "file": "Cargo.lock",
    "bytes": 42231,
    "sha256": "<64 lowercase hexadecimal characters>"
  }
}
```

The canonical test-evidence document uses schema `1.0.0` and contains a
self-hash, exact Change Receipt self-hash, exact check ID, fixed
`rust-workspace-all-targets` profile, `pliego-css-agent` runner name/version, observed Cargo/rustc
version lines and host triple, exact root Cargo manifest and lockfile identities, numeric exit code,
and derived `passed|failed` result. Exit zero is the only passing result. The verifier revalidates all three policy identities, canonical evidence,
check ID, Change Receipt hash, runner identity, profile, workspace input identities, and derived
result. A pass emits `PCSS-TEST-000`; a recorded nonzero exit emits `PCSS-TEST-001` and fails the
Verification Receipt. Missing, stale, malformed, or mismatched evidence is a tool error.

```json
{
  "schemaVersion": "1.0.0",
  "evidenceSha256": "<canonical payload hash>",
  "changeReceiptSha256": "<Change Receipt self-hash>",
  "checkId": "tests.workspace",
  "profile": "rust-workspace-all-targets",
  "runner": {
    "name": "pliego-css-agent",
    "version": "0.0.0"
  },
  "toolchain": {
    "cargoVersion": "cargo 1.96.0 (...)",
    "rustcVersion": "rustc 1.96.0 (...)",
    "host": "x86_64-unknown-linux-gnu"
  },
  "workspaceManifest": {
    "file": "Cargo.toml",
    "bytes": 1489,
    "sha256": "<exact file hash>"
  },
  "lockfile": {
    "file": "Cargo.lock",
    "bytes": 42231,
    "sha256": "<exact file hash>"
  },
  "result": "passed",
  "exitCode": 0
}
```

### Fixed PliegoRS Chromium evidence

`browser-evidence` is additive, like test evidence: another source-specific check must still cover
every changed source. Schema 1.4 permits at most one browser check, and only when top-level
`browserEvidence` is `required`:

```json
{
  "id": "browser.pliegors",
  "kind": "browser-evidence",
  "source": "src/app.css",
  "compatibilityProfile": "none",
  "browserEvidenceFile": {
    "file": "pliego.css.browser-evidence.json",
    "bytes": 2381,
    "sha256": "<64 lowercase hexadecimal characters>"
  },
  "browserProfileInputs": [
    { "file": "Cargo.lock", "bytes": 42231, "sha256": "<exact file hash>" },
    { "file": "Cargo.toml", "bytes": 1489, "sha256": "<exact file hash>" },
    { "file": "integration-tests/pliegors-smoke/Cargo.toml", "bytes": 1902, "sha256": "<exact file hash>" },
    { "file": "integration-tests/pliegors-smoke/client/Cargo.toml", "bytes": 511, "sha256": "<exact file hash>" },
    { "file": "integration-tests/pliegors-smoke/pliegors-contract.json", "bytes": 689, "sha256": "<exact file hash>" },
    { "file": "scripts/check-pliegors-browser.mjs", "bytes": 23459, "sha256": "<exact file hash>" },
    { "file": "scripts/check-pliegors-integration.mjs", "bytes": 38921, "sha256": "<exact file hash>" },
    { "file": "scripts/pliegors-contract.mjs", "bytes": 3912, "sha256": "<exact file hash>" }
  ]
}
```

The canonical browser evidence schema is `1.0.0`. It binds its self-hash, Change Receipt, check ID,
fixed `pliegors-visit-counter-chromium-cdp` profile, agent/Node identity, all eight profile inputs,
observed Chromium/CDP metadata, exact object-identity and state observations, process exit code, and
derived result. A pass requires the same document, island, button, and value objects; one island;
state/text `15→20`; stable `pc_*` classes; one typed `minutes=20` event; two module scripts; one
stylesheet preload and request; four client resource requests; stylesheet reuse; WASM ready; and no
relevant console/network or server errors.

```json
{
  "schemaVersion": "1.0.0",
  "evidenceSha256": "<canonical payload hash>",
  "changeReceiptSha256": "<Change Receipt self-hash>",
  "checkId": "browser.pliegors",
  "profile": "pliegors-visit-counter-chromium-cdp",
  "runner": {
    "name": "pliego-css-agent",
    "version": "0.0.0",
    "nodeVersion": "v24.16.0"
  },
  "profileInputs": [
    { "file": "Cargo.lock", "bytes": 42231, "sha256": "<exact file hash>" },
    { "file": "Cargo.toml", "bytes": 1489, "sha256": "<exact file hash>" },
    { "file": "integration-tests/pliegors-smoke/Cargo.toml", "bytes": 1902, "sha256": "<exact file hash>" },
    { "file": "integration-tests/pliegors-smoke/client/Cargo.toml", "bytes": 511, "sha256": "<exact file hash>" },
    { "file": "integration-tests/pliegors-smoke/pliegors-contract.json", "bytes": 689, "sha256": "<exact file hash>" },
    { "file": "scripts/check-pliegors-browser.mjs", "bytes": 23459, "sha256": "<exact file hash>" },
    { "file": "scripts/check-pliegors-integration.mjs", "bytes": 38921, "sha256": "<exact file hash>" },
    { "file": "scripts/pliegors-contract.mjs", "bytes": 3912, "sha256": "<exact file hash>" }
  ],
  "browser": {
    "family": "chromium",
    "executable": "chrome.exe",
    "product": "Chrome/150.0.7871.124",
    "revision": "@<revision>",
    "protocolVersion": "1.3",
    "jsVersion": "15.0.245.19",
    "userAgent": "<observed user agent>"
  },
  "observation": {
    "nodeIdentity": { "document": true, "island": true, "button": true, "value": true },
    "islandCount": 1,
    "islandId": "visit-counter",
    "initialMinutes": 15,
    "finalMinutes": 20,
    "increment": 5,
    "initialText": "15",
    "finalText": "20",
    "initialClass": "pc_<stable class>",
    "finalClass": "pc_<stable class>",
    "eventCount": 1,
    "eventKey": "minutes",
    "eventValue": 20,
    "moduleScriptCount": 2,
    "preloadEntryCount": 1,
    "preloadRequestCount": 1,
    "clientRequestCount": 4,
    "stylesheetPresent": true,
    "wasmReady": true,
    "relevantEventCount": 0,
    "serverErrorCount": 0
  },
  "result": "passed",
  "exitCode": 0
}
```

The verifier revalidates canonical evidence, the Change Receipt/check ID bindings, and every profile
input identity. Passing evidence emits `PCSS-BROWSER-000` and changes receipt browser state to
`required-passed`. A completed failed observation emits `PCSS-BROWSER-001`, changes browser state to
`required-failed`, and fails the receipt. Missing evidence leaves a required policy `blocked`;
malformed, stale, extra, missing, drifted, linked, or mismatched evidence is a tool error.

## Execution boundary

The verifier:

1. parses canonical Change Receipt and policy bytes;
2. resolves exactly the Change Receipt after-source set and every identity-bound supplementary
   budget/test/browser input beneath `--source-root`;
3. acquires the persistent `.pliegocss-repair.lock` used by apply;
4. re-reads both control artifacts, every source, and every supplementary input under that lock;
5. verifies source transitions and every supplementary byte count/SHA-256 identity;
6. dispatches only the compiled-in Lightning CSS audit, token-graph parser, CSS budget evaluator,
   or canonical test/browser evidence validator; `verify` never starts Cargo, Node, or a browser;
7. serializes and validates the complete canonical `FindingDocument` for every check;
8. re-reads control artifacts, sources, and supplementary inputs after execution;
9. stages all missing finding artifacts plus the Verification Receipt with synced file contents;
10. links every missing finding artifact create-if-absent and links the receipt last. A failure
    removes newly linked evidence before returning.

An existing byte-identical receipt is a no-op only when every adjacent finding artifact is also
present and byte-identical. A different finding artifact or receipt fails rather than overwriting
evidence. Pre-existing exact finding artifacts can complete an interrupted receipt-last publication.
The cooperative lock does not claim control over writers that deliberately ignore it.

### Adjacent FindingDocument names

The file for each check is deterministic and does not extend a user-controlled filename beyond
portable limits:

```text
pliego-css-findings-<sha256(asciiLower(receiptLogicalPath) + NUL + checkId)>.json
```

It is placed in the Verification Receipt directory. The receipt already binds each document's exact
bytes, SHA-256, and finding count; the naming rule binds that identity to one check without adding a
path field to the receipt. The public Rust result also exposes check ID, logical file, canonical bytes, and
whether the destination was newly created.

## Verification Receipt

New receipts use schema `1.4.0`; canonical `1.0.0`, `1.1.0`, `1.2.0`, and `1.3.0` receipts remain readable
under their original kind limits:

```json
{
  "schemaVersion": "1.4.0",
  "receiptSha256": "<canonical payload hash>",
  "changeReceipt": {
    "file": "pliego.css.change-receipt.json",
    "bytes": 1234,
    "sha256": "<exact file hash>"
  },
  "changeReceiptSha256": "<Change Receipt self-hash>",
  "checkPolicy": {
    "file": "pliego.css.check-policy.json",
    "bytes": 261,
    "sha256": "<exact policy hash>"
  },
  "result": "passed",
  "checks": [
    {
      "id": "audit",
      "kind": "standard-css-audit",
      "status": "passed",
      "source": {
        "file": "src/app.css",
        "bytes": 25,
        "sha256": "<exact after-source hash>"
      },
      "compatibilityProfile": "baseline-widely",
      "findingDocumentBytes": 1234,
      "findingDocumentSha256": "<canonical derived FindingDocument hash>",
      "findingCount": 1
    }
  ],
  "browserEvidence": "not-required"
}
```

`receiptSha256` is derived over every field except itself. `changeReceipt` binds the exact canonical
input file while `changeReceiptSha256` binds its semantic self-hash. Every check binds exact source,
profile, status, optional budget-policy identity, and canonical derived `FindingDocument` identity.
The adjacent bytes must match that identity exactly. A `css-budget-audit` result repeats
`budgetPolicy`; its ownership subjects remain integrity-bound through the exact `checkPolicy`
artifact rather than being duplicated in the receipt. A `test-suite-evidence` result repeats only
the exact `testEvidence` identity; the exact manifest/lockfile identities remain bound by both the
canonical evidence and exact check-policy artifact. A `browser-evidence` result repeats its exact
evidence identity and all profile input identities.

Result is derived, never accepted from policy:

| Result | Meaning |
|---|---|
| `passed` | Every check passed and browser evidence was either not required or exactly `required-passed`. |
| `failed` | At least one check failed, including `required-failed`; this dominates browser blocking. |
| `blocked` | All configured checks passed, but policy required browser evidence and supplied no browser check. |

## Explicit limits

- This is integrity and local execution evidence, not identity authentication or a signature.
- `verify` does not execute Cargo, Node, a browser, deployment, or user-defined commands.
  `run-tests` and `run-browser` are separate explicit process boundaries with one fixed profile each.
- `run-tests` executes project build scripts and test code by design. `--offline` prevents Cargo
  registry/network resolution; it is not an OS sandbox and cannot prevent project code from opening
  files, processes, or networks.
- Test evidence binds the Change Receipt after-source set plus root `Cargo.toml` and `Cargo.lock`.
  It does not yet inventory every unchanged workspace source, retain a transcript, count tests, sign
  runner identity, hash the PATH-resolved Cargo executable, or prove hermetic execution. Hosted
  attestation remains a release gate.
- Browser evidence binds the Change Receipt after-source set and eight fixed profile inputs, but it
  does not inventory every unchanged Rust source or hash/authenticate the PATH-resolved Node and
  Chrome executables. `PLIEGOCSS_CHROME_PATH` may select Chromium; `PLIEGORS_ROOT` may select a
  checkout only when it matches the exact bound contract revision and covered-source hash.
- `run-browser` executes the fixed integration scripts, Cargo builds, Rust build scripts, and
  browser code by design. The cooperative lock and byte rereads detect drift; they are not an OS
  sandbox and do not prevent executed code from opening files, processes, or networks.
- The implemented browser profile proves one PliegoRS visit-counter flow in one local Chromium
  product. It is not Firefox, WebKit/Safari, responsive/visual regression, accessibility, hosted,
  signed, hermetic, or production-deployment evidence.
- Individual sources/documents are capped at 16 MiB and all generated finding evidence together is
  capped at 64 MiB per verification.
- `browserEvidence: required` without an exact browser check yields `blocked`; it never becomes an
  inferred static pass.
- A `passed` receipt proves only the exact built-in policy and after-source set it binds.
