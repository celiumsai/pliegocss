//! Standards-first CSS audit contract tests.

use pliego_css_build::artifacts::{
    CompatibilityProfile, audit_standard_css, audit_standard_css_with_budgets, sha256_hex,
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
    assert_eq!(metric("maximum-specificity"), "1,1,0");
    assert_eq!(
        sha256_hex(json.as_bytes()),
        "dcd1fb52b9ee314eb3751f6aec2307283083089cc15427bfc83349e9f9b5e15a"
    );
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
