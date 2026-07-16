# Configure accessibility policies

Use an accessibility policy when CSS must cross a deterministic CI gate for declared contrast,
motion, focus visibility, forced colors, and input modality. Start with warnings for evidence that
requires runtime or human review, then tighten a result class only after the project has a reviewed
baseline.

This guide configures static analysis. It does not produce or certify WCAG conformance.

## Create the policy

Create `config/pliego.accessibility.json`:

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

Every check and enforcement field is required. `fail` turns that result class into an error;
`warn` keeps it visible without failing the accessibility gate. Do not use `warn` to describe a
verified pass: passes are informational automatically.

## Run the first audit

Run from the application root so paths remain portable:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json
```

Literal-to-literal pairs without `selections` need no token graph. The result includes
`PCSS-A11Y-000`, one result for every declared pair, and observations for relevant CSS rules.
Compatibility and accessibility findings share one canonical document but keep distinct codes and
policy causes.

Use canonical JSON when reviewing evidence or creating exceptions:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json --format json
```

Interpret the verification field narrowly:

- `verified`: PliegoCSS proved a pass or violation inside the stated static boundary;
- `unverified`: the input is relevant, but static resolution was insufficient;
- `manual-required`: browser, DOM, interaction, or human evidence is necessary.

Only unexcepted non-pass findings configured as `fail` make the accessibility outcome fail.

## Evaluate token relationships

Use the complete canonical `pliego.tokens.json` emitted by a controlled compile, watch, or bundle
that belongs to the application being audited. Do not substitute an unrelated graph merely because
it has matching token names.

Add a token relationship:

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

Then supply the graph explicitly:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json \
  --token-graph dist/pliego.tokens.json --format json
```

Token names use `color.<kebab-name>`, matching the graph's flattened color projection. With no
`selections`, every graph theme is evaluated. With a selection map, the complete map must match one
theme exactly. A missing graph, token, or theme remains `unverified`; PliegoCSS does not fall back to
string matching CSS custom properties.

Choose `minimumRatioMilli` from the component's reviewed requirement. Common values are `4500` for
4.5:1, `3000` for 3:1, and `7000` for 7:1. PliegoCSS does not infer rendered text size or weight.

## Author statically recognizable CSS

For a verified schema-1 motion pass, place active motion inside the exact positive
`no-preference` query:

```css
@media (prefers-reduced-motion: no-preference) {
  .notice {
    transition: opacity 150ms ease;
  }
}
```

A separate `reduce` override is still a good authoring pattern, but schema 1 cannot prove its
cascade against every competing declaration. It therefore never uses a separate override to invent
a pass. Partial longhands, mixed shorthand/longhand, a competing same-file selector/family rule,
`or`, `not`, custom/unknown media, and contradictory nested preferences remain `manual-required`.

For a verified input-modality relation, put exact hover and focus alternatives in the same selector
list and declaration block:

```css
.button:hover,
.button:focus-visible {
  background: #dbeafe;
  outline: 2px solid #1f2937;
  outline-offset: 2px;
}
```

Separate rules, different stylesheets, unequal declarations, nesting, `@scope`, layers, and complex
functional selectors remain `manual-required`; the analyzer does not assume they share runtime
context. Avoid `outline:none` or zero outline values in focus selectors. Those are verified static
suppression requests. A positive outline declaration still requires manual/browser evidence because
schema 1 does not compute the complete cascade or prove the rendered indicator. If a design
intentionally replaces the native outline, review the rendered result rather than treating the
static finding as a global accessibility judgment.

`forced-color-adjust:none` is similarly conservative: it disables user-agent adaptation and is a
verified policy violation. If that is essential for a reviewed asset, attach evidence through a
specific exception and still test the rendered result in forced-colors mode.

## Review an exception

Run JSON output and copy the exact `context.subject-id` from the non-pass finding. Add one exception
for that check and subject:

```json
{
  "id": "reviewed-legacy-logo",
  "check": "forcedColors",
  "subjectId": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "justification": "Brand-owner review requires the fixed-color mark until asset replacement.",
  "expiresOn": "2027-03-31"
}
```

Replace the illustrative digest with the emitted value. A match changes the result to the check's
`PCSS-A11Y-x02` code, retains evidence and verification, attaches the review record, and stops only
that observation from failing. An exception that matches nothing emits `PCSS-A11Y-999`.

The engine validates that `expiresOn` is a real date but does not consult the current clock. Enforce
expiry review in CI or governance tooling; otherwise identical source and policy bytes would produce
different results on different days.

## Publish reproducible evidence

Create the control directory, then publish and check the exact group:

```console
mkdir -p reports/pliegocss
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json \
  --token-graph dist/pliego.tokens.json \
  --control-dir reports/pliegocss --format json

pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json \
  --token-graph dist/pliego.tokens.json \
  --control-dir reports/pliegocss --check
```

The policy and graph are integrity-bound inputs. A supplied graph is also published as
`pliego.tokens.json`, so the group contains the graph, findings, control manifest, and receipt.
`--check` recomputes exact bytes without writing.

For code-scanning systems, render the same finding document as SARIF:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json \
  --format sarif > reports/pliegocss.sarif
```

## Complete the manual evidence

Before calling a feature accessible, test at least:

- foreground/background pairing in the real DOM, including states and backgrounds;
- text size and weight against the selected contrast threshold;
- keyboard operation, focus order, focus visibility, clipping, and overlays;
- reduced-motion behavior after real user interactions;
- forced-colors output on the relevant browser/platform;
- pointer, keyboard, touch, and JavaScript interaction equivalence;
- zoom, reflow, labels, names, roles, and semantics outside CSS analysis.

See the complete [schema reference](../reference/accessibility-policy.md) and
[troubleshooting guide](../troubleshooting/accessibility-audit.md). The runnable repository example
is in [`examples/accessibility`](../../examples/accessibility/README.md).
