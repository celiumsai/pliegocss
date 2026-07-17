# LSP diagnostic corpus schema 1

`pliegocss-lsp-diagnostic-corpus/1` freezes representative negative editor cases before the
complete CLI/LSP equality matrix. The canonical fixture is
`integration-tests/lsp-diagnostics/corpus.json`; `pnpm integration:lsp` is its executable
consumer.

## Closed document

The root has exactly:

- `kind = "pliegocss-lsp-diagnostic-corpus"`;
- `schemaVersion = 1`;
- ordered `cases`.

Every case has a portable lowercase `id`, one `kind`, an exact diagnostic `code` and
`message`, and one half-open UTF-8 byte range. Unknown kinds or unsupported schema versions fail
the gate.

| Case kind | Input | Frozen range |
|---|---|---|
| `style` | Decoded `pc!` literal value | `styleStart..styleEnd` inside the decoded value |
| `source` | Complete in-memory Rust source | `sourceStart..sourceEnd` inside the source |
| `format` | Complete in-memory Rust source | Complete non-canonical Rust literal token |

## Equality gate

Schema 1 contains seven cases:

- unknown utility `PCS001`;
- incomplete variant `PCS002`;
- wrong token domain `PCS004`;
- same-condition conflict `PCS005`;
- duplicate variant `PCS012`;
- malformed `pcx!` base `PSC002`;
- canonical-format drift `FMT001`.

For every `style` case, the gate first runs same-version
`pliego-cssc --diagnostic-format json check --style` and requires the frozen schema-1 code,
message, and style byte range. The `source` case uses `check --source` and requires the same
source range. The LSP session then applies each input as a new full-document version, waits for that
version's publication, rejects additional diagnostic codes, and requires the same code/message plus
the exact UTF-16 projection. `FMT001` is compared to the shared parser/formatter contract because
it is an editor drift finding rather than a semantic compiler failure.

The corpus exposed one prior mismatch: parser failures used the complete Rust literal range in the
LSP even though the parser and CLI retained an exact `Diagnostic.span`. Schema 1 requires that span
to be projected when the literal has an exact decoded-to-source mapping; escaped cooked literals
still fail closed to the complete token.

## Boundary

This is a bounded representative corpus, not proof that every current or future diagnostic code,
severity, suggestion, replacement, scanner failure, or configuration error is equivalent. Adding a
case is append-only inside schema 1 when its fields and interpretation are unchanged. Changing the
root shape, range meaning, or equality dimensions requires a new schema version and migration note.
