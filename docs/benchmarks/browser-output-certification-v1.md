# Browser/output certification v1

Status: **implemented local harness; hosted 3×3 evidence pending**

This page is generated from
[`authority.json`](../../benchmarks/browser-output-certification-v1/authority.json). Edit the
JSON contract and run `node scripts/render-browser-output-certification.mjs --write`; do not
maintain the matrix by hand.

## Scope

The certification compiles the frozen medium fixture once with PliegoCSS and once with the fresh
`tailwind-latest` lane from Benchmark Authority v2. Within each host it compares
52 computed properties on every styled node and captures both
original screenshots plus a pixel-diff image for every scenario.

Passing proves only the frozen shared utility intent. It is not general Tailwind compatibility,
does not compare pixels across different browser engines, and does not broaden PliegoCSS's utility
catalog.

## Reset contract

| Mode | Contract |
|---|---|
| `no-reset` | Neither PliegoCSS nor Tailwind receives Preflight or any external reset. |
| `shared-reset` | The exact tracked shared reset bytes are prepended to both outputs. |

The shared reset is isolated in `@layer reset`. Tailwind Preflight is not silently enabled because
PliegoCSS does not own a Preflight implementation. This keeps the reset/no-reset comparison
explicit and prevents an unlayered reset from outranking Tailwind's layered utilities.

## Hosted matrix

| Host | GitHub runner | OS | Architecture | Browser engine |
|---|---|---|---|---|
| `windows-x64-chromium` | `windows-latest` | win32 | x64 | chromium |
| `windows-x64-firefox` | `windows-latest` | win32 | x64 | firefox |
| `windows-x64-webkit` | `windows-latest` | win32 | x64 | webkit |
| `linux-x64-chromium` | `ubuntu-24.04` | linux | x64 | chromium |
| `linux-x64-firefox` | `ubuntu-24.04` | linux | x64 | firefox |
| `linux-x64-webkit` | `ubuntu-24.04` | linux | x64 | webkit |
| `macos-arm64-chromium` | `macos-15` | darwin | arm64 | chromium |
| `macos-arm64-firefox` | `macos-15` | darwin | arm64 | firefox |
| `macos-arm64-webkit` | `macos-15` | darwin | arm64 | webkit |

The matrix is the full browser/OS cross-product for Chromium, Firefox, and WebKit across Windows,
Linux, and macOS. Windows and Linux provide x64 evidence; the standard `macos-15` runner provides
ARM64 evidence.

## Scenarios

| Scenario | Viewport | State |
|---|---:|---|
| `base-mobile` | 375×812 | none |
| `base-tablet` | 768×1024 | none |
| `base-desktop` | 1440×900 | none |
| `hover-primary` | 1440×900 | hover |
| `focus-input` | 1440×900 | focus |

Every scenario runs under both reset modes. Computed colors normalize to 8-bit sRGB, equivalent
flex-end keywords normalize together, and layout lengths quantize to
0.25 CSS px to account for browser-internal
subpixel serialization. The same host must also match every styled border box, client/scroll extent,
and direct text-node rectangle captured at 0.015625 CSS
px resolution with at most 0.1 CSS px delta.
Each scenario and host summary retain the maximum observed geometry delta; aggregation recomputes
that maximum and rejects missing, non-finite, negative, over-budget, or summary-drifted values.
Screenshots use pixelmatch threshold
0.2 and may differ on at most
0.5% of CSS pixels; original PNG hashes
remain retained so the tolerance cannot hide or rewrite evidence.

## Commands

```console
pnpm check:browser-output-authority
pnpm exec playwright install chromium firefox webkit
pnpm check:browser-output -- --browser=chromium
pnpm check:browser-output -- --browser=firefox
pnpm check:browser-output -- --browser=webkit
```

CI executes each of the nine hosts with `--require-clean`, uploads the JSON and PNG artifacts,
then aggregates them only when every host is unexpired, clean-tree, bound to one commit/tree, and
complete. That source identity must equal the aggregator checkout, so nine mutually consistent but
stale documents cannot pass. Local dirty-tree runs are diagnostic and cannot satisfy the hosted matrix.
