#![allow(missing_docs)]

use pliego_css_config::{
    BudgetMeasurements, BudgetMetric, BudgetObservation, BudgetSubject, BudgetSubjectKind,
    CssSpecificity, evaluate_budget_policy, parse_budget_policy,
};

const POLICY: &str = r#"{
  "schemaVersion": 1,
  "policyVersion": 1,
  "budgets": [
    {
      "id": "app-file",
      "subject": { "kind": "file", "id": "src/app.css" },
      "limits": {
        "bytes": { "maximum": 100, "baseline": 80, "maxIncrease": 10 },
        "rules": { "maximum": 10, "baseline": null, "maxIncrease": null },
        "selectors": null,
        "specificity": { "maximum": "0,2,0", "baseline": "0,1,0", "maxIncrease": "0,1,0" },
        "semanticDuplicates": { "maximum": 0, "baseline": 0, "maxIncrease": 0 }
      }
    }
  ],
  "exceptions": [
    {
      "id": "reviewed-rule-growth",
      "budget": "app-file",
      "metrics": ["rules"],
      "justification": "temporary reviewed route split"
    }
  ]
}"#;

#[test]
fn evaluates_absolute_delta_specificity_and_reviewed_exceptions() {
    let policy = parse_budget_policy(POLICY.as_bytes()).expect("valid policy");
    let subject = BudgetSubject::new(BudgetSubjectKind::File, "src/app.css").expect("subject");
    let report = evaluate_budget_policy(
        &policy,
        &[BudgetObservation::new(
            subject,
            BudgetMeasurements::new(
                95,
                11,
                4,
                CssSpecificity::new(0, 3, 0).expect("specificity"),
                2,
            ),
        )],
    )
    .expect("evaluation");

    assert_eq!(report.matched_budgets(), 1);
    assert_eq!(report.evaluations().len(), 4);
    assert!(report.failed());

    let bytes = metric(&report, BudgetMetric::Bytes);
    assert!(!bytes.maximum_exceeded);
    assert!(bytes.delta_exceeded);
    assert_eq!(bytes.delta.as_deref(), Some("+15"));

    let rules = metric(&report, BudgetMetric::Rules);
    assert!(rules.maximum_exceeded);
    assert!(!rules.enforced());
    assert_eq!(
        rules
            .exception
            .as_ref()
            .map(|exception| exception.id.as_str()),
        Some("reviewed-rule-growth")
    );

    let specificity = metric(&report, BudgetMetric::Specificity);
    assert_eq!(specificity.actual, "0,3,0");
    assert_eq!(specificity.delta.as_deref(), Some("+0,+2,+0"));
    assert!(specificity.maximum_exceeded);
    assert!(specificity.delta_exceeded);
}

#[test]
fn canonicalizes_policy_order_without_weakening_closed_schema() {
    let reordered = br#"{
      "schemaVersion": 1,
      "policyVersion": 1,
      "budgets": [
        {
          "id": "route-z",
          "subject": { "kind": "route", "id": "/z" },
          "limits": { "bytes": { "maximum": 20, "baseline": null, "maxIncrease": null } }
        },
        {
          "id": "package-a",
          "subject": { "kind": "package", "id": "app" },
          "limits": { "rules": { "maximum": 5, "baseline": null, "maxIncrease": null } }
        }
      ],
      "exceptions": []
    }"#;
    let policy = parse_budget_policy(reordered).expect("valid policy");
    assert_eq!(policy.budget_count(), 2);
    assert!(policy.uses_subject_kind(BudgetSubjectKind::Package));
    assert!(policy.uses_subject_kind(BudgetSubjectKind::Route));
    assert!(!policy.uses_subject_kind(BudgetSubjectKind::File));
    assert!(!policy.uses_subject_kind(BudgetSubjectKind::Layer));
    let canonical = policy.to_json_pretty().expect("canonical JSON");
    assert!(
        canonical.find("package-a").expect("package") < canonical.find("route-z").expect("route")
    );
    assert!(canonical.ends_with('\n'));

    let unknown = reordered.to_vec();
    let mut value: serde_json::Value = serde_json::from_slice(&unknown).expect("JSON");
    value["unexpected"] = serde_json::json!(true);
    assert!(parse_budget_policy(&serde_json::to_vec(&value).expect("JSON")).is_err());
}

#[test]
fn rejects_ambiguous_subjects_deltas_and_exceptions() {
    let duplicate_subject = br#"{
      "schemaVersion": 1,
      "policyVersion": 1,
      "budgets": [
        {"id":"one","subject":{"kind":"layer","id":"app"},"limits":{"rules":{"maximum":1,"baseline":null,"maxIncrease":null}}},
        {"id":"two","subject":{"kind":"layer","id":"app"},"limits":{"bytes":{"maximum":1,"baseline":null,"maxIncrease":null}}}
      ]
    }"#;
    assert!(parse_budget_policy(duplicate_subject).is_err());

    let delta_without_baseline = br#"{
      "schemaVersion": 1,
      "policyVersion": 1,
      "budgets": [{
        "id":"bad-delta",
        "subject":{"kind":"package","id":"app"},
        "limits":{"rules":{"maximum":10,"baseline":null,"maxIncrease":1}}
      }]
    }"#;
    assert!(parse_budget_policy(delta_without_baseline).is_err());

    let unknown_exception_metric = br#"{
      "schemaVersion": 1,
      "policyVersion": 1,
      "budgets": [{
        "id":"app",
        "subject":{"kind":"package","id":"app"},
        "limits":{"rules":{"maximum":10,"baseline":null,"maxIncrease":null}}
      }],
      "exceptions":[{
        "id":"wrong-metric","budget":"app","metrics":["bytes"],"justification":"reviewed"
      }]
    }"#;
    assert!(parse_budget_policy(unknown_exception_metric).is_err());
}

fn metric(
    report: &pliego_css_config::BudgetEvaluationReport,
    metric: BudgetMetric,
) -> &pliego_css_config::BudgetEvaluation {
    report
        .evaluations()
        .iter()
        .find(|evaluation| evaluation.metric == metric)
        .expect("metric evaluation")
}
