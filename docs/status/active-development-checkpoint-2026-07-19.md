# Active development checkpoint — 2026-07-19

**Base commit:** `c3b4db56217cd46ce362d0db0085b374cfd9f97c`
**State:** development reactivated for controlled convergence toward a Tailwind-competitive release
**Version:** unreleased `0.1.0-rc.1`

## Preservation

A complete local Git bundle was created outside the worktree before changing production files:

- `D:\PliegoCSS-preservation\pliegocss-c3b4db5.bundle`
- SHA-256: `07aa157a3e4181bd563a987438819e1f751ce09e20a6cf8477ab0c2b9d554560`
- `git bundle verify` reports a complete history for the available refs.

The recovered repository starts at `06f1c4d`; the historical benchmark commit `c47239c` is not present
in any local ref, reflog, or unreachable commit. The Cargo repository URL does not currently resolve,
and neither the authenticated user nor the visible organizations expose a `pliegocss` repository.
No remote was invented or configured. Benchmark evidence remains blocked until the original history
is restored or the snapshots are explicitly superseded.

## Phase 0 changes

- Repaired the Rust 1.96 Clippy failure in the LSP cancellation lookup without changing behavior.
- Rebound the reviewed standards-provenance contract to the current frozen `pnpm-lock.yaml` bytes.
- Added cumulative verification profiles:
  - `pnpm verify:fast`
  - `pnpm verify:integration`
  - `pnpm verify:release`
- Added a machine-checked release-profile registry and explicit `passed|failed|not-configured`
  accounting. Release verification treats missing external prerequisites as blocked.
- Updated the release process from sixteen to seventeen publishable crates.
- Added release-profile, documentation, and compatibility checks to CI.

## Verification

The complete `verify:fast` profile passed after one transient Windows doctest launch failure was
replayed successfully. The passing profile includes:

- Rust format;
- workspace check;
- workspace all-target tests;
- workspace doctests;
- warning-denied Clippy;
- documentation graph;
- standards provenance;
- compatibility policy;
- repair authority corpus;
- migration contract corpus.

Executable onboarding, editor hosts, browsers, PliegoRS, fuzzing, network corpus, package replay, and
historical benchmark evidence remain integration/release gates and are not represented as passing by
the fast profile.

## Next controlling task

Fase 2.1 mechanically modularized the CLI without changing flags, schemas, diagnostics, bytes, or
goldens. `pliego-cssc` now separates atomic writes, publication, formatting, explanation, repair,
catalog, migration, inspection, compatibility, provenance, and source-candidate collection from
the remaining orchestration in `main.rs`. The complete CLI all-target/all-feature test suite,
strict Clippy, and formatting pass on Debian WSL2; Windows compilation and runnable tests pass
where Application Control permits generated executables.

Fase 2.2 mechanically modularized `pliego-css-agent` behind its existing repair, corpus, and
publication contracts. `lib.rs` fell from roughly 7,383 to 3,115 lines; CLI parsing, proposal and
plan contracts, artifact/source identities, test/browser evidence, check policy, change and
verification receipts, publication/rollback, repair-state preparation, shared validators, and
static verification now live in focused modules. Public APIs remain available through reexports.
The complete agent all-target/all-feature suite, frozen corpus, verification CLI, strict Clippy,
and formatting pass on Windows and Debian WSL2.

The product maturity map now classifies 21 intentional capabilities as six stable candidates,
eleven beta surfaces, and four experimental directions. Every entry binds owner paths, persisted
schemas, current gates, graduation criteria, and explicit claim limits; a machine validator is part
of `verify:fast`. The stable compatibility promise remains deferred until RC.

The Tailwind CSS v4.3.2 competitive matrix now freezes 22 dimensions against the installed package,
the 44-attribute/302-occurrence fixture, local evidence, and versioned upstream documentation. It
records five matched slices, six Pliego differentiators, five Tailwind advantages, three intentional
design differences, and three open gaps without claiming catalog parity. Its validator is part of
`verify:fast` and runs without network access.

Five representative applications now pass: ordinary CSS audit-first, Vite/Tailwind inventory,
CSS Modules consumer inventory, typed Rust controlled compilation, and framework-neutral
multiroute/island bundles. The fifth records PliegoRS and browser evidence as `not-configured` and
`not-run` because the sibling checkout was absent; no external success is inferred. A machine gap
map is part of `verify:fast`.

The first application-selected blocking gap now has a bounded end-to-end slice. `transform-css`
reuses the existing Lightning CSS boundary, accepts explicit targets and pretty/minified output,
keeps authored CSS read-only, publishes atomically, and supports drift-only `--check`. Optional
`--control-dir` binds transformed CSS, canonical audit findings, control manifest, and receipt into
one rollback-capable group. Directed core/CLI tests, strict Clippy, docs, Windows, and Debian WSL2
pass.

The second application-selected blocking gap now has a versioned bounded corpus. Rule-level
coverage includes eight frozen feature families; `declaration-shapes-1` distinguishes typed,
unparsed, and custom declarations; `selector-shapes-1` inventories components, combinators,
attributes, pseudo-classes, and pseudo-elements. `user-select`, `:focus-visible`, and `:has()` now
produce frozen web-features 3.32.0 decisions. The explicit partial-coverage finding remains because
declaration values and the complete selector surface are not yet compatibility-classified.

Local Windows Chrome and Edge now have automated CDP computed-style smoke evidence. Firefox, WSL
Chromium, and macOS Safari remain `not-configured`, never pass.

The first reversible migration slice now completes `plan → explicit apply → hash verification →
rollback` on the Vite/Tailwind representative application. Planning remains read-only and emits one
`automatic: false` additive sidecar proposal while `edits` stays empty. Apply publishes sidecar plus
SHA-256 receipt; rollback refuses drift and restores absence. Four authored application files remain
byte-identical throughout. The next migration task is a bounded source edit with exact before/after
bytes, not an implicit destructive conversion.
That next boundary now passes on a disposable full copy of the Vite/Tailwind representative app.
The plan's `automatic: false` replace proposal supplies exact before/after hashes; staging materializes
those bytes, applies via receipt, re-inventories to the after hash, rolls back, and proves the source
inventory is identical to its original canonical document. Fast workspace gates and Debian WSL2
source/CLI tests plus strict Clippy pass. No authored fixture was changed.
The migration plan now also binds `theme-root-projection` and `utility-projection`, both
`automatic: false`. Each passes apply → re-inventory → rollback on a disposable representative-app
copy. Chrome and Edge computed-style checks prove ordered `var()` theme values, derived spacing,
OKLCH color, `content-visibility`, and `contain-intrinsic-size`. Dynamic/wildcard utility forms and
modified/multiple theme forms remain fail-closed.
Grouped CSS/template migration is exposed through `migration-group-apply` and
`migration-group-rollback`. One schema-1 receipt binds ordered paths and exact before/after
identities. Windows `verify:fast` and isolated Debian WSL2 source/CLI tests plus strict Clippy pass;
Chrome and Edge computed-style equivalence pass for the static alias group.
Phase 13 closes the grouped migration filesystem boundary: full before/after preflight, deterministic
fault injection, compensation in both directions, synced create-new temporaries, Unix directory sync,
Windows reparse rejection, bounded no-follow CLI reads, and shared `pliego-css-io` publication APIs.
Windows `verify:fast` passes; isolated WSL2 runs pass 5 I/O tests, 51 source units, 11 reversible
contracts, 5 CLI contracts, and strict Clippy for all three affected crates.
Phase 14 adds machine-scored accessibility quality evidence to `verify:fast`: 4 TP, 4 TN, 0 FP,
0 FN on the decidable literal-sRGB contrast slice; four dynamic values correctly abstain; and eight
motion/focus/forced-colors/input-modality cases freeze exact production finding codes. The claim is
synthetic and bounded; hosted/manual/browser and real-incident evidence remain external release work.
Phase 15 expands the web-features 3.32.0 compatibility slice to five declaration/value and five
selector features. Two-color `color-mix()` is allowed while its variadic feature is independently
identified and rejected; all findings retain classifier/dataset/target evidence. Partial coverage
remains explicit because this is representative expansion, not complete CSS classification.
Phase 16 adds generic-CSS declaration identities and conservative usage evidence. Audit emits
context/selector/importance/declaration-bound hashes; `generic-css-usage` joins positive observations,
keeps absence `unknown`, and optionally publishes report/findings/manifest/receipt as one controlled
group. Its schema and CLI contracts are included in `verify:fast`.
The resolver now indexes canonical token paths and JSON Pointer roots, traverses token dependencies
without recursive token calls, rejects chains deeper than 256 tokens, and caps resolution at 100,000
work units. Deep acyclic, deep-cycle, fan-out budget, and nested-property pointer tests pass on native
Windows and Debian WSL2; strict Clippy and formatting pass for `pliego-css-config` on both hosts.

ThemeId format 2 now derives its compact identity from a domain-separated, length-framed SHA-256
stream and the theme artifact is independently versioned as binary format 2. Format-1 FNV artifacts
are rejected explicitly instead of being reinterpreted. Compiler streams, StyleIds/classes, semantic
IR, catalogs, manifests, Asset Plans, public-API fixtures, and migration documentation have been
updated as one compatibility transition. The complete Windows workspace passes with all features,
strict Clippy, formatting, manifest graph, physical trace, portability, public API, getting started,
and the complete `verify:fast` profile. The complete Debian WSL2 workspace, Clippy, and formatting
also pass.

The cross-platform goldens affected by the intentional identity migration are re-frozen and pass.
Public `Style` construction is now sealed behind the macro bridge: the former doc-hidden raw-ID
constructor is absent from the application API and has a compile-fail contract. Compiler tooling has
fallible composition variants that validate mutable/manual IR before indexing intern tables; legacy
infallible wrappers remain explicitly documented for compiler-produced validated styles. Targeted
tests, UI diagnostics, strict Clippy, the Windows all-feature workspace, and Debian WSL2 pass.

Bounded regular no-follow reads are now shared by source, watch, and LSP Project Index input. They
reject link-like parent/leaf components and Windows reparse points, reopen the leaf without following
it, verify the opened descriptor is a regular file, and cap bytes before allocation. TOML theme input
now uses the existing bounded DTCG reader, aligning its 16 MiB and no-link policy. New TOML, watch,
source-discovery, and LSP parent/leaf-link tests pass in Debian WSL2; affected-crate Clippy and format
pass on Windows and WSL. The full Windows workspace is compile/test green until Application Control
blocks a generated test executable, while the independent WSL workspace gate remains the complete
host oracle.

The LSP now invalidates semantic/catalog cache identity across compiler, root, and current bounded
configuration bytes; malformed notifications neither emit a JSON-RPC response nor terminate/mutate
the session; framing caps aggregate headers and rejects duplicate `Content-Length`; initialization
falls back to the first workspace folder; and file URI authorities, UNC paths, query, and fragment
boundaries are explicit. Nineteen unit tests, the real framed LSP integration/cancellation/corpus
gate, VS Code client tests/build, strict Clippy, and formatting pass. The complete safe-I/O workspace
gate also passes in Debian WSL2.

Publication locking and reservation are now shared through the dependency-light
`pliego-css-publication` crate. CLI and repair-agent coordination reject link-like lock leaves, reserve
temporary siblings with `create_new`, retain descriptor identity through preparation, and share
rollback-capable grouped publication with optional file/directory synchronization. The contract is
explicitly durable when requested and rollback-capable for observed failures, but not crash-atomic.
Five transaction contracts, CLI/agent suites, strict Clippy, and Debian WSL2 verification pass.
`pliego-css-io` separately keeps watch on a minimal dependency graph. The package boundary is now 19
crates; all archives package under the 62 KiB ceiling, with native Windows extraction still requiring
a shell-independent tar invocation in the package gate.

The package gate now converts Windows archive paths for Git-Bash tar, generates the temporary
downstream lock with Rust 1.85 before enforcing `--locked`, packages all 19 crates, compiles the
registry-shaped extracted workspace in release mode, and passes its downstream Rust 1.85 smoke in
Debian WSL2. Native Windows reaches the same final smoke but Application Control blocks generated
dependency build scripts, so WSL remains the independent executable oracle. The complete all-feature
workspace, strict Clippy, format, compatibility policy, public API, manifest graph, physical trace,
portability, docs, and Getting Started gates pass.

Phase 1 foundation hardening is complete. The next controlling task is mechanical modularization of
the CLI and agent behind frozen black-box contracts; behavior, flags, schemas, bytes, and diagnostics
must remain unchanged during movement.
