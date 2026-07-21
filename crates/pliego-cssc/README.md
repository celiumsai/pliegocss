<!-- SPDX-License-Identifier: Apache-2.0 -->

# pliego-cssc

`pliego-cssc` is the command-line surface for PliegoCSS. It audits standard
CSS, compiles typed utility literals, transforms CSS, explains cascade and
physical lineage, inventories migration candidates, and verifies
deterministic artifacts.

The crate remains an unpublished `0.1.0-rc.1` candidate. Install from the
repository tag:

```console
cargo install --git https://github.com/celiumsai/pliegocss \
  --tag v0.1.0-rc.1 pliego-cssc
```

## Discover commands

```console
pliego-cssc --help
pliego-cssc compile --help
pliego-cssc audit --help
pliego-cssc migration-project-plan --help
```

Representative workflows:

```console
pliego-cssc audit --input app.css --targets baseline-widely --format human
pliego-cssc transform-css --input app.css --output dist/app.css --targets modern
pliego-cssc compile --source src --seed --theme --output dist/app.css
pliego-cssc explain --style "hover:bg-accent/50 -mt-4" --seed --format json
```

`migration-project-plan DECLARATION.json|DIRECTORY` emits a canonical,
inventory-bound, read-only migration checkpoint. Schema 1 is deliberately
`inventory-only`, reversible, and contains zero edits; it does not claim a
codemod or mutate the project.

The complete command and artifact contract is in the
[CLI reference](../../docs/reference/cli.md). Release, security, and support
status are controlled by the repository-level documentation.
