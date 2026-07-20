# Representative application 3: CSS Modules consumer

Status: **passing local representative application gate**

This application exercises an incremental CSS Modules adoption boundary: two module stylesheets, a
resolved relative `composes` edge, and a TSX consumer using direct access, an alias, destructuring,
and a deliberately dynamic class lookup.

## Current evidence

- Two CSS Modules sources and one consumer.
- One resolved composition to `styles/base.module.css`.
- One static import, four static class usages, one dynamic usage, one alias, and one destructure.
- Inventory bytes: 4,481.
- Inventory SHA-256: `6994a7ee66ce5634370708bbea90afefbd98f4342ed0be96b23be908450a1f3c`.
- Two identical inventory runs and unchanged fixture hashes.

PliegoCSS does not execute TypeScript, a bundler, or CSS Modules transformation in this gate. The
inventory proves lexical seams and exact file identity, not that an exported class exists or that a
migration preserves rendering.

## Reproduction

```console
pnpm integration:representative:css-modules
```

This is application **3 of 5** required before selecting implementation gaps.
