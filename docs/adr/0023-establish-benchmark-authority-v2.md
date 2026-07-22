# ADR-0023: Establish Benchmark Authority v2

- Status: Implemented; owner acceptance pending
- Date: 2026-07-22

## Context

Gate B compared PliegoCSS with one handwritten Tailwind CSS 4.3.2 snapshot. It was useful as
historical evidence, but it could not answer whether the current competitor, the maintained v3
line, and the frozen regression control still behaved the same way. It also retained summary
statistics instead of every paired observation and did not measure direct child-process peak
working set or Brotli payloads.

A competitive claim that silently mixes a stale package, different corpora, unmatched process
orders, or incomplete output coverage is not an authority for a 1.0 LTS decision.

## Decision

Use `benchmarks/benchmark-authority-v2/oracle.json` as the machine authority for competitive
measurements. The authority defines exactly three lanes:

- `tailwind-latest`, resolved from the current npm `latest` tag;
- `tailwind-v3-lts`, resolved from the npm `v3-lts` tag; and
- `tailwind-frozen-release`, an explicitly pinned historical regression control.

The oracle expires after seven days. Its offline gate binds package aliases, installed versions,
lockfile integrity values, fixture materialization, methodology, and generated documentation. Its
network gate additionally proves the mutable npm tags and package integrity values are still
current.

Measurements use micro, medium, and deterministic large source-volume corpora. Every comparison
runs five warm-up pairs and thirty measured pairs, alternates adjacent process order, keeps every
sample, verifies a fresh output against measured hashes, requires complete Tailwind candidate
coverage, and records direct child-process peak working set. Payload evidence covers raw, gzip
level 9, and Brotli quality 11 bytes for CSS and shipped HTML.

Machine-local results can validate the harness but cannot freeze a competitive score. Immutable
evidence requires a clean source tree, canonical sample counts, a new file below
`benchmarks/evidence/v2`, and explicit security-tooling state. No score is valid if the oracle is
expired or the evidence is not immutable schema 2.

The old Gate B document and snapshot remain historical evidence. The Gate B command now delegates
to Benchmark Authority v2 and cannot create a new handwritten baseline.

## Rejected alternatives

### Compare only with the latest Tailwind release

Rejected because it loses the supported v3 adoption surface and a stable regression control.

### Keep summary-only timing evidence

Rejected because medians cannot prove pairing, order alternation, variance, or outliers.

### Publish a score from a dirty worktree

Rejected because the source state cannot be reconstructed from the commit alone.

## Consequences

- Competitive evidence fails closed when upstream knowledge is stale.
- All three lanes share identical corpora, reset policy, measurement process, and output checks.
- Large-corpus results measure source volume, not broader utility-catalog coverage.
- The generated authority and competitive-matrix pages derive their current state from JSON.
- A reviewed clean-tree run is still required before PliegoCSS can publish a current competitive
  score.

## Verification

`pnpm check:benchmark-authority` validates the local oracle and generated authority page.
`pnpm check:benchmark-oracle` validates the npm registry state. `pnpm check:benchmark-smoke`
executes all nine lane/corpus comparisons. A canonical machine-local run uses
`pnpm baseline:measure-authority-v2`, followed by `pnpm check:benchmark-result -- --canonical`.
Release CI includes the live oracle gate, while fast and integration profiles protect the local
contract and executable smoke path.
