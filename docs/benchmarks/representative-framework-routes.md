# Representative application 5: framework-neutral multiroute bundles

Status: **passing local representative application gate; PliegoRS/browser evidence not configured**

The expected sibling PliegoRS checkout was unavailable on this host. Instead of reusing stale claims
or marking an external gate as passing, this application exercises the framework-neutral contract
that a typed adapter would consume.

## Topology

- Routes: `/` and `/visit`.
- Island: `visit-counter`.
- Bundles: `global`, `home`, `visit`, and `counter`.
- Home selects `global + home`.
- Visit selects `global + visit`.
- The island selects `global + counter`.

The gate generates exact reachability from four source sites, builds schema-5 manifests with pruning,
emits an Asset Plan and Project Index, adds controlled Source Maps/token graph/findings/manifest/
receipt, and proves every tracked artifact is unchanged under `--check`.

## Current evidence

- Asset Plan: 2,317 bytes, SHA-256
  `be6f9629576c89dcf01374cdc1b60f4ae30762ad492443a8db84699da696ef32`.
- Project Index schema 1 over four source documents: 10,320 bytes.
- Four CSS bundles with exact Source Map and physical provenance manifests.
- Complete controlled group passes drift-only verification.
- PliegoRS checkout: `not-configured`.
- Browser replay: `not-run`.

This is intentionally not presented as PliegoRS or browser evidence. Those remain integration gates
requiring `PLIEGORS_ROOT` and the explicit browser environment.

## Reproduction

```console
pnpm integration:representative:framework-routes
```

This is application **5 of 5**. Its evidence, together with the first four applications, now controls
which product gaps are implemented next.
