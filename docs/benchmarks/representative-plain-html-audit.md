# Representative application 1: plain HTML audit-first

Status: **passing local representative application gate**

This fixture proves the adoption wedge that must work before utility or framework migration: an
ordinary HTML page keeps its authored CSS and receives compatibility, budget, accessibility, finding,
manifest, and receipt evidence.

## Application

- `index.html`: semantic static page with no script or styling runtime.
- `app.css`: 1,090 bytes of ordinary layered CSS.
- `pliego.budgets.json`: file and layer budgets.
- `pliego.accessibility.json`: declared literal contrast plus bounded static policies.

The gate invokes `pliego-cssc audit` from the application directory with `baseline-widely`, both
policies, canonical JSON output, and a temporary control directory. It then invokes the exact same
command with `--check` and verifies the three-artifact group.

## Current evidence

- CSS SHA-256: `5127aaf56103198ac61eef16ea546d5fed4088799678d30a7c5d511329337bcf`
- Deterministic modern/minified output: 772 bytes, SHA-256
  `022e821e34905610a8882426abbaaa7716eef3363456c6cd1f3e5afc23ee6815`; drift-only `--check`
  passes while authored CSS remains unchanged.
- HTML SHA-256: `b8167948bacf43f8a885ea2252487a087f775b73a2549737dc001d01b60f28a0`
- 14 findings, including inventory, compatibility coverage, passing budgets, accessibility summary,
  and explicit warning/manual boundaries.
- No error-severity finding.
- Control manifest and receipt schemas: `1.0.0`.
- Authored CSS and HTML remain unchanged.

The compatibility coverage warning and accessibility warning/manual findings are preserved rather
than relabeled as proof. This application does not claim complete CSS classification, WCAG certification, or browser visual
correctness. The transformed output is not yet part of the audit control manifest/receipt graph.

## Reproduction

```console
pnpm integration:representative:plain-html-audit
```

This is application **1 of 5** required before selecting implementation gaps.
