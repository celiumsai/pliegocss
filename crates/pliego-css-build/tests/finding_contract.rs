//! Contract tests for canonical `PliegoCSS` findings.

use pliego_css_build::artifacts::{
    FINDING_SCHEMA_VERSION, Finding, FindingCause, FindingDocument, FindingEvidence,
    FindingException, FindingRisk, FindingSeverity, FindingSource, FindingSourceMap,
    FindingSuggestion, FindingTool, FindingVerification, parse_finding_document,
};
use serde_json::Value;

fn contrast_finding() -> Finding {
    let source = FindingSource::new("src/button.css", 120, 141)
        .expect("source")
        .with_position(8, 3, 8, 24)
        .expect("position")
        .with_source_map(FindingSourceMap::new("dist/pliego.css", 512, 548).expect("source map"));
    let cause = FindingCause::new(
        "accessibility.contrast",
        "minimum-ratio",
        "declared token pair is below the selected policy threshold",
    )
    .expect("cause");
    Finding::new(
        "PCSS-A11Y-001",
        "accessibility",
        FindingSeverity::Error,
        "contrast 3.42:1; policy requires 4.5:1",
        FindingVerification::Verified,
        cause,
    )
    .expect("finding")
    .with_source(source)
    .expect("source finding")
    .with_context("theme", "dark")
    .expect("theme")
    .with_context("selector", ".button--muted")
    .expect("selector")
    .with_evidence(
        FindingEvidence::new("ratio", "contrast", "3.42", "static-token-graph")
            .expect("evidence")
            .with_unit("ratio")
            .expect("unit"),
    )
    .expect("finding evidence")
    .with_suggestion(
        FindingSuggestion::new(
            1,
            "raise-text-contrast",
            "select a text token that reaches the configured ratio",
            FindingRisk::Low,
            "token",
        )
        .expect("suggestion")
        .with_prerequisites(["token-graph.resolved".into()])
        .expect("prerequisite"),
    )
    .expect("finding suggestion")
}

#[test]
fn canonical_json_and_human_views_share_the_finding() {
    let document = FindingDocument::new(
        FindingTool::new("pliegocss", "0.0.0").expect("tool"),
        "audit",
        vec![contrast_finding()],
    )
    .expect("document");
    let json = document.to_json_pretty().expect("json");
    let expected = r#"{
  "schemaVersion": "1.0.0",
  "tool": {
    "name": "pliegocss",
    "version": "0.0.0"
  },
  "command": "audit",
  "findings": [
    {
      "fingerprint": "sha256:329574f60969017bca03f08d2d8e6d54a1e82f1c54d516ea65831254ca6a7427",
      "code": "PCSS-A11Y-001",
      "category": "accessibility",
      "severity": "error",
      "message": "contrast 3.42:1; policy requires 4.5:1",
      "verification": "verified",
      "deterministic": true,
      "source": {
        "file": "src/button.css",
        "byteStart": 120,
        "byteEnd": 141,
        "startLine": 8,
        "startColumn": 3,
        "endLine": 8,
        "endColumn": 24,
        "sourceMap": {
          "file": "dist/pliego.css",
          "byteStart": 512,
          "byteEnd": 548
        }
      },
      "cause": {
        "policy": "accessibility.contrast",
        "rule": "minimum-ratio",
        "explanation": "declared token pair is below the selected policy threshold"
      },
      "context": {
        "selector": ".button--muted",
        "theme": "dark"
      },
      "evidence": [
        {
          "kind": "ratio",
          "name": "contrast",
          "value": "3.42",
          "unit": "ratio",
          "source": "static-token-graph"
        }
      ],
      "suggestions": [
        {
          "rank": 1,
          "id": "raise-text-contrast",
          "message": "select a text token that reaches the configured ratio",
          "risk": "low",
          "scope": "token",
          "prerequisites": [
            "token-graph.resolved"
          ]
        }
      ],
      "exception": null
    }
  ]
}
"#;
    assert_eq!(json, expected);
    let value: Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(value["schemaVersion"], FINDING_SCHEMA_VERSION);
    assert_eq!(value["findings"][0]["code"], "PCSS-A11Y-001");
    assert_eq!(value["findings"][0]["verification"], "verified");
    assert_eq!(value["findings"][0]["suggestions"][0]["risk"], "low");
    assert!(
        value["findings"][0]["fingerprint"]
            .as_str()
            .is_some_and(|value| value.starts_with("sha256:"))
    );
    let human = document.to_human().expect("human");
    assert!(human.contains("error[PCSS-A11Y-001]"));
    assert!(human.contains("src/button.css:8:3"));
    assert!(human.contains("verification: verified"));
    assert!(human.contains("evidence[ratio]: contrast = 3.42 ratio"));
    assert!(human.contains("help[1/low]"));
}

#[test]
fn parse_rejects_tampering_unknown_fields_and_noncanonical_order() {
    let first = contrast_finding();
    let second = Finding::new(
        "PCSS-COMPAT-004",
        "compatibility",
        FindingSeverity::Warning,
        "feature requires a documented fallback",
        FindingVerification::Unverified,
        FindingCause::new(
            "compatibility.baseline",
            "fallback-required",
            "the target vector does not prove native support",
        )
        .expect("cause"),
    )
    .expect("finding");
    let document = FindingDocument::new(
        FindingTool::new("pliegocss", "0.0.0").expect("tool"),
        "audit",
        vec![second, first],
    )
    .expect("document");
    let json = document.to_json_pretty().expect("json");
    assert_eq!(
        parse_finding_document(json.as_bytes()).expect("parse"),
        document
    );

    let mut tampered: Value = serde_json::from_str(&json).expect("json value");
    tampered["findings"][0]["context"]["theme"] = Value::String("light".into());
    let error = parse_finding_document(
        serde_json::to_vec(&tampered)
            .expect("tampered bytes")
            .as_slice(),
    )
    .expect_err("fingerprint tampering must fail");
    assert!(error.to_string().contains("fingerprint"));

    let mut unknown: Value = serde_json::from_str(&json).expect("json value");
    unknown["invented"] = Value::Bool(true);
    let error = parse_finding_document(
        serde_json::to_vec(&unknown)
            .expect("unknown bytes")
            .as_slice(),
    )
    .expect_err("unknown field must fail");
    assert!(error.to_string().contains("unknown field"));

    let mut reordered: Value = serde_json::from_str(&json).expect("json value");
    reordered["findings"]
        .as_array_mut()
        .expect("findings")
        .reverse();
    let error = parse_finding_document(
        serde_json::to_vec(&reordered)
            .expect("reordered bytes")
            .as_slice(),
    )
    .expect_err("noncanonical order must fail");
    assert!(error.to_string().contains("canonical"));
}

#[test]
fn suggestions_and_portable_paths_fail_closed() {
    let error =
        FindingSource::new("C:\\src\\button.css", 0, 1).expect_err("platform path must fail");
    assert!(error.to_string().contains("portable relative"));

    let finding = contrast_finding();
    let error = finding
        .with_suggestion(
            FindingSuggestion::new(
                3,
                "third",
                "invalid noncontiguous rank",
                FindingRisk::High,
                "project",
            )
            .expect("suggestion"),
        )
        .expect_err("rank gap must fail");
    assert!(error.to_string().contains("contiguous"));

    let error = FindingException::new("accepted-risk", "reviewed against the release policy")
        .expect("exception")
        .expiring_on("2026-02-30")
        .expect_err("invalid calendar date must fail");
    assert!(error.to_string().contains("YYYY-MM-DD"));
}
