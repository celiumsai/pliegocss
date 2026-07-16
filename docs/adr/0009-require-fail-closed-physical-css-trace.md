# ADR-0009: Require a fail-closed physical CSS trace

Status: accepted and implemented for manifest schema 5

## Context

Manifest schema 4 and graph schema 1 identify canonical semantic assignments. That boundary is
stable across CSS printing, but it cannot answer which exact declarations and rules exist in the
adjacent stylesheet. One semantic assignment can emit several declarations, several assignments can
contribute to one generated declaration, and Lightning CSS can rewrite, expand, or otherwise change
the emitter output for a browser target.

Neither final `property:value` matching nor an ordinary source map proves this relationship. Text
matching is ambiguous for repeated properties, shorthands, generated initializers, and composed
effects. Source maps describe generated positions at their supported granularity; they do not attest
complete declaration-level lineage through every Lightning CSS transformation.

The physical trace is used by debuggers, editors, route-asset tooling, and to verify the final CSS
that remains after opt-in rule pruning; future reporting and token optimization can build on it.
Incorrect attribution would therefore be worse than an unavailable trace.

## Decision

Physical tracing is an explicit manifest schema 5 contract. Schema 5 requires the same strict
reachability schema 1 document as schema 4 and contains graph schema 2. Graph schema 2 is a strict
superset of graph schema 1: its semantic declarations, tokens, components, routes, islands, and five
semantic edge kinds project exactly to graph schema 1.

Graph schema 2 additionally records:

- final physical rules in depth-first preorder;
- final physical declarations and exact half-open UTF-8 byte ranges into the adjacent CSS;
- many-to-many contribution edges from semantic declarations to physical declarations;
- containment from physical declarations to rules and from nested rules to media wrappers;
- a synthetic theme producer for declarations emitted by `--theme`; and
- compiler-verified complete physical coverage.

Physical rule ID format 1 and physical declaration ID format 1 are ordinal, artifact-local
identities. They are deterministic for one final rule/declaration order but are not stable identities
across different CSS artifacts.

The compiler must carry declaration lineage from the emitter and reconcile it with the final
Lightning CSS AST and serialized stylesheet. A target transform that expands one declaration
inherits its complete producer set. A physical declaration composed from several semantic
assignments records every contributor. Theme declarations are attributed to `producer:theme`, not to
an application route or component.

The implementation uses the same emitter path for traced and untraced builds. Traced emission adds
canonical semantic ordinals to an out-of-band lineage structure; it does not inject correlation
markers into CSS. Reconciliation independently parses the raw emitter stylesheet and the exact final
stylesheet, serializes each Lightning CSS property under the selected target/printer contract, and
requires rule headers, declaration order, declaration text, producer coverage, and final byte ranges
to agree before graph construction.

The trace may not be inferred by comparing final property names or values, and a Lightning CSS
source map may not be treated as declaration-level proof. Internal correlation metadata is allowed,
but it must never alter or remain in the production stylesheet.

Compilation fails before output staging when any of the following occurs:

- emitter lineage cannot be reconciled with the final Lightning CSS structure;
- a semantic declaration has no final physical contribution;
- a final physical declaration has no semantic or synthetic producer;
- a physical declaration cannot be assigned to exactly one qualified rule;
- a nested rule cannot be assigned to its containing media rule;
- a byte range does not select the exact final CSS bytes it describes;
- the final stylesheet contains a physical rule kind unsupported by graph schema 2; or
- graph node, edge, ordinal, or integer limits are exceeded.

No partial graph may use the complete-coverage value.

## Compatibility boundary

Manifest schema 3 remains the default and has no graph. Manifest schema 4 remains the explicit
semantic ownership graph. Their JSON bytes and their established semantics do not change.

Selecting manifest schema 5 must not change CSS generation relative to schema 4 with the same
pruning setting. Given identical semantic inputs, theme emission, targets, and output format,
schemas 3, 4, and 5 produce byte-identical CSS when pruning is disabled; pruned schemas 4 and 5 are
likewise byte-identical to each other. Schema 5 only adds metadata bound to those bytes by
`cssSha256` and `cssBytes`.

Graph schema 2 retains graph schema 1 IDs and semantic ordering. Removing schema-2-only fields,
nodes, and edges and changing its nested `schemaVersion` to `1` must yield the same semantic graph as
a schema-4 build of the same snapshot.

## Targets and formats

The semantic graph is independent of the `baseline-widely`, `modern`, or `none` target contract and of the `minified` or
`pretty` printer. The physical graph describes the selected final artifact:

- target lowering may change physical values or the number and order of declarations and rules;
- pretty and minified output normally have different byte ranges, lengths, and digests; and
- ordinal physical IDs may coincide when final structure and order coincide, but consumers must not
  rely on cross-artifact stability.

Every consumer must verify the top-level target, format, identity versions, CSS byte count, and CSS
digest before following a physical edge or slicing a byte range.

## Consequences

- `--manifest-version 5` also requires `--reachability`; there is no separate trace flag.
- Schema 5 pays the additional tracing and reconciliation cost. Schemas 3 and 4 do not.
- Raw emitter CSS and final CSS are each capped at 16 MiB; graph schema 2 is capped at 65,535 total
  nodes and 65,535 edges.
- The physical manifest can be substantially larger than the stylesheet, especially with theme
  emission or target-dependent expansion.
- Effects such as ring plus shadow require many-to-many attribution rather than a one-to-one model.
- Future rule kinds require a graph-schema change or an explicit extension; schema 5 fails instead of
  claiming coverage it cannot prove.
- A Lightning CSS upgrade must pass the physical-trace golden and adversarial matrix before it can
  replace the pinned postprocessor behavior.

## Security

The manifest remains build metadata, not an authorization artifact. Reachability is adapter-attested,
and application topology, source origins, CSS selectors, declarations, and exact byte ranges may be
sensitive. Production deployments should ship schema 5 only when a trusted consumer needs it.

Consumers must treat all IDs, counts, and ranges as untrusted until schema versions and CSS integrity
have been verified. Range conversion must be checked rather than narrowed implicitly, even though the
contract serializes offsets as unsigned 64-bit values and graph budgets are much smaller.

See [manifest schema 5](../reference/manifest-schema-5.md) for the complete wire contract.
