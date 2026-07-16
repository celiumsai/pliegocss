# ADR-0007: Persist semantic IR through an explicit binary envelope

Status: accepted for the format-1 candidate

## Context

`SemanticStyle` is an internal Rust model shared by lockstep compiler crates. Its compact table IDs,
enum names, vector layout, and derived traits are implementation details. Applying Serde derives to
that model would turn those details into an accidental wire format, permit construction of invalid
states, and still leave the project without canonical bytes, defensive size limits, or theme-aware
identity verification.

The StyleId format-2 stream cannot substitute for persistence. It deliberately omits source spans
and the stored ID, sorts and deduplicates assignment records, and hashes the result. It proves
identity but cannot round-trip the provenance-bearing semantic input.

## Decision

- `pliego-css-compiler` owns a canonical semantic-IR binary envelope. The compiler is the lowest
  existing crate that can see the IR, the active theme, and StyleId derivation without introducing a
  dependency cycle or an eleventh package.
- Format 1 stores a magic header, explicit IR/StyleId/ThemeId format versions, the active ThemeId,
  the stored StyleId, a SHA-256 digest covering the full record payload including spans, and
  length-framed assignment records in big-endian form.
- Each assignment reuses the explicit semantic record grammar from StyleId format 2, resolving
  interned values, selectors, conditions, and arbitrary property names by content. The portable
  `Span` is stored beside that record and remains excluded from StyleId derivation.
- Records are sorted canonically by semantic bytes and span. Rust enum discriminants, `Debug`
  output, table insertion positions, and target-sized values never enter the artifact.
- The encoder accepts only the compiler's canonical cascade order. Decode restores that emitter
  order after reading wire identity order, so shorthand refinements cannot change meaning. Semantic
  duplicates and same-condition conflicts remain invalid even when their source spans differ.
- Decoding is an untrusted-input boundary: it applies hard artifact/count/text limits, rejects
  unknown tags, malformed UTF-8, invalid booleans, truncation, trailing data, unsafe arbitrary CSS,
  unknown theme references, noncanonical ordering, and identity mismatches.
- Successful decode must produce structurally valid IR, recompute the same StyleId under the supplied
  theme, and re-encode to exactly the input bytes.
- Changing any byte meaning requires a new IR binary format version and migration notes. Serde may
  later exist over a separate versioned DTO, but it will not define persisted bytes.

## Consequences

- IR artifacts remain deterministic across native and WASM targets without adding dependencies to
  `pliego-css-ir` or the procedural-macro graph.
- A cache or tool must provide the exact active theme and reject mixed format versions; a StyleId
  alone is not enough to authenticate or reconstruct IR.
- Source-span changes can change artifact bytes without changing StyleId. This is intentional:
  identity and provenance persistence are separate contracts. The payload digest still detects
  accidental span corruption, but is not an authentication mechanism.
- New utilities, values, states, or condition dimensions require an explicit tag review. Exhaustive
  tag tests and a frozen golden vector make accidental drift fail in CI.
- CSS text is validated for an external stylesheet. The codec is not an HTML-context encoder and
  decoded CSS must not be concatenated directly into a `<style>` element. The emitter revalidates
  manually assembled IR and composes the one leading selector anchor in linear time.
- The compiler package grows, so the package-size budget remains an executable release gate.
