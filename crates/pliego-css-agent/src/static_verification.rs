use std::collections::{BTreeMap, BTreeSet};

use pliego_css_build::artifacts::{
    Finding, FindingCause, FindingDocument, FindingSeverity, FindingSource, FindingTool,
    FindingVerification, audit_standard_css, audit_standard_css_with_budgets, sha256_hex,
};
use pliego_css_config::{parse_budget_policy, parse_token_graph};

use super::validation::validate_logical_path;
use super::{
    MAX_TOTAL_VERIFICATION_EVIDENCE_BYTES, REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION,
    RepairArtifactIdentity, RepairBrowserEvidenceResult, RepairBrowserRequirement,
    RepairChangeReceipt, RepairCheckDefinition, RepairCheckKind, RepairCheckPolicy,
    RepairContractError, RepairTestEvidenceResult, RepairVerificationBrowserEvidence,
    RepairVerificationCheck, RepairVerificationCheckStatus, RepairVerificationReceipt,
    derive_verification_result, parse_repair_browser_evidence, parse_repair_test_evidence,
    verification_receipt_payload_sha256,
};

/// Executes a closed built-in check policy over the exact after state of one Change Receipt.
///
/// No executable, argument vector, environment mutation, or shell fragment can be supplied by the
/// plan or policy. The closed schema supports only in-process standard-CSS, token-graph, and CSS
/// budget checks. Every changed source must be covered by at least one check, and every check ID
/// must exactly match the Change Receipt.
///
/// # Errors
///
/// Returns [`RepairContractError`] for policy/receipt mismatch, source drift or coverage gaps,
/// malformed UTF-8, unsupported policy, audit execution failure, or receipt serialization failure.
pub fn execute_repair_verification(
    change_receipt: &RepairChangeReceipt,
    change_receipt_file: &str,
    change_receipt_bytes: &[u8],
    policy_file: &str,
    policy_bytes: &[u8],
    policy: &RepairCheckPolicy,
    sources: &BTreeMap<String, Vec<u8>>,
) -> Result<RepairVerificationReceipt, RepairContractError> {
    let supplementary = BTreeMap::new();
    execute_repair_verification_with_inputs(
        change_receipt,
        change_receipt_file,
        change_receipt_bytes,
        policy_file,
        policy_bytes,
        policy,
        RepairVerificationInputs::new(sources, &supplementary),
    )
}

/// Exact changed and supplementary bytes supplied to post-change verification.
#[derive(Clone, Copy, Debug)]
pub struct RepairVerificationInputs<'a> {
    sources: &'a BTreeMap<String, Vec<u8>>,
    supplementary: &'a BTreeMap<String, Vec<u8>>,
}

impl<'a> RepairVerificationInputs<'a> {
    /// Binds the complete Change Receipt after-source set and exact supplementary policy inputs.
    #[must_use]
    pub const fn new(
        sources: &'a BTreeMap<String, Vec<u8>>,
        supplementary: &'a BTreeMap<String, Vec<u8>>,
    ) -> Self {
        Self {
            sources,
            supplementary,
        }
    }
}

/// Executes a closed built-in check policy with exact supplementary input artifacts.
///
/// Supplementary inputs are accepted only when the policy binds their portable path, byte length,
/// and SHA-256. Schema 1.2 admits CSS budget policies; schema 1.3 also admits fixed-profile test
/// evidence plus its exact root Cargo manifest and lockfile; schema 1.4 admits fixed-profile
/// `PliegoRS` Chromium evidence plus its exact profile inputs. Changed sources remain bound
/// exclusively by the Change Receipt. Extra, missing, overlapping, or drifted inputs fail closed.
///
/// # Errors
///
/// Returns [`RepairContractError`] for the same conditions as
/// [`execute_repair_verification`], plus supplementary-input identity or coverage drift.
pub fn execute_repair_verification_with_inputs(
    change_receipt: &RepairChangeReceipt,
    change_receipt_file: &str,
    change_receipt_bytes: &[u8],
    policy_file: &str,
    policy_bytes: &[u8],
    policy: &RepairCheckPolicy,
    inputs: RepairVerificationInputs<'_>,
) -> Result<RepairVerificationReceipt, RepairContractError> {
    prepare_repair_verification(
        change_receipt,
        change_receipt_file,
        change_receipt_bytes,
        policy_file,
        policy_bytes,
        policy,
        inputs,
    )
    .map(|prepared| prepared.receipt)
}

pub(crate) struct PreparedRepairVerification {
    pub(crate) receipt: RepairVerificationReceipt,
    pub(crate) finding_documents: BTreeMap<String, Vec<u8>>,
}

pub(crate) fn prepare_repair_verification(
    change_receipt: &RepairChangeReceipt,
    change_receipt_file: &str,
    change_receipt_bytes: &[u8],
    policy_file: &str,
    policy_bytes: &[u8],
    policy: &RepairCheckPolicy,
    inputs: RepairVerificationInputs<'_>,
) -> Result<PreparedRepairVerification, RepairContractError> {
    validate_repair_verification_inputs(
        change_receipt,
        change_receipt_file,
        change_receipt_bytes,
        policy_file,
        policy_bytes,
        policy,
        inputs,
    )?;
    let mut checks = Vec::with_capacity(policy.checks.len());
    let mut finding_documents = BTreeMap::new();
    let mut total_evidence_bytes = 0usize;
    for definition in &policy.checks {
        let source = inputs.sources.get(&definition.source).ok_or_else(|| {
            RepairContractError::new(format!(
                "check `{}` references source `{}` outside the Change Receipt",
                definition.id, definition.source
            ))
        })?;
        let (check, finding_document) = execute_repair_check(
            definition,
            source,
            inputs.sources,
            inputs.supplementary,
            &change_receipt.receipt_sha256,
        )?;
        total_evidence_bytes = total_evidence_bytes
            .checked_add(finding_document.len())
            .ok_or_else(|| {
                RepairContractError::new("verification evidence byte count overflowed")
            })?;
        if total_evidence_bytes > MAX_TOTAL_VERIFICATION_EVIDENCE_BYTES {
            return Err(RepairContractError::new(
                "verification finding documents exceed the 64 MiB aggregate limit",
            ));
        }
        finding_documents.insert(definition.id.clone(), finding_document);
        checks.push(check);
    }
    let browser_check = checks
        .iter()
        .find(|check| check.kind == RepairCheckKind::BrowserEvidence);
    let browser_evidence = match (policy.browser_evidence, browser_check) {
        (RepairBrowserRequirement::NotRequired, _) => {
            RepairVerificationBrowserEvidence::NotRequired
        }
        (RepairBrowserRequirement::Required, None) => {
            RepairVerificationBrowserEvidence::RequiredNotCollected
        }
        (RepairBrowserRequirement::Required, Some(check))
            if check.status == RepairVerificationCheckStatus::Passed =>
        {
            RepairVerificationBrowserEvidence::RequiredPassed
        }
        (RepairBrowserRequirement::Required, Some(_)) => {
            RepairVerificationBrowserEvidence::RequiredFailed
        }
    };
    let result = derive_verification_result(&checks, browser_evidence);
    let mut receipt = RepairVerificationReceipt {
        schema_version: REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION.into(),
        receipt_sha256: "0".repeat(64),
        change_receipt: RepairArtifactIdentity {
            file: change_receipt_file.into(),
            bytes: change_receipt_bytes.len(),
            sha256: sha256_hex(change_receipt_bytes),
        },
        change_receipt_sha256: change_receipt.receipt_sha256.clone(),
        check_policy: RepairArtifactIdentity {
            file: policy_file.into(),
            bytes: policy_bytes.len(),
            sha256: sha256_hex(policy_bytes),
        },
        result,
        checks,
        browser_evidence,
    };
    receipt.receipt_sha256 = verification_receipt_payload_sha256(&receipt)?;
    receipt.validate()?;
    Ok(PreparedRepairVerification {
        receipt,
        finding_documents,
    })
}

fn validate_repair_verification_inputs(
    change_receipt: &RepairChangeReceipt,
    change_receipt_file: &str,
    change_receipt_bytes: &[u8],
    policy_file: &str,
    policy_bytes: &[u8],
    policy: &RepairCheckPolicy,
    inputs: RepairVerificationInputs<'_>,
) -> Result<(), RepairContractError> {
    change_receipt.validate()?;
    validate_logical_path("changeReceipt.file", change_receipt_file)?;
    if change_receipt.to_json_pretty()?.as_bytes() != change_receipt_bytes {
        return Err(RepairContractError::new(
            "Change Receipt object does not match its canonical bytes",
        ));
    }
    policy.validate()?;
    validate_logical_path("checkPolicy.file", policy_file)?;
    if policy.to_json_pretty()?.as_bytes() != policy_bytes {
        return Err(RepairContractError::new(
            "repair-check policy object does not match its canonical bytes",
        ));
    }
    let required = change_receipt
        .checks
        .iter()
        .map(|check| check.id.as_str())
        .collect::<Vec<_>>();
    let configured = policy
        .checks
        .iter()
        .map(|check| check.id.as_str())
        .collect::<Vec<_>>();
    if configured != required {
        return Err(RepairContractError::new(
            "repair-check policy IDs do not exactly match Change Receipt checks",
        ));
    }
    if inputs.sources.len() != change_receipt.sources.len()
        || inputs.sources.keys().map(String::as_str).ne(change_receipt
            .sources
            .iter()
            .map(|source| source.file.as_str()))
    {
        return Err(RepairContractError::new(
            "verification sources do not exactly match Change Receipt sources",
        ));
    }
    validate_repair_after_sources(change_receipt, inputs.sources)?;
    validate_repair_verification_supplementary_inputs(
        policy,
        inputs.sources,
        inputs.supplementary,
    )?;
    let covered = policy
        .checks
        .iter()
        .filter(|check| {
            !matches!(
                check.kind,
                RepairCheckKind::TestSuiteEvidence | RepairCheckKind::BrowserEvidence
            )
        })
        .map(|check| check.source.as_str())
        .collect::<BTreeSet<_>>();
    if covered.len() != change_receipt.sources.len()
        || change_receipt
            .sources
            .iter()
            .any(|source| !covered.contains(source.file.as_str()))
    {
        return Err(RepairContractError::new(
            "repair-check policy must cover every changed source with a source-specific built-in check",
        ));
    }

    Ok(())
}

pub(crate) fn validate_repair_after_sources(
    change_receipt: &RepairChangeReceipt,
    sources: &BTreeMap<String, Vec<u8>>,
) -> Result<(), RepairContractError> {
    for transition in &change_receipt.sources {
        let bytes = sources.get(&transition.file).ok_or_else(|| {
            RepairContractError::new(format!("missing verification source `{}`", transition.file))
        })?;
        if bytes.len() != transition.after_bytes || sha256_hex(bytes) != transition.after_sha256 {
            return Err(RepairContractError::new(format!(
                "verification source `{}` does not match the Change Receipt after state",
                transition.file
            )));
        }
    }
    Ok(())
}

fn validate_repair_verification_supplementary_inputs(
    policy: &RepairCheckPolicy,
    sources: &BTreeMap<String, Vec<u8>>,
    inputs: &BTreeMap<String, Vec<u8>>,
) -> Result<(), RepairContractError> {
    if inputs.keys().any(|file| sources.contains_key(file)) {
        return Err(RepairContractError::new(
            "supplementary verification inputs cannot duplicate changed sources",
        ));
    }
    let mut expected = BTreeMap::<String, RepairArtifactIdentity>::new();
    for definition in &policy.checks {
        for identity in [
            definition.budget_policy.as_ref(),
            definition.test_evidence.as_ref(),
            definition.workspace_manifest.as_ref(),
            definition.lockfile.as_ref(),
            definition.browser_evidence_file.as_ref(),
        ]
        .into_iter()
        .flatten()
        .chain(definition.browser_profile_inputs.iter())
        {
            if !sources.contains_key(&identity.file) {
                match expected.insert(identity.file.clone(), identity.clone()) {
                    Some(previous) if previous != *identity => {
                        return Err(RepairContractError::new(format!(
                            "supplementary input `{}` has conflicting identities",
                            identity.file
                        )));
                    }
                    _ => {}
                }
            }
            let bytes = sources
                .get(&identity.file)
                .or_else(|| inputs.get(&identity.file))
                .ok_or_else(|| {
                    RepairContractError::new(format!(
                        "missing supplementary verification input `{}`",
                        identity.file
                    ))
                })?;
            if bytes.len() != identity.bytes || sha256_hex(bytes) != identity.sha256 {
                return Err(RepairContractError::new(format!(
                    "supplementary verification input `{}` does not match its policy identity",
                    identity.file
                )));
            }
        }
    }
    if inputs.len() != expected.len()
        || inputs
            .keys()
            .map(String::as_str)
            .ne(expected.keys().map(String::as_str))
    {
        return Err(RepairContractError::new(
            "supplementary verification inputs do not exactly match the check policy",
        ));
    }
    Ok(())
}

pub(crate) fn execute_repair_check(
    definition: &RepairCheckDefinition,
    source: &[u8],
    sources: &BTreeMap<String, Vec<u8>>,
    inputs: &BTreeMap<String, Vec<u8>>,
    change_receipt_sha256: &str,
) -> Result<(RepairVerificationCheck, Vec<u8>), RepairContractError> {
    let (document, passed) = match definition.kind {
        RepairCheckKind::StandardCssAudit => {
            let css = std::str::from_utf8(source).map_err(|error| {
                RepairContractError::new(format!(
                    "verification source `{}` is not UTF-8: {error}",
                    definition.source
                ))
            })?;
            let outcome =
                audit_standard_css(&definition.source, css, definition.compatibility_profile)
                    .map_err(|error| {
                        RepairContractError::new(format!(
                            "check `{}` could not execute: {error}",
                            definition.id
                        ))
                    })?;
            (outcome.document().clone(), outcome.passed())
        }
        RepairCheckKind::TokenGraphIntegrity => {
            execute_token_graph_integrity_check(definition, source)?
        }
        RepairCheckKind::CssBudgetAudit => {
            execute_css_budget_audit_check(definition, source, sources, inputs)?
        }
        RepairCheckKind::TestSuiteEvidence => execute_test_suite_evidence_check(
            definition,
            source,
            sources,
            inputs,
            change_receipt_sha256,
        )?,
        RepairCheckKind::BrowserEvidence => execute_browser_evidence_check(
            definition,
            source,
            sources,
            inputs,
            change_receipt_sha256,
        )?,
    };
    let finding_count = document.findings().len();
    let finding_document = document
        .to_json_pretty()
        .map_err(|error| {
            RepairContractError::new(format!(
                "check `{}` cannot serialize findings: {error}",
                definition.id
            ))
        })?
        .into_bytes();
    Ok((
        RepairVerificationCheck {
            id: definition.id.clone(),
            kind: definition.kind,
            status: if passed {
                RepairVerificationCheckStatus::Passed
            } else {
                RepairVerificationCheckStatus::Failed
            },
            source: RepairArtifactIdentity {
                file: definition.source.clone(),
                bytes: source.len(),
                sha256: sha256_hex(source),
            },
            compatibility_profile: definition.compatibility_profile,
            budget_policy: definition.budget_policy.clone(),
            test_evidence: definition.test_evidence.clone(),
            browser_evidence_file: definition.browser_evidence_file.clone(),
            browser_profile_inputs: definition.browser_profile_inputs.clone(),
            finding_document_bytes: finding_document.len(),
            finding_document_sha256: sha256_hex(&finding_document),
            finding_count,
        },
        finding_document,
    ))
}

fn execute_css_budget_audit_check(
    definition: &RepairCheckDefinition,
    source: &[u8],
    sources: &BTreeMap<String, Vec<u8>>,
    inputs: &BTreeMap<String, Vec<u8>>,
) -> Result<(FindingDocument, bool), RepairContractError> {
    let css = std::str::from_utf8(source).map_err(|error| {
        RepairContractError::new(format!(
            "verification source `{}` is not UTF-8: {error}",
            definition.source
        ))
    })?;
    let identity = definition.budget_policy.as_ref().ok_or_else(|| {
        RepairContractError::new(format!(
            "check `{}` has no budget policy identity",
            definition.id
        ))
    })?;
    let budget_bytes = sources
        .get(&identity.file)
        .or_else(|| inputs.get(&identity.file))
        .ok_or_else(|| {
            RepairContractError::new(format!(
                "check `{}` cannot load budget policy `{}`",
                definition.id, identity.file
            ))
        })?;
    let budget_policy = parse_budget_policy(budget_bytes).map_err(|error| {
        RepairContractError::new(format!(
            "check `{}` has an invalid budget policy: {error}",
            definition.id
        ))
    })?;
    let canonical_policy = budget_policy.to_json_pretty().map_err(|error| {
        RepairContractError::new(format!(
            "check `{}` cannot canonicalize its budget policy: {error}",
            definition.id
        ))
    })?;
    if canonical_policy.as_bytes() != budget_bytes {
        return Err(RepairContractError::new(format!(
            "check `{}` budget policy bytes are not canonical pretty JSON",
            definition.id
        )));
    }
    let outcome = audit_standard_css_with_budgets(
        &definition.source,
        css,
        definition.compatibility_profile,
        Some(&budget_policy),
        &definition.budget_subjects,
    )
    .map_err(|error| {
        RepairContractError::new(format!(
            "check `{}` could not execute: {error}",
            definition.id
        ))
    })?;
    Ok((outcome.document().clone(), outcome.passed()))
}

fn execute_test_suite_evidence_check(
    definition: &RepairCheckDefinition,
    source: &[u8],
    sources: &BTreeMap<String, Vec<u8>>,
    inputs: &BTreeMap<String, Vec<u8>>,
    change_receipt_sha256: &str,
) -> Result<(FindingDocument, bool), RepairContractError> {
    let identity = definition.test_evidence.as_ref().ok_or_else(|| {
        RepairContractError::new(format!(
            "check `{}` has no test evidence identity",
            definition.id
        ))
    })?;
    let evidence_bytes = sources
        .get(&identity.file)
        .or_else(|| inputs.get(&identity.file))
        .ok_or_else(|| {
            RepairContractError::new(format!(
                "check `{}` cannot load test evidence `{}`",
                definition.id, identity.file
            ))
        })?;
    let evidence = parse_repair_test_evidence(evidence_bytes).map_err(|error| {
        RepairContractError::new(format!(
            "check `{}` has invalid test evidence: {error}",
            definition.id
        ))
    })?;
    if evidence.change_receipt_sha256 != change_receipt_sha256 {
        return Err(RepairContractError::new(format!(
            "check `{}` test evidence belongs to another Change Receipt",
            definition.id
        )));
    }
    if evidence.check_id != definition.id {
        return Err(RepairContractError::new(format!(
            "check `{}` test evidence names check `{}`",
            definition.id, evidence.check_id
        )));
    }
    if definition.workspace_manifest.as_ref() != Some(&evidence.workspace_manifest)
        || definition.lockfile.as_ref() != Some(&evidence.lockfile)
    {
        return Err(RepairContractError::new(format!(
            "check `{}` test evidence workspace inputs differ from the check policy",
            definition.id
        )));
    }
    let source_range = FindingSource::new(&definition.source, 0, source.len())
        .map_err(|error| RepairContractError::new(error.to_string()))?;
    let passed = evidence.result == RepairTestEvidenceResult::Passed;
    let (code, severity, summary, condition, reason) = if passed {
        (
            "PCSS-TEST-000",
            FindingSeverity::Info,
            "fixed Rust workspace test profile passed",
            "passed-evidence",
            "the verifier validated canonical fixed-runner evidence and exact workspace inputs",
        )
    } else {
        (
            "PCSS-TEST-001",
            FindingSeverity::Error,
            "fixed Rust workspace test profile failed",
            "failed-evidence",
            "the canonical fixed-runner evidence records a nonzero test exit code",
        )
    };
    let finding = Finding::new(
        code,
        "test-suite",
        severity,
        summary,
        FindingVerification::Verified,
        FindingCause::new("test-suite.rust-workspace", condition, reason)
            .map_err(|error| RepairContractError::new(error.to_string()))?,
    )
    .map_err(|error| RepairContractError::new(error.to_string()))?
    .with_source(source_range)
    .map_err(|error| RepairContractError::new(error.to_string()))?
    .with_context("change-receipt-sha256", change_receipt_sha256)
    .and_then(|finding| finding.with_context("test-evidence-sha256", &identity.sha256))
    .and_then(|finding| finding.with_context("exit-code", evidence.exit_code.to_string()))
    .and_then(|finding| finding.with_context("profile", "rust-workspace-all-targets"))
    .map_err(|error| RepairContractError::new(error.to_string()))?;
    let document = FindingDocument::new(
        FindingTool::new("pliego-css-agent", env!("CARGO_PKG_VERSION"))
            .map_err(|error| RepairContractError::new(error.to_string()))?,
        "verify-test-suite-evidence",
        vec![finding],
    )
    .map_err(|error| RepairContractError::new(error.to_string()))?;
    Ok((document, passed))
}

fn execute_browser_evidence_check(
    definition: &RepairCheckDefinition,
    source: &[u8],
    sources: &BTreeMap<String, Vec<u8>>,
    inputs: &BTreeMap<String, Vec<u8>>,
    change_receipt_sha256: &str,
) -> Result<(FindingDocument, bool), RepairContractError> {
    let identity = definition.browser_evidence_file.as_ref().ok_or_else(|| {
        RepairContractError::new(format!(
            "check `{}` has no browser evidence identity",
            definition.id
        ))
    })?;
    let evidence_bytes = sources
        .get(&identity.file)
        .or_else(|| inputs.get(&identity.file))
        .ok_or_else(|| {
            RepairContractError::new(format!(
                "check `{}` cannot load browser evidence `{}`",
                definition.id, identity.file
            ))
        })?;
    let evidence = parse_repair_browser_evidence(evidence_bytes).map_err(|error| {
        RepairContractError::new(format!(
            "check `{}` has invalid browser evidence: {error}",
            definition.id
        ))
    })?;
    if evidence.change_receipt_sha256 != change_receipt_sha256 {
        return Err(RepairContractError::new(format!(
            "check `{}` browser evidence belongs to another Change Receipt",
            definition.id
        )));
    }
    if evidence.check_id != definition.id {
        return Err(RepairContractError::new(format!(
            "check `{}` browser evidence names check `{}`",
            definition.id, evidence.check_id
        )));
    }
    if evidence.profile_inputs != definition.browser_profile_inputs {
        return Err(RepairContractError::new(format!(
            "check `{}` browser profile inputs differ from the check policy",
            definition.id
        )));
    }
    let source_range = FindingSource::new(&definition.source, 0, source.len())
        .map_err(|error| RepairContractError::new(error.to_string()))?;
    let passed = evidence.result == RepairBrowserEvidenceResult::Passed;
    let (code, severity, summary, condition, reason) = if passed {
        (
            "PCSS-BROWSER-000",
            FindingSeverity::Info,
            "fixed PliegoRS Chromium profile passed",
            "passed-evidence",
            "the verifier validated canonical CDP evidence, object identity, runtime state, and exact profile inputs",
        )
    } else {
        (
            "PCSS-BROWSER-001",
            FindingSeverity::Error,
            "fixed PliegoRS Chromium profile failed",
            "failed-evidence",
            "the canonical CDP evidence records a failed fixed-profile observation or exit status",
        )
    };
    let finding = Finding::new(
        code,
        "browser-evidence",
        severity,
        summary,
        FindingVerification::Verified,
        FindingCause::new("browser.pliegors-visit-counter", condition, reason)
            .map_err(|error| RepairContractError::new(error.to_string()))?,
    )
    .map_err(|error| RepairContractError::new(error.to_string()))?
    .with_source(source_range)
    .map_err(|error| RepairContractError::new(error.to_string()))?
    .with_context("change-receipt-sha256", change_receipt_sha256)
    .and_then(|finding| finding.with_context("browser-evidence-sha256", &identity.sha256))
    .and_then(|finding| finding.with_context("browser-product", &evidence.browser.product))
    .and_then(|finding| finding.with_context("exit-code", evidence.exit_code.to_string()))
    .and_then(|finding| finding.with_context("profile", "pliegors-visit-counter-chromium-cdp"))
    .map_err(|error| RepairContractError::new(error.to_string()))?;
    let document = FindingDocument::new(
        FindingTool::new("pliego-css-agent", env!("CARGO_PKG_VERSION"))
            .map_err(|error| RepairContractError::new(error.to_string()))?,
        "verify-browser-evidence",
        vec![finding],
    )
    .map_err(|error| RepairContractError::new(error.to_string()))?;
    Ok((document, passed))
}

fn execute_token_graph_integrity_check(
    definition: &RepairCheckDefinition,
    source: &[u8],
) -> Result<(FindingDocument, bool), RepairContractError> {
    let source_range = FindingSource::new(&definition.source, 0, source.len())
        .map_err(|error| RepairContractError::new(error.to_string()))?;
    let (finding, passed) = match parse_token_graph(source) {
        Ok(graph) => {
            let finding = Finding::new(
                "PCSS-TOKEN-000",
                "token-graph",
                FindingSeverity::Info,
                "token graph is canonical and internally consistent",
                FindingVerification::Verified,
                FindingCause::new(
                    "token-graph.integrity",
                    "canonical-graph",
                    "the closed token-graph parser validated exact canonical bytes and relationships",
                )
                .map_err(|error| RepairContractError::new(error.to_string()))?,
            )
            .map_err(|error| RepairContractError::new(error.to_string()))?
            .with_source(source_range)
            .map_err(|error| RepairContractError::new(error.to_string()))?
            .with_context("source-sha256", sha256_hex(source))
            .and_then(|finding| finding.with_context("aliases", graph.aliases().to_string()))
            .and_then(|finding| {
                finding.with_context("derived-values", graph.derived_values().to_string())
            })
            .and_then(|finding| {
                finding.with_context("deprecations", graph.deprecations().to_string())
            })
            .map_err(|error| RepairContractError::new(error.to_string()))?;
            (finding, true)
        }
        Err(error) => {
            let reason = bounded_verification_reason(&error.to_string());
            let finding = Finding::new(
                "PCSS-TOKEN-001",
                "token-graph",
                FindingSeverity::Error,
                "token graph failed canonical integrity validation",
                FindingVerification::Verified,
                FindingCause::new("token-graph.integrity", "invalid-graph", reason)
                    .map_err(|error| RepairContractError::new(error.to_string()))?,
            )
            .map_err(|error| RepairContractError::new(error.to_string()))?
            .with_source(source_range)
            .map_err(|error| RepairContractError::new(error.to_string()))?
            .with_context("source-sha256", sha256_hex(source))
            .map_err(|error| RepairContractError::new(error.to_string()))?;
            (finding, false)
        }
    };
    let document = FindingDocument::new(
        FindingTool::new("pliego-css-agent", env!("CARGO_PKG_VERSION"))
            .map_err(|error| RepairContractError::new(error.to_string()))?,
        "verify-token-graph",
        vec![finding],
    )
    .map_err(|error| RepairContractError::new(error.to_string()))?;
    Ok((document, passed))
}

pub(crate) fn bounded_verification_reason(reason: &str) -> String {
    const MAX_REASON_BYTES: usize = 4_096;
    let mut bounded = String::new();
    for character in reason.chars() {
        let character = if character.is_control() {
            ' '
        } else {
            character
        };
        if bounded.len() + character.len_utf8() > MAX_REASON_BYTES {
            break;
        }
        bounded.push(character);
    }
    if bounded.is_empty() {
        "token graph validation failed".into()
    } else {
        bounded
    }
}
