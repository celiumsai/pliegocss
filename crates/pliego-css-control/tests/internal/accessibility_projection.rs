use pliego_css_theme::ThemeRegistry;
use serde_json::Value;

use super::*;

fn policy(pairs: &str, exceptions: &str) -> Vec<u8> {
    format!(
            r#"{{
              "schemaVersion":1,
              "policyVersion":1,
              "checks":{{
                "contrast":{{"violation":"fail","unverified":"fail","manualRequired":"fail"}},
                "motion":{{"violation":"fail","unverified":"fail","manualRequired":"fail"}},
                "focusVisibility":{{"violation":"fail","unverified":"fail","manualRequired":"fail"}},
                "forcedColors":{{"violation":"fail","unverified":"fail","manualRequired":"fail"}},
                "inputModality":{{"violation":"fail","unverified":"fail","manualRequired":"fail"}}
              }},
              "contrastPairs":{pairs},
              "exceptions":{exceptions}
            }}"#
        )
        .into_bytes()
}

fn codes(outcome: &AccessibilityAuditOutcome) -> Vec<String> {
    outcome
        .findings
        .iter()
        .map(|finding| {
            serde_json::to_value(finding)
                .expect("finding JSON")
                .get("code")
                .and_then(Value::as_str)
                .expect("finding code")
                .to_owned()
        })
        .collect()
}

fn code_for(outcome: &AccessibilityAuditOutcome, check: &str, selector: &str) -> String {
    outcome
        .findings
        .iter()
        .map(|finding| serde_json::to_value(finding).expect("finding JSON"))
        .find(|finding| {
            finding["context"]["check"] == check && finding["context"]["selector"] == selector
        })
        .and_then(|finding| finding["code"].as_str().map(str::to_owned))
        .expect("selector finding")
}

fn subject_for_code(outcome: &AccessibilityAuditOutcome, code: &str) -> String {
    outcome
        .findings
        .iter()
        .map(|finding| serde_json::to_value(finding).expect("finding JSON"))
        .find(|finding| finding["code"] == code)
        .and_then(|finding| finding["context"]["subject-id"].as_str().map(str::to_owned))
        .expect("finding subject")
}

fn subject_for_selector(
    outcome: &AccessibilityAuditOutcome,
    check: &str,
    selector: &str,
) -> String {
    outcome
        .findings
        .iter()
        .map(|finding| serde_json::to_value(finding).expect("finding JSON"))
        .find(|finding| {
            finding["context"]["check"] == check && finding["context"]["selector"] == selector
        })
        .and_then(|finding| finding["context"]["subject-id"].as_str().map(str::to_owned))
        .expect("selector subject")
}

#[test]
fn contrast_uses_wcag_22_cutoff_without_rounding_and_refuses_dynamic_claims() {
    let pairs = r##"[
          {"id":"black-white","foreground":{"literal":{"value":"#000"}},"background":{"literal":{"value":"#fff"}},"minimumRatioMilli":21000},
          {"id":"gray-white","foreground":{"literal":{"value":"#777"}},"background":{"literal":{"value":"#fff"}},"minimumRatioMilli":4500},
          {"id":"runtime","foreground":{"literal":{"value":"var(--text)"}},"background":{"literal":{"value":"#fff"}},"minimumRatioMilli":4500}
        ]"##;
    let bytes = policy(pairs, "[]");
    let outcome = evaluate_accessibility(
        &[],
        AccessibilityPolicySource {
            logical_path: "pliego.accessibility.json",
            bytes: &bytes,
        },
        None,
    )
    .expect("audit");
    let codes = codes(&outcome);

    assert!(!outcome.passed);
    assert_eq!(outcome.contrast_pairs, 3);
    assert!(codes.contains(&"PCSS-A11Y-100".into()));
    assert!(codes.contains(&"PCSS-A11Y-101".into()));
    assert!(codes.contains(&"PCSS-A11Y-108".into()));
}

#[test]
fn contrast_exception_is_invalidated_by_resolved_graph_evidence() {
    let pairs = r#"[{"id":"white-on-surface","foreground":{"token":{"name":"color.white"}},"background":{"token":{"name":"color.surface"}},"minimumRatioMilli":4500}]"#;
    let missing = policy(pairs, "[]");
    let initial = evaluate_accessibility(
        &[],
        AccessibilityPolicySource {
            logical_path: "pliego.accessibility.json",
            bytes: &missing,
        },
        None,
    )
    .expect("missing graph audit");
    let subject = subject_for_code(&initial, "PCSS-A11Y-108");
    let exceptions = format!(
        r#"[{{"id":"missing-graph-review","check":"contrast","subjectId":"{subject}","justification":"Reviewed while graph evidence was unavailable.","expiresOn":"2030-01-01"}}]"#
    );
    let reviewed = policy(pairs, &exceptions);
    let graph = TokenGraph::from_registry(&ThemeRegistry::seed());
    let resolved = evaluate_accessibility(
        &[],
        AccessibilityPolicySource {
            logical_path: "pliego.accessibility.json",
            bytes: &reviewed,
        },
        Some(&graph),
    )
    .expect("resolved graph audit");
    let resolved_codes = codes(&resolved);

    assert!(resolved_codes.contains(&"PCSS-A11Y-101".into()));
    assert!(resolved_codes.contains(&"PCSS-A11Y-999".into()));
    assert!(!resolved_codes.contains(&"PCSS-A11Y-102".into()));
}

#[test]
fn css_ast_checks_motion_focus_forced_colors_and_input_equivalence() {
    let bytes = policy("[]", "[]");
    let css = r"/* 😀 */ @media (prefers-reduced-motion: no-preference) { .safe { animation: spin 1s infinite; } }
          .unsafe { transition: opacity 200ms; }
          .button:focus-visible { outline: none; }
          .icon { forced-color-adjust: none; }
          .link:hover { color: red; }
          .paired:hover, .paired:focus { color: red; }
        ";
    let outcome = evaluate_accessibility(
        &[AccessibilityStylesheetInput {
            logical_path: "src/app.css",
            css,
        }],
        AccessibilityPolicySource {
            logical_path: "pliego.accessibility.json",
            bytes: &bytes,
        },
        None,
    )
    .expect("audit");
    let codes = codes(&outcome);

    for code in [
        "PCSS-A11Y-200",
        "PCSS-A11Y-201",
        "PCSS-A11Y-301",
        "PCSS-A11Y-401",
        "PCSS-A11Y-500",
        "PCSS-A11Y-501",
    ] {
        assert!(
            codes.contains(&code.to_owned()),
            "missing {code}: {codes:?}"
        );
    }
    let safe = outcome
        .findings
        .iter()
        .map(|finding| serde_json::to_value(finding).unwrap())
        .find(|finding| finding["code"] == "PCSS-A11Y-200")
        .expect("safe motion finding");
    let start = usize::try_from(safe["source"]["byteStart"].as_u64().unwrap()).unwrap();
    let end = usize::try_from(safe["source"]["byteEnd"].as_u64().unwrap()).unwrap();
    assert_eq!(safe["source"]["startLine"], 1);
    assert_eq!(&css[start..end], ".safe");
    assert!(!outcome.passed);
}

#[test]
fn motion_media_proofs_fail_closed() {
    let bytes = policy("[]", "[]");
    let css = r"@media (prefers-reduced-motion:no-preference){.exact{animation:x 1s}}
@media (prefers-reduced-motion:reduce){.reduced{animation:x 1s}}
@media not (prefers-reduced-motion:reduce){.negated{animation:x 1s}}
@media (prefers-reduced-motion:reduce) or (min-width:1px){.disjoined{animation:x 1s}}
@media (prefers-reduced-motion:reduce){@media (prefers-reduced-motion:no-preference){.contradictory{animation:x 1s}}}
.separate{animation:x 1s}@media (prefers-reduced-motion:reduce){.separate{animation:none}}
.split{animation-name:spin}.split{animation-duration:1s}
@media (prefers-reduced-motion:no-preference){.compete{animation:spin 1s}}
.compete{animation:none!important}
@media (prefers-reduced-motion:no-preference){.mixed{animation:none;animation-name:spin;animation-duration:1s}}";
    let outcome = evaluate_accessibility(
        &[AccessibilityStylesheetInput {
            logical_path: "motion.css",
            css,
        }],
        AccessibilityPolicySource {
            logical_path: "pliego.accessibility.json",
            bytes: &bytes,
        },
        None,
    )
    .expect("audit");

    assert_eq!(code_for(&outcome, "motion", ".exact"), "PCSS-A11Y-200");
    assert_eq!(code_for(&outcome, "motion", ".reduced"), "PCSS-A11Y-201");
    for selector in [
        ".negated",
        ".disjoined",
        ".contradictory",
        ".separate",
        ".split",
        ".compete",
        ".mixed",
    ] {
        assert_eq!(code_for(&outcome, "motion", selector), "PCSS-A11Y-209");
    }
}

#[test]
fn selector_correlation_never_crosses_rule_bundle_or_nesting() {
    let bytes = policy("[]", "[]");
    let first = r".cross-motion{animation:x 1s}.cross:hover{color:red}
.nested{&:hover,&:focus{color:red}}
.mismatch:hover{color:red}.mismatch:focus{color:blue}";
    let second = r"@media (prefers-reduced-motion:reduce){.cross-motion{animation:none}}
.cross:focus{color:red}";
    let outcome = evaluate_accessibility(
        &[
            AccessibilityStylesheetInput {
                logical_path: "a.css",
                css: first,
            },
            AccessibilityStylesheetInput {
                logical_path: "b.css",
                css: second,
            },
        ],
        AccessibilityPolicySource {
            logical_path: "pliego.accessibility.json",
            bytes: &bytes,
        },
        None,
    )
    .expect("audit");

    assert_eq!(
        code_for(&outcome, "motion", ".cross-motion"),
        "PCSS-A11Y-201"
    );
    for selector in [".cross:hover", "&:hover", ".mismatch:hover"] {
        assert_eq!(
            code_for(&outcome, "inputModality", selector),
            "PCSS-A11Y-509"
        );
    }
}

#[test]
fn pseudo_detection_ignores_attribute_text_and_escaped_colons() {
    let bytes = policy("[]", "[]");
    let css = r#"[data-state=":hover"]{color:red}.escaped\:hover{color:blue}
.actual[data-state="ready"]:hover,.actual[data-state="ready"]:focus{color:green}"#;
    let outcome = evaluate_accessibility(
        &[AccessibilityStylesheetInput {
            logical_path: "selectors.css",
            css,
        }],
        AccessibilityPolicySource {
            logical_path: "pliego.accessibility.json",
            bytes: &bytes,
        },
        None,
    )
    .expect("audit");

    let input = outcome
        .findings
        .iter()
        .map(|finding| serde_json::to_value(finding).unwrap())
        .filter(|finding| {
            finding["category"] == "accessibility" && finding["context"]["check"] == "inputModality"
        })
        .collect::<Vec<_>>();
    assert_eq!(input.len(), 1);
    assert_eq!(input[0]["code"], "PCSS-A11Y-500");
    assert_eq!(
        input[0]["context"]["selector"],
        ".actual[data-state=ready]:hover"
    );
}

#[test]
fn relational_exceptions_do_not_survive_removed_evidence() {
    let initial_css = r".motion{animation:spin 1s}@media (prefers-reduced-motion:reduce){.motion{animation:none}}
.link:hover{color:red}.link:focus{color:blue}";
    let plain = policy("[]", "[]");
    let initial = evaluate_accessibility(
        &[AccessibilityStylesheetInput {
            logical_path: "relations.css",
            css: initial_css,
        }],
        AccessibilityPolicySource {
            logical_path: "pliego.accessibility.json",
            bytes: &plain,
        },
        None,
    )
    .expect("initial relations audit");
    let motion_subject = subject_for_selector(&initial, "motion", ".motion");
    let input_subject = subject_for_selector(&initial, "inputModality", ".link:hover");
    let exceptions = format!(
        r#"[
          {{"id":"motion-review","check":"motion","subjectId":"{motion_subject}","justification":"Reviewed with a separate reduced-motion rule.","expiresOn":"2030-01-01"}},
          {{"id":"input-review","check":"inputModality","subjectId":"{input_subject}","justification":"Reviewed with a separate focus rule.","expiresOn":"2030-01-01"}}
        ]"#
    );
    let reviewed = policy("[]", &exceptions);
    let changed = evaluate_accessibility(
        &[AccessibilityStylesheetInput {
            logical_path: "relations.css",
            css: ".motion{animation:spin 1s}.link:hover{color:red}",
        }],
        AccessibilityPolicySource {
            logical_path: "pliego.accessibility.json",
            bytes: &reviewed,
        },
        None,
    )
    .expect("changed relations audit");
    let changed_codes = codes(&changed);

    assert!(changed_codes.contains(&"PCSS-A11Y-201".into()));
    assert!(changed_codes.contains(&"PCSS-A11Y-501".into()));
    assert_eq!(
        changed_codes
            .iter()
            .filter(|code| code.as_str() == "PCSS-A11Y-999")
            .count(),
        2
    );
    assert!(!changed_codes.contains(&"PCSS-A11Y-202".into()));
    assert!(!changed_codes.contains(&"PCSS-A11Y-502".into()));
}

#[test]
fn focus_requires_computed_cascade_evidence() {
    let bytes = policy("[]", "[]");
    let css = r".button{outline:none}.button:focus{color:red}
.suppressed:focus-visible{outline:none!important}
.positive:focus{outline:2px solid blue}";
    let outcome = evaluate_accessibility(
        &[AccessibilityStylesheetInput {
            logical_path: "focus.css",
            css,
        }],
        AccessibilityPolicySource {
            logical_path: "pliego.accessibility.json",
            bytes: &bytes,
        },
        None,
    )
    .expect("audit");

    assert_eq!(
        code_for(&outcome, "focusVisibility", ".button:focus"),
        "PCSS-A11Y-309"
    );
    assert_eq!(
        code_for(&outcome, "focusVisibility", ".suppressed:focus-visible"),
        "PCSS-A11Y-301"
    );
    assert_eq!(
        code_for(&outcome, "focusVisibility", ".positive:focus"),
        "PCSS-A11Y-309"
    );
    assert!(!codes(&outcome).contains(&"PCSS-A11Y-300".into()));
}

#[test]
fn reviewed_exception_preserves_verification_and_does_not_fail() {
    let pairs = r##"[{"id":"low","foreground":{"literal":{"value":"#777"}},"background":{"literal":{"value":"#fff"}},"minimumRatioMilli":7000}]"##;
    let initial = policy(pairs, "[]");
    let first = evaluate_accessibility(
        &[],
        AccessibilityPolicySource {
            logical_path: "pliego.accessibility.json",
            bytes: &initial,
        },
        None,
    )
    .expect("initial audit");
    let subject = first
        .findings
        .iter()
        .map(|finding| serde_json::to_value(finding).unwrap())
        .find(|finding| finding["code"] == "PCSS-A11Y-101")
        .and_then(|finding| finding["context"]["subject-id"].as_str().map(str::to_owned))
        .expect("contrast subject");
    let exceptions = format!(
        r#"[{{"id":"reviewed-low","check":"contrast","subjectId":"{subject}","justification":"Reviewed for this release.","expiresOn":"2030-01-01"}}]"#
    );
    let reviewed = policy(pairs, &exceptions);
    let outcome = evaluate_accessibility(
        &[],
        AccessibilityPolicySource {
            logical_path: "pliego.accessibility.json",
            bytes: &reviewed,
        },
        None,
    )
    .expect("reviewed audit");
    let finding = outcome
        .findings
        .iter()
        .map(|finding| serde_json::to_value(finding).unwrap())
        .find(|finding| finding["code"] == "PCSS-A11Y-102")
        .expect("exception finding");

    assert!(outcome.passed);
    assert_eq!(finding["severity"], "warning");
    assert_eq!(finding["verification"], "verified");
    assert_eq!(finding["exception"]["id"], "reviewed-low");

    for changed in [pairs.replace("7000", "7100"), pairs.replace("#777", "#666")] {
        let bytes = policy(&changed, &exceptions);
        let changed = evaluate_accessibility(
            &[],
            AccessibilityPolicySource {
                logical_path: "pliego.accessibility.json",
                bytes: &bytes,
            },
            None,
        )
        .expect("changed audit");
        let codes = codes(&changed);
        assert!(codes.contains(&"PCSS-A11Y-101".into()));
        assert!(codes.contains(&"PCSS-A11Y-999".into()));
        assert!(!codes.contains(&"PCSS-A11Y-102".into()));
    }
}
