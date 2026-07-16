# Project Index schemas 1 and 2

Status: **implemented versioned opt-in bundle artifact; shared CLI/adapter/LSP contract candidate**

`pliego-cssc bundle --project-index` emits one fixed file:

```text
OUTPUT_DIR/pliego.index.json
```

The option requires `--asset-plan`, `--manifest-version 5`, and `--reachability`. Project Index is
not a repository scan and does not infer application structure. It joins exact compiler snapshots
with the already validated schema-5 physical graph and adapter-attested reachability document.

Schemas 1 and 2 have the same closed field set. The version is selected only by output selection:
schema 1 represents `all-compiled` or `reachable-style-ids`, while schema 2 represents only
`reachable-or-retained-style-ids`. Existing no-retention inputs continue producing byte-identical
schema-1 output.

## Purpose

Manifest schema 5 proves semantic declaration to final CSS declaration lineage. Reachability proves
component, route, and island ownership. The asset plan proves which complete bundle artifacts belong
to each application root. Project Index adds the missing stable source-site layer and integrity-binds
the complete artifact set so CLI, adapters, and editor tooling can share one result.

The navigation chain is:

```text
source document + UTF-8 byte range
→ source site
→ StyleId
→ semantic declaration IDs
→ token IDs
→ adapter-attested component IDs
→ bundle-qualified physical declaration IDs
→ exact manifest and CSS artifacts
```

No consumer needs to derive a component from a Rust module, filename, macro name, Cargo graph, or
current working directory.

## Top-level fields

| Field | Contract |
|---|---|
| `schemaVersion` | `1` for `all-compiled` or `reachable-style-ids`; `2` only for `reachable-or-retained-style-ids`. Unknown versions and mismatched pairs fail closed. |
| `sourceSiteIdFormatVersion` | `1`; content-addressed site-ID preimage defined below. |
| `manifestSchemaVersion` / `graphSchemaVersion` | Always `5` / `2`. |
| `declarationIdFormatVersion` | Always `1`. |
| `physicalRuleIdFormatVersion` / `physicalDeclarationIdFormatVersion` | Always `1` / `1`. |
| `originCoverage` | `compiler-verified-complete`. |
| `applicationCoverage` | `adapter-attested-complete`. |
| `physicalCoverage` | `compiler-verified-complete`. |
| `styleIdFormatVersion`, `classNameFormatVersion`, `themeIdFormatVersion` | Common identity versions copied from every validated manifest. |
| `themeId`, `targets`, `format` | Common build identity copied from every validated manifest. |
| `ruleSelection` | `all-compiled`, `reachable-style-ids`, or `reachable-or-retained-style-ids`; it must use the same schema/selection pair as the bound Asset Plan. |
| `assetPlanFile` | Fixed portable filename `pliego.assets.json`. |
| `assetPlanBytes`, `assetPlanSha256` | Integrity of the exact generated asset-plan bytes. |
| `documents` | Canonical source-document inventory. |
| `bundles` | Canonical CSS/manifest artifact inventory. |
| `sites` | Canonical source-to-semantic/application/physical mapping. |

## Version pairing

The only version-specific fields are the discriminator pair below; each line is an excerpt rather
than a complete closed document:

```json
{ "schemaVersion": 1, "ruleSelection": "all-compiled" }
{ "schemaVersion": 1, "ruleSelection": "reachable-style-ids" }
{ "schemaVersion": 2, "ruleSelection": "reachable-or-retained-style-ids" }
```

Schema 2 means that the output contains the reachable StyleId union plus exact bundle-qualified
StyleIds admitted by a validated [usage retention schema 1](./usage-retention-schema-1.md) policy.
Retention does not rewrite topology: retained styles remain structurally unreachable and dead in
Usage Analysis, and they do not add route or island membership. Schema 1 cannot carry that
selection, while schema 2 cannot carry either schema-1 selection.

## Documents

Each document record contains:

| Field | Contract |
|---|---|
| `id` | `document:` plus 64 lowercase SHA-256 hex digits. |
| `path` | Plan-relative normalized UTF-8 path using `/`. |
| `bytes`, `sha256` | Exact source snapshot length and SHA-256. |
| `siteIds` | Sorted IDs of all represented source sites in the document. May be empty. |

Paths are never absolute. Empty paths, backslashes, drive/URI colons, `.`/`..` or empty segments,
control characters, paths above 4 KiB, and case-insensitive collisions are rejected. Documents must
be UTF-8 and at most 16 MiB. These constraints let the same index be consumed on Linux, macOS, and
Windows without remapping repository-specific absolute paths.

Document ID format 1 hashes this tagged, length-delimited preimage:

```text
"pliego-project-document-v1\0"
u64_be(path_utf8_length)
path_utf8
```

## Source sites

Each site record contains:

| Field | Contract |
|---|---|
| `id` | `site:` plus 64 lowercase SHA-256 hex digits. |
| `documentId`, `path` | Exact owning document reference and redundant portable path. |
| `byteStart`, `byteEnd` | Half-open UTF-8 byte range; `start < end <= document.bytes`. |
| `macroKind`, `reason`, `source` | Compiler provenance and canonical semantic source. |
| `styleId`, `className` | Theme-scoped semantic identity and native CSS class. |
| `bundleIds` | Sorted bundles whose manifests contain this source/style site. |
| `declarationIds` | Sorted schema-5 semantic declaration IDs for the StyleId. |
| `tokenIds` | Sorted direct token nodes used by those declarations. May be empty. |
| `componentIds` | Sorted adapter-attested component owners. Never empty. |
| `physicalDeclarations` | Sorted `{bundleId, id}` pairs for every final CSS declaration contribution. Never empty. |

Physical declaration IDs are qualified by bundle because physical ordinals restart for every
CSS/manifest pair. Semantic declaration, token, and component IDs retain the exact graph IDs so a
consumer can join the site to each integrity-bound manifest without translation.

Source-site ID format 1 hashes the tagged, length-delimited path, macro kind, reason, canonical
source, and StyleId, followed by `byteStart` and `byteEnd` as unsigned 64-bit big-endian integers.
ClassName is derived from StyleId and therefore is recorded but not duplicated in the preimage.

## Bundles

Every bundle record repeats the portable `<id>.css` and `<id>.manifest.json` filenames, theme flag,
exact byte lengths, lowercase SHA-256 digests, and sorted site IDs present in that manifest. The
integrity fields must equal the corresponding asset-plan bundle entry. Fully pruned bundles remain
in the inventory and may have no sites. Under schema 2, sites for a policy-retained dead StyleId
remain indexed with their exact semantic and physical lineage even when no application root selects
that bundle.

## Validation and publication

Generation first runs the complete asset-plan validation over the exact in-memory CSS and manifest
bytes. It additionally requires schema 5/graph 2, complete physical coverage, declaration/physical
ID format 1, a common build identity, complete document coverage for every origin, valid ranges, and
at least one adapter-attested component plus physical mapping for every site.

Equivalent bundle-plan table order, reachability array order, and source discovery order produce the
same bytes. Documents sort by path, the theme bundle sorts first, remaining bundles sort by ID, sites
sort by canonical source identity, and every nested reference list is sorted. Output is pretty JSON
with one final LF. For identical inputs using either no-retention selection, schema-1 bytes remain
unchanged by the addition of schema 2.

The index is generated before publication and joins the same rollback-capable output group as CSS,
manifests, and `pliego.assets.json`. `bundle --check --project-index` regenerates and byte-compares
the entire group without mutation. The protocol remains rollback-capable rather than crash-atomic.

## Trust boundary

The index detects drift but does not authenticate files that an attacker can replace together. A
trusted deployment ledger or signature remains responsible for authenticity. The compiler verifies
only represented source and physical coverage; `applicationCoverage` remains an assertion by the
adapter that produced reachability schema 1.

The current artifact is the project-wide data contract, not yet the LSP transport. Future CLI,
adapter, and LSP features must consume a supported numbered schema instead of reconstructing project
ownership independently.

See [manifest schema 5](./manifest-schema-5.md),
[reachability schema 1](./reachability-schema.md),
[asset-plan schemas 1 and 2](./asset-plan-schema.md),
[usage retention schema 1](./usage-retention-schema-1.md),
[ADR-0011](../adr/0011-share-one-portable-project-index.md), and
[ADR-0019](../adr/0019-retain-dead-styles-by-explicit-policy.md).
