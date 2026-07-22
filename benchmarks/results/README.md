# Benchmark results

Machine-specific measurements use the suffix `.local.json` and are not committed. Immutable clean
commit snapshots with complete environment metadata belong in `benchmarks/evidence/`; narrative
comparisons live under `docs/benchmarks/`.

Run the current competitor authority with:

```console
node scripts/measure-benchmark-authority-v2.mjs
```

The schema-2 local report is `benchmark-authority-v2.local.json`. It contains paired latency and
peak-working-set samples plus raw/gzip/Brotli output for latest, v3-LTS, and frozen Tailwind lanes
across micro, medium, and large corpora. Legacy Gate A/B local filenames are historical contracts.

The legacy `pnpm baseline:measure-legacy-v4` command remains an exploratory single-version harness
and is not a current competitive oracle.

Run the deterministic adjacent-media size gate with `pnpm check:media`. Use
`pnpm baseline:measure-media` to write the ignored `media-merge.local.json` report.

The script deliberately labels its timings `processBuildMs`: every sample starts a fresh CLI process.
Persistent watch-mode incremental latency is a different metric and must not be inferred from these
numbers.
