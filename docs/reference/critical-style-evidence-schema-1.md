# Critical-style evidence schema 1

Status: **canonical builder/parser/verifier implemented; CSS projection and browser producer pending**

The sidecar is an explicit input from a browser or adapter producer. It is not auto-discovered and
does not replace full stylesheets.

```json
{
  "schemaVersion": 1,
  "evidenceKind": "pliegocss-critical-style-capture/1",
  "universeSha256": "64-lowercase-hex-digits",
  "reachabilitySha256": "64-lowercase-hex-digits",
  "producer": { "name": "pliegocss-browser-gate", "version": "1.0.0" },
  "unknownDynamicInputs": ["authenticated-state"],
  "captures": [{
    "id": "home-mobile",
    "routeId": "route:home",
    "routePath": "/",
    "browser": "chromium-150",
    "viewportWidth": 390,
    "viewportHeight": 844,
    "stage": "largest-contentful-paint",
    "styles": [{
      "bundleId": "application",
      "styleId": "0123456789abcdef0123456789abcdef"
    }]
  }]
}
```

`stage` is closed to `first-contentful-paint`, `largest-contentful-paint`, or `adapter-ready`.
Viewport dimensions are positive CSS pixels. Capture IDs are unique; arrays and bundle-qualified
styles are canonical, bounded, duplicate-free, and case-sensitive. Objects reject unknown fields.

Verification requires:

- byte-canonical JSON with one final LF and at most 16 MiB;
- exact usage-universe and reachability SHA-256 bindings;
- an existing route ID with the identical path;
- each bundle in that route's Asset Plan selection; and
- each StyleId in the immutable retained compiler selection for that bundle.

Multiple captures may name the same route. Their positive StyleIds are unioned. Missing hits and
`unknownDynamicInputs` never authorize removal; the normal route/island stylesheets remain the
correctness path. The evidence proves only the declared capture matrix.

See [ADR-0022](../adr/0022-require-explicit-critical-style-evidence.md).
