//! Internal asset plan tests.
use serde_json::{Value, json};

use super::*;

const GLOBAL: &str = "component:app::global";
const HOME: &str = "component:app::home";
const VISIT: &str = "component:app::visit";

#[test]
fn physical_layer_roles_are_closed_and_declaration_safe() {
    let rules = [
        PhysicalRuleDocument {
            id: "css-rule:00000000".into(),
            ordinal: 0,
            kind: "layer-order".into(),
        },
        PhysicalRuleDocument {
            id: "css-rule:00000001".into(),
            ordinal: 1,
            kind: "layer".into(),
        },
        PhysicalRuleDocument {
            id: "css-rule:00000002".into(),
            ordinal: 2,
            kind: "qualified".into(),
        },
    ];
    let role_map = validate_physical_rules(&rules).expect("layer roles must validate");
    assert_eq!(
        role_map.get("css-rule:00000000"),
        Some(&PhysicalRuleRole::Statement)
    );
    assert_eq!(
        role_map.get("css-rule:00000001"),
        Some(&PhysicalRuleRole::Group)
    );
    assert_eq!(
        role_map.get("css-rule:00000002"),
        Some(&PhysicalRuleRole::Qualified)
    );

    let declaration = [PhysicalDeclarationDocument {
        id: "css-decl:00000000:00000000".into(),
        ordinal: 0,
    }];
    assert!(validate_physical_declarations(&declaration, &role_map).is_err());
}

#[test]
#[allow(clippy::too_many_lines)]
fn schema_one_plan_is_integrity_bound_and_canonical() {
    let global_css = b":root{--space:1rem}\n";
    let home_css = b".pc_home{padding:var(--space)}\n";
    let visit_css = b".pc_visit{display:block}\n";
    let global_manifest = manifest(4, global_css, &[GLOBAL], false);
    let home_manifest = manifest(4, home_css, &[HOME], false);
    let visit_manifest = manifest(4, visit_css, &[VISIT], false);
    let inputs = [
        AssetPlanBundle::new("visit", false, visit_css, &visit_manifest),
        AssetPlanBundle::new("home", false, home_css, &home_manifest),
        AssetPlanBundle::new("global", true, global_css, &global_manifest),
    ];

    let bytes = build_asset_plan(&inputs, AssetRuleSelection::AllCompiled).unwrap();
    assert_eq!(bytes.last(), Some(&b'\n'));
    assert!(!bytes.contains(&b'\r'));
    let plan: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(plan["schemaVersion"], 1);
    assert_eq!(plan["manifestSchemaVersion"], 4);
    assert_eq!(plan["graphSchemaVersion"], 1);
    assert_eq!(plan["ruleSelection"], "all-compiled");
    assert_eq!(plan["originCoverage"], "compiler-verified-complete");
    assert_eq!(plan["applicationCoverage"], "adapter-attested-complete");
    assert_eq!(plan["styleIdFormatVersion"], 2);
    assert_eq!(plan["classNameFormatVersion"], 1);
    assert_eq!(plan["themeIdFormatVersion"], 3);
    assert_eq!(plan["themeId"], "0123456789abcdef0123456789abcdef");
    assert_eq!(plan["targets"], "modern");
    assert_eq!(plan["format"], "minified");

    let bundles = plan["bundles"].as_array().unwrap();
    assert_eq!(bundle_ids(bundles), vec!["global", "home", "visit"]);
    assert_eq!(bundles[0]["cssFile"], "global.css");
    assert_eq!(bundles[0]["manifestFile"], "global.manifest.json");
    assert_eq!(bundles[0]["emitsTheme"], true);
    assert_eq!(bundles[0]["cssBytes"], global_css.len());
    assert_eq!(bundles[0]["cssSha256"], sha256_hex(global_css));
    assert_eq!(bundles[0]["manifestBytes"], global_manifest.len());
    assert_eq!(bundles[0]["manifestSha256"], sha256_hex(&global_manifest));

    let routes = plan["routes"].as_array().unwrap();
    assert_eq!(routes[0]["id"], "route:home");
    assert_eq!(routes[0]["path"], "/");
    assert_eq!(string_array(&routes[0]["bundles"]), vec!["global", "home"]);
    assert_eq!(routes[1]["id"], "route:visit");
    assert_eq!(string_array(&routes[1]["bundles"]), vec!["global"]);

    let islands = plan["islands"].as_array().unwrap();
    assert_eq!(islands[0]["id"], "island:visit-counter");
    assert_eq!(islands[0]["name"], "visit-counter");
    assert_eq!(
        string_array(&islands[0]["bundles"]),
        vec!["global", "visit"]
    );

    let reordered = [inputs[2], inputs[0], inputs[1]];
    assert_eq!(
        bytes,
        build_asset_plan(&reordered, AssetRuleSelection::AllCompiled).unwrap()
    );
}

#[test]
fn schema_five_physical_graph_and_reachable_selection_are_supported() {
    let css = b".pc_home{padding:1rem}\n";
    let manifest = manifest(5, css, &[HOME], false);
    let input = [AssetPlanBundle::new("home", false, css, &manifest)];
    let bytes = build_asset_plan(&input, AssetRuleSelection::ReachableStyleIds).unwrap();
    let plan: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(plan["schemaVersion"], 1);
    assert_eq!(plan["manifestSchemaVersion"], 5);
    assert_eq!(plan["graphSchemaVersion"], 2);
    assert_eq!(plan["ruleSelection"], "reachable-style-ids");
    assert_eq!(string_array(&plan["routes"][0]["bundles"]), vec!["home"]);
    assert!(string_array(&plan["routes"][1]["bundles"]).is_empty());
    assert!(string_array(&plan["islands"][0]["bundles"]).is_empty());
}

#[test]
fn retained_selection_versions_asset_plan_and_project_index_together() {
    let css = b".pc_home{padding:1rem}\n";
    let manifest = manifest(5, css, &[HOME], false);
    let input = [AssetPlanBundle::new("home", false, css, &manifest)];

    for (selection, expected) in [
        (AssetRuleSelection::AllCompiled, "all-compiled"),
        (AssetRuleSelection::ReachableStyleIds, "reachable-style-ids"),
    ] {
        let index_bytes = crate::artifacts::build_project_index(&input, &[], selection).unwrap();
        let index: Value = serde_json::from_slice(&index_bytes).unwrap();
        assert_eq!(index["schemaVersion"], 1);
        assert_eq!(index["ruleSelection"], expected);
    }

    let asset_bytes =
        build_asset_plan(&input, AssetRuleSelection::ReachableOrRetainedStyleIds).unwrap();
    let asset_plan: Value = serde_json::from_slice(&asset_bytes).unwrap();
    assert_eq!(asset_plan["schemaVersion"], 2);
    assert_eq!(
        asset_plan["ruleSelection"],
        "reachable-or-retained-style-ids"
    );

    let index_bytes = crate::artifacts::build_project_index(
        &input,
        &[],
        AssetRuleSelection::ReachableOrRetainedStyleIds,
    )
    .unwrap();
    let index: Value = serde_json::from_slice(&index_bytes).unwrap();
    assert_eq!(index["schemaVersion"], 2);
    assert_eq!(index["ruleSelection"], "reachable-or-retained-style-ids");
    assert_eq!(index["assetPlanBytes"], asset_bytes.len());
    assert_eq!(index["assetPlanSha256"], sha256_hex(&asset_bytes));
}

#[test]
fn baseline_profile_is_a_supported_asset_identity() {
    let css = b".pc_home{display:flex}\n";
    let mut document: Value = serde_json::from_slice(&manifest(4, css, &[HOME], false)).unwrap();
    document["targets"] = json!("baseline-widely");
    let mut manifest = serde_json::to_vec_pretty(&document).unwrap();
    manifest.push(b'\n');
    let input = [AssetPlanBundle::new("home", false, css, &manifest)];
    let bytes = build_asset_plan(&input, AssetRuleSelection::AllCompiled).unwrap();
    let plan: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(plan["targets"], "baseline-widely");
}

#[test]
fn schema_five_binds_theme_emission_to_the_physical_producer() {
    let css = b":root{--space:1rem}.pc_global{margin:0}\n";
    let manifest = manifest(5, css, &[GLOBAL], true);
    let valid = [AssetPlanBundle::new("global", true, css, &manifest)];
    assert!(build_asset_plan(&valid, AssetRuleSelection::AllCompiled).is_ok());

    let invalid = [AssetPlanBundle::new("global", false, css, &manifest)];
    let error = build_asset_plan(&invalid, AssetRuleSelection::AllCompiled).unwrap_err();
    assert!(error.contains("theme-emission flag"));

    let invalid_kind = rewrite(&manifest, |value| {
        value["graph"]["syntheticProducers"][0]["kind"] = json!("bogus");
    });
    let invalid = [AssetPlanBundle::new("global", true, css, &invalid_kind)];
    let error = build_asset_plan(&invalid, AssetRuleSelection::AllCompiled).unwrap_err();
    assert!(error.contains("unsupported physical producer"));
}

#[test]
fn bundle_ids_and_theme_provider_are_strict() {
    for valid in ["a", "global", "route-2", &"a".repeat(64)] {
        validate_asset_bundle_id(valid).unwrap();
    }
    for invalid in [
        "", "2route", "UPPER", "a_b", "a--b", "a-", "con", "com9", "lpt1",
    ] {
        assert!(validate_asset_bundle_id(invalid).is_err(), "{invalid}");
    }
    assert!(validate_asset_bundle_id(&"a".repeat(65)).is_err());

    let css = b".x{}\n";
    let manifest = manifest(4, css, &[HOME], false);
    let duplicate = [
        AssetPlanBundle::new("home", false, css, &manifest),
        AssetPlanBundle::new("home", false, css, &manifest),
    ];
    assert!(
        build_asset_plan(&duplicate, AssetRuleSelection::AllCompiled)
            .unwrap_err()
            .contains("duplicate asset bundle")
    );
    let themes = [
        AssetPlanBundle::new("global", true, css, &manifest),
        AssetPlanBundle::new("theme", true, css, &manifest),
    ];
    assert!(
        build_asset_plan(&themes, AssetRuleSelection::AllCompiled)
            .unwrap_err()
            .contains("at most one")
    );
}

#[test]
fn css_integrity_is_verified_before_topology() {
    let css = b".pc_home{display:block}\n";
    let other = b".pc_home{display:table}\n";
    let manifest = manifest(4, css, &[HOME], false);
    let input = [AssetPlanBundle::new("home", false, other, &manifest)];
    assert!(
        build_asset_plan(&input, AssetRuleSelection::AllCompiled)
            .unwrap_err()
            .contains("CSS SHA-256")
    );

    let bad_size = rewrite(&manifest, |value| value["cssBytes"] = json!(999));
    let input = [AssetPlanBundle::new("home", false, css, &bad_size)];
    assert!(
        build_asset_plan(&input, AssetRuleSelection::AllCompiled)
            .unwrap_err()
            .contains("CSS byte count")
    );
}

#[test]
fn build_identity_and_application_topology_must_be_common() {
    let css = b".x{}\n";
    let global = manifest(4, css, &[GLOBAL], false);
    let home = manifest(4, css, &[HOME], false);
    let other_theme = rewrite(&home, |value| {
        value["themeId"] = json!("fedcba9876543210fedcba9876543210");
    });
    let inputs = [
        AssetPlanBundle::new("global", true, css, &global),
        AssetPlanBundle::new("home", false, css, &other_theme),
    ];
    assert!(
        build_asset_plan(&inputs, AssetRuleSelection::AllCompiled)
            .unwrap_err()
            .contains("incompatible build identity")
    );

    let other_topology = rewrite(&home, |value| {
        value["graph"]["routes"][0]["path"] = json!("/changed");
    });
    let inputs = [
        AssetPlanBundle::new("global", true, css, &global),
        AssetPlanBundle::new("home", false, css, &other_topology),
    ];
    assert!(
        build_asset_plan(&inputs, AssetRuleSelection::AllCompiled)
            .unwrap_err()
            .contains("incompatible application topology")
    );
}

#[test]
fn schema_coverage_and_physical_contract_fail_closed() {
    let css = b".x{}\n";
    let schema_four = manifest(4, css, &[HOME], false);
    let incomplete = rewrite(&schema_four, |value| {
        value["graph"]["originCoverage"] = json!("partial");
    });
    assert_manifest_error(css, &incomplete, "origin coverage");

    let unsupported = rewrite(&schema_four, |value| value["schemaVersion"] = json!(3));
    assert_manifest_error(css, &unsupported, "expected 4 or 5");

    let mismatched = rewrite(&schema_four, |value| value["schemaVersion"] = json!(5));
    assert_manifest_error(css, &mismatched, "incompatible");

    let schema_five = manifest(5, css, &[HOME], false);
    let missing_physical = rewrite(&schema_five, |value| {
        value["graph"]
            .as_object_mut()
            .unwrap()
            .remove("physicalRules");
    });
    assert_manifest_error(css, &missing_physical, "lacks physical rules");
}

#[test]
fn graph_ids_edges_and_ownership_fail_closed() {
    let css = b".x{}\n";
    let manifest = manifest(4, css, &[HOME], false);

    let dangling = rewrite(&manifest, |value| {
        let edges = value["graph"]["edges"].as_array_mut().unwrap();
        let edge = edges
            .iter_mut()
            .find(|edge| edge["kind"] == "componentUsesDeclaration")
            .unwrap();
        edge["from"] = json!("component:missing");
    });
    assert_manifest_error(css, &dangling, "dangling endpoint");

    let duplicate = rewrite(&manifest, |value| {
        let edges = value["graph"]["edges"].as_array_mut().unwrap();
        edges.push(edges[0].clone());
    });
    assert_manifest_error(css, &duplicate, "duplicate manifest graph edge");

    let unowned = rewrite(&manifest, |value| {
        value["graph"]["edges"]
            .as_array_mut()
            .unwrap()
            .retain(|edge| edge["kind"] != "componentUsesDeclaration");
    });
    assert_manifest_error(css, &unowned, "component ownership");

    let invalid_id = rewrite(&manifest, |value| {
        value["graph"]["declarations"][0]["id"] = json!("decl:bad");
    });
    assert_manifest_error(css, &invalid_id, "invalid declaration ID");

    let unknown_edge = rewrite(&manifest, |value| {
        value["graph"]["edges"][0]["kind"] = json!("unknownEdge");
    });
    assert_manifest_error(css, &unknown_edge, "unknown graph edge kind");
}

#[test]
fn physical_endpoint_and_cycle_validation_fail_closed() {
    let css = b".x{}\n";
    let manifest = manifest(5, css, &[HOME], false);
    let wrong_rule = rewrite(&manifest, |value| {
        value["graph"]["physicalRules"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id": "css-rule:00000001",
                "ordinal": 1,
                "kind": "qualified"
            }));
        let edge = value["graph"]["edges"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|edge| edge["kind"] == "physicalDeclarationBelongsToRule")
            .unwrap();
        edge["to"] = json!("css-rule:00000001");
    });
    assert_manifest_error(css, &wrong_rule, "wrong rule");

    let cycle = rewrite(&manifest, |value| {
        value["graph"]["physicalRules"]
            .as_array_mut()
            .unwrap()
            .extend([
                json!({
                    "id": "css-rule:00000001",
                    "ordinal": 1,
                    "kind": "media"
                }),
                json!({
                    "id": "css-rule:00000002",
                    "ordinal": 2,
                    "kind": "media"
                }),
            ]);
        let edges = value["graph"]["edges"].as_array_mut().unwrap();
        edges.push(json!({
            "kind": "ruleNestedInRule",
            "from": "css-rule:00000001",
            "to": "css-rule:00000002"
        }));
        edges.push(json!({
            "kind": "ruleNestedInRule",
            "from": "css-rule:00000002",
            "to": "css-rule:00000001"
        }));
    });
    assert_manifest_error(css, &cycle, "cycle");

    let unknown_kind = rewrite(&manifest, |value| {
        value["graph"]["physicalRules"][0]["kind"] = json!("supports");
    });
    assert_manifest_error(css, &unknown_kind, "unsupported physical rule kind");

    let non_media_parent = rewrite(&manifest, |value| {
        value["graph"]["physicalRules"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id": "css-rule:00000001",
                "ordinal": 1,
                "kind": "qualified"
            }));
        value["graph"]["edges"].as_array_mut().unwrap().push(json!({
            "kind": "ruleNestedInRule",
            "from": "css-rule:00000001",
            "to": "css-rule:00000000"
        }));
    });
    assert_manifest_error(css, &non_media_parent, "non-group parent");
}

#[test]
fn hash_and_limit_helpers_have_exact_boundaries() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert!(validate_plan_length(MAX_ASSET_BYTES - 1).is_ok());
    assert!(validate_plan_length(MAX_ASSET_BYTES).is_err());
    assert_eq!(checked_sum([MAX_ITEMS, 0]).unwrap(), MAX_ITEMS);
    assert!(checked_sum([usize::MAX, 1]).is_err());
    assert!(checked_add(usize::MAX, 1).is_err());

    let bundles = [PreparedBundle {
        id: "global".into(),
        emits_theme: true,
        css_bytes: 1,
        css_sha256: String::new(),
        manifest_bytes: 1,
        manifest_sha256: String::new(),
    }];
    assert!(
        selected_bundles(&bundles, &BTreeMap::new(), &BTreeSet::new(), 0)
            .unwrap_err()
            .contains("item limit")
    );
    assert_eq!(
        selected_bundles(&bundles, &BTreeMap::new(), &BTreeSet::new(), 1).unwrap(),
        ["global"]
    );

    let mut parents = BTreeMap::new();
    for index in 0..4_096_u32 {
        parents.insert(format!("rule:{index}"), format!("rule:{}", index + 1));
    }
    validate_rule_parent_acyclic(&parents).unwrap();
}

#[allow(clippy::too_many_lines)]
fn manifest(schema: u8, css: &[u8], active_components: &[&str], emits_theme: bool) -> Vec<u8> {
    let mut styles = Vec::new();
    let mut declarations = Vec::new();
    let mut edges = application_edges();
    let mut physical_rules = Vec::new();
    let mut physical_declarations = Vec::new();
    let mut synthetic_producers = Vec::new();

    for (index, component) in active_components.iter().enumerate() {
        let style_id = style_id(component);
        let declaration_id = format!("decl:{style_id}:00000000");
        styles.push(json!({
            "styleId": style_id,
            "className": format!("pc_{}", &style_id[24..]),
            "origins": []
        }));
        declarations.push(json!({
            "id": declaration_id,
            "styleId": style_id,
            "ordinal": 0
        }));
        edges.push(json!({
            "kind": "styleHasDeclaration",
            "from": format!("style:{style_id}"),
            "to": declaration_id
        }));
        edges.push(json!({
            "kind": "componentUsesDeclaration",
            "from": component,
            "to": declaration_id
        }));
        if schema == 5 {
            add_physical_semantic(
                u32::try_from(index).unwrap(),
                &declaration_id,
                &mut physical_rules,
                &mut physical_declarations,
                &mut edges,
            );
        }
    }

    if schema == 5 && emits_theme {
        synthetic_producers.push(json!({"id": "producer:theme", "kind": "theme"}));
        let ordinal = u32::try_from(active_components.len()).unwrap();
        let rule_id = format!("css-rule:{ordinal:08x}");
        let declaration_id = format!("css-decl:{ordinal:08x}:00000000");
        physical_rules.push(physical_rule(ordinal));
        physical_declarations.push(physical_declaration(ordinal));
        edges.push(json!({
            "kind": "syntheticProducerProducesPhysicalDeclaration",
            "from": "producer:theme",
            "to": declaration_id
        }));
        edges.push(json!({
            "kind": "physicalDeclarationBelongsToRule",
            "from": declaration_id,
            "to": rule_id
        }));
    }

    let mut graph = json!({
        "schemaVersion": if schema == 5 { 2 } else { 1 },
        "declarationIdFormatVersion": 1,
        "originCoverage": "compiler-verified-complete",
        "applicationCoverage": "adapter-attested-complete",
        "declarations": declarations,
        "tokens": [],
        "components": [
            {"id": GLOBAL},
            {"id": HOME},
            {"id": VISIT}
        ],
        "routes": [
            {"id": "route:home", "path": "/"},
            {"id": "route:visit", "path": "/visit"}
        ],
        "islands": [
            {"id": "island:visit-counter", "name": "visit-counter"}
        ],
        "edges": edges
    });
    if schema == 5 {
        let object = graph.as_object_mut().unwrap();
        object.insert("physicalRuleIdFormatVersion".into(), json!(1));
        object.insert("physicalDeclarationIdFormatVersion".into(), json!(1));
        object.insert(
            "physicalCoverage".into(),
            json!("compiler-verified-complete"),
        );
        object.insert("syntheticProducers".into(), json!(synthetic_producers));
        object.insert("physicalRules".into(), json!(physical_rules));
        object.insert("physicalDeclarations".into(), json!(physical_declarations));
    }

    let document = json!({
        "schemaVersion": schema,
        "styleIdFormatVersion": 2,
        "classNameFormatVersion": 1,
        "themeIdFormatVersion": 3,
        "themeId": "0123456789abcdef0123456789abcdef",
        "targets": "modern",
        "format": "minified",
        "cssSha256": sha256_hex(css),
        "cssBytes": css.len(),
        "styles": styles,
        "graph": graph
    });
    let mut bytes = serde_json::to_vec_pretty(&document).unwrap();
    bytes.push(b'\n');
    bytes
}

fn application_edges() -> Vec<Value> {
    vec![
        json!({
            "kind": "routeUsesComponent",
            "from": "route:home",
            "to": GLOBAL
        }),
        json!({
            "kind": "routeUsesComponent",
            "from": "route:home",
            "to": HOME
        }),
        json!({
            "kind": "routeUsesComponent",
            "from": "route:visit",
            "to": GLOBAL
        }),
        json!({
            "kind": "islandUsesComponent",
            "from": "island:visit-counter",
            "to": VISIT
        }),
    ]
}

fn add_physical_semantic(
    ordinal: u32,
    semantic_declaration: &str,
    rules: &mut Vec<Value>,
    declarations: &mut Vec<Value>,
    edges: &mut Vec<Value>,
) {
    let rule_id = format!("css-rule:{ordinal:08x}");
    let declaration_id = format!("css-decl:{ordinal:08x}:00000000");
    rules.push(physical_rule(ordinal));
    declarations.push(physical_declaration(ordinal));
    edges.push(json!({
        "kind": "declarationContributesToPhysicalDeclaration",
        "from": semantic_declaration,
        "to": declaration_id
    }));
    edges.push(json!({
        "kind": "physicalDeclarationBelongsToRule",
        "from": declaration_id,
        "to": rule_id
    }));
}

fn physical_rule(ordinal: u32) -> Value {
    json!({
        "id": format!("css-rule:{ordinal:08x}"),
        "ordinal": ordinal,
        "kind": "qualified",
        "byteStart": 0,
        "byteEnd": 1,
        "headerByteStart": 0,
        "headerByteEnd": 1
    })
}

fn physical_declaration(rule_ordinal: u32) -> Value {
    json!({
        "id": format!("css-decl:{rule_ordinal:08x}:00000000"),
        "ordinal": 0,
        "property": "display",
        "important": false,
        "generated": false,
        "byteStart": 0,
        "byteEnd": 1,
        "propertyByteStart": 0,
        "propertyByteEnd": 1,
        "valueByteStart": 0,
        "valueByteEnd": 1
    })
}

fn style_id(component: &str) -> &'static str {
    match component {
        GLOBAL => "00000000000000000000000000000001",
        HOME => "00000000000000000000000000000002",
        VISIT => "00000000000000000000000000000003",
        _ => panic!("unknown test component"),
    }
}

fn rewrite(source: &[u8], mutate: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(source).unwrap();
    mutate(&mut value);
    serde_json::to_vec_pretty(&value).unwrap()
}

fn assert_manifest_error(css: &[u8], manifest: &[u8], expected: &str) {
    let input = [AssetPlanBundle::new("home", false, css, manifest)];
    let error = build_asset_plan(&input, AssetRuleSelection::AllCompiled).unwrap_err();
    assert!(
        error.contains(expected),
        "expected {expected:?}, got {error:?}"
    );
}

fn bundle_ids(bundles: &[Value]) -> Vec<&str> {
    bundles
        .iter()
        .map(|bundle| bundle["id"].as_str().unwrap())
        .collect()
}

fn string_array(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap())
        .collect()
}
