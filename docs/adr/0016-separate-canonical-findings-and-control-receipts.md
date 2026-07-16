# ADR-0016: Separate canonical findings and control receipts from legacy artifacts

## Status

Accepted. Finding schema 1.0.0 and its human, JSON, and SARIF 2.1.0 projections are implemented.
The unified control-manifest and build-receipt schema-1 Rust contracts are implemented in
`pliego-css-control`; analyzer population, grouped publication, and CLI integration remain open.

## Context

The existing CLI diagnostic schema 1 reports one-shot process failures. Existing style manifests
3-5 bind generated classes, origins, semantic ownership, and physical CSS. Neither contract can be
silently expanded into the report's audit finding, policy, budget, or receipt model without breaking
consumers and falsely implying evidence the documents do not carry.

Human diagnostics are also currently assembled separately from JSON fields. That is insufficient
for the report's requirement that humans, agents, CI, and SARIF receive the same semantic result.

## Decision

- Introduce canonical finding schema `1.0.0` in `pliego-css-build::artifacts`.
- Make a validated `Finding` the source for future human, JSON, and SARIF renderers.
- Require stable code, severity, cause/policy, exact logical source/source-map span, context,
  evidence, verification state, ranked suggestions with risk/scope/prerequisites, exception, and a
  recomputed fingerprint.
- Keep CLI diagnostic schema 1 as a legacy process-failure projection until commands migrate
  deliberately. Do not relabel it agent-ready.
- Reserve `pliego-css-control-manifest` schema `1.0.0` for the central
  `pliego.css.manifest.json`. It references existing manifests/Project Index instead of replacing
  them.
- Reserve a separate `pliego-css-build-receipt` schema `1.0.0` that hashes the complete central
  manifest and outputs. The manifest embeds only the receipt expectation (schema/file, required
  checks, expected result), avoiding both a self-hash cycle and a manifest/receipt hash cycle.
- Keep fix plans and change receipts separate from build receipts. No finding or suggestion grants
  write authorization.

Schema 1 rejects unknown fields, noncanonical logical paths/order, duplicate findings, fingerprint
tampering, nondeterministic flags, noncontiguous suggestion ranks, and defensive-limit violations.

## Consequences

- Existing consumers of CLI diagnostics and style manifests do not break.
- New audit policy can be built against a complete contract instead of adding ad hoc optional fields.
- Human/JSON/SARIF parity is executable; every SARIF result embeds its complete canonical finding.
- The central manifest can evolve independently while integrity-binding specialized artifacts.
- Absolute host paths cannot leak into canonical findings, enabling cross-OS byte identity.
- The control contracts live in `pliego-css-control`, which has no internal workspace dependencies,
  because `pliego-css-build` had only 665 compressed bytes of headroom when this slice began. Both
  package ceilings remain mandatory gates.

## Rejected alternatives

### Add fields to CLI diagnostic schema 1

Rejected because it changes a required closed object, still conflates process failure with audit
findings, and gives no clean path to policy evidence or verification states.

### Call the next style manifest schema 6

Rejected because style manifests describe one generated CSS artifact, while the control manifest
joins repository inputs, backend/data identity, budgets, findings, multiple outputs, and receipts.
Those are different document kinds and lifecycles.

### Let each renderer build its own diagnostics

Rejected because severity, evidence, source mapping, and suggestions would drift between human,
JSON, SARIF, editor, and agent clients.

See [finding schema 1.0.0](../reference/finding-schema-1.md), the
[control-artifact schema 1.0.0](../reference/control-artifacts-schema-1.md), and the
[R0 manifest/receipt design](../product/r0-manifest-receipt-design.md).
