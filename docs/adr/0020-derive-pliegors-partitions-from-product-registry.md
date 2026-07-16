# ADR-0020: Derive PliegoRS partitions from an explicit product registry

## Status

Accepted.

## Context

PliegoCSS already accepts explicit bundle plans and framework-neutral application reachability.
The compiler must not import PliegoRS or guess product structure from Rust filenames, module names,
or route conventions. Requiring a PliegoRS application to maintain a product graph, reachability
sidecar, and bundle plan independently would nevertheless duplicate the same ownership facts and
allow them to drift.

PliegoRS also knows a relation that the framework-neutral reachability schema intentionally does not:
which resumable islands occur on each route. That relation is needed by the SSG consumer, but it must
not be invented by the PliegoCSS compiler.

## Decision

PliegoRS owns one typed `ProductRegistry` containing:

- components with explicit Rust source units;
- routes with component membership and URL paths;
- islands with component membership and runtime names; and
- route-to-island occurrence.

`product_component!` captures the normalized `file!()` path at the component declaration site. It
removes duplicated path strings but does not discover components automatically. Registry validation
rejects invalid identifiers and paths, duplicates, dangling component/island references, unsafe
source paths, and bounded-size violations.

The PliegoCSS PliegoRS adapter consumes one validated registry snapshot and produces two canonical
inputs:

1. `ApplicationTopology` and reachability schema 1 for exact visible `pc!`/`pcx!` ownership; and
2. bundle-plan schema 1, partitioned by each source unit's exact set of route and island roots.

Sources with the same root set share one physical partition. A source owned by every route and no
island becomes `shared`; a single-route source becomes `route-<id>`; a single-island source becomes
`island-<id>`; a source with no roots becomes `unreachable`; and more complex root sets receive a
stable content-derived ID. Co-owned source units are unioned conservatively rather than duplicated.
Exactly one partition selected by every route emits the theme; generation fails when no universal
partition exists.

The generated plan remains an ordinary explicit input to `pliego-cssc bundle`. The compiler itself
does not gain PliegoRS dependencies or silently repartition a supplied plan, preserving ADR-0010's
framework-neutral boundary.

## Trust boundary

The registry is adapter-attested. The collector proves complete visible-macro ownership only inside
the source roots derived from registered components. It does not prove that the application supplied
every Cargo module, component, route, or island. Exhaustive Cargo-graph discovery requires separate
evidence.

The Asset Plan remains responsible for integrity-bound CSS/manifest selection. The SSG consumer uses
the same registry's route path and route-to-island occurrence, verifies Asset Plan and Project Index
before loading files, and converts portable filenames into deployment URLs. Preload remains an
explicit framework delivery decision and not a compiler performance claim.

## Consequences

- One product graph drives collection, partition generation, and SSG composition.
- Reachability and plan bytes are reproducible and can be regenerated twice as a drift gate.
- Developers register semantic ownership, not filenames that encode routing conventions.
- Shared and co-owned source units are handled conservatively.
- A deliberately unreachable component remains visible to the all-compiled baseline and pruning
  evidence without entering the deployed site.
- Future registry or partition wire changes require reviewed versioning rather than heuristic drift.

## Rejected alternatives

### Infer topology from paths or module names

Rejected because naming conventions cannot prove ownership, shared sources, or island occurrence.

### Keep a hand-authored bundle plan beside the product registry

Rejected because it duplicates the root-membership relation and can silently drift.

### Move PliegoRS discovery into the PliegoCSS compiler

Rejected because it couples the framework-neutral compiler to a host framework and weakens the
explicit attestation boundary.

### Claim complete Cargo discovery from `file!()`

Rejected because `file!()` binds a registered declaration to a source unit; it does not enumerate
unregistered modules.

See [ADR-0008](./0008-require-explicit-application-reachability.md),
[ADR-0010](./0010-generate-framework-neutral-asset-load-plans.md), and the
[PliegoRS integration](../integrations/pliegors.md).
