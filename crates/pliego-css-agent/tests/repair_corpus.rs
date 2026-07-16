//! Data-driven conformance corpus for the bounded repair authority boundary.

use std::collections::{BTreeMap, BTreeSet};

use pliego_css_agent::{RepairTool, build_repair_plan, parse_repair_proposal};
use pliego_css_build::artifacts::{
    Finding, FindingCause, FindingDocument, FindingException, FindingRisk, FindingSeverity,
    FindingSource, FindingSuggestion, FindingTool, FindingVerification, sha256_hex,
};
use serde::{Deserialize, Serialize};

const CORPUS_SOURCE: &str = include_str!("../../../benchmarks/repair-corpus/cases.json");
const CORPUS_ID: &str = "repair-authority-conformance-v1";
const CLAIM_BOUNDARY: &str = "conformance-only-not-real-incident-or-agent-turn-evidence";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Corpus {
    schema_version: u8,
    #[serde(rename = "corpusId")]
    id: String,
    provenance: Provenance,
    claim_boundary: String,
    cases: Vec<CorpusCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    kind: String,
    source: String,
    consent: String,
    redaction: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CorpusCase {
    id: String,
    category: String,
    mutation: String,
    expected: ExpectedOutcome,
    reason_contains: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum ExpectedOutcome {
    Accepted,
    Rejected,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseResult<'a> {
    id: &'a str,
    category: &'a str,
    expected: ExpectedOutcome,
    actual: ExpectedOutcome,
    reason_matched: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CorpusReport<'a> {
    schema_version: u8,
    corpus_id: &'a str,
    claim_boundary: &'a str,
    total: usize,
    accepted: usize,
    rejected: usize,
    cases: Vec<CaseResult<'a>>,
}

fn validate_corpus(corpus: &Corpus) {
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.id, CORPUS_ID);
    assert_eq!(corpus.claim_boundary, CLAIM_BOUNDARY);
    assert_eq!(corpus.provenance.kind, "synthetic");
    assert_eq!(
        corpus.provenance.source,
        "engineered from the public repair contract"
    );
    assert_eq!(corpus.provenance.consent, "not-applicable");
    assert_eq!(corpus.provenance.redaction, "not-required");
    assert_eq!(corpus.cases.len(), 16);

    let ids = corpus
        .cases
        .iter()
        .map(|case| case.id.as_str())
        .collect::<Vec<_>>();
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        ids.iter().copied().collect::<BTreeSet<_>>().len(),
        ids.len()
    );
    assert_eq!(
        corpus
            .cases
            .iter()
            .filter(|case| case.expected == ExpectedOutcome::Accepted)
            .count(),
        1
    );
}

struct CaseInputs {
    findings: Vec<u8>,
    proposal: serde_json::Value,
    sources: BTreeMap<String, Vec<u8>>,
    start: usize,
    fingerprint: String,
}

fn build_case_inputs(case: &CorpusCase) -> CaseInputs {
    let verification = if case.mutation == "unverified-finding" {
        FindingVerification::Unverified
    } else {
        FindingVerification::Verified
    };
    let risk = if case.mutation == "medium-risk-suggestion" {
        FindingRisk::Medium
    } else {
        FindingRisk::Low
    };
    let css = b".button { color: #777; }\n".to_vec();
    let start = std::str::from_utf8(&css)
        .expect("fixture CSS")
        .find("#777")
        .expect("fixture literal");
    let finding = Finding::new(
        "A11Y001",
        "accessibility",
        FindingSeverity::Error,
        "declared contrast is below policy",
        verification,
        FindingCause::new(
            "accessibility.contrast",
            "minimum-ratio",
            "ratio is below the configured minimum",
        )
        .expect("cause"),
    )
    .expect("finding")
    .with_source(FindingSource::new("src/app.css", start, start + 4).expect("source"))
    .expect("finding source")
    .with_suggestion(
        FindingSuggestion::new(
            1,
            "replace-color",
            "replace the local literal",
            risk,
            "declaration",
        )
        .expect("suggestion")
        .with_prerequisites(vec!["audit".into()])
        .expect("prerequisite"),
    )
    .expect("finding suggestion");
    let finding = if case.mutation == "reviewed-exception" {
        finding
            .with_exception(
                FindingException::new("reviewed", "accepted by policy").expect("exception"),
            )
            .expect("finding exception")
    } else {
        finding
    };
    let fingerprint = finding.fingerprint().to_owned();
    let findings = FindingDocument::new(
        FindingTool::new("pliegocss", "0.0.0").expect("finding tool"),
        "audit",
        vec![finding],
    )
    .expect("finding document")
    .to_json_pretty()
    .expect("finding JSON")
    .into_bytes();

    let proposal = serde_json::json!({
        "schemaVersion": "1.0.0",
        "findingDocumentSha256": sha256_hex(&findings),
        "changeBudget": {
            "maxFiles": 1,
            "maxEdits": 1,
            "maxInsertedBytes": 4,
            "maxRemovedBytes": 4
        },
        "requiredChecks": ["audit"],
        "edits": [{
            "findingFingerprint": fingerprint,
            "suggestionId": "replace-color",
            "file": "src/app.css",
            "byteStart": start,
            "byteEnd": start + 4,
            "replacement": "#111"
        }]
    });
    CaseInputs {
        findings,
        proposal,
        sources: BTreeMap::from([("src/app.css".into(), css)]),
        start,
        fingerprint,
    }
}

fn mutate_case_inputs(mutation: &str, inputs: &mut CaseInputs) {
    let proposal = &mut inputs.proposal;
    let sources = &mut inputs.sources;
    let start = inputs.start;
    let fingerprint = &inputs.fingerprint;
    match mutation {
        "none" | "unverified-finding" | "medium-risk-suggestion" | "reviewed-exception" => {}
        "inserted-budget-exceeded" => proposal["changeBudget"]["maxInsertedBytes"] = 3.into(),
        "removed-budget-exceeded" => proposal["changeBudget"]["maxRemovedBytes"] = 3.into(),
        "finding-hash-drift" => proposal["findingDocumentSha256"] = "0".repeat(64).into(),
        "invalid-source-path" => proposal["edits"][0]["file"] = "../app.css".into(),
        "missing-prerequisite" => {
            proposal["requiredChecks"] = serde_json::json!(["checks.other"]);
        }
        "non-utf8-source" => {
            sources.get_mut("src/app.css").expect("source")[0] = 0xff;
        }
        "overlapping-edits" => {
            proposal["changeBudget"]["maxEdits"] = 2.into();
            proposal["changeBudget"]["maxInsertedBytes"] = 8.into();
            proposal["changeBudget"]["maxRemovedBytes"] = 8.into();
            proposal["edits"] = serde_json::json!([
                {
                    "findingFingerprint": fingerprint,
                    "suggestionId": "replace-color",
                    "file": "src/app.css",
                    "byteStart": start,
                    "byteEnd": start + 3,
                    "replacement": "#11"
                },
                {
                    "findingFingerprint": fingerprint,
                    "suggestionId": "replace-color",
                    "file": "src/app.css",
                    "byteStart": start + 2,
                    "byteEnd": start + 4,
                    "replacement": "11"
                }
            ]);
        }
        "source-range-escape" => proposal["edits"][0]["byteStart"] = (start - 1).into(),
        "source-set-drift" => {
            sources.insert("src/extra.css".into(), b".extra {}\n".to_vec());
        }
        "unknown-field" => proposal["unexpected"] = true.into(),
        "unknown-finding" => {
            proposal["edits"][0]["findingFingerprint"] =
                format!("sha256:{}", "0".repeat(64)).into();
        }
        "unknown-suggestion" => proposal["edits"][0]["suggestionId"] = "other".into(),
        mutation => panic!("unsupported corpus mutation `{mutation}`"),
    }
}

fn execute_case(case: &CorpusCase) -> Result<(), String> {
    let mut inputs = build_case_inputs(case);
    mutate_case_inputs(&case.mutation, &mut inputs);
    let proposal_bytes = serde_json::to_vec(&inputs.proposal).expect("proposal JSON");
    let parsed = parse_repair_proposal(&proposal_bytes).map_err(|error| error.to_string())?;
    let first = build_repair_plan(
        RepairTool::new("pliegocss", "0.0.0").expect("repair tool"),
        "findings.json",
        &inputs.findings,
        &parsed,
        &inputs.sources,
    )
    .map_err(|error| error.to_string())?;
    let second = build_repair_plan(
        RepairTool::new("pliegocss", "0.0.0").expect("repair tool"),
        "findings.json",
        &inputs.findings,
        &parsed,
        &inputs.sources,
    )
    .map_err(|error| error.to_string())?;
    if first.to_json_pretty().map_err(|error| error.to_string())?
        != second.to_json_pretty().map_err(|error| error.to_string())?
    {
        return Err("accepted plan was not deterministic".into());
    }
    Ok(())
}

#[test]
fn frozen_repair_authority_corpus_matches_the_closed_contract() {
    let corpus: Corpus = serde_json::from_str(CORPUS_SOURCE).expect("closed corpus JSON");
    validate_corpus(&corpus);
    let mut results = Vec::with_capacity(corpus.cases.len());
    let mut accepted = 0;
    let mut rejected = 0;

    for case in &corpus.cases {
        let execution = execute_case(case);
        let (actual, reason) = match execution {
            Ok(()) => {
                accepted += 1;
                (ExpectedOutcome::Accepted, "accepted".to_owned())
            }
            Err(reason) => {
                rejected += 1;
                (ExpectedOutcome::Rejected, reason)
            }
        };
        assert_eq!(actual, case.expected, "{}: {reason}", case.id);
        let reason_matched = reason.contains(&case.reason_contains);
        assert!(
            reason_matched,
            "{}: expected reason containing {:?}, got {:?}",
            case.id, case.reason_contains, reason
        );
        results.push(CaseResult {
            id: &case.id,
            category: &case.category,
            expected: case.expected,
            actual,
            reason_matched,
        });
    }

    let report = CorpusReport {
        schema_version: 1,
        corpus_id: &corpus.id,
        claim_boundary: &corpus.claim_boundary,
        total: corpus.cases.len(),
        accepted,
        rejected,
        cases: results,
    };
    println!(
        "PLIEGOCSS_REPAIR_CORPUS_REPORT {}",
        serde_json::to_string(&report).expect("corpus report")
    );
}
