<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/evidence/","category":"Evidence","eyebrow":"Evidence and provenance","order":4}
-->

# A green build should explain itself.

Artifacts carry hashes, source lineage, graphs, findings, and receipts designed for replay.

## Canonical manifests {#manifest}

Style identity, class identity, theme identity, origins, conditions, physical declarations, and final CSS hashes live in a deterministic document.

```console
pliego-cssc compile --source src --seed --theme \
  --output dist/app.css --manifest dist/app.manifest.json
```

## Receipt-last publication {#receipts}

Controlled operations stage output, lock the destination, verify inputs, publish artifacts, and write the receipt last.

## Measured, inherited, pending, uncertain {#benchmarks}

Release readiness preserves the evidence class. Old hosted evidence never silently passes changed source.
