# Representative application 2: Vite/Tailwind inventory-first

Status: **passing local representative application gate**

This fixture represents an existing Tailwind CSS v4.3.2 application that is not ready to migrate.
PliegoCSS discovers and inventories it read-only without executing Vite, Tailwind, JavaScript,
plugins, or configuration.

## Application

- Vite-shaped `package.json` with pinned Tailwind/Vite dependencies.
- `index.html` containing 46 static class candidates.
- `src/app.css` with `@import "tailwindcss"`, `@theme`, and `@utility` seams.
- `src/main.ts` importing the stylesheet.

Directory discovery classifies one Tailwind source and one Tailwind template, records implicit
Preflight reliance and the external Tailwind import, and emits byte-identical inventory on two runs.
Every fixture file is hashed before and after to prove the command is read-only.

## Current evidence

- Inventory bytes: 10,471.
- Inventory SHA-256: `a739837c6a47cd23d7430f6ed49e0fb6ec7665723af69cd4db46eeaf516231b8`.
- Three static Tailwind CSS constructs.
- 46 static and zero dynamic template candidates.
- Zero unresolved dependencies.

This evidence is intentionally narrower than migration success. It does not prove generated CSS
parity, semantic class conversion, reversible codemods, or visual equivalence. Those remain the gap
exposed by this application.

## Reproduction

```console
pnpm integration:representative:vite-tailwind
```

This is application **2 of 5** required before selecting implementation gaps.
