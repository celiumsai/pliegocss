# Provenance manifest schema 5

Status: implemented opt-in contract

Manifest schema 5 extends the explicit semantic ownership graph with a fail-closed trace of the
physical CSS after emitter output has passed through Lightning CSS. It answers both why a canonical
assignment exists and which exact final declarations and rules that assignment contributes to.

Schema 5 is selected with the existing numbered manifest option:

```console
pliego-cssc compile \
  --source src \
  --seed \
  --output dist/pliego.css \
  --manifest dist/pliego.manifest.json \
  --manifest-version 5 \
  --reachability pliego.reachability.json
```

`--manifest-version 5` requires manifest output and a valid
[reachability schema 1](./reachability-schema.md) document. There is no separate physical-trace
flag. `compile`/`build`, `watch`, and `bundle` all enforce the same version/reachability
relationship. `--prune-unreachable` may be added to those commands; it uses the same sidecar to
select complete StyleId rule sets before physical tracing.

## Top-level contract

Schema 5 retains the established manifest fields and requires graph schema 2:

| Field | Meaning |
|---|---|
| `schemaVersion` | `5`. |
| `styleIdFormatVersion` | Semantic style identity version used by styles and semantic declarations. |
| `classNameFormatVersion` | Format version of the emitted `pc_*` class. |
| `themeIdFormatVersion` / `themeId` | Exact active registry identity. |
| `targets` | `baseline-widely`, `modern`, or `none`, matching the adjacent CSS artifact. |
| `format` | `minified` or `pretty`, matching the adjacent CSS artifact. |
| `cssSha256` / `cssBytes` | SHA-256 and exact UTF-8 byte count of the adjacent CSS, including its final newline. |
| `styles` | The schema-4 style identities and complete normalized origins for every emitted StyleId. |
| `graph` | Required graph schema 2. |

Changing only the manifest version from 4 to 5 with the same pruning setting must not change the CSS,
StyleId, class name, ThemeId, target behavior, or printer output. Without pruning, this also preserves
the established schema-3 CSS bytes. Schema 5 describes the already selected final CSS artifact; it
does not create a different one. Like schema 4, it does not encode the pruning policy; the command or
build ledger must retain that external build metadata.

Consumers must reject schema 5 until they support graph schema 2 and both physical ID format
versions. They must then verify all top-level identity versions, `cssBytes`, and `cssSha256` before
using a physical range.

## Graph schema 2

Graph schema 2 has this canonical field order:

```json
{
  "schemaVersion": 2,
  "declarationIdFormatVersion": 1,
  "physicalRuleIdFormatVersion": 1,
  "physicalDeclarationIdFormatVersion": 1,
  "originCoverage": "compiler-verified-complete",
  "applicationCoverage": "adapter-attested-complete",
  "physicalCoverage": "compiler-verified-complete",
  "declarations": [],
  "tokens": [],
  "components": [],
  "routes": [],
  "islands": [],
  "syntheticProducers": [],
  "physicalRules": [],
  "physicalDeclarations": [],
  "edges": []
}
```

The semantic arrays and semantic edges retain the complete
[graph schema 1](./manifest-schema-4.md#graph-schema-1) contract. Graph schema 2 neither changes a
semantic declaration into a CSS property nor adds postprocessor-dependent data to a semantic ID.

### Exact graph-1 projection

For the same source, theme, reachability snapshot, and pruning setting, a consumer can project graph
schema 2 to graph schema 1 by:

1. changing the nested `schemaVersion` from `2` to `1`;
2. removing `physicalRuleIdFormatVersion`, `physicalDeclarationIdFormatVersion`, and
   `physicalCoverage`;
3. removing `syntheticProducers`, `physicalRules`, and `physicalDeclarations`; and
4. removing the four physical edge kinds defined below.

The remaining declarations, tokens, components, routes, islands, coverage attestations, and five
semantic edge kinds must equal the graph emitted by manifest schema 4. Their IDs, values, ordering,
and meaning are unchanged. This projection is a required compatibility gate, not a best-effort
migration.

## Physical rule nodes

Graph schema 2 supports five final rule kinds:

- `qualified`: a final selector rule containing physical declarations;
- `media`: a final `@media` wrapper containing nested rules;
- `container`: a final `@container` wrapper containing nested rules;
- `layer`: a final `@layer <name>` block containing nested rules; and
- `layer-order`: a final `@layer <name-list>;` ordering statement with neither declarations nor
  nested rules.

The active PliegoCSS emitter and Lightning CSS pipeline must produce only these kinds when schema 5
is requested. Encountering another final rule kind fails compilation rather than weakening physical
coverage.

Physical rules are numbered in depth-first preorder by the byte position of their opening token in
the final stylesheet. A media/container/layer group therefore precedes every rule nested inside it.
Physical rule ID format 1 is:

```text
css-rule:<global-preorder-ordinal-u32-hex8>
```

A rule node has this shape:

```json
{
  "id": "css-rule:00000000",
  "ordinal": 0,
  "kind": "qualified",
  "byteStart": 0,
  "byteEnd": 80,
  "headerByteStart": 0,
  "headerByteEnd": 29
}
```

All offsets are unsigned 64-bit integers serialized as JSON integers. Every interval is half-open and
counts UTF-8 bytes, never Unicode scalar values or UTF-16 code units.

- `byteStart` points to the first rule token.
- `byteEnd` points immediately after the closing `}`, or after `;` for `layer-order`.
- `headerByteStart..headerByteEnd` selects the exact final selector or complete `@media ...`,
  `@container ...`, or `@layer ...` header. A group header excludes whitespace before its opening
  `{` and the brace itself; a `layer-order` header excludes its terminating semicolon.
- A nested rule interval is strictly contained by its parent media/container/layer interval.
- Top-level rule intervals are disjoint. Whitespace between them and the required final newline are
  not separate graph nodes.

For example, this 49-byte stylesheet has two preorder rule nodes:

```css
@media (min-width:48rem){.pc_demo{display:grid}}
```

| ID | Kind | Whole range | Header range |
|---|---|---:|---:|
| `css-rule:00000000` | `media` | `0..48` | `0..24` |
| `css-rule:00000001` | `qualified` | `25..47` | `25..33` |

The final newline is byte `48` and lies outside both rule intervals.

## Physical declaration nodes

A physical declaration is one declaration occurrence in the final Lightning CSS serialization. It
is not deduplicated by property name or value. Two identical occurrences are two nodes.

Physical declaration ID format 1 is:

```text
css-decl:<owning-rule-preorder-u32-hex8>:<ordinal-in-rule-u32-hex8>
```

The declaration ordinal is zero-based in final serialized order inside its qualified rule. A node
has this shape:

```json
{
  "id": "css-decl:00000000:00000000",
  "ordinal": 0,
  "property": "padding",
  "important": false,
  "generated": false,
  "byteStart": 30,
  "byteEnd": 42,
  "propertyByteStart": 30,
  "propertyByteEnd": 37,
  "valueByteStart": 38,
  "valueByteEnd": 42
}
```

Range rules are:

- `byteStart..byteEnd` selects the complete declaration without surrounding whitespace or its
  trailing semicolon;
- `propertyByteStart..propertyByteEnd` selects the exact serialized property spelling and must equal
  the UTF-8 bytes represented by `property`;
- `valueByteStart..valueByteEnd` selects only the serialized value;
- when `important` is true, the complete range includes the serialized `!important` suffix while the
  value range excludes it; and
- `generated` is true when the emitter created the occurrence as support output for one or more
  semantic assignments, rather than serializing a direct assignment occurrence; and
- property and value ranges are non-empty, ordered, contained by the complete range, and contained by
  exactly one qualified rule.

`generated` is false for direct semantic output and for theme declarations. Producer edges, not the
boolean, distinguish a direct semantic contribution from the synthetic theme producer.

Consider the existing 81-byte CSS artifact:

```css
.pc_efkogezwizbem6tcas20wpx7h{padding:1rem;background-color:var(--color-accent)}
```

Its qualified rule is `0..80` with header `0..29`. Its declarations are:

| ID | Complete | Property | Value |
|---|---:|---:|---:|
| `css-decl:00000000:00000000` | `30..42` | `30..37` (`padding`) | `38..42` (`1rem`) |
| `css-decl:00000000:00000001` | `43..79` | `43..59` (`background-color`) | `60..79` (`var(--color-accent)`) |

The semicolon at byte `42`, closing brace at byte `79`, and newline at byte `80` are deliberately not
part of either declaration range.

## Synthetic theme producer

Semantic declarations are the producers for class-rule output. Theme custom properties emitted by
`--theme` have no semantic assignment and must not be attributed to a route, island, or component.
Graph schema 2 represents them with:

```json
{
  "id": "producer:theme",
  "kind": "theme"
}
```

This node appears in `syntheticProducers` when the selected build emits physical theme declarations.
Each such declaration has a `syntheticProducerProducesPhysicalDeclaration` edge. The semantic token
array remains graph-1-compatible: schema 5 does not add every emitted theme variable as though a
semantic assignment referenced it.

Compiler-generated support declarations that are caused by semantic assignments, including ring and
shadow initializers and their composed final `box-shadow`, use semantic contribution edges. They are
not incorrectly assigned to the theme producer.

With `--prune-unreachable`, synthetic theme output contains only variable-backed tokens directly
referenced by retained semantic styles. If no route or island contributes a root component, schema
5 contains an empty physical `:root{}` rule, no theme declarations, and no `producer:theme` node.
Without pruning, `--theme` and bundle `emit-theme = true` retain the complete supported block.
Authored `var(...)` consumers outside semantic styles are not part of this proof.

## Edge contract

Graph schema 2 retains all five graph-1 edge kinds:

| Edge | Direction |
|---|---|
| `styleHasDeclaration` | `style:*` -> `decl:*` |
| `declarationUsesToken` | `decl:*` -> `token:*` |
| `componentUsesDeclaration` | `component:*` -> `decl:*` |
| `routeUsesComponent` | `route:*` -> `component:*` |
| `islandUsesComponent` | `island:*` -> `component:*` |

It adds four physical edge kinds:

| Edge | Direction and meaning |
|---|---|
| `declarationContributesToPhysicalDeclaration` | `decl:*` -> `css-decl:*`; the assignment contributes to this final occurrence. |
| `syntheticProducerProducesPhysicalDeclaration` | `producer:*` -> `css-decl:*`; non-semantic build output produced this occurrence. |
| `physicalDeclarationBelongsToRule` | `css-decl:*` -> `css-rule:*`; exact qualified-rule ownership. |
| `ruleNestedInRule` | child `css-rule:*` -> parent `css-rule:*`; the parent must be a media, container, or layer group. |

Contribution is explicitly many-to-many:

- one semantic assignment can produce several physical declarations, such as `transition-colors`;
- target lowering can expand one emitter declaration into several final declarations;
- several ring/shadow assignments can contribute to one composed `box-shadow`; and
- two identical physical declarations remain separate occurrences with separate ordinal IDs.

Every physical declaration has exactly one `physicalDeclarationBelongsToRule` edge and at least one
incoming semantic contribution or synthetic-producer edge. Every non-top-level rule has exactly one
`ruleNestedInRule` edge. A top-level rule has none.

## Physical coverage and reconciliation

`physicalCoverage: "compiler-verified-complete"` simultaneously attests that:

- every final qualified, media, container, layer, and layer-order rule is represented;
- every final physical declaration occurrence is represented with exact final byte ranges;
- every physical declaration belongs to exactly one qualified rule;
- every semantic declaration contributes to at least one final physical declaration;
- every final physical declaration has complete emitter lineage; and
- every nested rule has complete media/container/layer containment; and
- every layer-order statement owns no declaration and cannot be a nesting parent.

The compiler establishes this coverage by reconciling structured emitter lineage with the final
Lightning CSS AST and its exact serialization. The final AST is authoritative for physical rule
order, declaration order, property spelling, value spelling, and byte ranges. Emitter lineage is
authoritative for semantic and synthetic producers.

The compiler must fail before staging CSS or manifest output if the two views cannot be reconciled.
It may not fill gaps by matching property/value strings, choosing a nearest source-map segment, or
assigning an unknown declaration to the surrounding style. A source map can be diagnostic evidence,
but it is not the physical coverage proof.

Temporary internal correlation data must not occur in the production CSS. The CSS bound by
`cssSha256` and `cssBytes` must be exactly the same artifact that schema 4 emits for the same inputs
and pruning setting. With pruning disabled it also equals the established schema-3 artifact.

## Canonical ordering and limits

Canonicalization rules are:

- semantic nodes and semantic edges preserve graph schema 1 ordering;
- `syntheticProducers` are ordered by ID;
- `physicalRules` are ordered by their ordinal IDs, which is final depth-first preorder;
- `physicalDeclarations` are ordered by owning-rule ordinal and declaration ordinal;
- edges are deduplicated and ordered by `(kind, from, to)`; and
- changing only source enumeration or reachability array order cannot change manifest bytes.

Graph schema 2 retains the graph budget of at most 65,535 total nodes, including referenced
top-level style nodes, and at most 65,535 edges. Physical and synthetic nodes count toward the same
budget. Raw emitter CSS and final CSS are each limited to 16 MiB when schema 5 is selected. All
additions and multiplicative contribution paths use checked arithmetic.

Ordinals are represented as unsigned 32-bit values and encoded as fixed eight-digit lowercase hex in
IDs. Byte offsets are unsigned 64-bit values. Implementations and consumers must reject overflow,
out-of-bounds ranges, narrowing failures, duplicate IDs, duplicate ordinals, invalid containment, and
non-canonical order.

## Targets, formats, and stability

The semantic portion of graph schema 2 is independent of target and printer selection. The physical
portion is bound to them:

- `modern` may lower a value, add compatibility output, or change final rule structure;
- `none` preserves syntax that does not require the configured compatibility transforms;
- `pretty` and `minified` normally retain the same semantic graph but use different byte offsets,
  CSS lengths, and digests; and
- a physical ID is stable only while the final rule/declaration preorder before that node remains
  unchanged.

For example, one semantic arbitrary color declaration can print differently for the current target
contracts:

```css
/* modern */
color:var(--lightningcss-light,red)var(--lightningcss-dark,#00f)

/* none */
color:light-dark(red,#00f)
```

The semantic declaration ID remains unchanged. Its physical declaration value range and surrounding
CSS integrity fields change. If a future target transform expands it into several declarations, all
resulting nodes inherit the same semantic contributor.

Consumers must treat `css-rule:*` and `css-decl:*` as artifact-local references. They must not cache
or compare them across different `cssSha256` values as though they were semantic identities.

## Schema behavior

| Manifest | Reachability | Pruning | Graph | Physical trace | CSS effect |
|---:|---|---|---|---|---|
| 3 | forbidden | forbidden | absent | absent | established default bytes |
| 4 | required | off | schema 1 | absent | byte-identical to schema 3 |
| 5 | required | off | schema 2 | complete or build failure | byte-identical to schemas 3 and 4 |
| 4 | required | `--prune-unreachable` | schema 1 over emitted styles | absent | only StyleIds with at least one reachable exact origin |
| 5 | required | `--prune-unreachable` | schema 2 over emitted styles | complete or build failure | byte-identical to pruned schema 4 |

Absence of a physical trace in schema 3 or 4 means “not requested,” not an empty complete trace.
Schema 5 never emits graph schema 1 and never emits a graph schema 2 with partial physical coverage.

For declarative bundles, every CSS/manifest pair is a separate artifact: physical ordinals restart at
zero, all ranges address that bundle's adjacent CSS, and the bundle's own digest is the integrity
boundary. Pruning classifies only the styles and origins already assigned to that bundle. A
reachability-only watch rebuild with pruning disabled, or one that leaves the retained StyleId set
unchanged, may change application nodes or edges without changing physical nodes or CSS integrity
metadata.

## Consumer migration

1. Continue accepting manifest schema 3 and graph-free output.
2. Continue accepting manifest schema 4 with graph schema 1.
3. Add a separate schema-5 parser that requires graph schema 2 and both physical ID versions.
4. Verify CSS length and digest before converting or slicing any range.
5. Validate exact graph-1 projection and every typed physical endpoint.
6. Require `originCoverage`, `applicationCoverage`, and `physicalCoverage` at their exact complete
   values before using the graph for route or pruning decisions.
7. Treat any emitted theme-produced declarations as application-global; under pruning their
   declaration set is already filtered by retained semantic token consumption.
8. Reject unknown rule kinds, edge kinds, coverage values, or physical ID versions.

## Security and deployment

Manifest schema 5 is not an authorization graph. The application attestation remains supplied by the
framework adapter, while physical coverage is compiler-verified against one build artifact. Neither
attestation establishes that the producer or sidecar is trusted.

Schema 5 can reveal application topology, source paths and ranges, selectors, declaration values,
theme variables, and exact asset layout. Ship it to production only when a trusted runtime or
debugging consumer requires it. Otherwise keep it with build evidence.

Consumers must parse the manifest and CSS as untrusted input, apply graph budgets before allocation,
perform checked integer conversion, validate UTF-8 boundaries, and verify that every range is within
the exact digest-bound CSS artifact before displaying or acting on it.

The rationale for failing closed is recorded in
[ADR-0009](../adr/0009-require-fail-closed-physical-css-trace.md).
