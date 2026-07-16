# Accessibility audit troubleshooting

Accessibility findings are deliberately conservative. Begin with canonical JSON so the code,
verification state, `subject-id`, evidence, and policy cause are visible together:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json --format json
```

The accessibility gate describes a bounded static analysis. Passing it does not certify WCAG
conformance, and a warning does not automatically mean the CSS is inaccessible.

## The policy reports a missing field

Schema 1 requires all five entries under `checks`:

- `contrast`;
- `motion`;
- `focusVisibility`;
- `forcedColors`;
- `inputModality`.

Each requires `violation`, `unverified`, and `manualRequired`, each set to `fail` or `warn`. Unknown
fields also fail because the policy is closed. Keep `schemaVersion` and `policyVersion` at `1`.

## The policy is too large or has too many records

The exact limits are:

- 1 MiB for the complete policy input;
- 256 contrast pairs;
- 4,096 exceptions;
- 64 selection entries on one pair;
- `minimumRatioMilli` from 1,000 through 21,000.

Split organizational ownership outside the evaluator rather than bypassing a limit. One audit still
accepts only one unambiguous policy.

## `--token-graph` is rejected

`--token-graph` requires `--accessibility-policy`. The graph is evidence for declared relationships,
not an independent audit mode:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json \
  --token-graph dist/pliego.tokens.json
```

The graph must be canonical `pliegocss-token-graph/1` bytes. Use the artifact produced by the same
application/build boundary being audited.

## A token contrast pair is unverified

Check all of the following:

- the graph was supplied;
- the endpoint uses the externally tagged form `{"token":{"name":"color.name"}}`;
- the name uses one flattened kebab name after `color.`;
- the graph contains that projected color in the selected theme;
- `selections`, when present, equals a complete graph theme selection map.

PliegoCSS does not infer token identity from `--custom-property` spelling in arbitrary CSS. Missing
typed evidence remains unverified instead of silently using a similarly named value.

## A literal color is unverified or manual-required

Verified contrast requires two statically resolved, fully opaque, in-gamut sRGB colors. Runtime
values such as `var()`, `env()`, `attr()`, `currentColor`, inherited/reverted values, system colors,
or `light-dark()` need runtime context. Alpha below one, unresolved channels, and out-of-sRGB values
also cannot be reported as verified passes.

Resolve the value into the policy only when the literal is truly the application contract. Otherwise
keep the non-pass result and test actual composition in the browser. Do not replace dynamic evidence
with an unrelated convenient literal.

## Motion has no recognized safe guard

Schema 1 emits a verified motion pass only when the active declaration itself is inside the exact
positive `prefers-reduced-motion: no-preference` branch:

```css
@media (prefers-reduced-motion: no-preference) {
  .panel {
    transition: transform 180ms ease;
  }
}
```

An active declaration under `reduce` is a violation. Partial longhands, shorthand mixed with
longhands, a competing same-file selector/family rule, a separate `reduce` override, negated or `or`
query, nested contradiction, dynamic value, or cross-context relation cannot prove cascade and
therefore never creates a pass. Keep valid defensive overrides, but review the resulting
`manual-required` evidence. Complex application behavior still needs interaction testing.

## Focus suppression is intentional

`outline:none`, `outline-style:none`, and zero outline width in a `:focus` or `:focus-visible`
selector are verified suppression. The analyzer does not treat a box shadow, background change, or
other declaration as proven visible replacement because layout, clipping, contrast, and browser
rendering are outside its boundary.

Prefer retaining an explicit outline. A positive declaration still remains `manual-required`
because schema 1 does not prove computed cascade or rendered visibility. If a reviewed design must
suppress it, test the replacement across states and use a narrowly scoped exception tied to the
emitted `subject-id`. Do not disable the whole focus check.

## `forced-color-adjust:none` is reported

`forced-color-adjust:none` opts an element out of user-agent forced-colors adaptation, so it is a
verified policy violation. Other static values pass this narrow check; dynamic values require manual
verification.

For a logo or color-critical asset, verify the rendered result in forced-colors mode and record a
specific exception if the opt-out is intentional. That exception records review; it is not proof
that every platform renders the asset accessibly.

## A hover selector has no focus equivalent

Provide hover and focus alternatives in the same selector list so they share one exact declaration
block and context:

```css
.link:hover,
.link:focus-visible {
  text-decoration-thickness: 2px;
}
```

Separate rules/files, unequal declarations, nesting, scope/layer boundaries, and selectors
containing `:is()`, `:where()`, `:has()`, or `:not()` become `manual-required`. A static pass still
does not prove keyboard semantics, JavaScript handler equivalence, touch behavior, or target size.

## An exception does not match

Run the current policy without the proposed exception, render JSON, and copy
`context.subject-id` exactly. The exception's `check` must also match the observation. Only one
exception may cover one check/subject pair.

An unmatched exception produces `PCSS-A11Y-999`. This warns because stale review metadata should be
removed, but it does not fail the gate by itself.

Subject IDs intentionally exclude byte offsets and rendered severity, but include the semantic
claim and result class: relevant declaration values/priority, CSS context, and relational evidence;
or contrast endpoints, threshold, theme, resolved/error evidence, ratio, pair ID, and exact
selection map. Renaming a selector, moving the logical file, resolving previously missing evidence,
or changing any reviewed value therefore requires a new review.

## `expiresOn` is in the past but still applies

This is intentional in schema 1. The evaluator validates a real `YYYY-MM-DD` date but has no wall
clock, so identical input bytes remain deterministic on every day and machine. Enforce expiry using
CI/governance automation. Removing or renewing the exception changes the integrity-bound policy and
therefore the audit evidence.

## The gate passes but the page is not accessible

`PCSS-A11Y-000` summarizes only configured static checks. The analyzer cannot prove DOM pairing,
semantics, accessible names, focus order, clipping, overlays, keyboard operation, zoom/reflow,
background composition, actual forced-colors rendering, or runtime interaction behavior.

Continue with browser automation, assistive-technology review, and manual testing against the
applicable WCAG requirements. See the [policy reference](../reference/accessibility-policy.md) for
the exact non-claims and [finding schema](../reference/finding-schema-1.md) for verification
semantics.
