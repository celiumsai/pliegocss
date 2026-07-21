# PliegoCSS 0.1.0 release readiness

Status: **hardening in progress; final `0.1.0` promotion blocked**

The machine authority is
[`release-readiness-0.1.0.json`](./release-readiness-0.1.0.json). Its second
schema records each claim as **measured**, **inherited**, **pending**, or
**uncertain** and derives the overall result from checks required for
promotion. It no longer treats one obsolete release snapshot as a permanent
success condition.

## Current facts

- The canonical GitHub repository is public and its About panel points to
  `https://pliegocss.dev` with a searchable CSS, Rust, compiler, migration,
  token, LSP, WebAssembly, and static-analysis topic set. Public visibility
  does not by itself authorize promoting final `0.1.0`.
- The historical `v0.1.0-rc.1` tag and GitHub prerelease exist. That immutable
  tag points to an earlier commit, so this corrected source advances to
  `0.1.0-rc.2` rather than moving it.
- Hosted run
  [29767855672](https://github.com/celiumsai/pliegocss/actions/runs/29767855672)
  passed the Rust matrix on Windows, Ubuntu, and macOS at `aabf2b4`.
- That run is inherited evidence only: it predates the current corrections and
  skipped both media-query and historical-evidence checks.
- Focused tests for the confirmed audit findings pass locally.
- Gate A, Gate B, and paired Rust MSRV evidence were regenerated from reachable
  clean commit `d16fe5d`, hash-allowlisted, and verified against the exact Git
  harness and fixture blobs.
- The automated brand contract passes for the brandbook, symbols, lockups,
  raster dimensions, font licenses, token projection, six reviewed GPT Image 2
  masters, their AVIF/WebP derivatives, canonical prompt specifications, and
  generation manifest.
- Every fast-profile constituent passes on the current working source. Windows
  runs formatting, `cargo check`, and the JavaScript documentation/product
  gates; WSL runs the complete locked workspace tests, doctests, and Clippy
  with warnings denied. Local Windows Application Control error `4551` still
  prevents one-machine replay of newly linked test binaries, so fresh hosted
  Windows evidence remains independently required.
- The current PliegoRS integration passes in WSL against a remote-reachable
  revision and registry-replayable fixture locks.
- The expanded PliegoRS-authored public-preview website now has 90
  English/Spanish routes, legal and accessibility pages, reviewed brand
  imagery, and the ordered masthead/footer contract. It passes deterministic
  byte-for-byte rebuild, ledger and compiler-corpus integrity, gzip budgets,
  language and legal parity, generated-image delivery, computed styles,
  WebGL, playground, keyboard tabs, reduced motion, operations, command search,
  mobile layout, and horizontal-overflow checks in Chrome 150 on the current
  working source.
- The Cloudflare Workers Static Assets profile passes a Wrangler 4.110.0 dry
  run, local edge-response replay, and production replay. Worker
  `pliegocss-site` version `718df1ee-7afb-48af-9496-a034f0b9bcc2` is deployed
  at `https://pliegocss.dev` through an enabled custom domain, Worker-managed
  apex DNS, and active Google Trust Services certificates. English, Spanish,
  documentation, legal, playground, security.txt, 404, CSP/security-header,
  immutable-asset cache, WebGL canvas, accessibility-tree, and RC.2 masthead
  contracts all pass against the production edge and Cloudflare Browser
  Rendering.
- All nineteen crate archives package below the 72 KiB compressed limit and
  replay through a Rust 1.85 downstream fixture under WSL. This measurement
  allowed the reviewed dirty worktree and therefore must be repeated on the
  final clean commit before promotion.
- The owner authorized publishing the nineteen `0.1.0-rc.2` crates after all
  applicable CI passes. Final `0.1.0` promotion remains separately blocked.

## Remaining promotion work

1. retain GitHub Actions run `29850566211` as the hosted Windows, Ubuntu, and
   macOS authority and CodeQL run `29850565624` attempt 2 as the hosted static
   analysis authority; and
2. finish publishing the authorized `0.1.0-rc.2` compatibility unit, replay
   installation from crates.io, and keep final `0.1.0` promotion blocked until
   it receives separate explicit approval.

A local green gate never substitutes for hosted evidence, registry replay, or
release authorization.
