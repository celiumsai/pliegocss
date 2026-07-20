# Reversible migration plan schema 1

Status: **inventory-bound checkpoint implemented; automatic edits deliberately absent**

`pliego-cssc migration-project-plan DECLARATION.json|DIRECTORY` emits canonical two-space JSON with one trailing LF. The command performs the same bounded discovery and two-pass inventory as `migration-project-inventory`, writes only stdout, and does not modify project files.

Schema 1 contains:

- `schemaVersion: 1`;
- `mode: "inventory-only"`;
- `reversible: true`;
- `inventorySha256`, computed over canonical `(file, sourceSha256)` pairs;
- canonically sorted source identities and exact byte counts;
- `edits: []` (still never auto-executed by planning);
- one explicit additive `proposals` entry for `pliego.migration.css`, including exact after-bytes
  SHA-256 and `automatic: false`;
- one exact `replace` proposal for the first canonical migration source, with before/after SHA-256
  and `automatic: false`; the after hash binds authored bytes plus the non-semantic preparation
  marker, while the executable `edits` array remains empty;
- one `theme-root-projection` proposal for a single unmodified static `@theme` block; it copies only
  custom-property declarations into an additive `:root` block, is `automatic: false`, and has
  Chrome/Edge computed-style evidence for the representative OKLCH token;
- one `utility-projection` proposal for a single static class-safe `@utility`; wildcard, `--value()`,
  nested, and dynamic utilities fail closed. Chrome/Edge prove `content-visibility: auto` and
  `contain-intrinsic-size: auto 1000px` on the projected class;
- bounded static template aliases may be grouped with their CSS replacement. Group apply compensates
  prior files on failure; group rollback preflights every after hash before restoring in reverse
  order. Chrome/Edge prove the source and aliased element have identical computed style;
- grouped filesystem I/O uses the shared bounded/no-follow and durable-publication contract:
  create-new synced temporaries, rename-over-destination, Unix parent-directory sync, Windows
  reparse-point rejection, complete before/after preflight, and bidirectional compensation;
- rollback strategy `restore-exact-source-bytes`;
- preconditions `source-hashes-match` and `all-edits-have-before-bytes`.

The plan refuses inventories that do not exactly cover the declared source set by path and source kind. Consumers and auxiliaries remain present in the underlying project inventory but are not edit targets in this first slice.

The separate `migration-sidecar-apply` and `migration-sidecar-rollback` commands exercise that one
proposal only when explicitly invoked. Apply refuses existing output/receipt paths and publishes a
receipt bound to exact output bytes. Rollback refuses drift and removes the receipt only after the
sidecar is safely removed. This is still not a Tailwind codemod: authored sources and templates stay
untouched, and the plan's executable edit set remains empty.
