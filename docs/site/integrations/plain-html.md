<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/integrations/plain-html/","category":"Integrations","eyebrow":"Plain HTML","order":26}
-->

# Audit and transform without a framework.

Use PliegoCSS as a standards control layer around an ordinary stylesheet and static HTML build.

## Start with emitted CSS {#audit}

No adapter is required. Audit the exact stylesheet the browser loads and archive canonical findings beside the build.

```console
pliego-cssc audit --input public/app.css --targets baseline-widely
```

## Transform explicitly {#transform}

Compatibility transformation and minification are separate from analysis so a read-only audit never rewrites authored output.

```console
pliego-cssc transform-css --input src/app.css \
  --output public/app.css --targets modern --format minified
```

## Typed authoring is optional {#typed}

Rust applications can compile typed styles into the same static site, but plain HTML consumers only need the final CSS asset.
