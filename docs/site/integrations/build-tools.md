<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/integrations/build-tools/","category":"Integrations","eyebrow":"Generic build tools","order":27}
-->

# Integrate through files, not hidden hooks.

Vite, webpack, esbuild, CI systems, and custom pipelines consume explicit CSS, manifests, findings, maps, and asset plans.

## Run PliegoCSS as a producer {#producer}

Invoke compile or transform before the application bundler and pass its ordinary CSS output through the normal asset graph.

```text
"scripts": {
  "css": "pliego-cssc compile --source src --output dist/pliego.css",
  "build": "npm run css && vite build"
}
```

## Consume stable sidecars {#consumer}

Use the numbered JSON schemas for manifests, Token Graphs, findings, reachability, project indexes, and Asset Plans instead of scraping terminal text.

## Keep topology in its owner {#ownership}

A framework adapter may export route and component reachability. PliegoCSS validates and projects that sidecar but does not crawl application internals.
