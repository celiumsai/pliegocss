use std::collections::BTreeMap;

use pliego_css_build::artifacts::{parse_finding_document, sha256_hex};

use super::validation::validate_logical_path;
use super::{
    PreparedRepairApplication, REPAIR_CHANGE_RECEIPT_SCHEMA_VERSION, REPAIR_DRY_RUN_SCHEMA_VERSION,
    RepairChangeReceipt, RepairChangeState, RepairContractError, RepairDryRunReport,
    RepairDryRunState, RepairPlanDocument, RepairReceiptCheck, apply_file_edits,
    receipt_payload_sha256,
};

/// Verifies the original finding document and current sources without mutating them.
///
/// # Errors
///
/// Returns [`RepairContractError`] for finding-document drift, missing/unexpected files, stale
/// hashes, a partially applied state, or when applying ready edits in memory does not produce each
/// planned after hash.
pub fn verify_repair_plan(
    plan: &RepairPlanDocument,
    finding_file: &str,
    finding_bytes: &[u8],
    sources: &BTreeMap<String, Vec<u8>>,
) -> Result<RepairDryRunReport, RepairContractError> {
    plan.validate()?;
    validate_logical_path("findingDocument.file", finding_file)?;
    parse_finding_document(finding_bytes)
        .map_err(|error| RepairContractError::new(format!("invalid finding document: {error}")))?;
    if plan.finding_document.file != finding_file
        || plan.finding_document.bytes != finding_bytes.len()
        || plan.finding_document.sha256 != sha256_hex(finding_bytes)
    {
        return Err(RepairContractError::new(
            "finding document does not match the exact artifact bound by the repair plan",
        ));
    }
    if sources.len() != plan.sources.len() {
        return Err(RepairContractError::new(
            "source snapshot set must exactly equal plan.sources",
        ));
    }
    let mut before_count = 0usize;
    let mut after_count = 0usize;
    for transition in &plan.sources {
        let source = sources.get(&transition.file).ok_or_else(|| {
            RepairContractError::new(format!("missing source snapshot `{}`", transition.file))
        })?;
        let digest = sha256_hex(source);
        if source.len() == transition.before_bytes && digest == transition.before_sha256 {
            let projected = apply_file_edits(&transition.file, source, &plan.edits)?;
            if projected.len() != transition.after_bytes
                || sha256_hex(&projected) != transition.after_sha256
            {
                return Err(RepairContractError::new(format!(
                    "plan edits do not produce the bound after snapshot for `{}`",
                    transition.file
                )));
            }
            before_count += 1;
        } else if source.len() == transition.after_bytes && digest == transition.after_sha256 {
            after_count += 1;
        } else {
            return Err(RepairContractError::new(format!(
                "source `{}` matches neither the plan before nor after snapshot",
                transition.file
            )));
        }
    }
    let state = match (before_count, after_count) {
        (before, 0) if before == plan.sources.len() => RepairDryRunState::Ready,
        (0, after) if after == plan.sources.len() => RepairDryRunState::AlreadyApplied,
        _ => {
            return Err(RepairContractError::new(
                "source snapshots are partially applied; schema 1.0.0 requires an atomic state",
            ));
        }
    };
    Ok(RepairDryRunReport {
        schema_version: REPAIR_DRY_RUN_SCHEMA_VERSION.into(),
        plan_sha256: plan.plan_sha256.clone(),
        state,
        files: plan.change_summary.files,
        edits: plan.change_summary.edits,
        patch_sha256: plan.patch.sha256.clone(),
    })
}

/// Prepares exact after bytes and a change-only receipt for an atomic publisher.
///
/// The authorization is deliberately narrow: it must repeat `sha256:<planSha256>` exactly. It
/// prevents accidental or ambiguous plan selection but is not a signature or proof of identity.
/// Required checks and browser evidence remain explicitly pending in the returned receipt.
///
/// # Errors
///
/// Returns [`RepairContractError`] for authorization mismatch or any plan, finding, source, or
/// projected-after drift.
pub fn prepare_repair_application(
    plan: &RepairPlanDocument,
    finding_file: &str,
    finding_bytes: &[u8],
    sources: &BTreeMap<String, Vec<u8>>,
    authorization: &str,
) -> Result<PreparedRepairApplication, RepairContractError> {
    let expected_authorization = format!("sha256:{}", plan.plan_sha256);
    if authorization != expected_authorization {
        return Err(RepairContractError::new(format!(
            "apply authorization must exactly equal `{expected_authorization}`"
        )));
    }
    let report = verify_repair_plan(plan, finding_file, finding_bytes, sources)?;
    let (state, after_sources) = match report.state {
        RepairDryRunState::Ready => {
            let mut after = BTreeMap::new();
            for transition in &plan.sources {
                let source = sources.get(&transition.file).ok_or_else(|| {
                    RepairContractError::new(format!(
                        "missing verified source snapshot `{}`",
                        transition.file
                    ))
                })?;
                let projected = apply_file_edits(&transition.file, source, &plan.edits)?;
                after.insert(transition.file.clone(), projected);
            }
            (RepairChangeState::Applied, after)
        }
        RepairDryRunState::AlreadyApplied => (RepairChangeState::AlreadyApplied, sources.clone()),
    };
    let checks = plan
        .required_checks
        .iter()
        .map(|id| RepairReceiptCheck {
            id: id.clone(),
            status: "not-run".into(),
        })
        .collect();
    let mut receipt = RepairChangeReceipt {
        schema_version: REPAIR_CHANGE_RECEIPT_SCHEMA_VERSION.into(),
        receipt_sha256: "0".repeat(64),
        plan_sha256: plan.plan_sha256.clone(),
        patch_sha256: plan.patch.sha256.clone(),
        authorization: authorization.into(),
        state,
        result: "checks-pending".into(),
        finding_document: plan.finding_document.clone(),
        change_summary: plan.change_summary,
        sources: plan.sources.clone(),
        checks,
        browser_evidence: "not-collected".into(),
    };
    receipt.receipt_sha256 = receipt_payload_sha256(&receipt)?;
    receipt.validate()?;
    Ok(PreparedRepairApplication {
        receipt,
        sources: after_sources,
    })
}
