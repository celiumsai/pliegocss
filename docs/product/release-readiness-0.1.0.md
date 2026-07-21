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
  `pliegocss-site` version `c3f34e3d-43be-4f87-b07a-3cc9f946df4f` is deployed
  at `https://pliegocss.dev` through an enabled custom domain, Worker-managed
  apex DNS, and active Google Trust Services certificates. English, Spanish,
  documentation, legal, playground, security.txt, 404, CSP/security-header,
  immutable-asset cache, WebGL canvas, accessibility-tree, and RC.2 masthead
  contracts all pass against the production edge and Cloudflare Browser
  Rendering.
- All nineteen exact-version `0.1.0-rc.2` crates are visible on crates.io.
  Rust 1.85 installs `pliego-cssc` and `pliego-css-lsp` with `--locked`, resolves
  the downstream fixture exclusively from the crates.io registry, checks every
  target and feature, and executes the public API surface without local path or
  patch overrides.
- GitHub Actions run
  [29857699284](https://github.com/celiumsai/pliegocss/actions/runs/29857699284)
  passes the twelve-job Ubuntu, Windows, and macOS matrix at `42e0c22`;
  [CodeQL run 29857699053](https://github.com/celiumsai/pliegocss/actions/runs/29857699053)
  passes JavaScript/TypeScript, Rust, and Actions analysis on the same source.
- Final `0.1.0` promotion remains separately blocked and requires a new,
  explicit owner authorization.

## Remaining promotion work

1. merge the reviewed RC.2 source only after the protected branch checks pass;
2. retain a green CI and CodeQL matrix on the exact merged `main` commit;
3. create the immutable `v0.1.0-rc.2` tag and GitHub prerelease from that
   verified commit; and
4. keep final `0.1.0` promotion blocked until it receives separate explicit
   approval.

A local green gate never substitutes for hosted evidence, registry replay, or
release authorization.
