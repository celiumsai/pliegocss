//! Usage-analysis, observation, and retention contract tests.

use pliego_css_build::artifacts::{AssetRuleSelection, sha256_hex};
use pliego_css_ir::StyleId;
use pliego_css_usage::{
    UsageObservationCoverage, UsageObservationInput, UsageObservationScopeInput,
    UsageObservedStyleInput, UsageOriginInput, UsageRetentionEntryInput, UsageRetentionInput,
    UsageStyleCollector, UsageStyleInput, build_usage_analysis, build_usage_observation,
    build_usage_retention, parse_usage_analysis, parse_usage_observation, parse_usage_retention,
    prepare_usage_analysis, verify_usage_analysis,
};
use serde_json::{Value, json};

const LIVE_STYLE: &str = "11111111111111111111111111111111";
const DEAD_STYLE: &str = "22222222222222222222222222222222";
const OTHER_STYLE: &str = "33333333333333333333333333333333";
const MIXED_STYLE: &str = "44444444444444444444444444444444";

fn class_name(style_id: &str) -> String {
    StyleId::new(u128::from_str_radix(style_id, 16).unwrap()).to_class_name()
}

fn origin(file: &str, start: usize, end: usize) -> UsageOriginInput {
    UsageOriginInput::new("p-4", file, start, end, "pc", "rust-macro")
}

fn style_for(bundle: &str, style_id: &str, origins: Vec<UsageOriginInput>) -> UsageStyleInput {
    UsageStyleInput::new(bundle, style_id, class_name(style_id), origins)
}

fn style(style_id: &str, origins: Vec<UsageOriginInput>) -> UsageStyleInput {
    style_for("app", style_id, origins)
}

fn reachability() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema": 1,
        "applicationCoverage": "complete",
        "components": [
            {"id": "dead", "sites": [{"file": "src/dead.rs", "byteStart": 1, "byteEnd": 5}]},
            {"id": "live", "sites": [{"file": "src/live.rs", "byteStart": 2, "byteEnd": 6}]},
            {"id": "shared-dead", "sites": [{"file": "src/shared.rs", "byteStart": 3, "byteEnd": 7}]}
        ],
        "routes": [{"id": "home", "path": "/", "components": ["live"]}],
        "islands": []
    }))
    .unwrap()
}

fn observation(universe: &str, reachability: &[u8], observed: &Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": 1,
        "universeSha256": universe,
        "reachabilitySha256": sha256_hex(reachability),
        "coverage": "sampled",
        "producer": {"name": "fixture", "version": "1"},
        "contexts": {
            "routes": ["/"],
            "islands": [],
            "themes": ["light"],
            "browsers": ["chromium"],
            "viewports": ["1280x720"],
            "states": ["default"]
        },
        "unknownDynamicInputs": ["authenticated-user"],
        "observedStyles": observed
    }))
    .unwrap()
}

fn retention(
    universe: &str,
    reachability: &[u8],
    entries: Vec<UsageRetentionEntryInput>,
) -> Vec<u8> {
    build_usage_retention(UsageRetentionInput::new(
        universe,
        sha256_hex(reachability),
        entries,
    ))
    .unwrap()
}

fn grant(id: &str, bundle: &str, style_id: &str) -> UsageRetentionEntryInput {
    UsageRetentionEntryInput::new(
        id,
        bundle,
        style_id,
        format!("Keep {bundle}/{style_id} for an external renderer."),
    )
}

fn style_value<'a>(document: &'a Value, bundle: &str, style_id: &str) -> &'a Value {
    document["styles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|style| style["bundleId"] == bundle && style["styleId"] == style_id)
        .unwrap()
}

fn universe(styles: &[UsageStyleInput], reachability: &[u8]) -> String {
    prepare_usage_analysis(
        styles,
        Some(reachability),
        None,
        None,
        AssetRuleSelection::AllCompiled,
    )
    .unwrap()
    .universe_sha256()
    .to_owned()
}

#[test]
fn schema_1_without_evidence_is_stable_unknown_and_round_trips() {
    let styles = [style(LIVE_STYLE, vec![origin("src/live.rs", 2, 6)])];
    let bytes = build_usage_analysis(&styles, None, None, AssetRuleSelection::AllCompiled).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["stateModelVersion"], 1);
    assert!(value.get("retention").is_none());
    assert!(value["summary"].get("removalPolicyRetained").is_none());
    assert!(value["styles"][0].get("retention").is_none());
    assert_eq!(value["styles"][0]["staticReachability"], "unknown");
    assert_eq!(value["styles"][0]["usageState"], "unknown");
    assert_eq!(value["styles"][0]["removalDisposition"], "blocked");
    assert_eq!(parse_usage_analysis(&bytes).unwrap().schema_version(), 1);
    assert_eq!(
        sha256_hex(&bytes),
        "678dce44626b749a2dce6d4ade5e97eb9dc39df57ea803165c93a6baeb25d9f2"
    );
}

#[test]
fn complete_graph_separates_reachable_shared_and_dead_styles() {
    let reachability = reachability();
    let bytes = build_usage_analysis(
        &[
            style(
                LIVE_STYLE,
                vec![origin("src/live.rs", 2, 6), origin("src/shared.rs", 3, 7)],
            ),
            style(DEAD_STYLE, vec![origin("src/dead.rs", 1, 5)]),
        ],
        Some(&reachability),
        None,
        AssetRuleSelection::AllCompiled,
    )
    .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["summary"]["staticReachable"], 1);
    assert_eq!(value["summary"]["usageDead"], 1);
    assert_eq!(
        style_value(&value, "app", DEAD_STYLE)["removalDisposition"],
        "candidate"
    );
    assert_eq!(
        style_value(&value, "app", LIVE_STYLE)["staticReachability"],
        "reachable"
    );

    let prepared = prepare_usage_analysis(
        &[
            style(LIVE_STYLE, vec![origin("src/live.rs", 2, 6)]),
            style(DEAD_STYLE, vec![origin("src/dead.rs", 1, 5)]),
        ],
        Some(&reachability),
        None,
        None,
        AssetRuleSelection::ReachableStyleIds,
    )
    .unwrap();
    assert!(prepared.selection().contains("app", LIVE_STYLE));
    assert!(!prepared.selection().contains("app", DEAD_STYLE));
}

#[test]
fn observation_absence_is_unobserved_and_positive_dead_hit_is_contradictory() {
    let reachability = reachability();
    let styles = [
        style(LIVE_STYLE, vec![origin("src/live.rs", 2, 6)]),
        style(DEAD_STYLE, vec![origin("src/dead.rs", 1, 5)]),
    ];
    let universe = universe(&styles, &reachability);
    let observed = observation(
        &universe,
        &reachability,
        &json!([{"bundleId": "app", "styleId": LIVE_STYLE}]),
    );
    let bytes = build_usage_analysis(
        &styles,
        Some(&reachability),
        Some(&observed),
        AssetRuleSelection::AllCompiled,
    )
    .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        style_value(&value, "app", DEAD_STYLE)["observationState"],
        "unobserved"
    );
    assert_eq!(
        style_value(&value, "app", LIVE_STYLE)["usageState"],
        "observed"
    );

    let contradictory = observation(
        &universe,
        &reachability,
        &json!([{"bundleId": "app", "styleId": DEAD_STYLE}]),
    );
    let error = build_usage_analysis(
        &styles,
        Some(&reachability),
        Some(&contradictory),
        AssetRuleSelection::AllCompiled,
    )
    .unwrap_err();
    assert!(error.contains("contradicts unreachable style"));
}

#[test]
fn observation_builder_is_canonical_and_round_trips() {
    let reachability_sha256 = sha256_hex(&reachability());
    let build = |reverse: bool| {
        build_usage_observation(UsageObservationInput::new(
            "a".repeat(64),
            &reachability_sha256,
            UsageObservationCoverage::Sampled,
            "fixture",
            "1",
            UsageObservationScopeInput::new(
                if reverse {
                    vec!["/settings".into(), "/".into()]
                } else {
                    vec!["/".into(), "/settings".into()]
                },
                vec![],
                vec!["light".into()],
                vec!["chromium".into()],
                vec!["1280x720".into()],
                vec!["default".into()],
            ),
            vec!["authenticated-user".into()],
            if reverse {
                vec![
                    UsageObservedStyleInput::new("app", DEAD_STYLE),
                    UsageObservedStyleInput::new("app", LIVE_STYLE),
                ]
            } else {
                vec![
                    UsageObservedStyleInput::new("app", LIVE_STYLE),
                    UsageObservedStyleInput::new("app", DEAD_STYLE),
                ]
            },
        ))
        .unwrap()
    };
    let canonical = build(false);
    assert_eq!(canonical, build(true));
    parse_usage_observation(&canonical).unwrap();
    assert!(canonical.ends_with(b"\n"));
}

#[test]
fn retention_builder_is_canonical_closed_nonempty_and_duplicate_safe() {
    let build = |reverse: bool| {
        let entries = if reverse {
            vec![
                grant("keep-live", "zeta", LIVE_STYLE),
                grant("keep-dead", "app", DEAD_STYLE),
            ]
        } else {
            vec![
                grant("keep-dead", "app", DEAD_STYLE),
                grant("keep-live", "zeta", LIVE_STYLE),
            ]
        };
        build_usage_retention(UsageRetentionInput::new(
            "a".repeat(64),
            "b".repeat(64),
            entries,
        ))
        .unwrap()
    };
    let canonical = build(false);
    assert_eq!(canonical, build(true));
    parse_usage_retention(&canonical).unwrap();
    assert!(canonical.ends_with(b"\n"));
    assert!(
        build_usage_retention(UsageRetentionInput::new(
            "a".repeat(64),
            "b".repeat(64),
            vec![]
        ))
        .is_err()
    );
    assert!(
        build_usage_retention(UsageRetentionInput::new(
            "a".repeat(64),
            "b".repeat(64),
            vec![UsageRetentionEntryInput::new(
                "Bad_ID", "app", LIVE_STYLE, "required"
            )]
        ))
        .is_err()
    );
    assert!(
        build_usage_retention(UsageRetentionInput::new(
            "a".repeat(64),
            "b".repeat(64),
            vec![UsageRetentionEntryInput::new(
                "missing-justification",
                "app",
                LIVE_STYLE,
                ""
            )]
        ))
        .is_err()
    );
    assert!(
        build_usage_retention(UsageRetentionInput::new(
            "a".repeat(64),
            "b".repeat(64),
            vec![
                grant("same-id", "app", LIVE_STYLE),
                grant("same-id", "app", DEAD_STYLE)
            ]
        ))
        .is_err()
    );
    assert!(
        build_usage_retention(UsageRetentionInput::new(
            "a".repeat(64),
            "b".repeat(64),
            vec![
                grant("first-id", "app", LIVE_STYLE),
                grant("second-id", "app", LIVE_STYLE)
            ]
        ))
        .is_err()
    );
    let mut unknown: Value = serde_json::from_slice(&canonical).unwrap();
    unknown["unexpected"] = json!(true);
    assert!(parse_usage_retention(&serde_json::to_vec(&unknown).unwrap()).is_err());
    assert!(parse_usage_retention(&vec![b' '; 16 * 1024 * 1024 + 1]).is_err());
}

#[test]
fn retained_selection_is_bundle_qualified_and_schema_2_is_explainable() {
    let reachability = reachability();
    let styles = [
        style_for("app", DEAD_STYLE, vec![origin("src/dead.rs", 1, 5)]),
        style_for("email", DEAD_STYLE, vec![origin("src/dead.rs", 1, 5)]),
        style_for("app", LIVE_STYLE, vec![origin("src/live.rs", 2, 6)]),
    ];
    let universe = universe(&styles, &reachability);
    let retention = retention(
        &universe,
        &reachability,
        vec![grant("external-email", "email", DEAD_STYLE)],
    );
    let prepared = prepare_usage_analysis(
        &styles,
        Some(&reachability),
        None,
        Some(&retention),
        AssetRuleSelection::ReachableOrRetainedStyleIds,
    )
    .unwrap();
    assert_eq!(
        prepared.rule_selection(),
        AssetRuleSelection::ReachableOrRetainedStyleIds
    );
    assert_eq!(prepared.selection().len(), 2);
    assert!(prepared.selection().contains("app", LIVE_STYLE));
    assert!(!prepared.selection().contains("app", DEAD_STYLE));
    assert!(prepared.selection().contains("email", DEAD_STYLE));
    assert!(prepared.selection().is_policy_retained("email", DEAD_STYLE));
    assert!(!prepared.selection().is_policy_retained("app", DEAD_STYLE));

    let bytes = prepared.build_analysis().unwrap();
    let parsed = parse_usage_analysis(&bytes).unwrap();
    assert_eq!(parsed.schema_version(), 2);
    assert_eq!(parsed.universe_sha256(), universe);
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["stateModelVersion"], 2);
    assert_eq!(value["ruleSelection"], "reachable-or-retained-style-ids");
    assert_eq!(value["retention"]["state"], "bound");
    assert_eq!(value["retention"]["entries"], 1);
    assert_eq!(value["summary"]["removalPolicyRetained"], 1);
    let retained = style_value(&value, "email", DEAD_STYLE);
    assert_eq!(retained["selected"], true);
    assert_eq!(retained["staticReachability"], "unreachable");
    assert_eq!(retained["usageState"], "dead");
    assert_eq!(retained["retention"]["state"], "retained");
    assert_eq!(retained["retention"]["entryId"], "external-email");
    assert_eq!(retained["removalDisposition"], "policy-retained");
    assert_eq!(
        style_value(&value, "app", DEAD_STYLE)["retention"]["state"],
        "not-retained"
    );
}

#[test]
fn retained_style_preserves_every_origin() {
    let reachability = reachability();
    let styles = [style(
        DEAD_STYLE,
        vec![origin("src/dead.rs", 1, 5), origin("src/shared.rs", 3, 7)],
    )];
    let universe = universe(&styles, &reachability);
    let retention = retention(
        &universe,
        &reachability,
        vec![grant("external-renderer", "app", DEAD_STYLE)],
    );
    let bytes = prepare_usage_analysis(
        &styles,
        Some(&reachability),
        None,
        Some(&retention),
        AssetRuleSelection::ReachableOrRetainedStyleIds,
    )
    .unwrap()
    .build_analysis()
    .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        style_value(&value, "app", DEAD_STYLE)["origins"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn stale_retention_universe_and_reachability_have_distinct_errors() {
    let reachability = reachability();
    let styles = [style(DEAD_STYLE, vec![origin("src/dead.rs", 1, 5)])];
    let universe = universe(&styles, &reachability);
    let valid = retention(
        &universe,
        &reachability,
        vec![grant("external-renderer", "app", DEAD_STYLE)],
    );
    let mut stale_universe: Value = serde_json::from_slice(&valid).unwrap();
    stale_universe["universeSha256"] = Value::String("0".repeat(64));
    let error = prepare_usage_analysis(
        &styles,
        Some(&reachability),
        None,
        Some(&serde_json::to_vec(&stale_universe).unwrap()),
        AssetRuleSelection::ReachableOrRetainedStyleIds,
    )
    .unwrap_err();
    assert!(error.contains("universeSha256"));

    let mut stale_reachability: Value = serde_json::from_slice(&valid).unwrap();
    stale_reachability["reachabilitySha256"] = Value::String("0".repeat(64));
    let error = prepare_usage_analysis(
        &styles,
        Some(&reachability),
        None,
        Some(&serde_json::to_vec(&stale_reachability).unwrap()),
        AssetRuleSelection::ReachableOrRetainedStyleIds,
    )
    .unwrap_err();
    assert!(error.contains("reachabilitySha256"));
}

#[test]
fn retention_rejects_unknown_reachable_mixed_and_missing_application_evidence() {
    let reachability = reachability();
    let styles = [
        style(LIVE_STYLE, vec![origin("src/live.rs", 2, 6)]),
        style(DEAD_STYLE, vec![origin("src/dead.rs", 1, 5)]),
        style(
            MIXED_STYLE,
            vec![origin("src/live.rs", 2, 6), origin("src/shared.rs", 3, 7)],
        ),
    ];
    let universe = universe(&styles, &reachability);
    for (id, style_id, expected) in [
        ("unknown-style", OTHER_STYLE, "unknown style"),
        ("reachable-style", LIVE_STYLE, "structurally unreachable"),
        ("mixed-style", MIXED_STYLE, "structurally unreachable"),
    ] {
        let sidecar = retention(&universe, &reachability, vec![grant(id, "app", style_id)]);
        let error = prepare_usage_analysis(
            &styles,
            Some(&reachability),
            None,
            Some(&sidecar),
            AssetRuleSelection::ReachableOrRetainedStyleIds,
        )
        .unwrap_err();
        assert!(error.contains(expected), "{error}");
    }

    let valid = retention(
        &universe,
        &reachability,
        vec![grant("dead-style", "app", DEAD_STYLE)],
    );
    let error = prepare_usage_analysis(
        &styles,
        None,
        None,
        Some(&valid),
        AssetRuleSelection::ReachableOrRetainedStyleIds,
    )
    .unwrap_err();
    assert!(error.contains("requires exact reachability"));

    let missing = [style(OTHER_STYLE, vec![origin("src/missing.rs", 10, 20)])];
    let missing_universe =
        prepare_usage_analysis(&missing, None, None, None, AssetRuleSelection::AllCompiled)
            .unwrap()
            .universe_sha256()
            .to_owned();
    let missing_retention = retention(
        &missing_universe,
        &reachability,
        vec![grant("missing-origin", "app", OTHER_STYLE)],
    );
    let error = prepare_usage_analysis(
        &missing,
        Some(&reachability),
        None,
        Some(&missing_retention),
        AssetRuleSelection::ReachableOrRetainedStyleIds,
    )
    .unwrap_err();
    assert!(error.contains("usage origin has no component"));
}

#[test]
fn observation_cannot_resurrect_a_policy_retained_dead_style() {
    let reachability = reachability();
    let styles = [style(DEAD_STYLE, vec![origin("src/dead.rs", 1, 5)])];
    let universe = universe(&styles, &reachability);
    let retention = retention(
        &universe,
        &reachability,
        vec![grant("external-renderer", "app", DEAD_STYLE)],
    );
    let observation = observation(
        &universe,
        &reachability,
        &json!([{"bundleId": "app", "styleId": DEAD_STYLE}]),
    );
    let error = prepare_usage_analysis(
        &styles,
        Some(&reachability),
        Some(&observation),
        Some(&retention),
        AssetRuleSelection::ReachableOrRetainedStyleIds,
    )
    .unwrap_err();
    assert!(error.contains("contradicts unreachable style"));
}

#[test]
fn schema_2_parser_rejects_selection_retention_and_disposition_tampering() {
    let reachability = reachability();
    let styles = [style(DEAD_STYLE, vec![origin("src/dead.rs", 1, 5)])];
    let universe = universe(&styles, &reachability);
    let retention = retention(
        &universe,
        &reachability,
        vec![grant("external-renderer", "app", DEAD_STYLE)],
    );
    let bytes = prepare_usage_analysis(
        &styles,
        Some(&reachability),
        None,
        Some(&retention),
        AssetRuleSelection::ReachableOrRetainedStyleIds,
    )
    .unwrap()
    .build_analysis()
    .unwrap();
    for (field, replacement) in [
        ("selected", json!(false)),
        ("retention", json!({"state": "not-retained"})),
        ("removalDisposition", json!("removed")),
    ] {
        let mut tampered: Value = serde_json::from_slice(&bytes).unwrap();
        tampered["styles"][0][field] = replacement;
        assert!(parse_usage_analysis(&serde_json::to_vec(&tampered).unwrap()).is_err());
    }
    let mut wrong_version: Value = serde_json::from_slice(&bytes).unwrap();
    wrong_version["schemaVersion"] = json!(1);
    wrong_version["stateModelVersion"] = json!(1);
    assert!(parse_usage_analysis(&serde_json::to_vec(&wrong_version).unwrap()).is_err());
}

#[test]
fn evidence_verifier_rederives_exact_observation_and_retention_content() {
    let reachability = reachability();
    let styles = [
        style(LIVE_STYLE, vec![origin("src/live.rs", 2, 6)]),
        style(DEAD_STYLE, vec![origin("src/dead.rs", 1, 5)]),
    ];
    let universe = universe(&styles, &reachability);
    let observation = observation(
        &universe,
        &reachability,
        &json!([{"bundleId": "app", "styleId": LIVE_STYLE}]),
    );
    let retention = retention(
        &universe,
        &reachability,
        vec![grant("external-renderer", "app", DEAD_STYLE)],
    );
    let bytes = prepare_usage_analysis(
        &styles,
        Some(&reachability),
        Some(&observation),
        Some(&retention),
        AssetRuleSelection::ReachableOrRetainedStyleIds,
    )
    .unwrap()
    .build_analysis()
    .unwrap();

    verify_usage_analysis(
        &bytes,
        &styles,
        Some(&reachability),
        Some(&observation),
        Some(&retention),
    )
    .unwrap();

    let mut tampered_retention: Value = serde_json::from_slice(&bytes).unwrap();
    let retained = tampered_retention["styles"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|style| style["styleId"] == DEAD_STYLE)
        .unwrap();
    retained["retention"]["entryId"] = json!("different-policy-entry");
    let tampered_retention = serde_json::to_vec(&tampered_retention).unwrap();
    assert!(parse_usage_analysis(&tampered_retention).is_ok());
    assert!(
        verify_usage_analysis(
            &tampered_retention,
            &styles,
            Some(&reachability),
            Some(&observation),
            Some(&retention),
        )
        .is_err()
    );

    let mut tampered_observation: Value = serde_json::from_slice(&bytes).unwrap();
    tampered_observation["observation"]["producer"]["name"] = json!("different-producer");
    let tampered_observation = serde_json::to_vec(&tampered_observation).unwrap();
    assert!(parse_usage_analysis(&tampered_observation).is_ok());
    assert!(
        verify_usage_analysis(
            &tampered_observation,
            &styles,
            Some(&reachability),
            Some(&observation),
            Some(&retention),
        )
        .is_err()
    );
}

#[test]
fn analysis_parser_caps_aggregate_evidence_and_origin_labels() {
    let reachability = reachability();
    let styles = [style(LIVE_STYLE, vec![origin("src/live.rs", 2, 6)])];
    let universe = universe(&styles, &reachability);
    let observation = observation(&universe, &reachability, &json!([]));
    let bytes = build_usage_analysis(
        &styles,
        Some(&reachability),
        Some(&observation),
        AssetRuleSelection::AllCompiled,
    )
    .unwrap();

    let values = |prefix: &str, count: usize| {
        (0..count)
            .map(|index| Value::String(format!("{prefix}-{index:05}")))
            .collect::<Vec<_>>()
    };

    let mut excessive_observation: Value = serde_json::from_slice(&bytes).unwrap();
    excessive_observation["observation"]["contexts"]["routes"] =
        Value::Array(values("route", 32_768));
    excessive_observation["observation"]["contexts"]["islands"] =
        Value::Array(values("island", 32_768));
    assert!(parse_usage_analysis(&serde_json::to_vec(&excessive_observation).unwrap()).is_err());

    let mut excessive_origin: Value = serde_json::from_slice(&bytes).unwrap();
    excessive_origin["styles"][0]["origins"][0]["componentIds"] =
        Value::Array(values("component", 65_535));
    excessive_origin["styles"][0]["origins"][0]["routeIds"] =
        Value::Array(vec![json!("route-overflow")]);
    assert!(parse_usage_analysis(&serde_json::to_vec(&excessive_origin).unwrap()).is_err());
}

#[test]
fn class_name_must_be_the_style_id_encoding() {
    let invalid = [UsageStyleInput::new(
        "app",
        LIVE_STYLE,
        format!("pc_{LIVE_STYLE}"),
        vec![origin("src/live.rs", 2, 6)],
    )];
    let error = prepare_usage_analysis(&invalid, None, None, None, AssetRuleSelection::AllCompiled)
        .unwrap_err();
    assert!(error.contains("invalid usage analysis"));
}

#[test]
fn collector_rejects_duplicate_origins_instead_of_deduplicating() {
    let mut collector = UsageStyleCollector::new("app").unwrap();
    let exact = origin("src/live.rs", 2, 6);
    collector
        .push(LIVE_STYLE, class_name(LIVE_STYLE), exact.clone())
        .unwrap();
    let error = collector
        .push(LIVE_STYLE, class_name(LIVE_STYLE), exact)
        .unwrap_err();
    assert!(error.contains("duplicate usage origin"));
}

#[test]
fn utility_origin_source_accepts_layout_whitespace_and_rejects_dangerous_controls() {
    let source = "flex\tgap-4\nmd:grid\r\nhover:bg-brand";
    let styles = [style(
        LIVE_STYLE,
        vec![UsageOriginInput::new(
            source,
            "src/live.rs",
            2,
            6,
            "pc",
            "rust-macro",
        )],
    )];
    let bytes = build_usage_analysis(&styles, None, None, AssetRuleSelection::AllCompiled).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["styles"][0]["origins"][0]["source"], source);
    parse_usage_analysis(&bytes).unwrap();

    for dangerous in ['\0', '\u{000b}', '\u{000c}', '\u{001b}', '\u{007f}'] {
        let invalid = [style(
            LIVE_STYLE,
            vec![UsageOriginInput::new(
                format!("flex{dangerous}grid"),
                "src/live.rs",
                2,
                6,
                "pc",
                "rust-macro",
            )],
        )];
        assert!(
            build_usage_analysis(&invalid, None, None, AssetRuleSelection::AllCompiled).is_err(),
            "control U+{:04X} must be rejected",
            u32::from(dangerous)
        );
    }
}
