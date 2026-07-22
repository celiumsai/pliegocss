<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/first-audit/","category":"Start","eyebrow":"First audit","order":8}
-->

# Learn before changing.

Run a read-only CSS audit, inspect canonical findings, then decide whether transformation or policy is justified.

## Read the first report {#human}

Human output is optimized for local diagnosis. Every finding carries a stable code and source location rather than an opaque score.

```console
pliego-cssc audit --input app.css \
  --targets baseline-widely --format human
```

## Feed automation safely {#automation}

Canonical JSON and SARIF preserve the same finding model for CI, code scanning, and archival evidence.

```console
pliego-cssc audit --input app.css --targets modern --format sarif > findings.sarif
```

## Unknown means unknown {#unknown}

The standard-CSS classifier is bounded. Unsupported selectors, properties, or values fail closed under compatibility policy rather than being treated as safe.
