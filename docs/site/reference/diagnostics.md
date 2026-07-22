<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/reference/diagnostics/","category":"Reference","eyebrow":"Diagnostics","order":29}
-->

# Stable codes, precise ranges.

Parser, source scanner, composition, policy, invocation, migration, and formatter failures share a canonical transport.

## Human diagnostics {#human}

Terminal output favors the next corrective action while retaining the stable code, severity, file, line, column, and byte range.

## Canonical JSON {#json}

JSON mode writes one schema-1 document to stderr on failure and leaves stdout empty. Consumers should branch on codes, not localized prose.

```console
pliego-cssc --diagnostic-format json check --style "p-4 p-6" --seed
```

## SARIF for code scanning {#sarif}

Audit findings can be serialized as SARIF while preserving canonical rule identity and source regions for hosted review systems.
