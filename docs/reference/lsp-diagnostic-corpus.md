# LSP diagnostic corpus schema 2

`pliegocss-lsp-diagnostic-corpus/2` freezes the complete typed utility-authoring diagnostic matrix.
The canonical fixture is
`integration-tests/lsp-diagnostics/corpus.json`; `pnpm integration:lsp` is its executable
consumer.

## Closed document

The root has exactly:

- `kind = "pliegocss-lsp-diagnostic-corpus"`;
- `schemaVersion = 2`;
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

Schema 2 contains twenty cases:

- every style/parser/compiler code from `PCS001` through `PCS012`;
- every macro scanner code from `PSC001` through `PSC006`;
- invalid Rust syntax `PCR001`;
- canonical-format drift `FMT001`.

For every `style` case, the gate first runs same-version
`pliego-cssc --diagnostic-format json check --style` and requires the frozen CLI schema-1 code,
message, and style byte range. Source cases use `check --source`; `FMT001` uses
`fmt --source --check`. The LSP session then applies each input as a new full-document version,
waits for that version's publication, rejects additional diagnostic codes, and requires exact CLI
code, message, severity, category, suggestion, typed replacement, and UTF-16 range projection.
The same session independently compares compiler-backed cross-clause `PCX003` over all those fields.

The corpus exposed three prior mismatches: parser failures used the complete Rust literal range in the
LSP even though the parser and CLI retained an exact `Diagnostic.span`; zero-width `PCS008` ranges
were rejected; and `syn` context before an embedded `PSC003` marker caused a `PSC002` fallback.
Schema 2 requires the exact span
to be projected when the literal has an exact decoded-to-source mapping; escaped cooked literals
still fail closed to the complete token.

## Boundary

This closes the current typed utility-authoring families: PCS, PSC, PCR001, FMT001, and PCX003.
`PCL001`/`PCL002` remain operational fallback paths for unavailable or invalid tools and bounded
editor limits, so the same session covers them through invalid-JSON and 256-literal fault injection
rather than authoring corpus inputs. Future codes must add corpus or fault-injection coverage before
this coverage claim can remain true. Adding a case is append-only inside schema 2 when its fields and
interpretation are unchanged. Changing the root shape, range meaning, or equality dimensions
requires a new schema version and migration note.
