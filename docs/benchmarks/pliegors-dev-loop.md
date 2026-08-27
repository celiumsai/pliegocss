# PliegoRS development-loop gate

Status: **source-pinned local two-process/SSE gate green; controlled Chromium
replay observed; hosted multi-browser evidence pending**

The automated gate measures the server-side development path rather than only PliegoCSS
compilation:

```text
Rust `pc!` edit
→ `pliego-cssc watch` publishes changed CSS/manifest bytes
→ `pliego dev` rebuilds the SSG project
→ PliegoRS advances exactly one stable SSE generation
```

The browser reload itself is a separate controlled Chromium replay. The latency table below is not a
visual/browser completion measurement.

The fixture is a detached, lockfile-pinned Rust project under
`integration-tests/pliegors-dev-loop`. The harness copies it to a disposable directory and pins the
sibling PliegoRS revision plus source bytes, including `crates/pliego-cli/**` and the product-topology schema, before it starts either
process. The canonical contract hashes committed Git blobs for `Cargo.toml`, `Cargo.lock`, the core
SSR/SSG surfaces, `crates/pliego-starters/**`, `crates/pliego-cli/**`, and
`schemas/pliego.product-topology.schema.json`; covered dirty files fail
closed. The PliegoRS watcher excludes PliegoCSS's reserved `.pliego.lock`, `.tmp`, and `.bak`
coordination files. Among PliegoCSS publication files, only a changed final CSS or manifest is
application input.

## Reproduce

Run from the PliegoCSS root with a clean sibling `pliegors` checkout matching the pinned contract:

```console
pnpm integration:pliegors-dev
```

The command currently builds both CLIs with the stricter Rust 1.86 toolchain,
starts `pliego-cssc watch` and `pliego dev` on an
ephemeral loopback port, alternates `p-4` and `p-6` 20 times, and returns schema-2 JSON. On Windows
machines whose Application Control policy rejects freshly rebuilt test executables, run the same
script in Debian WSL2 with native Linux binaries and a target directory on the Linux filesystem:

```console
PLIEGO_DEV_TARGET_DIR="$HOME/.cache/pliegocss-dev-target" \
  node scripts/check-pliegors-dev-loop.mjs
```

The current local replay passed on native Linux x64 with Node.js 22.13.0,
Rust 1.86, PliegoRS `780ea41973908766211fa0e4e382f1b654d4ab40`, and
20 valid edit generations plus failure/recovery/no-op checks. The checked
schema-2 evidence below remains historical until a clean coordinated commit is
recorded.

On a clean PliegoCSS commit, `--record-evidence` writes the same validated schema-2 report to
`docs/benchmarks/data/pliegors-dev-loop.local.json`. It refuses a dirty starting tree.

For an interactive browser replay, start:

```console
node scripts/check-pliegors-dev-loop.mjs --serve-browser
```

Then open the reported loopback URL and drive the controlled phases from another terminal:

```console
node scripts/check-pliegors-dev-loop.mjs --browser-command valid
node scripts/check-pliegors-dev-loop.mjs --browser-command invalid
node scripts/check-pliegors-dev-loop.mjs --browser-command restore
node scripts/check-pliegors-dev-loop.mjs --browser-command equivalent
node scripts/check-pliegors-dev-loop.mjs --browser-command stop
```

Run the server and every phase command in the same host environment so their state-file path syntax
matches.

## Local result

Twenty alternating valid edits produced in the clean recorded run for PliegoCSS
`65d2e60a5ea1ea25070d603aadefb6e7d9f9a881` and PliegoRS
`ca9708cdf2e6ff2c56605baf21046792407c5326`. The complete schema-2 artifact is
[`data/pliegors-dev-loop.local.json`](./data/pliegors-dev-loop.local.json).

| Boundary | p50 | p95 | Min | Max |
|---|---:|---:|---:|---:|
| Edit → changed CSS bytes | 228.300 ms | 230.416 ms | 126.920 ms | 253.540 ms |
| Edit → matching HTML/CSS and one stable SSE generation | 1,695.859 ms | 1,697.041 ms | 1,544.919 ms | 1,697.408 ms |

Percentiles use nearest-rank over 20 samples. Every sample ended with the expected generated class
and semantic padding, no stale class from the previous edit, exact SHA-256 equality between served
and published CSS, one SSE increment, and a 2,000 ms stable window. The final cache assertion was one
discovered Rust file, one scan hit, zero parses, one semantic hit, zero lowerings, and zero removals.
This demonstrates reuse of the unchanged source unit when only the external line-oriented input
changed; it does not claim fragment-level CSS emission.

The local Chromium replay began with `p-4`, class `pc_89rkuolb9wxgvnpc398qxkvou`, and computed
padding `16px`. After the controlled valid edit, the same tab reloaded without a manual navigation
and exposed `p-6`, class `pc_7ru2gfaaana56ofnpik4nuzny`, and computed padding `24px`. Exactly one
PliegoRS reload client was present.

## Negative contracts

- Replacing the `pc!` value with an invalid spacing token made both compilers
  fail. PliegoCSS retained CSS/manifest bytes and modification times; PliegoRS
  advanced once to a bounded HTTP 500 diagnostic generation. Restoring valid
  source advanced exactly once more and recovered the page.
- Adding an ignored blank line to the line-oriented input caused PliegoCSS to report unchanged
  generated bytes. CSS and manifest hashes/modification times remained exact, and PliegoRS had not
  advanced its reload generation after the 2,000 ms quiet window.
- The first Windows run exposed a real integration defect: PliegoRS attempted to fingerprint the
  held `.pliego.css.pliego.lock` file. The upstream watcher now ignores only PliegoCSS's reserved
  coordination namespace, with unit tests. Measurements from the defective run are discarded.
- The source-pinned replay exposed access-event starvation and delayed duplicate
  no-op generations. PliegoRS now filters access-only events before queueing,
  drains non-access changes, and suppresses no-op publications with no changed
  artifacts.

## Boundary

This is full-page reload owned by PliegoRS, not CSS-only HMR. The local gate does not establish
hosted runner latency, Firefox/WebKit behavior, power usage, filesystem-event performance, or
fragment-level emission. Those claims require separate evidence.
