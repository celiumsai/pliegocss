# Control manifest and build receipt schema 1.0.0

Status: **wire contract plus direct-CSS/Asset-Plan audit, exact-snapshot compile/watch, generated
Source Map v3, canonical TokenGraph, and optional Usage Analysis publication, and bundle-build
generation/grouped publication implemented; audit-side accessibility policy and optional TokenGraph evidence are implemented;
DTCG-backed direct CLI and bundle-plan schema-2 selection have local gates green; Cargo build-macro
selection passes local workspace/MSRV/API and clean-package gates, while full attribution and
runtime accessibility evidence remain open**

PliegoCSS reserves two adjacent, canonical JSON artifacts:

- `pliego.css.manifest.json` explains the complete input, policy, backend, output, measurement,
  token, decision, and finding state of one output group;
- `pliego.css.receipt.json` hashes the exact manifest bytes and records the checks needed to trust
  that group.

The Rust contract lives in `pliego-css-control`. Unknown fields, unsupported schema versions,
absolute or host-specific paths, malformed hashes, duplicate identities, noncanonical collection
order, inconsistent cross-references, and documents larger than 16 MiB fail closed.

## Integrity graph

```text
source/config/policy/adapter inputs
                 |
                 v
CSS + maps + specialized manifests + Asset Plan/Project Index/Usage Analysis +
pliego.tokens.json + findings
                 |
                 v
       pliego.css.manifest.json
                 |
                 v
        pliego.css.receipt.json
```

The manifest never contains its own hash or the receipt hash. Its `receipt` section declares only
the expected schema, fixed adjacent filename, required check IDs, and expected derived result. The
receipt then records the byte length and SHA-256 of the complete manifest. This direction avoids a
self-hash or two-document hash cycle.

Every hash has the exact form `sha256:<64 lowercase hex>`. Semantic identity excludes wall-clock
build timestamps. Compatibility evidence uses an explicit official dataset source, version, and
`YYYY-MM-DD` evidence date.

## Control manifest

The top-level field order is fixed:

```json
{
  "documentKind": "pliego-css-control-manifest",
  "schemaVersion": "1.0.0",
  "tool": {},
  "inputs": {},
  "backend": {},
  "targets": {},
  "outputs": [],
  "rules": {},
  "tokens": {},
  "decisions": [],
  "violations": {},
  "receipt": {}
}
```

Important invariants:

- `inputs.files` sorts uniquely by logical `file`; adapters sort uniquely by `name`;
- `backend.stages`, target browsers, relationships, evidence IDs, output references, exception IDs,
  and required checks are sorted and duplicate-free;
- outputs sort uniquely by `artifact.file`;
- measured `pliegocss-token-graph/1` state requires exactly one `pliego.tokens.json` output with
  role `token-graph`, media type `application/json`, no source map, and SHA-256 equal to
  `tokens.graphHash`; every generated CSS/style manifest and the graph carry reciprocal relationships;
- graph-bearing manifests require `token-graph-integrity`; the receipt must mark it required and
  passed with evidence equal to the exact `pliego.tokens.json` output reference;
- `sourceMap` appears only on `text/css`, cannot point to its owner, and must match the bytes/hash of
  one `application/json` output whose role is exactly `css-source-map`;
- the CSS and map must reference each other through `relationships`; an unowned `css-source-map`
  output is invalid;
- a bundle `usage-analysis` output is `application/json`, relates to every generated CSS/style
  manifest and to the requested Asset Plan/Project Index, and remains a separate usage-state
  contract rather than populating `rules.byState`;
- every `decisions[].affectedOutputs` entry resolves to an output in the same manifest;
- each decision fingerprint is recomputed from its ID, feature, action, reason, evidence, and
  affected outputs;
- `rules` and `tokens` declare `observation: measured|unavailable`; unavailable sections require a
  reason and are forbidden from carrying invented counters, hashes, coverage, or attribution;
- measured audit token state may carry `coverageBasisPoints: 0`: this means a supplied graph was
  measured but the standard-CSS audit had no compiler-owned typed-use set. It is distinct from
  `observation: unavailable`, whose coverage must be absent;
- `tokens.contrastPairs` counts relationships declared by accessibility policy, not the number of
  themes in the graph or the number of contrast findings emitted;
- `violations.document` references the canonical finding 1.0.0 document rather than duplicating its
  findings;
- `violations.parityPassed` reports whether the human, JSON, and SARIF projections were proven to
  contain the same canonical findings.

Counts whose ownership cannot be proven use an explicit `unknown` attribution key. Producers must
not infer route, package, component, or state ownership from filenames or naming conventions.

An explicitly supplied, validated [ownership sidecar](ownership-schema-1.md) can support package
and composed-route **budget subjects**, but those findings do not manufacture Control Manifest
partitions. Route views overlap by design, so `rules.byRoute` remains `unknown`. Package projection
is likewise deferred until a future Control Manifest schema defines and separately validates that
partition, even though schema 1 ownership maps every bundle to exactly one package.

## Build receipt

The receipt repeats the source/config hashes, backend, and target identity; binds the exact manifest
and every output; summarizes decisions and findings; and lists canonical checks. Its `result` is
derived rather than asserted:

- `passed` requires every required check to be `passed` and zero unexcepted error findings;
- otherwise the result is `failed`;
- a passed browser check must reference a hashed evidence artifact; static analysis alone cannot be
  labeled browser evidence.

The library validates the exact supplied manifest bytes. Producers must also construct each output
and evidence reference from the exact in-memory bytes being published. Both audit modes do this and
publish findings/manifest/receipt through the rollback-capable output-group boundary.

## CLI generation

For direct CSS or an Asset Plan, use an existing directory:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --control-dir reports/pliegocss --format json
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --control-dir reports/pliegocss --check
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json \
  --token-graph dist/pliego.tokens.json --control-dir reports/pliegocss
pliego-cssc audit --asset-plan dist/pliego.assets.json --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json \
  --control-dir reports/pliegocss
pliego-cssc bundle --plan pliego.bundles.toml --output-dir dist \
  --manifest-version 5 --reachability pliego.reachability.json \
  --asset-plan --project-index --usage-report --control
pliego-cssc compile --source src --seed --output dist/app.css \
  --manifest dist/app.manifest.json --control-dir dist --check
pliego-cssc watch --source src --seed --output dist/app.css \
  --manifest dist/app.manifest.json --control-dir dist
```

The terminal renderer remains independent: `pliego.css.findings.json` is always canonical finding
schema 1.0.0, even when stdout is human or SARIF. The generated manifest uses measured CSS inventory
when parsing succeeds, explicit `unknown` ownership where no typed source attests ownership, and
`unavailable` token observation when no typed graph is supplied. Failed syntax produces a failed
receipt and unavailable rule measurement instead of suppressing evidence.

`audit --accessibility-policy FILE` works with both direct CSS and Asset Plan input. Exact policy
bytes are config-ledgered as `accessibility-policy`. Optional `--token-graph FILE` requires that
policy, enters the same ledger as `token-graph`, and publishes its canonical bytes at fixed
`pliego.tokens.json`. The budget-policy role remains `policy`; the Asset Plan source role is
`asset-plan`. All appear in `inputs.files` with their exact hashes. Policy and graph contribute to
`configHash`; the plan, CSS, and style-manifest source set contributes to `sourceHash`.

A graph-bearing audit group contains `pliego.tokens.json`, findings, control manifest, and receipt.
The token graph output has role `token-graph`, its hash must equal `tokens.graphHash`, and the required
`token-graph-integrity` receipt check points to that exact output. Without a supplied graph, audit
preserves the established three-artifact group and does not reconstruct token semantics from CSS.

Asset Plan audit extends the source set to the exact plan plus each adjacent CSS/style-manifest
pair, after proving byte-identical canonical regeneration. Rule totals aggregate all bundles and the
receipt requires `asset-plan-integrity`. Overlapping route budget views do not become false
`rules.byRoute` partitions.

The executable direct-CSS portability vector freezes the complete control group:

| Control artifact | SHA-256 |
|---|---|
| `pliego.css.findings.json` | `75015787cd20bd52e4c1d504943b675785f55d7376a583a103cad4ab5c7de464` |
| `pliego.css.manifest.json` | `bac4099153b23cfd990fc2e70506153d08b725ca27661ad102934f45df721ac1` |
| `pliego.css.receipt.json` | `cb680d32a41321b089cf2fd98b7f60501fa8ba306f408d5f448a11551598b9b1` |

Those bytes matched on local Windows x64 and a Linux x64 binary under Debian WSL2 on 2026-07-14;
the vector also proves that control `--check` is read-only. Those no-graph vectors remain
three-artifact groups. Direct CSS and independently reopened Asset Plan audit become four-artifact
groups only when `--token-graph` is supplied. Generated compile/watch groups contain seven artifacts.
The one-bundle portability fixture with Asset Plan and Project Index contains nine outputs; adding
Usage Analysis and Token Usage makes that group eleven. Their
TokenGraph-bearing hashes are frozen and green on local Windows and Debian WSL2 Linux x64; hosted
runners and macOS remain open gates.

`bundle --control` audits every generated CSS payload before publication and adds one shared
`pliego.tokens.json` plus the canonical findings/manifest/receipt files to the existing bundle
output group. Its input ledger hashes the
exact bundle plan, optional theme configuration or Resolver, reachability document, optional usage
observation, optional usage retention, and Rust source snapshots. A schema-2 DTCG Resolver has role
`token-resolver`; the plan keeps role `bundle-plan`, observation bytes use `usage-observation`, and
retention bytes use `usage-retention`. Every exact optional config input participates in
`configHash`.
Its output ledger hashes every CSS/source-map/style-manifest set, TokenGraph, Asset Plan, optional
Project Index, optional Usage Analysis, optional Token Usage, and findings document. Relationships connect CSS and style manifests to the shared
graph, the graph back to those outputs, each map back to its CSS, the Asset Plan to all CSS/manifest
pairs, the Project Index to the Asset Plan, and Usage Analysis to the generated bundle artifacts it
explains. One
rollback-capable publication and the existing bundle
`--check` cover the entire group.

For a schema-2 `dtcg-resolver` plan, CSS and retained-token coverage use the selected registry while
`pliego.tokens.json` contains the complete validated Resolver graph. The exact plan bytes—including
`[theme.inputs]`—already participate in `configHash`, independently of the selected `ThemeId`.
Bundle identity therefore does not collapse textually distinct plans that happen to resolve to the
same canonical contexts, registry, or CSS. Resolver/selection failure aborts before publication.
This producer path passes its local Windows/WSL E2E, workspace, Rust 1.85, and package gates;
hosted/macOS evidence remains a release boundary.

Controlled compile/build and watch use a common output root. They snapshot line input, expanded Rust
sources, the resolved TOML theme or explicit DTCG Resolver file, and reachability once; compilation,
generated-CSS audit, manifest, and receipt all derive from those exact bytes. A Resolver file enters
the config ledger with role `token-resolver`. Inline styles/compositions are represented by canonical
virtual source bytes. A separate canonical compiler configuration binds operation, output format,
theme emission/selection/identity, manifest version, pruning, and canonical Resolver selections.
Those selections participate in `configHash` independently of `ThemeId`. CSS, canonical map,
specialized manifest, and `pliego.tokens.json` form one related set, while findings relate to all
four. Finite compile/build supports read-only `--check`; watch publishes each confirmed valid
snapshot and retains the previous complete group after invalid input.

### Generated CSS source maps

Controlled compile/watch and `bundle --control` emit one compact canonical Source Map v3 at
`CSS_PATH.map`. The map records final generated selector positions and source positions in zero-based
UTF-16 units, as required by the source-map format. `sources` contains sorted portable logical paths
from the exact source ledger; inline `--style`/`--compose` input maps to
`pliego.cli-input.json:0:0`. `names` is empty, `sourcesContent` is intentionally omitted, and the JSON
ends in exactly one LF.

One source-map segment can carry one origin. For a style with multiple origins, PliegoCSS chooses the
first normalized origin that resolves in the exact ledger. The specialized style manifest remains
authoritative for complete many-to-many provenance. These maps are rule-level diagnostic evidence;
they do not replace schema-5 declaration-level physical tracing.

CSS bytes remain unchanged: PliegoCSS does not inject a `sourceMappingURL` comment. Consumers locate
and verify the map through `outputs[].sourceMap`; a tampered, missing, self-referential, wrong-role,
or wrong-media-type map fails the control contract. Normal compile/watch, direct standard-CSS audit,
and reopened Asset Plan audit do not invent generated maps. See the full
[source-map contract](./css-source-maps.md).

Generated compile/watch and bundle groups publish `pliegocss-token-graph/1`. Its `graphHash` is the
SHA-256 of exact canonical `pliego.tokens.json` bytes. Token coverage starts with distinct references
from retained styles and follows winning alias/derived edges transitively; bundle coverage unions
references across compiled bundles. The graph schema and DTCG Resolver preserve aliases, derived
values, deprecations, provenance, and validated theme permutations.

Audit-side graph projection uses a different, explicit boundary. It counts the union of declared
projected tokens across validated themes and records `coverageBasisPoints: 0`, because arbitrary
standard CSS has no compiler-owned typed-use references. That is measured no-use-evidence, not an
unavailable graph and not a zero-token-usage conclusion. Its `contrastPairs` value comes from the
accessibility policy's declared relationships and is not multiplied by graph themes.

For TOML/seed input the CLI calls `TokenGraph::from_registry`, producing one literal `default` theme,
empty selections, and no DTCG adapter. With explicit `--tokens FILE`, it publishes the Resolver's
complete canonical graph, selects one permutation through repeatable `--token-input`, and binds the
canonical selections plus exact Resolver bytes into `configHash`. Bundle schema 2 expresses the
same Resolver selection inside exact plan bytes without adding those flags and passes its local
implementation gates. The matching Cargo build/macro bridge selects only the active registry for
`pc!`/`pcx!`; it does not publish this complete graph, so CSS generation must repeat the same
Resolver/inputs through the CLI. Direct CSS and reopened Asset Plan audits report tokens as
`unavailable` when no graph is supplied because they cannot reconstruct one honestly. Explicit
audit `--token-graph` changes that state to measured and publishes the exact graph as the optional
fourth control artifact; it does not infer compiled token use.

For this one-source slice, `inputs.sourceHash` is SHA-256 over compact UTF-8 JSON containing one
ordered `{file, sha256}` leaf; the leaf hash is over the exact CSS bytes. `inputs.configHash` is
SHA-256 over compact UTF-8 JSON containing `targetProfile`, the complete canonical compatibility
policy hash, optional canonical budget-policy hash, sorted explicit budget subjects, and the exact
optional accessibility-policy/token-graph file identities. The input ledger separately records
budget policy as `policy`, accessibility policy as `accessibility-policy`, and audit graph as
`token-graph`. Asset Plan mode uses the same
compact source array with lexically ordered leaves for the exact plan and all CSS/style-manifest
pairs. No absolute path, clock, network, or stdout renderer participates.

## Rust usage

Producers construct `ControlManifest`, call `validate`, then serialize with
`to_canonical_json`. Consumers use `parse_control_manifest`. After the exact manifest bytes exist,
producers construct `BuildReceipt` with `ArtifactReference::from_bytes`, validate it against those
bytes, and call `to_canonical_json`. Consumers use `parse_build_receipt(receipt, manifest)`.

```rust
use pliego_css_control::{
    parse_build_receipt, parse_control_manifest, ArtifactReference, BuildReceipt, ControlManifest,
    CONTROL_MANIFEST_FILE,
};

fn verify(
    manifest_bytes: &[u8],
    receipt_bytes: &[u8],
) -> Result<(ControlManifest, BuildReceipt), Box<dyn std::error::Error>> {
    let manifest = parse_control_manifest(manifest_bytes)?;
    let receipt = parse_build_receipt(receipt_bytes, manifest_bytes)?;
    assert_eq!(
        receipt.manifest,
        ArtifactReference::from_bytes(CONTROL_MANIFEST_FILE, manifest_bytes)?
    );
    Ok((manifest, receipt))
}
```

Canonical serialization is UTF-8 pretty JSON ending in exactly one LF. Maps use lexical key order;
struct fields use schema order. The frozen fixture hashes are
`sha256:ccb4d20e8c443e51c9264f9927427d6177efa487b0f35b3e82a59a47c8382ee1` for the manifest and
`sha256:08c9cc69b3eefd4c04f7f4362b3c4f385f3ac57ed59d2b2fc660fdc65def7eeb` for its receipt.

## Relationship to existing artifacts

This contract does not replace style manifests 3-5, Asset Plan 1/2, Project Index 1/2, reachability
schema 1, Usage Analysis, or finding schema 1.0.0. The control manifest references and
integrity-binds those
specialized artifacts. See the [R0 design](../product/r0-manifest-receipt-design.md),
[finding schema](./finding-schema-1.md), [Asset Plan](./asset-plan-schema.md), and
[Project Index](./project-index-schema.md).
