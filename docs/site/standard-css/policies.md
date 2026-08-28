<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/standard-css/policies/","category":"Standard CSS","eyebrow":"Policies","order":14}
-->

# Make browser support and budgets executable.

Compatibility, accessibility, payload, package, route, and ownership policies turn expectations into reviewable inputs.

## Version the target {#compatibility}

baseline-widely and modern are frozen policy profiles with explicit browser and source-data identities. They do not drift silently with a dependency update.

```console
pliego-cssc compatibility --targets baseline-widely
```

## Bind budgets to owners {#budgets}

Direct files can name budget subjects manually. Asset Plans use an ownership sidecar to aggregate package and composed-route costs without guessing responsibility.

```console
pliego-cssc audit --asset-plan dist/pliego.assets.json \
  --ownership pliego.ownership.json --budget-policy budgets.json
```

## Audit static accessibility contracts {#accessibility}

Contrast and interaction guards use explicit policy plus token evidence. Dynamic visual behavior still requires browser and assistive-technology testing.
