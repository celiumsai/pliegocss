# Audit and budget example

Run from the repository root:

```console
cargo run -p pliego-cssc -- audit \
  --input examples/audit/app.css \
  --targets baseline-widely \
  --budget-policy examples/audit/pliego.budgets.json \
  --budget-subject package=example-app \
  --budget-subject route=/demo
```

The command is read-only. It observes the complete file automatically, accepts the explicit package
and route attestations, detects `layer:app.components`, and evaluates all matching definitions. The
mandatory compatibility partial-coverage warning does not fail the command; any unexcepted budget or
strict compatibility error does.

See [budget policy schema 1](../../docs/reference/budget-policy.md) before changing limits. The
example uses generous ceilings for clarity; production baselines must come from a reviewed exact
artifact rather than copied numbers.
