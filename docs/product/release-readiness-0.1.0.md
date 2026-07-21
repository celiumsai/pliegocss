# PliegoCSS 0.1.0 release readiness

Status: **hardening in progress; final `0.1.0` promotion blocked**

The machine authority is
[`release-readiness-0.1.0.json`](./release-readiness-0.1.0.json). Its second
schema records each claim as **measured**, **inherited**, **pending**, or
**uncertain** and derives the overall result from checks required for
promotion. It no longer treats one obsolete release snapshot as a permanent
success condition.

## Current facts

- The canonical GitHub repository exists and remains private while its product
  posture is prepared as a public preview. This does not authorize changing
  repository visibility, publishing crates, or promoting final `0.1.0`.
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
  run and local edge-response replay. It enforces the public custom-domain-only
  route, trailing-slash/404 behavior, CSP, HSTS, framing, MIME-sniffing,
  referrer, feature, cross-origin, security.txt, and immutable-asset contracts.
  The Cloudflare account currently has the active `pliegocss.dev` zone but no
  `pliegocss-site` Worker and no apex or `www` DNS record; deployment and DNS
  creation remain unperformed owner-authorized operations.
- All nineteen crate archives package below the 72 KiB compressed limit and
  replay through a Rust 1.85 downstream fixture under WSL. This measurement
  allowed the reviewed dirty worktree and therefore must be repeated on the
  final clean commit before promotion.
- The owner authorized publishing the nineteen `0.1.0-rc.2` crates after all
  applicable CI passes. Final `0.1.0` promotion remains separately blocked.

## Remaining promotion work

1. repeat the successful nineteen-crate package replay without `--allow-dirty`
   on the final clean source;
2. push the measured source and evidence commits and obtain fresh hosted
   cross-OS results;
3. use the fresh hosted Windows run as the Windows authority because local
   Application Control blocks newly linked test binaries;
4. after explicit owner approval, deploy the reviewed site to Cloudflare and
   bind `pliegocss.dev`, then verify production response headers, navigation,
   English/Spanish/legal parity, browser interaction, and Core Web Vitals; and
5. publish the authorized `0.1.0-rc.2` compatibility unit, replay installation
   from crates.io, and keep final `0.1.0` promotion blocked until it receives
   separate explicit approval.

A local green gate never substitutes for hosted evidence, registry replay, or
release authorization.
