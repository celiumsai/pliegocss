# pliego-cssc

`pliego-cssc migration-project-plan DECLARATION.json|DIRECTORY` emits a canonical inventory-bound,
read-only migration checkpoint. Schema 1 is deliberately `inventory-only`, reversible, and contains
zero edits; it does not claim a codemod or mutate the project.
