use super::validation::{validate_dotted_id, validate_sha256};
use super::{
    CSS_BUDGET_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION,
    LEGACY_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION, MAX_CHECKS, MAX_DOCUMENT_BYTES,
    REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION, RepairArtifactIdentity, RepairCheckKind,
    RepairContractError, TEST_EVIDENCE_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION,
    TOKEN_GRAPH_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION, validate_browser_profile_inputs,
};
use pliego_css_build::artifacts::{CompatibilityProfile, sha256_hex};
use serde::{Deserialize, Serialize};

/// Result of one executed built-in check.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RepairVerificationCheckStatus {
    /// The built-in check completed and its enforced policy passed.
    Passed,
    /// The built-in check completed and its enforced policy failed.
    Failed,
}

/// Derived result of the complete verification policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RepairVerificationResult {
    /// Every check passed and browser evidence was not required or exactly passed.
    Passed,
    /// At least one built-in check completed with a failing policy result.
    Failed,
    /// Configured checks passed but required browser evidence was not collected.
    Blocked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RepairVerificationCheck {
    pub(crate) id: String,
    pub(crate) kind: RepairCheckKind,
    pub(crate) status: RepairVerificationCheckStatus,
    pub(crate) source: RepairArtifactIdentity,
    pub(crate) compatibility_profile: CompatibilityProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) budget_policy: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) test_evidence: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) browser_evidence_file: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) browser_profile_inputs: Vec<RepairArtifactIdentity>,
    pub(crate) finding_document_bytes: usize,
    pub(crate) finding_document_sha256: String,
    pub(crate) finding_count: usize,
}

impl RepairVerificationCheck {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn validate(&self, schema_version: &str) -> Result<(), RepairContractError> {
        validate_dotted_id("verificationReceipt.checks.id", &self.id)?;
        self.source.validate("verificationReceipt.checks.source")?;
        match self.kind {
            RepairCheckKind::StandardCssAudit => {
                if self.compatibility_profile == CompatibilityProfile::None {
                    return Err(RepairContractError::new(
                        "standard-css-audit verification compatibility cannot be `none`",
                    ));
                }
            }
            RepairCheckKind::TokenGraphIntegrity => {
                if schema_version == LEGACY_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION {
                    return Err(RepairContractError::new(
                        "Verification Receipt schema 1.0.0 does not support token-graph-integrity",
                    ));
                }
                if self.compatibility_profile != CompatibilityProfile::None {
                    return Err(RepairContractError::new(
                        "token-graph-integrity verification compatibility must be `none`",
                    ));
                }
                if self.budget_policy.is_some()
                    || self.test_evidence.is_some()
                    || self.browser_evidence_file.is_some()
                    || !self.browser_profile_inputs.is_empty()
                {
                    return Err(RepairContractError::new(
                        "token-graph-integrity verification cannot record supplementary evidence",
                    ));
                }
            }
            RepairCheckKind::CssBudgetAudit => {
                if !matches!(
                    schema_version,
                    CSS_BUDGET_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION
                        | TEST_EVIDENCE_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION
                        | REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION
                ) {
                    return Err(RepairContractError::new(
                        "css-budget-audit requires Verification Receipt schema 1.2.0, 1.3.0, or 1.4.0",
                    ));
                }
                if self.compatibility_profile == CompatibilityProfile::None {
                    return Err(RepairContractError::new(
                        "css-budget-audit verification compatibility cannot be `none`",
                    ));
                }
                let budget_policy = self.budget_policy.as_ref().ok_or_else(|| {
                    RepairContractError::new(
                        "css-budget-audit verification requires a budget policy identity",
                    )
                })?;
                budget_policy.validate("verificationReceipt.checks.budgetPolicy")?;
                if budget_policy.bytes == 0 {
                    return Err(RepairContractError::new(
                        "verificationReceipt.checks.budgetPolicy.bytes must be greater than zero",
                    ));
                }
                if budget_policy.file == self.source.file {
                    return Err(RepairContractError::new(
                        "css-budget-audit verification source and budget policy must be distinct files",
                    ));
                }
                if self.test_evidence.is_some()
                    || self.browser_evidence_file.is_some()
                    || !self.browser_profile_inputs.is_empty()
                {
                    return Err(RepairContractError::new(
                        "css-budget-audit verification cannot record test or browser evidence",
                    ));
                }
            }
            RepairCheckKind::TestSuiteEvidence => {
                if !matches!(
                    schema_version,
                    TEST_EVIDENCE_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION
                        | REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION
                ) {
                    return Err(RepairContractError::new(
                        "test-suite-evidence requires Verification Receipt schema 1.3.0 or 1.4.0",
                    ));
                }
                if self.compatibility_profile != CompatibilityProfile::None {
                    return Err(RepairContractError::new(
                        "test-suite-evidence verification compatibility must be `none`",
                    ));
                }
                if self.budget_policy.is_some()
                    || self.browser_evidence_file.is_some()
                    || !self.browser_profile_inputs.is_empty()
                {
                    return Err(RepairContractError::new(
                        "test-suite-evidence verification cannot record budget or browser evidence",
                    ));
                }
                let evidence = self.test_evidence.as_ref().ok_or_else(|| {
                    RepairContractError::new(
                        "test-suite-evidence verification requires a test evidence identity",
                    )
                })?;
                evidence.validate("verificationReceipt.checks.testEvidence")?;
                if evidence.bytes == 0 || evidence.file == self.source.file {
                    return Err(RepairContractError::new(
                        "verification test evidence must be nonempty and distinct from its source",
                    ));
                }
            }
            RepairCheckKind::BrowserEvidence => {
                if schema_version != REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION {
                    return Err(RepairContractError::new(
                        "browser-evidence requires Verification Receipt schema 1.4.0",
                    ));
                }
                if self.compatibility_profile != CompatibilityProfile::None {
                    return Err(RepairContractError::new(
                        "browser-evidence verification compatibility must be `none`",
                    ));
                }
                if self.budget_policy.is_some() || self.test_evidence.is_some() {
                    return Err(RepairContractError::new(
                        "browser-evidence verification cannot record budget or test evidence",
                    ));
                }
                let evidence = self.browser_evidence_file.as_ref().ok_or_else(|| {
                    RepairContractError::new(
                        "browser-evidence verification requires a browser evidence identity",
                    )
                })?;
                evidence.validate("verificationReceipt.checks.browserEvidenceFile")?;
                if evidence.bytes == 0 || evidence.file == self.source.file {
                    return Err(RepairContractError::new(
                        "verification browser evidence must be nonempty and distinct from its source",
                    ));
                }
                validate_browser_profile_inputs(&self.browser_profile_inputs)?;
            }
        }
        if self.kind == RepairCheckKind::StandardCssAudit
            && (self.budget_policy.is_some()
                || self.test_evidence.is_some()
                || self.browser_evidence_file.is_some()
                || !self.browser_profile_inputs.is_empty())
        {
            return Err(RepairContractError::new(
                "standard-css-audit verification cannot record supplementary evidence",
            ));
        }
        if self.finding_document_bytes == 0 || self.finding_document_bytes > MAX_DOCUMENT_BYTES {
            return Err(RepairContractError::new(
                "verification finding document must contain between 1 byte and 16 MiB",
            ));
        }
        if self.finding_count > 65_536 {
            return Err(RepairContractError::new(
                "verification finding count exceeds 65,536",
            ));
        }
        validate_sha256(
            "verificationReceipt.checks.findingDocumentSha256",
            &self.finding_document_sha256,
        )
    }
}

/// Browser evidence state emitted by schema-1 post-change verification.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepairVerificationBrowserEvidence {
    /// The closed check policy explicitly did not require a browser.
    NotRequired,
    /// The policy required a browser, which this static verifier did not collect.
    RequiredNotCollected,
    /// One exact required browser-evidence check completed and passed.
    RequiredPassed,
    /// One exact required browser-evidence check completed and failed.
    RequiredFailed,
}

/// Deterministic receipt derived from executed built-in checks over exact after-source bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairVerificationReceipt {
    pub(crate) schema_version: String,
    pub(crate) receipt_sha256: String,
    pub(crate) change_receipt: RepairArtifactIdentity,
    pub(crate) change_receipt_sha256: String,
    pub(crate) check_policy: RepairArtifactIdentity,
    pub(crate) result: RepairVerificationResult,
    pub(crate) checks: Vec<RepairVerificationCheck>,
    pub(crate) browser_evidence: RepairVerificationBrowserEvidence,
}

impl RepairVerificationReceipt {
    /// Returns the complete derived verification result.
    #[must_use]
    pub const fn result(&self) -> RepairVerificationResult {
        self.result
    }

    /// Returns the Change Receipt self-hash verified by this receipt.
    #[must_use]
    pub fn change_receipt_sha256(&self) -> &str {
        &self.change_receipt_sha256
    }

    /// Serializes canonical two-space JSON with one trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`RepairContractError`] when validation or serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, RepairContractError> {
        self.validate()?;
        let mut output = serde_json::to_string_pretty(self).map_err(|error| {
            RepairContractError::new(format!("cannot serialize verification receipt: {error}"))
        })?;
        output.push('\n');
        if output.len() > MAX_DOCUMENT_BYTES {
            return Err(RepairContractError::new(
                "verification receipt exceeds 16 MiB",
            ));
        }
        Ok(output)
    }

    /// Renders the bounded result without implying cryptographic identity or deployment success.
    #[must_use]
    pub fn to_human(&self) -> String {
        format!(
            "Verification: {}\nChange receipt: sha256:{}\nChecks: {}\nBrowser evidence: {}\n",
            match self.result {
                RepairVerificationResult::Passed => "passed",
                RepairVerificationResult::Failed => "failed",
                RepairVerificationResult::Blocked => "blocked",
            },
            self.change_receipt_sha256,
            self.checks.len(),
            match self.browser_evidence {
                RepairVerificationBrowserEvidence::NotRequired => "not-required",
                RepairVerificationBrowserEvidence::RequiredNotCollected => {
                    "required-not-collected"
                }
                RepairVerificationBrowserEvidence::RequiredPassed => "required-passed",
                RepairVerificationBrowserEvidence::RequiredFailed => "required-failed",
            }
        )
    }

    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
        if !matches!(
            self.schema_version.as_str(),
            LEGACY_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION
                | TOKEN_GRAPH_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION
                | CSS_BUDGET_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION
                | TEST_EVIDENCE_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION
                | REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION
        ) {
            return Err(RepairContractError::new(format!(
                "unsupported verification receipt schema `{}`; expected {LEGACY_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION}, {TOKEN_GRAPH_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION}, {CSS_BUDGET_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION}, {TEST_EVIDENCE_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION}, or {REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        validate_sha256("verificationReceipt.receiptSha256", &self.receipt_sha256)?;
        validate_sha256(
            "verificationReceipt.changeReceiptSha256",
            &self.change_receipt_sha256,
        )?;
        self.change_receipt
            .validate("verificationReceipt.changeReceipt")?;
        self.check_policy
            .validate("verificationReceipt.checkPolicy")?;
        if self.checks.is_empty() || self.checks.len() > MAX_CHECKS {
            return Err(RepairContractError::new(
                "verificationReceipt.checks must contain between 1 and 256 results",
            ));
        }
        for check in &self.checks {
            check.validate(&self.schema_version)?;
        }
        for pair in self.checks.windows(2) {
            if pair[0].id >= pair[1].id {
                return Err(RepairContractError::new(
                    "verificationReceipt.checks must be sorted and unique by id",
                ));
            }
        }
        let browser_checks = self
            .checks
            .iter()
            .filter(|check| check.kind == RepairCheckKind::BrowserEvidence)
            .collect::<Vec<_>>();
        if browser_checks.len() > 1 {
            return Err(RepairContractError::new(
                "verificationReceipt may contain at most one browser-evidence check",
            ));
        }
        let expected_browser = match browser_checks.as_slice() {
            [] if self.browser_evidence == RepairVerificationBrowserEvidence::NotRequired => {
                RepairVerificationBrowserEvidence::NotRequired
            }
            [] => RepairVerificationBrowserEvidence::RequiredNotCollected,
            [check] if check.status == RepairVerificationCheckStatus::Passed => {
                RepairVerificationBrowserEvidence::RequiredPassed
            }
            [..] => RepairVerificationBrowserEvidence::RequiredFailed,
        };
        if self.browser_evidence != expected_browser {
            return Err(RepairContractError::new(
                "verificationReceipt.browserEvidence does not match its browser check",
            ));
        }
        let expected_result = derive_verification_result(&self.checks, self.browser_evidence);
        if self.result != expected_result {
            return Err(RepairContractError::new(
                "verificationReceipt.result does not match check and browser evidence",
            ));
        }
        let expected = verification_receipt_payload_sha256(self)?;
        if self.receipt_sha256 != expected {
            return Err(RepairContractError::new(format!(
                "verificationReceipt.receiptSha256 `{}` does not match `{expected}`",
                self.receipt_sha256
            )));
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RepairVerificationReceiptPayload<'a> {
    pub(crate) schema_version: &'a str,
    pub(crate) change_receipt: &'a RepairArtifactIdentity,
    pub(crate) change_receipt_sha256: &'a str,
    pub(crate) check_policy: &'a RepairArtifactIdentity,
    pub(crate) result: RepairVerificationResult,
    pub(crate) checks: &'a [RepairVerificationCheck],
    pub(crate) browser_evidence: RepairVerificationBrowserEvidence,
}

pub(crate) fn verification_receipt_payload_sha256(
    receipt: &RepairVerificationReceipt,
) -> Result<String, RepairContractError> {
    let payload = RepairVerificationReceiptPayload {
        schema_version: &receipt.schema_version,
        change_receipt: &receipt.change_receipt,
        change_receipt_sha256: &receipt.change_receipt_sha256,
        check_policy: &receipt.check_policy,
        result: receipt.result,
        checks: &receipt.checks,
        browser_evidence: receipt.browser_evidence,
    };
    serde_json::to_vec(&payload)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(|error| {
            RepairContractError::new(format!("cannot hash verification receipt: {error}"))
        })
}

pub(crate) fn derive_verification_result(
    checks: &[RepairVerificationCheck],
    browser: RepairVerificationBrowserEvidence,
) -> RepairVerificationResult {
    if checks
        .iter()
        .any(|check| check.status == RepairVerificationCheckStatus::Failed)
    {
        RepairVerificationResult::Failed
    } else {
        match browser {
            RepairVerificationBrowserEvidence::RequiredNotCollected => {
                RepairVerificationResult::Blocked
            }
            RepairVerificationBrowserEvidence::NotRequired
            | RepairVerificationBrowserEvidence::RequiredPassed => RepairVerificationResult::Passed,
            RepairVerificationBrowserEvidence::RequiredFailed => RepairVerificationResult::Failed,
        }
    }
}

/// Parses and validates a canonical post-change Verification Receipt.
///
/// # Errors
///
/// Returns [`RepairContractError`] for malformed, noncanonical, drifted, or unsupported input.
pub fn parse_repair_verification_receipt(
    source: &[u8],
) -> Result<RepairVerificationReceipt, RepairContractError> {
    if source.len() > MAX_DOCUMENT_BYTES {
        return Err(RepairContractError::new(
            "verification receipt exceeds 16 MiB",
        ));
    }
    let receipt: RepairVerificationReceipt = serde_json::from_slice(source).map_err(|error| {
        RepairContractError::new(format!("invalid verification receipt JSON: {error}"))
    })?;
    receipt.validate()?;
    if receipt.to_json_pretty()?.as_bytes() != source {
        return Err(RepairContractError::new(
            "verification receipt bytes are not canonical pretty JSON",
        ));
    }
    Ok(receipt)
}
