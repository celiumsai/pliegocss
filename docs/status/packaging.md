# Packaging status

Status: **CSS Modules consumer inventory passed the complete exact-clean `7f3b722` replay; no
registry upload performed**

The publishable workspace boundary has six dependency waves. `pliego-css-usage` and
`pliego-css-control` follow `pliego-css-build` because direct and optional Cargo dependencies both
affect registry publication order.

| Wave | Packages |
|---|---|
| 1 | `pliego-css-ir`, `pliego-css-cascade`, `pliego-css-ownership`, `pliego-css-source` |
| 2 | `pliego-css-parser`, `pliego-css-theme` |
| 3 | `pliego-css-config`, `pliego-css-compiler` |
| 4 | `pliego-css-build`, `pliego-css-macros` |
| 5 | `pliego-css-agent`, `pliego-css-usage`, `pliego-css-control`, `pliego-css` |
| 6 | `pliego-cssc` |

The exact dependency-first order is enforced by `scripts/check-packages.mjs` and documented in the
[release process](../contributing/release-process.md).

## CSS Modules consumer-inventory clean replay — 2026-07-16

The exact clean `7f3b722` Debian WSL2 gate packaged and extracted all fifteen archives with Cargo
1.96, compiled the extracted graph in release mode, and passed the registry-shaped downstream
consumer with Rust 1.85. Publication remained disabled, the worktree was clean, and the fixed
61,440-byte ceiling was not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-source` | 40,697 | 20,743 | `3b86e310148033b7d354ac428579e9b569cd00288be2eb26e32551abcdefc0da` |
| `pliego-cssc` | 61,390 | 50 | `53568a729136d0b1df548a4be9d6a1ca9779834776bddad766e871cc57767e37` |

Project inventory now links declared CSS Modules consumers to declared module sources and reports
exact property usages separately from dynamic or escaping bindings. The scanner remains lexical
and execution-free: it does not run JavaScript, TypeScript, a bundler, or application code. This
gate does not yet inspect Tailwind configuration/plugin/template contents, provide a representative
migration corpus, or close R0.8. The CLI package remains closed to additive feature growth.

## Tailwind auxiliary-seam clean replay — 2026-07-16

The exact clean `39167ac` Debian WSL2 gate packaged and extracted all fifteen archives with Cargo
1.96, compiled the extracted graph in release mode, and passed the registry-shaped downstream
consumer with Rust 1.85. Focused source tests, strict Clippy/rustdoc, and the versioned migration CLI
fixture also passed. Publication remained disabled and the fixed 61,440-byte ceiling was not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-source` | 37,187 | 24,253 | `92e92be4a631511cd34583bc8a06e8a8dbc828df4dca83733b937bbef776d85f` |
| `pliego-cssc` | 61,387 | 53 | `ce1e429bc397bfa3fe6423372945ca15cf073e7cb57d71448c8a4c22b9416c89` |

Project snapshots now retain Tailwind `@config`, `@plugin`, and `@source` ownership/discovery seams
as external, unresolved, or dynamic observations without executing their toolchain. This does not
yet inspect the referenced auxiliary files, discover templates/consumers, or close R0.8. The CLI
package remains closed to additive feature growth.

## Cross-toolchain migration fixture clean replay — 2026-07-16

The exact clean `29268cf` Debian WSL2 gate packaged and extracted all fifteen archives with Cargo
1.96, compiled the extracted graph in release mode, and passed the registry-shaped downstream
consumer with Rust 1.85. The versioned CLI integration fixture passed independently under Debian
WSL2 and covers six declared sources across Sass, Tailwind CSS v4, and CSS Modules. Publication
remained disabled and the fixed 61,440-byte ceiling was not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-source` | 36,794 | 24,646 | `7743a3af168ccecafa01f945959b26f6303bd4c7d14f644fe5d2c40959d2cad9` |
| `pliego-cssc` | 61,382 | 58 | `b559613dba5a4f7288f15ee3c6b91cecc8f1ca35585e78f804f01db70011400d` |

This freezes resolved Sass/CSS/CSS Modules edges, local composition, package/built-in external
references, extensionless Sass unresolved lookup, and an unsupported plugin seam in one real file
tree. It is a representative contract fixture, not yet the required corpus of migrated
applications. The CLI package remains effectively closed to additive growth.

## Migration project CLI clean replay — 2026-07-16

The exact clean `dde0d69` Debian WSL2 gate packaged and extracted all fifteen archives with Cargo
1.96, compiled the extracted graph in release mode, and passed the registry-shaped downstream
consumer with Rust 1.85. Focused Debian WSL2 testing also passed 30 `pliego-css-source` tests, 70
CLI unit tests, and every `pliego-cssc` integration target. Publication remained disabled and the
fixed 61,440-byte ceiling was not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-source` | 36,796 | 24,644 | `200b0df77dc1c48d037aa47e44dfcd09b969d3fc1940cdc668f280124d554821` |
| `pliego-cssc` | 61,391 | 49 | `b18b1f31dfbfd7beea92d8e7ee2b5458a254801e5d29090876748b28a89fc442` |

`pliego-cssc migration-project-inventory` now safely loads the bounded declaration, collects the
declared source set, and emits only canonical stdout. With 49 compressed bytes remaining, further
CLI growth is blocked on a structural size reduction or executable split; future R0.8 analysis must
remain in `pliego-css-source` unless that package boundary changes deliberately. This gate does not
close crawling, source-toolchain-specific resolution, configs/plugins/templates, downstream
composition consumers, real migration fixtures, registry publication, or R0.8 completion.

## Migration project declaration clean replay — 2026-07-16

The exact clean `003e276` Debian WSL2 gate packaged and extracted all fifteen archives with Cargo
1.96, compiled the extracted graph in release mode, and passed the registry-shaped downstream
consumer with Rust 1.85. The consumer parses the bounded schema-1 declaration through the extracted
`pliego-css-source` API. Publication remained disabled and the fixed 61,440-byte ceiling was not
raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-source` | 36,511 | 24,929 | `46ca7f93919a88953efc6d27a68b488f7a364f5049131e431b928a8264568728` |
| `pliego-cssc` | 61,253 | 187 | `7228cfe6084d82c8ef80b9f81774bce435c37b403fbcabb17228a98b40ea4d18` |

This closes packaging for a reviewable persisted declaration of the exact typed source set. It does
not close safe declaration-file loading, project CLI/crawling, source-toolchain-specific resolution,
configs/plugins/templates, downstream composition consumers, real migration fixtures, registry
publication, or R0.8 completion.

## Declared migration dependency clean replay — 2026-07-16

The exact clean `270eaa2` Debian WSL2 gate packaged and extracted all fifteen archives with Cargo
1.96, compiled the extracted graph in release mode, and passed the registry-shaped downstream
consumer with Rust 1.85. The consumer compile-checks the dependency DTOs, classifications, byte
ranges, targets, and immutable project accessor. Publication remained disabled and the fixed
61,440-byte ceiling was not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-source` | 35,781 | 25,659 | `35b6a5e764ea556ca2358f8329870007c4d96fccdedf663c616b56b4d7c024e6` |
| `pliego-cssc` | 61,256 | 184 | `4f1c1239df6870928d909b7aa2ca90a77678cf8f24a9c85b7d13fa3e6b5eed42` |

This closes packaging for conservative dependency observations across declared Sass, Tailwind/CSS,
and CSS Modules sources. Exact supported local targets fail closed when absent or mistyped;
external, unresolved, local, and dynamic observations remain explicit. It does not close project
CLI/crawling, source-toolchain-specific resolution, configs/plugins/templates, downstream
composition consumers, real migration fixtures, registry publication, or R0.8 completion.

## Confirmed migration project snapshot clean replay — 2026-07-16

The exact clean `992c33e` Debian WSL2 gate packaged and extracted all fifteen archives with Cargo
1.96, compiled the archive graph in release mode, and passed the registry-shaped downstream
consumer with Rust 1.85. The consumer compile-checks the new project declaration and immutable
snapshot types together with the single-file migration surface. Publication remained disabled and
the fixed 61,440-byte ceiling was not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-source` | 32,367 | 29,073 | `260de25e6e19d7dc58aef290aba2779e66e072dd6b77bc1673fd68bebba73f70` |
| `pliego-cssc` | 61,266 | 174 | `cd028ee313d286def0f91719980c7a23d66fd17d7cd38cbd2cf1509f66d78492` |

This closes packaging for the core declared-file snapshot: canonical input order, duplicate and
limit rejection, per-file safe reads, and two equal complete inventory passes. It does not close
the future project CLI or prove import/config/plugin/template/composition edges, atomic filesystem
snapshots, real migration projects, registry publication, or R0.8 completion.

## Migration inventory CLI clean replay — 2026-07-16

The exact clean `ba04e87` Debian WSL2 gate ran under Node.js 20.19.2, packaged and extracted all
fifteen archives with Cargo 1.96, compiled the archive graph in release mode, and passed the
registry-shaped downstream consumer with Rust 1.85. The consumer now compile-checks both exact-byte
and safe-file migration inventory entry points in addition to the established facade, collector,
ownership, usage, DTCG, TOML, control, and repair surfaces. Publication remained disabled and the
fixed 61,440-byte ceiling was not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-source` | 30,941 | 30,499 | `8028369aaac007c57a7e1a852ad4ef253dbc55ee2c1a82717b36f051fe996cde` |
| `pliego-css-build` | 59,913 | 1,527 | `e8b29b52caa5b6da80e28df2c9342b4e01064ad0a724db9928f4884ec5ab09e4` |
| `pliego-css-control` | 61,075 | 365 | `6f498434978554135fe229402f9c80f5526f3331f92d6f7a3178750f53f5c78f` |
| `pliego-cssc` | 61,262 | 178 | `14e9bd410f1c26dca92348421348b9beeb6c7f8cbfabe7e175c1d74933f0cd7a` |

This closes the exact-commit package gate for the bounded single-file CLI slice. It proves
canonical inventory output for Sass, Tailwind CSS v4 entry CSS, and CSS Modules plus regular-file,
path, link, UTF-8, and size enforcement. It does not prove complete project snapshots, import or
composition resolution, template/config/plugin discovery, migration correctness, registry
publication, or R0.8 completion.

## Fixed Chromium browser evidence clean replay — 2026-07-16

The exact clean `9b93314` Debian WSL2 gate ran under Node.js 20.19.2, packaged and extracted all
fifteen archives with Cargo 1.96, compiled the complete archive graph in release mode, and passed
the registry-shaped downstream consumer with Rust 1.85. The consumer exercises the public fixed
browser-evidence types, parser, runner, profile, and schema 1.4 constants in addition to fixed Rust
test evidence and the established standard CSS, token-graph, budget, adjacent FindingDocument,
facade, collector, ownership, usage, DTCG, and TOML surfaces. Canonical schema 1.0, 1.1, 1.2, and
1.3 policy/receipt documents remain readable under their original kind limits. Publication
remained disabled and the fixed 61,440-byte ceiling was not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-agent` | 57,286 | 4,154 | `025c5f9e28be0c1dbe997767b96b6a3e9bdde55979e3b0e3258d4638da008aca` |
| `pliego-css-build` | 59,917 | 1,523 | `d790ae0d447b6fb32cff760174e9c065334e523582eb2b69c23db9cf7c3c13b3` |
| `pliego-css-control` | 61,078 | 362 | `8cadd3c340d7cdd79f1e1957c332db4327f187bb359008c4c82bab8ec687e061` |
| `pliego-cssc` | 61,302 | 138 | `c9413f40a426efdb5599aa0d2be296805750c5498d9f8399fcb072790301b969` |

This closes the exact-commit package gate for `run-browser`, canonical browser evidence, and its
static `browser-evidence` verifier. The runner executes only the fixed PliegoRS visit-counter CDP
profile; it binds the Change Receipt, changed after-sources, eight profile inputs, Node identity,
Chromium/CDP identity, DOM object identity, state transition, network observations, and errors.
The first replay attempt exhausted the bounded WSL `/tmp`; cleaning only named PliegoCSS targets
restored 7.7 GiB and the unchanged commit then passed. This local evidence does not authenticate
the Node or Chrome executable, prove unchanged inventory sources, provide an operating-system
sandbox, or prove Firefox, WebKit, hosted runners, signed provenance, visual/accessibility
regression coverage, registry publication, or R1 completion.

## Fixed Rust test evidence clean replay — 2026-07-16

The exact clean `bb78e60` Debian WSL2 gate ran under Node.js 20.19.2, packaged and extracted all
fifteen archives with Cargo 1.96, compiled the complete archive graph in release mode, and passed
the registry-shaped downstream consumer with Rust 1.85. The consumer exercises the public fixed
test-evidence types, parser, and schema 1.3 constants in addition to the standard CSS, token-graph,
budget, adjacent FindingDocument, facade, collector, ownership, usage, DTCG, and TOML surfaces.
Canonical schema 1.0, 1.1, and 1.2 policy/receipt documents remain readable under their original
kind limits. Publication remained disabled and the fixed 61,440-byte ceiling was not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-agent` | 50,918 | 10,522 | `f66a4a9984e4c7ef4223f5c79dc67135719410f0bf74b955a8d39f32fc2b898c` |
| `pliego-css-build` | 59,907 | 1,533 | `85170e936555d2eb1911a06347bf885476a9a1cf01ee7b1278f6064b454d3044` |
| `pliego-css-control` | 61,073 | 367 | `ce0be0030b63272bb0b4a04b2b96139a9d4d7d8bcc8be633b01c8da935331265` |
| `pliego-cssc` | 61,300 | 140 | `089f825e3e02aa26642f93103b2576904feb046e61d8673aac014e319a9a56be` |

This closes the exact-commit package gate for the explicit `run-tests` process boundary and the
static `test-suite-evidence` verifier. The runner accepts no program, argument, shell, or policy
environment surface; it executes only `cargo test --workspace --all-targets --locked --offline`
and binds canonical evidence to the Change Receipt, check ID, toolchain, root manifest, and lockfile.
Offline Cargo is not an operating-system sandbox: project tests and build scripts still execute
project code, and this local evidence does not prove hermetic execution, signed provenance, real
browser evidence inside the repair receipt, hosted runners, multi-browser execution, registry
publication, or R1 completion.

## CSS budget repair verification clean replay — 2026-07-16

The exact clean `d7848e6` Debian WSL2 gate ran under Node.js 22.13.0, packaged and extracted all
fifteen archives with Cargo 1.96, compiled the complete archive graph in release mode, and passed
the registry-shaped downstream consumer with Rust 1.85. The consumer exercises the public
`css-budget-audit` kind, `RepairVerificationInputs`, and schema 1.2 constants in addition to the
standard CSS/token checks, adjacent FindingDocument results, and previous facade, collector,
ownership, usage, DTCG, and TOML surfaces. Canonical schema 1.0 and 1.1 policy/receipt documents
remain readable under their original kind limits. Publication remained disabled and the fixed
61,440-byte ceiling was not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-agent` | 44,847 | 16,593 | `0c50c8e2af653588c09d0db74d2512a6bc27fb1a1b9f12506550f249928f4ba7` |
| `pliego-css-build` | 59,907 | 1,533 | `50e96e8116260b56588853113aeb5d6ef3bddcbba4b1904fa69497bc4e20d8c7` |
| `pliego-css-control` | 61,071 | 369 | `68f2916ea3e7bfed111fa478eb03146de22655a34817280315a0f65bfbe9c1e1` |
| `pliego-cssc` | 61,299 | 141 | `c91dd15c4bef7095c812b6c14e24e764c20d71e4b6e671cf94e2f36f1f651a3a` |

This closes the exact-commit package gate for one changed CSS artifact evaluated against an exact
canonical budget policy plus optional package/route subjects. Supplementary policy files are
path/byte/hash bound, reject links and aliases, and are read twice under the repair lock; file and
layer subjects remain automatic. This evidence does not prove a test-suite check kind, real browser
evidence inside the repair receipt, hosted runners, multi-browser execution, registry publication,
or R1 completion.

## Token-graph repair verification clean replay — 2026-07-16

The exact clean `1ba869c` Debian WSL2 gate ran under Node.js 22.13.0, packaged and extracted all
fifteen archives with Cargo 1.96, compiled the complete archive graph in release mode, and passed
the registry-shaped downstream consumer with Rust 1.85. The consumer exercises the public
`token-graph-integrity` check kind and schema 1.1 constants in addition to the policy/receipt
parsers, adjacent FindingDocument results, and previous facade, collector, ownership, usage, DTCG,
and TOML surfaces. Schema 1.0 standard-CSS policy and receipt documents remain readable while new
documents are emitted as 1.1. Publication remained disabled and the fixed 61,440-byte ceiling was
not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-agent` | 41,494 | 19,946 | `4262347f9500d93e4fbf0c6108de42c66ed6db55f8f634943dab232f8401e105` |
| `pliego-css-build` | 59,914 | 1,526 | `4e75e146540834f8846e2a50a16c17c5c4a2416612813b292f150c2cec74ba89` |
| `pliego-css-control` | 61,081 | 359 | `87e944d36c6c98748795c95a7f500cfd3b662906346f0fbbed6d3f5b383016c2` |
| `pliego-cssc` | 61,306 | 134 | `a4e606bf1ff60102cca854393343efe5be8b6278f44e834d486f45c4277da41d` |

This closes the exact-commit package gate for canonical token-graph verification after an
authorized repair. The check parses in-process, emits a canonical passed or failed FindingDocument,
and does not invoke a shell, filesystem resolver, network, or browser. This evidence does not prove
budget verification, real browser evidence inside the repair receipt, hosted runners, multi-browser
execution, registry publication, or R1 completion.

## Adjacent repair evidence clean replay — 2026-07-16

The exact clean `9e524a2` Debian WSL2 gate ran under Node.js 22.13.0, packaged and extracted all
fifteen archives with Cargo 1.96, compiled the complete archive graph in release mode, and passed
the registry-shaped downstream consumer with Rust 1.85. The consumer exercises the public adjacent
FindingDocument result surface in addition to the policy/receipt parsers and previous facade,
collector, ownership, usage, DTCG, and TOML surfaces. Publication remained disabled and the fixed
61,440-byte ceiling was not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-agent` | 38,956 | 22,484 | `ea967323eeaa19df917f1861c34d3d66f47ae5e57e62f834144e79d3fe9bc3ac` |
| `pliego-css-build` | 59,906 | 1,534 | `869884f0d64fce0005a57c5d3945ae828cc308858f896770a566459a4dde9ec8` |
| `pliego-css-control` | 61,075 | 365 | `b6eced8eb59572286f3f1e4a6a4bd1064c5647ea2191b2b5145e0f60cbbd7931` |
| `pliego-cssc` | 61,294 | 146 | `1eef4da45fc60d7425ad5b18c2e7882e1327e0a0f2d2c121b4650664f4db4390` |

This closes the exact-commit package gate for complete per-check FindingDocument publication with
create-if-absent staging, receipt-last ordering, collision rejection, replay, and rollback. It does
not prove the later token-graph or budget checks, real browser evidence, hosted runners, multi-browser
execution, registry publication, or R1 completion.

## Built-in repair verification clean replay — 2026-07-16

The exact clean `cdf57dc` Debian WSL2 gate ran under Node.js 22.13.0, packaged and extracted all
fifteen archives with Cargo 1.96, compiled the complete archive graph in release mode, and passed
the registry-shaped downstream consumer with Rust 1.85. The consumer exercises the public check
policy and Verification Receipt parsers in addition to the previous Change Receipt, facade,
collector, ownership, usage, DTCG, and TOML surfaces. Publication remained disabled and the fixed
61,440-byte ceiling was not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-agent` | 36,964 | 24,476 | `a97502d8b38210fba85c17cf6730175e1bcd2831a5e773cb54e07d057b620a4c` |
| `pliego-css-build` | 59,909 | 1,531 | `6b29d3e7935e696f681d0b111c08db582d5fc6e8b7c680f5aec0a82911b92a5c` |
| `pliego-css-control` | 61,075 | 365 | `4b814fc7f01cf0d2a4f5d454ea7ae821351552b61464d3df6c47dd7d8c282c93` |
| `pliego-cssc` | 61,300 | 140 | `d12e15f870b27b195ed9a60676148b80b8809386a0a2ace549bb6ef0e3a5a2c4` |

This closes the exact-commit package gate for the first closed built-in verification kind. It
predates adjacent after-FindingDocument publication and does not prove token-graph or budget checks,
real browser evidence, hosted runners, multi-browser execution, registry publication, or R1 completion. A
browser-required policy remains truthfully `blocked` until that evidence is collected.

## Authorized apply clean replay — 2026-07-16

The exact clean `29f8630` Debian WSL2 gate packaged and extracted the authorized apply candidate,
compiled all fifteen archives together with Cargo 1.96, and passed the twelve-package
registry-shaped downstream consumer with Rust 1.85. The consumer exercises the public Change
Receipt parser in addition to the established facade, collector, ownership, usage, DTCG, and TOML
surfaces. Publication remained disabled and the fixed 61,440-byte ceiling was not raised.

| Package | Compressed bytes | Remaining margin | SHA-256 |
|---|---:|---:|---|
| `pliego-css-agent` | 26,797 | 34,643 | `324c7485a6777e0423b8c2d645d1fe26b74f0c23baf4f442b0677c21926e7b87` |
| `pliego-css-build` | 59,918 | 1,522 | `8166bb58563ef5918af21d5f7763c64fa3de1b0600f497f8abecd9a19016d52a` |
| `pliego-css-control` | 61,077 | 363 | `188a8a45c4850256aa633dd8df84badd885ea4fbf4dbe5b9cc9a510b9ad63e24` |
| `pliego-cssc` | 61,299 | 141 | `457d9997dad8111a4c7ed121640a61f907825b6fe9422b17bd704fb184bf9db8` |

The CLI margin is deliberately reported as 141 bytes; no budget increase or hidden exclusion is
implied. This closes the local exact-commit package gate for the change-only apply boundary. It does
not prove required-check execution, final verification, hosted runners, or registry publication.

## Fifteen-crate agent clean replay — 2026-07-16

The exact clean `0905ded` Debian WSL2 gate packaged and extracted all fifteen candidate archives,
compiled them together in release mode with Cargo 1.96, and passed the registry-shaped downstream
consumer with Rust 1.85, including `pliego-css-agent` as a direct dependency. It did not upload
anything. This replay supersedes the fourteen-crate cascade snapshot below as the controlling local
package evidence.

The new package and current tight boundaries were:

| Package | Compressed bytes | Remaining margin |
|---|---:|---:|
| `pliego-css-agent` | 18,639 | 42,801 |
| `pliego-css-build` | 59,907 | 1,533 |
| `pliego-css-control` | 61,071 | 369 |
| `pliego-cssc` | 61,132 | 308 |

The clean gate retained the fixed 61,440-byte ceiling, verified the exact six-wave order, and ran
the extracted Rust 1.85 consumer with stable facade, DTCG, and TOML outputs. Repair CLI parsing lives
in the agent package so the process binary remains within that budget. This exact-clean replay
validates proposal/plan/dry-run schema 1.0.0 but predates the authorized apply candidate. The current
dirty candidate adds Change Receipt 1.0.0 and rollback-capable application; package creation alone
is not evidence that required checks ran or that R1 is complete.

## Fourteen-crate cascade clean replay — 2026-07-16

The exact clean `ae877a4` Debian WSL2 gate passed with Cargo 1.96.0 after extracting and compiling
all fourteen normalized archives together in release mode. The registry-shaped downstream fixture
then compiled and ran against those extracted packages with Rust 1.85. Publication remained
disabled.

The split preserved the fixed 61,440-byte archive ceiling instead of weakening it:

| Package | Compressed bytes | Remaining margin |
|---|---:|---:|
| `pliego-css-cascade` | 19,466 | 41,974 |
| `pliego-css-config` | 58,134 | 3,306 |
| `pliego-css-compiler` | 59,214 | 2,226 |
| `pliego-css-build` | 59,563 | 1,877 |
| `pliego-cssc` | 60,129 | 1,311 |
| `pliego-css-control` | 61,070 | 370 |

The gate also verified exact internal versions, README/license contents, package portability, the
six-wave publication order, and the public facade/build closure. This exact-clean replay supersedes
the previous thirteen-crate collector evidence as the release-controlling local package gate.

## Local evidence

On 2026-07-13, Windows x64 with Cargo 1.96.0 and Node.js 24.16.0 successfully:

- validated the fixed ten-package boundary and common `0.0.0` candidate version;
- verified exact internal `=0.0.0` requirements and the Rust 1.85 package MSRV;
- confirmed every archive includes its README and the complete Apache-2.0 `LICENSE`;
- rejected non-portable tracked/package paths and ran `cargo package --list` for all ten crates;
- packaged all normalized archives and compiled them together in release profile from an isolated
  verification workspace patched to the extracted `.crate` contents;
- resolved a registry-shaped downstream application through the eight-package facade/build closure,
  compiled and ran `Style`, `StyleId`, `pc!`, `pcx!`, and `theme!` with Rust 1.85, and rejected any
  internal PliegoCSS graph edge or manifest that escaped the extracted archives;
- enforced a maximum compressed archive size of 60 KiB (61,440 bytes) and kept every archive within
  that budget.

The clean `ff85319` determinism/fuzzing milestone was checked under Debian WSL2 with the required
Node.js 22.13.0, Cargo 1.96, and Rust 1.85 downstream toolchain. Its largest archives were
`pliego-cssc` at 59,520 bytes (1,920 bytes of margin) and `pliego-css-compiler` at 58,733 bytes
(2,707 bytes of margin). The gate extracted every archive and compiled the downstream fixture;
publication remained disabled. A native Windows rerun packaged the same source and compiled the
extracted archive graph, but local Application Control blocked the newly generated downstream
`build.rs` executable. The earlier clean `e6ff917` gate remains the last complete native-Windows
run. These local values are package contract checks, not final release evidence.

The clean `e9c9d25` reachability-pruning milestone passed the same extracted-package and downstream
gate under Debian WSL2. `pliego-cssc` was 60,320 bytes (1,120 bytes of margin),
`pliego-css-compiler` was 58,732 bytes (2,708 bytes of margin), and `pliego-css-build` was 28,143
bytes (33,297 bytes of margin). The CLI margin is now critical: subsequent artifact work must move
reusable parsing, validation, hashing, and schema generation into `pliego-css-build`, and the 60 KiB
ceiling must not be raised. Publication remained disabled.

The clean `6920b2c` framework-neutral Asset Plan milestone passed the extracted-package and
downstream gate under Debian WSL2. `pliego-cssc` was 58,149 bytes (3,291 bytes of margin),
`pliego-css-compiler` was 58,733 bytes (2,707 bytes of margin), and `pliego-css-build` was 39,820
bytes (21,620 bytes of margin). Moving reusable Asset Plan validation, hashing, and schema generation
into `pliego-css-build`, while keeping external CLI regression tests out of the registry archive,
recovered CLI margin without raising the fixed 60 KiB ceiling. The extracted Rust 1.85 downstream
consumer passed and publication remained disabled.

The clean `9cdfd1a` PliegoRS development-loop evidence milestone passed the same gate under Debian
WSL2 with Node.js 22.13.0 and Cargo 1.96. `pliego-cssc` was 58,147 bytes (3,293 bytes of margin),
`pliego-css-compiler` was 58,732 bytes (2,708 bytes of margin), and `pliego-css-build` was 39,813
bytes (21,627 bytes of margin). All ten extracted archives compiled together, the Rust 1.85
downstream facade/build consumer ran successfully, the worktree was clean, and publication remained
disabled.

The clean `c32ea61` Project Index milestone passed the same extracted-package gate under Debian
WSL2 with Node.js 22.13.0 and Cargo 1.96. `pliego-cssc` was 58,626 bytes (2,814 bytes of margin),
`pliego-css-compiler` was 58,733 bytes (2,707 bytes of margin), and `pliego-css-build` was 44,256
bytes (17,184 bytes of margin). The shared source-to-output schema and generator remain in the build
crate rather than duplicating project-model logic in the CLI. All ten archives compiled together,
the extracted Rust 1.85 downstream consumer ran, the worktree was clean, and publication remained
disabled.

The clean `ae59c3d` Baseline compatibility-policy milestone passed the complete native Windows gate
on 2026-07-14 with Cargo 1.96. `pliego-cssc` was 59,869 bytes (1,571 bytes of margin),
`pliego-css-compiler` was 58,729 bytes (2,711 bytes of margin), and `pliego-css-build` was 46,665
bytes (14,775 bytes of margin). All ten extracted archives compiled together, the registry-shaped
Rust 1.85 downstream consumer ran successfully, the worktree was clean, and publication remained
disabled. The CLI margin is again critical; the 61,440-byte ceiling remains fixed.

The clean `385914c` typed attribute/direction variant milestone passed the same native Windows gate.
`pliego-cssc` was 60,019 bytes (1,421 bytes of margin), `pliego-css-compiler` was 59,696 bytes
(1,744 bytes of margin), and `pliego-css-build` was 46,751 bytes (14,689 bytes of margin). The
compiler owns the classified-selector registry so the CLI does not duplicate it. The extracted Rust
1.85 consumer passed, the worktree was clean, and publication remained disabled. Further A3/A5
work must preserve both the compiler and CLI ceilings through crate-boundary discipline.

The clean `514bdb6` typed container-query milestone passed the complete native Windows gate with
Cargo 1.96 and the Rust 1.85 extracted downstream consumer. `pliego-cssc` was 60,013 bytes (1,427
bytes of margin), `pliego-css-compiler` was 59,906 bytes (1,534 bytes of margin), and
`pliego-css-build` was 47,697 bytes (13,743 bytes of margin). The new compiler integration and
physical-trace tests remain outside registry archives; the typed semantic implementation remains in
the owning crates. All ten archives compiled together, the worktree was clean, publication remained
disabled, and the 61,440-byte ceiling was not raised.

The clean `a4e8fb8` configurable typed-attribute milestone passed the same complete native Windows
gate. `pliego-cssc` was 60,010 bytes (1,430 bytes of margin), `pliego-css-compiler` was 59,985 bytes
(1,455 bytes of margin), and `pliego-css-build` was 47,750 bytes (13,690 bytes of margin). The
bounded dynamic selector grammar lives in `pliego-css-ir`; compiler integration tests remain
outside the registry archive. All ten archives compiled together, the Rust 1.85 extracted
downstream consumer ran, the worktree was clean, publication remained disabled, and the fixed
61,440-byte ceiling was preserved.

The clean `eb54b75` typed writing-mode milestone passed the same complete native Windows gate.
`pliego-cssc` was 60,009 bytes (1,431 bytes of margin), `pliego-css-compiler` was 60,280 bytes (1,160
bytes of margin), and `pliego-css-build` was 47,785 bytes (13,655 bytes of margin). The append-only
IR tags, emitter mapping, catalog descriptors, and policy classification remain in their owning
crates; black-box writing-mode tests remain outside the registry archive. All ten archives compiled
together, the Rust 1.85 extracted downstream consumer ran, the worktree was clean, publication
remained disabled, and the 61,440-byte ceiling was not raised.

The clean `edee59c` typed cascade-layer milestone passed the complete native Windows gate with Cargo
1.96 and the Rust 1.85 extracted downstream consumer. `pliego-cssc` was 60,084 bytes (1,356 bytes of
margin), `pliego-css-compiler` was 60,921 bytes (519 bytes of margin), and `pliego-css-build` was
48,542 bytes (12,898 bytes of margin). The fixed layer vocabulary lives in semantic IR; black-box
compiler and physical-trace tests remain outside registry archives. All ten archives compiled
together, the worktree was clean, publication remained disabled, and the 61,440-byte ceiling was not
raised. Compiler margin is now critical and must be recovered structurally before another semantic
dimension is added.

The clean `3876ebc` compiler-package compaction passed the same complete native Windows gate.
`pliego-css-compiler` fell to 59,232 bytes (2,208 bytes of margin), while `pliego-cssc` was 60,087
bytes (1,353 bytes of margin) and `pliego-css-build` was 48,542 bytes (12,898 bytes of margin).
Emitter behavior assertions moved from the published production source into the already excluded
black-box `emission_contract` target and were consolidated against public APIs; 15 external emission
tests plus all remaining compiler unit/integration/doc tests pass. No runtime code or coverage was
removed, all ten extracted archives and the Rust 1.85 downstream consumer passed, publication
remained disabled, and the size ceiling was unchanged.

The 2026-07-14 DTCG bridge development gate passed with `--allow-dirty` after regenerating the
Rust 1.85 public-API fixture lock for the new `serde_json` dependency. `pliego-css-config` was
22,666 bytes (38,774 bytes of margin), `pliego-css-compiler` was 59,219 bytes (2,221 bytes of
margin), `pliego-cssc` was 60,089 bytes (1,351 bytes of margin), and every other archive remained
below the same ceiling. All ten extracted archives compiled, and the Rust 1.85 registry-shaped
downstream consumer ran with the expected ThemeId/class output. This dirty-tree run is development
evidence only and must be repeated clean before release; no publication occurred.

The 2026-07-14 canonical-finding development gate passed with `--allow-dirty` under Debian WSL2.
All ten extracted archives compiled together and the registry-shaped downstream consumer ran on
Rust 1.85. `pliego-css-build` was 54,767 bytes (6,673 bytes of margin),
`pliego-css-compiler` was 59,222 bytes (2,218 bytes of margin), and `pliego-cssc` was 60,102 bytes
(1,338 bytes of margin). The build crate's contract tests remain tracked and run in workspace/CI
matrices, but are excluded from its registry archive just like the existing compiler and CLI
black-box suites. Publication remained disabled and the fixed ceiling was unchanged.

The 2026-07-14 standard-CSS audit ingestion gate also passed with `--allow-dirty` under Debian
WSL2. All extracted packages compiled and the Rust 1.85 downstream fixture ran. After adding the
audit engine and thin CLI surface, `pliego-css-build` was 57,330 bytes (4,110 bytes of margin),
`pliego-css-compiler` was 59,216 bytes (2,224 bytes of margin), and `pliego-cssc` was 60,967 bytes
(473 bytes of margin). The ceiling was not raised. The CLI margin is now an explicit engineering
constraint: later E1 commands require source/package compaction before adding substantial code.
Publication remained disabled.

The immediate CLI compaction gate then moved 53 internal tests from published `src/main.rs` into the
excluded integration harness without changing their private-code coverage. The complete CLI suite,
Clippy, Rust 1.85 check, all extracted packages, and the downstream fixture passed. `pliego-cssc`
fell to 44,839 bytes (16,601 bytes of margin), `pliego-css-build` was 57,324 bytes (4,116 bytes of
margin), and `pliego-css-compiler` was 59,214 bytes (2,226 bytes of margin). `package-verify` only
prevents the release all-target check from resolving excluded unit-test source; default workspace
tests execute all 53 cases. Publication remained disabled and the ceiling was unchanged.

The 2026-07-14 frozen compatibility-data and explainable-audit development gate passed with
`--allow-dirty` under Debian WSL2. The immutable Baseline snapshot and policy generator moved behind
the optional `pliego-css-config/compatibility-data` feature, so audit users receive the data without
duplicating it in the build crate's archive. `pliego-css-config` was 30,039 bytes (31,401 bytes of
margin), `pliego-css-build` was 58,697 bytes (2,743 bytes of margin), `pliego-css-compiler` was
59,218 bytes (2,222 bytes of margin), and `pliego-cssc` was 44,898 bytes (16,542 bytes of margin).
All ten extracted archives compiled, the Rust 1.85 downstream consumer ran, publication remained
disabled, and the 61,440-byte ceiling was unchanged.

The 2026-07-14 artifact-budget development gate also passed with `--allow-dirty` under Debian WSL2.
Canonical AST measurement moved behind `pliego-css-config/css-analysis` rather than duplicating the
engine in the finding adapter, and the build-crate README now delegates detailed audit contracts to
the versioned reference guides. `pliego-css-config` was 39,228 bytes (22,212 bytes of margin),
`pliego-css-build` was 60,812 bytes (628 bytes of margin), `pliego-css-compiler` was 59,215 bytes
(2,225 bytes of margin), and `pliego-cssc` was 45,548 bytes (15,892 bytes of margin). All ten
extracted archives compiled, the Rust 1.85 downstream consumer ran, publication remained disabled,
and the 61,440-byte ceiling was not raised.

The 2026-07-14 Asset Plan route-budget development gate passed with `--allow-dirty` under Debian
WSL2. Shared budget projection stayed in `pliego-css-build`; cross-artifact counter/fingerprint
aggregation stayed behind `pliego-css-config/css-analysis`; the CLI owns read-only ledger I/O and
canonical plan regeneration. `pliego-css-config` was 39,846 bytes (21,594 bytes of margin),
`pliego-css-build` was 60,771 bytes (669 bytes of margin), `pliego-css-compiler` was 59,216 bytes
(2,224 bytes of margin), and `pliego-cssc` was 49,367 bytes (12,073 bytes of margin). All ten
extracted archives compiled, the Rust 1.85 downstream consumer ran, publication remained disabled,
and the 61,440-byte ceiling was not raised.

The 2026-07-14 SARIF development gate passed with `--allow-dirty` under Debian WSL2, Node 24.16,
Cargo 1.96, and the Rust 1.85 downstream toolchain. Windows Application Control blocked execution of
a temporary dependency build script with OS error 4551, so the complete extracted-package gate was
rerun on Linux rather than represented as a product failure. `pliego-css-config` was 39,841 bytes
(21,599 bytes of margin), `pliego-css-build` was 60,775 bytes (665 bytes of margin),
`pliego-css-compiler` was 59,218 bytes (2,222 bytes of margin), and `pliego-cssc` was 51,245 bytes
(10,195 bytes of margin). All ten extracted archives compiled, the Rust 1.85 downstream consumer
ran, publication remained disabled, and the 61,440-byte ceiling was unchanged.

The 2026-07-14 control-artifact schema development gate passed with `--allow-dirty` under Debian
WSL2, Node 24.16, Cargo 1.96, and the Rust 1.85 downstream toolchain. The publishable boundary is
now eleven crates: `pliego-css-control` has no internal workspace dependencies and joins wave 1.
Its archive was 18,335 bytes (43,105 bytes of margin); `pliego-css-build` remained below the frozen
ceiling at 60,768 bytes (672 bytes of margin), `pliego-css-compiler` was 59,222 bytes (2,218 bytes
of margin), and `pliego-cssc` was 51,237 bytes (10,203 bytes of margin). All eleven extracted
archives compiled together, the unchanged eight-package downstream facade/build closure ran under
Rust 1.85, publication remained disabled, and the 61,440-byte ceiling was unchanged.

The 2026-07-14 direct-audit control-group development gate passed with `--allow-dirty` under the
same Debian WSL2, Node 24.16, Cargo 1.96, and Rust 1.85 toolchains. Canonical findings, control
manifest, and manifest-bound receipt now publish as one rollback-capable group; `--check` detects
byte drift without rewriting it, and the input ledger hashes the exact CSS and optional policy
bytes. `pliego-css-control` was 19,256 bytes (42,184 bytes of margin), `pliego-css-config` was
39,969 bytes (21,471 bytes of margin), `pliego-css-build` was 60,765 bytes (675 bytes of margin),
`pliego-css-compiler` was 59,219 bytes (2,221 bytes of margin), and `pliego-cssc` was 56,991 bytes
(4,449 bytes of margin). All eleven extracted archives compiled together, the eight-package
downstream facade/build closure ran under Rust 1.85, publication remained disabled, and the
61,440-byte ceiling was unchanged.

The 2026-07-14 Asset Plan audit-control development gate passed with `--allow-dirty` under Debian
WSL2 after Windows Application Control rejected a newly generated test executable with OS error
4551. Full workspace tests therefore ran with Linux binaries and a Linux target directory; this is
Debian/WSL2 evidence, not a generic native-Linux or native-Windows claim. The audit receipt now
binds the canonically regenerated Asset Plan, every adjacent CSS/style-manifest pair, and exact
policy bytes, with a required `asset-plan-integrity` check. `pliego-css-control` was 19,244 bytes
(42,196 bytes of margin), `pliego-css-config` was 39,956 bytes (21,484 bytes of margin),
`pliego-css-build` was 60,762 bytes (678 bytes of margin), `pliego-css-compiler` was 59,219 bytes
(2,221 bytes of margin), and `pliego-cssc` was 57,815 bytes (3,625 bytes of margin). All eleven
extracted archives compiled together, the eight-package downstream facade/build closure ran under
Rust 1.85, publication remained disabled, and the 61,440-byte ceiling was unchanged.

The 2026-07-14 bundle-build control-group development gate also passed with `--allow-dirty` under
Debian WSL2, Cargo 1.96, and the Rust 1.85 extracted downstream consumer. The orchestration script
ran under the distro's Node 20.19; this is extra local evidence and does not lower the supported
Node 22.13 floor. `bundle --control` now integrity-binds exact plan/config/reachability/source
snapshots plus generated CSS, style manifests, Asset Plan, optional Project Index, findings,
control manifest, and receipt in one output group. `pliego-css-control` was 19,244 bytes (42,196
bytes of margin), `pliego-css-config` was 39,959 bytes (21,481 bytes of margin),
`pliego-css-build` was 60,767 bytes (673 bytes of margin), `pliego-css-compiler` was 59,218 bytes
(2,222 bytes of margin), and `pliego-cssc` was 59,956 bytes (1,484 bytes of margin). All eleven
extracted archives compiled together, the eight-package downstream facade/build closure ran under
Rust 1.85, publication remained disabled, and the 61,440-byte ceiling was unchanged.

The 2026-07-14 control-projection boundary gate moved analyzer-to-receipt construction out of the
CLI package and behind the optional `pliego-css-control/projection` feature. Schema-only consumers
keep the feature disabled; publication order now places `pliego-css-build` before
`pliego-css-control`. The complete dirty-explicit gate passed under Debian WSL2 with Cargo 1.96 and
the extracted Rust 1.85 downstream consumer. `pliego-css-build` was 60,761 bytes (679 bytes of
margin), `pliego-css-control` was 28,937 bytes (32,503 bytes of margin), and `pliego-cssc` was
54,581 bytes (6,859 bytes of margin). All eleven extracted archives compiled together with all
features, publication remained disabled, and the ceiling was unchanged.

The 2026-07-14 exact-snapshot compile/watch control gate then passed the same complete
dirty-explicit Debian WSL2 package workflow. The new CLI surface publishes or checks CSS,
specialized manifest, findings, control manifest, and receipt as one group while retaining the
projection feature boundary. `pliego-css-build` was 60,762 bytes (678 bytes of margin),
`pliego-css-control` was 28,921 bytes (32,519 bytes of margin), and `pliego-cssc` was 59,219 bytes
(2,221 bytes of margin). All eleven extracted archives compiled together with all features, the
eight-package downstream closure passed Rust 1.85, publication remained disabled, and the
61,440-byte ceiling was unchanged.

The generated flat-token projection gate passed the complete dirty-explicit Debian WSL2 workflow
after adding measured registry identity and use coverage to bundle/compile/watch control groups.
`pliego-css-build` was 60,769 bytes (671 bytes of margin), `pliego-css-control` was 30,223 bytes
(31,217 bytes of margin), and `pliego-cssc` was 59,759 bytes (1,681 bytes of margin). The canonical
graph builder lives behind `pliego-css-control/projection`; moving it out of the CLI package restored
678 bytes of CLI margin without changing any frozen control hash. All eleven
extracted archives compiled together with all features, the eight-package downstream closure passed
Rust 1.85, and the complete workspace/all-targets checkout passed `cargo +1.85.0 check --locked`.
Publication remained disabled and the 61,440-byte ceiling was unchanged. Further CLI growth remains
constrained and requires explicit compaction or a reviewed package-boundary change.

The deterministic Source Map v3 gate passed the complete dirty-explicit Debian WSL2 workflow after
adding one integrity-bound map per generated CSS output to bundle, compile, and watch control
groups. `pliego-css-build` was 60,762 bytes (678 bytes of margin),
`pliego-css-compiler` was 59,221 bytes (2,219 bytes of margin), `pliego-css-control` was 34,109
bytes (27,331 bytes of margin), and `pliego-cssc` was 61,011 bytes (429 bytes of margin). All eleven
extracted archives compiled together with all features and the eight-package downstream closure
passed Rust 1.85. Publication remained disabled and the fixed 61,440-byte ceiling was not raised.
The remaining CLI margin is critical: subsequent milestones must compact the CLI package or place
new projection logic behind an existing specialized crate boundary.

The 2026-07-14 canonical TokenGraph publication gate passed the complete dirty-explicit local
Windows workflow with Cargo 1.96, Node.js 24.16, and the extracted Rust 1.85 downstream consumer.
Compile and watch now publish seven integrity-bound artifacts, while a one-bundle invocation plus
Project Index publishes nine; both groups include the canonical `pliego.tokens.json` and a receipt
check whose evidence is the exact graph artifact reference. `pliego-css-build` was 60,757 bytes
(683 bytes of margin), `pliego-css-compiler` was 59,213 bytes (2,227 bytes of margin),
`pliego-css-control` was 41,211 bytes (20,229 bytes of margin), and `pliego-cssc` was 61,067 bytes
(373 bytes of margin). All eleven extracted archives compiled together with all features, the
eight-package downstream closure passed Rust 1.85, publication remained disabled, and the fixed
61,440-byte ceiling was not raised. A fresh graph-bearing Linux replay remains a separate
portability gate.

The 2026-07-14 package-boundary compaction moved deterministic SARIF projection from
`pliego-cssc` into the already specialized `pliego-css-control/projection` feature and moved
inline-only test modules out of both registry archives without dropping them from workspace test
coverage. In the dirty-explicit Windows package vector, `pliego-cssc` fell from 61,055 to 58,818
bytes (2,622 bytes of margin), `pliego-css-build` fell from 60,753 to 54,519 bytes (6,921 bytes of
margin), and `pliego-css-control` grew from 41,203 to 42,884 bytes (18,556 bytes of margin). The
affected suites passed in Debian WSL2, including 22 build tests, 60 CLI unit tests, the SARIF/audit
matrix, compile control, and all 16 control-contract tests. Windows produced and compiled the exact
archives; execution of the regenerated downstream `.exe` was blocked by local Application Control
with OS error 4551, so that intermediate vector was not clean release evidence. The later clean
package gates below supersede it.

The 2026-07-14 direct DTCG CLI gate then passed the complete dirty-explicit Debian WSL2 package
workflow with Cargo 1.96 and the extracted Rust 1.85 downstream consumer. The bounded
`--tokens FILE [--token-input modifier=context]...` implementation keeps Resolver parsing and graph
construction in `pliego-css-config`; the CLI adds only selection, snapshot, and control-ledger
wiring. `pliego-cssc` was 60,087 bytes (1,353 bytes of margin), `pliego-css-build` was 54,513 bytes
(6,927 bytes of margin), `pliego-css-control` was 42,889 bytes (18,551 bytes of margin),
`pliego-css-config` was 57,327 bytes (4,113 bytes of margin), and `pliego-css-compiler` was 59,216
bytes (2,224 bytes of margin). All eleven archives compiled together with all features, the
eight-package downstream closure passed Rust 1.85, publication remained disabled, and the fixed
61,440-byte ceiling was not raised. Node.js 20.19.2 orchestrated this extra local Linux evidence; it
does not lower the supported Node.js 22.13 floor.

Commit `3831e38` then passed the same complete Debian WSL2 workflow from a clean worktree.
`pliego-cssc` was 60,062 bytes (1,378 bytes of margin), `pliego-css-build` was 54,517 bytes
(6,923 bytes of margin), and every other archive retained at least 2,224 bytes of margin. The clean
all-feature archive workspace and extracted eight-package Rust 1.85 consumer both passed;
publication remained disabled. This closes the local clean package gate for direct DTCG CLI
selection without widening the ceiling or claiming hosted release evidence.

The additive bundle-plan schema-2 DTCG gate then passed the complete clean Debian WSL2 workflow at
feature commit `e391e91` after hardening plan/Resolver reads and compacting the packaged CLI README.
`pliego-cssc` was 60,107 bytes (1,333 bytes of margin), `pliego-css-build` was 54,514 bytes (6,926
bytes of margin), `pliego-css-config` was 57,331 bytes (4,109 bytes of margin), and
`pliego-css-compiler` was 59,219 bytes (2,221 bytes of margin). All eleven all-feature archives and
the extracted eight-package Rust 1.85 consumer passed from the clean worktree; publication remained
disabled. This closes the local clean package gate without widening the ceiling or claiming hosted
release evidence.

The 2026-07-15 Cargo build-macro DTCG bridge passed the complete dirty-explicit Windows x64 package
workflow after moving the bounded Resolver reader behind architecture-correct `libc` flags on Unix.
`pliego-cssc` was 60,097 bytes (1,343 bytes of margin), `pliego-css-build` was 55,468 bytes (5,972
bytes of margin), `pliego-css-config` was 57,981 bytes (3,459 bytes of margin), and
`pliego-css-compiler` was 59,214 bytes (2,226 bytes of margin). All eleven all-feature archives
compiled, the extracted Rust 1.85 downstream workspace passed first with the DTCG macro and then
with the legacy TOML macro, and both consumers produced the exact same selected-registry output.
Publication remained disabled and the 61,440-byte ceiling was unchanged. This is development
evidence only.

Commit `9714b09` then passed the complete clean Debian WSL2 package workflow. `pliego-cssc` was
60,092 bytes (1,348 bytes of margin), `pliego-css-build` was 55,454 bytes (5,986 bytes of margin),
`pliego-css-config` was 58,043 bytes (3,397 bytes of margin), `pliego-css-compiler` was 59,212 bytes
(2,228 bytes of margin), and every other archive retained at least 18,551 bytes of margin. All
eleven all-feature archives compiled together, and the extracted Rust 1.85 downstream workspace
again produced exact identical output from the DTCG and legacy-TOML build scripts:
`1b052ee4ca1192db2e1ef91594166f70` / `pc_1ll5x99utndihr64pdw2gqgs0`.
Publication remained disabled and the fixed 61,440-byte ceiling was not raised. This closes the
local clean package gate for Cargo build-macro DTCG selection without claiming registry or hosted
release evidence.

The 2026-07-15 accessibility-policy and standards-provenance candidate passed the complete
dirty-explicit Debian WSL2 package workflow. `pliego-css-control` was 61,087 bytes (353 bytes of
margin, SHA-256 `a05e9914d687a03cdb0df2181b97d867f56fca5852bed8a661d215216c421461`),
`pliego-cssc` was 60,528 bytes (912 bytes of margin, SHA-256
`d24d849248a5f40f180bf293cb81b378cc5e55381ec64972ee092d07660dc571`), and
`pliego-css-compiler` was 59,218 bytes (2,222 bytes of margin). All eleven all-feature archives
compiled together, and the extracted Rust 1.85 downstream workspace again produced exact identical
DTCG and legacy-TOML output. Publication remained disabled and the fixed ceiling was not raised.
This is dirty development evidence; the exact feature commit still requires the clean replay.

Commit `046a0b8` then passed the complete clean Debian WSL2 package workflow. The normalized clean
archives were `pliego-css-control` 61,075 bytes (365 bytes of margin, SHA-256
`6ed34743ed741f787a5ced4510cc6075a996ad1b6e72bdf943e519f285d9e2ab`), `pliego-cssc` 60,513
bytes (927 bytes of margin, SHA-256
`a231e580955105c02e057b51df8c60ec7ec983a16fd8b8a57507ad7c32c60a42`), and
`pliego-css-compiler` 59,219 bytes (2,221 bytes of margin). All eleven archives and the extracted
Rust 1.85 downstream DTCG/TOML consumer passed with exact identical output; publication remained
disabled. This closes the exact-commit local package replay for the accessibility and provenance
milestone without claiming hosted or registry evidence.

The 2026-07-15 ownership-schema candidate passed the complete dirty-explicit Debian WSL2 package
workflow after expanding the boundary to twelve crates in six waves. `pliego-css-control` was
61,091 bytes (349 bytes of margin), `pliego-cssc` was 60,413 bytes (1,027 bytes of margin),
`pliego-css-compiler` was 59,223 bytes (2,217 bytes of margin), and the new
`pliego-css-ownership` archive was 13,587 bytes. All twelve archives compiled together; the
registry-shaped Rust 1.85 downstream closure now contains nine packages and exercises the public
ownership parser/builder in addition to the facade and build APIs. DTCG and legacy-TOML output
remained identical, publication stayed disabled, and the 61,440-byte ceiling was not raised. This
was development evidence and did not replace the then-pending clean replay.

Commit `6d9fe01` then passed the complete clean Debian WSL2 package workflow in the enforced six-wave
order. The normalized archives were `pliego-css-control` 61,069 bytes (371 bytes of margin, SHA-256
`823f2ce92cb9e02cf2cf7af043546ba255058306bc72852065dca48edc6cc30d`), `pliego-cssc`
60,392 bytes (1,048 bytes of margin, SHA-256
`abe2bfd9dd27d0986c46d88b39e13e3938f6f599dfa97cbbc1f007833310aa06`),
`pliego-css-compiler` 59,215 bytes (2,225 bytes of margin), and `pliego-css-ownership` 13,578
bytes (47,862 bytes of margin, SHA-256
`04ffe4cf257a6391d67a38332cb2bb9cb4bd1e4b5370fb15586332284c11a27b`). All twelve
archives compiled together with all features, and the extracted nine-package Rust 1.85 consumer
exercised the ownership bridge while preserving exact DTCG/TOML output. Publication remained
disabled and the fixed 61,440-byte ceiling was not raised. This closes the exact-commit local
package replay for ownership schema 1 without claiming hosted or registry evidence.

The 2026-07-15 usage-analysis and retention candidate then passed the complete dirty-explicit
Debian WSL2 workflow after expanding the boundary to thirteen crates. Cargo 1.96 produced every
archive, the extracted all-feature workspace compiled in release mode, and the registry-shaped
Rust 1.85 downstream closure expanded to ten packages and ran the public usage observation and
retention builders/parsers alongside the facade, build, and ownership surfaces. DTCG and legacy
TOML build-macro output remained identical. Exact archive sizes were `pliego-css-control` 61,085
bytes (355 bytes of margin), `pliego-css-build` 59,543 (1,897 bytes), `pliego-cssc` 59,051 (2,389
bytes), and the new `pliego-css-usage` 21,719 bytes. The verifier now distinguishes required graph
edges from declared optional edges activated by feature unification, while rejecting any undeclared
edge. Node 20.19.2 orchestrated this extra local Linux evidence; it does not lower the supported
Node 22.13 floor. Publication remained disabled and the fixed ceiling was unchanged. A clean replay
of the exact thirteen-crate commit remains required.

The hardened candidate superseded that size vector in a second complete dirty-explicit Debian
WSL2 replay using native Linux Node 24.14.0, Cargo 1.96, and the Rust 1.85 downstream toolchain.
`pliego-css-usage` now enables the narrow `pliego-css-build/usage-artifacts` feature, and its normal
dependency tree contains neither Lightning CSS nor Parcel Selectors. Its archive fell to 19,790
bytes; `pliego-css-build` was 59,571 bytes (1,869 bytes of margin), `pliego-cssc` 59,056 (2,384
bytes), and `pliego-css-control` 61,087 (353 bytes). All thirteen extracted all-feature packages
compiled in release mode, and the ten-package registry-shaped Rust 1.85 consumer exercised the
evidence-aware usage verifier plus observation, retention, ownership, DTCG, and TOML surfaces with
identical output. Publication remained disabled and the exact clean-commit replay remains required.

Commit `8f83036` then passed the exact clean Debian WSL2 replay with the same native Linux Node
24.14.0, Cargo 1.96, and Rust 1.85 toolchains. Normalized archives were `pliego-css-control` 61,073
bytes (367 bytes of margin, SHA-256
`224331188d0c719aac1b14330cf5f86ac6947c1d6f23af1fdac5d4f4e7e8a833`),
`pliego-css-build` 59,564 bytes (1,876 bytes, SHA-256
`2ee8017ee0e83a8269f2d069b46653398422e7a36f619356c60c6c701e37c6d9`), `pliego-cssc`
59,037 bytes (2,403 bytes, SHA-256
`048aa476394c95cdaa78c4cef22b041c858e34e040f5a49ef2757d247ab429e3`), and
`pliego-css-usage` 19,768 bytes (41,672 bytes, SHA-256
`2d080c2e4e450340ffbbaf12d1302a5f8bb8c4b8d347c3f5ec39cf540db0e180`). All thirteen
extracted archives compiled together in release/all-features mode, and the ten-package
registry-shaped Rust 1.85 consumer passed usage verification, ownership, DTCG, and TOML with exact
identical output. Publication remained disabled and the fixed ceiling was not raised.

The typed application collector milestone passed the full dirty-explicit Windows package replay
with Cargo 1.96 and the Rust 1.85 downstream toolchain. The initial collector replay measured
`pliego-css-source` at 22,573 compressed bytes. After the final junction/depth/expanded-site
hardening, its isolated package measured 23,024 bytes (38,416 bytes of margin, SHA-256
`82432a3244f9918c50e7c5dd6e0d9ab18cd0302b9dc1d65a50214a48fb899694`) while keeping the same
thirteen-crate publication graph. The tight archives remained below the frozen ceiling:
`pliego-css-control` 61,073 bytes (367 bytes of margin), `pliego-css-build` 59,573 (1,867),
`pliego-css-compiler` 59,219 (2,221), and `pliego-cssc` 59,040 (2,400). All extracted packages
compiled together in release/all-features mode, and the eleven-package Rust 1.85 consumer exercised
the collector plus the established application/build/usage surfaces and retained
identical DTCG/TOML/StyleId output. Publication remained disabled; an exact clean-commit replay is
still required for release evidence.

A subsequent full Windows dirty replay reached package creation but Windows Application Control
blocked a fresh temporary `serde_core` build script under `%TEMP%`; this is an environmental
execution-policy failure, not a package compile or size failure. The previously completed full dirty
replay plus the post-hardening isolated package, workspace/MSRV, public-API, Clippy, rustdoc, and
PliegoRS gates remain the current development evidence. The exact clean Debian WSL2 replay after
commit is the controlling package evidence for this milestone.

Commit `45238f1` passed that controlling clean Debian WSL2 replay with native Linux Node 24.14.0,
Cargo 1.96, and Rust 1.85. `pliego-css-source` was 23,018 compressed bytes (38,422 bytes of margin,
SHA-256 `a7401c341e085c05e93bf0a5801c6d8908fbbd43e73c45747a5b498132808e21`). The tight archives
remained below the fixed ceiling: `pliego-css-control` 61,074 bytes (366 bytes of margin, SHA-256
`60ebb3469185ab6c988375572fa8bbc2fd47db7b66cfbcd997ad7b73a5925304`),
`pliego-css-build` 59,565 (1,875 bytes, SHA-256
`34f8f91165efa53c2012a7fe16a4e5cf0466d7f06b9fb0799b1df7d28731dfb4`),
`pliego-css-compiler` 59,218 (2,222 bytes), and `pliego-cssc` 59,049 (2,391 bytes, SHA-256
`b5bf52ce8ede15759944d13998cb75e6825eae613528dd05b7454a81e733e836`). All thirteen extracted
packages compiled together in release/all-features mode. The eleven-package registry-shaped Rust
1.85 consumer exercised the collector, application facade, build bridge, ownership, usage, DTCG,
and TOML surfaces with identical output. The worktree was clean, publication remained disabled, and
the 60 KiB ceiling was not raised.

`pliego-css-compiler` keeps its integration tests tracked and executes them in workspace/CI matrices,
but excludes `tests/**` from the registry archive. No runtime source, README, or license file is
omitted; the separation preserves compressed-size margin without weakening local verification.

The gate distinguishes development evidence from release evidence. A run with `--allow-dirty` can
inspect work in progress, but the final gate rejects a dirty tree and must be rerun on the exact
release commit. Neither form performs a registry upload.

Run the development form with:

```console
node scripts/check-packages.mjs --allow-dirty
```

Run the release form only from a clean tree:

```console
pnpm check:packages
```

## CI boundary

The checked-in CI workflow has a dedicated Ubuntu package job using Node.js 22.13, Cargo 1.96 for
archive creation, and Rust 1.85 for the extracted downstream consumer. Hosted green-run evidence
remains pending because this checkout has no configured Git remote. Cargo 1.96 remains release
tooling; the application packages declare and execute against Rust 1.85 as their MSRV.

## What this does not prove

- No crate has been uploaded or reserved.
- The Cargo repository URL is not currently reachable from a configured remote.
- An isolated archive-workspace and patched downstream pass is not a crates.io ownership or
  index-propagation check.
- Real-registry `cargo publish --dry-run` must still run in dependency waves after each lower layer is
  visible.
- Hosted CI, Cloudflare, browsers, clean-clone PliegoRS, API freeze, and onboarding remain separate
  release blockers.
