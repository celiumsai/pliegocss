# Tailwind Benchmark Authority v2

Status: **active machine authority; generated from [`oracle.json`](../../benchmarks/benchmark-authority-v2/oracle.json)**

This document is generated. Edit the schema-2 JSON authority and run
`node scripts/render-benchmark-authority-v2.mjs --write`; do not maintain the tables by hand.

The primary competitor is `tailwind-latest`. The oracle was observed at
`2026-08-26T12:40:21.000Z`, expires at `2026-09-02T12:40:21.000Z`, and has a maximum age of
168 hours. An expired or live-registry-mismatched oracle blocks a new
competitive result. The reset contract is explicitly **no-preflight** because
PliegoCSS does not emit Preflight.

## Competitor lanes

| Lane | Role | Registry selector | Tailwind | CLI |
|---|---|---|---:|---:|
| `tailwind-latest` | current-competitor | `latest` | `4.3.3` | `4.3.3` |
| `tailwind-v3-lts` | supported-historical-major | `v3-lts` | `3.4.19` | `3.4.19` |
| `tailwind-frozen-release` | historical-regression-control | `pinned version` | `4.3.2` | `4.3.2` |

`tailwind-frozen-release` is a regression control. It is not allowed to stand in for current
Tailwind. The v3 lane follows the upstream npm `v3-lts` dist-tag; “LTS” here names that registry
lane and does not expand Tailwind's support promises.

## Corpora

| Corpus | Scope | HTML bytes | Class attributes | Utility occurrences | Unique utilities |
|---|---|---:|---:|---:|---:|
| `micro` | one representative interactive component | 682 | 2 | 25 | 25 |
| `medium` | five representative views | 5,868 | 44 | 302 | 90 |
| `large` | source-volume stress corpus; utility vocabulary intentionally matches medium | 179,981 | 1,408 | 9,664 | 90 |

The large corpus repeats the medium body 32
times. It measures source-volume scaling while deliberately keeping the same utility vocabulary;
it is not presented as broader catalog coverage.

## Measurement contract

- 5 warmup pairs and 30 measured pairs per lane/corpus.
- Pair order alternates: odd pairs run PliegoCSS then Tailwind; even pairs run Tailwind then PliegoCSS.
- Every observation is a fresh process. Complete per-pair latency and peak-working-set samples are retained.
- A separate fresh invocation must reproduce each measured output hash.
- CSS and rewritten/source HTML report raw, gzip level 9, and Brotli quality 11 bytes.
- Transfer compares separate HTML and CSS streams under both compression formats.
- Candidate coverage must be 100% for the shared corpus before a pair is comparable.
- PliegoCSS and Tailwind use the same no-Preflight utility intent; browser computed-style equivalence belongs to G4.

Peak working set is measured for the direct child process by
[`benchmark-process-metrics.rs`](../../scripts/benchmark-process-metrics.rs), not inferred from
host free-memory deltas. Process startup, security tooling and normal OS caches remain part of the
fresh-process observation; the release build happens outside the timed region.

## Commands

```console
pnpm check:benchmark-authority
pnpm check:benchmark-oracle
node scripts/measure-benchmark-authority-v2.mjs --smoke
node scripts/measure-benchmark-authority-v2.mjs
```

Immutable evidence additionally uses `--evidence=benchmarks/evidence/v2/<snapshot>.json`, the
canonical 5/30 sample counts, a clean source tree, a new destination, and an explicit security-tooling
declaration. Machine-local results remain ignored at
`benchmarks/results/benchmark-authority-v2.local.json`.

## Claim boundary

No competitive score is valid without a non-expired oracle and immutable schema-2 evidence from a clean source tree.

Legacy Gate A/B snapshots remain historical integrity evidence. They cannot supply the current
competitor lane, memory, Brotli, or micro/medium/large claims and therefore cannot produce the v2
competitive score.
