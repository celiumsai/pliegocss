//! Public CLI contract for deterministic route and island asset plans.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use pliego_css_build::artifacts::{
    AssetPlanBundle, AssetRuleSelection, build_asset_plan, sha256_hex,
};
use pliego_css_compiler::STYLE_ID_FORMAT_VERSION;
use pliego_css_control::{
    CheckStatus, MeasurementState, ReceiptResult, content_hash, parse_build_receipt,
    parse_control_manifest,
};
use pliego_css_ir::CLASS_NAME_FORMAT_VERSION;
use pliego_css_ownership::{
    AssetRuleSelection as OwnershipRuleSelection, BundlePackageInput, RouteCompositionInput,
    build_ownership_document, parse_asset_plan, parse_ownership,
};
use pliego_css_theme::THEME_ID_FORMAT_VERSION;
use serde_json::{Value, json};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

const GLOBAL_SOURCE: &str = r#"fn global() { let _ = pc!("font-sans bg-surface"); }"#;
const HOME_SOURCE: &str = r#"fn home() { let _ = pc!("grid gap-4"); }"#;
const VISIT_SOURCE: &str = r#"fn visit() { let _ = pc!("flex gap-4"); }"#;
const COUNTER_SOURCE: &str = r#"fn counter() { let _ = pc!("inline-flex rounded-md"); }"#;
const DEAD_SOURCE: &str = r#"fn dead() { let _ = pc!("hidden bg-accent-strong"); }"#;
const SHARED_SOURCE: &str = r#"fn shared_live() { let _ = pc!("p-4"); }
fn shared_dead() { let _ = pc!("p-4"); }
"#;
const COOWNED_SOURCE: &str = r#"fn coowned() { let _ = pc!("rounded-md"); }"#;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Site {
    file: &'static str,
    start: usize,
    end: usize,
}

struct Sites {
    global: Site,
    home: Site,
    visit: Site,
    counter: Site,
    dead: Site,
    shared_live: Site,
    shared_dead: Site,
    coowned: Site,
}

struct TemporaryTree(PathBuf);

impl TemporaryTree {
    fn new() -> Self {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("pliego-cssc must live below the workspace root");
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos();
        for _ in 0..32 {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = workspace
                .join("target/tests/asset-plan")
                .join(format!("{}-{timestamp}-{sequence}", std::process::id()));
            match fs::create_dir_all(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("cannot create `{}`: {error}", path.display()),
            }
        }
        panic!("cannot allocate a unique asset-plan test directory");
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryTree {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!(
                "warning: cannot remove asset-plan test directory `{}`: {error}",
                self.0.display()
            );
        }
    }
}

struct Fixture {
    tree: TemporaryTree,
    sites: Sites,
}

impl Fixture {
    fn new() -> Self {
        let tree = TemporaryTree::new();
        let root = tree.path();
        for (file, source) in [
            ("src/global.rs", GLOBAL_SOURCE),
            ("src/home.rs", HOME_SOURCE),
            ("src/visit.rs", VISIT_SOURCE),
            ("src/counter.rs", COUNTER_SOURCE),
            ("src/dead.rs", DEAD_SOURCE),
            ("src/shared.rs", SHARED_SOURCE),
            ("src/coowned.rs", COOWNED_SOURCE),
        ] {
            write_file(&root.join(file), source);
        }
        let shared = sites("src/shared.rs", SHARED_SOURCE, r#"pc!("p-4")"#);
        assert_eq!(
            shared.len(),
            2,
            "shared fixture must expose two exact sites"
        );
        let sites = Sites {
            global: unique_site(
                "src/global.rs",
                GLOBAL_SOURCE,
                r#"pc!("font-sans bg-surface")"#,
            ),
            home: unique_site("src/home.rs", HOME_SOURCE, r#"pc!("grid gap-4")"#),
            visit: unique_site("src/visit.rs", VISIT_SOURCE, r#"pc!("flex gap-4")"#),
            counter: unique_site(
                "src/counter.rs",
                COUNTER_SOURCE,
                r#"pc!("inline-flex rounded-md")"#,
            ),
            dead: unique_site(
                "src/dead.rs",
                DEAD_SOURCE,
                r#"pc!("hidden bg-accent-strong")"#,
            ),
            shared_live: shared[0],
            shared_dead: shared[1],
            coowned: unique_site("src/coowned.rs", COOWNED_SOURCE, r#"pc!("rounded-md")"#),
        };
        write_file(
            &root.join("pliego.bundles.toml"),
            &bundle_plan(false, false),
        );
        write_json(root.join("reachability.json"), &reachability(&sites));
        Self { tree, sites }
    }

    fn root(&self) -> &Path {
        self.tree.path()
    }
}

#[test]
fn asset_plan_public_cli_contract() {
    let fixture = Fixture::new();
    verify_schema_four(&fixture);
    let schema_five = verify_schema_five_pruning(&fixture);
    verify_schema_two_asset_plan_ownership_audit(&fixture, &schema_five);
    verify_asset_plan_budget_audit(&fixture, &schema_five);
    verify_reorder_check_and_read_only_drift(&fixture, &schema_five);
    verify_boolean_and_invalid_combinations(&fixture);
    verify_asset_generation_failure_publishes_nothing(&fixture, &schema_five);
}

#[allow(clippy::too_many_lines)]
fn verify_schema_two_asset_plan_ownership_audit(
    fixture: &Fixture,
    snapshot: &BTreeMap<String, Vec<u8>>,
) {
    let output = fixture.root().join("schema-two-audit");
    fs::create_dir(&output).expect("schema-two audit directory must be created");
    let bundle_ids = ["counter", "dead", "global", "home", "visit"];
    for id in bundle_ids {
        for suffix in ["css", "manifest.json"] {
            let name = format!("{id}.{suffix}");
            fs::write(output.join(&name), &snapshot[&name]).unwrap_or_else(|error| {
                panic!("cannot write schema-two fixture `{name}`: {error}")
            });
        }
    }

    let bundles = bundle_ids
        .iter()
        .map(|id| {
            let css = format!("{id}.css");
            let manifest = format!("{id}.manifest.json");
            AssetPlanBundle::new(id, *id == "global", &snapshot[&css], &snapshot[&manifest])
        })
        .collect::<Vec<_>>();
    let plan_bytes = build_asset_plan(&bundles, AssetRuleSelection::ReachableOrRetainedStyleIds)
        .expect("schema-two Asset Plan must build from exact bundle bytes");
    let plan_json: Value =
        serde_json::from_slice(&plan_bytes).expect("schema-two Asset Plan must be JSON");
    assert_eq!(plan_json["schemaVersion"], 2);
    assert_eq!(plan_json["manifestSchemaVersion"], 5);
    assert_eq!(plan_json["graphSchemaVersion"], 2);
    assert_eq!(
        plan_json["ruleSelection"],
        "reachable-or-retained-style-ids"
    );
    fs::write(output.join("pliego.assets.json"), &plan_bytes)
        .expect("schema-two Asset Plan must be written");

    let plan = parse_asset_plan(&plan_bytes).expect("schema-two Asset Plan must parse");
    assert_eq!(
        plan.rule_selection(),
        OwnershipRuleSelection::ReachableOrRetainedStyleIds
    );
    let mappings = [
        BundlePackageInput::new("counter", "fixture-islands"),
        BundlePackageInput::new("dead", "fixture-dead"),
        BundlePackageInput::new("global", "fixture-shell"),
        BundlePackageInput::new("home", "fixture-app"),
        BundlePackageInput::new("visit", "fixture-app"),
    ];
    let home_islands = ["island:counter"];
    let no_islands = [];
    let compositions = [
        RouteCompositionInput::new("route:home", &home_islands),
        RouteCompositionInput::new("route:visit", &no_islands),
    ];
    let ownership_bytes = build_ownership_document(&plan, &mappings, &compositions)
        .expect("canonical ownership must bind to exact schema-two Asset Plan bytes");
    let ownership_json: Value =
        serde_json::from_slice(&ownership_bytes).expect("canonical ownership must be JSON");
    assert_eq!(
        ownership_json["assetPlanBytes"].as_u64(),
        Some(usize_u64(plan_bytes.len()))
    );
    assert_eq!(ownership_json["assetPlanSha256"], sha256_hex(&plan_bytes));
    let ownership =
        parse_ownership(&ownership_bytes, &plan).expect("canonical ownership must round-trip");
    assert_eq!(ownership.packages().len(), 4);
    assert_eq!(ownership.composed_routes().len(), 2);
    fs::write(
        fixture.root().join("schema-two.ownership.json"),
        &ownership_bytes,
    )
    .expect("schema-two ownership must be written");

    let control = fixture.root().join("schema-two-control");
    fs::create_dir(&control).expect("schema-two control directory must be created");
    let audit_arguments = arguments(&[
        "audit",
        "--asset-plan",
        "schema-two-audit/pliego.assets.json",
        "--targets",
        "none",
        "--ownership",
        "schema-two.ownership.json",
        "--control-dir",
        "schema-two-control",
        "--format",
        "json",
    ]);
    let output = run(fixture.root(), &audit_arguments);
    assert_success(&output);
    serde_json::from_slice::<Value>(&output.stdout)
        .expect("schema-two Asset Plan audit must emit JSON");

    let manifest_bytes =
        fs::read(control.join("pliego.css.manifest.json")).expect("schema-two control manifest");
    let receipt_bytes =
        fs::read(control.join("pliego.css.receipt.json")).expect("schema-two build receipt");
    let manifest =
        parse_control_manifest(&manifest_bytes).expect("valid schema-two control manifest");
    let receipt = parse_build_receipt(&receipt_bytes, &manifest_bytes)
        .expect("schema-two manifest-bound receipt");
    assert_eq!(receipt.result, ReceiptResult::Passed);
    assert_eq!(
        manifest.outputs[0].relationships,
        ["schema-two-audit/pliego.assets.json"]
    );
    let integrity = receipt
        .checks
        .iter()
        .find(|check| check.id == "asset-plan-integrity")
        .expect("schema-two receipt must ledger Asset Plan integrity");
    assert!(integrity.required);
    assert_eq!(integrity.status, CheckStatus::Passed);
    for (file, role) in [
        ("schema-two-audit/pliego.assets.json", "asset-plan"),
        ("schema-two.ownership.json", "ownership"),
    ] {
        let input = manifest
            .inputs
            .files
            .iter()
            .find(|input| input.file == file)
            .unwrap_or_else(|| panic!("missing schema-two ledger input `{file}`"));
        assert_eq!(input.role, role);
        assert_eq!(
            input.sha256,
            content_hash(
                &fs::read(fixture.root().join(file)).expect("exact schema-two input bytes")
            )
        );
    }

    let control_before = output_snapshot(&control);
    let mut stale_ownership: Value =
        serde_json::from_slice(&ownership_bytes).expect("canonical ownership JSON");
    stale_ownership["assetPlanSha256"] = json!("0".repeat(64));
    write_json(
        fixture.root().join("schema-two.stale.ownership.json"),
        &stale_ownership,
    );
    let stale = run(
        fixture.root(),
        &arguments(&[
            "audit",
            "--asset-plan",
            "schema-two-audit/pliego.assets.json",
            "--targets",
            "none",
            "--ownership",
            "schema-two.stale.ownership.json",
            "--control-dir",
            "schema-two-control",
            "--format",
            "json",
        ]),
    );
    assert_failure(
        &stale,
        "ownership assetPlanSha256 does not match exact Asset Plan bytes",
    );
    assert_eq!(
        output_snapshot(&control),
        control_before,
        "stale schema-two ownership mutated the successful control ledger"
    );
}

#[allow(clippy::too_many_lines)]
fn verify_asset_plan_budget_audit(fixture: &Fixture, snapshot: &BTreeMap<String, Vec<u8>>) {
    let plan_bytes = &snapshot["pliego.assets.json"];
    write_json(
        fixture.root().join("audit.ownership.json"),
        &json!({
            "schemaVersion": 1,
            "ownershipCoverage": "adapter-attested-complete",
            "assetPlanBytes": plan_bytes.len(),
            "assetPlanSha256": sha256_hex(plan_bytes),
            "bundlePackages": [
                {"bundleId": "counter", "packageId": "fixture-islands"},
                {"bundleId": "dead", "packageId": "fixture-dead"},
                {"bundleId": "global", "packageId": "fixture-shell"},
                {"bundleId": "home", "packageId": "fixture-app"},
                {"bundleId": "visit", "packageId": "fixture-app"}
            ],
            "routeCompositions": [
                {"routeId": "route:home", "islandIds": ["island:counter"]},
                {"routeId": "route:visit", "islandIds": []}
            ]
        }),
    );
    write_json(
        fixture.root().join("audit.budgets.json"),
        &json!({
            "schemaVersion": 1,
            "policyVersion": 1,
            "budgets": [
                {
                    "id": "home-route-bytes",
                    "subject": {"kind": "route", "id": "/"},
                    "limits": {"bytes": {"maximum": 999_999, "baseline": null, "maxIncrease": null}}
                },
                {
                    "id": "application-package-bytes",
                    "subject": {"kind": "package", "id": "fixture-app"},
                    "limits": {"bytes": {"maximum": 999_999, "baseline": null, "maxIncrease": null}}
                }
            ]
        }),
    );
    write_json(
        fixture.root().join("pliego.accessibility.json"),
        &json!({
            "schemaVersion": 1,
            "policyVersion": 1,
            "checks": {
                "contrast": {"violation": "fail", "unverified": "warn", "manualRequired": "warn"},
                "motion": {"violation": "fail", "unverified": "warn", "manualRequired": "warn"},
                "focusVisibility": {"violation": "fail", "unverified": "warn", "manualRequired": "warn"},
                "forcedColors": {"violation": "fail", "unverified": "warn", "manualRequired": "warn"},
                "inputModality": {"violation": "fail", "unverified": "warn", "manualRequired": "warn"}
            },
            "contrastPairs": [{
                "id": "black-on-white",
                "foreground": {"literal": {"value": "#000"}},
                "background": {"literal": {"value": "#fff"}},
                "minimumRatioMilli": 4500
            }],
            "exceptions": []
        }),
    );
    write_json(
        fixture.root().join("audit.file-budget.json"),
        &json!({
            "schemaVersion": 1,
            "policyVersion": 1,
            "budgets": [{
                "id": "home-file-bytes",
                "subject": {"kind": "file", "id": "schema-five/home.css"},
                "limits": {"bytes": {"maximum": 999_999, "baseline": null, "maxIncrease": null}}
            }]
        }),
    );
    let file_only = run(
        fixture.root(),
        &arguments(&[
            "audit",
            "--asset-plan",
            "schema-five/pliego.assets.json",
            "--targets",
            "none",
            "--budget-policy",
            "audit.file-budget.json",
            "--format",
            "json",
        ]),
    );
    assert_success(&file_only);
    let file_document: Value =
        serde_json::from_slice(&file_only.stdout).expect("file-only budget JSON");
    assert_eq!(
        budget_actual(&file_document, "home-file-bytes"),
        canonical_bytes(fixture, "schema-five/home.css")
    );
    let missing_ownership = run(
        fixture.root(),
        &arguments(&[
            "audit",
            "--asset-plan",
            "schema-five/pliego.assets.json",
            "--targets",
            "none",
            "--budget-policy",
            "audit.budgets.json",
        ]),
    );
    assert_failure(
        &missing_ownership,
        "asset-plan package or route budgets require `audit --ownership FILE`",
    );
    write_json(
        fixture.root().join("audit.partial-budget.json"),
        &json!({
            "schemaVersion": 1,
            "policyVersion": 1,
            "budgets": [
                {
                    "id": "home-file-covered",
                    "subject": {"kind": "file", "id": "schema-five/home.css"},
                    "limits": {"bytes": {"maximum": 999_999, "baseline": null, "maxIncrease": null}}
                },
                {
                    "id": "missing-package",
                    "subject": {"kind": "package", "id": "fixture-missing"},
                    "limits": {"bytes": {"maximum": 999_999, "baseline": null, "maxIncrease": null}}
                }
            ]
        }),
    );
    let partial_coverage = run(
        fixture.root(),
        &arguments(&[
            "audit",
            "--asset-plan",
            "schema-five/pliego.assets.json",
            "--targets",
            "none",
            "--budget-policy",
            "audit.partial-budget.json",
            "--ownership",
            "audit.ownership.json",
            "--format",
            "json",
        ]),
    );
    assert!(
        !partial_coverage.status.success(),
        "partially matched ownership policy unexpectedly passed"
    );
    assert!(partial_coverage.stderr.is_empty());
    let partial_document: Value =
        serde_json::from_slice(&partial_coverage.stdout).expect("partial budget finding JSON");
    assert!(
        partial_document["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "PCSS-BUDGET-199"),
        "partial policy coverage did not emit PCSS-BUDGET-199"
    );
    fs::create_dir(fixture.root().join("audit-control")).expect("audit control directory");
    let audit_arguments = arguments(&[
        "audit",
        "--asset-plan",
        "schema-five/pliego.assets.json",
        "--targets",
        "none",
        "--budget-policy",
        "audit.budgets.json",
        "--ownership",
        "audit.ownership.json",
        "--accessibility-policy",
        "pliego.accessibility.json",
        "--control-dir",
        "audit-control",
        "--format",
        "json",
    ]);
    let output = run(fixture.root(), &audit_arguments);
    assert_success(&output);
    let document: Value =
        serde_json::from_slice(&output.stdout).expect("asset-plan audit must emit JSON");
    let home_actual = budget_actual(&document, "home-route-bytes");
    let package_actual = budget_actual(&document, "application-package-bytes");
    assert_eq!(
        home_actual,
        canonical_bytes(fixture, "schema-five/global.css")
            + canonical_bytes(fixture, "schema-five/home.css")
            + canonical_bytes(fixture, "schema-five/counter.css")
    );
    assert_eq!(
        package_actual,
        ["home", "visit"]
            .iter()
            .map(|name| canonical_bytes(fixture, &format!("schema-five/{name}.css")))
            .sum::<u64>()
    );
    verify_asset_plan_control_group(fixture);
    let mut check_arguments = audit_arguments;
    check_arguments.push("--check".to_owned());
    assert_success(&run(fixture.root(), &check_arguments));

    let ownership_path = fixture.root().join("audit.ownership.json");
    let ownership_baseline = fs::read(&ownership_path).expect("ownership baseline");
    let baseline_manifest: Value = serde_json::from_slice(
        &fs::read(
            fixture
                .root()
                .join("audit-control/pliego.css.manifest.json"),
        )
        .expect("baseline control manifest"),
    )
    .expect("baseline control manifest JSON");
    let baseline_config_hash = baseline_manifest["inputs"]["configHash"]
        .as_str()
        .expect("baseline configHash")
        .to_owned();
    let mut semantically_equal_drift = ownership_baseline.clone();
    semantically_equal_drift.insert(semantically_equal_drift.len() - 1, b' ');
    fs::write(&ownership_path, &semantically_equal_drift).expect("ownership drift");

    let drift_control_dir = fixture.root().join("ownership-drift-control");
    fs::create_dir(&drift_control_dir).expect("ownership-drift control directory");
    let mut drift_arguments = check_arguments[..check_arguments.len() - 1].to_vec();
    let control_dir_value = drift_arguments
        .iter()
        .position(|argument| argument == "--control-dir")
        .expect("control-dir argument")
        + 1;
    "ownership-drift-control".clone_into(&mut drift_arguments[control_dir_value]);
    assert_success(&run(fixture.root(), &drift_arguments));
    let drift_manifest: Value = serde_json::from_slice(
        &fs::read(drift_control_dir.join("pliego.css.manifest.json"))
            .expect("ownership-drift control manifest"),
    )
    .expect("ownership-drift control manifest JSON");
    assert_ne!(
        drift_manifest["inputs"]["configHash"]
            .as_str()
            .expect("ownership-drift configHash"),
        baseline_config_hash,
        "exact ownership-byte drift must change manifest.inputs.configHash"
    );

    let control_before = output_snapshot(&fixture.root().join("audit-control"));
    let drift = run(fixture.root(), &check_arguments);
    assert_failure(&drift, "drift");
    assert_eq!(
        output_snapshot(&fixture.root().join("audit-control")),
        control_before,
        "audit --check mutated control outputs after ownership-byte drift"
    );
    fs::write(&ownership_path, &ownership_baseline).expect("restore ownership baseline");

    let collision_dir = fixture.root().join("ownership-collision");
    fs::create_dir(&collision_dir).expect("ownership collision directory");
    let colliding_ownership = collision_dir.join("pliego.css.manifest.json");
    fs::write(&colliding_ownership, &ownership_baseline).expect("colliding ownership input");
    let collision = run(
        fixture.root(),
        &arguments(&[
            "audit",
            "--asset-plan",
            "schema-five/pliego.assets.json",
            "--targets",
            "none",
            "--budget-policy",
            "audit.budgets.json",
            "--ownership",
            "ownership-collision/pliego.css.manifest.json",
            "--control-dir",
            "ownership-collision",
        ]),
    );
    assert_failure(&collision, "aliases input `--ownership`");
    assert_eq!(
        fs::read(&colliding_ownership).expect("colliding ownership remains readable"),
        ownership_baseline,
        "ownership/control collision mutated the input"
    );

    let mut dangling: Value = serde_json::from_slice(&ownership_baseline).expect("ownership JSON");
    dangling["routeCompositions"][0]["islandIds"] = json!(["island:missing"]);
    write_json(&ownership_path, &dangling);
    let invalid = run(
        fixture.root(),
        &check_arguments[..check_arguments.len() - 1],
    );
    assert_failure(&invalid, "unknown ownership island `island:missing`");
    fs::write(&ownership_path, &ownership_baseline).expect("restore ownership baseline");

    let mut wrong_hash: Value =
        serde_json::from_slice(&ownership_baseline).expect("ownership JSON");
    wrong_hash["assetPlanSha256"] = json!("0".repeat(64));
    write_json(&ownership_path, &wrong_hash);
    let invalid = run(
        fixture.root(),
        &check_arguments[..check_arguments.len() - 1],
    );
    assert_failure(
        &invalid,
        "ownership assetPlanSha256 does not match exact Asset Plan bytes",
    );
    fs::write(&ownership_path, &ownership_baseline).expect("restore ownership baseline");

    fs::write(
        fixture.root().join("schema-five/home.css"),
        b".tampered{color:red}\n",
    )
    .expect("tampered CSS must be written");
    let tampered = run(
        fixture.root(),
        &arguments(&[
            "audit",
            "--asset-plan",
            "schema-five/pliego.assets.json",
            "--targets",
            "none",
        ]),
    );
    assert_failure(&tampered, "invalid asset-plan ledger");
    fs::write(
        fixture.root().join("schema-five/home.css"),
        &snapshot["home.css"],
    )
    .expect("home CSS baseline must be restored");
}

fn verify_asset_plan_control_group(fixture: &Fixture) {
    let control = fixture.root().join("audit-control");
    let manifest_bytes = fs::read(control.join("pliego.css.manifest.json")).expect("manifest");
    let receipt_bytes = fs::read(control.join("pliego.css.receipt.json")).expect("receipt");
    let manifest_json: Value =
        serde_json::from_slice(&manifest_bytes).expect("control manifest JSON");
    let manifest = parse_control_manifest(&manifest_bytes).expect("valid control manifest");
    let receipt =
        parse_build_receipt(&receipt_bytes, &manifest_bytes).expect("manifest-bound receipt");
    assert_eq!(manifest.rules.observation, MeasurementState::Measured);
    assert_eq!(
        manifest_json["rules"]["byPackage"]["unknown"],
        manifest_json["rules"]["rules"]
    );
    assert_eq!(
        manifest_json["rules"]["byRoute"]["unknown"],
        manifest_json["rules"]["rules"]
    );
    assert_eq!(receipt.result, ReceiptResult::Passed);
    assert_eq!(manifest.tool.contracts["asset-plan"], "1");
    assert_eq!(
        manifest.outputs[0].relationships,
        ["schema-five/pliego.assets.json"]
    );
    assert!(
        receipt
            .checks
            .iter()
            .any(|check| check.id == "asset-plan-integrity" && check.required)
    );
    for (file, role) in [
        ("schema-five/pliego.assets.json", "asset-plan"),
        ("audit.budgets.json", "policy"),
        ("audit.ownership.json", "ownership"),
        ("pliego.accessibility.json", "accessibility-policy"),
    ] {
        let input = manifest
            .inputs
            .files
            .iter()
            .find(|input| input.file == file)
            .unwrap_or_else(|| panic!("missing exact input `{file}`"));
        assert_eq!(input.role, role);
        assert_eq!(
            input.sha256,
            content_hash(&fs::read(fixture.root().join(file)).expect("exact input bytes"))
        );
    }
}

fn canonical_bytes(fixture: &Fixture, input: &str) -> u64 {
    let output = run(
        fixture.root(),
        &arguments(&[
            "audit",
            "--input",
            input,
            "--targets",
            "none",
            "--format",
            "json",
        ]),
    );
    assert_success(&output);
    let document: Value = serde_json::from_slice(&output.stdout).expect("CSS audit must emit JSON");
    let inventory = document["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|finding| finding["code"] == "PCSS-AUDIT-000")
        .expect("inventory finding");
    evidence_value(inventory, "canonical-bytes")
}

fn budget_actual(document: &Value, budget_id: &str) -> u64 {
    let finding = document["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|finding| finding["context"]["budget-id"] == budget_id)
        .expect("budget finding");
    assert_eq!(finding["code"], "PCSS-BUDGET-100");
    evidence_value(finding, "actual")
}

fn evidence_value(finding: &Value, name: &str) -> u64 {
    finding["evidence"]
        .as_array()
        .expect("evidence")
        .iter()
        .find(|evidence| evidence["name"] == name)
        .and_then(|evidence| evidence["value"].as_str())
        .expect("numeric evidence")
        .parse()
        .expect("evidence must be an integer")
}

fn verify_schema_four(fixture: &Fixture) {
    let output = fixture.root().join("schema-four");
    fs::create_dir(&output).expect("schema-four output directory must be created");
    let result = run(
        fixture.root(),
        &asset_arguments("schema-four", 4, "reachability.json", false, false),
    );
    assert_success(&result);
    let plan = assert_asset_contract(fixture, "schema-four", 4, 1, "all-compiled", false);
    assert_eq!(plan["ruleSelection"], "all-compiled");
}

fn verify_schema_five_pruning(fixture: &Fixture) -> BTreeMap<String, Vec<u8>> {
    let output = fixture.root().join("schema-five");
    fs::create_dir(&output).expect("schema-five output directory must be created");
    let result = run(
        fixture.root(),
        &project_arguments("schema-five", "reachability.json", true, false),
    );
    assert_success(&result);
    assert_asset_contract(fixture, "schema-five", 5, 2, "reachable-style-ids", true);
    assert_shared_and_coowned_projection(fixture, "schema-five");
    output_snapshot(&output)
}

fn verify_reorder_check_and_read_only_drift(
    fixture: &Fixture,
    baseline: &BTreeMap<String, Vec<u8>>,
) {
    write_file(
        &fixture.root().join("pliego.bundles.toml"),
        &bundle_plan(true, false),
    );
    let reordered = reordered_reachability(reachability(&fixture.sites));
    write_json(fixture.root().join("reachability.json"), &reordered);
    let arguments = project_arguments("schema-five", "reachability.json", true, false);
    let output = run(fixture.root(), &arguments);
    assert_success(&output);
    assert!(stdout(&output).contains("0 changed"));
    assert_eq!(
        output_snapshot(&fixture.root().join("schema-five")),
        *baseline
    );

    let check = run(
        fixture.root(),
        &project_arguments("schema-five", "reachability.json", true, true),
    );
    assert_success(&check);
    assert!(stdout(&check).contains("output(s) match"));
    assert_eq!(
        output_snapshot(&fixture.root().join("schema-five")),
        *baseline
    );

    fs::write(
        fixture.root().join("schema-five/pliego.assets.json"),
        b"{\"drift\":true}\n",
    )
    .expect("asset-plan drift must be written");
    let drifted = output_snapshot(&fixture.root().join("schema-five"));
    let failed = run(
        fixture.root(),
        &project_arguments("schema-five", "reachability.json", true, true),
    );
    assert_failure(&failed, "bundle output drift detected in 1 artifact");
    assert!(stderr(&failed).contains("pliego.assets.json"));
    assert_eq!(
        output_snapshot(&fixture.root().join("schema-five")),
        drifted,
        "bundle --check repaired or mutated asset-plan drift"
    );
    fs::write(
        fixture.root().join("schema-five/pliego.assets.json"),
        &baseline["pliego.assets.json"],
    )
    .expect("baseline asset plan must be restored");
    assert_eq!(
        output_snapshot(&fixture.root().join("schema-five")),
        *baseline
    );

    fs::write(
        fixture.root().join("schema-five/pliego.index.json"),
        b"{\"drift\":true}\n",
    )
    .expect("project-index drift must be written");
    let drifted = output_snapshot(&fixture.root().join("schema-five"));
    let failed = run(
        fixture.root(),
        &project_arguments("schema-five", "reachability.json", true, true),
    );
    assert_failure(&failed, "bundle output drift detected in 1 artifact");
    assert!(stderr(&failed).contains("pliego.index.json"));
    assert_eq!(
        output_snapshot(&fixture.root().join("schema-five")),
        drifted,
        "bundle --check repaired or mutated project-index drift"
    );
    fs::write(
        fixture.root().join("schema-five/pliego.index.json"),
        &baseline["pliego.index.json"],
    )
    .expect("baseline project index must be restored");
    assert_eq!(
        output_snapshot(&fixture.root().join("schema-five")),
        *baseline
    );
}

fn verify_boolean_and_invalid_combinations(fixture: &Fixture) {
    fs::create_dir(fixture.root().join("without-plan"))
        .expect("without-plan output directory must be created");
    let plain = run(
        fixture.root(),
        &arguments(&[
            "bundle",
            "--plan",
            "pliego.bundles.toml",
            "--output-dir",
            "without-plan",
        ]),
    );
    assert_success(&plain);
    assert!(
        !fixture
            .root()
            .join("without-plan/pliego.assets.json")
            .exists()
    );

    fs::create_dir(fixture.root().join("invalid"))
        .expect("invalid output directory must be created");
    let missing_graph = run(
        fixture.root(),
        &arguments(&[
            "bundle",
            "--plan",
            "pliego.bundles.toml",
            "--output-dir",
            "invalid",
            "--asset-plan",
        ]),
    );
    assert_failure(
        &missing_graph,
        "`--asset-plan` requires manifest version 4 or 5 and `--reachability`",
    );

    let mut repeated = asset_arguments("invalid", 4, "reachability.json", false, false);
    repeated.push("--asset-plan".into());
    let repeated = run(fixture.root(), &repeated);
    assert_failure(&repeated, "`--asset-plan` may only be provided once");

    let mut path_argument = asset_arguments("invalid", 4, "reachability.json", false, false);
    let position = path_argument
        .iter()
        .position(|argument| argument == "--asset-plan")
        .expect("asset flag must exist");
    path_argument.insert(position + 1, "custom-assets.json".into());
    let path_argument = run(fixture.root(), &path_argument);
    assert_failure(&path_argument, "unknown bundle option `custom-assets.json`");
    assert!(output_snapshot(&fixture.root().join("invalid")).is_empty());
    assert!(!fixture.root().join("custom-assets.json").exists());
}

fn verify_asset_generation_failure_publishes_nothing(
    fixture: &Fixture,
    baseline: &BTreeMap<String, Vec<u8>>,
) {
    write_file(
        &fixture.root().join("pliego.bundles.toml"),
        &bundle_plan(true, true),
    );
    let failed = run(
        fixture.root(),
        &project_arguments("schema-five", "reachability.json", true, false),
    );
    assert_failure(
        &failed,
        "asset plan accepts at most one theme-emitting bundle",
    );
    assert_eq!(
        output_snapshot(&fixture.root().join("schema-five")),
        *baseline,
        "asset-plan generation failure published partial bundle outputs"
    );
}

#[allow(clippy::too_many_lines)]
fn assert_asset_contract(
    fixture: &Fixture,
    output_name: &str,
    manifest_schema: u64,
    graph_schema: u64,
    selection: &str,
    pruned: bool,
) -> Value {
    let output = fixture.root().join(output_name);
    let snapshot = output_snapshot(&output);
    let mut expected_files = vec![
        "counter.css",
        "counter.manifest.json",
        "dead.css",
        "dead.manifest.json",
        "global.css",
        "global.manifest.json",
        "home.css",
        "home.manifest.json",
        "pliego.assets.json",
        "visit.css",
        "visit.manifest.json",
    ];
    if manifest_schema == 5 {
        expected_files.insert(9, "pliego.index.json");
    }
    assert_eq!(
        snapshot.keys().map(String::as_str).collect::<Vec<_>>(),
        expected_files
    );
    let plan_bytes = &snapshot["pliego.assets.json"];
    assert_eq!(plan_bytes.last(), Some(&b'\n'));
    assert!(!plan_bytes.contains(&b'\r'));
    let plan: Value = serde_json::from_slice(plan_bytes).expect("asset plan must be valid JSON");
    assert_eq!(
        object_keys(&plan),
        BTreeSet::from([
            "applicationCoverage",
            "bundles",
            "classNameFormatVersion",
            "format",
            "graphSchemaVersion",
            "islands",
            "manifestSchemaVersion",
            "originCoverage",
            "routes",
            "ruleSelection",
            "schemaVersion",
            "styleIdFormatVersion",
            "targets",
            "themeId",
            "themeIdFormatVersion",
        ])
    );
    assert_eq!(plan["schemaVersion"], 1);
    assert_eq!(plan["manifestSchemaVersion"], manifest_schema);
    assert_eq!(plan["graphSchemaVersion"], graph_schema);
    assert_eq!(plan["ruleSelection"], selection);
    assert_eq!(plan["originCoverage"], "compiler-verified-complete");
    assert_eq!(plan["applicationCoverage"], "adapter-attested-complete");
    assert_eq!(plan["styleIdFormatVersion"], STYLE_ID_FORMAT_VERSION);
    assert_eq!(plan["classNameFormatVersion"], CLASS_NAME_FORMAT_VERSION);
    assert_eq!(plan["themeIdFormatVersion"], THEME_ID_FORMAT_VERSION);
    assert_eq!(plan["targets"], "modern");
    assert_eq!(plan["format"], "minified");
    if manifest_schema == 5 {
        assert_project_index_contract(fixture, &snapshot, &plan, selection);
    }

    let bundles = plan["bundles"]
        .as_array()
        .expect("asset bundles must be an array");
    assert_eq!(
        bundles
            .iter()
            .map(|bundle| value_string(bundle, "id"))
            .collect::<Vec<_>>(),
        ["global", "counter", "dead", "home", "visit"]
    );
    for bundle in bundles {
        assert_eq!(
            object_keys(bundle),
            BTreeSet::from([
                "cssBytes",
                "cssFile",
                "cssSha256",
                "emitsTheme",
                "id",
                "manifestBytes",
                "manifestFile",
                "manifestSha256",
            ])
        );
        let id = value_string(bundle, "id");
        let css_file = format!("{id}.css");
        let manifest_file = format!("{id}.manifest.json");
        assert_eq!(bundle["cssFile"], css_file);
        assert_eq!(bundle["manifestFile"], manifest_file);
        assert_eq!(bundle["emitsTheme"], id == "global");
        let css = &snapshot[&css_file];
        let manifest_bytes = &snapshot[&manifest_file];
        assert_eq!(bundle["cssBytes"].as_u64(), Some(usize_u64(css.len())));
        assert_eq!(bundle["cssSha256"], sha256_hex(css));
        assert_eq!(
            bundle["manifestBytes"].as_u64(),
            Some(usize_u64(manifest_bytes.len()))
        );
        assert_eq!(bundle["manifestSha256"], sha256_hex(manifest_bytes));
        let manifest: Value =
            serde_json::from_slice(manifest_bytes).expect("bundle manifest must be JSON");
        assert_eq!(manifest["schemaVersion"], manifest_schema);
        assert_eq!(manifest["graph"]["schemaVersion"], graph_schema);
        assert_eq!(
            manifest["styleIdFormatVersion"],
            plan["styleIdFormatVersion"]
        );
        assert_eq!(
            manifest["classNameFormatVersion"],
            plan["classNameFormatVersion"]
        );
        assert_eq!(
            manifest["themeIdFormatVersion"],
            plan["themeIdFormatVersion"]
        );
        assert_eq!(manifest["themeId"], plan["themeId"]);
        assert_eq!(manifest["targets"], plan["targets"]);
        assert_eq!(manifest["format"], plan["format"]);
        assert_eq!(manifest["cssBytes"].as_u64(), Some(usize_u64(css.len())));
        assert_eq!(manifest["cssSha256"], sha256_hex(css));
    }

    assert!(css_text(&snapshot["global.css"]).contains(":root{"));
    for name in ["counter", "dead", "home", "visit"] {
        assert!(!css_text(&snapshot[&format!("{name}.css")]).contains(":root{"));
    }
    assert_eq!(
        plan["routes"]
            .as_array()
            .expect("asset routes must be an array")
            .iter()
            .map(|route| value_string(route, "id"))
            .collect::<Vec<_>>(),
        ["route:home", "route:visit"]
    );
    let home = object_by_id(&plan, "routes", "route:home");
    let visit = object_by_id(&plan, "routes", "route:visit");
    let counter = object_by_id(&plan, "islands", "island:counter");
    assert_eq!(home["path"], "/");
    assert_eq!(visit["path"], "/visit");
    assert_eq!(counter["name"], "counter");
    assert_eq!(string_array(&home["bundles"]), ["global", "home"]);
    assert_eq!(string_array(&visit["bundles"]), ["global", "visit"]);
    assert_eq!(string_array(&counter["bundles"]), ["global", "counter"]);
    for owner in [home, visit, counter] {
        assert!(!string_array(&owner["bundles"]).contains(&"dead"));
    }
    assert!(!string_array(&home["bundles"]).contains(&"counter"));
    assert!(!string_array(&counter["bundles"]).contains(&"home"));

    let dead_manifest: Value = serde_json::from_slice(&snapshot["dead.manifest.json"])
        .expect("dead manifest must be JSON");
    if pruned {
        assert_eq!(snapshot["dead.css"], b"\n");
        assert!(dead_manifest["styles"].as_array().unwrap().is_empty());
        assert!(
            dead_manifest["graph"]["declarations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    } else {
        assert_ne!(snapshot["dead.css"], b"\n");
        assert!(!dead_manifest["styles"].as_array().unwrap().is_empty());
    }
    plan
}

#[allow(clippy::too_many_lines)]
fn assert_project_index_contract(
    fixture: &Fixture,
    snapshot: &BTreeMap<String, Vec<u8>>,
    plan: &Value,
    selection: &str,
) {
    let bytes = &snapshot["pliego.index.json"];
    assert_eq!(bytes.last(), Some(&b'\n'));
    assert!(!bytes.contains(&b'\r'));
    let index: Value = serde_json::from_slice(bytes).expect("project index must be valid JSON");
    assert_eq!(
        object_keys(&index),
        BTreeSet::from([
            "applicationCoverage",
            "assetPlanBytes",
            "assetPlanFile",
            "assetPlanSha256",
            "bundles",
            "classNameFormatVersion",
            "declarationIdFormatVersion",
            "documents",
            "format",
            "graphSchemaVersion",
            "manifestSchemaVersion",
            "originCoverage",
            "physicalCoverage",
            "physicalDeclarationIdFormatVersion",
            "physicalRuleIdFormatVersion",
            "ruleSelection",
            "schemaVersion",
            "sites",
            "sourceSiteIdFormatVersion",
            "styleIdFormatVersion",
            "targets",
            "themeId",
            "themeIdFormatVersion",
        ])
    );
    assert_eq!(index["schemaVersion"], 1);
    assert_eq!(index["sourceSiteIdFormatVersion"], 1);
    assert_eq!(index["manifestSchemaVersion"], 5);
    assert_eq!(index["graphSchemaVersion"], 2);
    assert_eq!(index["declarationIdFormatVersion"], 1);
    assert_eq!(index["physicalRuleIdFormatVersion"], 1);
    assert_eq!(index["physicalDeclarationIdFormatVersion"], 1);
    assert_eq!(index["originCoverage"], "compiler-verified-complete");
    assert_eq!(index["applicationCoverage"], "adapter-attested-complete");
    assert_eq!(index["physicalCoverage"], "compiler-verified-complete");
    for field in [
        "styleIdFormatVersion",
        "classNameFormatVersion",
        "themeIdFormatVersion",
        "themeId",
        "targets",
        "format",
    ] {
        assert_eq!(index[field], plan[field], "index field {field} drifted");
    }
    assert_eq!(index["ruleSelection"], selection);
    assert_eq!(index["assetPlanFile"], "pliego.assets.json");
    assert_eq!(
        value_usize(&index, "assetPlanBytes"),
        snapshot["pliego.assets.json"].len()
    );
    assert_eq!(
        index["assetPlanSha256"],
        sha256_hex(&snapshot["pliego.assets.json"])
    );

    let documents = index["documents"]
        .as_array()
        .expect("project documents must be an array");
    assert_eq!(documents.len(), 7);
    assert_eq!(
        documents
            .iter()
            .map(|document| value_string(document, "path"))
            .collect::<Vec<_>>(),
        [
            "src/coowned.rs",
            "src/counter.rs",
            "src/dead.rs",
            "src/global.rs",
            "src/home.rs",
            "src/shared.rs",
            "src/visit.rs",
        ]
    );
    let mut known_sites = BTreeSet::new();
    for document in documents {
        assert!(value_string(document, "id").starts_with("document:"));
        let path = value_string(document, "path");
        let source = fs::read(fixture.root().join(path)).expect("source document must be readable");
        assert_eq!(value_usize(document, "bytes"), source.len());
        assert_eq!(document["sha256"], sha256_hex(&source));
        for site in string_array(&document["siteIds"]) {
            assert!(known_sites.insert(site));
        }
    }

    let sites = index["sites"]
        .as_array()
        .expect("project sites must be an array");
    assert_eq!(sites.len(), 7);
    assert_eq!(known_sites.len(), sites.len());
    for site in sites {
        let id = value_string(site, "id");
        assert!(id.starts_with("site:"));
        assert!(known_sites.contains(id));
        assert!(value_string(site, "documentId").starts_with("document:"));
        let path = value_string(site, "path");
        let source = fs::read(fixture.root().join(path)).expect("site source must be readable");
        let start = value_usize(site, "byteStart");
        let end = value_usize(site, "byteEnd");
        assert!(start < end && end <= source.len());
        assert!(value_string(site, "className").starts_with("pc_"));
        assert_eq!(value_string(site, "styleId").len(), 32);
        assert!(!string_array(&site["bundleIds"]).is_empty());
        assert!(!string_array(&site["declarationIds"]).is_empty());
        assert!(!string_array(&site["componentIds"]).is_empty());
        let physical = site["physicalDeclarations"]
            .as_array()
            .expect("physical references must be an array");
        assert!(!physical.is_empty());
        for reference in physical {
            assert!(
                string_array(&site["bundleIds"]).contains(&value_string(reference, "bundleId"))
            );
            assert!(value_string(reference, "id").starts_with("css-decl:"));
        }
    }

    let bundles = index["bundles"]
        .as_array()
        .expect("project bundles must be an array");
    assert_eq!(bundles.len(), plan["bundles"].as_array().unwrap().len());
    for bundle in bundles {
        let id = value_string(bundle, "id");
        let asset = object_by_id(plan, "bundles", id);
        for field in [
            "cssFile",
            "manifestFile",
            "emitsTheme",
            "cssBytes",
            "cssSha256",
            "manifestBytes",
            "manifestSha256",
        ] {
            assert_eq!(bundle[field], asset[field], "bundle field {field} drifted");
        }
        for site in string_array(&bundle["siteIds"]) {
            assert!(known_sites.contains(site));
        }
    }
}

fn assert_shared_and_coowned_projection(fixture: &Fixture, output_name: &str) {
    let manifest = read_json(fixture.root().join(output_name).join("home.manifest.json"));
    let shared = style_for_site(&manifest, fixture.sites.shared_live);
    let shared_origins = origin_sites(shared);
    assert!(shared_origins.contains(&fixture.sites.shared_live));
    assert!(shared_origins.contains(&fixture.sites.shared_dead));
    assert_eq!(shared_origins.len(), 2);
    assert_component_owners(
        &manifest,
        value_string(shared, "styleId"),
        &["component:shared-live", "component:shared-dead"],
    );

    let coowned = style_for_site(&manifest, fixture.sites.coowned);
    assert_component_owners(
        &manifest,
        value_string(coowned, "styleId"),
        &["component:co-home", "component:co-dead"],
    );
}

fn assert_component_owners(manifest: &Value, style_id: &str, owners: &[&str]) {
    let declarations = manifest["graph"]["declarations"]
        .as_array()
        .expect("declarations must be an array")
        .iter()
        .filter(|declaration| declaration["styleId"] == style_id)
        .map(|declaration| value_string(declaration, "id"))
        .collect::<Vec<_>>();
    assert!(!declarations.is_empty());
    let edges = manifest["graph"]["edges"]
        .as_array()
        .expect("graph edges must be an array");
    for owner in owners {
        for declaration in &declarations {
            assert!(
                edges.iter().any(|edge| {
                    edge["kind"] == "componentUsesDeclaration"
                        && edge["from"] == *owner
                        && edge["to"] == **declaration
                }),
                "owner `{owner}` lost declaration `{declaration}`"
            );
        }
    }
}

fn reachability(sites: &Sites) -> Value {
    json!({
        "schema": 1,
        "applicationCoverage": "complete",
        "components": [
            component("global", sites.global),
            component("home", sites.home),
            component("visit", sites.visit),
            component("counter", sites.counter),
            component("dead", sites.dead),
            component("shared-live", sites.shared_live),
            component("shared-dead", sites.shared_dead),
            component("co-home", sites.coowned),
            component("co-dead", sites.coowned),
        ],
        "routes": [
            {
                "id": "home",
                "path": "/",
                "components": ["global", "home", "shared-live", "co-home"],
            },
            {
                "id": "visit",
                "path": "/visit",
                "components": ["global", "visit"],
            },
        ],
        "islands": [{
            "id": "counter",
            "name": "counter",
            "components": ["counter"],
        }],
    })
}

fn reordered_reachability(mut document: Value) -> Value {
    for field in ["components", "routes", "islands"] {
        document[field]
            .as_array_mut()
            .expect("reachability collection must be an array")
            .reverse();
    }
    for component in document["components"].as_array_mut().unwrap() {
        component["sites"].as_array_mut().unwrap().reverse();
    }
    for route in document["routes"].as_array_mut().unwrap() {
        route["components"].as_array_mut().unwrap().reverse();
    }
    for island in document["islands"].as_array_mut().unwrap() {
        island["components"].as_array_mut().unwrap().reverse();
    }
    document
}

fn component(id: &str, site: Site) -> Value {
    json!({
        "id": id,
        "sites": [{
            "file": site.file,
            "byteStart": site.start,
            "byteEnd": site.end,
        }],
    })
}

fn bundle_plan(reordered: bool, second_theme: bool) -> String {
    let counter_theme = if second_theme { "true" } else { "false" };
    if reordered {
        format!(
            r#"schema = 1
targets = "modern"
format = "minified"

[theme]
kind = "seed"

[bundles.visit]
sources = ["src/visit.rs"]

[bundles.home]
sources = ["src/coowned.rs", "src/shared.rs", "src/home.rs"]

[bundles.global]
sources = ["src/global.rs"]
emit-theme = true

[bundles.dead]
sources = ["src/dead.rs"]

[bundles.counter]
sources = ["src/counter.rs"]
emit-theme = {counter_theme}
"#
        )
    } else {
        format!(
            r#"schema = 1
targets = "modern"
format = "minified"

[theme]
kind = "seed"

[bundles.counter]
sources = ["src/counter.rs"]
emit-theme = {counter_theme}

[bundles.dead]
sources = ["src/dead.rs"]

[bundles.global]
sources = ["src/global.rs"]
emit-theme = true

[bundles.home]
sources = ["src/home.rs", "src/shared.rs", "src/coowned.rs"]

[bundles.visit]
sources = ["src/visit.rs"]
"#
        )
    }
}

fn asset_arguments(
    output: &str,
    schema: u8,
    reachability: &str,
    prune: bool,
    check: bool,
) -> Vec<String> {
    let mut values = arguments(&[
        "bundle",
        "--plan",
        "pliego.bundles.toml",
        "--output-dir",
        output,
        "--manifest-version",
        &schema.to_string(),
        "--reachability",
        reachability,
        "--asset-plan",
    ]);
    if prune {
        values.push("--prune-unreachable".into());
    }
    if check {
        values.push("--check".into());
    }
    values
}

fn project_arguments(output: &str, reachability: &str, prune: bool, check: bool) -> Vec<String> {
    let mut values = asset_arguments(output, 5, reachability, prune, check);
    let position = values
        .iter()
        .position(|argument| argument == "--asset-plan")
        .expect("asset plan flag must exist");
    values.insert(position + 1, "--project-index".into());
    values
}

fn run(root: &Path, arguments: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("pliego-cssc must execute")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed with {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
    assert!(output.stderr.is_empty());
}

fn assert_failure(output: &Output, expected: &str) {
    assert!(!output.status.success(), "command unexpectedly succeeded");
    assert!(
        output.stdout.is_empty(),
        "failed command wrote stdout: {}",
        stdout(output)
    );
    assert!(
        stderr(output).contains(expected),
        "stderr did not contain `{expected}`:\n{}",
        stderr(output)
    );
}

fn output_snapshot(path: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(path)
        .expect("output directory must be readable")
        .map(|entry| {
            let entry = entry.expect("output entry must be readable");
            (
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path()).expect("output bytes must be readable"),
            )
        })
        .filter(|(name, _)| !name.ends_with(".pliego.lock"))
        .collect()
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("fixture parent must be created");
    }
    fs::write(path, contents).expect("fixture file must be written");
}

fn write_json(path: impl AsRef<Path>, value: &Value) {
    let mut bytes = serde_json::to_vec_pretty(value).expect("fixture JSON must serialize");
    bytes.push(b'\n');
    fs::write(path, bytes).expect("fixture JSON must be written");
}

fn read_json(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path).expect("JSON artifact must be readable"))
        .expect("JSON artifact must parse")
}

fn sites(file: &'static str, source: &str, invocation: &str) -> Vec<Site> {
    source
        .match_indices(invocation)
        .map(|(start, matched)| Site {
            file,
            start,
            end: start + matched.len(),
        })
        .collect()
}

fn unique_site(file: &'static str, source: &str, invocation: &str) -> Site {
    let found = sites(file, source, invocation);
    assert_eq!(found.len(), 1, "fixture invocation must be unique");
    found[0]
}

fn style_for_site(manifest: &Value, site: Site) -> &Value {
    manifest["styles"]
        .as_array()
        .expect("styles must be an array")
        .iter()
        .find(|style| origin_sites(style).contains(&site))
        .unwrap_or_else(|| panic!("no style owns {}:{}..{}", site.file, site.start, site.end))
}

fn origin_sites(style: &Value) -> BTreeSet<Site> {
    style["origins"]
        .as_array()
        .expect("style origins must be an array")
        .iter()
        .map(|origin| Site {
            file: match value_string(origin, "file") {
                "src/global.rs" => "src/global.rs",
                "src/home.rs" => "src/home.rs",
                "src/visit.rs" => "src/visit.rs",
                "src/counter.rs" => "src/counter.rs",
                "src/dead.rs" => "src/dead.rs",
                "src/shared.rs" => "src/shared.rs",
                "src/coowned.rs" => "src/coowned.rs",
                other => panic!("unexpected fixture origin `{other}`"),
            },
            start: value_usize(origin, "byteStart"),
            end: value_usize(origin, "byteEnd"),
        })
        .collect()
}

fn object_by_id<'a>(document: &'a Value, field: &str, id: &str) -> &'a Value {
    document[field]
        .as_array()
        .unwrap_or_else(|| panic!("`{field}` must be an array"))
        .iter()
        .find(|item| item["id"] == id)
        .unwrap_or_else(|| panic!("`{field}` lacks `{id}`"))
}

fn object_keys(value: &Value) -> BTreeSet<&str> {
    value
        .as_object()
        .expect("value must be an object")
        .keys()
        .map(String::as_str)
        .collect()
}

fn string_array(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .expect("value must be an array")
        .iter()
        .map(|item| item.as_str().expect("array item must be a string"))
        .collect()
}

fn value_string<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("`{field}` must be a string"))
}

fn value_usize(value: &Value, field: &str) -> usize {
    usize::try_from(
        value[field]
            .as_u64()
            .unwrap_or_else(|| panic!("`{field}` must be an integer")),
    )
    .expect("integer must fit usize")
}

fn usize_u64(value: usize) -> u64 {
    u64::try_from(value).expect("artifact length must fit u64")
}

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn css_text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("CSS must be UTF-8")
}

fn stdout(output: &Output) -> &str {
    std::str::from_utf8(&output.stdout).expect("stdout must be UTF-8")
}

fn stderr(output: &Output) -> &str {
    std::str::from_utf8(&output.stderr).expect("stderr must be UTF-8")
}
