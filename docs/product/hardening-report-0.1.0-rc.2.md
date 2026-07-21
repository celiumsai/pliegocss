<!-- SPDX-License-Identifier: Apache-2.0 -->

# PliegoCSS 0.1.0-rc.2 hardening report

**Status:** corrections implemented; RC.2 registry replay passed; final
`0.1.0` promotion remains blocked by its owner-controlled release gate
**Source basis:** corrected public-preview source prepared after
`aabf2b48ae53ee988d6f0ceff8276988668bbd31`
**Authority:** the machine-readable
[`release-readiness-0.1.0.json`](./release-readiness-0.1.0.json) controls the
promotion state

This report records the complete pre-release audit closure without turning a
local working tree into a release claim. It separates what was reproduced on
the current source from older hosted evidence and from work that can only exist
after a reviewed commit.

## Corrected findings

| Original severity | Finding | Correction | Current evidence |
| --- | --- | --- | --- |
| P1 | A generated `box-shadow` lost the effective `!important` contract of `shadow-*` and `ring-*` contributors. | The composed physical declaration now becomes important whenever an effective contributor is important; physical lineage records the same result. | Focused emitter, lineage, CLI, and Chrome cascade tests pass. |
| P1 | Release readiness enforced a stale private/pre-push photograph instead of deriving current blockers. | Readiness schema 2 models measured, inherited, pending, and uncertain evidence and derives `ready` or `blocked` from required checks. | Contract tests and the readiness checker pass. |
| P1 | The frozen media-query benchmark had drifted and CI disabled it. | Frozen hashes and sizes were refreshed from the corrected compiler; CI and the release profile run both media merging and reachability pruning. | Both benchmark checks pass on current source. |
| P1 | Historical benchmark snapshots referenced unreachable commit `c47239c`, while CI normally skipped the verifier. | CI always runs the verifier. The invalid snapshot set was superseded by three immutable snapshots from reachable clean commit `d16fe5d`. | The verifier validates the explicit filename/SHA-256 allowlist, exact Git blobs, environment declarations, statistics, and profile contracts. |
| P1 | The PliegoRS fixture depended on an unpublished local revision and stale `0.0.0` locks. | Fixtures pin public PliegoRS `0.0.2`, use registry-replayable locks, declare `project.id`, and require Rust 1.86 where the public framework requires it. | WSL SSG, browser resumability, route CSS, and development-loop checks pass against remote-reachable `f3f4eb9`. |
| P2 | Migration apply and rollback had a preflight-to-rename race that could overwrite concurrent edits. | Destination-scoped locks and adjacent revalidation were added before replace/remove; grouped operations compensate when any destination drifts. | Unit, CLI, corpus, and adversarial concurrent-edit tests pass. |
| P2 | LSP compiler children had no deadline and read unbounded output. | Catalog, hover, and diagnostic children have deadlines, cancellation, process-tree termination, and bounded stream capture. | Timeout and over-output tests pass. |
| P2 | Repair verification could hold its lock while unbounded child processes ran. | Cargo, browser, and toolchain probes now have deadlines, bounded capture, and process-tree termination while preserving authorization and rollback. | Repair-agent process-bound and verification tests pass. |
| P2 | `pliego-cssc --help` was a five-line migration note, not usable CLI help. | The CLI has a real synopsis, command inventory, global options, examples, and command help routing. | Parser/help contract tests and executable smoke checks pass. |
| P2 | Package documentation promised 62 KiB while code allowed 72 KiB; Windows converted paths into WSL form before invoking native `tar.exe`. | Documentation and the executable contract now agree on 72 KiB; native Windows paths stay native. | Nineteen archives are below budget and replay through the Rust 1.85 downstream fixture under WSL. |
| P2 | The DTCG watch regression was skipped in hosted CI. | The skip was removed and the macOS fallback is bounded to 100 ms polling. | Fifty repeated WSL runs pass; hosted run `29857699284` passes Ubuntu, Windows, and macOS. |
| P2 | `cargo-deny` had no policy and its license phase rejected every dependency. | A versioned `deny.toml` governs licenses, bans, and sources. `cargo-audit` owns advisories so CI does not fetch and interpret two separate advisory databases. | `cargo audit --deny warnings` reports zero advisories; `cargo deny check licenses bans sources` passes. |

## Product, brand, and website closure

- The repository now includes a versioned brandbook, DTCG source tokens, CSS
  projections, primary/reversed/monochrome symbols, lockups, app icon, favicon,
  social card, licensed local fonts, image art-direction prompts, and an
  executable brand contract.
- The README has the logo, project links, CI, CodeQL, release, license, MSRV,
  public-preview, public-repository, crates.io, and docs.rs badges.
- The site is authored and rendered by PliegoRS `0.0.2`; styles and the
  bounded laboratory corpus are compiled by the current PliegoCSS binary.
- The generator emits 90 routes and 134 content files; the release artifact
  contains 135 files after adding Cloudflare's `_headers` policy, including
  English and Spanish parity, categorized documentation, command search,
  generated utility catalog,
  playground, working examples, benchmark and brand surfaces, the complete
  legal register, security, accessibility, and changelog.
- The homepage uses GSAP, ScrollTrigger, Lenis, and Three.js as progressive
  enhancement. Phone layouts keep essential hero and workbench content visible
  without waiting for scroll animation; reduced-motion and no-WebGL paths
  remain readable.
- The laboratory exposes 144 real compiler recipes, three explanations, and
  three conflict cases. It does not simulate arbitrary Rust compilation in
  JavaScript.
- The Cloudflare Workers Static Assets profile is custom-domain-only:
  `workers.dev` and preview URLs are disabled, `_headers` defines the edge
  security/cache contract, and the local Wrangler response replay validates
  redirects, 404s, HTML, immutable assets, and `security.txt` without
  publishing anything.

## Validation matrix

| Surface | State | Evidence class | Result |
| --- | --- | --- | --- |
| Rust formatting and workspace check | Current working source / Windows | Measured | PASS |
| Workspace tests and doctests | Current working source / WSL | Measured | PASS |
| Clippy with `-D warnings` | Current working source / WSL | Measured | PASS |
| Targeted audit regressions | Current working source / Windows and WSL | Measured | PASS |
| Documentation graph | 164 Markdown files and 489 internal links | Measured | PASS |
| Brand contract | 25 canonical files, seven vector variants, two raster exports, fourteen token projections | Measured | PASS |
| Generated editorial images | Six reviewed GPT Image 2 masters plus AVIF/WebP derivatives and prompt provenance | Measured | PASS |
| PliegoRS integration | Public `0.0.2` contract | Measured | PASS |
| Website build | Two byte-identical PliegoRS builds | Measured | PASS |
| Website browser contract | Chrome 150, desktop + 390 px phone + reduced motion + EN/ES/legal/accessibility routes | Measured | PASS |
| Cloudflare deployment contract | Wrangler 4.110.0 dry run + local replay + Worker `c3f34e3d-43be-4f87-b07a-3cc9f946df4f` at `pliegocss.dev` + Browser Rendering | Measured | PASS |
| Site corpus | 144 recipes, three explanations, three conflicts | Measured | PASS |
| Package replay | Nineteen hosted archives plus registry-only Rust 1.85 downstream fixture | Measured | PASS |
| Dependency advisories | 105 dependencies | Measured | 0 advisories |
| Hosted OS matrix | Run `29857699284` at `42e0c22`; twelve jobs across Ubuntu, Windows, and macOS | Measured | PASS |
| Frozen benchmark evidence | Reachable clean commit `d16fe5d` | Measured | PASS |
| Registry replay | Nineteen crates plus CLI/LSP install and downstream execution from crates.io | Measured | PASS |
| Final `0.1.0` promotion | Owner approval | Pending authorization | BLOCKED |

## Remaining release sequence

1. Deploy the final public release status to `pliegocss.dev`.
2. Merge the reviewed release branch after its protected checks pass.
3. Create the immutable `v0.1.0-rc.2` tag and GitHub prerelease only after the
   final source commit has green hosted CI and CodeQL.
4. Promote final `0.1.0` only after separate owner authorization.

No step above is implied by the existence of an RC tag, the repository's
public visibility, the public-preview product stage, or this local hardening
pass.
