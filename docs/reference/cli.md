# Command-line compiler

Status: implemented extraction, validation, inspection, declarative manual bundles, theme
configuration, schema-4 semantic ownership, schema-5 physical tracing, bounded repair planning and
read-only dry-run verification, opt-in unreachable-rule
pruning, framework-neutral asset load plans, event-driven watch, and generated control groups with
canonical Source Map v3 and TokenGraph artifacts, plus bounded accessibility-policy audit and
ownership-backed package/composed-route budgets for Asset Plans; direct and bundle-plan schema-2
DTCG selection have passed local Windows/WSL E2E, workspace, and package gates. Ownership schema 1
and its explicit audit option have passed the local core and Debian WSL2 CLI gates. Bundle usage
analysis now separates static reachability, scoped observation, usage verdict, and removal state

`pliego-cssc` discovers visible `pc!`/`pcx!` literals or accepts explicit utility lists, lowers them
against one theme registry, and produces deterministic CSS and provenance metadata. It is currently a
workspace binary, not yet a published package.

## Synopsis

```text
pliego-cssc [--diagnostic-format human|json] compile|build [--style "flex gap-4"] [--compose "base" "branch"] [--input styles.txt] [--source file-or-directory ...] [--config theme.toml|--seed|--tokens tokens.json [--token-input modifier=context]...] [--theme] [--targets baseline-widely|modern|none] [--format minified|pretty] [--output app.css] [--manifest manifest.json [--manifest-version 3|4|5] [--reachability pliego.reachability.json] [--prune-unreachable]] [--control-dir DIR [--check]]
pliego-cssc [--diagnostic-format human|json] bundle --plan pliego.bundles.toml --output-dir dist/assets [--check] [--manifest-version 3|4|5] [--reachability pliego.reachability.json] [--prune-unreachable] [--asset-plan] [--project-index] [--usage-report [--observations FILE.json] [--retention FILE.json]] [--control]
pliego-cssc [--diagnostic-format human|json] check|inspect [same input/theme/target/format options]
pliego-cssc watch [--input styles.txt] [--source file-or-directory ...] --output app.css [--config theme.toml|--seed|--tokens tokens.json [--token-input modifier=context]...] [--theme] [--targets baseline-widely|modern|none] [--format minified|pretty] [--manifest manifest.json [--manifest-version 3|4|5] [--reachability pliego.reachability.json] [--prune-unreachable]] [--control-dir DIR]
pliego-cssc [--diagnostic-format human|json] catalog [--config theme.toml|--seed] [--format markdown|json] [--output catalog.md|--check catalog.md]
pliego-cssc [--diagnostic-format human|json] audit --input FILE.css --targets baseline-widely|modern|none [--budget-policy FILE.json [--budget-subject package=NAME|route=/PATH ...]] [--accessibility-policy FILE.json [--token-graph FILE.json]] [--control-dir DIR [--check]] [--format human|json|sarif]
pliego-cssc [--diagnostic-format human|json] audit --asset-plan FILE.json [--ownership FILE.json] --targets baseline-widely|modern|none [--budget-policy FILE.json] [--accessibility-policy FILE.json [--token-graph FILE.json]] [--control-dir DIR [--check]] [--format human|json|sarif]
pliego-cssc [--diagnostic-format human|json] transform-css --input FILE.css --output FILE.css [--targets baseline-widely|modern|none] [--format minified|pretty] [--control-dir DIR] [--check]
pliego-cssc [--diagnostic-format human|json] compatibility --targets baseline-widely|modern|none
pliego-cssc [--diagnostic-format human|json] migration-inventory sass|tailwind|css-modules FILE
pliego-cssc [--diagnostic-format human|json] migration-project-inventory DECLARATION.json|DIRECTORY
pliego-cssc [--diagnostic-format human|json] migration-project-plan DECLARATION.json|DIRECTORY
pliego-cssc [--diagnostic-format human|json] explain --style "utilities" [--config theme.toml|--seed] [--targets baseline-widely|modern|none] [--format text|json]
pliego-cssc [--diagnostic-format human|json] explain-cascade --input FILE.css --element 'button#save.action' --property LONGHAND [--format text|json]
pliego-cssc [--diagnostic-format human|json] plan --findings FILE.json --proposal FILE.json --source-root DIR [--format text|json]
pliego-cssc [--diagnostic-format human|json] fix --plan FILE.json --findings FILE.json --source-root DIR --dry-run [--format text|json]
pliego-cssc [--diagnostic-format human|json] fmt (--style "utilities" ...|--input styles.txt) [--output formatted.txt|--check]
pliego-cssc [--diagnostic-format human|json] fmt --source file-or-directory ... (--check|--apply)
pliego-cssc --version
```

Run it from the workspace through Cargo:

```console
cargo run -p pliego-cssc -- compile --source src/lib.rs --theme --output pliego.css
```

Install the public-preview binary from crates.io and invoke it from an application workspace:

```console
cargo install pliego-cssc --version '=0.1.0-rc.3' --locked
pliego-cssc --version
pliego-cssc check --source src
```

`build` is an exact alias of `compile`. `version`, `-V`, and `--version` print
`pliego-cssc <package-version>` and reject extra arguments. Registry consumers must
pin the complete exact prerelease because the nineteen crates form one compatibility
unit.

## Inputs

At least one input is required. The build commands accept all input types together and process them
in argument-category order: inline styles, explicit compositions, the line-oriented input, then Rust
source files in the order supplied.

| Option | Repeatable | Contract |
|---|---:|---|
| `--style "utilities"` | Yes | Adds one complete utility list. |
| `--compose "base" "branch"` | Yes | Adds one branch-over-base semantic composition. |
| `--input path` | No | Reads complete utility lists from a UTF-8 line-oriented file. |
| `--source path` | Yes | Scans one `.rs` file or a directory tree for visible `pc!` and `pcx!` invocations. |
| `--config path.toml` | No | Selects this theme configuration explicitly. Conflicts with `--seed` and `--tokens`. |
| `--seed` | Boolean | Forces the built-in seed registry and disables conventional theme discovery. Conflicts with `--config` and `--tokens`. |
| `--tokens path.json` | No | Explicit compile/build/check/inspect/watch input for one DTCG Resolver document. Conflicts with `--config` and `--seed`; JSON is never auto-discovered. |
| `--token-input modifier=context` | Yes | Selects one resolver context. Requires `--tokens`; duplicate/case-colliding modifiers and unknown names/values fail. |
| `--targets baseline-widely\|modern\|none` | No | Selects a versioned compatibility profile; compile commands default to `modern`. |
| `--format minified\|pretty` | No | Selects Lightning CSS output formatting; default is `minified`. |
| `--theme` | Boolean | Prepends active-registry custom properties; with explicit pruning, emits only variable-backed tokens consumed by retained styles. Compile/build/watch only. |
| `--output path` | No | Writes CSS; compile/build prints CSS when omitted. |
| `--manifest path` | No | Writes default schema 3 or explicitly selected schema 4/5 JSON. Compile/build/watch only. |
| `--manifest-version 3\|4\|5` | No | Selects a numbered manifest contract; requires manifest output. Version 3 is the default. |
| `--reachability path.json` | No | Supplies strict reachability schema 1; requires `--manifest-version 4` or `5`. Compile/build/watch and bundle only. |
| `--prune-unreachable` | Boolean | Retains only StyleIds with at least one exact origin owned by a route- or island-reachable component. Requires `--reachability` and therefore manifest schema 4 or 5. Compile/build/watch and bundle only. |
| `compile/build --control-dir path` | No | Requires CSS and manifest outputs inside this existing common root; publishes them with fixed Source Map, `pliego.tokens.json`, findings, control manifest, and receipt files as one rollback-capable group. |
| `compile/build --check` | Boolean | Recompiles from one exact snapshot and fails on drift across the complete seven-artifact control group without writing. Requires `--control-dir`. |
| `watch --control-dir path` | No | Requires CSS and manifest outputs inside this existing common root and republishes the complete seven-artifact group from each confirmed valid watch snapshot. |
| `bundle --plan path.toml` | No | Selects one exact bundle plan. Schema 1 supports seed/config; schema 2 additionally supports plan-owned `dtcg-resolver` selection. |
| `--asset-plan` | Boolean | Adds fixed `OUTPUT_DIR/pliego.assets.json` to a bundle build. Requires manifest schema 4 or 5 plus `--reachability`; takes no value. |
| `bundle --control` | Boolean | Requires `--asset-plan`; adds one Source Map per CSS plus shared `pliego.tokens.json`, findings, control manifest, and receipt outputs. |
| `audit --asset-plan path.json` | No | Audits and regenerates an existing Asset Plan plus adjacent CSS/manifests; mutually exclusive with audit `--input`. |
| `audit --ownership path.json` | No | Supplies closed ownership schema 1 for Asset Plan package and composed-route subjects. Requires `--asset-plan`, is never discovered, and conflicts with every `--budget-subject`. |
| `audit --accessibility-policy path.json` | No | Applies closed accessibility policy schema 1 to direct CSS or every Asset Plan stylesheet. The five check enforcement vectors are explicit; no policy is auto-discovered. |
| `audit --token-graph path.json` | No | Supplies canonical `pliegocss-token-graph/1` evidence for token-backed or theme-selected contrast relationships. Requires `--accessibility-policy`. |
| `audit --control-dir path` | No | For direct CSS or Asset Plan input, publishes canonical findings, control manifest, and receipt files plus fixed `pliego.tokens.json` when `--token-graph` is supplied. |
| `audit --check` | Boolean | Recomputes the complete three- or four-artifact audit control group and fails on byte drift without writing; requires `--control-dir`. |
| `--project-index` | Boolean | Adds fixed `OUTPUT_DIR/pliego.index.json` to a bundle build. Requires `--asset-plan`, manifest schema 5, and `--reachability`; takes no value. |
| `bundle --usage-report` | Boolean | Adds fixed `OUTPUT_DIR/pliego.usage.json` and `OUTPUT_DIR/pliego.token-usage.json`. The first inventories the all-compiled bundle-qualified StyleId universe; the second projects the active Token Graph through the retained selection. Reachability is optional. |
| `bundle --observations path.json` | No | Supplies explicit usage-observation schema 1. Requires `--usage-report` plus exact `--reachability`; the universe and reachability hashes must match. The file is never discovered. |
| `bundle --retention path.json` | No | Supplies explicit non-empty usage-retention schema 1. Requires `--usage-report`, `--prune-unreachable`, and exact `--reachability`; selects the reachable union plus only the exact bundle-qualified dead StyleIds granted by policy. The file is never discovered. |

Unknown options, duplicate single-value options, missing values, unreadable sources, invalid UTF-8,
Rust parse errors, scanner diagnostics, invalid theme/token configuration or selection, style
diagnostics, emission errors, and output failures exit unsuccessfully.

Ownership adds one non-repeatable, audit-only option:

```text
audit --asset-plan FILE.json --ownership FILE.json
```

The same revision contains the public parser/builder, exact-plan join, package/route aggregation, and
negative CLI tests. `--ownership` is never discovered, requires `--asset-plan`, and conflicts with
direct `--input` and all manual `--budget-subject` values.

### Controlled compile and watch

`compile` and its `build` alias can integrity-bind the generated CSS and specialized style manifest:

```console
mkdir -p dist
pliego-cssc compile --source src --seed --theme \
  --output dist/app.css --manifest dist/app.manifest.json \
  --control-dir dist
pliego-cssc compile --source src --seed --theme \
  --output dist/app.css --manifest dist/app.manifest.json \
  --control-dir dist --check
```

The first command snapshots every line input, expanded Rust source, resolved theme file, and
reachability document once, compiles only those bytes, audits the generated CSS in memory, and
publishes seven outputs through one rollback-capable boundary: CSS, `CSS_PATH.map`, style manifest,
`pliego.tokens.json`, findings, control manifest, and receipt. The second command recomputes those
same seven artifacts and
compares them without locks, writes, or repair. CSS, its derived map, and style manifest must
resolve inside `--control-dir`; the control manifest records their portable paths relative to that
root. Inputs must resolve inside the current workspace and cannot traverse symlinks or reparse
points.

`sourceHash` covers exact line/Rust bytes plus canonical `pliego.cli-input.json` bytes for inline
`--style`/`--compose` sources. `configHash` separately covers target policy plus canonical compiler
settings, exact theme/token configuration, canonical resolver selections, and exact reachability
bytes. Selections remain bound independently of `ThemeId`, including when different contexts resolve
to the same typed registry. Compiler settings bind operation, format, theme
emission/selection/identity, manifest version, and pruning. CSS, source map, and style
manifest relate to one another and the shared token graph; findings relate to all four. The map uses
canonical Source Map v3,
selects one deterministic authored origin per final generated class selector, and does not alter CSS
with a discovery comment. The control manifest hashes exact canonical `pliegocss-token-graph/1`
bytes and records transitive referenced-token coverage after reachability pruning. TOML/seed input
projects the active registry with `TokenGraph::from_registry`, producing one literal `default`
theme. The `--tokens` path selects one resolved registry but publishes the Resolver's
complete graph with every validated permutation. The local full-workspace and package gates are
green for this direct surface. Bundle schema 2 carries DTCG selection inside the plan and passes its
local Windows/WSL E2E, workspace, Rust 1.85, and package gates. The Cargo build macro now accepts
the same Resolver inputs for macro identity; its local workspace/MSRV/API and clean-package gates
pass.

`watch --control-dir DIR` applies the same contract to each two-poll-confirmed snapshot. A failed
compile or invalid reachability edit retains the last valid seven-artifact group. A byte-only config
change can update the receipt even when CSS and specialized manifest bytes are unchanged. Watch has
no `--check` mode because it is a long-lived producer.

### Structured diagnostics

`--diagnostic-format human|json` is a global, single-use option and may appear before or after the
command; `--diagnostic-format=json` is equivalent. It does not conflict with command-specific
`--format` options. The global parser respects each command option's value arity, so a style or path
whose complete value is `--diagnostic-format=json` remains a value rather than being stolen as a
global flag. The default `human` behavior remains intended for terminals.

On a JSON-mode failure, one schema-1 document is written to standard error, standard output remains
empty, and the process exits with status 1. Successful command output is unchanged. `watch` rejects
JSON mode before entering its loop because a long-lived stream needs a separate NDJSON/event
contract. PCS, PSC, PCX003, and FMT001 findings remain typed and ordered; invocation and untyped tool
failures use the stable PCL001 and PCL002 fallbacks. See the complete
[diagnostic JSON schema](./diagnostic-schema.md).

### Line-oriented input

Each non-empty line is one style. Unescaped outer whitespace is ignored by the parser; escaped
whitespace remains part of its candidate payload. A line whose first non-whitespace character is `#`
is reserved as a comment in this file format; inline comments are not supported. Files may use LF or
CRLF, while isolated carriage returns fail instead of silently merging logical lines.

```text
# styles.txt
flex items-center gap-4
grid grid-cols-2 gap-6 md:grid-cols-3
rounded-lg bg-surface p-6 hover:bg-surface-raised
```

```console
cargo run -p pliego-cssc -- compile --input styles.txt --theme --output pliego.css
```

### Rust source scanning

`--source` accepts a `.rs` file or directory. Directories are traversed recursively in lexical order;
only `.rs` files are scanned. Symbolic links are never followed, and `.git`, `target`, and hidden
directories are omitted. Repeated and overlapping file/directory arguments are canonicalized and
deduplicated before scanning.

The shared syntax-tree scanner, not text or regular-expression matching, discovers qualified and
unqualified `pc!`/`pcx!` calls, including calls nested in another macro's token tree. Comments,
string contents, similarly named macros, and `macro_rules!` templates are ignored.

The terminal macro names `pc` and `pcx` are reserved within scanned source. The scanner does not
resolve Rust imports or macro ownership: any invocation whose final path segment is exactly `pc` or
`pcx` is interpreted as PliegoCSS, including a local or third-party macro with that name. Avoid those
names in scanned units or narrow the supplied source paths.

```rust
fn styles(active: bool, compact: bool) {
    let _ = pc!("flex items-center gap-4");
    let _ = pcx!(
        "rounded-md bg-surface",
        if active { "bg-accent" } else { "bg-transparent" },
        if compact { "p-2" } else { "p-4" },
    );
}
```

The example yields one `pc!` style and four reachable `pcx!` styles. For every Cartesian selection,
the compiler lowers the base, then composes each selected clause branch in source order using the
same semantic override operation as the macro. It never lowers the scanner's joined diagnostic
string as a normal conflicting utility list. One `pcx!` call is capped at 64 reachable
combinations.

Before composing, the CLI invokes the compiler's same cross-clause conflict analyzer used by the
macro. If independently selectable clauses can assign intersecting semantic slots under the same
structural condition, extraction fails with `PCX003` and the right-hand branch literal's file and
range. Source scanning therefore cannot introduce silent clause-order precedence.

Every scanner diagnostic is fatal for the command. Diagnostics include the file, one-based line and
column, and exact half-open byte range:

```text
PSC001: `pc!` requires one Rust string literal at src/view.rs:8:21 [bytes 143..155)
```

The scanner does not follow Rust `mod` declarations or Cargo dependencies beyond the supplied
directory tree, expand macros, or inspect generated code outside that tree.

## Theme configuration

For `compile`/`build`, `check`, `inspect`, and `watch`, the explicit selectors are mutually
exclusive:

1. `--tokens path.json` selects exactly that DTCG Resolver document;
2. `--config path.toml` selects exactly that TOML theme;
3. `--seed` forces `ThemeRegistry::seed()`;
4. otherwise the CLI discovers only conventional `pliego.theme.toml` files.

Discovery considers `pliego.theme.toml` in the current directory and the nearest such file for every
`--input` and `--source` path, walking upward through the containing Cargo package. Canonically
equivalent candidates deduplicate. Zero candidates select the seed registry, one candidate selects
that file, and multiple distinct candidates fail with an instruction to use `--config` or `--seed`.
Inline `--style` and `--compose` inputs provide no path hint, so they use only current-directory
discovery. The CLI never searches for `*.json` or token documents. A TOML configuration extends the
seed and may override or add typed tokens and breakpoints:

```toml
schema = 1
extends = "seed"

[tokens.color]
brand = "oklch(62% 0.18 250)"

[tokens.spacing]
gutter = "1.5rem"

[breakpoints]
tablet = "48rem"
```

```console
cargo run -p pliego-cssc -- compile \
  --config pliego.theme.toml \
  --style "bg-brand tablet:flex" \
  --theme --output pliego.css
```

A Resolver input uses explicit, repeatable context selections:

```console
cargo run -p pliego-cssc -- compile \
  --tokens examples/product.resolver.json \
  --token-input appearance=dark \
  --style "bg-brand" --theme --output pliego.css
```

Every `--token-input` requires `--tokens`. Omitted modifiers use their declared defaults. Modifier
names and context values are case-insensitive and are replaced by the canonical selection map before
identity binding. Repeating a modifier, including a case-fold collision, fails; unknown modifiers,
unknown contexts, and missing required non-default inputs also fail closed. The active permutation
drives parsing, lowering, CSS, and `ThemeId`, while controlled compile/build/watch publishes the
complete validated resolver graph. The canonical selection map participates in `configHash` even
when two contexts have the same `ThemeId`.

Parsing, lowering, `StyleId`, CSS values, breakpoint media queries, emitted variables, manifest
`themeId`, and inspection all use that same active registry. `--theme` does not select a registry; it
only decides whether its supported custom properties are included in the CSS. Procedural macros do
not use CLI discovery: custom-theme macro expansion still requires one package `build.rs` call.
For DTCG, use `theme!(tokens = FILE, inputs = { "modifier" => "context" })` and repeat the same
Resolver/selections as `--tokens`/`--token-input`; the Cargo artifact contains only the selected
registry while controlled CLI output publishes the complete graph. `bundle` has no DTCG flags;
schema 2 instead declares `kind = "dtcg-resolver"`, `path`, and optional `[theme.inputs]` inside its
exact plan snapshot.

## Browser target and policy contract

`--targets baseline-widely` selects compatibility policy schema 2 / policy version 7. It retains the
WebDX Widely Available mapping observed on 2026-07-14: Chrome/Edge 120, Firefox 121, and Safari/iOS
Safari 17.2 and integrity-binds `web-features@3.32.0` plus
`baseline-browser-mapping@2.10.43`. It fails closed before output publication when semantic IR contains unclassified
arbitrary values (`CMP001`), properties (`CMP002`), or selectors (`CMP003`).

Typed `container-inline`, `cq-<theme-breakpoint>:`, `aria-[name=value]:`, `data-[name]:`,
`data-[name=value]:`, the three typed `writing-*` utilities, and the four fixed-order `layer-*`
variants are classified and allowed by policy 7. Lightning CSS may serialize the equivalent
container min-width query using range syntax for the frozen targets.

`--targets modern` remains the compile default for candidate-output compatibility and configures
Lightning CSS with fixed version numbers:

- Chrome 111;
- Edge 111;
- Firefox 128;
- Safari 16.4.

Versions use Lightning CSS's `major << 16 | minor << 8 | patch` representation. No browserslist file,
environment lookup, or `browserslist` Cargo feature participates, so identical inputs cannot change
because of host configuration. Lightning CSS applies required transforms/prefixes for the contract
and prints the selected output format. For example, syntax unsupported by Safari 16.4, such as CSS
nesting, is lowered.

`--targets none` sets no compatibility targets and makes no browser guarantee. Lightning CSS still
parses and prints the artifact, but it does not request compatibility transforms. This is the
explicit escape hatch when a later build stage owns browser lowering. The chosen profile is recorded
in manifest and inspection JSON.

All profiles are independent of Browserslist files, environment, network, and current date. The
`compatibility --targets PROFILE` command emits the deterministic browser, feature, reset, and scope
policy without compiling styles. Reset profile `none` means PliegoCSS never injects a reset;
scope profile `standard-class` means the host owns the external stylesheet boundary. See
[compatibility policy schema 2](./compatibility-policy.md).

## Output format contract

`--format minified` is the default and preserves the compact artifact contract. `--format pretty`
asks Lightning CSS itself to print indented, development-readable CSS; PliegoCSS does not apply a
second formatter. Both modes run the same parsing, compatibility transforms, semantic ordering, and
deduplication. Before printing, exactly equal adjacent `@media` siblings are combined without
crossing an intervening rule; their qualified children stay in the same order. Consequently, a style
set has the same `StyleId` and class names under both modes,
while CSS byte length and digest normally differ.

The selected `format` is recorded in both manifest and inspection JSON. Their `cssSha256` and
`cssBytes` fields describe the exact selected-format CSS bytes, including the single final newline.
This makes format changes explicit to artifact consumers instead of presenting them as unexplained
integrity mismatches.

### Identity format fields

Every successful JSON document that exposes identity carries three required top-level fields:
`styleIdFormatVersion`, `classNameFormatVersion`, and `themeIdFormatVersion`. Their current values
are 2, 1, and 3 respectively. They appear in manifest schemas 3, 4, and 5, inspection schema 3, catalog
schema 4, and explain schema 2. A consumer must check them before interpreting a `styleId`, class, or theme
identity. The schema-1 diagnostic envelope is unchanged and does not claim that identity resolution
completed.

## Commands

### `audit`

`audit` requires exactly one project-relative CSS file or Asset Plan plus one explicit target
profile. It performs no transforms and, unless `--control-dir` is explicit, no writes. It emits
canonical finding schema 1.0.0 in human, JSON, or SARIF form:

```console
pliego-cssc audit --input src/app.css --targets baseline-widely --format json
pliego-cssc audit --input src/app.css --targets baseline-widely --format sarif
pliego-cssc audit --input src/app.css --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json \
  --token-graph dist/pliego.tokens.json --format json
pliego-cssc audit --asset-plan dist/pliego.assets.json --targets baseline-widely \
  --accessibility-policy config/pliego.accessibility.json --format json
```

Typed ownership is explicit:

```console
pliego-cssc audit --asset-plan dist/pliego.assets.json \
  --ownership config/pliego.ownership.json --targets baseline-widely \
  --budget-policy config/pliego.budgets.json --format json
```

Ownership schema 1 binds the plan's exact bytes/SHA-256, assigns every bundle to exactly one
package, and supplies islands for every route. The public core contract tests, Asset Plan ownership
CLI E2E, and checked example replay pass locally in Debian WSL2; that is not hosted-CI evidence.

The current `css-rule-features-1` classifier reports frozen official compatibility decisions for
layers, container query kinds, nesting, `@property`, `@scope`, and `@starting-style`. Syntax or
enforced compatibility errors exit nonzero. A mandatory partial-coverage warning states that the
complete declaration-value and selector surfaces are not yet classified. See the dedicated
[audit contract](./audit-command.md).

`--format sarif` emits SARIF 2.1.0 from that same canonical document. Results retain `ruleId`,
severity as SARIF `level`, portable byte/line regions, `partialFingerprints`, and the entire source
finding under `properties.pliegoCssFinding`. The embedded finding is the executable parity boundary:
SARIF cannot omit evidence, verification state, ranked suggestions, reviewed exceptions, or source
maps that exist in JSON/human output.

For direct CSS or Asset Plan input, `--control-dir DIR` additionally publishes one rollback-capable
group:

- `pliego.tokens.json` — present only when `--token-graph` supplies exact canonical graph bytes;
- `pliego.css.findings.json` — the canonical finding document regardless of terminal renderer;
- `pliego.css.manifest.json` — source/config hashes, backend/targets/data identity, measured rules,
  explicit unknown attribution, compatibility decisions, violations, and token observation state;
- `pliego.css.receipt.json` — exact manifest/output hashes, decision/finding summaries, required
  checks, and a derived `passed|failed` result.

`--check` recomputes the same three or four artifacts and compares exact bytes without writing. A
failed syntax audit still emits a valid failed receipt: `rules.observation` becomes `unavailable`
with a reason, rather than
inventing zero measured rules. Without `--token-graph`, `tokens.observation` is honestly
`unavailable`. With it, the exact graph is published, `tokens.observation` is `measured`,
`coverageBasisPoints` is `0` because standard-CSS audit has no compiler-owned typed-use set, and
`contrastPairs` counts declarations in the accessibility policy rather than per-theme evaluations.
That zero is a measured boundary, not an unavailable state and not a claim that the application uses
no tokens. Asset Plan mode binds its canonically regenerated plan plus every CSS and style-manifest
input and requires `asset-plan-integrity`. Audit-only inputs do not reconstruct a graph or generated
CSS Source Map: the graph must be supplied explicitly, while maps exist only for controlled
generator outputs.

The control input ledger uses stable roles: the Asset Plan is `asset-plan`, a budget policy is
`policy`, ownership is `ownership`, an accessibility policy is `accessibility-policy`, and the
optional audit graph is `token-graph`. Canonically serialized budget-policy content contributes to
`configHash`; ownership, accessibility-policy, and token-graph inputs contribute their exact file
bytes. Asset Plan/CSS/style-manifest bytes contribute to `sourceHash`.

`--budget-policy FILE.json` optionally applies closed budget schema 1. The repeatable
`--budget-subject package=NAME|route=/PATH` explicitly attributes the complete input artifact;
file ownership and parsed layer subjects are automatic. Limits cover canonical minified bytes,
rules, selectors, exact specificity tuples, and context-aware normalized declaration duplication,
with absolute ceilings, baseline deltas, and reviewed exceptions. Every definition in a selected
policy must match one verified subject; any unmatched definition makes even partial policy coverage
fail closed with `PCSS-BUDGET-199`. See [budget policy schema 1](./budget-policy.md).

With `--asset-plan dist/pliego.assets.json`, every manual `--budget-subject` is rejected. The command
reads every adjacent CSS/manifest pair and regenerates the exact canonical plan with the existing
validator. File subjects cover each CSS file and layer subjects aggregate over the complete bundle
ledger. A policy containing package or route subjects requires `--ownership`.

The ownership companion does not change Asset Plan schema 1. After exact plan binding, it requires
total/exclusive bundle-to-package ownership and a total route-to-islands map. Package subjects
aggregate their owned bundles once; route subjects aggregate the stable deduplicated union of base
and island bundles. Routes may overlap and therefore remain invalid as `Control Manifest
rules.byRoute` partitions. Package observations likewise do not populate `rules.byPackage` until a
future Control Manifest schema defines and separately validates that partition.

`--accessibility-policy FILE.json` enables the bounded static accessibility analyzer for the same
direct or Asset Plan stylesheet set. It evaluates declared contrast relationships plus CSS-AST
evidence for exact motion-preference guards, explicit focus-outline suppression, explicit
forced-color adjustment, and same-rule hover/focus declaration equivalence. Unproven cascade or
cross-context correlation remains `manual-required`. Token endpoints and selected resolver themes need `--token-graph`; an
unselected literal-to-literal pair does not. Non-summary findings preserve `verified`, `unverified`,
and `manual-required`, carry the stable `context.subject-id` used by reviewed exceptions, and obey
the policy's `fail|warn` enforcement. A passing gate is not WCAG conformance and does not replace DOM,
browser, assistive-technology, interaction, or human testing. See
[accessibility policy schema 1](./accessibility-policy.md).

### `compatibility`

`compatibility` requires exactly one explicit `--targets` profile and writes policy schema 2 JSON to
standard output. It accepts no source, theme, format, or output options. The document has one final
newline and is byte-deterministic:

```console
pliego-cssc compatibility --targets baseline-widely
```

The document enumerates target versions and per-feature `allow`, `transform`, `warn`, or `error`
decisions. It is the machine contract used by CI, adapters, and future editor tooling; it is not a
live browser-data query.

### `migration-inventory`

`migration-inventory` creates a conservative, read-only schema-1 inventory for exactly one Sass,
Tailwind CSS v4 entry, or CSS Modules source:

```console
pliego-cssc migration-inventory sass src/legacy.scss
pliego-cssc migration-inventory tailwind src/app.css > tailwind.inventory.json
pliego-cssc migration-inventory css-modules src/card.module.css
```

The source kind and input are mandatory positional arguments. The input must be a portable,
project-relative path and kind and extension must agree. Canonical JSON is written only to stdout; the command has no mutation or
tool-execution surface. Callers may redirect stdout and compare the resulting bytes in their own
build system.

The command executes no Sass, Tailwind, PostCSS, JavaScript, plugins, configuration, imports, or
templates. A `static` disposition means only that PliegoCSS bounded and recorded the lexical
construct. Dynamic and unsupported seams stay explicit. See
[migration inventory schema 1](./migration-inventory-schema-1.md) for constructs, Preflight
classification, defensive limits, and current project-graph exclusions.

### `migration-project-inventory`

`migration-project-inventory DECLARATION.json|DIRECTORY` either reads one bounded, regular,
project-relative schema-1 declaration or performs bounded typed discovery below one relative
directory. It collects the resulting typed project and writes the canonical inventory only to
stdout:

```console
pliego-cssc migration-project-inventory migration.project.json > migration.inventory.json
pliego-cssc migration-project-inventory . > migration.inventory.json
```

The command does not crawl the repository, infer source kinds, execute source toolchains, or mutate
the project when given a declaration. Directory mode applies the documented bounded/no-follow
discovery contract and still does not execute source toolchains or mutate the project. Source,
consumer, and auxiliary paths in a declaration are relative to the command working
directory. Missing or mistyped exact local dependencies fail the complete command; external, local,
unresolved, and dynamic seams stay explicit. Declared CSS Modules JS/TS consumers add exact ESM,
TypeScript import-equals, and simple CommonJS imports, static class accesses, and conservative
dynamic-usage observations. Simple one-level binding aliases propagate the same target and remain
separate from class-usage counts. Simple destructuring retains shorthand and renamed class exports;
computed, rest, default, nested, typed, or otherwise ambiguous patterns fail closed as dynamic
without partial guesses. Declared Tailwind configs,
plugins, and templates add exact content identities and resolve matching relative `@config`,
`@plugin`, and exact-file `@source` seams. Literal template `class`/`className` values emit exact
class candidates while expressions remain dynamic, without executing JavaScript or template code.
The same physical component may be declared in distinct roles, such as CSS Modules consumer and
Tailwind template; duplicate entries remain rejected within each role.
Discovery classification and collection errors name the exact portable candidate path.
Declared Sass sources resolve containing-directory URLs without requiring `./`, including exact
`.scss`/`.sass`, extensionless files, partials, directory indexes, and legacy import-only files;
ambiguity fails closed, while configured load paths and importers are not executed.
The inventory also records a closed set of config keys and plugin registration API calls as
unsupported lexical seams; this does not claim that their bodies can be migrated. See the
[migration project inventory schema 1](./migration-project-inventory-schema-1.md).

### Reversible migration groups

```console
pliego-cssc migration-group-apply --manifest group.json --receipt group.receipt.json
pliego-cssc migration-group-rollback --receipt group.receipt.json
```

The apply command consumes a closed schema-1 manifest with ordered `{file, after}` entries, captures
exact before bytes, compensates prior files if a later member fails, and publishes one receipt. The
rollback command preflights every after hash before restoring all files in reverse order, then
removes the receipt. Both commands remain explicit; planning never executes them automatically.

### `compile` and `build`

The CSS pipeline is:

1. collect every supplied finding and fail on extraction diagnostics;
2. parse the utility syntax;
3. lower against the active typed token/breakpoint registry;
4. compose `pcx!` branch selections semantically in clause order;
5. encode each normalized semantic style as the theme-sensitive StyleId format-2 byte stream and
   derive its `StyleId`;
6. deduplicate equal canonical streams while retaining every origin and reject any inconsistent or
   colliding stream/ID mapping;
7. when `--prune-unreachable` is set, validate every exact origin and retain a canonical stream when
   any of its origins belongs to a component referenced by any route or island;
8. enforce the selected compatibility profile against retained normalized semantic IR;
9. order retained CSS rules by canonical stream bytes and optionally prepend the active theme;
10. parse, transform, and print with Lightning CSS under the selected target and format contracts;
11. stage CSS and the optional manifest only after successful compilation.

The CSS always ends in one newline. CSS rule order and bytes are independent of source discovery
order for the same set of semantic styles and compiler version.

### `bundle`

`bundle` compiles multiple explicitly named CSS/manifest pairs from one TOML plan. Frozen schema 1
supports seed/config themes; schema 2 retains those kinds and adds an explicit DTCG Resolver:

```console
pliego-cssc bundle --plan pliego.bundles.toml --output-dir dist/assets

# Opt into a complete application-ownership graph in every bundle manifest
pliego-cssc bundle --plan pliego.bundles.toml --output-dir dist/assets \
  --manifest-version 4 --reachability pliego.reachability.json

# Add fail-closed final CSS rule/declaration tracing to every bundle manifest
pliego-cssc bundle --plan pliego.bundles.toml --output-dir dist/assets \
  --manifest-version 5 --reachability pliego.reachability.json

# Prune whole unreachable StyleId rule sets independently inside each explicit bundle
pliego-cssc bundle --plan pliego.bundles.toml --output-dir dist/assets \
  --manifest-version 5 --reachability pliego.reachability.json \
  --prune-unreachable

# Also emit the framework-neutral route/island asset load plan
pliego-cssc bundle --plan pliego.bundles.toml --output-dir dist/assets \
  --manifest-version 5 --reachability pliego.reachability.json \
  --prune-unreachable --asset-plan --project-index --control

# Keep reviewed structurally dead StyleIds for an external consumer
pliego-cssc bundle --plan pliego.bundles.toml --output-dir dist/assets \
  --manifest-version 5 --reachability pliego.reachability.json \
  --prune-unreachable --usage-report --retention pliego.retention.json \
  --asset-plan --project-index --control
```

The schema-2 selection is plan-owned, not a new CLI flag:

```toml
schema = 2
targets = "modern"
format = "minified"

[theme]
kind = "dtcg-resolver"
path = "product.resolver.json"

[theme.inputs]
appearance = "dark"

[bundles.application]
sources = ["src/styles.rs"]
emit-theme = true
```

Schema 1 rejects `dtcg-resolver` and any `theme.inputs`. Under schema 2, `path` is required for
`config` and `dtcg-resolver`; inputs are accepted only for the Resolver kind. Omitted inputs select
declared defaults, while invalid modifiers, contexts, and case-fold collisions fail before any
bundle output is published.

Targets, output format, and theme selection are global to the plan. Each bundle declares its own
Rust source files or directories and whether to emit theme variables. Plan-contained paths resolve
relative to the plan file, while `--output-dir` must already exist. A bundle named `visit` produces
`visit.css` and `visit.manifest.json`. Every bundle must discover at least one style before pruning;
an opt-in pruned bundle with no reachable roots emits only the final newline, or the complete theme
custom-property block plus the final newline when `emit-theme = true`.

`--asset-plan` is a boolean switch with one fixed destination: `OUTPUT_DIR/pliego.assets.json`. It
validates every complete schema-4/5 manifest against its exact CSS, requires one common application
topology and build identity, and records every bundle's exact CSS/manifest byte counts and SHA-256
digests. `ruleSelection` is `all-compiled` without pruning and `reachable-style-ids` with pruning.
Routes and islands stay separate; an optional bundle with actual theme declarations is global and
appears first in every root selection. A requested but empty pruned `:root{}` is not globally
selected. The plan reports the explicit source partition and never emits URLs, links, or
preload policy.

`--project-index` is a second boolean switch with fixed destination
`OUTPUT_DIR/pliego.index.json`. It is accepted only with schema 5 and `--asset-plan`. The generated
schema-1 or schema-2 index hashes the exact asset plan and every source/CSS/manifest input, rejects
unsafe or case-colliding logical paths, and maps each source site to semantic declarations, tokens,
adapter-attested components, and bundle-qualified final CSS declarations. Schema 2 is used only for
retained-style selection. It is the shared CLI/adapter/LSP handoff; consumers do not rescan
repository layout to rebuild ownership.

`--usage-report` adds fixed `OUTPUT_DIR/pliego.usage.json`. Unlike the selected-only Project Index,
it retains the all-compiled `(bundleId, StyleId)` universe and exact origins before pruning. It
separates `staticReachability`, `observationState`, `usageState`, and `removalDisposition`; a
reachable style is not called observed, and absence from a sampled observation is never called
dead. `--observations FILE` is optional explicit positive-hit evidence bound to the report's
universe hash and the exact reachability digest. Contradictory or stale evidence fails before
publication.

The same option also adds fixed `OUTPUT_DIR/pliego.token-usage.json`. It classifies every active
resolved token as `direct`, `dependency`, or `unused`, records exact retained `(bundleId, StyleId)`
consumers, and distinguishes a compiler-owned custom-property spelling from actual emission in this
build. Query it without recompilation using
`pliego-css-tokens explain --report FILE --token KIND.NAME [--format text|json]`. See
[token-usage report schema 1](./token-usage-schema-1.md).

`--retention FILE` is an independent policy input over the same exact universe and reachability
bytes. It is accepted only with `--usage-report`, `--prune-unreachable`, and `--reachability`.
Each entry must name one structurally unreachable `(bundleId, StyleId)`; missing, reachable,
mixed-origin, unknown, stale-hash, duplicate, or contradictory targets fail before publication.
The selected set becomes `reachable-or-retained-style-ids`: retained entries remain
`staticReachability: unreachable` and `usageState: dead`, but are emitted whole with
`removalDisposition: policy-retained`. Usage Analysis, Asset Plan, and Project Index use schema 2
for that selection; no-retention modes retain their schema-1 shape. Observation never becomes a
retention instruction. See [usage analysis schemas 1 and 2](./usage-analysis-schema-1.md) and the
[usage retention sidecar](./usage-retention-schema-1.md).

Observation and retention paths must resolve to existing regular UTF-8 files inside the bundle-plan
directory, cannot traverse symbolic links/reparse points, and cannot collide with another input or
output. Each option is single-use and neither file is discovered.

`--control` requires `--asset-plan`. For `B` declared bundles it adds one Source Map per CSS and one
shared `OUTPUT_DIR/pliego.tokens.json`, findings, control manifest, and receipt. The complete group
contains `3B + 5 + I + 2U` artifacts: CSS/map/style-manifest triples, Asset Plan, four fixed control
artifacts, `I = 1` only when the Project Index is requested, and `U = 1` only when both usage
reports are requested. Seed/config plans publish the
canonical projection of the active registry. A schema-2 DTCG plan publishes the Resolver's complete
canonical graph and uses the selected registry for CSS and coverage. Its exact Resolver bytes enter
the input ledger as `token-resolver`; exact plan bytes, including `[theme.inputs]`, participate in
`configHash`. Textually different but semantically equivalent plans therefore do not promise the
direct CLI's canonical flag convergence. Bundle input intentionally has no DTCG `--tokens` or
`--token-input` flags.

The command reads every resolved input once into an in-memory snapshot, resolves the complete
pre-pruning universe and evidence-bound selection, compiles every bundle before publishing any
output, and publishes the complete set, including opted-in Asset Plan, Project Index, and usage
analysis, as a
rollback-capable group. The capture is
stable after each read but is not a filesystem-wide transaction against unrelated concurrent
writers. `--check` instead performs a byte-exact, read-only comparison against every expected CSS
and manifest file plus the Asset Plan, Project Index, and both usage reports when requested. It does not
delete stale assets.

Bundle ownership remains explicit: the plan does not infer Cargo or PliegoRS reachability, route
semantics, shared extraction, or preload links. The optional reachability sidecar supplies exact
component/route/island metadata without changing either plan schema. Each per-bundle graph contains only
the emitted semantic declarations and direct token dependencies from styles/origins compiled into
that bundle; site paths must match the plan-relative source labels. Pruning does not create or
repartition bundles. See the [declarative bundle-plan contract](./bundle-plan.md),
[reachability schema 1](./reachability-schema.md), and
[manifest schema 4](./manifest-schema-4.md). Schema 5 uses the same per-bundle ownership boundary and
restarts physical ordinals at zero for each CSS/manifest pair; see
[manifest schema 5](./manifest-schema-5.md). The complete emitted load-plan contract is
[asset load-plan schemas 1 and 2](./asset-plan-schema.md); the source-to-output handoff is
[Project Index schemas 1 and 2](./project-index-schema.md).

### `check`

`check` runs collection, theme/token validation, parsing, semantic composition, emission, and
Lightning CSS validation entirely in memory using the selected format. The command accepts
`--tokens` and repeatable `--token-input` under the same selection contract. It writes no CSS or
manifest. On success it
prints a short count, the active theme ID, and the lowercase format name:

```console
cargo run -p pliego-cssc -- check --config pliego.theme.toml --source src/lib.rs
```

`check` rejects output, manifest, theme, reachability, pruning, asset-plan, and project-index options
because it has no artifact output.

### `inspect`

`inspect` performs the same validation, including explicit DTCG selection, and writes
deterministic, pretty JSON to standard output:

```console
cargo run -q -p pliego-cssc -- inspect --config pliego.theme.toml --source src/lib.rs > inspection.json
```

The document contains schema version 2, all three identity-format versions, the active theme ID and
canonical token/breakpoint lists, the
target and format contracts, exact in-memory CSS digest and byte length, findings in canonical
provenance order independent of argument ordering, and semantic styles in canonical identity-stream
order. Each style retains all of its deduplicated origins. Like `check`, it rejects CSS artifact options.

### `watch`

`watch` uses native Windows/Linux filesystem events and compiles after observing the same exact snapshot in two
captures separated by up to 100 ms. Events only schedule captures; the confirmation prevents a
transient valid prefix from being published while an editor truncates and rewrites a file:

```console
cargo run -p pliego-cssc -- watch \
  --source src --config pliego.theme.toml \
  --output app.css --theme --manifest app.manifest.json
```

It accepts one optional line-oriented `--input` and repeatable `--source` files or directories; at
least one input form and exactly one `--output` are required. Each wakeup captures one immutable,
exact-byte snapshot containing the line input, deterministically expanded Rust source tree, and
explicit theme or token configuration. Resolver selections are derived from immutable CLI arguments
against those exact bytes, then bound canonically into `configHash`; JSON is never discovered. When
schema 4 or 5 is selected, the snapshot also contains
the exact reachability bytes. Change detection and compilation consume that same
snapshot, so an editor's transient write cannot make detection observe one revision while the
compiler reads another. Added, removed, renamed, or modified Rust files therefore trigger
compilation without relying on timestamps or a truncated content hash.

Rust syntax-tree scans are cached by canonical path, logical provenance path, and exact contents.
Successful resolved candidates are cached again by theme identity. On a source edit only changed
units are reparsed and lowered; a theme-only edit reuses every `ScanReport` but invalidates semantic
IR for all source units. Global `StyleId` deduplication, origin aggregation, emission, and Lightning
CSS still run over the complete current candidate set. Watch reports discovered units, scan
hits/parses, semantic hits/lowerings, and removals to standard error. A read, parse, theme, compile,
or optimize failure does not publish the failed artifact. A write failure is retried without waiting
for another source change. Native watches cover directory trees recursively and file parents
non-recursively so atomic file replacement remains visible. A 2 s snapshot timeout detects missed
or unsupported events; on unsupported platforms or when the native backend cannot initialize,
watch reports the reason and falls back to 100 ms polling. It does not notify a browser itself.

Successful compilation is write-if-changed per destination. If existing CSS or manifest bytes are
identical, that file is not replaced; CSS and manifest can change independently when provenance
or application reachability changes without changing the retained StyleId set. With
`--prune-unreachable`, a sidecar change that changes that set also republishes CSS. This preserves
asset modification times and avoids redundant rebuild signals. For PliegoRS, use the documented [two-process development loop](../how-to/pliegors-dev-loop.md)
so `pliego dev` owns its existing browser SSE reload channel.

### `catalog`

`catalog` renders compiler-owned utility metadata together with the active theme's tokens and
breakpoints. Every example runs through the real parser, semantic lowering, emitter, and Lightning
CSS before the document is produced:

```console
# Print Markdown with conventional theme discovery
cargo run -p pliego-cssc -- catalog

# Regenerate the versioned seed reference
cargo run -p pliego-cssc -- catalog --seed --format markdown \
  --output docs/reference/generated-catalog.md

# Fail CI when the versioned file has drifted
cargo run -p pliego-cssc -- catalog --seed --format markdown \
  --check docs/reference/generated-catalog.md
```

`--format markdown|json` selects the document shape; Markdown is the default. `--output` publishes
only changed bytes through a staged writer. `--check` performs an exact read-only comparison and
fails on drift; the two options are mutually exclusive. Theme selection uses the same `--config`,
`--seed`, and conventional discovery precedence as other commands, and an output cannot overwrite
the resolved theme configuration. Each utility record includes the documented `pattern` and the
exact `matchName` used by fixed/longest-prefix lookup, plus form, domain, capabilities, example,
summary, and executable CSS. Catalog JSON is schema 3 and reports all three identity-format
versions; its required-field shape is pinned by tests,
and adding, removing, renaming, or changing the type of a required field requires a schema bump.
This JSON is the initial completion-data contract.

The versioned seed output is [the generated utility catalog](./generated-catalog.md). Change compiler
metadata and regenerate it instead of editing its generated table.

### `explain`

`explain` is the initial hover/query surface for one complete style list:

```console
pliego-cssc explain --style "hover:bg-accent/50 -mt-4" --seed
pliego-cssc explain --style "hover:bg-accent/50 -mt-4" --seed --format json
```

It executes parsing, theme-aware lowering, emission, and Lightning CSS. Text output is intended for a
developer; JSON schema 2 returns all three identity-format versions, the original and canonical
source, `themeId`, targets, `styleId`,
class name, pretty CSS, and one resolved descriptor per authored utility. Descriptor records include
the exact source fragment, zero-based `byteStart`/`byteEnd` offsets into the complete style string,
pattern, `matchName`, form, domain, capabilities, and summary. Variants and negative/important markers
remain visible in `source` while metadata identifies the underlying utility family. Explain JSON is
schema 2 and follows the same schema-bump rule. `--config`/`--seed` and conventional theme discovery
match other commands.

### `explain-cascade`

`explain-cascade` is the schema-1 bounded standard-CSS cascade query:

```console
pliego-cssc explain-cascade --input dist/app.css \
  --element 'button#save.action' --property color
pliego-cssc explain-cascade --input dist/app.css \
  --element 'button#save.action' --property color --format json
```

It parses one complete stylesheet and reports a winner only when a simple compound HTML element,
one supported longhand, top-level named layers, importance, specificity, and source order are
enough to prove it. Exact authored declaration ranges and shorthand extraction are preserved.
Potentially matching pseudo/combinator/conditional/nested/scoped/imported/runtime constructs return
the successful status `browser-required` with stable blocker codes and `winner: null`. `no-match`
means no direct declaration in this one stylesheet, not that the browser has no computed value.

The [cascade explanation reference](./cascade-explain-command.md) defines the closed property set,
precedence algorithm, schema, blocker codes, and non-claims. This command is distinct from utility
`explain`: it consumes normal CSS and does not load a PliegoCSS theme.

This is a process-level tooling contract, not an LSP transport. Editors can combine catalog JSON for
completion candidates, `explain --format json` for hover, `fmt --source --check` for formatting
diagnostics, and schema-3 manifests for source-to-class navigation. Schema-4 manifests additionally
provide the explicit application ownership and token graph; schema-5 manifests add exact navigation
to the final digest-bound CSS rules and declarations.

### `plan`, `fix --dry-run`, and `fix --apply`

`plan` turns a closed agent proposal into a deterministic read-only repair plan:

```console
pliego-cssc plan --findings findings.json --proposal proposal.json \
  --source-root . --format json > pliego.css.plan.json
```

It accepts only exact UTF-8 byte edits linked to verified, unexcepted, low-risk suggestions in the
bound FindingDocument. Every edit must stay inside that finding's exact source range, remain
non-overlapping, include all suggestion prerequisites, and fit explicit file/edit/byte budgets. The
plan binds the FindingDocument path/bytes/hash, complete source before/after snapshots, structured
edits, a deterministic byte-edit patch, and its canonical payload hash.

`fix --dry-run` provides read-only verification:

```console
pliego-cssc fix --plan pliego.css.plan.json --findings findings.json \
  --source-root . --dry-run --format json
```

It reports `ready` only when every source equals its exact before snapshot and in-memory application
reproduces the plan's after hashes. It reports `already-applied` only when every source equals the
complete after state. Stale, partially applied, missing, extra, symlinked/reparse, escaped, invalid
UTF-8, or FindingDocument-drift states fail closed. It never changes source files.

Explicit application uses a token that repeats the exact plan hash and an adjacent receipt path:

```console
pliego-cssc fix --plan pliego.css.plan.json --findings findings.json \
  --source-root . --apply --authorize sha256:<planSha256> \
  --receipt pliego.css.change-receipt.json --format json
```

The plan remains `dry-run-only`: it cannot authorize itself. The external token prevents accidental
selection of another plan, but is not a signature or identity proof. Apply re-verifies and re-reads
the complete source set under `.pliegocss-repair.lock`, then publishes exact source bytes and Change
Receipt as one staged, rollback-capable group. A repeated successful apply is a no-op and preserves
the existing same-plan receipt; receipt collisions and before-state reversion fail closed.

Change Receipt schema 1.0.0 is deliberately change-only. It binds plan, patch, FindingDocument,
before/after sources, summary, and authorization, but reports `result: checks-pending`, every
required check as `not-run`, and browser evidence as `not-collected`. Apply does not execute commands
from proposal/plan content and does not claim semantic verification.

The [repair schema reference](./repair-plan-schema.md) specifies proposal 1.0.0, plan 1.0.0,
dry-run report 1.0.0, Change Receipt 1.0.0, integrity/authorization semantics, grouped publication,
defensive limits, and explicit non-goals.

### `pliego-css-agent run-tests`, `run-browser`, and `verify`

Post-change verification lives in the dedicated agent executable so the main compiler archive keeps
its fixed size boundary. When a plan requires the Rust workspace suite, first execute the explicit
fixed runner from the project root:

```console
pliego-css-agent run-tests \
  --change-receipt pliego.css.change-receipt.json \
  --source-root . \
  --check-id tests.workspace \
  --evidence pliego.css.test-evidence.json
```

This surface runs only `cargo test --workspace --all-targets --locked --offline`, with an isolated
target directory. It records the observed Cargo/rustc version lines and host triple, rereads the
Change Receipt after sources, root `Cargo.toml`, and root `Cargo.lock` under the repair lock, and
publishes canonical passed/failed evidence. It accepts no
program, argument, shell, or environment extension. The evidence is then identity-bound by the
check policy.

When browser behavior is required, the second explicit runner exposes one fixed local profile:

```console
pliego-css-agent run-browser \
  --change-receipt pliego.css.change-receipt.json \
  --source-root . \
  --check-id browser.pliegors \
  --evidence pliego.css.browser-evidence.json
```

It rebuilds the exact pinned PliegoRS fixture and launches local headless Chromium through CDP. The
profile requires stable document/island/button/value objects, state and text `15→20`, stable classes,
one typed event, preload reuse, exact client requests, WASM readiness, and no relevant browser/server
errors. A caller cannot replace the script, URL, selectors, assertions, or browser arguments. The
runner binds the Change Receipt plus eight fixed profile inputs, including the pinned PliegoRS
contract and gate scripts, before the static verifier runs:

```console
pliego-css-agent verify \
  --change-receipt pliego.css.change-receipt.json \
  --check-policy pliego.css.check-policy.json \
  --source-root . \
  --receipt pliego.css.verification-receipt.json \
  --format json
```

The latest schema-1.4 policy admits only built-in `standard-css-audit`, `token-graph-integrity`,
`css-budget-audit`, `test-suite-evidence`, and `browser-evidence` definitions over exact changed sources. CSS and budget
checks require `modern|baseline-widely`; token/test/browser evidence requires `none`. A budget check binds one
canonical policy plus optional sorted package/route subjects. A test check binds the canonical
runner evidence and exact root Cargo manifest/lockfile; it is additive and cannot replace
source-specific audit coverage. A browser check binds one canonical browser evidence artifact and
the exact fixed profile inputs; it is also additive. Canonical 1.0.0/1.1.0/1.2.0/1.3.0 policy and receipt evidence remains
readable under its original kind limits. The policy contains no executable, arguments, shell,
environment, or package-script surface. `verify` revalidates after sources and every supplementary
input under the repair lock, runs only in-process evaluators, and never launches Cargo, Node, or a browser.
For every check it also publishes the complete canonical FindingDocument beside the receipt as
`pliego-css-findings-<sha256(asciiLower(receiptLogicalPath) + NUL + checkId)>.json`. Finding artifacts are
linked create-if-absent before the receipt; a publication failure removes newly linked evidence.

Exit `0` means every check passed and browser evidence was either not required or exactly
`required-passed`. A valid `failed` or browser-required `blocked` receipt is still published and exits
`1`; invocation, path, drift, or tool errors exit `2`. See the complete
[repair verification schema](./repair-verification-schema.md).

### `fmt`

`fmt` uses the real syntax parser and canonical printer. In `--style` mode it preserves candidate
payloads, negative and important markers, and ordered variant chains exactly; it only normalizes
top-level whitespace to one ASCII space between utilities:

```console
pliego-cssc fmt --style "  dark:hover:-mt-[2rem]!   [&>p]:block  "
# dark:hover:-mt-[2rem]! [&>p]:block

pliego-cssc fmt --input styles.txt --output formatted.txt
pliego-cssc fmt --input styles.txt --check
pliego-cssc fmt --source src --check
pliego-cssc fmt --source src --apply
```

Choose either repeatable `--style` or one `--input`, not both. `--output` and `--check` are mutually
exclusive, and an output cannot alias the input. Line-oriented documents retain blank lines, reserve
first-non-whitespace `#` for comments, normalize comments to trimmed text, preserve escaped candidate
whitespace through the parser, use LF endings, and end with one newline when non-empty. CRLF input is
accepted; isolated carriage returns are rejected. `--check` compares exact canonical bytes and exits
unsuccessfully on drift.

`fmt` is syntax-only: a well-shaped unknown utility passed through `--style` remains printable. In a
line document, a leading `#` is a comment rather than a utility. Use `pliego-cssc check` with the
intended theme as the semantic linter. Rust source remains read-only unless `--apply` is explicit.

Repeatable `--source` requires exactly one of `--check` or `--apply`. Both traverse Rust files with
the same rules as compilation and inspect every `pc!` literal plus the base and each branch literal
of `pcx!`. Check mode emits stable `FMT001` findings with file, line, column, byte range, actual text,
and canonical replacement without changing a file.

Apply mode first snapshots every input, validates non-overlapping complete literal-token ranges,
escapes each canonical decoded value as valid Rust, reparses and rescans every complete result, then
locks and revalidates all snapshots before any write. Changed files publish as one rollback-capable
group through same-directory temporary and backup files while preserving permissions. A concurrent
source change, duplicate/case-folded destination, symlink, overlapping/stale range, post-rewrite
scanner error, or publication failure leaves the original group intact. The advisory lock prevents
cooperating PliegoCSS writers from racing; termination between filesystem renames is not claimed to
be crash-atomic. Constructed/non-literal macro inputs remain scanner errors.

## Reachability pruning

`--prune-unreachable` is an explicit artifact option for `compile`/`build`, `watch`, and `bundle`.
It requires a strict reachability sidecar and therefore `--manifest-version 4` or `5` plus manifest
output. Omitting the flag preserves the established CSS and manifest compatibility vectors byte for
byte.

The root component set is the union of every component referenced by any route and any island in the
sidecar. The compiler still verifies every compiled origin against an exact sidecar site, including
origins whose components are not roots. An exact site is reachable when any component that owns it
is in the root set. After canonical identity deduplication, a StyleId is retained when any one of its
origins is reachable; the retained style keeps all normalized origins rather than rewriting its
provenance to only the reachable subset.

Pruning removes complete StyleId output, not individual declarations from a retained style. The
schema-4/schema-5 graph emits semantic declarations and direct token nodes only for retained styles;
schema 5 traces only the resulting physical CSS. With an empty route/island root set, CSS is one
newline without theme output and `:root{}` plus one newline when theme output was requested. Under
pruning, `--theme` and bundle `emit-theme = true` emit only variable-backed tokens directly consumed
by retained semantic styles. Without pruning, the complete supported block remains byte-compatible.
Authored CSS `var(...)` consumers outside semantic styles are not discovered.

For `bundle`, classification runs independently over only the styles and origins already assigned to
each bundle by the plan. All bundles use the same union root set, and the flag neither derives source
partitions nor creates route/island assets.

Manifest schemas 4 and 5 describe the selected result through `styles`, graph nodes, CSS length, and
digest; they do not add a field recording how that result was selected. Consumers cannot infer from a
manifest alone whether a smaller style set came from pruning or from a narrower source input. An
opted-in asset load plan records this external choice as `all-compiled` or `reachable-style-ids`.

## Manifest schemas

Schema 3 remains the default compatibility contract. Schema 4 is an explicit, fail-closed semantic
ownership graph. Schema 5 adds a fail-closed physical trace over the exact final CSS. Both require
their matching `--manifest-version` plus `--reachability`; selecting either without pruning does not
change CSS, StyleId, class, ThemeId, targets, format, digest, or byte count. With pruning enabled,
schemas 4 and 5 remain CSS-identical to each other for the same inputs and flag. See the complete
[manifest schema-4 reference](./manifest-schema-4.md),
[manifest schema-5 reference](./manifest-schema-5.md), and
[reachability input schema](./reachability-schema.md).

### Manifest schema 3

For example, `compile --style "flex gap-4" --seed --theme --output pliego.css --manifest
pliego.manifest.json` writes pretty JSON with one trailing newline:

```json
{
  "schemaVersion": 3,
  "styleIdFormatVersion": 2,
  "classNameFormatVersion": 1,
  "themeIdFormatVersion": 3,
  "themeId": "b3d5ad77175995c2b8f51ef7c0d41991",
  "targets": "modern",
  "format": "minified",
  "cssSha256": "d208d2960bd28fb29354b1a89c74c106686c8a8a2d71a9cc7cc9971eb2e97070",
  "cssBytes": 424,
  "styles": [
    {
      "styleId": "70cb04ef9bf9621f5826351f1778f68e",
      "className": "pc_6oe73ec16rbb7ublcoa3bpzf2",
      "origins": [
        {
          "source": "flex gap-4",
          "file": null,
          "byteStart": null,
          "byteEnd": null,
          "macroKind": "cli",
          "reason": "explicit-style-1"
        }
      ]
    }
  ]
}
```

`styleId` and `themeId` are complete 128-bit values encoded as 32 lowercase hexadecimal digits.
The three format-version fields are required and identify how those IDs and classes must be
interpreted. `className` is the style ID's lowercase base-36 CSS encoding. Styles are
deterministically ordered by their canonical identity streams, not by hash value. Equal streams
produce one entry; a distinct stream with the same compact ID is a fatal collision. Origins and
top-level inspection findings are
deduplicated and sorted by `(file, byteStart, byteEnd, macroKind, reason, source)`, making JSON bytes
independent of argument order.

`cssSha256` is the SHA-256 digest of the exact emitted CSS bytes, encoded as 64 lowercase hexadecimal
characters. `cssBytes` is their exact byte length, including the final newline. Consumers that read
CSS and manifest as one logical artifact must verify both fields before accepting the pair. `format`
is `minified` or `pretty` and identifies which Lightning CSS printer profile produced those bytes.

For scanner findings, `file`, `byteStart`, and `byteEnd` identify the exact macro invocation. A
`pcx` reason identifies the selected Cartesian path, for example
`reachable-composition[c0:b1,c1:b0]`. Explicit CLI inputs use `macroKind: "cli"` and null file/range;
line-oriented findings identify their file and exact trimmed line range.

### Manifest schema 4

Schema 4 retains every schema-3 top-level field and style record, then adds required `graph` schema
1. Its canonical nodes represent semantic declarations, directly referenced tokens, components,
routes, and islands. Typed edges support forward and reverse traversal through
route/island → component → semantic declaration → token.

A semantic declaration is one canonical typed-IR assignment, not necessarily one final physical CSS
property after emission and Lightning CSS. Every compiled origin must match an exact component site
from the sidecar or compilation fails; partially owned origin graphs are never emitted. Framework
topology completeness remains a separate adapter attestation. Equivalent input and sidecar ordering
produces identical bytes.

### Manifest schema 5

Schema 5 retains every schema-4 semantic node and edge exactly, upgrades the nested graph to schema
2, and adds final physical rule/declaration nodes. Every physical interval is a half-open UTF-8 byte
range into the digest-bound adjacent CSS. Contribution edges are many-to-many, generated support
declarations are explicit, and theme output uses the synthetic `producer:theme` node.

The compiler carries canonical assignment lineage from the emitter and reconciles it against the
final Lightning CSS AST and serialization. Missing producers, unsupported final rule kinds,
ambiguous structure, invalid ranges, or incomplete coverage fail before output staging. Physical IDs
are ordinal and artifact-local; verify `cssBytes` and `cssSha256` before using them.

## Artifact publication

CSS, manifest, and opted-in asset-plan/control paths may not alias each other or any explicit input,
source, configuration, or reachability path after lexical normalization and filesystem canonicalization.
After conventional theme discovery, the resolved configuration is checked again so an output cannot
overwrite an auto-discovered theme. Portable path comparisons are ASCII case-insensitive on every
host so a Linux-approved plan cannot alias inputs when replayed on Windows. Artifact
destinations cannot use a `.rs` extension, cannot already be a symbolic link/reparse point, and
cannot use the reserved
`.<name>.pliego.lock` or `.<name>.pliego-*.(tmp|bak)` coordination namespace.

Compilation and Lightning CSS processing finish before publication begins. Before comparing any
existing output, the CLI takes advisory locks named `.<destination>.pliego.lock` for every requested
CSS, manifest, and asset-plan destination, including a byte-identical destination. This keeps a
metadata-only update from racing another writer's artifact group. Every changed destination is then written
completely to a create-new sibling temporary file and synchronized. Cooperating PliegoCSS processes
therefore cannot publish the same physical path concurrently. Existing destinations then move to
unique sibling backups. Lock identity canonicalizes the destination parent but retains the original
final component, so that temporary move cannot redirect a concurrent writer to the backup name. If a
prepared rename reports failure in the same process, the CLI attempts to
remove partial files and restore every previous destination. A rollback failure is returned with the
affected destination and backup paths instead of being reported as success.

CSS and manifest remain separate paths rather than one filesystem transaction. The CLI does not
promise gap-free replacement or crash durability: terminating the process between renames can leave
destinations absent and `*.bak` files requiring manual restoration; terminating during preparation can
leave sibling `*.tmp` files. A backup cleanup failure emits a warning and retains that backup.
Persistent empty `.pliego.lock` files are intentional; the OS lock, not file existence, owns
exclusion. Consumers must still enforce the `cssSha256` and `cssBytes` integrity contract. Advisory
locking also assumes a filesystem that implements the host lock primitive.

For `bundle`, all named CSS/manifest destinations, requested Source Maps, fixed token graph, Asset
Plan, optional Project Index, findings, control manifest, and receipt participate in one publication
group after every bundle has compiled and the plan has validated successfully. The same
handled-failure rollback and crash boundary apply to the whole group; this is not a claim of
multi-file transactional or crash-atomic replacement.

## Current limits

- There is no Cargo dependency graph, glob syntax, stdin, generated-code discovery, automatic route
  split, critical CSS, or inferred reachability pass. Schema-1/schema-2 bundle plans partition only the
  source sets declared by the application; schema-4/schema-5 ownership comes from an explicit adapter
  sidecar. The asset load plan projects that supplied topology but does not collect it, infer
  route-to-island occurrence, or generate public URLs, HTML links, or preload instructions.
- Watch mode polls line-oriented inputs, Rust source trees, theme configuration, and an optional
  reachability sidecar; it does not emit
  browser hot-reload events or use filesystem notifications.
- Theme CSS currently emits the registry token kinds supported by the emitter's custom-property
  contract; `--prune-unreachable` does not filter that block. Other tokens still affect lowering and
  identity.
- Identity and class formats are explicitly versioned. The `0.1.0-rc.3` vectors are a
  published prerelease contract rather than a final `0.1.0` stability promise. See the
  [StyleId format-2 reference](./style-id-format-v2.md).
- After publication, the CLI may be installed from exact crates.io version `0.1.0-rc.3` or from an
  exact checkout revision. Once installed, `pliego-cssc` is standalone and can run from an application directory; it does not
  require that application to belong to the PliegoCSS Cargo workspace.
