# Migration bridge contract corpus

Status: **three authored contract cases and a read-only CLI runner implemented; reviewed public
role evidence is tracked separately**

The tracked `integration-tests/migration-corpus/cases.json` manifest freezes three focused migration
bridge cases: a resolved Sass module edge, Tailwind-owned config/plugin/template auxiliaries, and a
CSS Modules consumer with static plus dynamic usage. Run the gate from the repository root:

```console
pnpm check:migration-corpus
```

The runner builds the locked `pliego-cssc` workspace binary, invokes
`migration-project-inventory` once per declaration, checks the selected summary values and required
dependency seams, requires clean stdout/stderr behavior, and hashes every fixture before and after
execution to prove that the read-only command did not mutate it. `PLIEGOCSS_BIN` can select an
already-built exact CLI binary.

The manifest classification is deliberately `authored-contract`. These cases prove deterministic
boundary behavior and prevent feature-specific fixtures from drifting independently; they are not
anonymized production applications, developer interviews, or evidence of migration precision and
recall. The separate [reviewed public role corpus](./migration-real-corpus.md) now supplies pinned
MIT-project provenance plus file-role precision/recall; migration outcomes and broader diagnostic
measurements remain open.

Current frozen expectations include:

- exact local Sass `@use` resolution;
- typed `@config`, `@plugin`, and exact-file `@source` resolution;
- literal and dynamic template candidate counts;
- closed Tailwind config-key and plugin-API seam counts;
- CSS Modules composition, ESM/CommonJS/TypeScript import forms, static class usage, and dynamic
  class usage, including one-level binding-alias propagation and one renamed destructured class;
- one TSX file inventoried independently as both a CSS Modules consumer and Tailwind template.

Adding a case requires a unique ID, a project declaration, explicit summary expectations, at least
one required dependency seam when dependencies are in scope, and the same authored-contract claim
boundary. Public-project evidence belongs under the separate `reviewed-public-role-corpus`
classification and must retain exact revision and license verification.
