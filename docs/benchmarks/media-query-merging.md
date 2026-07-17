# Adjacent media-query merging

Date: 2026-07-13

Status: adopted for exactly equal adjacent `@media` siblings

PliegoCSS now combines consecutive media rules when their parsed Lightning CSS `MediaList` values
are exactly equal. The pass is deliberately narrower than semantic media-query equivalence:

```css
@media (width >= 48rem) { .a { display: block } }
@media (width >= 48rem) { .b { display: flex } }
```

becomes:

```css
@media (width >= 48rem) {
  .a { display: block }
  .b { display: flex }
}
```

A qualified rule, a different query, or any other sibling is a hard boundary. Child rules retain
their original order, so the pass does not move declarations across a cascade boundary. Nested
media lists use the same rule after their parents are merged. The pass does not merge `@supports`,
`@container`, `@layer`, reordered query lists, or merely equivalent boolean expressions.

## Deterministic size fixture

The tracked fixture contains 20 real `md:*` styles and isolates the repeated-wrapper cost. The
control compiles every style independently in the aggregate manifest's canonical style order. The
candidate compiles the same styles together. The harness proves that the candidate is exactly one
wrapper around the unchanged ordered rule bodies before measuring bytes.

The same deterministic values were reproduced with Node 22.13/24.16 and Rust 1.85/1.96 debug
compilers. A local report records the exact Node, zlib, and compiler-binary hash:

| Profile | Control raw | Merged raw | Raw saved | Control gzip | Merged gzip | Gzip saved |
|---|---:|---:|---:|---:|---:|---:|
| Utilities only | 1,449 B | 1,012 B | 437 B (30.159%) | 652 B | 639 B | 13 B (1.994%) |
| Seed theme + utilities | 1,820 B | 1,383 B | 437 B (24.011%) | 822 B | 809 B | 13 B (1.582%) |

The adoption gate requires an exact rule sequence, 20 wrappers reduced to one, a raw reduction, and
at least 8 bytes and 1% gzip reduction in the theme profile. These are deterministic size and hash
checks, not timing samples.

The check recompiles the exact frozen Gate A and Gate B inputs and requires their established CSS
hashes, raw sizes, gzip sizes, and wrapper counts. Gate A contains no media rules. Gate B contains 12
media wrappers but no equal wrappers that are physically adjacent, so this pass is byte-neutral for
both comparison fixtures. It
is useful for responsive-only style runs; it is not evidence that every application becomes 1.582%
smaller.

Run the machine check without writing a result:

```console
pnpm check:media
```

Without `PLIEGO_CSSC`, the harness builds `pliego-cssc` with Rust 1.96 in an isolated target. Set
`PLIEGO_CSSC` to an already built executable when the host's application-control policy forbids new
executables in that directory. The output contract is still checked against reviewed exact hashes;
the override's binary hash and source mode are recorded rather than treated as release provenance.

Write the ignored machine-local JSON, including input/compiler hashes and runtime versions, with:

```console
pnpm baseline:measure-media
```

The fixture is
[`benchmarks/media-merge/adjacent-md.styles.txt`](../../benchmarks/media-merge/adjacent-md.styles.txt),
and the harness is [`scripts/measure-media-merge.mjs`](../../scripts/measure-media-merge.mjs).

## Physical trace contract

Manifest schema 5 reconciles the already-merged AST rather than the emitter's pre-optimization
wrapper layout. Qualified-rule order and declaration lineage remain unchanged; two former wrappers
therefore become one physical media parent with both qualified children. The trace gate covers
schema 3/4/5 CSS equality, `modern`/`none`, pretty/minified output, exact graph-1 projection, and
fail-closed physical reconciliation for this case.

This optimization does not select an atomic, grouped, or hybrid global output strategy. That wider
comparison, automatic route/island splitting, and critical CSS remain open F7 work. Theme-variable
pruning is measured by the separate opt-in reachability benchmark, not by this media-merge fixture.
