//! Standards-first CSS audit contract tests.

use pliego_css_build::artifacts::{
    CompatibilityProfile, StandardCssFormat, audit_standard_css, audit_standard_css_with_budgets,
    sha256_hex, transform_standard_css,
};
use pliego_css_config::{BudgetSubject, BudgetSubjectKind, parse_budget_policy};

#[test]
fn valid_standard_css_emits_deterministic_inventory_evidence() {
    let css = "@layer components {\n  .card, #app:hover { color: red !important; margin: 0; }\n  @media (width > 40rem) { .card__title { display: block; } }\n}\n";
    let first = audit_standard_css("src/app.css", css, CompatibilityProfile::BaselineWidely)
        .expect("audit");
    let second = audit_standard_css("src/app.css", css, CompatibilityProfile::BaselineWidely)
        .expect("repeat audit");
    assert!(first.passed());
    assert_eq!(first, second);

    let json = first.document().to_json_pretty().expect("JSON");
    let value: serde_json::Value = serde_json::from_str(&json).expect("document");
    assert_eq!(value["command"], "audit");
    let findings = value["findings"].as_array().expect("findings");
    let inventory = findings
        .iter()
        .find(|finding| finding["code"] == "PCSS-AUDIT-000")
        .expect("inventory");
    assert_eq!(inventory["verification"], "verified");
    assert_eq!(inventory["context"]["target-profile"], "baseline-widely");
    let evidence = inventory["evidence"].as_array().expect("evidence");
    let metric = |name: &str| {
        evidence
            .iter()
            .find(|item| item["name"] == name)
            .and_then(|item| item["value"].as_str())
            .expect("metric")
    };
    assert_eq!(metric("rules"), "4");
    assert_eq!(metric("style-rules"), "2");
    assert_eq!(metric("selectors"), "3");
    assert_eq!(metric("style-declarations"), "3");
    assert_eq!(metric("important-declarations"), "1");
    assert_eq!(metric("generic-css-declaration-identities"), "3");
    assert_eq!(
        metric("generic-css-declaration-identity-evidence-truncated"),
        "false"
    );
    let identities = evidence
        .iter()
        .filter(|item| item["kind"] == "identity")
        .collect::<Vec<_>>();
    assert_eq!(identities.len(), 3);
    assert!(identities.iter().all(|item| {
        item["source"] == "pliegocss-generic-css-declaration-v1"
            && item["value"].as_str().unwrap().starts_with("sha256:")
    }));
    assert_eq!(metric("maximum-specificity"), "1,1,0");
    assert_eq!(
        sha256_hex(json.as_bytes()),
        "f7a396013d107a2988196dc1f50535f47c41e2d2937da9c0a3dc1fac2cccd752"
    );
}

#[test]
fn standard_css_transform_is_deterministic_targeted_and_read_only() {
    let css = ".card {\n  color: color(display-p3 1 0 0);\n  user-select: none;\n}\n";
    let first = transform_standard_css(
        "src/app.css",
        css,
        CompatibilityProfile::Modern,
        StandardCssFormat::Minified,
    )
    .expect("transform");
    let second = transform_standard_css(
        "src/app.css",
        css,
        CompatibilityProfile::Modern,
        StandardCssFormat::Minified,
    )
    .expect("repeat transform");
    assert_eq!(first, second);
    assert_eq!(first.source_sha256(), sha256_hex(css.as_bytes()));
    assert_eq!(first.output_sha256(), sha256_hex(first.css().as_bytes()));
    assert!(first.css().ends_with('\n'));
    assert!(first.css().contains(".card{"));
    assert_ne!(first.css(), css);
}

#[test]
fn standard_css_transform_fails_closed_for_path_size_and_syntax() {
    for path in ["", "/app.css", "../app.css", "src\\app.css", "C:/app.css"] {
        let error = transform_standard_css(
            path,
            ".ok {}",
            CompatibilityProfile::Modern,
            StandardCssFormat::Pretty,
        )
        .expect_err("unsafe path");
        assert!(error.contains("portable relative"), "{path}: {error}");
    }
    let oversized = " ".repeat(16 * 1024 * 1024 + 1);
    let error = transform_standard_css(
        "src/app.css",
        &oversized,
        CompatibilityProfile::Modern,
        StandardCssFormat::Pretty,
    )
    .expect_err("oversized");
    assert!(error.contains("16 MiB"));

    let error = transform_standard_css(
        "src/app.css",
        ".bad > { color: red; }",
        CompatibilityProfile::Modern,
        StandardCssFormat::Pretty,
    )
    .expect_err("invalid CSS");
    assert!(!error.is_empty());
}

#[test]
fn invalid_css_is_a_verified_exact_source_finding() {
    let css = ".café { color: red; }\n.bad > { color: blue; }\n";
    let outcome = audit_standard_css("src/unicode.css", css, CompatibilityProfile::BaselineWidely)
        .expect("audit finding");
    assert!(!outcome.passed());
    let json = outcome.document().to_json_pretty().expect("JSON");
    let value: serde_json::Value = serde_json::from_str(&json).expect("document");
    let finding = value["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|finding| finding["code"] == "PCSS-SYNTAX-001")
        .expect("syntax finding");
    assert_eq!(finding["code"], "PCSS-SYNTAX-001");
    assert_eq!(finding["severity"], "error");
    assert_eq!(finding["verification"], "verified");
    assert_eq!(finding["source"]["file"], "src/unicode.css");
    assert!(finding["source"]["byteStart"].as_u64().is_some());
    assert!(finding["source"]["startLine"].as_u64().is_some());
}

#[test]
fn audit_rejects_host_paths_and_defensive_limit_overflow() {
    let error = audit_standard_css(
        "C:\\src\\app.css",
        ".ok {}",
        CompatibilityProfile::BaselineWidely,
    )
    .expect_err("host path");
    assert!(error.contains("portable relative"));

    let oversized = " ".repeat(16 * 1024 * 1024 + 1);
    let error = audit_standard_css(
        "src/app.css",
        &oversized,
        CompatibilityProfile::BaselineWidely,
    )
    .expect_err("oversized");
    assert!(error.contains("16 MiB"));
}

#[test]
fn frozen_web_features_decisions_are_explainable_and_fail_closed() {
    let css = "@layer app { .card { color: red; } }\n@scope (.shell) { .card { color: blue; } }\n";
    let outcome = audit_standard_css(
        "src/features.css",
        css,
        CompatibilityProfile::BaselineWidely,
    )
    .expect("audit");
    assert!(!outcome.passed());
    let value: serde_json::Value =
        serde_json::from_str(&outcome.document().to_json_pretty().expect("JSON"))
            .expect("document");
    let findings = value["findings"].as_array().expect("findings");
    let layer = findings
        .iter()
        .find(|finding| finding["context"]["feature-id"] == "cascade-layers")
        .expect("layer decision");
    assert_eq!(layer["code"], "PCSS-COMPAT-100");
    assert_eq!(layer["source"]["byteStart"], 0);
    assert_eq!(layer["source"]["byteEnd"], 6);
    assert_eq!(layer["context"]["web-features-version"], "3.32.0");

    let scope = findings
        .iter()
        .find(|finding| finding["context"]["feature-id"] == "scope")
        .expect("scope decision");
    assert_eq!(scope["code"], "PCSS-COMPAT-101");
    assert_eq!(scope["severity"], "error");
    assert_eq!(scope["verification"], "verified");
    assert_eq!(scope["suggestions"].as_array().map(Vec::len), Some(2));
}

#[test]
fn unknown_at_rules_never_become_implicit_compatibility_proof() {
    let css = "@vendor-magic enabled;\n.ok { color: green; }\n";
    let strict = audit_standard_css("src/unknown.css", css, CompatibilityProfile::BaselineWidely)
        .expect("strict audit");
    assert!(!strict.passed());
    let json = strict.document().to_json_pretty().expect("JSON");
    let value: serde_json::Value = serde_json::from_str(&json).expect("document");
    let unknown = value["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|finding| finding["code"] == "PCSS-COMPAT-199")
        .expect("unknown finding");
    assert_eq!(unknown["verification"], "unverified");
    assert_eq!(unknown["severity"], "error");
    assert_eq!(unknown["source"]["byteStart"], 0);
    assert_eq!(unknown["source"]["byteEnd"], 13);

    let unmanaged = audit_standard_css("src/unknown.css", css, CompatibilityProfile::None)
        .expect("unmanaged audit");
    assert!(unmanaged.passed());
}

#[test]
fn supported_rule_classifier_corpus_covers_every_frozen_feature_family() {
    let cases = [
        ("cascade-layers", "@layer app { .x { color: red; } }"),
        (
            "container-queries",
            "@container (width > 20rem) { .x { color: red; } }",
        ),
        (
            "container-style-queries",
            "@container style(--theme: dark) { .x { color: red; } }",
        ),
        (
            "container-scroll-state-queries",
            "@container scroll-state(stuck: top) { .x { color: red; } }",
        ),
        ("nesting", ".x { & .y { color: red; } }"),
        (
            "registered-custom-properties",
            "@property --tone { syntax: \"<color>\"; inherits: false; initial-value: red; }",
        ),
        ("scope", "@scope (.shell) { .x { color: red; } }"),
        ("starting-style", "@starting-style { .x { opacity: 0; } }"),
    ];
    for (feature, css) in cases {
        let outcome = audit_standard_css("src/corpus.css", css, CompatibilityProfile::None)
            .unwrap_or_else(|error| panic!("{feature}: {error}"));
        let value: serde_json::Value =
            serde_json::from_str(&outcome.document().to_json_pretty().expect("finding JSON"))
                .expect("document");
        assert!(
            value["findings"].as_array().unwrap().iter().any(|finding| {
                finding["context"]["feature-id"] == feature
                    && finding["context"]["classifier-version"] == "css-rule-features-1"
            }),
            "missing classifier evidence for {feature}: {value}"
        );
    }
}

#[test]
fn unknown_container_conditions_fail_as_unclassified_syntax() {
    let css = "@container (mystery) { .card { color: red; } }\n";
    let outcome = audit_standard_css(
        "src/container.css",
        css,
        CompatibilityProfile::BaselineWidely,
    )
    .expect("audit");
    assert!(!outcome.passed());
    let value: serde_json::Value =
        serde_json::from_str(&outcome.document().to_json_pretty().expect("finding JSON"))
            .expect("document");
    let finding = value["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|finding| finding["code"] == "PCSS-COMPAT-199")
        .expect("unclassified condition");
    assert_eq!(finding["context"]["syntax"], "@container condition");
    assert_eq!(finding["source"]["byteStart"], 0);
    assert_eq!(finding["source"]["byteEnd"], 10);
}

#[test]
fn declaration_shape_inventory_distinguishes_typed_unparsed_and_custom_values() {
    let css = ".card { display: grid; color: var(--ink); --gap: 1rem; mystery-prop: value; }";
    let outcome =
        audit_standard_css("src/declarations.css", css, CompatibilityProfile::None).expect("audit");
    let value: serde_json::Value =
        serde_json::from_str(&outcome.document().to_json_pretty().expect("finding JSON"))
            .expect("document");
    let inventory = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"] == "PCSS-AUDIT-000")
        .unwrap();
    let metric = |name: &str| {
        inventory["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .find(|evidence| evidence["name"] == name)
            .and_then(|evidence| evidence["value"].as_str())
            .unwrap()
    };
    assert_eq!(metric("typed-declarations"), "1");
    assert_eq!(metric("unparsed-declarations"), "1");
    assert_eq!(metric("custom-declarations"), "2");
}

#[test]
fn user_select_declaration_is_a_frozen_compatibility_decision() {
    let css = ".card { user-select: none; }";
    let modern = audit_standard_css("src/user-select.css", css, CompatibilityProfile::Modern)
        .expect("modern audit");
    assert!(
        !modern.passed(),
        "Safari support is absent from the frozen feature data"
    );
    let value: serde_json::Value =
        serde_json::from_str(&modern.document().to_json_pretty().expect("finding JSON"))
            .expect("document");
    let finding = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["context"]["feature-id"] == "user-select")
        .expect("user-select decision");
    assert_eq!(finding["code"], "PCSS-COMPAT-101");
    assert_eq!(
        finding["context"]["compat-key"],
        "css.properties.user-select"
    );
    assert_eq!(
        finding["context"]["classifier-version"],
        "css-rule-features-1"
    );
}

#[test]
fn aspect_ratio_declaration_is_a_frozen_widely_available_decision() {
    let css = ".media { aspect-ratio: 16 / 9; }";
    let outcome = audit_standard_css(
        "src/aspect-ratio.css",
        css,
        CompatibilityProfile::BaselineWidely,
    )
    .expect("audit");
    assert!(outcome.passed());
    let value: serde_json::Value =
        serde_json::from_str(&outcome.document().to_json_pretty().expect("finding JSON"))
            .expect("document");
    let finding = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["context"]["feature-id"] == "aspect-ratio")
        .expect("aspect-ratio decision");
    assert_eq!(finding["code"], "PCSS-COMPAT-100");
    assert_eq!(
        finding["context"]["compat-key"],
        "css.properties.aspect-ratio"
    );
    assert_eq!(
        finding["context"]["classifier-version"],
        "css-rule-features-1"
    );
    let baseline = finding["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|evidence| evidence["name"] == "baseline-status")
        .and_then(|evidence| evidence["value"].as_str())
        .unwrap();
    assert_eq!(baseline, "high");
}

#[test]
fn modern_color_values_are_frozen_widely_available_decisions() {
    let css = ".wide { color: color(display-p3 1 0 0); } .tone { color: oklch(60% 0.2 250); }";
    let outcome = audit_standard_css(
        "src/color-values.css",
        css,
        CompatibilityProfile::BaselineWidely,
    )
    .expect("audit");
    assert!(outcome.passed());
    let value: serde_json::Value =
        serde_json::from_str(&outcome.document().to_json_pretty().expect("finding JSON"))
            .expect("document");
    let findings = value["findings"].as_array().unwrap();
    for (id, key) in [
        ("color-function", "css.types.color.color"),
        ("oklab", "css.types.color.oklch"),
    ] {
        let finding = findings
            .iter()
            .find(|finding| finding["context"]["feature-id"] == id)
            .unwrap();
        assert_eq!(finding["code"], "PCSS-COMPAT-100");
        assert_eq!(finding["context"]["compat-key"], key);
    }
}

#[test]
fn color_mix_is_a_frozen_widely_available_decision() {
    let css = ".mix { color: color-mix(in srgb, currentColor, blue); }";
    let outcome = audit_standard_css(
        "src/color-mix.css",
        css,
        CompatibilityProfile::BaselineWidely,
    )
    .expect("audit");
    assert!(outcome.passed());
    let value: serde_json::Value =
        serde_json::from_str(&outcome.document().to_json_pretty().expect("finding JSON"))
            .expect("document");
    let finding = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["context"]["feature-id"] == "color-mix")
        .expect("color-mix decision");
    assert_eq!(finding["code"], "PCSS-COMPAT-100");
    assert_eq!(
        finding["context"]["compat-key"],
        "css.types.color.color-mix"
    );
}

#[test]
fn variadic_color_mix_fails_closed_instead_of_inheriting_two_color_support() {
    let css = ".mix { color: color-mix(in srgb, red, blue, green); }";
    let outcome = audit_standard_css(
        "src/color-mix-variadic.css",
        css,
        CompatibilityProfile::BaselineWidely,
    )
    .expect("audit");
    assert!(!outcome.passed());
    let value: serde_json::Value =
        serde_json::from_str(&outcome.document().to_json_pretty().expect("finding JSON"))
            .expect("document");
    let finding = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["context"]["feature-id"] == "color-mix-variadic")
        .expect("variadic decision");
    assert_eq!(finding["code"], "PCSS-COMPAT-101");
}

#[test]
fn selector_shape_inventory_counts_application_selector_families() {
    let css = "main, .card:hover, #app > [data-state=open]::before { color: red; }";
    let outcome =
        audit_standard_css("src/selectors.css", css, CompatibilityProfile::None).expect("audit");
    let value: serde_json::Value =
        serde_json::from_str(&outcome.document().to_json_pretty().expect("finding JSON"))
            .expect("document");
    let inventory = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"] == "PCSS-AUDIT-000")
        .unwrap();
    let metric = |name: &str| {
        inventory["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .find(|evidence| evidence["name"] == name)
            .and_then(|evidence| evidence["value"].as_str())
            .unwrap()
    };
    assert_eq!(metric("selector-components"), "8");
    assert_eq!(metric("selector-combinators"), "2");
    assert_eq!(metric("selector-attributes"), "1");
    assert_eq!(metric("selector-pseudo-classes"), "1");
    assert_eq!(metric("selector-pseudo-elements"), "1");
}

#[test]
fn selector_features_are_frozen_compatibility_decisions() {
    let css = ".button:focus-visible, .card:has(> img), :is(main, article) .title, :where(section, aside) .note, button:not(.primary, .secondary) { outline: 2px solid; }";
    let baseline = audit_standard_css(
        "src/selector-features.css",
        css,
        CompatibilityProfile::BaselineWidely,
    )
    .expect("baseline audit");
    assert!(baseline.passed());
    let value: serde_json::Value =
        serde_json::from_str(&baseline.document().to_json_pretty().expect("finding JSON"))
            .expect("document");
    let findings = value["findings"].as_array().unwrap();
    for feature in ["focus-visible", "has", "is", "where", "not"] {
        let finding = findings
            .iter()
            .find(|finding| finding["context"]["feature-id"] == feature)
            .unwrap_or_else(|| panic!("missing {feature}"));
        assert_eq!(finding["code"], "PCSS-COMPAT-100");
        assert_eq!(
            finding["context"]["classifier-version"],
            "css-rule-features-1"
        );
    }
}

#[test]
fn budgets_attribute_canonical_metrics_deltas_layers_and_exceptions() {
    let css = "@layer components {\n  .a { color: red; }\n  .b { color: red; }\n}\n#app:hover { display: block; }\n";
    let policy = parse_budget_policy(
        br#"{
          "schemaVersion": 1,
          "policyVersion": 1,
          "budgets": [
            {
              "id": "file-duplication",
              "subject": {"kind":"file","id":"src/budget.css"},
              "limits": {"semanticDuplicates":{"maximum":0,"baseline":0,"maxIncrease":0}}
            },
            {
              "id": "package-selectors",
              "subject": {"kind":"package","id":"app"},
              "limits": {"selectors":{"maximum":8,"baseline":3,"maxIncrease":1}}
            },
            {
              "id": "route-specificity",
              "subject": {"kind":"route","id":"/home"},
              "limits": {"specificity":{"maximum":"0,2,0","baseline":"0,1,0","maxIncrease":"0,1,0"}}
            },
            {
              "id": "components-layer",
              "subject": {"kind":"layer","id":"components"},
              "limits": {"rules":{"maximum":1,"baseline":1,"maxIncrease":0}}
            }
          ],
          "exceptions": [{
            "id":"reviewed-components-growth",
            "budget":"components-layer",
            "metrics":["rules"],
            "justification":"component split reviewed for this release"
          }]
        }"#,
    )
    .expect("budget policy");
    let subjects = [
        BudgetSubject::new(BudgetSubjectKind::Package, "app").expect("package"),
        BudgetSubject::new(BudgetSubjectKind::Route, "/home").expect("route"),
    ];
    let first = audit_standard_css_with_budgets(
        "src/budget.css",
        css,
        CompatibilityProfile::BaselineWidely,
        Some(&policy),
        &subjects,
    )
    .expect("budget audit");
    let second = audit_standard_css_with_budgets(
        "src/budget.css",
        css,
        CompatibilityProfile::BaselineWidely,
        Some(&policy),
        &subjects,
    )
    .expect("repeat budget audit");
    assert_eq!(first, second);
    assert!(!first.passed());

    let value: serde_json::Value =
        serde_json::from_str(&first.document().to_json_pretty().expect("JSON")).expect("document");
    let findings = value["findings"].as_array().expect("findings");
    let inventory = findings
        .iter()
        .find(|finding| finding["code"] == "PCSS-AUDIT-000")
        .expect("inventory");
    assert!(
        inventory["evidence"]
            .as_array()
            .expect("inventory evidence")
            .iter()
            .any(|evidence| {
                evidence["kind"] == "fingerprint"
                    && evidence["name"] == "semantic-duplicate-0001"
                    && evidence["value"]
                        .as_str()
                        .is_some_and(|value| value.contains(";occurrences=2"))
            })
    );
    let duplicate = budget_finding(findings, "file-duplication", "semantic-duplicates");
    assert_eq!(duplicate["code"], "PCSS-BUDGET-101");
    assert_eq!(budget_evidence(duplicate, "actual"), "1");

    let package = budget_finding(findings, "package-selectors", "selectors");
    assert_eq!(package["code"], "PCSS-BUDGET-100");
    assert_eq!(budget_evidence(package, "delta"), "+0");

    let route = budget_finding(findings, "route-specificity", "specificity");
    assert_eq!(route["code"], "PCSS-BUDGET-101");
    assert_eq!(budget_evidence(route, "actual"), "1,1,0");
    assert_eq!(budget_evidence_item(route, "actual")["unit"], "specificity");
    assert!(budget_evidence_item(route, "maximum-exceeded")["unit"].is_null());

    let layer = budget_finding(findings, "components-layer", "rules");
    assert_eq!(layer["code"], "PCSS-BUDGET-102");
    assert_eq!(layer["source"]["byteStart"], 0);
    assert_eq!(layer["source"]["byteEnd"], 6);
    assert_eq!(layer["exception"]["id"], "reviewed-components-growth");
}

#[test]
fn explicit_budget_policy_fails_when_no_subject_matches() {
    let policy = parse_budget_policy(
        br#"{
          "schemaVersion":1,
          "policyVersion":1,
          "budgets":[{
            "id":"other-file",
            "subject":{"kind":"file","id":"src/other.css"},
            "limits":{"bytes":{"maximum":100,"baseline":null,"maxIncrease":null}}
          }]
        }"#,
    )
    .expect("policy");
    let outcome = audit_standard_css_with_budgets(
        "src/app.css",
        ".app { color: red; }",
        CompatibilityProfile::BaselineWidely,
        Some(&policy),
        &[],
    )
    .expect("audit");
    assert!(!outcome.passed());
    let value: serde_json::Value =
        serde_json::from_str(&outcome.document().to_json_pretty().expect("JSON"))
            .expect("document");
    assert!(
        value["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "PCSS-BUDGET-199")
    );
}

#[test]
fn explicit_budget_policy_fails_when_any_subject_is_unobserved() {
    let policy = parse_budget_policy(
        br#"{
          "schemaVersion":1,
          "policyVersion":1,
          "budgets":[
            {
              "id":"app-file",
              "subject":{"kind":"file","id":"src/app.css"},
              "limits":{"bytes":{"maximum":100,"baseline":null,"maxIncrease":null}}
            },
            {
              "id":"missing-package",
              "subject":{"kind":"package","id":"missing"},
              "limits":{"bytes":{"maximum":100,"baseline":null,"maxIncrease":null}}
            }
          ]
        }"#,
    )
    .expect("policy");
    let outcome = audit_standard_css_with_budgets(
        "src/app.css",
        ".app { color: red; }",
        CompatibilityProfile::BaselineWidely,
        Some(&policy),
        &[],
    )
    .expect("audit");
    assert!(!outcome.passed());
    let value: serde_json::Value =
        serde_json::from_str(&outcome.document().to_json_pretty().expect("JSON"))
            .expect("document");
    let unmatched = value["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|finding| finding["code"] == "PCSS-BUDGET-199")
        .expect("partial policy coverage finding");
    assert_eq!(budget_evidence(unmatched, "declared-budgets"), "2");
    assert_eq!(budget_evidence(unmatched, "matched-budgets"), "1");
}

#[test]
fn semantic_duplication_fingerprints_include_conditional_and_layer_context() {
    let css = ".root { color: red; }\n@media (width > 40rem) { .wide { color: red; } }\n@layer alternate { .layered { color: red; } }\n";
    let policy = parse_budget_policy(
        br#"{
          "schemaVersion":1,
          "policyVersion":1,
          "budgets":[{
            "id":"context-aware-duplicates",
            "subject":{"kind":"file","id":"src/context.css"},
            "limits":{"semanticDuplicates":{"maximum":0,"baseline":0,"maxIncrease":0}}
          }]
        }"#,
    )
    .expect("policy");
    let outcome = audit_standard_css_with_budgets(
        "src/context.css",
        css,
        CompatibilityProfile::BaselineWidely,
        Some(&policy),
        &[],
    )
    .expect("audit");
    assert!(outcome.passed());
    let value: serde_json::Value =
        serde_json::from_str(&outcome.document().to_json_pretty().expect("JSON"))
            .expect("document");
    let finding = budget_finding(
        value["findings"].as_array().expect("findings"),
        "context-aware-duplicates",
        "semantic-duplicates",
    );
    assert_eq!(finding["code"], "PCSS-BUDGET-100");
    assert_eq!(budget_evidence(finding, "actual"), "0");
}

fn budget_finding<'a>(
    findings: &'a [serde_json::Value],
    budget: &str,
    metric: &str,
) -> &'a serde_json::Value {
    findings
        .iter()
        .find(|finding| {
            finding["context"]["budget-id"] == budget && finding["context"]["metric"] == metric
        })
        .expect("budget finding")
}

fn budget_evidence<'a>(finding: &'a serde_json::Value, name: &str) -> &'a str {
    budget_evidence_item(finding, name)["value"]
        .as_str()
        .expect("budget evidence value")
}

fn budget_evidence_item<'a>(finding: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    finding["evidence"]
        .as_array()
        .expect("evidence")
        .iter()
        .find(|evidence| evidence["name"] == name)
        .expect("budget evidence")
}
