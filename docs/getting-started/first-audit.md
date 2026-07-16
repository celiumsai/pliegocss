# First standard-CSS audit

Use this path when you already have CSS and do not want to adopt PliegoCSS utility syntax. The audit
is read-only: it does not rewrite CSS, create a configuration, or require PliegoRS.

## 1. Build the current CLI

From the PliegoCSS checkout:

```console
cargo build --locked -p pliego-cssc
```

Until registry publication, the checkout revision and `Cargo.lock` are the executable identity.

## 2. Audit one ordinary CSS file

Run from the project containing the CSS so `--input` remains a portable project-relative path:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely
```

Choose the target contract explicitly:

- `baseline-widely` fails closed when a recognized feature was not Widely Available in the frozen
  snapshot and when parsed at-rule syntax is unclassified;
- `modern` compares classified features with its fixed browser vector and warns on unclassified
  parsed syntax;
- `none` records that compatibility is unmanaged instead of implying a browser guarantee.

The first successful output includes `PCSS-AUDIT-000`, a mandatory `PCSS-COMPAT-001` coverage
warning, and any rule-feature decisions found in the file. The warning means the current classifier
does not yet cover every declaration value or selector feature; success is not a complete browser or
WCAG certification.

Use canonical JSON for agents or SARIF 2.1.0 for CI/code-scanning consumers:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely --format json > audit.json
pliego-cssc audit --input src/app.css --targets baseline-widely --format sarif > audit.sarif
```

Syntax and enforced policy errors produce the same finding document and a nonzero exit status. Tool
or invocation failures use the separate diagnostic channel on standard error.

## 3. Publish a verifiable audit group

Create one output directory and ask CSS audit to publish its canonical evidence group:

```console
mkdir -p reports/pliegocss
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --control-dir reports/pliegocss
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --control-dir reports/pliegocss --check
```

The directory contains canonical findings, `pliego.css.manifest.json`, and
`pliego.css.receipt.json`. The receipt hashes the exact manifest and finding output. `--check`
detects drift without rewriting files. A syntax failure still writes an inspectable failed receipt;
missing rule measurements and an omitted typed token graph are marked `unavailable` with reasons.
When a later audit supplies `--token-graph`, the same directory also receives fixed
`pliego.tokens.json`; the graph-bearing group has four artifacts and `--check` covers all four.

The same flags work with `--asset-plan`. That mode integrity-binds the plan and every adjacent CSS
and style manifest after canonical regeneration; its rule totals aggregate all declared bundles.

## 4. Add a bounded accessibility policy

Start with the [executable example](../../examples/accessibility/pliego.accessibility.json), or
create `config/pliego.accessibility.json`. Schema 1 requires all five checks and all three
enforcement decisions even when `contrastPairs` is empty:

```json
{
  "schemaVersion": 1,
  "policyVersion": 1,
  "checks": {
    "contrast": { "violation": "fail", "unverified": "warn", "manualRequired": "warn" },
    "motion": { "violation": "fail", "unverified": "warn", "manualRequired": "warn" },
    "focusVisibility": { "violation": "fail", "unverified": "warn", "manualRequired": "warn" },
    "forcedColors": { "violation": "fail", "unverified": "warn", "manualRequired": "warn" },
    "inputModality": { "violation": "fail", "unverified": "warn", "manualRequired": "warn" }
  },
  "contrastPairs": [
    {
      "id": "body-on-surface",
      "foreground": { "literal": { "value": "#1f2937" } },
      "background": { "literal": { "value": "#ffffff" } },
      "minimumRatioMilli": 4500
    }
  ],
  "exceptions": []
}
```

Run the same direct audit with the explicit policy:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json --format json
```

Literal-to-literal contrast without `selections` needs no token graph. If either endpoint uses a
`color.<kebab-name>` token or the pair selects a resolver theme, supply the canonical graph:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json \
  --token-graph dist/pliego.tokens.json --control-dir reports/pliegocss
```

`--token-graph` requires `--accessibility-policy`. The policy bytes enter the control config ledger
as `accessibility-policy`; graph bytes use `token-graph`. With the graph, token observation is
measured and `coverageBasisPoints` is `0` because standard-CSS audit has no compiler-owned typed-use
set. That zero is not `unavailable` and is not evidence that the application uses no tokens.
`contrastPairs` counts declarations in this policy, not per-theme evaluations.

The same policy works over every integrity-checked stylesheet in an Asset Plan:

```console
pliego-cssc audit --asset-plan dist/assets/pliego.assets.json --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json --format json
```

Read each finding's `verification` and exact `context.subject-id`. `verified` proves only the stated
static result; `unverified` preserves missing evidence; `manual-required` identifies browser, DOM,
interaction, or human work. A passing `PCSS-A11Y-000` summary is not WCAG conformance. Continue with
the [accessibility policy reference](../reference/accessibility-policy.md) before adding reviewed
exceptions.

## 5. Add a first budget

Copy [the executable example policy](../../examples/audit/pliego.budgets.json) or create a smaller
`pliego.budgets.json`:

```json
{
  "schemaVersion": 1,
  "policyVersion": 1,
  "budgets": [
    {
      "id": "app-css",
      "subject": { "kind": "file", "id": "src/app.css" },
      "limits": {
        "bytes": { "maximum": 20000, "baseline": 18000, "maxIncrease": 500 },
        "specificity": {
          "maximum": "1,3,1",
          "baseline": "0,3,1",
          "maxIncrease": "0,1,0"
        },
        "semanticDuplicates": { "maximum": 4, "baseline": 2, "maxIncrease": 1 }
      }
    }
  ],
  "exceptions": []
}
```

Use measurements from a reviewed exact artifact for `baseline`; do not copy the example numbers.
Then run:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --budget-policy pliego.budgets.json
```

The file subject is automatic. For an exact artifact known to belong to a package or route, add
explicit attestations:

```console
pliego-cssc audit --input dist/home.css --targets baseline-widely \
  --budget-policy pliego.budgets.json \
  --budget-subject package=storefront \
  --budget-subject route=/home
```

Only make the route assertion when the input is the complete route artifact.

For a PliegoCSS multi-bundle build, use the compiler-produced Asset Plan for file/layer subjects. A
policy containing package or route subjects also needs the explicit, exact-bound ownership sidecar:

```console
pliego-cssc audit --asset-plan dist/assets/pliego.assets.json --targets baseline-widely \
  --ownership config/pliego.ownership.json \
  --budget-policy pliego.budgets.json --format json
```

The command verifies every adjacent CSS/manifest pair by regenerating the plan, audits each CSS
file, assigns every bundle to its unique package, and aggregates each route as the stable deduplicated
union of its base bundles and all bundles for its declared islands. The sidecar is never discovered,
and manual `--budget-subject` values are rejected in Asset Plan mode. Routes can overlap and are not
Control Manifest partitions. See [configure ownership](../how-to/configure-ownership.md).

## 6. Read a budget result

- `PCSS-BUDGET-100`: current and regression limits pass.
- `PCSS-BUDGET-101`: an absolute or delta boundary fails.
- `PCSS-BUDGET-102`: the boundary still failed, but an explicit reviewed exception prevents gate
  failure.
- `PCSS-BUDGET-198`: an Asset Plan bundle failed syntax ingestion, so aggregate budgets were not
  partially evaluated.
- `PCSS-BUDGET-199`: at least one declared budget definition matched no verified observed subject.
  Coverage is total, so one matched definition plus one stale or misspelled definition still fails.

Each result records the policy hash, subject, metric, current value, maximum, optional baseline,
signed delta, permitted increase, and exact source span. Never interpret `PCSS-BUDGET-102` as “under
budget”; its exception and measured overage remain visible.

For `PCSS-BUDGET-199`, compare its `declared-budgets`, `matched-budgets`, and `observed-subjects`
evidence. Fix every unmatched definition; do not delete the finding while leaving partial policy
coverage.

Next read the complete [audit contract](../reference/audit-command.md) and
[budget policy schema 1](../reference/budget-policy.md), plus
[accessibility policy schema 1](../reference/accessibility-policy.md) when that gate is enabled.
