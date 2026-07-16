# PliegoCSS Gate B benchmark

Date: 2026-07-13

Status: **complete fixture accepted by the compiler; production compatibility not yet proven**

Gate B measures the complete tracked Tailwind fixture without removing any responsive or state
variant. The harness reads `benchmarks/tailwind-v4/fixtures.html` directly, extracts its 44 `class`
attributes, compiles their complete values and rewrites only those values using the generated
`pc_*` manifest. It verifies that every attribute resolves and that all bytes outside `class`
values remain unchanged.

## Frozen contract

- Same five DOM fixtures: button, card, navbar, form and dashboard.
- 44/44 class attributes resolved, representing 31 unique style lists.
- 302 utility occurrences and 90 unique concrete utility tokens.
- All variants in the source retained: `sm`, `md`, `lg`, `hover`, `focus`, `focus-visible`,
  `placeholder` and `disabled`.
- Theme seed and Lightning CSS processing included.
- Release binary build, manifest generation and HTML rewriting excluded from timing.
- Five discarded warmup processes followed by exactly thirty fresh measured processes.

The input fixture hash is
`338eb480327e54fe7e56e81b42c67324464afaa8c65d606e5d57c1c45690e86c`, identical to the
frozen Tailwind complete-fixture baseline.

## Results

| Metric | PliegoCSS complete | Tailwind Full complete | Tailwind No-preflight complete |
|---|---:|---:|---:|
| Fresh-process minified, median | 37.894 ms | 210.202 ms | 204.826 ms |
| Fresh-process minified, minimum | 35.811 ms | 187.341 ms | 185.374 ms |
| Fresh-process minified, p95 | 41.290 ms | 318.531 ms | 418.003 ms |
| CSS raw | 10,463 B | 12,377 B | 8,618 B |
| CSS gzip | 2,049 B | 3,333 B | 2,327 B |
| Rendered HTML gzip | 1,501 B | 1,506 B | 1,506 B |
| Rendered HTML + CSS gzip | 3,550 B | 4,839 B | 3,833 B |

The primary comparison is Tailwind **No-preflight complete**. Under that closest available reset
contract, PliegoCSS has an 81.5% lower median fresh-process time (5.41x), 11.9% less CSS gzip and
7.4% less HTML+CSS transfer gzip. Its raw CSS is 21.4% larger, so the payload advantage depends on
compression and must not be described as a win on every size metric.

Against Tailwind Full complete, PliegoCSS has an 82.0% lower median, 38.5% less CSS gzip and 26.6%
less transfer gzip. That comparison is context only: Tailwind Full includes Preflight while
PliegoCSS emits no reset.

## Reset contract

PliegoCSS currently emits utility rules and its seed theme, but no browser reset or Preflight
equivalent. Therefore:

- No-preflight is the primary performance and payload baseline.
- Full is not an apples-to-apples payload baseline and cannot support a claim that PliegoCSS has a
  smaller complete styling stack.
- An application that needs normalized browser defaults must add an external reset today. Its
  bytes and behavior are not included in the PliegoCSS result.
- A future PliegoCSS reset must be measured as a separate profile before comparing it directly with
  Tailwind Full.

The theme payloads also differ: both systems implement the fixture tokens, but their theme
infrastructure is not byte-identical.

## Timing distribution

Twenty-nine samples fell between 35.811 ms and 41.290 ms. One process-start outlier measured
271.727 ms, raising the mean to 45.763 ms and the standard deviation to 41.977 ms. The median and
its 0.588 ms MAD describe the common path; the maximum keeps the isolated outlier visible. The frozen
Tailwind baselines also contain process-level outliers and use the same five-warmup, thirty-sample
method.

## HTML and transfer

Manifest classes reduce the full fixture from 5,868 B raw to 3,917 B raw. Gzip changes only from
1,506 B to 1,501 B because Tailwind utility names repeat and compress efficiently. PliegoCSS's
operational transfer number is therefore 1,501 B generated HTML gzip plus 2,049 B CSS gzip, or
3,550 B.

Keeping the original utility strings with PliegoCSS CSS would total 3,555 B. That is a useful
control, but not the current render contract because the compiler emits one generated class per
semantic style list.

## Coverage and determinism

The harness proves 44/44 class-value rewrites, coverage of all eight variant names, 31 collision-free
manifest entries and preservation of the tracked fixture outside class values. All thirty measured
processes emitted the same CSS hash:

```text
11f632e3e4321ab93a2c8df77e7ad1ce6034f912cbec48e62b92654f01a700fe
```

The measured release executable SHA-256 was
`d3dce60245d4f9f508298ef8f5f245d3d0e3f7d60de3ae9366cdb6cf64b4551c`.

This demonstrates deterministic compilation on this executable and machine. A separate Chromium
smoke validated computed styles at 375, 768, and 1440 pixels plus focus, disabled, placeholder, and
dark behavior. Visual parity, a multi-browser matrix, reset equivalence, cross-platform determinism,
and stability across compiler versions remain open. See
[`browser-validation.md`](./browser-validation.md).

## Reproduction

```console
pnpm baseline:measure-pliego-b
```

The local machine-readable result is written to `benchmarks/results/pliego-gate-b.local.json` and
remains gitignored. Frozen Tailwind figures come from the 4.3.2 complete profiles documented in
[`tailwind-v4-baseline.md`](./tailwind-v4-baseline.md). The immutable clean-commit result is
[`pliego-gate-b-2026-07-13-c47239c.json`](../../benchmarks/evidence/pliego-gate-b-2026-07-13-c47239c.json).
