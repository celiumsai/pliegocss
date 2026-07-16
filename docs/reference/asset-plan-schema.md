# Asset load plan schemas 1 and 2

Status: implemented as a versioned explicit `bundle` output contract

An asset load plan is framework-neutral build metadata that maps declared application routes and
resumable islands to the CSS bundles that contain their emitted styles. It is generated from the
same compiled CSS, manifests, bundle plan, and reachability snapshot as the bundle output group.
The producing bundle plan may use frozen schema 1 or additive schema 2; its version is independent
of the Asset Plan version. The Asset Plan does not copy DTCG Resolver selections. A controlled
build binds those exact plan and Resolver inputs separately.

Enable it with the boolean `--asset-plan` option:

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --manifest-version 5 \
  --reachability pliego.reachability.json \
  --asset-plan
```

The option takes no value. It always writes:

```text
dist/assets/pliego.assets.json
```

`--asset-plan` requires `--manifest-version 4` or `5` and the matching `--reachability` input.
Schema-3 manifests do not contain the application ownership graph needed to derive load
membership. `--prune-unreachable` remains independently optional.

For manifest schema 5, `--project-index` may be added to emit the companion
`pliego.index.json`. It requires `--asset-plan` and integrity-binds the exact asset-plan bytes while
adding portable source-document and source-site navigation. The asset plan remains the smaller
route/island loading contract; consumers that need source, declaration, token, component, or final
CSS lineage should use [Project Index schemas 1 and 2](./project-index-schema.md) instead of
rebuilding that model independently.

The [ownership sidecar schema 1](./ownership-schema-1.md) is a separate audit input. It binds the
exact bytes and SHA-256 of a supported Asset Plan schema 1 or 2, assigns every complete bundle to
exactly one package, and explicitly lists the islands that may render on every route. It does not add
fields to `pliego.assets.json` or participate in bundle publication. Its closed public parser,
canonical producer, and explicit `audit --ownership` integration are implemented and locally
replayed.

## Complete shape

Schemas 1 and 2 have the same closed field set. All fields shown below are required. Digest strings
and byte counts in this schema-1 example are illustrative; generated values bind the exact adjacent
files.

```json
{
  "schemaVersion": 1,
  "manifestSchemaVersion": 5,
  "graphSchemaVersion": 2,
  "ruleSelection": "reachable-style-ids",
  "originCoverage": "compiler-verified-complete",
  "applicationCoverage": "adapter-attested-complete",
  "styleIdFormatVersion": 2,
  "classNameFormatVersion": 1,
  "themeIdFormatVersion": 1,
  "themeId": "c46b8b7ec8c3aa6daadf15cc9196ba3e",
  "targets": "modern",
  "format": "minified",
  "bundles": [
    {
      "id": "global",
      "cssFile": "global.css",
      "manifestFile": "global.manifest.json",
      "emitsTheme": true,
      "cssBytes": 1024,
      "cssSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "manifestBytes": 8192,
      "manifestSha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    },
    {
      "id": "home",
      "cssFile": "home.css",
      "manifestFile": "home.manifest.json",
      "emitsTheme": false,
      "cssBytes": 256,
      "cssSha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
      "manifestBytes": 4096,
      "manifestSha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
    }
  ],
  "routes": [
    {
      "id": "route:home",
      "path": "/",
      "bundles": [
        "global",
        "home"
      ]
    }
  ],
  "islands": [
    {
      "id": "island:counter",
      "name": "Counter",
      "bundles": [
        "global"
      ]
    }
  ]
}
```

Schema 2 changes only the version/selection pair. These are excerpts, not complete documents:

```json
{ "schemaVersion": 1, "ruleSelection": "all-compiled" }
{ "schemaVersion": 1, "ruleSelection": "reachable-style-ids" }
{ "schemaVersion": 2, "ruleSelection": "reachable-or-retained-style-ids" }
```

The inverse combinations are invalid: schema 1 cannot claim policy retention, and schema 2 cannot
claim either schema-1 selection. JSON objects are closed in both versions. Consumers should reject
missing or unknown fields, unknown enum values, invalid identities, duplicate records, dangling
bundle references, and values outside the limits below.

## Top-level fields

| Field | Contract |
|---|---|
| `schemaVersion` | Asset load plan schema. Must be `1` for `all-compiled` or `reachable-style-ids`; must be `2` only for `reachable-or-retained-style-ids`. It is independent of manifest and graph schema versions. |
| `manifestSchemaVersion` | Common manifest version for every bundle. Must be `4` or `5`. |
| `graphSchemaVersion` | Common graph version. Must be `1` for manifest schema 4 and `2` for manifest schema 5. |
| `ruleSelection` | Closed output-selection discriminator; its three supported values and required schema pair are described below. |
| `originCoverage` | Must be `compiler-verified-complete`. Every emitted semantic declaration has exact component ownership in its bundle manifest. |
| `applicationCoverage` | Must be `adapter-attested-complete`. The application adapter, not PliegoCSS, attests that the supplied topology is complete. |
| `styleIdFormatVersion` | Common non-zero StyleId format version used by all bundle manifests. |
| `classNameFormatVersion` | Common non-zero class-name format version used by all bundle manifests. |
| `themeIdFormatVersion` / `themeId` | Common theme identity. `themeId` is exactly 32 lowercase hexadecimal characters. |
| `targets` | Common CSS target contract: `modern` or `none`. |
| `format` | Common printer contract: `minified` or `pretty`. |
| `bundles` | Every CSS/manifest pair produced by the declarative bundle plan, including bundles selected by no route or island. |
| `routes` | Canonical route records copied from the common application topology, with their selected bundle IDs. |
| `islands` | Canonical island records copied from the common application topology, with their selected bundle IDs. |

Every bundle manifest must agree on all top-level identity fields and on the complete component,
route, island, route-to-component, and island-to-component topology. A mixed manifest schema,
different ThemeId, target, format, identity version, route path, island name, or topology causes the
whole bundle command to fail before publication.

## Rule selection

`ruleSelection` records the build decision that manifest schemas 4 and 5 intentionally do not
encode:

| Schema | Value | Meaning |
|---|---|---|
| 1 | `all-compiled` | Reachability pruning was absent. Every compiled StyleId remains in its explicitly declared source bundle. |
| 1 | `reachable-style-ids` | Reachability pruning retained only complete StyleId rule sets selected by the union of route and island component roots. |
| 2 | `reachable-or-retained-style-ids` | Reachability pruning retained that same reachable union plus exact bundle-qualified StyleIds admitted by a validated [usage retention schema 1](./usage-retention-schema-1.md) policy. |

The value describes how the adjacent bundle artifacts were selected. It does not list removed
StyleIds, prune individual declarations, or claim that theme variables were pruned. A retained
shared StyleId still carries every normalized origin and all corresponding component ownership
edges. Policy retention does not change structural reachability: a retained dead StyleId remains
unreachable and dead in Usage Analysis, while its complete rule set remains published.

Modes without retention always use schema 1. Introducing schema 2 therefore does not change a
previously generated `all-compiled` or `reachable-style-ids` document: given identical inputs,
those schema-1 bytes remain byte-identical.

## Bundle records and integrity

Each `bundles[]` record has this exact contract:

| Field | Contract |
|---|---|
| `id` | Bundle name from the schema-1 or schema-2 bundle plan. It is 1-64 lowercase kebab-case ASCII bytes, starts with a letter, has no repeated or trailing hyphen, and is not a Windows device name. |
| `cssFile` | Exactly `<id>.css`, relative to the directory containing `pliego.assets.json`. |
| `manifestFile` | Exactly `<id>.manifest.json`, relative to the directory containing `pliego.assets.json`. |
| `emitsTheme` | Whether the bundle plan requested `emit-theme = true` for this bundle. |
| `cssBytes` | Exact CSS file length, including the required final newline. |
| `cssSha256` | Lowercase SHA-256 of the exact CSS file bytes. It must also equal the digest in the adjacent manifest. |
| `manifestBytes` | Exact manifest file length, including its final newline. |
| `manifestSha256` | Lowercase SHA-256 of the exact manifest file bytes. |

The generator parses and validates every complete manifest before using its topology. It verifies
the manifest schema/graph pair, identity versions, ThemeId, target and format, CSS byte count and
digest, semantic graph endpoints and coverage, and, for schema 5, the physical graph contract and
producer coverage. The plan is generated only after all bundle CSS and manifest bytes exist in
memory.

The asset plan has no self-digest because including one would be recursive. Authenticity of the
plan itself belongs to the surrounding build ledger, signature, or trusted deployment channel.

## Route and island selection

Routes and islands are separate records because a reachability sidecar does not assert which island
instance appears on which route. An explicitly supplied ownership schema 1 companion may provide
that relationship to audit without changing these records.

For each bundle, the generator derives its **active components** from
`componentUsesDeclaration` edges in that bundle's validated manifest. It then selects bundles as
follows:

```text
route bundle = emits theme
            OR bundle active components intersect route components

island bundle = emits theme
             OR bundle active components intersect island components
```

Consequently:

- one bundle may appear in several routes and islands;
- a bundle ID appears at most once in one root's `bundles` array;
- a bundle that contains only unreachable or otherwise unselected styles remains in the top-level
  `bundles` integrity ledger but may be referenced by no root;
- under schema 2, a bundle whose only selected styles were retained for an external consumer also
  remains in the integrity ledger but is not invented into a route or island selection;
- route and island records with no component match have only the theme bundle, when one exists;
  otherwise their `bundles` arrays are empty; and
- consumers load the route's bundle list and separately add the lists for islands actually present
  on that page, deduplicating by bundle ID.

This mapping describes the explicit source partitions in the bundle plan. It does not move styles
between bundles, extract shared CSS, or derive a new partition.

## Global theme bundle

Zero or one bundle may have `emitsTheme: true`. More than one is rejected as ambiguous. When it
exists, the theme bundle:

- is the first top-level bundle;
- is the first bundle in every route and island selection; and
- remains selected even when it has no active semantic component for that root.

This makes the current custom-property block application-global. It does not prove that every
deployment needs to transfer the theme bundle separately, nor does it prune individual variables.
A plan with no theme-emitting bundle is valid; the application is then responsible for supplying
any required custom properties by another trusted mechanism.

Manifest schema 5 independently checks `emitsTheme` against the presence of the synthetic
`producer:theme`. Graph schema 1 does not represent physical theme declarations, so for manifest
schema 4 the value comes from the already validated declarative bundle plan.

## Canonical bytes

Equivalent input order does not affect asset plan bytes. The generator canonicalizes as follows:

1. validate every bundle ID and reject duplicates;
2. place the optional theme-emitting bundle first;
3. sort all remaining bundles by `id`;
4. sort routes and islands by their namespaced graph IDs;
5. order each root's bundle references using the canonical bundle order;
6. serialize required fields in the schema order as pretty JSON; and
7. append exactly one LF newline.

The document contains only fixed relative bundle filenames, never the process CWD, output
directory, source paths, or input-plan path. Equivalent plan/sidecar array ordering and execution
from a different working directory therefore produce identical asset plan bytes when the compiled
artifacts are identical.

Object member order is part of PliegoCSS's deterministic emitted bytes, but JSON consumers must
still parse objects by field name. Array order is normative for canonical output. For identical
non-retention inputs, schema-1 serialization and bytes are unchanged by the existence of schema 2.

## Publication and `--check`

`pliego.assets.json` is part of the same output group as every `<id>.css` and
`<id>.manifest.json` pair. Normal `bundle` execution:

- validates all input/output aliases and symlink or reparse-point boundaries;
- compiles every bundle and generates the asset plan before staging any output;
- acquires the existing advisory locks for the complete destination set;
- skips byte-identical destinations; and
- uses the same handled-failure rollback path for the complete group.

An asset-plan validation or generation failure publishes nothing. A handled publication failure
attempts to restore previous CSS, manifests, and asset plan together. This is rollback-capable
grouped publication, not a crash-atomic filesystem transaction: process termination can still leave
missing destinations or sibling `*.tmp`/`*.bak` recovery files.

With both `--asset-plan` and `--check`, the compiler regenerates all expected bytes in memory and
compares the complete output group. A missing or different asset plan is drift. Check mode does not
create, repair, delete, or otherwise mutate outputs.

## Consumer verification

A consumer should process schemas 1 and 2 in this order:

1. apply a defensive document-size limit and parse a dedicated closed schema-specific type;
2. require one exact supported pair: schema 1 with `all-compiled` or `reachable-style-ids`, or
   schema 2 with `reachable-or-retained-style-ids`;
3. require the exact manifest, graph, identity, target, format, and coverage values it supports;
4. validate unique portable bundle IDs and exact derived filenames;
5. validate unique route IDs/paths and island IDs/names;
6. reject dangling or duplicate bundle references;
7. read each referenced CSS and manifest relative to the trusted asset-plan directory;
8. verify `cssBytes`, `cssSha256`, `manifestBytes`, and `manifestSha256` before using either file;
9. parse each manifest under its numbered schema and independently verify its CSS integrity and
   common build identity; and
10. only then use route and island membership to choose load candidates.

`pliego-cssc audit --asset-plan dist/assets/pliego.assets.json --targets PROFILE` implements the
same read-only integrity path by regenerating the canonical plan from every adjacent CSS/manifest
pair. File and layer budgets can operate on that verified ledger alone. Package or route policies
require the explicit `audit --asset-plan PLAN --ownership FILE` companion: ownership must bind the
exact plan bytes, cover every bundle/package, and supply total route composition before those typed
subjects are measured. The route measurement is the stable deduplicated union of its base bundles
and all bundles for its declared islands.

The asset plan is build metadata, not authorization. Hashes detect accidental or malicious byte
drift relative to the plan; they do not authenticate a plan and assets that an attacker can replace
together.

## Limits and rejected states

The generator fails closed for malformed or incompatible manifests, dangling or unknown graph
edges, incomplete semantic or physical coverage, invalid hashes, inconsistent topology, duplicate
or unsafe bundle IDs, and ambiguous theme ownership.

Current defensive limits include:

- at most 65,535 bundles;
- at most 65,535 graph nodes and 65,535 graph edges per input manifest;
- at most 65,535 total asset-plan items, counting bundle records, route records, island records,
  and every route/island bundle reference;
- at most 16 MiB for each CSS input, each manifest input, and the final asset plan including its
  trailing newline;
- route/island/component identifier payloads of 1-256 UTF-8 bytes with no control characters; and
- route paths and island names of 1-4,096 UTF-8 bytes with no control characters.

Unknown manifest schema, graph schema, edge kinds, rule-selection values, target contracts, or
printer formats are rejected rather than approximated.

## Edge cases

- With no declared routes or islands, both root arrays are empty; the bundle integrity ledger is
  still complete.
- A fully pruned non-theme bundle remains listed with the exact newline-only CSS and pruned manifest
  but is selected by no root.
- A fully pruned theme bundle still contains the global theme block and is selected by every
  declared route and island.
- A schema-2 bundle selected only by retention contains the complete retained StyleId rules and
  remains in `bundles`, but receives no route or island membership merely because of that policy.
- A shared or co-owned emitted style can make one bundle active for several components and therefore
  several roots. Each root still contains the bundle ID once.
- A component emitted in several explicit bundles selects all of those bundles for each route or
  island that references the component.
- A route and an island that reference the same component receive independent records. The plan does
  not infer route-to-island containment.
- Changing only `--prune-unreachable` changes `ruleSelection` and can change CSS, manifests, hashes,
  active components, and root memberships.
- Applying retention to an otherwise pruned snapshot changes the schema/selection pair to
  `2`/`reachable-or-retained-style-ids`; removing that policy returns the same no-retention inputs to
  their byte-identical schema-1 representation.
- Schema 4 and schema 5 can produce identical CSS and root membership for the same pruning setting,
  but their manifest and graph version fields, manifest bytes, and manifest hashes differ.

## Trust boundary and non-goals

The application adapter remains the authority for components, routes, islands, and their
relationships. `applicationCoverage: "adapter-attested-complete"` is copied only after every bundle
manifest agrees on that topology; PliegoCSS cannot discover an omitted framework node or prove the
attestation true.

Schema 1 deliberately does not:

- collect PliegoRS or another framework's routes, islands, components, or source sites;
- infer ownership from Cargo modules, paths, function names, or runtime behavior;
- derive bundle partitions, split shared CSS, or choose an atomic/grouped/hybrid output strategy;
- generate URLs, public base paths, HTML `<link>` elements, preload directives, fetch priority, or
  cache headers;
- infer which islands occur on a route;
- emit critical CSS or prune individual declarations, tokens, or theme variables;
- publish files to a server or mutate a framework build ledger; or
- authenticate application topology or authorize access to an asset.

Framework integrations translate the portable filenames and separate route/island selections into
their own deployment URLs and loading policy after verification.

See [bundle plan schemas 1 and 2](./bundle-plan.md), [reachability schema 1](./reachability-schema.md),
[manifest schema 4](./manifest-schema-4.md), [manifest schema 5](./manifest-schema-5.md), and
[Project Index schemas 1 and 2](./project-index-schema.md), plus the separate
[ownership sidecar schema 1](./ownership-schema-1.md),
[usage retention schema 1](./usage-retention-schema-1.md), and
[ADR-0019](../adr/0019-retain-dead-styles-by-explicit-policy.md).
