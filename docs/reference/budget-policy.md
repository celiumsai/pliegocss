# CSS budget policy schema 1

Status: **implemented for one exact CSS artifact and verified Asset Plan file/layer plus
ownership-backed package/composed-route aggregation**

Budget policy schema 1 makes CSS growth a deterministic, reviewable gate. It is optional and
read-only:

```console
pliego-cssc audit \
  --input dist/home.css \
  --targets baseline-widely \
  --budget-policy config/pliego.budgets.json \
  --budget-subject package=storefront \
  --budget-subject route=/home \
  --format json
```

In `--input` mode, the file is always observed as `file:dist/home.css`. Each repeatable
`--budget-subject` attests that the same complete artifact belongs to one package or route. Named
`@layer` blocks are discovered from the AST; nested layer names are dot-qualified. Anonymous layer
blocks are reported under the explicit `anonymous` aggregate (or `<parent>.anonymous` when nested),
not represented as one semantic layer identity. File and layer subjects cannot be supplied
manually. Coverage is total: every `budgets[]` definition in an explicitly selected policy must
match one verified observed subject. If even one remains unobserved, `PCSS-BUDGET-199` fails the
audit; matching definitions may still emit their metric findings, but they cannot make a partially
covered policy pass.

For generated multi-bundle output, use the Asset Plan directly:

```console
pliego-cssc audit \
  --asset-plan dist/assets/pliego.assets.json \
  --ownership config/pliego.ownership.json \
  --targets baseline-widely \
  --budget-policy config/pliego.budgets.json \
  --format json
```

The plan is not trusted as an unchecked list. The command loads its adjacent CSS/manifests and
requires the original bytes to equal a canonical `build_asset_plan` regeneration. `file` covers
each bundle CSS and `layer` aggregates matching layers over the complete plan. Asset Plan schema 1
keeps routes and islands separate, so every package or route policy additionally requires the
explicit [ownership sidecar schema 1](./ownership-schema-1.md). That sidecar binds the exact plan
byte count and SHA-256, assigns every bundle to exactly one package, and supplies one island list
for every route. `--budget-subject` is rejected in Asset Plan mode; the sidecar is the sole
package/route authority and is never discovered.

Ownership-backed observations use:

```text
package = every complete bundle exclusively assigned to packageId
route   = stable dedupe(route base bundles ∪ every declared island bundle)
```

The stable dedupe preserves top-level Asset Plan bundle order, so a shared theme/global bundle is
measured once per subject. Packages partition the complete bundle ledger. Routes remain overlapping
views and must not be projected as `Control Manifest rules.byRoute` partitions.

## Closed JSON contract

```json
{
  "schemaVersion": 1,
  "policyVersion": 1,
  "budgets": [
    {
      "id": "home-route",
      "subject": { "kind": "route", "id": "/home" },
      "limits": {
        "bytes": { "maximum": 18000, "baseline": 16000, "maxIncrease": 500 },
        "rules": { "maximum": 240, "baseline": 220, "maxIncrease": 8 },
        "selectors": { "maximum": 300, "baseline": 274, "maxIncrease": 10 },
        "specificity": {
          "maximum": "1,3,1",
          "baseline": "0,3,1",
          "maxIncrease": "0,1,0"
        },
        "semanticDuplicates": { "maximum": 4, "baseline": 2, "maxIncrease": 1 }
      }
    },
    {
      "id": "component-layer",
      "subject": { "kind": "layer", "id": "app.components" },
      "limits": {
        "rules": { "maximum": 100, "baseline": null, "maxIncrease": null }
      }
    }
  ],
  "exceptions": [
    {
      "id": "reviewed-home-growth",
      "budget": "home-route",
      "metrics": ["rules"],
      "justification": "temporary split retained through the measured checkout redesign"
    }
  ]
}
```

Unknown fields, unsupported versions, empty policies, duplicate IDs or subjects, unsafe paths,
ambiguous exceptions, and `maxIncrease` without `baseline` fail closed. Budget and exception arrays
are canonicalized before hashing, so cosmetic input order cannot change finding identity. Each
subject may have only one budget definition. The canonical policy SHA-256 and policy version appear
on every result.

`kind` accepts:

| Kind | Identifier | Source of ownership |
|---|---|---|
| `file` | Portable project-relative CSS path | Automatic from `--input`, or once per Asset Plan CSS file. |
| `package` | Stable package ID | Explicit `--budget-subject package=NAME` for direct `--input`; ownership schema 1 is the exclusive typed authority for Asset Plan bundle groups. |
| `route` | Canonical absolute route path | Explicit `--budget-subject route=/PATH` for direct `--input`; ownership schema 1 supplies the Asset Plan base-plus-island bundle union. |
| `layer` | Canonical serialized layer name | Automatic from parsed `@layer` blocks; aggregated across an Asset Plan ledger. |

Explicit route/package attribution is an assertion by the caller or adapter, not an inference from
filename or selector text. In Asset Plan mode, the verified plan supplies base bundle membership and
the exact-bound ownership sidecar supplies complete package ownership and route/island composition.

## Metric semantics

| Metric | Exact meaning |
|---|---|
| `bytes` | Byte length of the parsed stylesheet serialized with the frozen Lightning CSS backend and `minify=true`. Formatting/comments do not consume budget. Layer bytes cover the canonical contents of all blocks with that full layer name. |
| `rules` | Recursive AST rule count. A layer subject includes descendants in nested conditional/layer blocks. |
| `selectors` | Number of selectors across style rules, including nested style rules. |
| `specificity` | Highest selector tuple in standard `id,class-or-attribute,type` order. Values use strings such as `"1,3,1"`, never a lossy scalar score. |
| `semanticDuplicates` | Additional style-rule occurrences beyond the first that share the same normalized ordered declaration block and the same canonical ancestor condition/layer/nesting context. Selector names are intentionally excluded so mergeable sibling rules can be detected. |

Semantic duplication is a consolidation signal, not proof that a rule is dead or safe to delete.
Different media/supports/container/scope conditions, cascade layers, and nesting parents produce
different fingerprints. Declaration order and `!important` remain part of the fingerprint because
they can affect behavior. The inventory emits sorted SHA-256 evidence for at most the first 256
duplicate groups plus `semantic-fingerprint-evidence-truncated`; the total group/occurrence metrics
remain complete even when detailed evidence is bounded.

## Absolute and regression boundaries

`maximum` is always enforced. Optional `baseline` records the reviewed prior value. When
`maxIncrease` is present, the current value must also be less than or equal to
`baseline + maxIncrease`; this catches a regression even while the broad ceiling still passes.
Findings record current, maximum, baseline, signed delta, permitted increase, and which boundary was
exceeded.

Specificity addition and comparison are component-aware and then use CSS lexicographic precedence.
For example, baseline `0,2,1` plus permitted increase `0,1,0` allows at most `0,3,1`; it does not
invent a weighted specificity score.

## Exceptions and findings

Exceptions live inside the integrity-bound policy, require a stable ID and justification, name an
existing budget and configured metric, and cannot overlap. They do not erase a violation:

| Code | Meaning | Gate result |
|---|---|---|
| `PCSS-BUDGET-100` | Both absolute and regression boundaries pass. | Pass. |
| `PCSS-BUDGET-101` | Boundary exceeded with no reviewed exception. | Fail. |
| `PCSS-BUDGET-102` | Boundary exceeded under the recorded exception. | Warning/pass. |
| `PCSS-BUDGET-198` | At least one Asset Plan CSS bundle could not be measured after syntax ingestion. No partial route/package/layer budget is reported. | Fail. |
| `PCSS-BUDGET-199` | One or more definitions in the explicit policy matched no verified observed subject. Evidence records declared, matched, and observed counts. | Fail, including partial coverage. |

An excepted finding retains actual evidence and attaches the canonical finding-schema exception; it
is never rewritten as “within budget.” Schema 1 deliberately has no wall-clock expiry behavior,
because local clock state would make identical inputs produce different results. Time-bound review
workflow belongs in the future receipt/policy layer.

`PCSS-BUDGET-199` is not an exception mechanism. It proves coverage of the policy itself:
`matched-budgets` must equal `declared-budgets`. A typo, stale package or route subject, or undetected
layer therefore fails closed instead of silently evaluating only the definitions that happened to
match.

## Current boundary

- Direct `--input` package/route ownership remains an explicit `--budget-subject` assertion. Asset
  Plan package/route policies require the implemented total/exclusive ownership sidecar.
- Route views can overlap and therefore are not Control Manifest partitions. Package budget views
  are exclusive, but `rules.byPackage` also remains `unknown` until a future manifest schema carries
  and separately validates that partition. Aggregation does not calculate gzip/brotli size.
- It does not classify a duplicate as removable, unused, unobserved, or dead.
- It does not yet produce a unified receipt; human, JSON, and SARIF derive from finding schema 1.0.0.
- Layer metrics are attached to the first `@layer` occurrence span and merge repeated blocks with the
  same canonical full name.

These limits keep the current result exact. Observation state, conservative removal, and broader
receipt evidence remain separate R0 gates.
