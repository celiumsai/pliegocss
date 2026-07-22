<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/tooling/repair/","category":"Tooling","eyebrow":"Repair agent","order":25}
-->

# Plan first. Authorize exact bytes.

Generate bounded repair proposals, bind authorization to source hashes, verify in staging, then publish receipts.

## Join findings to a proposal {#plan}

Repair plans name exact finding codes, files, byte ranges, replacements, expected hashes, and verification commands.

```console
pliego-cssc plan --findings findings.json \
  --proposal proposal.json --source-root .
```

## Verify before mutation {#dry-run}

Dry-run applies the authorized patch inside a staging copy and executes bounded Rust or browser checks without replacing the source tree.

```console
pliego-cssc fix --plan plan.json --findings findings.json \
  --source-root . --dry-run
```

## Time and output are part of safety {#resource}

Test, browser, toolchain, and version subprocesses use explicit deadlines and output limits while the repair lock is held.
