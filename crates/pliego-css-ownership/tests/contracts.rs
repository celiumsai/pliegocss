//! Integration tests for the closed ownership and Asset Plan contracts.

use pliego_css_ownership::{
    AssetBundle, AssetIsland, AssetPlan, AssetRoute, AssetRuleSelection, BundlePackageInput,
    RouteCompositionInput, build_ownership_document, parse_asset_plan, parse_ownership,
};
use serde_json::{Value, json};

const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;

fn sample_plan_value() -> Value {
    let hash_a = "a".repeat(64);
    let hash_b = "b".repeat(64);
    json!({
        "schemaVersion": 1,
        "manifestSchemaVersion": 5,
        "graphSchemaVersion": 2,
        "ruleSelection": "reachable-style-ids",
        "originCoverage": "compiler-verified-complete",
        "applicationCoverage": "adapter-attested-complete",
        "styleIdFormatVersion": 1,
        "classNameFormatVersion": 1,
        "themeIdFormatVersion": 3,
        "themeId": "0123456789abcdef0123456789abcdef",
        "targets": "modern",
        "format": "minified",
        "bundles": [
            {
                "id": "visit",
                "cssFile": "visit.css",
                "manifestFile": "visit.manifest.json",
                "emitsTheme": false,
                "cssBytes": 120,
                "cssSha256": hash_a.clone(),
                "manifestBytes": 240,
                "manifestSha256": hash_b.clone()
            },
            {
                "id": "shared",
                "cssFile": "shared.css",
                "manifestFile": "shared.manifest.json",
                "emitsTheme": false,
                "cssBytes": 121,
                "cssSha256": hash_a.clone(),
                "manifestBytes": 241,
                "manifestSha256": hash_b.clone()
            },
            {
                "id": "global",
                "cssFile": "global.css",
                "manifestFile": "global.manifest.json",
                "emitsTheme": true,
                "cssBytes": 122,
                "cssSha256": hash_a.clone(),
                "manifestBytes": 242,
                "manifestSha256": hash_b.clone()
            },
            {
                "id": "counter",
                "cssFile": "counter.css",
                "manifestFile": "counter.manifest.json",
                "emitsTheme": false,
                "cssBytes": 123,
                "cssSha256": hash_a,
                "manifestBytes": 243,
                "manifestSha256": hash_b
            }
        ],
        "routes": [
            {"id": "route:visit", "path": "/visit", "bundles": ["visit", "global"]},
            {"id": "route:home", "path": "/", "bundles": ["global"]}
        ],
        "islands": [
            {"id": "island:shared", "name": "Shared", "bundles": ["shared", "global"]},
            {"id": "island:counter", "name": "Counter", "bundles": ["counter", "global"]}
        ]
    })
}

fn plan_bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec_pretty(value).expect("sample plan serializes")
}

fn sample_plan() -> (Vec<u8>, AssetPlan) {
    let bytes = plan_bytes(&sample_plan_value());
    let plan = parse_asset_plan(&bytes).expect("sample plan validates");
    (bytes, plan)
}

fn ownership_value(plan: &AssetPlan) -> Value {
    json!({
        "schemaVersion": 1,
        "ownershipCoverage": "adapter-attested-complete",
        "assetPlanBytes": plan.exact_bytes(),
        "assetPlanSha256": plan.sha256(),
        "bundlePackages": [
            {"bundleId": "shared", "packageId": "islands"},
            {"bundleId": "visit", "packageId": "app-shell"},
            {"bundleId": "global", "packageId": "app-shell"},
            {"bundleId": "counter", "packageId": "islands"}
        ],
        "routeCompositions": [
            {"routeId": "route:visit", "islandIds": ["island:shared", "island:counter"]},
            {"routeId": "route:home", "islandIds": []}
        ]
    })
}

fn parse_value(value: &Value, plan: &AssetPlan) -> pliego_css_ownership::Ownership {
    let bytes = serde_json::to_vec(value).expect("sample ownership serializes");
    parse_ownership(&bytes, plan).expect("sample ownership validates")
}

#[test]
fn resolves_packages_and_overlapping_route_views_in_canonical_plan_order() {
    let (_, plan) = sample_plan();

    assert_eq!(
        plan.bundles()
            .iter()
            .map(AssetBundle::id)
            .collect::<Vec<_>>(),
        ["global", "counter", "shared", "visit"]
    );
    assert_eq!(
        plan.routes().iter().map(AssetRoute::id).collect::<Vec<_>>(),
        ["route:home", "route:visit"]
    );
    assert_eq!(
        plan.islands()
            .iter()
            .map(AssetIsland::id)
            .collect::<Vec<_>>(),
        ["island:counter", "island:shared"]
    );

    let ownership = parse_value(&ownership_value(&plan), &plan);
    assert_eq!(ownership.packages().len(), 2);
    assert_eq!(ownership.packages()[0].package_id(), "app-shell");
    assert_eq!(ownership.packages()[0].bundle_ids(), ["global", "visit"]);
    assert_eq!(ownership.packages()[1].package_id(), "islands");
    assert_eq!(ownership.packages()[1].bundle_ids(), ["counter", "shared"]);

    let routes = ownership.composed_routes();
    assert_eq!(routes[0].route_id(), "route:home");
    assert_eq!(routes[0].path(), "/");
    assert!(routes[0].island_ids().is_empty());
    assert_eq!(routes[0].bundle_ids(), ["global"]);
    assert_eq!(routes[1].route_id(), "route:visit");
    assert_eq!(routes[1].island_ids(), ["island:counter", "island:shared"]);
    assert_eq!(
        routes[1].bundle_ids(),
        ["global", "counter", "shared", "visit"]
    );
}

#[test]
fn binds_ownership_to_exact_plan_length_and_sha256() {
    let (bytes, plan) = sample_plan();
    let valid = ownership_value(&plan);

    let mut wrong_length = valid.clone();
    wrong_length["assetPlanBytes"] = json!(plan.exact_bytes() + 1);
    let error = parse_ownership(&serde_json::to_vec(&wrong_length).unwrap(), &plan).unwrap_err();
    assert!(error.reason().contains("assetPlanBytes"));

    let mut wrong_hash = valid.clone();
    wrong_hash["assetPlanSha256"] = json!("0".repeat(64));
    let error = parse_ownership(&serde_json::to_vec(&wrong_hash).unwrap(), &plan).unwrap_err();
    assert!(error.reason().contains("assetPlanSha256"));

    let mut same_json_with_different_bytes = bytes;
    same_json_with_different_bytes.push(b'\n');
    let changed_plan = parse_asset_plan(&same_json_with_different_bytes).unwrap();
    let error = parse_ownership(&serde_json::to_vec(&valid).unwrap(), &changed_plan).unwrap_err();
    assert!(error.reason().contains("assetPlanBytes"));
}

#[test]
fn rejects_unknown_missing_and_duplicate_keys_at_every_closed_boundary() {
    let (_, plan) = sample_plan();

    let mut unknown_plan = sample_plan_value();
    unknown_plan["unexpected"] = json!(true);
    assert!(parse_asset_plan(&plan_bytes(&unknown_plan)).is_err());

    let mut unknown_bundle = sample_plan_value();
    unknown_bundle["bundles"][0]["unexpected"] = json!(true);
    assert!(parse_asset_plan(&plan_bytes(&unknown_bundle)).is_err());

    let mut missing_plan = sample_plan_value();
    missing_plan.as_object_mut().unwrap().remove("targets");
    assert!(parse_asset_plan(&plan_bytes(&missing_plan)).is_err());

    let duplicate_plan = br#"{"schemaVersion":1,"schema\u0056ersion":1}"#;
    let error = parse_asset_plan(duplicate_plan).unwrap_err();
    assert!(error.reason().contains("duplicate field"));

    let mut unknown_ownership = ownership_value(&plan);
    unknown_ownership["unexpected"] = json!(true);
    assert!(parse_ownership(&serde_json::to_vec(&unknown_ownership).unwrap(), &plan).is_err());

    let mut unknown_mapping = ownership_value(&plan);
    unknown_mapping["bundlePackages"][0]["unexpected"] = json!(true);
    assert!(parse_ownership(&serde_json::to_vec(&unknown_mapping).unwrap(), &plan).is_err());

    let mut missing = ownership_value(&plan);
    missing.as_object_mut().unwrap().remove("ownershipCoverage");
    assert!(parse_ownership(&serde_json::to_vec(&missing).unwrap(), &plan).is_err());

    let duplicate = br#"{"schemaVersion":1,"schema\u0056ersion":1}"#;
    let error = parse_ownership(duplicate, &plan).unwrap_err();
    assert!(error.reason().contains("duplicate field"));
}

#[test]
fn requires_total_exclusive_bundle_ownership_and_portable_package_ids() {
    let (_, plan) = sample_plan();

    let mut missing = ownership_value(&plan);
    missing["bundlePackages"].as_array_mut().unwrap().pop();
    let error = parse_ownership(&serde_json::to_vec(&missing).unwrap(), &plan).unwrap_err();
    assert!(error.reason().contains("every Asset Plan bundle"));

    let mut duplicate = ownership_value(&plan);
    duplicate["bundlePackages"][3]["bundleId"] = json!("global");
    let error = parse_ownership(&serde_json::to_vec(&duplicate).unwrap(), &plan).unwrap_err();
    assert!(error.reason().contains("duplicate ownership mapping"));

    let mut dangling = ownership_value(&plan);
    dangling["bundlePackages"][3]["bundleId"] = json!("missing");
    let error = parse_ownership(&serde_json::to_vec(&dangling).unwrap(), &plan).unwrap_err();
    assert!(error.reason().contains("unknown ownership bundle"));

    for invalid in [
        String::new(),
        "bad/package".to_owned(),
        "paquete-é".to_owned(),
        "a".repeat(129),
    ] {
        let mut ownership = ownership_value(&plan);
        ownership["bundlePackages"][0]["packageId"] = json!(invalid);
        let error = parse_ownership(&serde_json::to_vec(&ownership).unwrap(), &plan).unwrap_err();
        assert!(error.reason().contains("invalid packageId"));
    }
}

#[test]
fn requires_total_routes_and_unique_valid_islands_per_route() {
    let (_, plan) = sample_plan();

    let mut missing = ownership_value(&plan);
    missing["routeCompositions"].as_array_mut().unwrap().pop();
    let error = parse_ownership(&serde_json::to_vec(&missing).unwrap(), &plan).unwrap_err();
    assert!(error.reason().contains("every Asset Plan route"));

    let mut duplicate_route = ownership_value(&plan);
    duplicate_route["routeCompositions"][1]["routeId"] = json!("route:visit");
    let error = parse_ownership(&serde_json::to_vec(&duplicate_route).unwrap(), &plan).unwrap_err();
    assert!(error.reason().contains("duplicate ownership composition"));

    let mut dangling_route = ownership_value(&plan);
    dangling_route["routeCompositions"][1]["routeId"] = json!("route:missing");
    let error = parse_ownership(&serde_json::to_vec(&dangling_route).unwrap(), &plan).unwrap_err();
    assert!(error.reason().contains("unknown ownership route"));

    let mut unknown_island = ownership_value(&plan);
    unknown_island["routeCompositions"][0]["islandIds"] = json!(["island:missing"]);
    let error = parse_ownership(&serde_json::to_vec(&unknown_island).unwrap(), &plan).unwrap_err();
    assert!(error.reason().contains("unknown ownership island"));

    let mut duplicate_island = ownership_value(&plan);
    duplicate_island["routeCompositions"][0]["islandIds"] =
        json!(["island:counter", "island:counter"]);
    let error =
        parse_ownership(&serde_json::to_vec(&duplicate_island).unwrap(), &plan).unwrap_err();
    assert!(error.reason().contains("duplicate ownership island"));
}

#[test]
fn asset_plan_parser_supports_all_targets_and_rejects_unsafe_topology() {
    let mut baseline_plan = sample_plan_value();
    baseline_plan["targets"] = json!("baseline-widely");
    parse_asset_plan(&plan_bytes(&baseline_plan)).expect("baseline-widely Asset Plan");

    let mut unsupported_targets = sample_plan_value();
    unsupported_targets["targets"] = json!("legacy");
    let error = parse_asset_plan(&plan_bytes(&unsupported_targets)).unwrap_err();
    assert!(error.reason().contains("targets contract"));

    let mut duplicate_bundle = sample_plan_value();
    duplicate_bundle["bundles"][3]["id"] = json!("visit");
    duplicate_bundle["bundles"][3]["cssFile"] = json!("visit.css");
    duplicate_bundle["bundles"][3]["manifestFile"] = json!("visit.manifest.json");
    let error = parse_asset_plan(&plan_bytes(&duplicate_bundle)).unwrap_err();
    assert!(error.reason().contains("duplicate asset plan bundle"));

    let mut dangling = sample_plan_value();
    dangling["routes"][0]["bundles"] = json!(["global", "missing"]);
    let error = parse_asset_plan(&plan_bytes(&dangling)).unwrap_err();
    assert!(error.reason().contains("unknown bundle"));

    let mut duplicate_reference = sample_plan_value();
    duplicate_reference["routes"][0]["bundles"] = json!(["global", "global"]);
    let error = parse_asset_plan(&plan_bytes(&duplicate_reference)).unwrap_err();
    assert!(error.reason().contains("duplicate bundle"));

    let mut duplicate_route = sample_plan_value();
    duplicate_route["routes"][1]["id"] = json!("route:visit");
    let error = parse_asset_plan(&plan_bytes(&duplicate_route)).unwrap_err();
    assert!(error.reason().contains("duplicate asset plan route ID"));

    let mut unsafe_bundle = sample_plan_value();
    unsafe_bundle["bundles"][0]["id"] = json!("CON");
    unsafe_bundle["bundles"][0]["cssFile"] = json!("CON.css");
    unsafe_bundle["bundles"][0]["manifestFile"] = json!("CON.manifest.json");
    let error = parse_asset_plan(&plan_bytes(&unsafe_bundle)).unwrap_err();
    assert!(error.reason().contains("unsafe bundle name"));
}

#[test]
fn asset_plan_schema_versions_are_bound_to_rule_selection() {
    let reachable = parse_asset_plan(&plan_bytes(&sample_plan_value())).unwrap();
    assert_eq!(
        reachable.rule_selection(),
        AssetRuleSelection::ReachableStyleIds
    );

    let mut all_compiled = sample_plan_value();
    all_compiled["ruleSelection"] = json!("all-compiled");
    let all_compiled = parse_asset_plan(&plan_bytes(&all_compiled)).unwrap();
    assert_eq!(
        all_compiled.rule_selection(),
        AssetRuleSelection::AllCompiled
    );

    let mut retained = sample_plan_value();
    retained["schemaVersion"] = json!(2);
    retained["ruleSelection"] = json!("reachable-or-retained-style-ids");
    let retained = parse_asset_plan(&plan_bytes(&retained)).unwrap();
    assert_eq!(
        retained.rule_selection(),
        AssetRuleSelection::ReachableOrRetainedStyleIds
    );

    for (schema, selection) in [
        (1, "reachable-or-retained-style-ids"),
        (2, "all-compiled"),
        (2, "reachable-style-ids"),
        (3, "reachable-or-retained-style-ids"),
    ] {
        let mut invalid = sample_plan_value();
        invalid["schemaVersion"] = json!(schema);
        invalid["ruleSelection"] = json!(selection);
        let error = parse_asset_plan(&plan_bytes(&invalid)).unwrap_err();
        assert!(error.reason().contains("schemaVersion"));
        assert!(error.reason().contains("ruleSelection"));
    }
}

#[test]
fn enforces_document_and_item_limits_before_expensive_resolution() {
    let oversized = vec![b' '; MAX_DOCUMENT_BYTES + 1];
    let error = parse_asset_plan(&oversized).unwrap_err();
    assert!(error.reason().contains("16 MiB"));

    let (_, plan) = sample_plan();
    let mut too_many_mappings = ownership_value(&plan);
    too_many_mappings["bundlePackages"] = Value::Array(
        (0..65_536)
            .map(|_| json!({"bundleId": "global", "packageId": "app"}))
            .collect(),
    );
    let bytes = serde_json::to_vec(&too_many_mappings).unwrap();
    assert!(bytes.len() < MAX_DOCUMENT_BYTES);
    let error = parse_ownership(&bytes, &plan).unwrap_err();
    assert!(error.reason().contains("item limit"));

    let mut too_many_plan_references = sample_plan_value();
    too_many_plan_references["routes"][0]["bundles"] =
        Value::Array((0..65_536).map(|_| json!("global")).collect());
    let bytes = plan_bytes(&too_many_plan_references);
    assert!(bytes.len() < MAX_DOCUMENT_BYTES);
    let error = parse_asset_plan(&bytes).unwrap_err();
    assert!(error.reason().contains("item limit"));
}

#[test]
fn canonicalizes_equivalent_plan_orderings_for_resolved_consumers() {
    let original_bytes = plan_bytes(&sample_plan_value());
    let original = parse_asset_plan(&original_bytes).unwrap();

    let mut reordered_value = sample_plan_value();
    reordered_value["bundles"].as_array_mut().unwrap().reverse();
    reordered_value["routes"].as_array_mut().unwrap().reverse();
    reordered_value["islands"].as_array_mut().unwrap().reverse();
    for route in reordered_value["routes"].as_array_mut().unwrap() {
        route["bundles"].as_array_mut().unwrap().reverse();
    }
    for island in reordered_value["islands"].as_array_mut().unwrap() {
        island["bundles"].as_array_mut().unwrap().reverse();
    }
    let reordered_bytes = plan_bytes(&reordered_value);
    let reordered = parse_asset_plan(&reordered_bytes).unwrap();

    let original_bundles = original
        .bundles()
        .iter()
        .map(AssetBundle::id)
        .collect::<Vec<_>>();
    let reordered_bundles = reordered
        .bundles()
        .iter()
        .map(AssetBundle::id)
        .collect::<Vec<_>>();
    assert_eq!(original_bundles, reordered_bundles);

    let original_routes = original
        .routes()
        .iter()
        .map(|route| (route.id(), route.bundle_ids()))
        .collect::<Vec<_>>();
    let reordered_routes = reordered
        .routes()
        .iter()
        .map(|route| (route.id(), route.bundle_ids()))
        .collect::<Vec<_>>();
    assert_eq!(original_routes, reordered_routes);

    let original_islands = original
        .islands()
        .iter()
        .map(|island| (island.id(), island.bundle_ids()))
        .collect::<Vec<_>>();
    let reordered_islands = reordered
        .islands()
        .iter()
        .map(|island| (island.id(), island.bundle_ids()))
        .collect::<Vec<_>>();
    assert_eq!(original_islands, reordered_islands);
}

#[test]
fn canonical_builder_binds_exact_plan_bytes_and_round_trips() {
    let (_, plan) = sample_plan();
    let mappings = [
        BundlePackageInput::new("shared", "islands"),
        BundlePackageInput::new("visit", "app-shell"),
        BundlePackageInput::new("global", "app-shell"),
        BundlePackageInput::new("counter", "islands"),
    ];
    let visit_islands = ["island:shared", "island:counter"];
    let compositions = [
        RouteCompositionInput::new("route:visit", &visit_islands),
        RouteCompositionInput::new("route:home", &[]),
    ];

    let bytes = build_ownership_document(&plan, &mappings, &compositions).unwrap();
    assert!(bytes.starts_with(b"{\n  \"schemaVersion\": 1,\n"));
    assert!(bytes.ends_with(b"\n"));
    assert!(!bytes.ends_with(b"\n\n"));

    let document: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(document["assetPlanBytes"], plan.exact_bytes());
    assert_eq!(document["assetPlanSha256"], plan.sha256());
    assert_eq!(
        document["bundlePackages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|mapping| mapping["bundleId"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["counter", "global", "shared", "visit"]
    );
    assert_eq!(
        document["routeCompositions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|composition| composition["routeId"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["route:home", "route:visit"]
    );
    assert_eq!(
        document["routeCompositions"][1]["islandIds"],
        json!(["island:counter", "island:shared"])
    );

    let ownership = parse_ownership(&bytes, &plan).unwrap();
    assert_eq!(
        ownership.composed_routes()[1].bundle_ids(),
        ["global", "counter", "shared", "visit"]
    );
}

#[test]
fn canonical_builder_is_byte_stable_across_equivalent_input_orders() {
    let (_, plan) = sample_plan();
    let first_mappings = [
        BundlePackageInput::new("shared", "islands"),
        BundlePackageInput::new("visit", "app-shell"),
        BundlePackageInput::new("global", "app-shell"),
        BundlePackageInput::new("counter", "islands"),
    ];
    let second_mappings = [
        BundlePackageInput::new("counter", "islands"),
        BundlePackageInput::new("global", "app-shell"),
        BundlePackageInput::new("visit", "app-shell"),
        BundlePackageInput::new("shared", "islands"),
    ];
    let first_visit_islands = ["island:shared", "island:counter"];
    let second_visit_islands = ["island:counter", "island:shared"];
    let first_compositions = [
        RouteCompositionInput::new("route:visit", &first_visit_islands),
        RouteCompositionInput::new("route:home", &[]),
    ];
    let second_compositions = [
        RouteCompositionInput::new("route:home", &[]),
        RouteCompositionInput::new("route:visit", &second_visit_islands),
    ];

    let first = build_ownership_document(&plan, &first_mappings, &first_compositions).unwrap();
    let second = build_ownership_document(&plan, &second_mappings, &second_compositions).unwrap();
    assert_eq!(first, second);
}

#[test]
fn canonical_builder_rejects_invalid_adapter_inputs_through_parser_contract() {
    let (_, plan) = sample_plan();
    let complete_mappings = [
        BundlePackageInput::new("global", "app-shell"),
        BundlePackageInput::new("counter", "islands"),
        BundlePackageInput::new("shared", "islands"),
        BundlePackageInput::new("visit", "app-shell"),
    ];
    let visit_islands = ["island:counter", "island:shared"];
    let complete_compositions = [
        RouteCompositionInput::new("route:home", &[]),
        RouteCompositionInput::new("route:visit", &visit_islands),
    ];

    let error = build_ownership_document(&plan, &complete_mappings[..3], &complete_compositions)
        .unwrap_err();
    assert!(error.reason().contains("every Asset Plan bundle"));

    let invalid_package = [
        BundlePackageInput::new("global", "bad/package"),
        BundlePackageInput::new("counter", "islands"),
        BundlePackageInput::new("shared", "islands"),
        BundlePackageInput::new("visit", "app-shell"),
    ];
    let error =
        build_ownership_document(&plan, &invalid_package, &complete_compositions).unwrap_err();
    assert!(error.reason().contains("invalid packageId"));

    let unknown_route = [
        RouteCompositionInput::new("route:missing", &[]),
        RouteCompositionInput::new("route:visit", &visit_islands),
    ];
    let error = build_ownership_document(&plan, &complete_mappings, &unknown_route).unwrap_err();
    assert!(error.reason().contains("unknown ownership route"));

    let duplicate_islands = ["island:counter", "island:counter"];
    let duplicate_composition = [
        RouteCompositionInput::new("route:home", &[]),
        RouteCompositionInput::new("route:visit", &duplicate_islands),
    ];
    let error =
        build_ownership_document(&plan, &complete_mappings, &duplicate_composition).unwrap_err();
    assert!(error.reason().contains("duplicate ownership island"));
}
