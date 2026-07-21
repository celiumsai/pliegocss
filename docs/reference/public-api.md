# Public API candidate

Status: **application, build-macro, and adapter surfaces published at `0.1.0-rc.2`; the
Cargo DTCG, ownership, usage-sidecar, and application-topology collector bridges are covered by the
Rust 1.85 downstream smoke; this remains a prerelease contract**

PliegoCSS must package several crates because Cargo builds the procedural macro, theme bridge,
parser, compiler, and CLI as a version-locked graph. Registry publication does not make
every `pub` item in that graph an application API. All internal dependencies use exact versions and
one source revision.

## Supported application surface

The intended `0.1.x` Rust application API is deliberately small:

```rust
use pliego_css::{Style, StyleId, pc, pcx};
```

- `pc!` validates one visible utility literal and returns `Style`.
- `pcx!` validates every statically enumerable `if`/`match` branch, evaluates each selector once,
  and returns the selected precompiled `Style`.
- `Style::EMPTY`, `Style::id`, `Style::class_name`, and `Style::is_empty` are supported.
- `Style` remains `Copy`, `Clone`, `Debug`, `Eq`, `Hash`, `Send`, `Sync`, `Unpin`, `'static`,
  `Display`, and convertible into `String`.
- `StyleId::get`, `StyleId::is_unresolved`, and `StyleId::to_class_name` are supported.
- `StyleId` remains a transparent 128-bit, copyable, hashable, orderable, `Send`, `Sync`, `Unpin`,
  `'static` value. Application code obtains it from a validated `Style`; integer construction is
  not part of the application API.
- `Style::EMPTY`, `Style::id`, `Style::is_empty`, `StyleId::get`, and
  `StyleId::is_unresolved` remain usable in constant evaluation.
- Cargo dependency renames are supported: reexported macros resolve the facade's actual dependency
  name instead of assuming `pliego_css`.

`Style::EMPTY` is the conditional no-style handle. It carries the unresolved zero sentinel, renders
as an empty string, and has no emitted CSS selector. Its raw `StyleId` still encodes mechanically as
`pc_0`; render a `Style` through `Style::class_name` or `Display`, not by formatting the empty ID.

`Style` has no integer constructor. The hygienic `pc!` and `pcx!` expansions construct handles through
the exact-version facade internals after procedural validation; application code can only obtain a
non-empty `Style` from those macros. This remains compatible with Cargo dependency renames because
the declarative facade resolves helpers through `$crate`.

## Custom-theme bridge

The supported build-script entrypoint is:

```rust,no_run
fn main() {
    pliego_css_build::theme!("pliego.theme.toml");
}
```

The same macro selects one DTCG Resolver permutation without changing the TOML form:

```rust,no_run
fn main() {
    pliego_css_build::theme!(
        tokens = "product.resolver.json",
        inputs = {
            "appearance" => "dark",
        },
    );
}
```

`theme!` returns `()` after exporting the canonical theme artifact to the Rust compilation, and
panics with a build error when configuration fails. Relative paths use `CARGO_MANIFEST_DIR`; empty
DTCG inputs use Resolver defaults; modifier/context entries are string literals; and exact or
case-fold duplicate modifiers fail closed. Call the macro once per package. The DTCG artifact
contains only the selected registry; CSS generation must repeat the same Resolver and inputs through
CLI `--tokens`/`--token-input`, whose controlled mode publishes the complete graph.
`THEME_PATH_ENV` and `THEME_ID_ENV` are supported names for the versioned artifact handoff. Exact
signatures of `configure_theme`, `write_theme_artifact`, the DTCG configuration helpers,
`ThemeArtifact`, and `BuildError` are hidden implementation hooks and are not promoted by the macro
contract.

The non-default `pliego-css-build/usage-artifacts` feature exposes only the bounded reachability,
selection, identifier, and hashing contracts needed by `pliego-css-usage`; it does not pull the CSS
optimizer. The broader `artifacts` feature adds CSS analysis and manifest graph construction for the
CLI. Both are exact-version tooling surfaces, not supported application/build-script APIs, and
ordinary `theme!` users do not enable them.

## Adapter ownership bridge

Framework adapters may depend directly on the exact-version `pliego-css-ownership` crate. The
supported producer/consumer path is deliberately read-only apart from the returned JSON bytes:

```rust,no_run
use pliego_css_ownership::{
    BundlePackageInput, RouteCompositionInput, build_ownership_document, parse_asset_plan,
    parse_ownership,
};

# fn example(asset_plan_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
let plan = parse_asset_plan(asset_plan_bytes)?;
let mappings = [BundlePackageInput::new("app", "example-app")];
let compositions = [RouteCompositionInput::new("route:home", &[])];
let sidecar = build_ownership_document(&plan, &mappings, &compositions)?;
let ownership = parse_ownership(&sidecar, &plan)?;
assert_eq!(ownership.packages()[0].package_id(), "example-app");
# Ok(())
# }
```

The promoted adapter API consists of:

- `parse_asset_plan`, `build_ownership_document`, and `parse_ownership`;
- `BundlePackageInput::new` and its `bundle_id`/`package_id` getters;
- `RouteCompositionInput::new` and its `route_id`/`island_ids` getters;
- the immutable `AssetPlan`, `AssetBundle`, `AssetRoute`, and `AssetIsland` views and their published
  identity, file, selection, hash, route/path, island/name, and bundle-list getters;
- the immutable `Ownership`, `PackageOwnership`, and `ComposedRoute` views and their package, route,
  path, island-list, and deduplicated bundle-list getters;
- `AssetRuleSelection` values returned by `AssetPlan::rule_selection` and `OwnershipError` returned
  by the three fallible entrypoints.

`parse_asset_plan` proves the closed document structure and internal references, not its provenance.
An adapter must regenerate the Asset Plan from its exact adjacent CSS/manifest inputs, or receive
equivalent trusted evidence, before publishing a sidecar. The builder binds the exact plan byte count
and SHA-256, emits canonical two-space JSON with one final LF, and reparses its own output. Composed
routes are overlapping deployment views, not attribution partitions. Mutable schema DTOs, unchecked
constructors, and direct serialization internals are not part of this bridge.

## Usage-observation and retention adapter bridge

Observation and retention producers may depend directly on the exact-version `pliego-css-usage`
crate. The promoted adapter surface builds and validates closed, canonical sidecars without
granting an observation producer authority to classify CSS as dead or letting policy rewrite
reachability evidence:

```rust,no_run
use pliego_css_usage::{
    UsageObservationCoverage, UsageObservationInput, UsageObservationScopeInput,
    UsageObservedStyleInput, UsageRetentionEntryInput, UsageRetentionInput,
    build_usage_observation, build_usage_retention, parse_usage_observation,
    parse_usage_retention,
};

# fn example() -> Result<(), String> {
let input = UsageObservationInput::new(
    "0".repeat(64),
    "1".repeat(64),
    UsageObservationCoverage::Sampled,
    "browser-matrix",
    "1.0.0",
    UsageObservationScopeInput::new(
        vec!["/".into()], vec![], vec!["light".into()],
        vec!["chromium-138".into()], vec!["1280x720".into()], vec!["default".into()],
    ),
    vec![],
    vec![UsageObservedStyleInput::new(
        "application",
        "11111111111111111111111111111111",
    )],
);
let bytes = build_usage_observation(input)?;
let _verified = parse_usage_observation(&bytes)?;

let retention = build_usage_retention(UsageRetentionInput::new(
    "0".repeat(64),
    "1".repeat(64),
    vec![UsageRetentionEntryInput::new(
        "external-email-renderer",
        "email",
        "22222222222222222222222222222222",
        "Consumed by the external renderer outside application routing.",
    )],
))?;
let _verified_retention = parse_usage_retention(&retention)?;
# Ok(())
# }
```

The supported producer API is the observation and retention builders/parsers plus their input
constructors and `UsageObservationCoverage`. The analysis builder, `parse_usage_analysis`,
`verify_usage_analysis`, compiler-origin inputs, the reexported usage `AssetRuleSelection`, selection
checks, and mutable schema internals
remain exact-version tooling APIs. The parser proves self-contained structure; the verifier
rederives the report from exact compiler origins and sidecar bytes. Both sidecars bind
the pre-pruning universe and exact reachability bytes; sampled absence never proves deadness, while
retention can name only an exact bundle-qualified StyleId that the compiler independently proves
structurally unreachable.

## Application-topology collector bridge

Framework adapters may use the selected `pliego-css-source` collector surface to turn typed
framework topology into canonical reachability schema 1:

```rust,no_run
use pliego_css_source::{
    ApplicationComponent, ApplicationIsland, ApplicationRoute, ApplicationTopology,
};

# fn example() -> Result<(), Box<dyn std::error::Error>> {
let topology = ApplicationTopology::new()
    .source_root("src/styles")
    .component(
        ApplicationComponent::new("app::home")
            .source_unit("src/styles/home.rs"),
    )
    .route(ApplicationRoute::new("home", "/").component("app::home"))
    .island(ApplicationIsland::new("counter", "counter"));
let collected = topology.collect(".")?;
assert!(collected.as_bytes().ends_with(b"\n"));
# Ok(())
# }
```

The promoted bridge consists of the four topology/component/route/island builders, their `new`,
`source_root`, `source_unit`, `site`, `component`, `route`, and `island` methods;
`ApplicationTopology::collect`; `CollectedReachability` byte/count accessors; and `CollectError`.
It is an exact-version adapter surface until an RC activates the compatibility promise. The bridge
attests only the declared roots and graph: it does not discover the product Cargo graph or framework
registry, infer identities from paths, or derive bundle partitions. Scanner DTOs and standalone
scanner functions remain advanced tooling rather than promoted adapter API.

## Migration-inventory tooling bridge

Exact-version audit and migration tooling may use the selected `pliego-css-source` inventory
producer without running the source toolchain:

```rust
use pliego_css_source::{MigrationSourceKind, inventory_migration_source};

# fn example() -> Result<(), Box<dyn std::error::Error>> {
let inventory = inventory_migration_source(
    MigrationSourceKind::Sass,
    "src/app.scss",
    "$brand: red;\n.button { color: $brand; }\n",
)?;
assert_eq!(inventory.source_sha256().len(), 64);
# Ok(())
# }
```

The exact-version bridge consists of `MIGRATION_INVENTORY_SCHEMA_VERSION`,
`inventory_migration_source`, `inventory_migration_file`, `MigrationSourceKind`, `MigrationDisposition`,
`MigrationPreflightReliance`, the immutable `MigrationInventory`/`MigrationConstruct` getters, and
`MigrationInventoryError`. The exact-version project layer additionally exposes `MigrationProject`,
`MigrationProjectSource`, `MigrationProjectInventory`, `MigrationDependency`,
`MigrationDependencyKind`, and `MigrationDependencyResolution` for a confirmed, canonically ordered
declared-file snapshot. It emits canonical schema-1 JSON, records dynamic/unsupported syntax, and
derives conservative dependency observations for Sass module/import seams, CSS import/reference,
Tailwind config/plugin/source seams, and CSS Modules/ICSS composition. Exact supported local targets fail closed unless declared with the
expected kind; Sass resolves declared relative files, partials, indexes, and legacy import-only
files with ambiguity rejection, while configured load paths/importers and dynamic resolution remain
explicit without toolchain execution;
`MigrationProject::from_json` parses a bounded, closed, reviewable schema-1 declaration without
reading source files; `MigrationProject::from_file` adds safe regular-file loading without following
link-like path components;
the consumer layer additionally exposes `MigrationProjectConsumer`, `MigrationConsumerKind`,
`MigrationConsumerInventory`, `MigrationConsumerObservation`,
`MigrationConsumerObservationKind`, `inventory_migration_consumer_source`, and
`inventory_migration_consumer_file` for declared CSS Modules JS/TS consumers, including explicit
`BindingAlias` and `DestructuredClass` observations and the `alias()` getter;
`discover_migration_project` adds deterministic, bounded, no-follow typed discovery that returns an
uncollected `MigrationProject`; the auxiliary layer
exposes `MigrationProjectAuxiliary`, `MigrationAuxiliaryKind`, `MigrationAuxiliaryInventory`,
`MigrationAuxiliaryObservation`, `MigrationAuxiliaryObservationKind`,
`inventory_migration_auxiliary_source`, and `inventory_migration_auxiliary_file` for exact Tailwind
config/plugin/template identity, typed relative seam linking, and conservative template candidates;
`inventory_migration_file` additionally enforces a bounded regular project-relative file and rejects
symlink/reparse-point components before reading. The bridge does not execute
Sass/Tailwind/plugins/configs, crawl transitive dependencies outside the bounded discovery set, or
promise a codemod.
This tooling bridge is not part of the minimal application/build-script SemVer surface.

## CLI and document surface

The candidate process API includes the one-shot `compile`/`build`, `check`, `inspect`, `bundle`,
`catalog`, `migration-inventory`, `migration-project-inventory`, `explain`, `explain-cascade`, `plan`, `fix --dry-run`, explicitly authorized `fix
--apply`, and `fmt` commands, their
exit behavior, and the numbered documents they emit.
Default manifest 3, opt-in manifest 4 with graph 1/reachability 1, opt-in manifest 5 with graph 2 and
physical rule/declaration ID formats 1, inspection 2, catalog 3, utility explain 2, cascade explain
1, repair proposal/plan/dry-run report/Change Receipt 1.0.0, check policy/Verification Receipt
1.4.0 with canonical 1.0.0/1.1.0/1.2.0/1.3.0 read support, fixed test/browser evidence 1.0.0,
diagnostic 1, bundle-plan 1/2, usage
observation/retention 1, usage analysis 1/2, Asset Plan 1/2, Project Index
1/2, and ownership 1 are covered by exact shape tests. Human error prose is not a byte-stable API.
`watch` remains experimental until its
long-lived event and hot-reload contract is designed.

Single-value CLI options are single-use even when repeated with the same value. Repeatability is
limited to options explicitly marked repeatable, such as `--style`, `--source`, and `--compose`.

## Repair-tooling boundary

Exact-version tooling may use `pliego-css-agent` to parse a closed proposal, build a deterministic
plan, verify its bound FindingDocument/source state, prepare exact after bytes, parse a Change
Receipt, and publish the selected repair through the checked path/lock/rollback boundary. The schema
constants, `RepairTool`, `RepairChangeBudget`, immutable document/report/application values,
`parse_repair_proposal`, `build_repair_plan`, `parse_repair_plan`, `verify_repair_plan`,
`prepare_repair_application`, `parse_repair_change_receipt`, `resolve_repair_receipt_path`,
`publish_repair_application_checked`, `apply_repair_plan_checked`, `parse_repair_check_policy`,
`execute_repair_verification`, `RepairVerificationInputs`,
`execute_repair_verification_with_inputs`,
`verify_repair_change_checked`, `RepairTestProfile`, `RepairTestEvidenceResult`,
`RepairTestToolchainIdentity`, `RepairTestEvidence`, `PublishedRepairTestEvidence`, `parse_repair_test_evidence`,
`run_repair_rust_tests_checked`, `RepairBrowserProfile`, `RepairBrowserEvidenceResult`,
`RepairBrowserRunnerIdentity`, `RepairBrowserIdentity`, `RepairBrowserNodeIdentity`,
`RepairBrowserObservation`, `RepairBrowserEvidence`, `PublishedRepairBrowserEvidence`,
`parse_repair_browser_evidence`, `run_repair_pliegors_browser_checked`,
`parse_repair_verification_receipt`, and `RepairContractError` are the
selected exact-version implementation boundary. Lower-level unchecked publisher composition and CLI
parsing DTOs are public only so `pliego-cssc` can remain thin and are not promoted consumer APIs.

The supporting `pliego-css-build/usage-artifacts` feature exposes read-only Finding accessors for
code, verification, exception state, exact source range, and ranked suggestion
risk/scope/prerequisites. Those accessors let the agent core prove edit authority; they do not grant
consumers permission to construct or apply arbitrary findings. This repair-tooling boundary remains
exact-version until an RC freezes it. Apply authorization must exactly repeat the plan hash and is a
selection guard, not authentication. Change Receipt 1.0.0 is change-only: required checks remain
`not-run` and browser evidence remains `not-collected`. The separate Verification Receipt can report
`passed` only for exact built-in checks plus either non-required browser evidence or an
identity-bound fixed-profile browser pass. Both fixed runners are explicit process boundaries; no
plan/policy can supply their programs, scripts, URLs, selectors, or arguments. Checked verification also returns every complete adjacent
FindingDocument with its check ID, deterministic logical file, exact bytes, and create status.

## Versioned formats, not general Rust APIs

The StyleId, class, ThemeId, theme-binary, configuration, and JSON/TOML format versions remain
machine-enforced interoperability contracts. Their stability does not promote constructors or
mutable IR structures from implementation crates into the application surface. See
[Compatibility](./compatibility.md) and [StyleId format 2](./style-id-format-v2.md).

## Implementation and advanced crates

The following crates are packaged in exact-version lockstep but remain
implementation or advanced tooling surfaces for the candidate:

- `pliego-css-cascade`
- `pliego-css-agent` outside the selected schema implementation boundary
- `pliego-css-ir`
- `pliego-css-parser`
- `pliego-css-compiler`
- `pliego-css-theme`
- `pliego-css-config`
- `pliego-css-source` outside the selected application-topology collector and migration-inventory
  tooling bridges
- direct use of `pliego-css-macros`

They expose `pub` items because sibling packages need them. Their mutable IR, partial parser stages,
catalog model, binary encoder/decoder, scanner DTOs/functions, and composition helpers are not
covered by the minimal `0.1.x` application promise unless a later RC document explicitly promotes
an item.
Persisted bytes and schemas named above remain versioned regardless of this Rust-API classification.

## Change policy

Before the first RC, a change to this page requires updating the downstream public-API fixture and
the changelog. After the RC:

- breaking the supported application, build-macro, or adapter surface requires at least `0.2.0`;
- breaking a persisted format also requires its own format/schema bump and migration;
- internal crates continue to move only as one exact-version compatibility unit;
- adding a supported API is allowed only with documentation and downstream compile coverage.

This selection closes the scope decision; it does not authorize publishing `0.1.0`, uploading crates,
or claiming the hosted, multi-browser, Cloudflare, PliegoRS clean-clone, and onboarding gates are green.
