# Benchmark methodology

PliegoCSS comparisons freeze DOM, content, accessibility attributes, theme values, and browser
targets. Only the styling authoring and compilation surface may change.

## Fixtures

| Fixture | Contract |
|---|---|
| Button | Inline layout, spacing, color, radius, hover, focus-visible, disabled |
| Card | Surface, border, shadow, typography, media, responsive composition |
| Navbar | Responsive layout, alignment, navigation states |
| Form | Grid, inputs, placeholder, focus, disabled, textarea |
| Dashboard | Responsive grid, spans, repeated patterns, deduplication |

Verification viewports are 375×812, 768×1024, and 1440×900. Fixtures use local text and shapes,
with no remote fonts, images, or network requests.

## Performance metrics

### Fresh-process build

- Launch the compiler process for every sample.
- Keep normal operating-system caches; do not claim a cold-system result.
- Discard five warmups and record thirty samples for frozen reports.
- Report median, p95, minimum, maximum, and median absolute deviation.
- Measure development and minified output separately.

### Persistent rebuild

Keep watch mode alive and measure:

1. content change with no candidate change;
2. an additional use of a known utility;
3. introduction of a new valid utility.

Tailwind watch and `pliego-cssc watch` are compared separately from Rust macro overhead.

Before using latency numbers, validate incremental correctness without timing noise: record the
watcher's discovered/scan-hit/parsed/semantic-hit/lowered/removed source-unit counts and compare
warm-cache CSS, manifest, and diagnostics byte-for-byte with a fresh-cache compilation after create,
edit, rename, delete, and theme mutations. A one-file edit in an N-file tree must report one parse
and semantic miss plus N-1 scan and semantic hits.

### Rust integration

Measure `cargo check` on the same fixture crate without and with `pc!`, then report absolute and
percentage delta. Do not compare `cargo check` directly to a standalone Tailwind CLI invocation.

The local harness records six cold pairs, thirty no-op pairs, and thirty changed-source pairs. Each
pair runs the plain and styled fixtures adjacently, alternating `plain → styled` and `styled → plain`
between pairs to reduce periodic host and security-scanner bias. Report the median of the per-pair
absolute and percentage deltas; do not subtract independently sampled medians.

Before every changed pair, both sources receive the same new arbitrary gap value. Before every cold
pair, both isolated target directories are removed. The harness restores the generated fixture
sources after measurement and verifies that both copied lockfiles and their versioned templates
remained byte-identical.

Rust integration intentionally uses exact Rust/Cargo 1.85.0, the project MSRV: versioned fixture
lockfiles and dependency fetches are prepared outside the timed region, every measured invocation uses
`cargo +1.85.0 check --locked --offline` with the explicit host target, and every child command has a
120-second timeout. Incremental compilation remains enabled because no-op and changed-source checks
measure the developer loop. An exclusive ignored run lock prevents concurrent harnesses from deleting
or sharing generated fixtures. Build-affecting Cargo/Rust environment overrides are removed, and an
isolated Cargo home prevents untracked user Cargo configuration from changing the measured graph.

## Output size

Production output is measured without source maps:

- CSS raw and gzip level 9;
- HTML raw and gzip level 9;
- HTML gzip plus CSS gzip as separate HTTP streams;
- bytes in style/class attributes;
- rules, declarations, and custom properties.

CSS alone is insufficient: grouped output may reduce HTML while increasing CSS, and atomic output may
do the opposite.

Targeted deterministic optimizations may use a structural control rather than timing samples. The
adjacent-media harness compiles each style separately in canonical manifest order, proves the merged
candidate is one wrapper around the exact unchanged rule sequence, and compares raw plus gzip level
9. Its threshold is scoped to that responsive-only fixture and cannot be reported as a universal
application reduction.

Two reset contracts are required before the final comparison:

- `full`: theme, preflight, and utilities;
- `no-preflight`: theme and utilities.

PliegoCSS must be compared under the same reset contract.

## Robustness mutations

The test corpus includes typo, missing token, wrong value domain, exact conflict, incomplete variant,
unknown breakpoint, malformed arbitrary value, duplicate, redundant override, and illegal variant.

For every mutation record exit code, source span, explanation, suggestion, and whether the suggested
fix compiles.

## Repair-agent corpus

The tracked repair authority corpus is a synthetic conformance gate, not the strategic real-incident
or turn-reduction corpus. `pnpm check:repair-corpus` validates its closed provenance and claim
boundary, hashes the exact bytes, and requires the data-driven Rust replay to preserve one accepted
control plus fifteen intended fail-closed rejections.

Agent-loop evidence must use a separately reviewed frozen dataset. For every trial record the exact
case hash, model and agent surface, initial prompt/context, tool calls, assistant turns, accepted
plan and receipt hashes, required-check outcome, final violations, and failure category. Compare the
same model/agent configuration before and after the repair workflow. A reduction is valid only when
the post-workflow run uses fewer assistant turns and does not increase remaining or newly introduced
violations. Time and token counts are secondary diagnostics, not substitutes for the turn and
violation gate.

Real incidents additionally require provenance, consent/redaction status, expected diagnosis,
allowed ambiguity, and stable content hashes. Synthetic conformance cases cannot be counted toward
the required 20 incidents or diagnostic precision/recall.

## Environment and reproducibility

Gate A and Gate B require the exact Rust 1.96.0 toolchain. Their harnesses invoke
`cargo +1.96.0` and `rustc +1.96.0`, derive the host target from `rustc -vV`, build with an
explicit `--target`, disable incremental compilation, and isolate artifacts under
`target/benchmarks/pliego-gate-a` or `target/benchmarks/pliego-gate-b`. Every measured compiler
invocation selects the built-in theme explicitly with `--seed`.

Each report records:

- Git commit, dirty state, status-entry count, and a hash of the status stream;
- benchmark-harness path and SHA-256;
- OS platform, type, release, version, and architecture;
- CPU model(s), logical core count, available parallelism, and total RAM;
- Node and zlib versions;
- exact Cargo and rustc versions, host target, controlled Cargo target directory, and incremental
  build state;
- power-plan probe status, source, and value;
- security-tooling status and description;
- timestamp plus fixture, derived-input, executable, manifest, CSS, and HTML hashes as applicable.

The active power plan is probed with `powercfg /getactivescheme` on Windows, `pmset -g` on macOS,
and `powerprofilesctl get` with a Linux CPU-governor fallback. Set `PLIEGO_BENCH_POWER_PLAN` when the
host cannot expose a queryable plan or when an external runner owns that state.

Security tooling is intentionally declared rather than guessed. Set
`PLIEGO_BENCH_SECURITY_TOOLING_STATUS` to `active`, `disabled`, `not-installed`, or `unknown`; for
`active` and `disabled`, also set `PLIEGO_BENCH_SECURITY_TOOLING` to the product and relevant mode.
If no status is supplied, a local report records `not-reported` explicitly; evidence mode requires a
declared status.

Normal runs update the ignored local result files. To also create an immutable, versionable snapshot,
pass a new `.json` path strictly below `benchmarks/evidence/`:

```console
node scripts/measure-rust-check.mjs --evidence benchmarks/evidence/<rust-check-snapshot>.json
node scripts/measure-pliego-gate-a.mjs --evidence benchmarks/evidence/<gate-a-snapshot>.json
node scripts/measure-pliego-gate-b.mjs --evidence benchmarks/evidence/<gate-b-snapshot>.json
```

The local result and evidence snapshot contain identical JSON bytes. Evidence creation requires a
clean Git worktree and refuses paths outside that root, linked parent directories, and existing
destinations. Gate A evidence additionally requires the canonical thirty measured samples
(`PLIEGO_BENCH_RUNS=30`, the default). Rust-check evidence uses its fixed 6/30/30 paired sample counts.
Rust-check and Gate B evidence use schema version 3; Gate A evidence uses schema version 4. These
schemas require exact source-commit blobs for every tracked harness, fixture, and lock input.

`pnpm check:evidence` verifies every committed snapshot against the harness blob at its recorded Git
commit, recomputes summary statistics and Gate B comparisons, and reconstructs every Rust paired
delta from nanosecond observations. CI checks out full history for this integrity gate; it validates
frozen evidence but does not rerun host performance measurements. Snapshot filenames and complete
file hashes are allowlisted explicitly; accepting new evidence is therefore a reviewed source change.
Evidence generation additionally rejects tracked harness, fixture, or lock bytes that differ from
their source-commit Git blobs, including differences hidden by line-ending normalization.

Machine-local exploratory results use `.local.json` and are not committed as universal claims.
