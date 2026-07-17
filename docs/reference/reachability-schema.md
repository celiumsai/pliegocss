# Reachability sidecar schema 1

Status: implemented input contract for provenance manifest schemas 4 and 5 and opt-in rule pruning

PliegoCSS can prove semantic style and token relationships itself, but it cannot know which
framework component, route, or resumable island owns a Rust macro invocation. That information
belongs to the application or framework adapter. Reachability schema 1 is the neutral JSON seam
through which the adapter supplies it.

```json
{
  "schema": 1,
  "applicationCoverage": "complete",
  "components": [
    {
      "id": "app::components::card",
      "sites": [
        {
          "file": "src/components/card.rs",
          "byteStart": 120,
          "byteEnd": 151
        }
      ]
    }
  ],
  "routes": [
    {
      "id": "home",
      "path": "/",
      "components": ["app::components::card"]
    }
  ],
  "islands": [
    {
      "id": "visit-counter",
      "name": "visit-counter",
      "components": ["app::components::card"]
    }
  ]
}
```

Use it only with schema-4 or schema-5 manifest output:

```console
pliego-cssc compile \
  --source src \
  --manifest dist/pliego.manifest.json \
  --manifest-version 4 \
  --reachability pliego.reachability.json \
  --output dist/pliego.css
```

Replace `--manifest-version 4` with `5` to request the graph-schema-2 physical trace over the same
sidecar and CSS artifact.

Add `--prune-unreachable` to `compile`/`build`, `watch`, or `bundle` when this same explicit graph
should also decide which complete StyleId rule sets are emitted. The flag requires the sidecar and
manifest schema 4 or 5; reachability input without the flag remains provenance-only.

## Fields

The document is strict: every listed field is required and unknown fields are rejected.

| Field | Contract |
|---|---|
| `schema` | Must be `1`. |
| `applicationCoverage` | Must be `complete`; the adapter attests that its component, route, island, and relationship records cover the application snapshot. |
| `components` | Component records with unique `id` values and zero or more exact style sites. |
| `components[].id` | Adapter-owned stable ID, 1–256 UTF-8 bytes, with no control characters. |
| `components[].sites` | Exact compiler source-origin keys owned by that component. |
| `sites[].file` | Portable relative slash path matching the compiler's logical source label. |
| `sites[].byteStart` / `byteEnd` | Half-open UTF-8 byte range of the compiler origin: a complete `pc!`/`pcx!` invocation or the parsed style span in a line input; start must be smaller than end. |
| `routes` | Route nodes with unique IDs and unique paths. |
| `routes[].path` | Application-owned route path or pattern; PliegoCSS stores but does not interpret it. |
| `routes[].components` | Unique references to declared component IDs. |
| `islands` | Resumable-island nodes with unique IDs and unique names. |
| `islands[].name` | Application-owned human-readable island name. |
| `islands[].components` | Unique references to declared component IDs. |

Routes and islands may have empty component lists. Components may have empty site lists, which lets
an adapter preserve topology for components that do not currently own a PliegoCSS invocation.
References to undeclared components are rejected.

## Exact site ownership

A site is keyed by `(file, byteStart, byteEnd)`. It must equal the source origin emitted by the Rust
scanner; containment and path-prefix matching are not used. This prevents two components in one
file from being merged accidentally. A stale collector fails when an origin that still compiles
moves away from the site it declares. Removing that origin entirely leaves an unused extra site,
which is allowed. On Windows only,
scanner-origin `\\` separators are normalized to `/` before the exact
comparison. A literal backslash in a POSIX filename fails schema-4 or schema-5 projection instead of
colliding with a slash path.

Offsets count UTF-8 bytes, not Unicode scalar values, UTF-16 code units, lines, or columns. The
range is half-open: it includes `byteStart` and excludes `byteEnd`. A Rust scanner range covers the
complete macro invocation, not only the quoted style literal. A line-oriented `--input` origin
covers its parsed utility span.

One site can appear under several components. This represents an explicitly shared helper or other
application reachability and produces deduplicated `componentUsesDeclaration` edges for every
declared owner. For pruning, ownership is OR-combined: the site is reachable if any component that
owns it is reachable. Repeating the same site within one component is invalid.

`pcx!` can yield several statically reachable final styles from one invocation. All those styles
use the same site key and are therefore attributed to the same explicit owners.

## Portable paths

Site paths must:

- be relative and use `/`, even on Windows;
- contain no empty, `.` or `..` segment;
- contain no drive prefix or leading slash;
- contain none of the Windows-reserved `< > : " | ? *` characters;
- contain no Windows device-name segment such as `CON`, `NUL`, `COM1`, or `LPT9`, even with an
  extension;
- contain no segment ending in a dot or space;
- be at most 4,096 UTF-8 bytes; and
- exactly match the logical source label used for the compilation.

For normal `compile` and `watch`, invoke the CLI from the application root and pass relative source
paths such as `--source src`. For `bundle`, source origins are already normalized relative to the
bundle plan directory, so a declared `sources = ["src/card.rs"]` uses `src/card.rs` in the sidecar.

The `--reachability` file path itself is resolved by the process working directory. It is an input
and cannot alias a CSS, manifest, or bundle output destination.

## Validation and limits

The decoder validates and canonicalizes before compilation:

- input must be a regular file and is bounded while reading; maximum document size: 16 MiB;
- maximum aggregate components + routes + islands + sites: 65,535;
- maximum aggregate route/island component references: 65,535;
- maximum projected graph nodes, including referenced top-level style nodes: 65,535;
- maximum projected graph edges: 65,535;
- component, route, and island IDs: 1–256 bytes;
- file paths, route paths, and island names: 1–4,096 bytes;
- no control characters in IDs, paths, or names;
- unique component IDs, route IDs and paths, island IDs and names;
- unique sites within each component and unique component references within each owner;
- every route/island component reference must resolve.

Arrays, sites, and component references are sorted canonically after validation. Reordering an
equivalent input document does not change schema-4 or schema-5 manifest bytes. Node and edge budgets
are enforced while projecting so shared sites or large reference sets cannot amplify one valid
sidecar into an unbounded manifest.

## Two coverage boundaries

The sidecar's required `applicationCoverage: "complete"` is an adapter attestation. PliegoCSS can
validate graph closure and exact site ownership, but it cannot independently discover missing
framework routes, islands, components, or relationships. A consumer must trust the adapter and its
build pipeline before using this attestation for pruning or asset selection.

PliegoCSS independently verifies source-origin coverage. Compilation fails if any compiled style
origin:

- came from an explicit `--style` or `--compose` with no file/range;
- lacks `file`, `byteStart`, or `byteEnd`;
- uses a nonmatching path or byte range; or
- has no component owner in the sidecar.

The emitted graph reports `originCoverage: "compiler-verified-complete"` separately from
`applicationCoverage: "adapter-attested-complete"`. Extra valid application nodes and sites are
allowed. Per-bundle manifests project only semantic declarations present in that bundle while
retaining the supplied application nodes and route/island-to-component edges.

## Opt-in unreachable-rule pruning

`--prune-unreachable` uses the sidecar only after all compiled origins pass the exact coverage checks
above. An unreachable origin is valid and owned; it differs from a missing or unowned origin, which
still fails compilation.

The deterministic selection contract is:

1. form one root component set from the union of every `routes[].components` and
   `islands[].components` reference;
2. classify each exact site as reachable when at least one component that owns it is a root;
3. aggregate equal canonical identity streams exactly as normal; and
4. retain a StyleId and all of its normalized origins when any exact origin of that identity is
   reachable.

The compiler does not discard the unreachable origins of a retained shared StyleId. Consequently,
its schema-3-compatible `styles[].origins` and component-to-declaration evidence continue to describe
every compiled producer of the emitted class. Semantic declarations and direct token nodes are
projected only for retained styles; schema 5 then traces only their resulting physical rules and
declarations.

If routes and islands contribute no root components, every StyleId is removed. The CSS artifact is
then exactly one newline when theme emission is disabled. With `--theme`, or `emit-theme = true` in a
bundle plan, it is `:root{}` plus the final newline because no retained semantic style consumes a
variable-backed token. With non-empty roots, only directly referenced variable-backed tokens remain.
Without pruning, the complete supported theme block remains byte-compatible. Arbitrary authored
`var(...)` consumers are outside reachability schema 1 and cannot make a token reachable.

For `bundle`, the plan first assigns source sets explicitly. Each bundle classifies only its own
compiled styles and origins against the same union root set. This can remove rules inside a bundle;
it does not infer a route-to-bundle mapping, move a style between bundles, or derive a partition.

## Adapter responsibilities

The producer should generate the sidecar from the same immutable source snapshot used for CSS
compilation and set `applicationCoverage` to `complete` only after its own framework graph has been
fully collected. A PliegoRS adapter can derive component IDs and route/island relationships from
its own macro or build graph, but PliegoCSS deliberately imports no PliegoRS types and performs no
naming heuristics.

`pliego-css-source::ApplicationTopology` now provides the framework-neutral collector core. An
adapter supplies its typed component/route/island graph plus either whole-source-unit or exact-site
ownership. `collect(project_root)` inventories the declared Rust roots, uses the same `syn` scanner
as compilation, rejects every unowned visible `pc!`/`pcx!` invocation, rejects stale exact sites,
canonicalizes registration order, and returns these schema-1 bytes. Whole-source-unit ownership is
an explicit attestation, not path-based inference. The collector does not execute Cargo or infer a
module graph; a product adapter must form its topology from framework-owned APIs before making the
`complete` attestation.

Collector inputs are bounded independently before schema serialization: each source is at most 16
MiB, the complete source inventory at most 256 MiB, portable paths at most 256 segments, and the
expanded invocation-to-owner site set at most 65,535 records. Source roots and every descendant
reject symbolic links and, on Windows, junction/reparse points. These collector limits do not weaken
the separate 16 MiB sidecar parser limit or the graph-node/edge projection limits above.

Without `--prune-unreachable`, changing the projected sidecar graph changes the manifest, not CSS;
order-only changes canonicalize away. With pruning enabled, a graph change republishes CSS only when
it changes the retained StyleId set (or the graph-bound manifest independently). `watch` includes the
sidecar bytes in its exact snapshot and applies the same rule.

See [manifest schema 4](./manifest-schema-4.md) for semantic node IDs, edges, traversal, and consumer
rules, and [manifest schema 5](./manifest-schema-5.md) for the additional physical CSS trace.
