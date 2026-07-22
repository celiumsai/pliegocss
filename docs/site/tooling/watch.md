<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/tooling/watch/","category":"Tooling","eyebrow":"Watch mode","order":24}
-->

# Keep the last complete truth.

Poll coherent snapshots, republish only valid groups, and retain the last valid output across transient edits.

## Watch inputs and output {#run}

The watcher accepts the same source, theme, target, format, manifest, reachability, and controlled-output options as compile.

```console
pliego-cssc watch --source src --theme \
  --output dist/app.css --manifest dist/app.manifest.json
```

## Confirm one coherent snapshot {#snapshot}

Inputs are observed twice before compilation so an editor save sequence does not publish a mixture of old and new files.

## Never replace validity with a partial edit {#last-valid}

A parser error, invalid token document, or failed group publication leaves the previous complete artifact set in place.
