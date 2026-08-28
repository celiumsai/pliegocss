# Tailwind CSS v4 baseline

Status: **historical F0 measurement; superseded for current comparison by [Benchmark Authority v2](./tailwind-benchmark-authority-v2.md)**

## Frozen inputs

- Tailwind CSS: 4.3.2
- Tailwind CLI: 4.3.2
- Node: 24.16.0
- Package manager: pnpm 11.7.0
- Fixtures: button, card, navbar, form, dashboard
- Source detection: explicit `source(none)` plus one `@source`

The F0 preview contains 44 class attributes, 302 utility occurrences, and 90 unique concrete utility
tokens across 103 lines. These are not 90 semantic families.

## Frozen local F0 measurement

Environment:

- Windows 10.0.26200.0, x64
- Intel Core Ultra 9 285H

Each fresh-process value uses five discarded warmups and thirty measured samples. Every profile
produced one output hash across all thirty development builds and one across all thirty minified
builds.

| Profile | Dev median | Min median | CSS raw | CSS gzip | HTML+CSS gzip |
|---|---:|---:|---:|---:|---:|
| Full complete | 231.306 ms | 210.202 ms | 12,377 B | 3,333 B | 4,839 B |
| No-preflight complete | 204.917 ms | 204.826 ms | 8,618 B | 2,327 B | 3,833 B |
| Full Gate-A core | 203.734 ms | 202.605 ms | 10,379 B | 2,879 B | 4,189 B |
| No-preflight Gate-A core | 202.488 ms | 198.967 ms | 6,620 B | 1,838 B | 3,148 B |

The class-only control produced the same minified hash as scanning the complete fixture HTML. No
candidate outside a `class` attribute changed this fixture's output.

## Persistent watch

The watch harness uses twenty warmups, one hundred content-only mutations, one hundred additional
uses of an already-known utility, and fifty new arbitrary `z-index` utilities.

| Mutation | Wall median | Engine median | Engine p95 |
|---|---:|---:|---:|
| Content only | 77.678 ms | 0.787 ms | 1 ms |
| Additional known utility | 77.777 ms | 0.756 ms | 1 ms |
| New utility | 77.780 ms | 6 ms | 7 ms |

Wall time includes Windows filesystem notification and CLI reporting. Engine time is parsed from
Tailwind's completion line and is the meaningful engine baseline.

The scripts write complete machine-local output to `benchmarks/results/tailwind-v4.local.json` and
`benchmarks/results/tailwind-watch.local.json`.

## Gates

- Gate A, end of week 2: core parser, IR, semantic validation, deterministic generation, and core
  fixture subset.
- Gate B, end of week 3: complete fixtures including responsive, hover, focus-visible, disabled, and
  conditional composition.

This split avoids comparing a week-2 compiler against Tailwind features scheduled for week 3.
