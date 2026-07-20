# PliegoCSS and Tailwind CSS v4.3.2

Status: **verified competitive matrix for release planning, not a claim of full feature parity**

This comparison pins Tailwind CSS **4.3.2** and separates representative parity, product
differentiation, Tailwind advantages, intentional design differences, and open release gaps. The
machine authority is [`tailwind-v4-competitive-matrix.json`](./tailwind-v4-competitive-matrix.json).

PliegoCSS is not positioned as “Tailwind in Rust.” Tailwind is the production utility-framework
baseline; PliegoCSS is a standards-first compiler/verifier with an optional typed utility frontend.
A competitive `0.1.0` therefore needs a credible authoring path and superior verifiability, not a
line-by-line clone of Tailwind's catalog.

## Pinned evidence

- Installed `tailwindcss` and `@tailwindcss/cli`: **4.3.2**, MIT.
- Complete benchmark fixture: five views, 44 class attributes, 302 utility occurrences, 90 unique
  concrete utility tokens.
- Fixture SHA-256: `338eb480327e54fe7e56e81b42c67324464afaa8c65d606e5d57c1c45690e86c`.
- Upstream documentation was read from the v4.3 installation, compatibility, source-detection,
  theme, states, and custom-style pages named in the machine matrix.
- Pliego evidence comes from frozen Gate A/B results and current machine-enforced contracts.

## Executive matrix

| Dimension | Assessment | Honest conclusion |
|---|---|---|
| Five-view utility fixture | Matched | Pliego compiles all tracked class lists and eight tracked variants. This is representative coverage, not full catalog parity. |
| Catalog breadth | Tailwind advantage | Tailwind has a much broader production utility catalog. Pliego should add only evidence-backed families. |
| Responsive/state authoring | Matched slice | `sm/md/lg`, hover/focus/focus-visible/placeholder/disabled pass the shared fixture; Pliego adds typed condition normalization. |
| Arbitrary CSS escape | Matched slice | Both preserve escape hatches; Pliego intentionally validates a narrower grammar. |
| Theme model | Different by design | Tailwind uses CSS-first `@theme`; Pliego uses typed TOML/DTCG, theme identity, and a canonical token graph while retaining CSS interoperability. |
| Source discovery | Different by design | Tailwind scans complete text tokens; Pliego prefers visible Rust literals or explicit typed adapter manifests and refuses inferred ownership. |
| Reset | Tailwind advantage | Tailwind ships Preflight; Pliego currently requires an external reset. No-preflight is the correct primary benchmark. |
| Fixture fresh-process latency | Matched/competitive | Frozen medians: Pliego 37.894 ms versus Tailwind no-preflight 204.826 ms. This is fixture evidence only. |
| Fixture payload | Matched/competitive | Pliego has larger raw CSS but smaller gzip CSS and combined transfer under the frozen fixture. All three metrics remain visible. |
| Rust cold build | Tailwind advantage | Typed macros add significant cold Cargo cost; standard-CSS audit must remain valuable without Rust migration. |
| Semantic diagnostics | Pliego differentiator | Typed domains, conflict slots, impossible conditions, and stable diagnostics fail early instead of silently dropping intent. |
| Standard-CSS governance | Pliego differentiator, open R0 work | Audit/policy/receipts exist, but complete supported classification and the generic transform/minify path are release gaps. |
| Provenance and receipts | Pliego differentiator | Versioned semantic/physical graphs and integrity-bound output/check receipts exceed the compared utility generation contract. |
| Budgets and usage evidence | Pliego differentiator | Ownership-backed budgets and conservative deadness/pruning exist; generic-CSS granularity and hosted evidence remain open. |
| Accessibility guardrails | Pliego differentiator | Explicit verified/unverified/manual/exception states exist; no WCAG certification is claimed. |
| Agent repair | Pliego differentiator, beta | Closed plans, authorization, rollback, fixed checks, and receipts exist; real-incident efficacy and hosted approvals are open. |
| Migration | Open gap | Inventory is read-only and bounded; reversible semantic codemods and visual proof are not complete. |
| Ecosystem/install | Tailwind advantage | Tailwind is published and broadly integrated; Pliego remains checkout-distributed with a strong but bounded PliegoRS path. |
| Browser support evidence | Open gap | Pliego's policy is versioned, but complete classification and hosted Chromium/Firefox/WebKit evidence block `0.1.0`. |
| Plugin model | Different by design | Pliego intentionally rejects unrestricted extension execution until a deterministic sandboxed contract exists. |

## Frozen fixture results

The closest payload comparison is Tailwind **no-preflight**, because PliegoCSS emits no reset.

| Metric | Pliego complete | Tailwind no-preflight complete |
|---|---:|---:|
| Fresh-process median | 37.894 ms | 204.826 ms |
| Fresh-process minimum | 35.811 ms | 185.374 ms |
| Fresh-process p95 | 41.290 ms | 418.003 ms |
| CSS raw | 10,463 B | 8,618 B |
| CSS gzip | 2,049 B | 2,327 B |
| HTML gzip | 1,501 B | 1,506 B |
| HTML + CSS gzip | 3,550 B | 3,833 B |

These figures do **not** prove universal speed, payload, visual parity, or production superiority.
They freeze one shared fixture, executable, methodology, and host. Tailwind full-import figures include
Preflight and are context only.

## What “competitive 0.1.0” means

A release is competitive when all of the following are true:

1. The matched fixture and selected stable authoring API remain green.
2. Standard CSS receives first value without Rust or utility migration.
3. R0 compatibility, receipts, budgets, token graph, accessibility, report parity, and
   determinism/portability gates close honestly.
4. Packages and installation are replayed from actual RC artifacts.
5. Pliego's differentiators remain useful in representative applications, not merely synthetic unit
   fixtures.
6. Tailwind advantages are documented rather than hidden; catalog breadth is not used as the sole
   definition of competitiveness.

## Priority gaps selected by the matrix

### Blocking `0.1.0`

- Complete the supported standard-CSS transform/minify path and classification corpus.
- Converge the release manifest/receipt evidence without reinterpreting existing schemas.
- Close generic-CSS budget/usage identity gaps required by R0.
- Add accessibility precision/recall and browser/manual evidence.
- Obtain hosted cross-OS and Chromium/Firefox/WebKit evidence.
- Publish and replay RC artifacts after owner approval.

### Should before release

- Turn bounded migration inventory into at least one conservative reversible migration slice with
  visual/receipt evidence, or move it through the required reviewed ADR.
- Prove one additional non-PliegoRS typed host adapter and document installation compatibility.

### Explicitly post-0.1

- Full Tailwind catalog parity.
- Responsive causal diagnosis.
- Unrestricted plugins.
- Broad automatic migration or perfect-migration claims.

## Verification

```console
pnpm check:tailwind-matrix
pnpm check:maturity
pnpm verify:fast
```

The matrix validator binds the pinned package and fixture hashes, validates all source references and
status/release-impact enums, rejects duplicate dimensions, and freezes the status counts. It does not
fetch the network during normal verification.
