# ADR-0011: Share one portable Project Index

## Status

Accepted and implemented for explicit schema-5 bundle builds.

## Context

PliegoCSS already emits exact source origins, semantic declarations, token dependencies,
adapter-attested application ownership, physical CSS ranges, and an integrity-bound asset plan.
Those facts were split across source snapshots, per-bundle manifests, reachability, and the asset
plan. A CLI command, framework adapter, and LSP could each rejoin them, but separate joins would
create inconsistent path rules, stale-file behavior, graph validation, and security boundaries.

Repository heuristics are especially unsafe here. Rust paths do not prove components, Cargo modules
do not prove runtime routes, and a physical declaration ordinal is meaningful only inside its own
bundle manifest.

## Decision

The shared handoff is independent Project Index schema 1 at fixed bundle output
`pliego.index.json`. It is opt-in through `bundle --project-index`, requires schema 5 plus the asset
plan and explicit reachability, and is generated in `pliego-css-build` so all host integrations can
reuse the same implementation.

The index records exact portable documents, source sites, semantic declarations, token IDs,
component owners, bundle-qualified physical declarations, and integrity metadata for the asset
plan plus every CSS/manifest pair. Source and application semantics remain compiler-verified and
adapter-attested respectively.

Project Index participates in the existing grouped publication and byte-exact `--check` contract.
It never stores an absolute path or consults the current working directory when generating IDs.

## Consequences

- CLI, adapters, and future LSP features have one numbered project model.
- A consumer can navigate source to final CSS without rescanning Rust or inferring framework state.
- Every physical reference is bundle-qualified, avoiding collisions between per-bundle ordinals.
- Source edits invalidate document hashes and affected site mappings explicitly.
- Schema-4 builds cannot request the index because they lack fail-closed physical coverage.
- The generated JSON duplicates some integrity and identity fields deliberately so it can reject a
  mixed or stale artifact set before following references.
- Adding a new project-wide semantic dimension requires a schema decision, not an unversioned
  editor-only cache.

## Rejected alternatives

### Let the LSP scan the repository

Rejected because editor working directories, Cargo membership, ignored files, and framework
ownership are not the compiler contract.

### Extend manifest schema 5 with project-wide files

Rejected because manifests are independently integrity-bound to one CSS bundle. A project join has
different lifecycle, ordering, and bundle-qualified reference requirements.

### Put source sites in the asset plan

Rejected because the asset plan answers route/island-to-bundle loading. Editor/source provenance is
a separate consumer contract and would bloat simple deployment loaders.

### Emit absolute paths for convenience

Rejected because they break cross-machine identity, leak workstation layout, and make remote or
container tooling non-portable.

## Non-goals

This decision does not implement an LSP server, filesystem watching for editors, automatic
framework collection, Cargo reachability, authentication, deployment URLs, or runtime CSS loading.

See [Project Index schema 1](../reference/project-index-schema.md),
[manifest schema 5](../reference/manifest-schema-5.md), and
[asset-plan schema 1](../reference/asset-plan-schema.md).
