# Declarative bundle plan

Status: schema 1 remains the frozen seed/config contract; schema 2 adds an explicit DTCG Resolver
theme to the same one-shot bundle boundary. Both are implemented and locally verified; no new
bundle CLI flag is introduced.

`pliego-cssc bundle` compiles several named CSS artifacts from one declarative plan:

```console
pliego-cssc bundle --plan pliego.bundles.toml --output-dir dist/assets
```

The command is intentionally a manual partitioning boundary. It makes an application-owned bundle
map reproducible, but it does not infer Cargo modules, PliegoRS components, routes, or islands.

## Plan schemas

Schema 1 remains supported without reinterpretation. It accepts only `seed` and `config` theme
kinds. Schema 2 retains both kinds and adds `dtcg-resolver` plus its optional context inputs:

| Plan schema | `seed` | `config` | `dtcg-resolver` |
|---|---|---|---|
| 1 | no `path` or `inputs` | `path` required; no `inputs` | rejected |
| 2 | no `path` or `inputs` | `path` required; no `inputs` | `path` required; `inputs` optional |

### Schema 1

```toml
schema = 1
targets = "modern"
format = "minified"

[theme]
kind = "seed"

[bundles.global]
sources = ["src/styles/global.rs"]
emit-theme = true

[bundles.home]
sources = ["src/styles/home.rs"]
emit-theme = false

[bundles.visit]
sources = ["src/styles/visit.rs", "src/components/visit"]
emit-theme = false
```

The top-level fields are:

| Field | Contract |
|---|---|
| `schema` | Required integer. Accepted values are the frozen schema 1 and additive schema 2. |
| `targets` | Required `baseline-widely`, `modern`, or `none` compatibility profile, shared by every bundle. |
| `format` | Required `minified` or `pretty` printer contract, shared by every bundle. |
| `theme` | Required registry selection, shared by every bundle. |
| `bundles` | One or more named bundle tables. |

Bundle names use lowercase kebab case. A bundle named `account-settings` produces exactly:

- `account-settings.css`;
- `account-settings.manifest.json`.

When the CLI's boolean `--asset-plan` option is enabled, the command also produces the one fixed
group-level file `OUTPUT_DIR/pliego.assets.json`. The bundle plan itself has no output-path field for
that document.

When `--project-index` is enabled, the command additionally produces fixed
`OUTPUT_DIR/pliego.index.json`. It requires `--asset-plan`, manifest schema 5, and reachability. The
bundle plan has no output-path field for this artifact either.

When `--control` is enabled, the command additionally produces the fixed
`OUTPUT_DIR/pliego.css.findings.json`, `OUTPUT_DIR/pliego.css.manifest.json`, and
`OUTPUT_DIR/pliego.css.receipt.json` files. It requires `--asset-plan`; the Project Index remains
optional. These paths are also CLI-owned rather than configurable plan fields.

Each bundle has a non-empty `sources` array of Rust files or directories. Directory traversal uses
the normal source-scanner rules: deterministic lexical expansion, `.rs` files only, no symlink
following, and omission of hidden, `.git`, and `target` directories. `emit-theme` decides whether the
bundle prepends the active registry's supported custom properties. A typical application enables it
only for its global bundle. Every bundle must discover at least one `pc!` or reachable `pcx!` style
before optional pruning; schema 1 intentionally rejects source sets that are initially empty or
theme-only, and schema 2 preserves that rule.

All paths written inside the plan are relative to the directory containing the plan, not to the
process working directory. This includes bundle sources and a configured theme or Resolver. `--output-dir` is a
CLI destination and must name an existing directory; the command does not create it. Plan paths must
be non-empty normal relative paths: absolute paths and `.` or `..` components are rejected. The plan
file, configured theme/Resolver file, explicit source roots, and output directory cannot be symbolic links.
Existing derived CSS, manifest, asset-plan, project-index, or control-artifact destinations cannot
be symbolic links either. Windows reparse points, including directory junctions, are treated as
links for this boundary.

### Schema 2 DTCG Resolver theme

Schema 2 selects one bounded DTCG Resolver document inside the plan; `bundle` does not gain
`--tokens` or `--token-input` flags:

```toml
schema = 2
targets = "modern"
format = "minified"

[theme]
kind = "dtcg-resolver"
path = "product.resolver.json"

[theme.inputs]
appearance = "dark"
channel = "light"

[bundles.application]
sources = ["src/styles.rs"]
emit-theme = true
```

`path` is resolved relative to the plan with the same regular-file, containment, and link defenses
as a configured theme. The plan and exact Resolver are each limited to 16 MiB and opened with
no-follow/reparse-safe flags before their opened handle is validated as a regular file. The Resolver is parsed once, validated across
all of its permitted context permutations, and shared by every declared bundle. Omitting `inputs`
uses the Resolver defaults. Supplied modifier and context names are resolved case-insensitively;
unknown, missing, duplicate-after-case-fold, or invalid selections fail closed through the DTCG
Resolver profile.

The resolved active registry drives CSS and ThemeId identity. A controlled bundle publishes the
Resolver's complete canonical graph, including every validated permutation, rather than projecting
only the selected registry. The canonical resolved selections identify the active graph theme.

Two textually different plans are different input snapshots even when they select the same
canonical contexts, ThemeId, and CSS. Bundle `configHash` binds the exact plan bytes; unlike the
direct CLI flag surface, bundle plans do not promise semantic convergence across different TOML
spellings or layouts.

### Seed theme

```toml
[theme]
kind = "seed"
```

This selects `ThemeRegistry::seed()` for every bundle under schema 1 or 2. `path` and `inputs` are
rejected.

### Configured theme

```toml
[theme]
kind = "config"
path = "pliego.theme.toml"
```

`path` is required for `config` under schema 1 or 2 and is resolved relative to the plan. `inputs`
is rejected. The configuration is parsed once and the resulting registry identity is shared by
every bundle. Plan mode does not perform conventional theme discovery and does not accept a
different registry per bundle.

## Compilation contract

The command reads the plan, resolved theme configuration or Resolver, and every unique expanded
source exactly once into an in-memory input snapshot. It does not freeze the filesystem against an
unrelated writer
while that capture is in progress, but it never rereads a captured input during compilation. A source
file referenced by more than one bundle is allowed. Each
bundle compiles its own declared source set independently, so shared styles appear in every bundle
that declares them; there is no cross-bundle extraction or deduplication.

Every bundle runs the normal source scan, semantic compilation, deterministic canonical
identity-stream ordering,
optional theme emission, and Lightning CSS target/format pipeline. Each `.manifest.json` is an
independent [schema-3 manifest](./cli.md#manifest-schema-3) by default, whose digest and byte count describe its
adjacent CSS file. Each manifest reports StyleId format 2, class-name format 1, and ThemeId format 3
in required top-level fields. This output-schema change does not alter the independently selected
bundle-plan schema 1 or 2.

To project explicit application ownership into every manifest without changing either plan schema:

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --manifest-version 4 \
  --reachability pliego.reachability.json
```

The sidecar path is resolved from the process working directory. Site paths inside it must match
the logical plan-relative labels recorded by the bundle scanner. Schema 4 fails if any compiled
origin lacks an exact component owner. Application nodes and route/island edges come from the
sidecar; each bundle contributes only its own semantic declarations and token dependencies. CSS
bytes and partitioning remain identical to schema 3 when pruning is disabled. See
[manifest schema 4](./manifest-schema-4.md) and
[reachability schema 1](./reachability-schema.md).

Selecting `--manifest-version 5` with the same sidecar adds a complete physical trace to each
bundle. Rule/declaration ordinals restart at zero per bundle and every range addresses that bundle's
own digest-bound CSS. See [manifest schema 5](./manifest-schema-5.md).

### Opt-in pruning inside explicit bundles

Add `--prune-unreachable` only with the same schema-4/schema-5 reachability options:

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --manifest-version 5 \
  --reachability pliego.reachability.json \
  --prune-unreachable
```

The sidecar's route and island component references form one union root set. For each bundle, the
compiler validates and classifies only the styles and exact origins already selected by that
bundle's `sources`. A StyleId remains in that bundle when any one of its exact origins is reachable,
and the retained manifest keeps every origin for the style. A shared site owned by several
components is reachable when any owner is a root.

This filtering does not change the plan partition: it does not assign a bundle to a route, move
styles between bundles, extract a shared bundle, or derive source ownership. Semantic declarations
and direct token nodes in each graph describe only styles emitted by that bundle. If no root reaches
any style in one bundle, its CSS is exactly one newline when `emit-theme = false`; with
`emit-theme = true`, it is `:root{}` plus one newline under pruning. Non-pruned theme emission keeps
the complete supported block. With retained styles, pruning emits only directly consumed
variable-backed tokens.

All bundles finish compilation before publication starts. Therefore a plan, source, theme, Resolver
selection, or CSS error in any bundle publishes none of the newly compiled outputs. A schema-1 plan
containing schema-2 theme fields is rejected before publication.

### Optional asset load plan

Add `--asset-plan` only with manifest schema 4 or 5 and the same required reachability sidecar:

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --manifest-version 5 \
  --reachability pliego.reachability.json \
  --prune-unreachable \
  --asset-plan
```

The compiler validates every complete manifest and adjacent CSS pair before joining their common
topology. `pliego.assets.json` records all explicit bundles with exact CSS/manifest byte counts and
SHA-256 digests, then maps routes and islands to bundle IDs independently. A route or island selects
a bundle when its declared component set intersects that bundle's emitted semantic ownership; the
single optional bundle that actually emits theme declarations is application-global and is selected
first for every root. Under pruning, a requested theme bundle with no retained variable consumer
contains only `:root{}` and is not treated as theme-emitting by the Asset Plan.

The plan records `ruleSelection: "all-compiled"` when pruning is absent and
`"reachable-style-ids"` when `--prune-unreachable` selected the emitted StyleId sets. A fully pruned
non-theme bundle remains in the top-level integrity ledger but may appear in no route or island. This
output does not discover framework topology, merge island membership into routes, repartition source
sets, construct deployment URLs, or choose `<link>`/preload policy. See the complete
[asset load-plan schema 1](./asset-plan-schema.md).

### Optional control group

Add `--control` to an Asset Plan build:

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --manifest-version 5 \
  --reachability pliego.reachability.json \
  --asset-plan \
  --project-index \
  --control
```

Before publication, PliegoCSS audits every generated CSS payload in memory. The control manifest
hashes the exact plan, optional theme configuration or DTCG Resolver, reachability document, and
Rust source snapshots, plus every requested output and the canonical findings document. It emits one canonical
`NAME.css.map` for each CSS asset. Output relationships connect each CSS file to its source map and
style manifest, each map back to CSS, the Asset Plan to all CSS/manifest pairs, and the Project Index
to the Asset Plan. A configured/seed theme projects its active flat typed registry. A schema-2
`dtcg-resolver` theme instead publishes the complete Resolver graph and measures retained-token
coverage against the selected registry. The exact Resolver bytes enter `inputs.files` with role
`token-resolver`; the exact plan bytes, including `theme.inputs`, already participate in
`configHash`. Consequently two selections remain distinguishable even when they resolve to the same
ThemeId and CSS.

## Publication and `--check`

Normal execution publishes the complete output set as one rollback-capable group under advisory
destination locks. Existing byte-identical files are retained. If a handled rename fails, the CLI
attempts to restore every previous destination before returning an error.

This is recoverable grouped publication, not a crash-atomic filesystem transaction. Terminating the
process during preparation or between renames can leave sibling `*.tmp` files, a partial destination
set, or `*.bak` files requiring manual recovery. Consumers must verify each manifest's `cssSha256`
and `cssBytes` before accepting a CSS asset.

Use read-only verification in CI:

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --check
```

`--check` compiles from the same snapshot contract and compares every expected output byte for byte.
To include the complete control group in that comparison, repeat its required producer options:

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --manifest-version 5 \
  --reachability pliego.reachability.json \
  --prune-unreachable \
  --asset-plan \
  --project-index \
  --control \
  --check
```

Check mode writes nothing and fails if any expected CSS, source map, specialized manifest, Asset
Plan, Project Index, findings, control manifest, or receipt is missing or differs. Normal execution
publishes all requested outputs as the same rollback-capable group; this does not make replacement crash-atomic.

## Verification

The schema-1 regression and schema-2 DTCG contracts run separately:

```text
cargo test -p pliego-cssc --test bundle_regression
cargo test -p pliego-cssc --test bundle_dtcg
cargo test --workspace --all-features
node scripts/check-packages.mjs --allow-dirty
```

The local schema-2 gate passes on Windows x64 and Debian WSL2 Linux x64. It covers default and
explicit selections, case-fold collisions, exact Resolver evidence, complete graph publication,
same-ThemeId selection identity, read-only matching/drift checks, and last-valid-group retention.
The package workflow compiles every extracted archive and its downstream consumer with Rust 1.85.
Hosted and macOS evidence remain release work.

## Deliberate limits

Neither schema provides:

- inferred Cargo/module, component, route, or island reachability; optional schema-4/schema-5
  manifests import only explicit reachability schema 1;
- automatic topology collection, route-to-island occurrence, or automatic `<link>`/preload
  generation; the optional asset plan only projects explicit validated route/island ownership;
- glob patterns, imports, dependencies between bundles, or a shared-style extraction policy;
- cross-bundle deduplication, critical CSS, individual declaration pruning, or tree shaking
  that derives or changes the explicitly supplied source partition;
- cleanup of stale files in the output directory;
- bundle watch mode or hot reload;
- crash-atomic publication.

Schema 2 is only the bundle-plan bridge for the existing DTCG Resolver. It does not add external
Resolver I/O, JSON discovery, or one theme per bundle. Cargo does not read this plan: configure the
same Resolver directly with `theme!(tokens = FILE, inputs = {...})` once per package, then repeat
that selection in CLI generation. The Cargo artifact contains only the selected registry; controlled
bundle output remains responsible for the complete Resolver graph and exact plan evidence.

Removing or renaming a bundle in the plan does not delete its previous output. The application or
packaging layer owns stale-asset cleanup and translates verified asset-plan selections into its own
deployment URLs and loading policy.

For the current explicit PliegoRS asset seam, see the
[PliegoRS integration boundary](../integrations/pliegors.md).
