# Reviewed public migration role corpus

Status: **three pinned MIT projects pass the bounded file-role discovery gate; this is not a
whole-migration accuracy claim**

The network-gated corpus clones exact revisions of three official public repositories into an
ignored temporary directory, verifies the pinned MIT license bytes, runs
`migration-project-inventory .`, and compares every emitted file-role tuple with reviewed gold
labels. Run it from the repository root:

```console
PLIEGOCSS_RUN_NETWORK_CORPUS=1 pnpm check:migration-real-corpus
```

`PLIEGOCSS_BIN` may select an already-built CLI. `PLIEGOCSS_KEEP_REAL_CORPUS=1` preserves the
temporary sparse checkouts for inspection; the default removes them. A run without
`PLIEGOCSS_RUN_NETWORK_CORPUS=1` reports `skipped` instead of silently using stale local clones.

## Frozen evidence — 2026-07-16

The canonical Debian WSL2 replay passed all three exact commits:

| Case | Official revision | Reviewed files | Gold roles | TP | FP | FN | Precision | Recall |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Bootstrap Sass | `twbs/bootstrap@6f20e52759a0e0dee2c2f171e9357f3ccca52992` | 104 | 99 | 99 | 0 | 0 | 1.000 | 1.000 |
| Tailwind Vite playground | `tailwindlabs/tailwindcss@094bf62605870311a8def6ae45c87d578b198ebf` | 7 | 3 | 3 | 0 | 0 | 1.000 | 1.000 |
| Next.js basic CSS Modules | `vercel/next.js@e75d082512cd7633d0fd91e04ea2dafd4a29f861` | 8 | 2 | 2 | 0 | 0 | 1.000 | 1.000 |
| **Aggregate** | — | **119** | **104** | **104** | **0** | **0** | **1.000** | **1.000** |

The gold unit is one `(role, project-relative file)` tuple. Bootstrap's reviewed rule labels every
materialized `scss/**/*.scss` file as `source:sass`; Tailwind and Next.js use exact tuples for the
entry, templates, module, and consumer. Every other materialized file is a negative for the roles
emitted by the collector. The runner also freezes selected inventory summaries, requires canonical
JSON with trailing LF and empty stderr, and proves the Git checkout remains clean.

The selected semantic observations were 9,062 Sass constructs with 144 dependencies in Bootstrap,
six static Tailwind candidates in two templates, and one exact CSS Modules import plus one static
class use in Next.js. All 144 Bootstrap dependencies resolve to declared Sass sources through the
closed relative file/partial/index/import-only rules; three representative exact edges are frozen
in the manifest. Those counts are regression expectations, not complete separately annotated
semantic precision measurements.

## Claim boundary

The measured precision and recall apply only to discovery of the currently emitted source,
consumer, and auxiliary roles on these 119 reviewed files. They do not measure correctness of every
construct, dependency resolution against an original toolchain, arbitrary template languages,
codemod output, visual equivalence, developer resolution time, or generalize to all projects.

This corpus supplies reproducible real-project evidence for R0.8, but R0.8 remains partial while
configured load paths/importers, broader syntax, migration outcomes, and a larger representative
sample are open. Diagnostic precision/recall across the complete product and the incident/interview
research needed for broader claims may be added as real usage grows; neither is a prerequisite for
continuing the bounded migration work.
