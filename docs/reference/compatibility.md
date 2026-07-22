# Compatibility candidate contract

Status: **machine-enforced prerelease vectors at `0.1.0-rc.2`; final `0.1.0` promotion remains open**

PliegoCSS already persists identities and artifacts across the macro, build-script bridge, CLI, SSR,
and CSS boundary. Those bytes cannot be allowed to drift accidentally while the broader 0.1 API is
still being designed. This page separates the format invariants enforced now from promises that only
begin when a release candidate is cut.

## Same-revision rule

All PliegoCSS crates participating in one build must use the same package version and source
revision. In particular, `pliego-css-macros`, `pliego-css-compiler`, `pliego-css-theme`,
`pliego-css-build`, and `pliego-cssc` are one compatibility unit. Mixing revisions can produce a
class identity, theme artifact, or manifest that another component does not understand.

Cargo manifests use local paths plus exact internal version requirements for this reason.
Packaging removes the paths and retains the exact requirements. Public-preview consumers
must pin every direct PliegoCSS dependency to exact version `0.1.0-rc.2`.

## Enforced vectors

| Surface | Version/vector | Enforced by |
|---|---|---|
| Representative StyleId vector | `STYLE_ID_FORMAT_VERSION = 2`; minimal-theme vector below | `v2_style_identity_and_class_are_frozen` |
| Minimal-theme style | `flex gap-gutter tablet:grid` → `b742ceb589d4f412c6ba77e81f632f53` | compiler unit test |
| Class-name encoding | `CLASS_NAME_FORMAT_VERSION = 1`; same style → `pc_aukxxmkm8bmauj8duf8zpjdcj` | IR/compiler unit tests |
| Full-width class boundary | `u128::MAX` → `pc_f5lxx1zz5pnorynqglhzmsp33` | IR unit test |
| Candidate seed ThemeId | `b98b78da29201938d135b8bb94717788` | seed/portability vectors |
| Candidate seed style | `flex gap-4` → `fe3a92576be2bb53e3240249bc45b829` / `pc_f1u1l7d58kemkdqdjie56hdex` | compiler unit test |
| Theme identity stream | `THEME_ID_FORMAT_VERSION = 3`; SHA-256 truncated to its first 128 bits; breakpoint cascade rank is explicit | `v3_theme_identity_stream_digest_and_id_are_frozen` |
| Theme binary | magic `PLGCTHM\0`, format 3; older formats rejected explicitly | theme unit test |
| Cargo theme bridge | legacy TOML `theme!(PATH)` plus DTCG `theme!(tokens = PATH, inputs = { "modifier" => "context" })`; one selected registry artifact per package | build unit tests, Debian WSL2 full-workspace/Rust 1.85 public-API smoke, Windows dirty-package extracted DTCG/TOML consumers, and clean package gate at `9714b09` |
| Semantic IR binary | magic `PLGCIR\0\0`, format 2; 309-byte golden SHA-256 `1d19d4bf7735acaf6b574d95fdb930389713c55983fff87c4c26a6cc2880532b`; append-only container, writing-mode, and cascade-layer extensions; embeds StyleId format 2 and ThemeId format 3 | compiler golden/round-trip tests |
| Minimal theme fixture | ID `eda25b5ed8fa8ce973662d4f18a46bdc` | theme unit test |
| Minimal theme binary | format 3, 104 bytes, SHA-256 `d2ce5bd919ba720e3b108fe1479e52c70b0441af708ddd5c7025e16fb2a0f4e0` | theme unit test |
| Theme configuration | schema 1 | config tests and reference |
| CSS manifest | schema 3 default; schema 4 semantic graph; schema 5 physical trace | CLI, manifest-graph, and physical-trace gates |
| Nested provenance graph | schema 1 semantic; schema 2 physical superset; declaration/physical ID formats 1 | graph/trace gates and references |
| Reachability sidecar | schema 1 | strict decoder and manifest-graph gate |
| Structured diagnostics | schema 1 | CLI tests and process smoke |
| Inspect document | schema 3 | CLI tests |
| Catalog document | schema 4 | CLI tests and generated-reference smoke |
| Explain document | schema 2 | CLI tests and editor-query smoke |
| Repair proposal / plan / dry-run report / Change Receipt | schemas `1.0.0` / `1.0.0` / `1.0.0` / `1.0.0`; plan mode `dry-run-only`; patch format `pliegocss-byte-edits/1`; receipt result `checks-pending` | agent unit tests, CLI black-box apply/rollback gate, public-API smoke, and extracted-package gate |
| Repair check policy / Verification Receipt | latest schemas `1.4.0` / `1.4.0`, canonical `1.0.0`/`1.1.0`/`1.2.0`/`1.3.0` read support under original kind limits; kinds `standard-css-audit|token-graph-integrity|css-budget-audit|test-suite-evidence|browser-evidence`; fixed test/browser evidence schemas `1.0.0`; results `passed|failed|blocked`; complete adjacent FindingDocuments | agent core/bin tests, fixed-runner/verify black-box gates, real Chromium replay, public-API smoke, and extracted-package gate |
| Declarative bundles | frozen plan schema 1 plus additive schema 2; per-bundle manifest schema 3 or explicit 4/5 | schema-1 bundle/graph/PliegoRS gates; schema-2 DTCG local gates green |
| PliegoRS source-surface contract | schema 2 | cross-repository integration gate |
| Portable bundle bytes | CSS/manifest hashes in the portability contract | three-OS × Rust 1.85/1.96 CI; every green run is exact-source evidence and cannot promote an older tag |
| Browser lowering configuration | Chrome/Edge 111, Firefox 128, Safari 16.4 | CLI target tests and reference |
| Compatibility policy | schema 2 / policy 7; frozen `baseline-widely` snapshot dated 2026-07-14, integrity-bound official data packages, plus bounded configurable attribute and typed direction/container/writing-mode/cascade-layer support | `pnpm check:compatibility`, policy golden, and ADR-0012/0013 |

The theme fixture is deliberately independent of the seed catalog, so adding a reviewed seed token
does not pretend that the binary encoding changed. The primary style-format fixture uses that same
minimal theme. A separate end-to-end seed vector exists because theme identity is part of `StyleId`;
a seed-registry change must review and intentionally update the seed-dependent candidate and
portability vectors before 0.1.0.

For identity guarantees, “identical inputs” means identical normalized semantic records under the
same `ThemeId` and identity-format version. Source spelling alone is not sufficient: changing the
active registry changes `ThemeId` and intentionally partitions all derived style identities.

StyleId format 2 encodes an explicit tagged and length-framed byte stream. Multibyte integers and
lengths are big-endian, text lengths count UTF-8 bytes, interned text is encoded by content, and
provenance spans, table insertion positions, and the previously derived ID are excluded. Assignment
records are sorted by their encoded bytes and exact duplicates are removed. SHA-256 is computed over
the complete stream; its first 16 digest bytes are interpreted as one big-endian `u128`. The
all-zero result is remapped to one because zero is reserved for unresolved IR.

This compact identity is deterministic. Because it retains only 128 digest bits, distinct streams
can map to the same number; the digest also cannot recover the stream and must not be used for
authentication or authorization. CLI aggregation compares canonical streams and fails if distinct
streams map to the same `StyleId`; the hash alone does not prove semantic equality. See the complete
[StyleId format-2 specification](./style-id-format-v2.md).

## Publication filesystem guarantee

Grouped CLI outputs and agent repair writes use a shared coordination primitive owned by
`pliego-css-build`. Lock leaves reject symlinks/reparse points, and temporary siblings are reserved
with `create_new` while retaining the created file identity. Existing outputs are staged through
collision-safe backup reservations.

The guarantee is **rollback-capable, durable when requested, not crash-atomic**. A failure observed
by the live process restores previous bytes and cleans temporary/backup siblings. Durable mode
synchronizes prepared files and, on Unix where directory handles support it, parent directories
after rename/removal. Abrupt process or machine loss can still expose an intermediate multi-file
rename state; no claim of cross-file crash atomicity is made.

Transaction migration is intentionally staged: CLI and agent now share hardened lock and temporary
reservation code while their established transaction/error adapters remain in place to preserve
coordination names and public diagnostics. Backup/rollback adapters will converge on the shared
transaction only after compatibility fixtures cover every existing public error projection; no
third transaction implementation should be introduced.

## Change rules before 0.1.0

- A failing vector is a compatibility decision, not snapshot churn. The change must explain why the
  old bytes are no longer correct and update this page in the same commit.
- Changing an existing canonical identity encoding requires a format bump. Append-only candidate
  tags or disjoint optional markers may remain format 2 only when all earlier streams/IDs remain
  byte-identical and the extension is frozen explicitly.
- Changing the base-36 class encoding requires incrementing `CLASS_NAME_FORMAT_VERSION`; it is not
  implicitly covered by the identity-stream version.
- Changing the theme binary layout requires a new binary format version. Decoders continue to reject
  unsupported versions loudly; silent reinterpretation is forbidden.
- Changing the semantic IR artifact header, record grammar, included provenance, canonical order, or
  decoder meaning requires a new IR binary format version. Embedded identity versions remain
  independently checked.
- Breaking a JSON/TOML document requires a new schema number. New consumers must not infer a new
  shape while reporting an old schema.
- Additive optional JSON fields in an existing schema are permitted only when that schema explicitly
  requires consumers to ignore unknown fields or values. Otherwise the addition requires a schema
  bump as well.
- Adding utilities or seed tokens may intentionally change catalog/theme-derived vectors before the
  release candidate, but never without explicit review and regenerated evidence.
- Changing CSS bytes without changing semantic identity is allowed only when the printer/target
  contract makes the reason explicit; integrity hashes and portability vectors must move together.

The JSON schema bumps associated with StyleId format 2 make identity versions self-describing at
the document boundary. Manifest schemas 3, 4, and 5, inspection schema 3, catalog schema 4, and
utility explain schema 2 require the top-level `styleIdFormatVersion`, `classNameFormatVersion`, and
`themeIdFormatVersion` fields. Diagnostic schema 1, theme configuration schema 1, theme identity and
binary format 3, semantic IR binary format 2, bundle-plan schemas 1/2, reachability schema 1, nested
graph schemas 1/2, cascade explain schema 1, repair proposal/plan/dry-run/Change Receipt schemas
1.0.0, check-policy/Verification Receipt schemas 1.4.0 with canonical
1.0.0/1.1.0/1.2.0/1.3.0 read support, fixed test/browser evidence schemas 1.0.0, patch format 1,
and the PliegoRS source-surface contract schema 2
remain independently versioned.

Bundle-plan schema 1 remains accepted with exactly its seed/config theme contract. Schema 2 retains
those meanings and adds `dtcg-resolver`, `path`, and optional `inputs`; it does not reinterpret a
schema-1 document or add a bundle CLI flag. Consumers must reject unknown plan schemas and must not
silently accept schema-2 theme fields while a document reports schema 1. Schema-2 Windows/WSL E2E,
workspace, Rust 1.85, rollback/read-only, and package gates pass locally.

The Cargo macro preserves `theme!(PATH)` and adds one DTCG Resolver form without changing the theme
binary format. Relative paths use `CARGO_MANIFEST_DIR`; empty inputs use defaults; input keys/values
are literals; and duplicate modifier entries fail closed even after case folding. Only the selected
registry enters the binary artifact. A controlled CLI producer must repeat the same Resolver and
inputs to generate matching CSS and publish the complete graph. Call `theme!` once per package.

## Candidate format-1 migration

StyleId format 2 is intentionally incompatible with the earlier format-1 candidate. There is no
conversion from an old hash to a new hash because the hash does not retain its semantic input.
Consumers moving an existing checkout must:

1. recompile procedural macros and every SSR/SSG producer;
2. regenerate CSS, manifests, catalog output, and every named bundle;
3. invalidate caches or indexes keyed by StyleId or `pc_*` class;
4. deploy markup, manifests, and CSS as one coordinated revision; and
5. reject mixed artifacts by checking all three reported format-version fields.

`CLASS_NAME_FORMAT_VERSION` remains 1 because `pc_` plus lowercase base 36 did not change. Every
previously generated class must nevertheless be treated as obsolete because the encoded StyleId
contract changed. ThemeId format 3 and theme-binary format 3 are also incompatible with older
candidates. Consumers must regenerate theme binaries and every ThemeId-derived artifact; the decoder
rejects an older theme binary with an explicit unsupported-version error rather than reinterpreting
a registry that has no explicit breakpoint cascade rank.

## Candidate SemVer policy

For subsequent release candidates and the eventual `0.1.x` line:

- patch releases must preserve existing IDs, classes, supported schema meanings, and theme binary
  decoding for identical inputs;
- additive utilities, tokens, diagnostics, or optional fields require tests showing old inputs remain
  valid;
- an identity or binary-format break requires a format/schema bump and migration notes, even though
  Cargo SemVer treats pre-1.0 minor releases specially;
- removing or reinterpreting a supported utility, token, field, or CLI contract requires a documented
  deprecation/migration path.
- Rust 1.85 remains the MSRV throughout `0.1.x`; raising it requires at least `0.2.0` and migration
  notes.

This policy governs the published `0.1.0-rc.2` prerelease. It does not claim the
separate final `0.1.0` promotion has occurred. The supported application/build
subset is selected and compile-checked through a registry-only Rust 1.85 consumer.

The intended RC surface includes `pc!`, `pcx!` (including Cargo facade dependency renames),
`Style`/`StyleId` class interoperability, the
`theme!` macro and the behavior of the build-script-generated theme artifact bridge, the documented one-shot CLI commands, and the numbered
configuration/diagnostic/manifest/bundle schemas. A breaking change to that supported 0.1 surface
requires at least a `0.2.0` release plus the relevant internal format/schema bump. Manifest schemas
4/5, graph schemas 1/2, physical ID formats 1, and reachability schema 1 are explicit candidate
contracts; automatic framework collection,
preload, LSP transport, hot reload, and APIs explicitly marked experimental are not silently
promoted into that scope.

The exact Rust signatures of the doc-hidden `configure_theme`, `write_theme_artifact`,
`ThemeArtifact`, and `BuildError` remain implementation details; interoperability of the generated
artifact does not freeze those helper types by implication. See the
[public API candidate](./public-api.md) for the exact selected subset.

## Still experimental

- crate boundaries and most public Rust types;
- full utility/catalog coverage and seed-token contents;
- CLI installation and registry publication;
- normal `compile`/`watch` provenance bytes across different hosts;
- automatic PliegoRS reachability collection, preload, browser resumability, and hot reload; the
  neutral explicit reachability input and schema-4/schema-5 graphs are implemented;
- multi-browser output certification beyond the frozen Lightning CSS target configuration;
- performance or payload guarantees outside the frozen benchmark fixtures;
- support for WASI, `no_std`, every WASM target, or deployed Cloudflare Workers.

Run the local candidate gates with:

```console
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
pnpm check:compatibility
pnpm check:api
pnpm check:manifest
pnpm check:pruning
pnpm check:portability
```

See the [bundle portability evidence](../status/portability.md) and the
[CLI schema reference](./cli.md) for their narrower boundaries. Browser, reset, scope, and
per-feature decisions are specified separately by
[compatibility policy schema 2](./compatibility-policy.md).
