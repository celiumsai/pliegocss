use super::validation::validate_sha256;
use super::{
    MAX_CHECKS, MAX_DOCUMENT_BYTES, MAX_FILES, REPAIR_CHANGE_RECEIPT_SCHEMA_VERSION,
    RepairArtifactIdentity, RepairChangeSummary, RepairContractError, RepairSourceTransition,
    validate_checks, validate_source_order,
};
use pliego_css_build::artifacts::sha256_hex;
use serde::{Deserialize, Serialize};

/// Read-only source state proved against a repair plan.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepairDryRunState {
    /// Every source equals the plan's exact before snapshot.
    Ready,
    /// Every source equals the plan's exact after snapshot.
    AlreadyApplied,
}

impl RepairDryRunState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::AlreadyApplied => "already-applied",
        }
    }
}

/// Canonical report produced without mutating source files.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairDryRunReport {
    pub(crate) schema_version: String,
    pub(crate) plan_sha256: String,
    pub(crate) state: RepairDryRunState,
    pub(crate) files: u32,
    pub(crate) edits: u32,
    pub(crate) patch_sha256: String,
}

impl RepairDryRunReport {
    /// Serializes the report with one trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`RepairContractError`] only if serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, RepairContractError> {
        let mut output = serde_json::to_string_pretty(self).map_err(|error| {
            RepairContractError::new(format!("cannot serialize dry-run report: {error}"))
        })?;
        output.push('\n');
        Ok(output)
    }

    /// Renders a concise human report.
    #[must_use]
    pub fn to_human(&self) -> String {
        format!(
            "Dry-run: {}\nPlan: sha256:{}\nChanges: {} file(s), {} edit(s)\nPatch: sha256:{}\n",
            self.state.as_str(),
            self.plan_sha256,
            self.files,
            self.edits,
            self.patch_sha256
        )
    }
}

/// Outcome recorded by an atomically published Change Receipt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepairChangeState {
    /// The exact before snapshots were replaced by their planned after snapshots.
    Applied,
    /// Sources already matched every after snapshot; no source bytes were changed.
    AlreadyApplied,
}

/// One required check that has not yet been executed by the change-only apply boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RepairReceiptCheck {
    pub(crate) id: String,
    pub(crate) status: String,
}

/// Deterministic receipt for an explicitly authorized source transition.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairChangeReceipt {
    pub(crate) schema_version: String,
    pub(crate) receipt_sha256: String,
    pub(crate) plan_sha256: String,
    pub(crate) patch_sha256: String,
    pub(crate) authorization: String,
    pub(crate) state: RepairChangeState,
    pub(crate) result: String,
    pub(crate) finding_document: RepairArtifactIdentity,
    pub(crate) change_summary: RepairChangeSummary,
    pub(crate) sources: Vec<RepairSourceTransition>,
    pub(crate) checks: Vec<RepairReceiptCheck>,
    pub(crate) browser_evidence: String,
}

impl RepairChangeReceipt {
    /// Returns the exact plan hash authorized by this receipt.
    #[must_use]
    pub fn plan_sha256(&self) -> &str {
        &self.plan_sha256
    }

    /// Returns whether source bytes were changed or were already at the after state.
    #[must_use]
    pub const fn state(&self) -> RepairChangeState {
        self.state
    }

    /// Serializes the validated receipt with one trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`RepairContractError`] when validation or serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, RepairContractError> {
        self.validate()?;
        let mut output = serde_json::to_string_pretty(self).map_err(|error| {
            RepairContractError::new(format!("cannot serialize change receipt: {error}"))
        })?;
        output.push('\n');
        if output.len() > MAX_DOCUMENT_BYTES {
            return Err(RepairContractError::new("change receipt exceeds 16 MiB"));
        }
        Ok(output)
    }

    /// Renders the honest change-only result without claiming checks passed.
    #[must_use]
    pub fn to_human(&self) -> String {
        format!(
            "Change: {}\nPlan: sha256:{}\nReceipt: sha256:{}\nResult: {}\nChecks: {} pending\nBrowser evidence: {}\n",
            match self.state {
                RepairChangeState::Applied => "applied",
                RepairChangeState::AlreadyApplied => "already-applied",
            },
            self.plan_sha256,
            self.receipt_sha256,
            self.result,
            self.checks.len(),
            self.browser_evidence,
        )
    }

    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
        if self.schema_version != REPAIR_CHANGE_RECEIPT_SCHEMA_VERSION {
            return Err(RepairContractError::new(format!(
                "unsupported change receipt schema `{}`; expected {REPAIR_CHANGE_RECEIPT_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        validate_sha256("receipt.receiptSha256", &self.receipt_sha256)?;
        validate_sha256("receipt.planSha256", &self.plan_sha256)?;
        validate_sha256("receipt.patchSha256", &self.patch_sha256)?;
        if self.authorization != format!("sha256:{}", self.plan_sha256) {
            return Err(RepairContractError::new(
                "receipt.authorization must exactly equal `sha256:<planSha256>`",
            ));
        }
        if self.result != "checks-pending" {
            return Err(RepairContractError::new(
                "receipt.result must be `checks-pending` in schema 1.0.0",
            ));
        }
        self.finding_document.validate("findingDocument")?;
        if self.sources.is_empty() || self.sources.len() > MAX_FILES {
            return Err(RepairContractError::new(
                "receipt.sources must contain between 1 and 65,535 files",
            ));
        }
        for source in &self.sources {
            source.validate()?;
        }
        validate_source_order(&self.sources)?;
        if self.checks.is_empty() || self.checks.len() > MAX_CHECKS {
            return Err(RepairContractError::new(
                "receipt.checks must contain between 1 and 256 checks",
            ));
        }
        let check_ids = self
            .checks
            .iter()
            .map(|check| {
                if check.status != "not-run" {
                    return Err(RepairContractError::new(
                        "receipt check status must be `not-run`",
                    ));
                }
                Ok(check.id.clone())
            })
            .collect::<Result<Vec<_>, _>>()?;
        validate_checks(&check_ids)?;
        if self.browser_evidence != "not-collected" {
            return Err(RepairContractError::new(
                "receipt.browserEvidence must be `not-collected` in schema 1.0.0",
            ));
        }
        let expected = receipt_payload_sha256(self)?;
        if expected != self.receipt_sha256 {
            return Err(RepairContractError::new(format!(
                "receipt.receiptSha256 `{}` does not match `{expected}`",
                self.receipt_sha256
            )));
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RepairReceiptPayload<'a> {
    pub(crate) schema_version: &'a str,
    pub(crate) plan_sha256: &'a str,
    pub(crate) patch_sha256: &'a str,
    pub(crate) authorization: &'a str,
    pub(crate) state: RepairChangeState,
    pub(crate) result: &'a str,
    pub(crate) finding_document: &'a RepairArtifactIdentity,
    pub(crate) change_summary: RepairChangeSummary,
    pub(crate) sources: &'a [RepairSourceTransition],
    pub(crate) checks: &'a [RepairReceiptCheck],
    pub(crate) browser_evidence: &'a str,
}

pub(crate) fn receipt_payload_sha256(
    receipt: &RepairChangeReceipt,
) -> Result<String, RepairContractError> {
    let payload = RepairReceiptPayload {
        schema_version: &receipt.schema_version,
        plan_sha256: &receipt.plan_sha256,
        patch_sha256: &receipt.patch_sha256,
        authorization: &receipt.authorization,
        state: receipt.state,
        result: &receipt.result,
        finding_document: &receipt.finding_document,
        change_summary: receipt.change_summary,
        sources: &receipt.sources,
        checks: &receipt.checks,
        browser_evidence: &receipt.browser_evidence,
    };
    serde_json::to_vec(&payload)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(|error| RepairContractError::new(format!("cannot hash change receipt: {error}")))
}

/// Parses and validates a canonical Change Receipt.
///
/// # Errors
///
/// Returns [`RepairContractError`] for malformed JSON, drift, noncanonical collections, or an
/// unsupported claim.
pub fn parse_repair_change_receipt(
    source: &[u8],
) -> Result<RepairChangeReceipt, RepairContractError> {
    if source.len() > MAX_DOCUMENT_BYTES {
        return Err(RepairContractError::new("change receipt exceeds 16 MiB"));
    }
    let receipt: RepairChangeReceipt = serde_json::from_slice(source).map_err(|error| {
        RepairContractError::new(format!("invalid change receipt JSON: {error}"))
    })?;
    receipt.validate()?;
    if receipt.to_json_pretty()?.as_bytes() != source {
        return Err(RepairContractError::new(
            "change receipt bytes are not canonical pretty JSON",
        ));
    }
    Ok(receipt)
}
