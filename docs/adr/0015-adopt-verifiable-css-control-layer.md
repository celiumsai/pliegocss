# ADR-0015: Adopt a standards-first verifiable CSS control layer

## Status

Accepted. This decision changes product positioning and release priority; it does not remove the
implemented utility syntax or invalidate its semantic-IR contracts.

## Context

The initial plan described PliegoCSS as a utility-first framework native to Rust and PliegoRS. That
description overweights an authoring surface and implementation language. The 14 July 2026 strategic
report found a stronger category in compatibility policy, provenance, accessibility guardrails,
budgets, agent contracts, and deterministic receipts over standard CSS.

Fast Rust CSS compilers, utility frameworks, static extraction, typed tokens, and LLM-oriented
documentation already exist. PliegoCSS cannot defend a category merely through Rust, speed, small
generated CSS, or another utility taxonomy. Its existing semantic IR, stable identities, compatibility
policy, provenance graph, Project Index, and deterministic artifacts are more valuable as the
foundation of a framework-neutral verification layer.

## Decision

Position and design PliegoCSS as a standards-first compiler and verifier:

- normal input and output are standard CSS;
- `pc!`, `pcx!`, and utility syntax are optional typed producers of the same semantic pipeline;
- `pliegocss audit` is the first-value path for an existing repository;
- Lightning CSS remains the initial parsing/transform/minification backend behind Pliego-owned
  contracts;
- the product moat is semantic IR, policy-as-code, provenance, explanation, agent-safe plans, and
  receipts;
- PliegoRS gets the deepest typed integration while generic CSS and other hosts remain supported;
- deterministic static analysis declares its limits and delegates layout/rendering evidence to real
  browsers;
- the core requires no JavaScript/WASM styling runtime and no embedded generative model;
- no accessibility output may claim automatic WCAG compliance;
- no public `0.1.0` is cut until the R0 gates in the strategic product contract are satisfied.

The development sequence is evidence corpus, audit/compile R0, explain/agent R1, browser-assisted R2,
then governed ecosystem R3. Broad authoring-language expansion is paused unless it is necessary to
close one of those gates.

## Consequences

- Previously implemented utility-first work remains supported but becomes one frontend, not the
  product definition.
- The old five-to-seven-week framework estimate is superseded. Remaining `0.1.0` work is estimated
  separately from interviews and real-incident access.
- Existing manifests remain numbered contracts; a future unified manifest/receipt receives a new
  schema instead of overloading schemas 3-5.
- Performance stays a release gate, but claims emphasize verifiability and measured diagnostic
  quality.
- Tailwind fixtures remain useful for regression and migration analysis, not category positioning.
- Features that only make proprietary authoring more convenient move behind audit, policy, evidence,
  and interoperability work.

## Rejected alternatives

### Continue as a Tailwind-like framework for Rust

Rejected because it narrows adoption to a migration, competes in an occupied category, and does not
address the highest-priority pain points in existing CSS repositories.

### Replace the existing compiler with a new CSS parser or browser engine

Rejected because it duplicates mature infrastructure and diverts effort from semantic verification.
Real browsers remain authoritative for layout, fonts, pseudo-state rendering, and viewport evidence.

### Put an LLM inside the core

Rejected because policy and build outcomes must be deterministic, reproducible, and usable by any
external agent. Agents propose; the compiler verifies.

See the [strategic product contract](../product/strategic-product-contract-2026.md) and the
[research traceability matrix](../product/research-requirements.md).
