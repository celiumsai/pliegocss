# Rust check baseline

Status: clean-commit paired snapshot frozen; host-security variance recorded

Date: 2026-07-13

The harness compares two standalone crates with equivalent source shape:

- plain Rust carrying the utility text in a small local `Style` type;
- `pliego-css` using the compile-time `pc!` macro.

The authoritative run uses adjacent pairs, balances `plain → styled` and `styled → plain`, and
reports the median of the observed per-pair deltas (`styled - plain`). It was produced from clean
commit `c47239ce0bf1d046aa01710d436baea61f4e6528`.

| Scenario | Pairs | Plain median | Styled median | Paired delta median | Paired delta % median | Delta MAD |
|---|---:|---:|---:|---:|---:|---:|
| Cold target | 6 | 378.025 ms | 10,160.012 ms | +9,782.699 ms | +2,848.833% | 254.188 ms |
| No-op check | 30 | 65.811 ms | 90.002 ms | +24.316 ms | +37.407% | 2.403 ms |
| Style literal changed | 30 | 129.825 ms | 159.112 ms | +28.228 ms | +21.614% | 138.672 ms |

Plain and styled medians are marginal summaries, not values to subtract. The paired columns are the
comparison: each delta is calculated inside one adjacent pair before summarization.

The incremental center is approximately 24–28 ms on this host and is acceptable for continuing the
developer loop. It is not a precision claim. Microsoft Defender and Windows Application Control
were active and coincided with positive and negative outliers. No-op retained a narrow 2.403 ms
delta MAD; changed reached 138.672 ms and must be read as a noisy directional result. Alternating
pair order prevents all periodic load from being assigned systematically to one fixture, but cannot
remove a scanner hit that lands on only one member of a pair; every sample remains visible in the
JSON.

Two preceding acquisition attempts terminated before producing a snapshot when Windows Application
Control blocked newly built dependency build scripts (`os error 4551`). The file below is the first
complete successful run, not a selected best result. Those failed attempts are not timing samples
and may have warmed ordinary OS/security caches, which the cold-target contract already declares as
retained.

The +9.783 s cold delta is material and remains an optimization target. It pays for the procedural
macro/compiler dependency graph from empty per-fixture targets. Cold pairs retain ordinary OS and
registry-source caches, so this is a cold Cargo target measurement, not a cold machine measurement.

Each changed pair receives the same arbitrary gap value in both sources before either check runs.
The styled member therefore includes syntax parsing, catalog lowering, conflict analysis,
canonicalization, and semantic `StyleId` derivation, while the plain member controls for the same
source mutation.

The harness requires exact Rust/Cargo 1.85.0, copies versioned fixture lockfiles and fetches their
exact dependencies before timing, and runs every measured check with `--locked --offline`, an
explicit host target, and a 120-second timeout. It uses isolated Cargo targets and Cargo home plus
an exclusive run lock, and records paired samples, environment, Git state, harness/fixture/lock
hashes, power-plan status, and declared security-tooling status. Schema 3 additionally requires the
harness and both lock-template byte streams to equal their Git blobs at the recorded source commit;
line-ending normalization cannot silently change the measured dependency graph.

The previous 2026-07-12 sequential figures (+6,038.164 ms cold, +19.397 ms no-op, and +6.206 ms
changed) are superseded. They measured all plain samples before all styled samples and subtracted
independent medians, so they are not used for the gate decision.

The immutable report with all 66 pairs is
[`rust-check-2026-07-13-c47239c.json`](../../benchmarks/evidence/rust-check-2026-07-13-c47239c.json).

Run locally with:

```shell
pnpm baseline:measure-rust
```

Machine-local samples are written to `benchmarks/results/rust-check.local.json`.

To create an immutable snapshot below the evidence root, start from a clean worktree, declare
security-tooling state, and use a new path:

```shell
node scripts/measure-rust-check.mjs --evidence benchmarks/evidence/<rust-check-snapshot>.json
```

The evidence path must be a new `.json` file strictly below `benchmarks/evidence/`; links, existing
destinations, and paths escaping that root are rejected.
