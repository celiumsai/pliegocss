<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/audit-and-transform/","category":"Standard CSS","eyebrow":"Audit and transform","order":3}
-->

# Standards-first, not framework-first.

Analyze standard CSS, enforce policy, and transform output without claiming ownership of authored source.

## Bounded classification {#classifier}

The classifier recognizes deliberate standard-CSS surfaces and reports unsupported constructs instead of guessing. A versioned corpus controls the claim.

## Deterministic transformation {#transform}

Target browsers, minify, and check output drift. Add --check in CI to remain read-only.

```console
pliego-cssc transform-css --input app.css \
  --output dist/app.css --targets modern --format minified
```

## Compatibility, accessibility, and budgets {#policy}

Policy documents bind findings to explicit targets and limits. The same canonical findings feed terminals, JSON automation, and SARIF.
