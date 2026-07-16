# CLI diagnostic JSON schema

Status: implemented schema 1

This is the legacy one-shot process-failure envelope. It remains supported, but it is not the richer
R0 audit contract. New policy/audit work uses [canonical finding schema 1.0.0](./finding-schema-1.md),
which adds causes, evidence, verification boundaries, ranked suggestions, exceptions, fingerprints,
and a single semantic source for human/JSON/SARIF audit output.

`pliego-cssc --diagnostic-format json <command> ...` exposes failures without requiring an editor,
CI job, or wrapper to parse terminal prose. The option may also follow the command or use the inline
form `--diagnostic-format=json`.

## Process contract

- Success keeps each command's normal standard output and exit status 0.
- Failure writes exactly one compact JSON document plus a newline to standard error, leaves standard
  output empty, and exits with status 1.
- The human `error: ...` and usage rendering is never appended to JSON.
- Diagnostics are deterministic: input categories retain the CLI pipeline order, source files use
  canonical lexical path order, and findings inside one source use byte order. A single failure
  still uses the `diagnostics` array.
- `watch` rejects JSON mode before polling. A future streaming mode requires its own NDJSON/event
  schema instead of pretending one final document can describe a long-lived process.

## Envelope

```json
{
  "schemaVersion": 1,
  "command": "check",
  "diagnostics": [
    {
      "code": "PCS001",
      "category": "style",
      "severity": "error",
      "message": "unknown utility `flec`",
      "suggestion": "flex",
      "origin": {
        "kind": "cli",
        "label": "explicit-style-1"
      },
      "range": null,
      "styleRange": {
        "byteStart": 0,
        "byteEnd": 4
      },
      "replacement": null
    }
  ]
}
```

`command` retains the raw command token (`build` remains `build`, and an unknown token remains
visible); it is null only when no command could be recovered. Every diagnostic object contains every
documented field; unavailable values are JSON null. Adding, removing, renaming, or changing the type
or meaning of a required field requires a schema-version bump.

## Diagnostic fields

| Field | Contract |
|---|---|
| `code` | Stable PCS001-PCS012, PSC001-PSC006, PCX003, FMT001, PCL001, or PCL002 identifier. |
| `category` | `style`, `source`, `composition`, `format`, `invocation`, or `tool`. |
| `severity` | `error` in schema 1. |
| `message` | Human-readable explanation without a duplicated code or usage block. |
| `suggestion` | Optional guidance or canonical style spelling; not automatically a source edit. |
| `origin` | Optional `{kind,label}` describing the input surface and reason. Repeated CLI styles/compositions use one-based indexed labels; composition errors also identify `base` or `branch-N`. |
| `range` | Optional file-backed outer range. |
| `styleRange` | Optional half-open UTF-8 byte range inside the decoded utility string. |
| `replacement` | Optional typed replacement value. |

`range` has this exact shape:

```json
{
  "file": "src/view.rs",
  "byteStart": 120,
  "byteEnd": 138,
  "startLine": 8,
  "startColumn": 19,
  "endLine": 8,
  "endColumn": 37
}
```

Byte ranges are zero-based and half-open. Lines and columns are one-based when known; columns count
Unicode scalar values, not UTF-8 bytes or UTF-16 code units. An LSP transport must therefore convert
or negotiate its position encoding explicitly. Line/column fields may be null when a later
compilation stage only retained exact file bytes. Scanner and formatter findings retain both byte
and line/column coordinates.

`range` and `styleRange` always use independent coordinate spaces and must never be added together.
For Rust literals, `range` identifies a safe enclosing source token or macro range while
`styleRange` refers to decoded string bytes. Escape sequences and raw-string delimiters prevent a
general offset translation; leading whitespace in line-oriented inputs also means the two origins
need not share an anchor. PliegoCSS deliberately does not invent a mapping it cannot prove.

FMT001 uses a typed replacement:

```json
{
  "kind": "decoded-style-value",
  "value": "flex gap-4"
}
```

The value is canonical decoded literal content, not a ready-to-splice Rust token. A future opt-in fix
engine must preserve the literal's raw/escaped representation or re-encode it collision-safely.

## Fallback codes

- `PCL001` means invalid CLI invocation, including an unknown option or unsupported JSON watch mode.
- `PCL002` means an I/O, configuration, emission, publication, or other tool failure that does not
  yet expose a narrower typed code.

These fallbacks let clients recognize a generic failure without parsing terminal prose. PCL002 does
not distinguish I/O from configuration, emission, or publication; doing that by inspecting
`message` is unsupported. New typed families can replace PCL002 cases in later schema-compatible
releases.
