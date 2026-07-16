# Benchmark results

Machine-specific measurements use the suffix `.local.json` and are not committed. Immutable clean
commit snapshots with complete environment metadata belong in `benchmarks/evidence/`; narrative
comparisons live under `docs/benchmarks/`.

Run the current Tailwind baseline with:

```shell
pnpm baseline:measure
```

Run the deterministic adjacent-media size gate with `pnpm check:media`. Use
`pnpm baseline:measure-media` to write the ignored `media-merge.local.json` report.

The script deliberately labels its timings `processBuildMs`: every sample starts a fresh CLI process.
Persistent watch-mode incremental latency is a different metric and must not be inferred from these
numbers.
