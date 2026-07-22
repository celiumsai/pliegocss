<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/manifests/","category":"Evidence","eyebrow":"Style manifests","order":16}
-->

# Trace source intent into emitted bytes.

Manifest schema 5 connects source origins, semantic assignments, final selectors, physical declarations, and CSS ranges.

## Identity chain {#identity}

StyleId identifies canonical semantics. Class identity, theme identity, configuration, and final CSS hash bind that style into one build.

```console
pliego-cssc compile --source src --manifest dist/app.manifest.json \
  --manifest-version 5 --output dist/app.css
```

## Physical lineage {#physical}

Generated custom properties and synthesized declarations retain contributors, importance, conditions, selector, property, value, and output byte range.

## Request the schema you consume {#versions}

Schema 3 supports style manifests, schema 4 adds reachability ownership, and schema 5 adds closed physical tracing. Consumers must pin their contract.
