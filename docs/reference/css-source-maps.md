# Canonical CSS Source Map v3

Status: implemented for generated control groups

PliegoCSS emits a source-map sidecar only when it owns a generated, integrity-bound output group:

- `compile`/`build --control-dir DIR`;
- `watch --control-dir DIR`;
- `bundle --control`.

For `app.css`, the fixed derived path is `app.css.map`. Normal compile/watch, direct standard-CSS
audit, and independently reopened Asset Plan audit do not create a map because those paths either did
not request a complete control group or do not own CSS generation.

## Wire shape

The sidecar is compact UTF-8 JSON with these fields in exact order and one trailing LF:

```json
{"version":3,"sources":["src/app.rs"],"names":[],"mappings":"AAAA"}
```

The contract is:

- `version` is exactly `3`;
- `sources` contains sorted, unique portable logical paths from the exact input ledger that received
  at least one mapping;
- `names` is empty because generated class identities already live in the CSS and style manifest;
- `mappings` uses standard Base64 VLQ segments;
- generated and original positions are zero-based UTF-16 line/column units;
- `sourcesContent`, host paths, clocks, and absolute URLs are absent.

The source ledger and its SHA-256 leaves already live in `pliego.css.manifest.json`; omitting source
content avoids silently copying application source into deployment artifacts. A consumer that needs
the text must reopen the exact hashed workspace input.

## Mapping semantics

Each emitted `.pc_*` class-selector start maps to one authored origin. PliegoCSS selects the first
normalized style origin whose file and UTF-8 byte offset resolve inside the exact source snapshot.
The offset is converted to the UTF-16 coordinates required by Source Map v3. A class absent from the
final CSS or an origin outside the exact ledger fails generation instead of producing a guessed map.

Inline `--style` and `--compose` values have no physical file. Their deterministic fallback is the
canonical virtual input `pliego.cli-input.json` at line 0, column 0.

A standard source-map segment can encode only one origin. When equivalent styles are co-owned by
several sites, the source map exposes the deterministic primary origin while style manifest 3–5
retains the complete normalized origin list. Source maps are diagnostic rule-level evidence. They
must not be treated as schema-5 proof of final declaration ownership or physical byte ranges.

## Discovery and integrity

Generated CSS does not receive a `sourceMappingURL` comment, so opting into maps cannot change CSS
bytes, StyleIds, caching, or ordinary compile/watch output. Tools discover the sidecar through the
owning CSS output in `pliego.css.manifest.json`:

```json
{
  "artifact": {"file": "app.css", "bytes": 424, "sha256": "sha256:..."},
  "role": "generated-css",
  "mediaType": "text/css",
  "sourceMap": {"file": "app.css.map", "bytes": 79, "sha256": "sha256:..."},
  "relationships": ["app.css.map", "app.manifest.json"]
}
```

That reference must exactly match another output with role `css-source-map`, media type
`application/json`, and a relationship back to the CSS. Missing outputs, byte/hash drift,
self-references, wrong roles, and wrong media types fail parsing. The adjacent build receipt binds
both artifacts. Group publication and finite `--check` include the map, so handled write failures
roll back it with its siblings and drift checks remain read-only.

## Current limits

- No browser discovery comment is emitted. DevTools integration must consume the control manifest or
  a later explicit adapter.
- `sourcesContent` is intentionally absent.
- Mapping granularity is generated class selector to primary authored origin, not every declaration,
  token, or transformation stage.
- Hosted macOS and clean hosted-runner evidence remain release gates even though the frozen local
  Windows x64 and Debian WSL2 Linux x64 vectors are byte-identical.

See [control artifacts schema 1.0.0](./control-artifacts-schema-1.md),
[manifest schema 5](./manifest-schema-5.md), and the
[portability evidence](../status/portability.md).
