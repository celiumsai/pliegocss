# Representative applications and selected gaps

Status: **five application gates complete; first evidence-driven gap selected**

The machine authority is [`representative-applications-gap-map.json`](./representative-applications-gap-map.json).
The applications deliberately cover distinct adoption modes rather than five variants of the typed
utility fixture.

| Application | Gate | Proven boundary | Main exposed gap |
|---|---|---|---|
| Plain HTML audit-first | `integration:representative:plain-html-audit` | Ordinary CSS, budgets, static accessibility, receipt | No deterministic standard-CSS transform/minify output |
| Vite/Tailwind inventory-first | `integration:representative:vite-tailwind` | Read-only source/template inventory and Preflight reliance | No reversible semantic migration |
| CSS Modules consumer | `integration:representative:css-modules` | Sources, imports, aliases, destructuring, dynamic usage, composition | No export/bundler proof or reversible migration |
| Typed Rust controlled build | `integration:representative:rust-control` | Rust 1.85 API, `pc`/`pcx`, seven controlled artifacts | Clean-build and rendered-app evidence |
| Framework-neutral routes | `integration:representative:framework-routes` | Routes/island, Asset Plan, Project Index, physical trace | PliegoRS/browser/hosted evidence not configured |

## Priority order

1. **Standard-CSS transform/minify output** — release blocking and directly exposed by the first-value
   plain HTML application.
2. **Supported classification corpus** — release blocking; the coverage warning remains honest.
3. **Hosted browser/OS evidence** — release blocking and cannot be manufactured locally.
4. **One reversible migration slice** — report-level Should, selected from Tailwind or CSS Modules.
5. **Published RC replay** — release blocking and owner-dependent at publication time.

The first implementation gap is therefore a bounded deterministic output path for ordinary CSS. It
must reuse the existing Lightning CSS boundary, preserve audit findings and target policy, keep input
read-only, and bind generated output into explicit artifacts rather than changing `audit` silently.

## Evidence boundary

Four applications are fully local. The fifth proves the neutral adapter contract because the sibling
PliegoRS checkout was absent. Its report records `not-configured` and `not-run`; no PliegoRS or browser
success is inferred. External environment gates remain separate.
