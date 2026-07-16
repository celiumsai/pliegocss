# PliegoCSS + PliegoRS development-loop fixture

This detached Rust project is the executable source for the F6 two-process development gate. The
gate copies it to a disposable directory, rewrites only its path dependencies to absolute local
checkout paths, and then runs:

```console
pliego-cssc watch --source src --input ../driver.styles.txt --theme --format pretty \
  --output assets/pliego.css --manifest assets/pliego.manifest.json
pliego dev <ephemeral-loopback-port>
```

The fixture deliberately reads the generated stylesheet at site-build time. A valid `pc!` edit must
therefore converge across both watchers into matching HTML and CSS. A compile-time-invalid utility
inside otherwise valid Rust must retain the last valid site, and the tested semantically identical
external input must not replace either published artifact or trigger a PliegoRS rebuild.

Run the complete local gate from the PliegoCSS root with:

```console
node scripts/check-pliegors-dev-loop.mjs
```

The sibling `pliegors` checkout must match the revision and source contract pinned by the existing
PliegoRS integration fixture.
