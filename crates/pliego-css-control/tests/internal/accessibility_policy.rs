use super::*;

fn complete_policy(pairs: &str, exceptions: &str) -> String {
    format!(
        r#"{{
  "schemaVersion": 1,
  "policyVersion": 1,
  "checks": {{
    "contrast": {{"violation":"fail","unverified":"warn","manualRequired":"warn"}},
    "motion": {{"violation":"fail","unverified":"warn","manualRequired":"warn"}},
    "focusVisibility": {{"violation":"fail","unverified":"warn","manualRequired":"warn"}},
    "forcedColors": {{"violation":"fail","unverified":"warn","manualRequired":"warn"}},
    "inputModality": {{"violation":"fail","unverified":"warn","manualRequired":"warn"}}
  }},
  "contrastPairs": {pairs},
  "exceptions": {exceptions}
}}"#
    )
}

#[test]
fn parser_canonicalizes_relationships_and_preserves_external_endpoint_tags() {
    let source = complete_policy(
        r##"[
          {"id":"z-last","foreground":{"literal":{"value":"#000"}},"background":{"literal":{"value":"#fff"}},"minimumRatioMilli":4500},
          {"id":"a-first","foreground":{"token":{"name":"color.text-primary"}},"background":{"token":{"name":"color.surface"}},"minimumRatioMilli":7000,"selections":{"appearance":"dark"}}
        ]"##,
        "[]",
    );
    let policy = parse_accessibility_policy(source.as_bytes()).expect("policy");
    let output = policy.to_json_pretty().expect("canonical JSON");

    assert_eq!(policy.policy_version(), 1);
    assert_eq!(policy.contrast_pair_count(), 2);
    assert!(output.find("a-first").unwrap() < output.find("z-last").unwrap());
    assert!(output.contains("\"token\": {"));
    assert!(output.ends_with('\n'));
}

#[test]
fn parser_rejects_duplicate_keys_in_free_and_closed_objects() {
    let duplicate_selection = complete_policy(
        r##"[{"id":"body","foreground":{"literal":{"value":"#000"}},"background":{"literal":{"value":"#fff"}},"minimumRatioMilli":4500,"selections":{"appearance":"dark","appe\u0061rance":"light"}}]"##,
        "[]",
    );
    let error = parse_accessibility_policy(duplicate_selection.as_bytes())
        .expect_err("decoded duplicate selection key");
    assert!(
        error
            .to_string()
            .contains("duplicate JSON object key `appearance`")
    );

    let duplicate_enforcement = complete_policy("[]", "[]").replace(
        r#""contrast": {"violation":"fail""#,
        r#""contrast": {"violation":"fail","violation":"warn""#,
    );
    let error = parse_accessibility_policy(duplicate_enforcement.as_bytes())
        .expect_err("duplicate closed-object key");
    assert!(error.to_string().contains("duplicate field `violation`"));
}

#[test]
fn all_checks_and_all_three_enforcement_fields_are_mandatory() {
    let complete = complete_policy("[]", "[]");
    let sources = [
        complete.replace("    \"forcedColors\": {\"violation\":\"fail\",\"unverified\":\"warn\",\"manualRequired\":\"warn\"},\n", ""),
        complete.replace("    \"forcedColors\": {\"violation\":\"fail\",\"unverified\":\"warn\",\"manualRequired\":\"warn\"},\n", "    \"forcedColors\": {\"violation\":\"fail\",\"unverified\":\"warn\"},\n"),
    ];
    for source in sources {
        let error = parse_accessibility_policy(source.as_bytes()).expect_err("missing field");
        assert!(error.to_string().contains("missing field"));
    }
}

#[test]
fn parser_rejects_unknown_fields_versions_and_invalid_pair_boundaries() {
    let unknown = complete_policy("[]", "[]").replace(
        "  \"policyVersion\": 1,",
        "  \"policyVersion\": 1,\n  \"ambientDate\": \"2099-01-01\",",
    );
    assert!(
        parse_accessibility_policy(unknown.as_bytes())
            .unwrap_err()
            .to_string()
            .contains("unknown field")
    );

    let version =
        complete_policy("[]", "[]").replace("\"policyVersion\": 1", "\"policyVersion\": 2");
    assert!(
        parse_accessibility_policy(version.as_bytes())
            .unwrap_err()
            .to_string()
            .contains("expected `1`")
    );

    let ratio = complete_policy(
        r##"[{"id":"body","foreground":{"literal":{"value":"#000"}},"background":{"literal":{"value":"#fff"}},"minimumRatioMilli":999}]"##,
        "[]",
    );
    assert!(
        parse_accessibility_policy(ratio.as_bytes())
            .unwrap_err()
            .to_string()
            .contains("minimumRatioMilli")
    );
}

#[test]
fn exceptions_are_sorted_and_cannot_overlap() {
    let subject = format!("sha256:{}", "a".repeat(64));
    let exceptions = format!(
        r#"[
          {{"id":"z-review","check":"contrast","subjectId":"{subject}","justification":"Reviewed by the design-system owner.","expiresOn":"2028-02-29"}},
          {{"id":"a-review","check":"contrast","subjectId":"{subject}","justification":"Second overlapping review.","expiresOn":"2028-12-01"}}
        ]"#
    );
    let error = parse_accessibility_policy(complete_policy("[]", &exceptions).as_bytes())
        .expect_err("overlap");
    assert!(error.to_string().contains("more than one exception"));

    let invalid_date = exceptions.replace("2028-02-29", "2027-02-29").replace(
        "\"check\":\"contrast\",\"subjectId\"",
        "\"check\":\"motion\",\"subjectId\"",
    );
    let error = parse_accessibility_policy(complete_policy("[]", &invalid_date).as_bytes())
        .expect_err("invalid date");
    assert!(error.to_string().contains("real `YYYY-MM-DD`"));
}
