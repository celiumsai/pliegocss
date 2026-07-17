use core::fmt::Debug;
use core::hash::Hash;

use css::{Style, StyleId, pc, pcx};
use pliego_css_agent::{
    PublishedRepairBrowserEvidence, PublishedRepairFindingDocument,
    REPAIR_BROWSER_EVIDENCE_SCHEMA_VERSION, REPAIR_CHANGE_RECEIPT_SCHEMA_VERSION,
    REPAIR_CHECK_POLICY_SCHEMA_VERSION, REPAIR_DRY_RUN_SCHEMA_VERSION, REPAIR_PLAN_SCHEMA_VERSION,
    REPAIR_PROPOSAL_SCHEMA_VERSION, REPAIR_TEST_EVIDENCE_SCHEMA_VERSION,
    REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION, RepairBrowserEvidence,
    RepairBrowserEvidenceResult, RepairBrowserIdentity, RepairBrowserObservation,
    RepairBrowserProfile, RepairBrowserRunnerIdentity, RepairChangeBudget, RepairCheckKind,
    RepairContractError, RepairTestEvidence, RepairTestEvidenceResult, RepairTestProfile,
    RepairTestToolchainIdentity, RepairTool, RepairVerificationInputs,
    execute_repair_verification_with_inputs, parse_repair_browser_evidence,
    parse_repair_change_receipt, parse_repair_check_policy, parse_repair_plan,
    parse_repair_proposal, parse_repair_test_evidence, parse_repair_verification_receipt,
    run_repair_pliegors_browser_checked, run_repair_rust_tests_checked,
};
use pliego_css_ownership::{
    AssetBundle, AssetIsland, AssetPlan, AssetRoute,
    AssetRuleSelection as OwnershipRuleSelection, BundlePackageInput, ComposedRoute, Ownership,
    OwnershipError, PackageOwnership, RouteCompositionInput,
    build_ownership_document, parse_asset_plan, parse_ownership,
};
use pliego_css_source::{
    MIGRATION_INVENTORY_SCHEMA_VERSION, ApplicationComponent, ApplicationRoute,
    ApplicationTopology, MigrationAuxiliaryInventory, MigrationAuxiliaryKind,
    MigrationAuxiliaryObservation, MigrationAuxiliaryObservationKind, MigrationConsumerInventory,
    MigrationConsumerKind,
    MigrationConsumerObservation, MigrationConsumerObservationKind, MigrationDependency,
    MigrationDependencyKind, MigrationDependencyResolution, MigrationDisposition,
    MigrationInventory, MigrationInventoryError, MigrationPreflightReliance, MigrationProject,
    MigrationProjectAuxiliary, MigrationProjectConsumer, MigrationProjectInventory,
    MigrationProjectSource, MigrationSourceKind, inventory_migration_auxiliary_file,
    inventory_migration_auxiliary_source, inventory_migration_consumer_file,
    inventory_migration_consumer_source, inventory_migration_file, inventory_migration_source,
};
use pliego_css_usage::{
    AssetRuleSelection as UsageRuleSelection, UsageCandidateInput, UsageObservationCoverage,
    UsageObservationInput, UsageObservationScopeInput, UsageObservedStyleInput,
    UsageRetentionEntryInput, UsageRetentionInput, build_usage_analysis, build_usage_observation,
    build_usage_retention, collect_usage_style_inputs, parse_usage_observation,
    parse_usage_retention, verify_usage_analysis,
};

const OWNERSHIP_ASSET_PLAN: &[u8] = br#"{
  "schemaVersion": 1,
  "manifestSchemaVersion": 4,
  "graphSchemaVersion": 1,
  "ruleSelection": "all-compiled",
  "originCoverage": "compiler-verified-complete",
  "applicationCoverage": "adapter-attested-complete",
  "styleIdFormatVersion": 2,
  "classNameFormatVersion": 1,
  "themeIdFormatVersion": 1,
  "themeId": "00000000000000000000000000000000",
  "targets": "none",
  "format": "minified",
  "bundles": [{
    "id": "app",
    "cssFile": "app.css",
    "manifestFile": "app.manifest.json",
    "emitsTheme": true,
    "cssBytes": 0,
    "cssSha256": "0000000000000000000000000000000000000000000000000000000000000000",
    "manifestBytes": 0,
    "manifestSha256": "0000000000000000000000000000000000000000000000000000000000000000"
  }],
  "routes": [{"id": "route:home", "path": "/", "bundles": ["app"]}],
  "islands": [{"id": "island:counter", "name": "counter", "bundles": ["app"]}]
}"#;

const FIXED_STYLE: Style = pc!("flex gap-4 md:grid");
const EMPTY_ID: StyleId = Style::EMPTY.id();
const EMPTY_ID_BITS: u128 = EMPTY_ID.get();
const EMPTY_STYLE_IS_EMPTY: bool = Style::EMPTY.is_empty();
const EMPTY_ID_IS_UNRESOLVED: bool = EMPTY_ID.is_unresolved();

fn require_style_traits<T: Copy + Clone + Debug + Eq + Hash + Send + Sync + Unpin + 'static>() {}

fn require_id_traits<T: Copy + Clone + Debug + Eq + Hash + Ord + Send + Sync + Unpin + 'static>() {}

fn exercise_repair_tooling_surface() {
    assert_eq!(REPAIR_PROPOSAL_SCHEMA_VERSION, "1.0.0");
    assert_eq!(REPAIR_PLAN_SCHEMA_VERSION, "1.0.0");
    assert_eq!(REPAIR_DRY_RUN_SCHEMA_VERSION, "1.0.0");
    assert_eq!(REPAIR_CHANGE_RECEIPT_SCHEMA_VERSION, "1.0.0");
    assert_eq!(REPAIR_CHECK_POLICY_SCHEMA_VERSION, "1.4.0");
    assert_eq!(REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION, "1.4.0");
    assert_eq!(REPAIR_TEST_EVIDENCE_SCHEMA_VERSION, "1.0.0");
    assert_eq!(REPAIR_BROWSER_EVIDENCE_SCHEMA_VERSION, "1.0.0");
    let _: RepairCheckKind = RepairCheckKind::TokenGraphIntegrity;
    let _: RepairCheckKind = RepairCheckKind::CssBudgetAudit;
    let _: RepairCheckKind = RepairCheckKind::TestSuiteEvidence;
    let _: RepairCheckKind = RepairCheckKind::BrowserEvidence;
    let _: RepairTestProfile = RepairTestProfile::RustWorkspaceAllTargets;
    let _: RepairTestEvidenceResult = RepairTestEvidenceResult::Passed;
    let _: Option<RepairTestEvidence> = None;
    let _: Option<RepairTestToolchainIdentity> = None;
    let _: RepairBrowserProfile = RepairBrowserProfile::PliegorsVisitCounterChromiumCdp;
    let _: RepairBrowserEvidenceResult = RepairBrowserEvidenceResult::Passed;
    let _: Option<RepairBrowserEvidence> = None;
    let _: Option<RepairBrowserRunnerIdentity> = None;
    let _: Option<RepairBrowserIdentity> = None;
    let _: Option<RepairBrowserObservation> = None;
    let _: Option<PublishedRepairBrowserEvidence> = None;
    let _: Option<RepairVerificationInputs<'static>> = None;
    let _ = execute_repair_verification_with_inputs;
    let _ = parse_repair_test_evidence;
    let _ = run_repair_rust_tests_checked;
    let _ = parse_repair_browser_evidence;
    let _ = run_repair_pliegors_browser_checked;
    let _tool = RepairTool::new("public-api-smoke", env!("CARGO_PKG_VERSION"))
        .expect("valid repair tool identity");
    let _budget = RepairChangeBudget::new(1, 1, 16, 16).expect("valid repair budget");
    let proposal_error: RepairContractError =
        parse_repair_proposal(b"{}").expect_err("closed proposal must reject missing fields");
    assert!(!proposal_error.to_string().is_empty());
    let plan_error: RepairContractError =
        parse_repair_plan(b"{}").expect_err("closed plan must reject missing fields");
    assert!(!plan_error.to_string().is_empty());
    let receipt_error: RepairContractError = parse_repair_change_receipt(b"{}")
        .expect_err("closed change receipt must reject missing fields");
    assert!(!receipt_error.to_string().is_empty());
    let policy_error: RepairContractError =
        parse_repair_check_policy(b"{}").expect_err("closed check policy must reject missing fields");
    assert!(!policy_error.to_string().is_empty());
    let verification_error: RepairContractError = parse_repair_verification_receipt(b"{}")
        .expect_err("closed verification receipt must reject missing fields");
    assert!(!verification_error.to_string().is_empty());
    let _finding_document_surface: fn(&PublishedRepairFindingDocument) = |document| {
        let _: &str = document.check_id();
        let _: &str = document.file();
        let _: &[u8] = document.bytes();
        let _: bool = document.changed();
    };
}

fn exercise_migration_inventory_surface() {
    assert_eq!(MIGRATION_INVENTORY_SCHEMA_VERSION, 1);
    let _file_reader = inventory_migration_file;
    let project = MigrationProject::new().source(MigrationProjectSource::new(
        MigrationSourceKind::Sass,
        "src/app.scss",
    ));
    let declared_project = MigrationProject::from_json(
        br#"{"schemaVersion":1,"sources":[{"sourceKind":"sass","file":"src/app.scss"}]}"#,
    )
    .expect("valid closed migration project declaration");
    let _project_collector: fn(
        MigrationProject,
    ) -> Result<MigrationProjectInventory, MigrationInventoryError> = MigrationProject::collect;
    let _project_dependencies: fn(&MigrationProjectInventory) -> &[MigrationDependency] =
        MigrationProjectInventory::dependencies;
    let _project_consumers: fn(&MigrationProjectInventory) -> &[MigrationConsumerInventory] =
        MigrationProjectInventory::consumers;
    let _project_auxiliaries: fn(&MigrationProjectInventory) -> &[MigrationAuxiliaryInventory] =
        MigrationProjectInventory::auxiliaries;
    let _consumer_reader = inventory_migration_consumer_file;
    let consumer = inventory_migration_consumer_source(
        MigrationConsumerKind::CssModules,
        "src/Card.tsx",
        "import styles from \"./card.module.css\"; const card = styles.card;",
    )
    .expect("valid CSS Modules consumer inventory");
    let _consumer_declaration =
        MigrationProjectConsumer::new(MigrationConsumerKind::CssModules, "src/Card.tsx");
    let _consumer_surface: fn(&MigrationConsumerObservation) = |observation| {
        let _: MigrationConsumerObservationKind = observation.kind();
        let _: MigrationDisposition = observation.disposition();
        let _: usize = observation.byte_start();
        let _: usize = observation.byte_end();
        let _: Option<&str> = observation.binding();
        let _: Option<&str> = observation.specifier();
        let _: Option<&str> = observation.target();
        let _: Option<&str> = observation.class_name();
    };
    assert_eq!(consumer.consumer_kind(), MigrationConsumerKind::CssModules);
    assert_eq!(consumer.file(), "src/Card.tsx");
    assert_eq!(consumer.source_sha256().len(), 64);
    assert_eq!(consumer.observations().len(), 2);
    let _auxiliary_reader = inventory_migration_auxiliary_file;
    let auxiliary = inventory_migration_auxiliary_source(
        MigrationAuxiliaryKind::TailwindConfig,
        "tailwind.config.js",
        "export default {};",
    )
    .expect("valid Tailwind auxiliary inventory");
    let _auxiliary_declaration = MigrationProjectAuxiliary::new(
        MigrationAuxiliaryKind::TailwindTemplate,
        "src/index.html",
    );
    assert_eq!(auxiliary.auxiliary_kind(), MigrationAuxiliaryKind::TailwindConfig);
    assert_eq!(auxiliary.file(), "tailwind.config.js");
    assert_eq!(auxiliary.source_bytes(), 18);
    assert_eq!(auxiliary.source_sha256().len(), 64);
    assert!(auxiliary.observations().is_empty());
    let template = inventory_migration_auxiliary_source(
        MigrationAuxiliaryKind::TailwindTemplate,
        "src/index.html",
        "<div class=\"grid gap-2\"></div>",
    )
    .expect("valid Tailwind template inventory");
    let _auxiliary_observation_surface: fn(&MigrationAuxiliaryObservation) = |observation| {
        let _: MigrationAuxiliaryObservationKind = observation.kind();
        let _: MigrationDisposition = observation.disposition();
        let _: usize = observation.byte_start();
        let _: usize = observation.byte_end();
        let _: Option<&str> = observation.value();
    };
    assert_eq!(template.observations().len(), 2);
    let _dependency_surface: fn(&MigrationDependency) = |dependency| {
        let _: &str = dependency.from();
        let _: MigrationDependencyKind = dependency.kind();
        let _: MigrationDependencyResolution = dependency.resolution();
        let _: usize = dependency.byte_start();
        let _: usize = dependency.byte_end();
        let _: Option<&str> = dependency.specifier();
        let _: Option<&str> = dependency.target();
    };
    let _: MigrationDependencyKind = MigrationDependencyKind::SassUse;
    let _: MigrationDependencyResolution = MigrationDependencyResolution::Resolved;
    drop(project);
    drop(declared_project);
    let inventory: MigrationInventory = inventory_migration_source(
        MigrationSourceKind::Tailwind,
        "src/app.css",
        "@import \"tailwindcss\";\n@source inline(\"bg-red-{100..900..100}\");\n",
    )
    .expect("valid migration inventory");
    assert_eq!(inventory.source_kind(), MigrationSourceKind::Tailwind);
    assert_eq!(inventory.file(), "src/app.css");
    assert_eq!(inventory.source_sha256().len(), 64);
    assert!(inventory.source_bytes() > 0);
    assert_eq!(
        inventory.preflight_reliance(),
        MigrationPreflightReliance::Implicit
    );
    assert_eq!(inventory.dynamic_count(), 1);
    assert_eq!(inventory.unsupported_count(), 0);
    assert_eq!(inventory.constructs().len(), 2);
    assert_eq!(
        inventory.constructs()[1].disposition(),
        MigrationDisposition::Dynamic
    );
    assert!(inventory.constructs()[0].byte_end() > inventory.constructs()[0].byte_start());
    assert!(!inventory.constructs()[0].kind().is_empty());
    assert!(!inventory.constructs()[0].syntax().is_empty());
    assert!(inventory.as_bytes().ends_with(b"\n"));

    let error: MigrationInventoryError = inventory_migration_source(
        MigrationSourceKind::CssModules,
        "src/app.css",
        "",
    )
    .expect_err("CSS Modules requires a module filename");
    assert!(!error.to_string().is_empty());
}

fn exercise_ownership_adapter_surface() {
    let invalid: OwnershipError =
        parse_asset_plan(b"{}").expect_err("invalid public Asset Plan fixture");
    let _: &dyn std::error::Error = &invalid;
    assert_eq!(invalid.to_string(), invalid.reason());

    let plan: AssetPlan =
        parse_asset_plan(OWNERSHIP_ASSET_PLAN).expect("valid public Asset Plan fixture");
    assert_eq!(plan.exact_bytes(), OWNERSHIP_ASSET_PLAN.len());
    assert_eq!(plan.sha256().len(), 64);
    assert_eq!(plan.rule_selection(), OwnershipRuleSelection::AllCompiled);

    let bundle: &AssetBundle = &plan.bundles()[0];
    assert_eq!(bundle.id(), "app");
    assert_eq!(bundle.css_file(), "app.css");
    assert_eq!(bundle.manifest_file(), "app.manifest.json");
    assert!(bundle.emits_theme());

    let route: &AssetRoute = &plan.routes()[0];
    assert_eq!(route.id(), "route:home");
    assert_eq!(route.path(), "/");
    assert_eq!(route.bundle_ids().len(), 1);
    assert_eq!(route.bundle_ids()[0], "app");

    let island: &AssetIsland = &plan.islands()[0];
    assert_eq!(island.id(), "island:counter");
    assert_eq!(island.name(), "counter");
    assert_eq!(island.bundle_ids().len(), 1);
    assert_eq!(island.bundle_ids()[0], "app");

    let mapping = BundlePackageInput::new("app", "example-app");
    assert_eq!(mapping.bundle_id(), "app");
    assert_eq!(mapping.package_id(), "example-app");
    let active_islands = ["island:counter"];
    let composition = RouteCompositionInput::new("route:home", &active_islands);
    assert_eq!(composition.route_id(), "route:home");
    assert_eq!(composition.island_ids(), &active_islands);

    let document = build_ownership_document(&plan, &[mapping], &[composition])
        .expect("valid public ownership fixture");
    assert_eq!(document.last(), Some(&b'\n'));
    let ownership: Ownership =
        parse_ownership(&document, &plan).expect("builder output must parse");
    let package: &PackageOwnership = &ownership.packages()[0];
    assert_eq!(package.package_id(), "example-app");
    assert_eq!(package.bundle_ids().len(), 1);
    assert_eq!(package.bundle_ids()[0], "app");
    let route: &ComposedRoute = &ownership.composed_routes()[0];
    assert_eq!(route.route_id(), "route:home");
    assert_eq!(route.path(), "/");
    assert_eq!(route.island_ids().len(), 1);
    assert_eq!(route.island_ids()[0], "island:counter");
    assert_eq!(route.bundle_ids().len(), 1);
    assert_eq!(route.bundle_ids()[0], "app");
}

fn exercise_usage_adapter_surface() {
    let style_id = format!("{:032x}", FIXED_STYLE.id().get());
    let class_name = FIXED_STYLE.id().to_class_name();
    let styles = collect_usage_style_inputs(
        "app",
        [UsageCandidateInput::new(
            &style_id,
            &class_name,
            "flex gap-4 md:grid",
            Some("src/main.rs"),
            Some(0),
            Some(18),
            "pc",
            "public API smoke fixture",
        )],
    )
    .expect("complete public usage origin must collect");
    assert_eq!(styles.len(), 1);

    let build = |reverse: bool| {
        let routes = if reverse {
            vec!["/settings".into(), "/".into()]
        } else {
            vec!["/".into(), "/settings".into()]
        };
        let observed_styles = if reverse {
            vec![
                UsageObservedStyleInput::new("settings", "22222222222222222222222222222222"),
                UsageObservedStyleInput::new("app", &style_id),
            ]
        } else {
            vec![
                UsageObservedStyleInput::new("app", &style_id),
                UsageObservedStyleInput::new("settings", "22222222222222222222222222222222"),
            ]
        };
        build_usage_observation(UsageObservationInput::new(
            "a".repeat(64),
            "b".repeat(64),
            UsageObservationCoverage::Sampled,
            "public-api-smoke",
            env!("CARGO_PKG_VERSION"),
            UsageObservationScopeInput::new(
                routes,
                vec!["island:counter".into()],
                vec!["light".into()],
                vec!["chromium".into()],
                vec!["1280x720".into()],
                vec!["default".into()],
            ),
            vec!["authenticated-user".into()],
            observed_styles,
        ))
        .expect("valid public usage observation input")
    };

    let canonical = build(false);
    assert_eq!(canonical, build(true));
    assert_eq!(canonical.last(), Some(&b'\n'));
    let _observation =
        parse_usage_observation(&canonical).expect("builder output must parse canonically");

    let report = build_usage_analysis(&styles, None, None, UsageRuleSelection::AllCompiled)
        .expect("valid public usage universe must build");
    let _verified_report = verify_usage_analysis(&report, &styles, None, None, None)
        .expect("analysis must verify against the exact compiler universe");

    let build_retention = |reverse: bool| {
        let mut entries = vec![
            UsageRetentionEntryInput::new(
                "external-email-renderer",
                "email",
                "22222222222222222222222222222222",
                "The external email renderer is outside the application route graph.",
            ),
            UsageRetentionEntryInput::new(
                "external-pdf-renderer",
                "pdf",
                "33333333333333333333333333333333",
                "The external PDF renderer is outside the application route graph.",
            ),
        ];
        if reverse {
            entries.reverse();
        }
        build_usage_retention(UsageRetentionInput::new(
            "a".repeat(64),
            "b".repeat(64),
            entries,
        ))
        .expect("valid public usage retention input")
    };
    let canonical_retention = build_retention(false);
    assert_eq!(canonical_retention, build_retention(true));
    let _retention = parse_usage_retention(&canonical_retention)
        .expect("retention builder output must parse canonically");
}

fn exercise_application_collector_surface() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let topology = ApplicationTopology::new()
        .source_root("src")
        .component(ApplicationComponent::new("public-api-smoke::main").source_unit("src/main.rs"))
        .route(ApplicationRoute::new("home", "/").component("public-api-smoke::main"));
    let collected = topology
        .collect(root)
        .expect("public fixture topology must collect");
    assert_eq!(collected.source_file_count(), 1);
    assert!(collected.invocation_count() >= 2);
    assert!(collected.as_bytes().ends_with(b"\n"));
    let owned = collected.into_bytes();
    assert!(owned.windows(b"public-api-smoke::main".len()).any(|window| {
        window == b"public-api-smoke::main"
    }));
}

fn main() {
    exercise_repair_tooling_surface();
    exercise_migration_inventory_surface();
    exercise_ownership_adapter_surface();
    exercise_usage_adapter_surface();
    exercise_application_collector_surface();
    require_style_traits::<Style>();
    require_id_traits::<StyleId>();
    assert_eq!(EMPTY_ID_BITS, 0);
    assert!(EMPTY_STYLE_IS_EMPTY);
    assert!(EMPTY_ID_IS_UNRESOLVED);

    let id: StyleId = FIXED_STYLE.id();
    assert!(!id.is_unresolved());
    assert_eq!(FIXED_STYLE.class_name(), id.to_class_name());
    assert_eq!(FIXED_STYLE.to_string(), FIXED_STYLE.class_name());

    let active = true;
    let selected: Style = pcx!(
        "block rounded-lg",
        if active {
            "opacity-100 text-brand"
        } else {
            "opacity-50 text-ink"
        },
    );
    let selected_class: String = selected.into();
    assert!(selected_class.starts_with("pc_"));

    assert!(Style::EMPTY.is_empty());
    assert!(Style::EMPTY.id().is_unresolved());
    assert_eq!(Style::EMPTY.id().to_class_name(), "pc_0");
    assert_eq!(Style::EMPTY.class_name(), "");

    println!("{:032x}\t{}", id.get(), FIXED_STYLE.class_name());
}
