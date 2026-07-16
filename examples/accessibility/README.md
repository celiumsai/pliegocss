# Accessibility policy example

This fixture demonstrates all five schema-1 checks without requiring a token graph. Run it from the
repository root:

```console
cargo run -p pliego-cssc -- audit \
  --input examples/accessibility/app.css \
  --targets baseline-widely \
  --accessibility-policy examples/accessibility/pliego.accessibility.json
```

The policy declares one literal contrast relationship. The CSS scopes motion to the exact
`prefers-reduced-motion: no-preference` query, places hover and focus-visible in one declaration
rule, retains an explicit focus outline, and leaves forced-colors adaptation enabled. The audit
should emit verified pass findings for contrast, motion, forced colors, and input modality. Focus
visibility remains `manual-required`: static declarations do not prove the computed indicator or
its visibility. `PCSS-A11Y-000` summarizes only this bounded gate.

Use JSON to inspect exact evidence and stable subject identities:

```console
cargo run -p pliego-cssc -- audit \
  --input examples/accessibility/app.css \
  --targets baseline-widely \
  --accessibility-policy examples/accessibility/pliego.accessibility.json \
  --format json
```

The mandatory compatibility partial-coverage warning is independent of this accessibility result.
Neither a successful command nor `PCSS-A11Y-000` is WCAG certification. See the
[configuration guide](../../docs/how-to/configure-accessibility-policies.md) and
[schema reference](../../docs/reference/accessibility-policy.md) before adapting the policy.
