# Provenance manifest schema 4

Status: implemented as an explicit opt-in; schema 3 remains the default

Manifest schema 4 adds a deterministic semantic ownership graph to the existing CSS integrity and
style-origin contract. It is intended for build adapters, editors, debuggers, route asset planners,
and other consumers that need to answer why a style is present.

```console
pliego-cssc compile \
  --source src \
  --seed \
  --output dist/pliego.css \
  --manifest dist/pliego.manifest.json \
  --manifest-version 4 \
  --reachability pliego.reachability.json
```

Both graph options are required together. Omitting them emits schema 3 with exactly the established
schema-3 shape and bytes. Schema 4 never guesses component, route, or island ownership from a Rust
path, function name, or Cargo module.

`--prune-unreachable` is a separate opt-in that requires these same graph options. Without it,
schema 4 retains the established CSS bytes. With it, the sidecar additionally selects complete
StyleId rule sets as described below.

## Top-level contract

Schema 4 retains every required schema-3 field and adds one required `graph` object:

| Field | Meaning |
|---|---|
| `schemaVersion` | `4`. |
| `styleIdFormatVersion` | Semantic style identity version used by every style and declaration. |
| `classNameFormatVersion` | `pc_*` class encoder version. |
| `themeIdFormatVersion` / `themeId` | Exact registry identity against which token references resolve. |
| `targets` / `format` | Lightning CSS target and printer contracts used for the adjacent CSS. |
| `cssSha256` / `cssBytes` | Integrity binding to the exact adjacent CSS bytes. |
| `styles` | Schema-3-shaped style nodes and normalized source origins for every emitted StyleId. Without pruning, this is the unchanged complete style set. |
| `graph` | Graph schema 1, described below. |

Consumers must verify all identity versions plus `cssSha256` and `cssBytes` before accepting the
graph. Graph metadata does not replace CSS integrity verification.

## Graph schema 1

The nested graph has this fixed shape:

```json
{
  "schemaVersion": 1,
  "declarationIdFormatVersion": 1,
  "originCoverage": "compiler-verified-complete",
  "applicationCoverage": "adapter-attested-complete",
  "declarations": [
    {
      "id": "decl:321e429fcbcbbfd069227acdeda4bb0a:00000000",
      "styleId": "321e429fcbcbbfd069227acdeda4bb0a",
      "ordinal": 0
    }
  ],
  "tokens": [
    {
      "id": "token:spacing:310ca263",
      "kind": "spacing",
      "tokenId": "310ca263",
      "name": "4"
    }
  ],
  "components": [{ "id": "component:app::card" }],
  "routes": [{ "id": "route:home", "path": "/" }],
  "islands": [{ "id": "island:visit-counter", "name": "visit-counter" }],
  "edges": [
    {
      "kind": "componentUsesDeclaration",
      "from": "component:app::card",
      "to": "decl:321e429fcbcbbfd069227acdeda4bb0a:00000000"
    }
  ]
}
```

The token ID above is the current seed-registry ID for spacing token `4`; consumers must use the
value in their generated manifest rather than copying an example across theme revisions.

### Semantic declaration boundary

A schema-4 declaration is one canonical `Assignment` record in the typed semantic IR. It is not a
promise that the final stylesheet contains exactly one physical `property:value` pair. One semantic
declaration can emit multiple CSS declarations, initializers, or target-dependent output, and
Lightning CSS can transform the result afterward.

This boundary is deliberate: the graph can prove the declaration-to-token relationship without
serializing internal IR or pretending that semantic and physical declarations are one-to-one.
Consumers that need the separately versioned final CSS trace can opt into
[manifest schema 5 / graph schema 2](./manifest-schema-5.md).

Declaration ID format 1 is:

```text
decl:<style-id-hex32>:<canonical-assignment-ordinal-hex8>
```

The ordinal is zero-based in the validated canonical assignment vector. Reordering equivalent
source utilities does not change it. A StyleId format change can change declaration IDs even when
declaration ID format 1 itself remains unchanged.

### Token nodes

Only tokens referenced directly by semantic declarations are present; the graph does not copy the
complete theme. `SemanticValue::Token` retains its typed namespace. A token-backed color, including
one with alpha such as `bg-accent/20`, uses the `color` namespace.

Token node IDs use:

```text
token:<kind-kebab-case>:<token-id-u32-hex8>
```

Supported kind labels are `spacing`, `color`, `font-family`, `font-size`, `font-weight`,
`line-height`, `letter-spacing`, `radius`, `shadow`, and `z-index`. Every ID must resolve in the
active registry; an unknown or stale reference aborts compilation.

### Application nodes and edges

Application IDs from the reachability document become globally namespaced graph IDs:

| Sidecar type | Graph ID |
|---|---|
| component `app::card` | `component:app::card` |
| route `home` | `route:home` |
| island `visit-counter` | `island:visit-counter` |

Graph schema 1 defines five edge kinds:

| Edge | Direction and meaning |
|---|---|
| `styleHasDeclaration` | `style:<StyleId>` → `decl:*`; the canonical style owns the semantic declaration. |
| `declarationUsesToken` | `decl:*` → `token:*`; the declaration directly references that theme token. |
| `componentUsesDeclaration` | `component:*` → `decl:*`; an exact component site produced the declaration's style. |
| `routeUsesComponent` | `route:*` → `component:*`; supplied explicitly by the application adapter. |
| `islandUsesComponent` | `island:*` → `component:*`; supplied explicitly by the application adapter. |

Forward traversal answers which CSS semantics a route or island can reach. Reverse traversal answers
which application owners caused one declaration or token dependency to be included.

## Coverage and canonical order

`originCoverage: "compiler-verified-complete"` means every source origin in the compiled artifact
had `file`, `byteStart`, and `byteEnd`, and every exact site was owned by at least one component.
Missing metadata or an unowned origin fails closed; the CLI never emits a partially owned schema-4
graph.

`applicationCoverage: "adapter-attested-complete"` preserves the reachability producer's required
claim that it supplied the complete component/route/island graph for the application snapshot.
PliegoCSS validates its shape and references but cannot independently prove that the framework
adapter omitted nothing. These fields are deliberately separate so compiler verification is never
confused with adapter trust.

Canonicalization rules are:

- declaration, token, component, route, and island arrays are ordered by node ID;
- edges are deduplicated and ordered by `(kind, from, to)`;
- repeated equivalent input styles retain all normalized schema-3 origins but one style node;
- equivalent source ordering and sidecar array ordering produce identical manifest bytes;
- without `--prune-unreachable`, changing the projected reachability graph changes manifest bytes,
  never CSS, StyleId, class, ThemeId, CSS digest, or CSS byte count; order-only sidecar changes
  canonicalize away.

Graph schema 1 rejects more than 65,535 total nodes (including the referenced top-level style
nodes) or 65,535 edges. These budgets are enforced during projection, including the multiplicative
component-owner × declaration edge path.

For a `pcx!` invocation, every statically reachable final StyleId shares the invocation's exact
source site. Schema 4 therefore attributes each final style's semantic declarations to the owning
component. It does not claim branch-literal provenance for individual assignments.

## Opt-in emitted-rule pruning

When `--prune-unreachable` is present, the root set is the union of components referenced by all
routes and islands. Every compiled origin must still match an exact component site, whether or not
that component is a root. A site shared by several components is reachable when any owner is in the
root set.

Selection happens after canonical identity aggregation. If any exact origin for a StyleId is
reachable, its complete semantic style is emitted and every normalized origin remains in
`styles[].origins`; otherwise all rules produced by that StyleId are omitted. The compiler does not
remove individual declarations from a retained style.

The graph's semantic declarations, direct token nodes, `styleHasDeclaration`,
`declarationUsesToken`, and `componentUsesDeclaration` edges describe only emitted styles. Supplied
application nodes and route/island-to-component edges retain their application-topology meaning. An
empty route/island root set therefore yields no style/declaration/token nodes. CSS is one newline
without theme output and `:root{}` plus one newline when theme output was requested.

With pruning enabled, `--theme` emits only variable-backed tokens directly referenced by retained
semantic styles. Schema 4 does not model those physical theme declarations. Without pruning, theme
emission remains the complete supported custom-property block. For `bundle`, the same classification is applied separately
to only the styles and origins already assigned to each explicit bundle; it does not derive or alter
the bundle partition.

Schema 4 intentionally adds no pruning-policy field. Its styles, graph, CSS byte count, and digest
self-describe the selected artifact, while the producer command or build ledger records whether
`--prune-unreachable` was used. A consumer cannot distinguish pruning from a narrower source set by
reading the manifest alone; adding such a field would require a new schema rather than silently
changing this frozen shape.

## Consumer migration

1. Continue accepting schema 3 as the default manifest.
2. Add a separate schema-4 parser and reject unknown graph schema or declaration ID versions.
3. Verify identity versions and CSS integrity before traversing nodes or edges.
4. Validate global node IDs, typed edge endpoints, uniqueness, and canonical order.
5. For pruning or asset-selection decisions, require both exact coverage fields and trust the
   adapter that produced the application attestation.
6. Enable schema 4 in the producer only after the application adapter emits
   [reachability schema 1](./reachability-schema.md).

Absence of `graph` in schema 3 means “not requested”; it must not be interpreted as an empty,
complete graph.

The checked [manifest-graph fixture](../../integration-tests/manifest-graph) contains a Rust source,
sidecar, bundle plan, exact CSS, and full schema-4 golden. Run it with `pnpm check:manifest`.
The independent [physical-trace fixture](../../integration-tests/physical-trace) freezes schema 5,
its exact graph-1 projection, and final CSS ranges; run it with `pnpm check:trace`.

## Security and scope

The manifest is build metadata, not an authorization graph. IDs, hashes, and an adapter attestation
do not prove trust, and the sidecar must come from the same reviewed build snapshot as the Rust
sources. Schema 4 can expose
application topology and the schema-3 source strings/ranges, so production deployments should ship
it only when a runtime or debugging consumer needs it.

The graph never infers Cargo or framework reachability and does not generate `<link>` or preload
tags. Without `--prune-unreachable` it is evidence only. With that explicit flag it can remove whole
unreachable StyleId rule sets and their now-unused emitted theme variables, but it still does not
partition assets, remove individual declarations, or discover arbitrary authored `var(...)` consumers.
