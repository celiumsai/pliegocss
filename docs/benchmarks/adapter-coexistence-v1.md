# Adapter coexistence certification v1

Status: **local Windows host passed; hosted three-host matrix is produced by the dedicated workflow**

The schema-1 authority converts G6 from a migration proposal into a falsifiable adapter contract.
It owns 21 unique project snapshots, exact Tailwind and Vite versions, a static migration policy,
browser comparison fields, and three required Chromium hosts.

## Coverage

| Dimension | Contract |
|---|---|
| Projects | 21 unique rendered documents; minimum 20 |
| HTML | 7 direct-document projects |
| Vite | 7 production-build projects on Vite 8.1.5 |
| PliegoRS | 7 rendered-output projects plus one pinned real framework/browser gate |
| Tailwind | 10 v3-LTS projects and 11 current-v4 projects |
| Migration | exact two-file group, Tailwind retained, exact rollback |
| Browser | DOM, ARIA snapshot, 49 computed properties, geometry ≤ 0.1 CSS px |
| Hosted matrix | Windows x64, Linux x64, macOS arm64; Chromium |

## Local evidence observed on 2026-07-22

The Windows x64 Chromium run passed all 21 projects:

- 21/21 DOM-equivalent;
- 21/21 ARIA-equivalent;
- 21/21 computed-style-equivalent;
- 21/21 geometry-equivalent;
- 21/21 byte-exact rollbacks;
- 7/7 Vite production builds;
- 21/21 deterministic v3/v4 source inventories with zero dynamic or unsupported constructs;
- Tailwind output audit errors: zero.

The real PliegoRS gate also passed against pinned revision
`cc61a41b49ab60eb76e82fedebccf17fe00bdc0c`. It verified two SSG routes, five selected bundles,
stylesheet preload, JS and WASM requests, island node identity, one resumed event, and the expected
state transition from 15 to 20 in Chrome 150.

Local evidence is diagnostic and was generated from a dirty implementation tree. It does not stand
in for the clean exact-source hosted matrix. The workflow artifacts are the authority for promotion.

## Claim boundary

Passing proves only literal complete-class-group migration in the tracked corpus and named hosts.
Tailwind remains present during coexistence. Dynamic templates, arbitrary plugins/config execution,
general Tailwind compatibility, and external projects are excluded.

## Verification

```console
pnpm check:adapter-coexistence-authority
pnpm check:adapter-coexistence -- --browser=chromium
```
