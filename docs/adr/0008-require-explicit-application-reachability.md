# ADR-0008: Require explicit application reachability

## Status

Accepted.

## Context

The compiler can derive styles, canonical semantic declarations, and direct design-token references
from `pc!` and `pcx!`. It cannot derive framework ownership from that syntax alone. A Rust file can
contain several components, a helper can be reused by several components, and route or resumable
island semantics belong to PliegoRS or another application framework.

Inferring ownership from paths, function names, or Cargo modules would make a plausible-looking but
incorrect provenance graph. Importing PliegoRS types into the compiler would also couple the CSS
engine to one framework and move application graph churn into the core compatibility unit.

## Decision

PliegoCSS accepts a strict, framework-neutral reachability sidecar produced by the application
adapter. The sidecar maps application component IDs to exact scanner sites and maps route/island IDs
to declared components. Exact sites use a portable logical file plus the half-open UTF-8 byte range
of the complete macro invocation.

Manifest schema 4 is explicit opt-in and requires the sidecar. It fails if any compiled source
origin is incomplete or unowned. The sidecar separately requires the adapter to attest that its
application graph is complete; the compiler validates that graph but cannot discover omissions in
framework-owned topology. The compiler projects the supplied direction:

```text
route / island -> component -> semantic declaration -> token
```

The separate `--prune-unreachable` opt-in uses the union of route- and island-referenced components
as roots. Every compiled origin must still resolve to an exact owned site. A canonical StyleId is
retained when any one of its exact origins is reachable, and all of its origins remain attached to
the emitted style. A site shared by several components uses OR semantics across its owners.

Schema 3 remains the default and byte-compatible output. Without pruning, the CSS pipeline, StyleId,
class name, ThemeId, targets, printer, CSS digest, and CSS bytes remain independent of the
reachability document. With pruning, schemas 4 and 5 describe and bind the selected emitted-style
subset.

## Consequences

- PliegoCSS stays usable by Rust frameworks other than PliegoRS.
- Component ownership is exact even when several components share a file or site.
- A stale sidecar fails closed after source ranges move.
- The application adapter must generate the sidecar from the same build snapshot and is responsible
  for the truth of its application-completeness attestation.
- With pruning disabled, changing only application reachability republishes the manifest but not
  CSS. With `--prune-unreachable`, membership changes that alter the retained StyleId set republish
  both artifacts.
- Schema-4 semantic declarations are canonical IR assignments. They do not pretend to be a
  one-to-one trace of physical declarations after CSS emission and Lightning CSS processing.
- Manifest schema 5 now provides the separately versioned, fail-closed physical trace while leaving
  the schema-4 semantic contract unchanged.
- The graph contains semantic declarations and direct token nodes only for emitted styles, while a
  retained shared style preserves every compiled origin.
- Theme custom properties remain application-global: `--theme` and bundle `emit-theme = true` are
  not filtered by reachability.
- Automatic application collection, token-variable pruning, bundle derivation, and link/preload
  generation remain separate work.

See [reachability schema 1](../reference/reachability-schema.md),
[manifest schema 4](../reference/manifest-schema-4.md), and
[manifest schema 5](../reference/manifest-schema-5.md).
