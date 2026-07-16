# Accessibility policy schema 1

Status: **implemented bounded static-analysis contract; browser and manual evidence remain separate**

`pliego-cssc audit` can apply one explicit accessibility policy to ordinary CSS or every stylesheet
referenced by an Asset Plan. The policy configures five deterministic checks, declares contrast
relationships, and records reviewed exceptions. It does not rewrite CSS and it never represents its
result as WCAG certification.

```console
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json
```

Use `--token-graph` when a contrast endpoint names a token or a relationship selects a resolved
theme:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json \
  --token-graph dist/pliego.tokens.json --format json
```

`--token-graph` requires `--accessibility-policy`. A policy containing only literal-to-literal
relationships without `selections` can be evaluated without a graph.

## Complete document

Schema 1 is a closed object. All five checks and all three enforcement fields are mandatory:

```json
{
  "schemaVersion": 1,
  "policyVersion": 1,
  "checks": {
    "contrast": {
      "violation": "fail",
      "unverified": "warn",
      "manualRequired": "warn"
    },
    "motion": {
      "violation": "fail",
      "unverified": "warn",
      "manualRequired": "warn"
    },
    "focusVisibility": {
      "violation": "fail",
      "unverified": "warn",
      "manualRequired": "warn"
    },
    "forcedColors": {
      "violation": "fail",
      "unverified": "warn",
      "manualRequired": "warn"
    },
    "inputModality": {
      "violation": "fail",
      "unverified": "warn",
      "manualRequired": "warn"
    }
  },
  "contrastPairs": [
    {
      "id": "body-on-surface",
      "foreground": {
        "literal": {
          "value": "#1f2937"
        }
      },
      "background": {
        "literal": {
          "value": "#ffffff"
        }
      },
      "minimumRatioMilli": 4500
    }
  ],
  "exceptions": []
}
```

Unknown fields, unsupported versions, missing check settings, duplicate IDs, ambiguous exceptions,
and invalid limits fail before CSS evaluation.

| Field | Contract |
|---|---|
| `schemaVersion` | Exactly `1`. A wire-shape change requires a new schema. |
| `policyVersion` | Exactly `1`. A semantic interpretation change requires a new policy version. |
| `checks` | Closed object containing the five required checks below. |
| `contrastPairs` | Zero to 256 explicit foreground/background relationships, canonicalized by `id`. |
| `exceptions` | Zero to 4,096 reviewed exceptions, canonicalized by `id`. |

The complete input is limited to 1 MiB. `AccessibilityPolicy::to_json_pretty` emits validated,
canonically ordered pretty JSON with one trailing newline. Parsing accepts ordinary JSON ordering,
then canonicalizes pair and exception arrays before policy evaluation.

Additional defensive limits are part of the schema:

| Value | Limit |
|---|---:|
| Pair or exception ID | Lowercase kebab-case ASCII, at most 128 bytes |
| Token endpoint | `color.` plus one lowercase kebab name, at most 256 bytes after the prefix |
| Literal color or exception justification | Non-empty, at most 4,096 bytes, no control characters |
| Selection map | 1 to 64 entries when present |
| Selection key or value | 1 to 128 bytes, no controls or surrounding whitespace |
| Exception subject | Canonical `sha256:` plus 64 lowercase hexadecimal characters |

## Enforcement vector

Each check configures the result classes that are not automatic passes:

| Field | Meaning |
|---|---|
| `violation` | The static analyzer proved that the configured rule failed. |
| `unverified` | The analyzer had relevant input but could not prove pass or failure. |
| `manualRequired` | The result depends on runtime, browser, DOM, interaction, or human evidence. |

Each value is `fail` or `warn`. `fail` produces an error finding and makes the accessibility gate
fail. `warn` produces a warning and does not fail it. A verified pass is always informational. A
reviewed matching exception is always a warning and does not fail, regardless of the configured
enforcement, while retaining the original verification class and evidence.

The checks are:

| Check | Static boundary |
|---|---|
| `contrast` | Evaluates only declared color relationships. It does not infer foreground/background pairing from selectors or DOM. |
| `motion` | Verifies active animation, transition, or smooth scrolling only when its own rule is guarded by the exact positive `prefers-reduced-motion: no-preference` query and no competing same-file selector/family declaration is observed. Active motion under `reduce` is a violation; partial longhands, mixed shorthand/longhand, separate overrides, competitors, and ambiguous media expressions do not produce a pass without cascade proof. |
| `focusVisibility` | Proves explicit static outline suppression on `:focus` and `:focus-visible` as a violation. Other focus rules remain manual-required until computed cascade and indicator visibility are available. |
| `forcedColors` | Inspects explicit `forced-color-adjust`; `none` is a violation and dynamic values require manual verification. |
| `inputModality` | Verifies only a same-file, same-context style rule whose selector list provides both exact `:hover` and focus alternatives over the same base and declaration set. Separate rules, different files, nesting, scope, layer, complex selectors, or unequal declarations require manual verification. |

## Declared contrast relationships

A pair ID is unique lowercase kebab-case ASCII. Both endpoints use an externally tagged object:

```json
{
  "id": "button-label-dark",
  "foreground": {
    "token": {
      "name": "color.button-label"
    }
  },
  "background": {
    "token": {
      "name": "color.button-surface"
    }
  },
  "minimumRatioMilli": 4500,
  "selections": {
    "appearance": "dark"
  }
}
```

`literal.value` is a CSS color string without surrounding whitespace. `token.name` must use
`color.<kebab-name>` and match the flattened canonical color name in the token graph exactly. DTCG
paths are projected into that kebab name; a dotted path after `color.` is not accepted.

`minimumRatioMilli` stores the required ratio without a floating-point policy value. `4500` means
4.5:1, `3000` means 3:1, and `7000` means 7:1. The accepted range is 1,000 through 21,000.
PliegoCSS uses the WCAG 2.2 relative-luminance formula, including the `0.04045` channel cutoff, and
compares the unrounded computed ratio with the configured boundary. Policy authors select the
threshold; static CSS analysis does not infer whether rendered text is large or bold.

When `selections` is absent and a graph is supplied, the pair is evaluated independently for every
resolved graph theme. When it is present, it must contain 1 to 64 modifier/context entries and must
match one theme's complete selection map exactly. A missing selection is `unverified`. Without a
graph, only a literal/literal pair with no selection is verifiable; any token endpoint or selection
produces an `unverified` finding.

Verified color evidence is restricted to statically resolved, fully opaque, in-gamut sRGB values.
Runtime-dependent values such as `var()`, `currentColor`, system colors, or `light-dark()` require
manual verification. Unresolved channels, alpha below one, and out-of-sRGB values remain
`unverified`; they are never reported as passes.

The same graph can produce several findings for one declared pair because each theme is separate.
`tokens.contrastPairs` counts declared relationships, not the number of theme evaluations.

## Finding codes

All output formats derive from the same canonical finding document. The policy owns severity; the
renderer does not.

| Code | Meaning |
|---|---|
| `PCSS-A11Y-000` | Summary of this configured static gate. It is not a global compliance result. |
| `PCSS-A11Y-100` / `101` / `102` / `108` | Contrast pass, violation, reviewed exception, or unverified/manual result. |
| `PCSS-A11Y-200` / `201` / `202` / `208` / `209` | Motion pass, violation, reviewed exception, unverified, or manual-required result. |
| `PCSS-A11Y-300` / `301` / `302` / `308` / `309` | Reserved focus pass, explicit suppression violation, reviewed exception, unverified, or manual-required result. Schema 1 does not emit `300` without computed-cascade proof. |
| `PCSS-A11Y-400` / `401` / `402` / `409` | Forced-colors pass, violation, reviewed exception, or unverified/manual-required result. |
| `PCSS-A11Y-500` / `501` / `502` / `508` / `509` | Input-modality pass, violation, reviewed exception, unverified, or manual-required result. |
| `PCSS-A11Y-999` | A configured exception matched no current non-pass observation. It warns and does not fail. |

Every non-summary observation carries `context.subject-id` in JSON, rendered as `subject-id` in
human output and preserved inside SARIF's canonical finding property.

## Stable subjects and exceptions

An exception never erases a result. It changes the finding to the check's `x02` code, attaches the
review metadata, keeps source and evidence, and prevents that observation from failing the gate.

```json
{
  "id": "reviewed-legacy-logo",
  "check": "forcedColors",
  "subjectId": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "justification": "Brand-owner review requires this fixed-color mark until asset replacement.",
  "expiresOn": "2027-03-31"
}
```

Copy the exact `context.subject-id` from a current JSON finding; the digest above is only a shape
example. IDs use lowercase kebab-case ASCII. `subjectId` is `sha256:` plus 64 lowercase hexadecimal
characters. Justifications are required and limited to 4,096 bytes. `expiresOn` is required and must
be a real `YYYY-MM-DD` date.

The subject digest uses the domain `pliegocss-accessibility-subject-v1\0`, then each semantic part as
an unsigned 64-bit big-endian byte length followed by its UTF-8 bytes. It binds the complete semantic
claim under review: contrast endpoints, threshold, theme, resolved/error evidence, ratio and result
class as well as pair ID/selections; CSS observations bind the result class, logical stylesheet,
canonical selector/context, family, relevant declaration values/`!important`, and relational
candidate/competitor evidence. Source offsets and rendered severity are excluded. Changing the
claim invalidates an old exception; moving otherwise identical evidence does not.

Only one exception may cover the same check and subject. `expiresOn` is integrity-bound metadata but
the evaluator deliberately has no wall clock. A date becoming past therefore does not change the
same input's result. CI or review automation must enforce time-based review separately; a future
explicit evaluation-date contract would require a versioned policy change.

## Control artifacts

The exact policy bytes enter the audit input ledger with role `accessibility-policy`. The optional
canonical graph bytes enter with role `token-graph`. Both affect `configHash` independently of
stdout format. With `--control-dir`, a supplied graph is also published as the fixed
`pliego.tokens.json` output and integrity-bound by the manifest and receipt. A graph-bearing audit
group therefore contains four artifacts; a policy-only audit retains the established three-artifact
group.

The audit-side token projection records the union of declared typed tokens across validated themes
and the declared contrast-pair count. Its zero coverage means this audit mode observed no
compiler-owned typed-use set; it is not evidence that the application uses zero tokens. PliegoCSS
never infers typed token use from custom-property spelling in arbitrary CSS.

## Static-analysis limits and non-claims

- A declared pair is an application contract, not an inferred DOM relationship.
- The analyzer does not model inheritance, background stacks, images, gradients, blending,
  transparency composition, text size/weight, or state-dependent rendering.
- A separate reduced-motion override does not prove cascade or that all runtime interaction is safe
  and usable; it therefore cannot create a verified pass in schema 1.
- Outline analysis can prove an explicit suppression request, but does not prove a positive computed
  indicator, focus order, clipping, overlay visibility, keyboard operation, or replacement-indicator
  visibility.
- Forced-colors behavior requires execution on the relevant browser and platform.
- Same-rule hover/focus declaration equivalence does not prove DOM semantics, touch behavior, target
  size, or JavaScript keyboard equivalence.
- `verified` means verified only inside the recorded static-analysis boundary.
- `PCSS-A11Y-000` and a successful process exit are not WCAG conformance or certification.

Use the analyzer alongside the authoritative [WCAG 2.2 contrast requirements](https://www.w3.org/TR/2024/REC-WCAG22-20241212/#contrast-minimum),
[animation-from-interactions guidance](https://www.w3.org/WAI/WCAG22/Understanding/animation-from-interactions.html),
[focus-indicator failure guidance](https://www.w3.org/WAI/WCAG22/Techniques/failures/F78.html),
[CSS Color Adjustment](https://www.w3.org/TR/2025/CR-css-color-adjust-1-20251216/#forced-color-adjust-prop), and
[Media Queries Level 5](https://www.w3.org/TR/2026/WD-mediaqueries-5-20260219/#prefers-reduced-motion).

See [finding schema 1](./finding-schema-1.md), the
[token graph contract](./token-graph-schema-1.md), and the
[configuration guide](../how-to/configure-accessibility-policies.md).
