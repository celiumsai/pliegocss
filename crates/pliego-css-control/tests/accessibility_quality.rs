//! Machine-scored accessibility quality corpora.

use pliego_css_control::projection::{
    AccessibilityPolicySource, AccessibilityStylesheetInput, evaluate_accessibility,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Corpus {
    schema_version: u8,
    #[serde(rename = "corpusVersion")]
    version: String,
    claim_limit: String,
    abstention_cases: Vec<AbstentionCase>,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AbstentionCase {
    id: String,
    foreground: String,
    background: String,
    minimum_ratio_milli: u16,
    expected: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Case {
    id: String,
    foreground: String,
    background: String,
    minimum_ratio_milli: u16,
    expected: Expected,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Expected {
    Pass,
    Violation,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StaticCorpus {
    schema_version: u8,
    #[serde(rename = "corpusVersion")]
    version: String,
    claim_limit: String,
    cases: Vec<StaticCase>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StaticCase {
    id: String,
    check: String,
    css: String,
    expected_code: String,
}

fn pair_policy(id: &str, foreground: &str, background: &str, ratio: u16) -> Vec<u8> {
    format!(r#"{{"schemaVersion":1,"policyVersion":1,"checks":{{"contrast":{{"violation":"fail","unverified":"fail","manualRequired":"fail"}},"motion":{{"violation":"fail","unverified":"fail","manualRequired":"fail"}},"focusVisibility":{{"violation":"fail","unverified":"fail","manualRequired":"fail"}},"forcedColors":{{"violation":"fail","unverified":"fail","manualRequired":"fail"}},"inputModality":{{"violation":"fail","unverified":"fail","manualRequired":"fail"}}}},"contrastPairs":[{{"id":"{id}","foreground":{{"literal":{{"value":"{foreground}"}}}},"background":{{"literal":{{"value":"{background}"}}}},"minimumRatioMilli":{ratio}}}],"exceptions":[]}}"#).into_bytes()
}
fn empty_policy() -> Vec<u8> {
    br#"{"schemaVersion":1,"policyVersion":1,"checks":{"contrast":{"violation":"fail","unverified":"fail","manualRequired":"fail"},"motion":{"violation":"fail","unverified":"fail","manualRequired":"fail"},"focusVisibility":{"violation":"fail","unverified":"fail","manualRequired":"fail"},"forcedColors":{"violation":"fail","unverified":"fail","manualRequired":"fail"},"inputModality":{"violation":"fail","unverified":"fail","manualRequired":"fail"}},"contrastPairs":[],"exceptions":[]}"#.to_vec()
}

#[test]
fn literal_srgb_contrast_corpus_has_perfect_closed_slice_precision_and_recall() {
    let corpus: Corpus = serde_json::from_slice(include_bytes!(
        "../../../docs/benchmarks/accessibility-contrast-corpus.json"
    ))
    .unwrap();
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.version, "contrast-literal-srgb-1");
    assert!(corpus.claim_limit.contains("not WCAG certification"));
    let (mut tp, mut tn, mut fp, mut fn_) = (0_u64, 0_u64, 0_u64, 0_u64);
    for case in &corpus.cases {
        let bytes = pair_policy(
            &case.id,
            &case.foreground,
            &case.background,
            case.minimum_ratio_milli,
        );
        let outcome = evaluate_accessibility(
            &[],
            AccessibilityPolicySource {
                logical_path: "corpus-policy.json",
                bytes: &bytes,
            },
            None,
        )
        .unwrap();
        let predicted = outcome
            .findings
            .iter()
            .any(|f| serde_json::to_value(f).unwrap()["code"] == "PCSS-A11Y-101");
        match (matches!(case.expected, Expected::Violation), predicted) {
            (true, true) => tp += 1,
            (false, false) => tn += 1,
            (false, true) => fp += 1,
            (true, false) => fn_ += 1,
        }
    }
    assert_eq!((tp, tn, fp, fn_), (4, 4, 0, 0));
    assert_eq!(
        (tp * 1000 / (tp + fp), tp * 1000 / (tp + fn_)),
        (1000, 1000)
    );
}

#[test]
fn dynamic_contrast_cases_abstain_without_false_decisions() {
    let corpus: Corpus = serde_json::from_slice(include_bytes!(
        "../../../docs/benchmarks/accessibility-contrast-corpus.json"
    ))
    .unwrap();
    let mut abstained = 0_u64;
    for case in &corpus.abstention_cases {
        assert_eq!(case.expected, "unverified");
        let bytes = pair_policy(
            &case.id,
            &case.foreground,
            &case.background,
            case.minimum_ratio_milli,
        );
        let outcome = evaluate_accessibility(
            &[],
            AccessibilityPolicySource {
                logical_path: "corpus-policy.json",
                bytes: &bytes,
            },
            None,
        )
        .unwrap();
        let codes = outcome
            .findings
            .iter()
            .map(|f| {
                serde_json::to_value(f).unwrap()["code"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect::<Vec<_>>();
        assert!(codes.contains(&"PCSS-A11Y-108".into()));
        assert!(!codes.contains(&"PCSS-A11Y-100".into()));
        assert!(!codes.contains(&"PCSS-A11Y-101".into()));
        abstained += 1;
    }
    let total = corpus.cases.len() as u64 + abstained;
    assert_eq!(
        (
            abstained,
            corpus.cases.len() as u64 * 1000 / total,
            abstained * 1000 / total
        ),
        (4, 666, 333)
    );
}

#[test]
fn static_guard_corpus_emits_exact_expected_check_codes() {
    let corpus: StaticCorpus = serde_json::from_slice(include_bytes!(
        "../../../docs/benchmarks/accessibility-static-guards-corpus.json"
    ))
    .unwrap();
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.version, "static-guards-1");
    assert!(corpus.claim_limit.contains("Synthetic closed"));
    let policy = empty_policy();
    for case in &corpus.cases {
        let outcome = evaluate_accessibility(
            &[AccessibilityStylesheetInput {
                logical_path: "corpus.css",
                css: &case.css,
            }],
            AccessibilityPolicySource {
                logical_path: "corpus-policy.json",
                bytes: &policy,
            },
            None,
        )
        .unwrap();
        let matched = outcome.findings.iter().any(|finding| {
            let value = serde_json::to_value(finding).unwrap();
            value["code"] == case.expected_code && value["context"]["check"] == case.check
        });
        assert!(
            matched,
            "case {} did not emit {}",
            case.id, case.expected_code
        );
    }
}
