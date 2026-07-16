# Cascade explanation command

Status: bounded schema 1 implemented and locally verified.

`pliego-cssc explain-cascade` answers a deliberately narrow question over ordinary CSS: which
direct author declaration for one longhand property wins for one declared HTML element inside one
complete stylesheet? It uses the same Lightning CSS AST boundary as `audit`, preserves exact
authored declaration ranges, and never turns missing runtime context into a guessed winner.

```console
pliego-cssc explain-cascade \
  --input dist/app.css \
  --element 'button#save.action' \
  --property color \
  --format text

pliego-cssc explain-cascade \
  --input dist/app.css \
  --element 'button#save.action' \
  --property color \
  --format json
```

The command exits successfully for all three analysis statuses. Invalid invocation, invalid CSS,
unsafe/non-regular input, unsupported query properties, or source-reconciliation failure is a CLI
error. `browser-required` is a valid, machine-readable answer rather than a failed process.

## Static proof boundary

Schema 1 resolves only:

- one complete UTF-8 author stylesheet of at most 16 MiB;
- one target expressed as a single compound HTML selector containing only a type, `#id`, `.class`,
  or `*`;
- one closed, direction-independent standard longhand;
- direct style-rule declarations, including values extracted from a parsed shorthand;
- top-level named cascade layers in first-appearance order;
- normal versus `!important` declarations;
- the specificity of the most-specific matching selector in a selector list;
- source order using the exact declaration byte start.

The initial longhand set is:

```text
background-color  color  display  font-family  font-size  font-style  font-weight
line-height  opacity  order  overflow-x  overflow-y  pointer-events  position
text-align  text-decoration-color  text-decoration-line  text-decoration-style
text-transform  visibility  white-space  z-index
```

The set is closed because logical/physical property interaction, writing mode, and a broader
shorthand graph must not be silently approximated. Expanding it requires executable cases proving
the additional cascade relationships.

This is a stylesheet-local author-cascade result. It does not include inline styles, other
stylesheets, user or user-agent origins, inheritance, custom-property substitution, computed/used
values, layout, or paint. A missing element type means that a type selector cannot be disproved; it
therefore becomes a blocker if it could affect the query.

## Precedence

When the query is statically resolvable, candidates are compared in this order:

1. important declarations outrank normal declarations;
2. normal unlayered declarations outrank normal layered declarations;
3. later named layers outrank earlier named layers for normal declarations;
4. important layered declarations outrank important unlayered declarations;
5. earlier named layers outrank later named layers for important declarations;
6. higher selector specificity wins;
7. later source order wins.

This preserves the native reversal of layer order for `!important`. Each overridden candidate
records the first decisive criterion: `importance`, `layer-order`, `specificity`, or `source-order`.
The document also reports the minimum, maximum, and winning specificity and whether the winner
exceeds the least-specific competitor.

## Statuses

| Status | Meaning |
|---|---|
| `resolved` | `winner` references one candidate proven inside the schema-1 boundary. |
| `no-match` | No declaration in the input matches both the element and property. This does not mean the browser has no computed value. |
| `browser-required` | At least one potentially relevant construct requires DOM, condition, origin, or runtime evidence. `winner` is `null`. |

Candidate declarations remain visible under `browser-required` so a developer or agent can see the
known part of the competition. They retain disposition `candidate`; none is mislabeled as winner or
overridden.

## Browser-required blockers

Schema 1 can emit these stable blocker codes:

| Code | Required evidence |
|---|---|
| `unsupported-selector` | Combinators, attributes, pseudo-classes/elements, namespace selectors, or an unknown target type need DOM/state matching. |
| `media-context` | Active viewport/media features. |
| `supports-context` | Actual target-browser feature support. |
| `container-context` | Container size/style and layout. |
| `scope-context` | DOM scope membership and proximity. |
| `document-context` | Browser document condition. |
| `nested-selector` / `nested-declarations` | Parent selector and nested-rule ownership. |
| `starting-style-context` | Runtime transition state. |
| `transition-origin` | A matching transition can outrank author declarations at runtime. |
| `animation-origin` | Keyframes can contribute the queried property. |
| `imported-stylesheet` | Imported rules are absent from the single input snapshot. |
| `anonymous-layer` / `nested-layer` / `nested-layer-order` | Layer ordering exceeds the top-level named-layer model. |
| `namespace-context` | Namespace-aware matching is required. |
| `all-property` | `all` needs full CSS-wide keyword expansion. |
| `revert-keyword` | `revert` or `revert-layer` needs broader origin/layer history. |
| `unknown-at-rule` / `custom-at-rule` | The parser cannot prove the at-rule's cascade semantics. |

A selector with an already-proven mismatch does not block the query merely because it also contains
a dynamic component. For example, `.other:hover` cannot match an element whose exact class set does
not contain `other`.

## JSON schema 1

JSON uses `schemaVersion: 1` and these required top-level fields:

```json
{
  "schemaVersion": 1,
  "scope": "single-author-stylesheet-direct-element-declarations",
  "input": "dist/app.css",
  "property": "color",
  "element": {
    "selector": "button#save.action",
    "localName": "button",
    "id": "save",
    "classes": ["action"]
  },
  "status": "resolved",
  "winner": "candidate-0001",
  "candidates": [{
    "id": "candidate-0001",
    "selector": "button#save.action",
    "declaration": "color: blue",
    "effectiveDeclaration": "color: blue",
    "important": false,
    "layer": null,
    "layerOrder": null,
    "specificity": {"ids": 1, "classes": 1, "types": 1},
    "sourceOrder": 21,
    "source": {
      "path": "dist/app.css",
      "byteStart": 21,
      "byteEnd": 32,
      "startLine": 1,
      "startColumn": 22,
      "endLine": 1,
      "endColumn": 33
    },
    "disposition": "winner",
    "decisiveCriterion": null
  }],
  "specificityEscalation": {
    "winnerExceedsMinimum": false,
    "minimum": {"ids": 1, "classes": 1, "types": 1},
    "maximum": {"ids": 1, "classes": 1, "types": 1},
    "winner": {"ids": 1, "classes": 1, "types": 1}
  },
  "blockers": [],
  "limitations": [
    "does not include inline styles, other stylesheets, user or user-agent origins",
    "does not compute inheritance, custom-property substitution, layout, or used values",
    "browser-required means no static winner is claimed"
  ]
}
```

Every candidate contains `id`, `selector`, exact `declaration`, canonical
`effectiveDeclaration`, `important`, `layer`, `layerOrder`, structured specificity, `sourceOrder`,
exact source range, `disposition`, and `decisiveCriterion`. A shorthand such as `font` remains in
`declaration`; the extracted queried longhand appears in `effectiveDeclaration`. Source ranges use
zero-based UTF-8 bytes and one-based lines/UTF-16 columns. The bytes at
`source.byteStart..source.byteEnd` equal `declaration` exactly.

Adding, removing, renaming, or changing the type of a required field requires a schema bump. Human
text is informative and is not a byte-stable wire contract.

## What remains open

This slice does not close strategic pillar S3 or R1. Full cascade explanation still needs multiple
stylesheets and origins, inline styles, complete selector matching, nested layers, scopes, logical
properties, custom-property provenance, inheritance, animation/transition evidence, typed
component/route/artifact joins, and a browser-assisted continuation. The bounded command is the
fail-closed static foundation for that work, not a claim that PliegoCSS replaces the browser's
cascade engine.
