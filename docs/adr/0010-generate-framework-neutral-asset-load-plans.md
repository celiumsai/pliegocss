# ADR-0010: Generate framework-neutral asset load plans

## Status

Accepted.

## Context

The declarative bundle plan gives PliegoCSS an application-owned set of source partitions. Manifest
schemas 4 and 5 add exact application topology and identify which components own the semantic
declarations emitted in each partition. Neither artifact by itself tells a framework which complete
CSS files are candidates for one route or one resumable island.

Leaving that join to every framework integration would duplicate security-sensitive graph logic.
An adapter could accidentally trust an unverified manifest, ignore a shared or co-owned style, omit
the global theme bundle, conflate route membership with island containment, or lose the external
record of whether unreachable StyleIds were pruned.

PliegoCSS still must not import PliegoRS types or infer framework structure from Rust paths, Cargo
modules, function names, or conventions. Application topology remains explicit, framework-owned,
and adapter-attested.

## Decision

`pliego-cssc bundle` accepts a boolean `--asset-plan` option. It requires manifest schema 4 or 5 and
the corresponding reachability-schema-1 sidecar. The option has one fixed destination inside the
bundle output directory:

```text
OUTPUT_DIR/pliego.assets.json
```

The generated document uses independent asset-load-plan schema 1. It records the common manifest,
graph, identity, ThemeId, target, and printer contracts; the external rule-selection decision; an
integrity record for every emitted CSS/manifest pair; and separate route and island records that
refer to portable bundle IDs.

The rule-selection values are:

- `all-compiled` when `--prune-unreachable` is absent; and
- `reachable-style-ids` when whole StyleId rule sets were selected by the union of route and island
  roots.

This field is required because manifest schemas 4 and 5 bind the selected result but intentionally
do not encode the command policy that selected it.

## Selection algorithm

The generator consumes the exact in-memory CSS and manifest bytes that are about to be published. It
does not rescan source files or rederive framework ownership.

Before topology is used, every manifest must:

- be schema 4 with graph schema 1 or schema 5 with graph schema 2;
- match its exact CSS byte length and SHA-256 digest;
- have complete semantic ownership and valid graph endpoints;
- satisfy the complete physical contract when graph schema 2 is present; and
- agree with every other bundle on manifest/graph versions, identity versions, ThemeId, targets,
  format, and complete application topology.

For one bundle, active components are the component nodes that own at least one emitted semantic
declaration through `componentUsesDeclaration`. A route selects that bundle when its declared
component set intersects the active set. An island applies the same rule independently.

Zero or one bundle may emit theme custom properties. The theme-emitting bundle is application-global:
it is ordered first and selected for every declared route and island regardless of component
intersection. Multiple theme-emitting bundles are rejected as ambiguous. A build with no
theme-emitting bundle is valid.

Routes and islands remain separate in the output. The reachability sidecar does not assert which
island occurs on which route, so the compiler may not invent that relationship. A framework combines
one route's candidates with the candidates of the islands it actually renders and deduplicates by
bundle ID.

## Integrity and deterministic bytes

Each bundle record contains fixed relative `<id>.css` and `<id>.manifest.json` filenames plus exact
byte counts and lowercase SHA-256 digests for both files. The asset plan has no recursive self-hash;
its authenticity belongs to a surrounding signed ledger or trusted deployment channel.

Input bundle order, bundle-plan table order, and equivalent reachability array order do not affect
the result. The optional theme bundle is first, remaining bundles are sorted by ID, routes and
islands are sorted by namespaced ID, and each root's references follow canonical bundle order. The
document is pretty JSON with one final LF and contains no CWD or absolute path.

Asset-plan generation occurs after every bundle has compiled and before any output is staged. The
asset plan participates in the same alias checks, symlink/reparse-point rejection, advisory locks,
byte-identical skip, grouped publication, and handled-failure rollback as every CSS/manifest pair.
`bundle --check --asset-plan` regenerates and compares the complete group without mutation.

This publication protocol is rollback-capable, not crash-atomic. Abrupt termination can still leave
missing destinations or temporary/backup files requiring recovery.

## Trust boundary

`originCoverage: "compiler-verified-complete"` means the compiler verified ownership for the
declarations actually represented by each manifest. `applicationCoverage:
"adapter-attested-complete"` remains a statement made by the framework adapter. Agreement among
bundle manifests proves a consistent topology snapshot, not that the adapter discovered every real
route, island, component, or relationship.

The asset plan is build metadata, not authorization metadata. Digests detect byte drift relative to
the plan but do not authenticate a plan and files that an attacker can replace together. Consumers
must parse a dedicated numbered schema, enforce budgets, reject dangling references, and verify all
CSS and manifest hashes before using root membership.

## Consequences

- Frameworks receive one deterministic, portable join between verified graph ownership and complete
  CSS files without depending on PliegoCSS internals.
- The rule-selection policy becomes explicit outside manifest schemas 4 and 5.
- Shared and co-owned styles naturally select a bundle for every applicable route or island while
  each root lists the bundle only once.
- Fully pruned bundles remain in the integrity ledger even when no root selects them.
- Theme CSS remains global and is never mistaken for component-owned output.
- A schema-5 plan can verify the theme-emission flag through `producer:theme`; schema 4 must retain
  the already validated bundle-plan flag because graph schema 1 has no physical producer.
- Every bundle pays manifest parsing and graph validation when asset-plan generation is requested.
- All bundle manifests must carry the same full application topology. A partial per-bundle topology
  is rejected instead of merged heuristically.
- Future wire changes require a new asset-plan schema rather than silent optional-field growth.

## Rejected alternatives

### Derive membership from source paths or bundle names

Rejected because paths and names do not prove component ownership, shared sites, or route/island
relationships.

### Join the reachability sidecar directly to the bundle plan

Rejected because the sidecar describes possible source ownership, not which StyleIds and semantic
declarations survived compilation and optional pruning in each final bundle.

### Let every framework parse and join manifest graphs

Rejected as duplicated fail-closed validation and a likely source of inconsistent handling for
integrity, theme output, shared ownership, and schema evolution.

### Merge island assets into route records

Rejected because schema 1 has no route-to-island occurrence relation. Such a merge would fabricate
framework state.

### Emit deployment URLs, links, or preload policy

Rejected because public base URLs, page composition, loading priority, cache policy, and HTML
generation belong to the framework/deployment layer. The asset plan exposes portable filenames and
selection candidates only.

### Derive or rewrite bundle partitions

Rejected because `--asset-plan` reports the explicit bundle plan; it does not implement shared
extraction, route splitting, critical CSS, or output-strategy selection.

## Non-goals

This decision does not add automatic framework collection, route-to-island discovery, preload or
`<link>` generation, URL construction, critical CSS, individual declaration/token pruning, theme
variable pruning, runtime loading, deployment, or authorization.

See the complete [asset load plan schema 1](../reference/asset-plan-schema.md),
[bundle plan contract](../reference/bundle-plan.md),
[reachability schema 1](../reference/reachability-schema.md),
[manifest schema 4](../reference/manifest-schema-4.md), and
[manifest schema 5](../reference/manifest-schema-5.md).
