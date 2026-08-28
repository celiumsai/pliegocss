<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/control-artifacts/","category":"Evidence","eyebrow":"Control artifacts","order":17}
-->

# Publish the receipt last.

Controlled builds stage related outputs, lock the destination, verify hashes, compensate failures, and expose one completed receipt.

## One rollback-capable group {#group}

CSS, source map, style manifest, Token Graph, findings, control manifest, and receipt are generated from one exact input snapshot.

```console
pliego-cssc compile --source src --output dist/app.css \
  --manifest dist/app.manifest.json --control-dir dist
```

## Check without writing {#check}

The same command with --check recomputes every artifact and fails on byte drift without taking publication locks or replacing files.

```console
pliego-cssc compile --source src --output dist/app.css \
  --manifest dist/app.manifest.json --control-dir dist --check
```

## Completion is explicit {#receipt}

Consumers treat the fixed receipt as the publication boundary. Its absence means the group is incomplete, even if individual files are present.
