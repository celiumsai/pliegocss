# ADR-0026: Certify static adapter coexistence before migration promotion

- Status: Accepted for the G6 adapter architecture
- Date: 2026-07-22

## Context

Read-only Tailwind inventory and byte-exact replacement primitives do not prove that an application
can adopt PliegoCSS safely. A useful migration claim must bind the competitor version, compiled
Tailwind output, selected PliegoCSS output, template edits, browser behavior, and rollback to the
same project snapshot. One synthetic Vite fixture is insufficient for an LTS architecture.

PliegoCSS also needs an ownership boundary. The compiler can translate supported static utility
groups into semantic styles, but it cannot truthfully execute arbitrary Tailwind configuration,
plugins, JavaScript class construction, or framework-specific template semantics.

## Decision

G6 uses a versioned adapter coexistence protocol with these stages:

1. An adapter supplies a complete static HTML document and rejects every non-literal class seam.
2. The exact complete class groups are compiled by `pliego-cssc`; manifest origins are the only
   authority for mapping source groups to generated `pc_` classes.
3. Tailwind v3-LTS or current v4 compiles the same document without Preflight. Its exact CSS is
   inspected by `pliego-cssc audit`, including a read-only control replay.
4. The adapter creates an explicit two-file migration group: template bytes and a Pliego sidecar.
   `migration-group-apply` applies only the approved after bytes. Tailwind CSS remains loaded so both
   runtimes coexist during the migration window.
5. A real browser compares the pre-apply and post-apply body DOM, ARIA snapshot, frozen computed
   properties, and node geometry. A Vite adapter must additionally pass a production Vite build.
6. `migration-group-rollback` must restore the exact original template, empty sidecar, and unchanged
   Tailwind CSS bytes.

The schema-1 authority contains 21 unique owned project snapshots: seven plain HTML, seven Vite, and
seven PliegoRS rendered-output cases. Ten run Tailwind v3-LTS and eleven run current v4. Hosted proof
uses Chromium on Windows x64, Linux x64, and macOS arm64. The workflow also runs the pinned real
PliegoRS SSG/resumability browser gate; rendered-output cases alone do not substitute for it.

## Consequences

- The protocol provides a reviewable coexistence path without claiming a general Tailwind codemod.
- Dynamic class construction, unsupported utilities, and ambiguous manifest mappings fail closed.
- Keeping Tailwind loaded makes incremental adoption possible, but removing Tailwind is a later,
  explicit project decision.
- DOM comparison intentionally ignores `class` and the certification node marker; every other body
  attribute and text node remains authoritative.
- PliegoRS source compatibility is pinned independently from the rendered-output corpus.
- Passing owned projects is product evidence, not external adoption evidence; external pilots belong
  to G7.

## Verification

```console
pnpm check:adapter-coexistence-authority
pnpm check:adapter-coexistence -- --browser=chromium
pnpm integration:pliegors-browser
```

Hosted proof and matrix aggregation are owned by
`.github/workflows/adapter-coexistence.yml`.
