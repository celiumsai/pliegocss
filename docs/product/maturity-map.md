# Product maturity map

Status: **normative candidate for 0.1.0 planning; generated claims are bounded by the linked gates**

PliegoCSS preserves broad intentional capability without pretending every implemented surface carries
the same support promise. The machine-readable authority is
[`maturity-map.json`](./maturity-map.json). This page explains how to read it.

## Levels

| Level | Meaning before RC | 0.1.x treatment |
|---|---|---|
| **Stable** | Selected for the supported `0.1.x` contract and protected by deterministic local gates. | The compatibility promise activates when an RC is cut. Breaking changes then require the documented migration/version policy. |
| **Beta** | Implemented and useful, but missing release-grade completeness, corpus, host, portability, or publication evidence. | Preserved intentionally, versioned where persisted, but not promoted silently into the stable promise. |
| **Experimental** | Bounded preview, research surface, or planned capability with explicit limitations. | May change before promotion; it must still fail honestly and must not weaken stable boundaries. |

A maturity label is not a quality score. For example, rollback-capable filesystem publication is
stable because its narrow guarantee is complete and tested. Static cascade explanation is
experimental because its intended product scope is explicitly broader than its safe current slice.

## Stable candidate surface

- Typed Rust authoring: `pc!`, `pcx!`, `Style`, and `StyleId`.
- Deterministic typed-utility compilation and selected format/version vectors.
- Schema-1 TOML themes and the Cargo `theme!` bridge.
- Finite one-shot CLI contracts, structured failures, inspection, catalog, explain, and formatting.
- Standard-CSS audit finding projections across human, JSON, and SARIF.
- Bounded no-follow filesystem reads and rollback-capable publication.

These claims are deliberately narrower than “all public Rust types,” “complete CSS,” or “published
packages.” Their exact boundaries are enforced by the public API, compatibility, determinism,
packaging, and portability gates named in the JSON map.
The pure `CompileRequest → CompileResult` engine, shared `PcxRequest` frontend, and
semantic/physical planner separation are implementation architecture inside that exact-version
unit; they do not expand the stable application API.

## Beta surface

Beta contains the implemented systems that need representative or hosted evidence before a stable
promise:

- compatibility policy and DTCG/token graph;
- semantic/physical provenance, bundles, Asset Plans, and Project Index;
- budgets, ownership, usage evidence, retention, and pruning;
- static accessibility policy;
- bounded agent repair and verification receipts;
- paired Tailwind Benchmark Authority v2 (latest, v3-LTS, and frozen regression lanes);
- computed and bounded visual browser-output certification against Tailwind across the declared
  3×3 host matrix;
- LSP/editor clients;
- migration inventory plus bounded static Tailwind v3/v4 coexistence across 21 HTML, Vite, and
  PliegoRS project snapshots with browser equivalence and exact rollback. Its 0.1.x adapter policy
  is frozen, while the separate G7 authority remains honestly blocked at zero external records;
- watch mode;
- PliegoRS SSR/SSG/resumability integration;
- exact-version crates.io distribution for the Rust workspace plus repository-hosted native CLI/LSP
  archives and a no-lifecycle pnpm package. The npm-format package is never published to npmjs.

Each row in the machine map names the owning paths, persisted schemas, current gates, graduation
criteria, and claim limits. A beta capability cannot graduate solely because its tests pass locally;
its listed evidence gap must also close.

## Experimental surface

- Static cascade explanation beyond its current closed slice.
- General standard-CSS transform/bundle output beyond the audit-first wedge.
- Responsive/layout browser diagnosis.

Experimental does not mean disposable. These are intentional product directions and may not be
removed merely to simplify the release. They remain bounded until their graduation criteria are met.

## Release rules

1. A capability moves upward only by updating `maturity-map.json`, this page when category meaning
   changes, and the relevant gate/evidence in the same reviewed change.
2. A persisted stable or beta schema never changes meaning without a schema/format transition.
3. Missing hosted or externally configured evidence remains `not-configured`, never `passed`.
4. Stable claims must be exercisable from registry-shaped artifacts before RC.
5. Beta and experimental capabilities remain documented and fail honestly; they are not deleted to
   manufacture a smaller release.
6. The strategic R0 contract remains authoritative: this map describes support maturity and does not
   waive required `0.1.0` gates.

## Verification

Run the closed map validator and documentation gate:

```console
pnpm check:maturity
pnpm check:docs
```

The validator rejects duplicate IDs, unknown maturity values, empty ownership/gate/graduation fields,
missing owner paths, and a mismatch between the expected stable/beta/experimental inventory and the
machine document.
