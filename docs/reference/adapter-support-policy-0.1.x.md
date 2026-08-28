# Adapter support policy for 0.1.x

Status: **frozen for 0.1 promotion**

The machine-readable authority is
[`adapter-support-policy-0.1.x.json`](../product/adapter-support-policy-0.1.x.json). The policy is
deliberately narrower than Tailwind's complete feature surface: it publishes only what the G6
coexistence protocol can reproduce, compare in a real browser, and roll back byte-for-byte.

## Certified matrix

| Input adapter | Certified version/input | Tailwind profile |
|---|---|---|
| Static HTML | Complete static HTML document | 3.4.19 or 4.3.3 |
| Vite | Vite 8.1.5 production build | 3.4.19 or 4.3.3 |
| PliegoRS | Rendered static HTML plus `pliego-dom=0.0.2` / `pliego-ssg=0.0.2` browser replay | 3.4.19 or 4.3.3 |

Certified adoption operates on complete, literal class groups. Tailwind remains loaded without
Preflight during coexistence. Apply changes only the approved class-group bytes and PliegoCSS
sidecar; rollback restores the exact original source bytes.

## Required promotion evidence

Every promoted adapter/profile pair must retain:

- clean Windows x64, Linux x64, and macOS arm64 Chromium lanes;
- DOM and ARIA equivalence;
- frozen computed-style equivalence;
- layout geometry within 0.1 CSS px;
- exact rollback;
- the separate pinned PliegoRS framework/browser replay when that adapter is claimed.

## Explicit exclusions

The certified tier does not include dynamic class construction, arbitrary Tailwind config or plugin
execution, unsupported utilities, Tailwind removal, or versions other than those listed above.
Reports outside the matrix may be triaged best-effort, but they cannot extend the 0.1.x compatibility
claim without a versioned policy change and new evidence.

## Verification

```console
pnpm check:adapter-coexistence-authority
pnpm check:external-adoption-authority
```
