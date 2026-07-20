# Release process

Status: **local package and public-API candidate verification implemented; PliegoCSS is not
release-candidate ready**

This process covers the nineteen publishable crates in one version-locked compatibility unit. It does not
authorize an upload. Publishing to crates.io, creating a public repository, pushing a tag, or changing
production infrastructure requires explicit owner approval at the moment of the action.

## Publishable boundary

All publishable crates share one version. Every internal dependency uses both a local `path` and an
exact registry requirement. Cargo uses the path in this workspace and removes it from the normalized
package manifest, leaving only the exact requirement.

The publication order has six dependency waves and is dependency-first:

1. `pliego-css-ir`, `pliego-css-cascade`, `pliego-css-ownership`, `pliego-css-io`, `pliego-css-publication`
2. `pliego-css-source`, `pliego-css-watch`, `pliego-css-parser`, `pliego-css-theme`
3. `pliego-css-config`, `pliego-css-compiler`
4. `pliego-css-build`, `pliego-css-macros`
5. `pliego-css-agent`, `pliego-css-usage`, `pliego-css-control`, `pliego-css`
6. `pliego-css-lsp`, `pliego-cssc`

Packages in one row are independent. Do not start the next row until the previous row is visible in
the target registry index. Mixing PliegoCSS package versions in one build is unsupported.

## Tooling boundary

Rust 1.85 is the library MSRV. Release packaging is a contributor operation and uses the repository's
pinned Cargo 1.96 exactly because the package gate verifies an unpublished interdependent workspace
by extracting the current archives into an isolated temporary workspace and patching every internal
crates.io name to those extracted contents. Node.js 22.13 or newer runs the metadata checks; Node is
not a crate or application dependency.

## 1. Choose the release

Before changing a version:

- decide whether the result is another candidate, such as `0.1.0-rc.1`, or the final `0.1.0`;
- close or explicitly defer every item in the release-blocker section below;
- move the release's user-visible changes and migrations from **Unreleased** into a dated, exact
  version section in `CHANGELOG.md`, leaving a new empty **Unreleased** section above it;
- update installation examples and every README version reference to that same exact version;
- confirm that the selected [public API candidate](../reference/public-api.md) is unchanged and
  decide explicitly whether this release activates it as a SemVer promise;
- confirm `STYLE_ID_FORMAT_VERSION = 2`, `CLASS_NAME_FORMAT_VERSION = 1`,
  `THEME_ID_FORMAT_VERSION = 2`, and `IR_BINARY_FORMAT_VERSION = 2`, plus default manifest 3,
  opt-in manifests 4/5, graphs 1/2, declaration and physical ID formats 1, reachability 1,
  observation/retention sidecars 1, usage analysis 1/2, Asset Plan and Project Index 1/2,
  ownership 1, migration inventory 1, inspect/catalog/utility-explain/cascade-explain schemas
  2/3/2/1, repair
  proposal/plan/dry-run/Change Receipt schemas 1.0.0, receipt result `checks-pending`, and byte-edit
  patch format 1, plus repair check-policy/Verification Receipt schemas 1.4.0 with canonical
  1.0.0/1.1.0/1.2.0/1.3.0 read support and `passed|failed|blocked` derivation, including
  identity-bound CSS budget inputs, fixed-profile test evidence/root Cargo inputs, fixed-profile
  PliegoRS Chromium evidence/eight profile inputs, complete adjacent FindingDocuments, and
  receipt-last rollback;
- confirm that single-value CLI options remain single-use and that only explicitly documented
  repeatable options accept multiple occurrences;
- confirm that every exact package name is controlled by Celiums Solutions LLC in the target
  registry.

Update `workspace.package.version` and every version in `workspace.dependencies` in the same commit.
The package gate rejects a partial bump.

## 2. Run local quality gates

Run from a clean checkout at the intended release commit:

The repository exposes three cumulative verification profiles. `verify:fast` is the mandatory
development and pull-request floor and does not launch generated executables. `verify:integration`
adds executable onboarding plus local browser/editor/framework seams
when their declared environment is configured. `verify:release` adds packaging, evidence, fuzzing,
network corpus, and every integration gate. Missing external prerequisites are reported as
`not-configured`; the release profile treats any such result as blocked rather than passed.

```console
pnpm verify:fast
pnpm verify:integration
pnpm verify:release
```

The expanded commands below remain useful for diagnosing one failed gate:

```console
cargo fmt --all -- --check
cargo test --workspace --all-targets --locked
cargo +1.85.0 test --workspace --doc --locked
cargo +1.96.0 test --workspace --doc --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo +1.85.0 test --workspace --all-targets --locked
pnpm baseline:check-diagnostics
pnpm check:api
pnpm check:properties
pnpm check:determinism
pnpm check:manifest
pnpm check:trace
pnpm check:media
pnpm check:pruning
pnpm check:fuzz
pnpm check:evidence
pnpm check:repair-corpus
pnpm check:migration-corpus
PLIEGOCSS_RUN_NETWORK_CORPUS=1 pnpm check:migration-real-corpus
pnpm check:attribution
pnpm check:portability
pnpm integration:pliegors
pnpm integration:pliegors-dev
pnpm integration:lsp
pnpm integration:vscode
pnpm integration:vscode-host
```

Run rustdoc with warnings denied. In a POSIX shell:

```sh
RUSTDOCFLAGS="-D warnings" cargo +1.96.0 doc --workspace --no-deps --locked
```

In PowerShell:

```powershell
$env:RUSTDOCFLAGS = "-D warnings"
cargo +1.96.0 doc --workspace --no-deps --locked
Remove-Item Env:RUSTDOCFLAGS
```

`pnpm check:api` builds an external Rust 1.85 fixture against the selected application/build
surface, then checks the standalone CLI, identity agreement, numbered JSON documents, CSS/manifest
integrity, and strict single-value option behavior from an unrelated working directory. It is a
machine smoke, not the human documentation-onboarding gate.

`pnpm check:manifest` freezes schema-4 bytes and verifies schema-3 compatibility, CSS invariance,
semantic declaration/token edges, exact application ownership, canonical sidecar ordering,
fail-closed validation, and bundle `--check`. It uses only relative logical source labels so its
golden is host-independent. `check:api` also invokes it, but release operators may run it separately
while diagnosing provenance changes.

`pnpm check:trace` freezes schema-5/graph-2 bytes, proves its exact graph-1 projection, validates every
physical range and endpoint against the adjacent CSS, covers theme/generated output plus target and
printer variants, and exercises compile/watch/bundle fail-closed behavior. `check:api` invokes this
gate as part of the candidate process surface.

`pnpm check:attribution` is a required release gate for the curated standards, backend, dataset, and
benchmark provenance boundary. It validates the closed manifest, notices, locks, frozen metadata,
documentation link, and CI command offline. It is not an exhaustive dependency SBOM or legal audit.

`pnpm check:media` proves that the narrowly scoped adjacent-media optimization preserves canonical
rule order, reduces 20 equal wrappers to one, and clears its raw/gzip adoption thresholds. It does
not substitute for the final real-application output-strategy benchmark.

`pnpm check:pruning` repeats baseline and pruned schema-5 compilation over one controlled sidecar,
checks graph closure, and freezes raw/gzip payload deltas. The fixture is intentionally dead-heavy;
its percentage reduction is a change gate, not a production-application claim.

`pnpm integration:pliegors` additionally validates one PliegoRS product registry, generates
reachability plus five source partitions twice, and builds them with manifest schema 5, pruning,
`--asset-plan`, and `--project-index`. Its PliegoRS SSG consumer verifies the generated plan, exact
source/CSS/manifest snapshots, source-site backlinks, semantic/application/final
CSS lineage, and separate route/island mappings before combining a rendered island. An
unreferenced dead bundle remains in the integrity inventory but is excluded from deployment. The
gate includes detached-index and unsafe-filename negatives. It does not prove exhaustive Cargo-graph
discovery, a universal measured preload policy, or crash-atomic publication.

`pnpm integration:pliegors-browser` first runs that complete SSG gate, then serves the output on an
ephemeral loopback port and drives local headless Chrome through CDP. It requires that one delegated
resumability event preserve the exact SSR document, island, button, and bound-text element while state
advances from 15 to 20, classes remain stable, one typed state event fires, and browser/server errors
remain empty. It also requires one `/assets/shared.css` preload, the matching applied stylesheet,
exactly one loopback request for that resource, and one successful Rust/WASM client startup with one
request per bootstrap, bindgen module, WASM binary, and resume runtime. This is local Chromium
evidence and a bounded payload measurement, not a latency claim, production budget, or the hosted
Chrome/Firefox/WebKit release matrix.

`pliego-css-agent run-browser` wraps that same local replay as a separate fixed process boundary. It
holds `.pliegocss-repair.lock`, binds the Change Receipt after-source set plus the exact Cargo,
fixture-contract, and gate-script inputs, forces a clean fixture rebuild, records Node and observed
Chromium/CDP identity, and publishes canonical browser evidence. `pliego-css-agent verify` only
validates those bytes in process; it never launches Node or Chrome. `PLIEGORS_ROOT` may select a
checkout only when the bound contract revision and covered-source hash match.

`pnpm integration:plain-html` separately compiles the explicit line-oriented non-Rust fixture,
integrity-checks its schema-3 manifest, binds exact source ranges to declared HTML markers, and
requires the expected computed styles in local Chrome with zero page scripts and zero WASM resource
loads. It proves standard external-CSS/class consumption, not typed JavaScript reachability or a
published framework adapter.

`pnpm integration:pliegors-dev` pins the PliegoRS lockfile, starters, CLI, and transitive SSR/SSG
surfaces by commit/blob hash. It performs 20 valid edits, requires exact HTTP/artifact CSS equality,
semantic padding, stale-class removal, one SSE generation followed by a 2,000 ms stable window, and
closed negative cases for invalid and unchanged publication groups. It is a two-process/server gate;
the controlled Chromium replay and hosted multi-browser evidence remain separate.

`pnpm check:properties` executes 4,096 fixed-seed semantic cases plus a 16-thread shared-theme
contract without adding test sources to the published compiler archive. `pnpm check:determinism`
executes 43 compiler invocations: two sequential references, 40 workers in five cohorts of eight,
and one deterministically rejected lock probe. It compares schema-5 CSS and manifest bytes and
verifies same-destination locking plus cross-profile pair coherence. `pnpm check:fuzz` requires the
pinned nightly and cargo-fuzz environment described in the [fuzzing guide](./fuzzing.md); its two
targets run 20,000 fixed-seed executions each and retain reproduction artifacts on failure.

`pnpm check:evidence` requires Git history for every recorded source commit. It verifies the frozen
report's clean-state record, harness and tracked-input blobs, conventional median/MAD summaries,
Tailwind comparison arithmetic, and all Rust pair deltas without rerunning the machine benchmarks.
A new immutable snapshot must also be added to the explicit filename/SHA-256 allowlist in
`scripts/check-benchmark-evidence.mjs`; the JSON is not an externally signed attestation.
Do not squash or rebase a recorded source commit after approval: the verifier deliberately resolves
that exact historical blob. A history rewrite must supersede and regenerate the affected evidence.

Also regenerate and check the generated catalog, benchmark evidence, and every snippet gate. A local
success does not substitute for the hosted OS matrix, browser matrix, Cloudflare build, or developer
onboarding.

Review the exact StyleId format-2 golden stream, digest, ID, class, enum-tag, Unicode/framing, and
collision-failure tests. A changed vector is a format decision: bump the corresponding format,
document migration, and regenerate all dependent assets rather than accepting snapshot drift.

Review the semantic-IR format-1 full golden bytes, 309-byte length, complete-artifact SHA-256,
payload digest, inverse tags, cascade-preservation case, defensive limits, and corruption matrix.
Any drift likewise requires an explicit format decision and migration notes.

## 3. Verify package archives

Run:

```console
pnpm check:packages
```

The gate fails on a dirty worktree and verifies all of the following without publishing:

- the public boundary is exactly nineteen packages in the dependency-first order above;
- all packages share one version and Rust 1.85 MSRV;
- every internal dependency has the exact matching registry requirement;
- descriptions, repository metadata, categories, keywords, README, and the complete Apache-2.0
  license are present;
- tracked and packaged paths are relative, Unicode-normalized, collision-free, and safe on
  case-insensitive Windows/macOS filesystems; each package includes source files;
- Cargo 1.96 packages all nineteen archives without native registry verification, validates their
  normalized dependency contracts, exact resolved graph, and 62 KiB (63,488-byte) maximum
  compressed size per archive, then compiles only their extracted contents in the isolated patched
  workspace using the release profile;
- Rust 1.85 resolves, compiles, and runs the public facade, custom-theme bridge, ownership contract,
  and usage observation/retention producer surface as a separate registry-shaped release-profile
  downstream application patched exclusively to the extracted archives.

During development only, `node scripts/check-packages.mjs --allow-dirty` may be used to inspect a
candidate. Its archives are evidence for the dirty worktree, never release artifacts. The final run
must be clean and must not use that override.

Inspect any archive manually when its file list changes:

```console
cargo package --list -p pliego-cssc --locked
cargo package --list -p pliego-css --locked
```

## 4. Close hosted and external gates

All of these must be evidenced for the exact release commit:

- the repository URL in Cargo metadata resolves publicly and a Git remote points to it;
- Ubuntu, Windows, and macOS CI pass on Rust 1.85 and the pinned current toolchain;
- the package-archive CI job passes from a clean checkout;
- Chromium, Firefox, and WebKit/Safari behavior passes the documented browser fixtures;
- the Cloudflare production build pipeline passes without weakening the Cloudflare-first deployment
  rule;
- PliegoRS integration passes from clean, revision-pinned checkouts;
- documentation onboarding is completed by developers without prior project context;
- the final benchmark is reproduced and compared with the pinned Tailwind baseline.

Name availability, repository reachability, owners, credentials, and hosted runs are time-sensitive;
verify them again immediately before publication.

## 5. Dry-run against the real registry

After the release version's dependencies are present in the registry index, run each package in the
same dependency waves:

```console
cargo publish --dry-run -p <package> --locked
```

The workspace package gate proves that the unpublished set compiles together locally. The real
registry dry-run separately proves index visibility, ownership, normalized dependency resolution,
and the final binary lockfile. Both are required.

## 6. Publish only with explicit approval

For each dependency wave, obtain approval and then publish the named packages:

```console
cargo publish -p <package> --locked
```

Wait for index propagation and repeat the real-registry dry-run before advancing. Never reuse a
package archive generated from a dirty tree. Never publish all waves optimistically in parallel.

Published crate versions are immutable. A bad release can be yanked, but it cannot be replaced with
different bytes under the same version. Stop the sequence on the first anomaly and document exactly
which packages reached the registry.

## 7. Verify the installed release

From a new directory with no workspace patches:

```console
cargo install pliego-cssc --version '=0.1.0' --locked
pliego-cssc --version
```

Create a minimal application using exact `pliego-css` and, when needed, `pliego-css-build` versions.
Compile seed and custom-theme examples, run the CLI against the application source, and compare the
class/manifest identities with the release evidence. Verify that default manifest schema 3 and
opt-in schemas 4/5, inspection schema 2, catalog schema 3, and utility-explain schema 2 report
StyleId/class/ThemeId formats 2/1/2. Separately verify cascade-explain schema 1 statuses, candidates,
blockers, and exact declaration spans. Also verify graph schemas 1/2, declaration and physical ID
formats 1, exact graph-1 projection, CSS identity across manifest versions, and reachability schema
1 with the installed binary. Run `bundle --asset-plan` with manifest 4 or 5 and reachability, verify
the fixed `OUTPUT_DIR/pliego.assets.json` schema-1 document and every adjacent hash, then repeat with
`--check` to cover the complete read-only output group. For manifest 5, also add `--project-index`
and `--usage-report`; verify Project Index schema 1, Usage Analysis schema 1, their exact bindings,
source-site backlinks, and bundle-qualified physical declaration references. Then supply canonical
observation and retention sidecars, confirm stale and contradictory evidence fails without mutation,
and verify that retained selection alone upgrades Usage Analysis, Asset Plan, and Project Index to
schema 2 with `reachable-or-retained-style-ids`. Confirm route/island selections stay separate and
no artifact contains a deployment URL or preload policy. For a release candidate, substitute its
exact pre-release version in every command.

The changelog, README, installation documentation, package manifests, and code must already belong to
the release commit that produced the uploaded archives. After registry installation succeeds, tag
that exact unchanged commit and publish release notes from its changelog section. Do not amend the
release commit after upload; any correction requires a new package version.

## Current blockers

The following remain open as of the `0.0.0` candidate:

- the minimal public API candidate is selected and machine-checked locally, but no approved RC has
  activated its SemVer promise and the final RC/version has not been chosen;
- the configured hosted CI matrix has no recorded green run because this checkout has no remote;
- browser, Cloudflare, clean-clone PliegoRS, and onboarding evidence remain incomplete;
- the strategic R0 contract is not closed: direct CLI and bundle-plan schema-2 DTCG Resolver and
  canonical graph gates are green locally; Cargo build-macro DTCG selection passes the full Debian
  WSL2 workspace/MSRV/API gates, the Windows dirty-package consumer gate, and the clean package gate
  at commit `9714b09`. Complete attribution, accessibility policy checks, hosted cross-OS proof, and
  the diagnostic-quality real-incident corpus and same-model turn-reduction evidence remain open;
  the 16-case synthetic repair authority corpus is only a local conformance gate;
- the PliegoRS development stabilization revision is local-only until its `codex/` branch is reviewed
  and published/merged upstream;
- the repository URL and registry ownership must be established and rechecked;
- the full documentation and final benchmark gates remain open.

See the [compatibility contract](../reference/compatibility.md),
[strategic product contract](../product/strategic-product-contract-2026.md),
[research-derived alpha gates](../product/research-requirements.md),
[public API candidate](../reference/public-api.md),
[StyleId format-2 specification](../reference/style-id-format-v2.md),
[asset load-plan schemas 1 and 2](../reference/asset-plan-schema.md),
[Project Index schemas 1 and 2](../reference/project-index-schema.md),
[portability evidence](../status/portability.md), and [changelog](../../CHANGELOG.md).
