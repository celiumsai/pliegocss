use super::validation::{validate_dotted_id, validate_logical_path};
use super::{
    CSS_BUDGET_REPAIR_CHECK_POLICY_SCHEMA_VERSION, LEGACY_REPAIR_CHECK_POLICY_SCHEMA_VERSION,
    MAX_CHECKS, MAX_DOCUMENT_BYTES, REPAIR_CHECK_POLICY_SCHEMA_VERSION, RepairArtifactIdentity,
    RepairContractError, TEST_EVIDENCE_REPAIR_CHECK_POLICY_SCHEMA_VERSION,
    TOKEN_GRAPH_REPAIR_CHECK_POLICY_SCHEMA_VERSION, supplementary_repair_input_identities,
    validate_browser_profile_inputs,
};
use pliego_css_build::artifacts::CompatibilityProfile;
use pliego_css_config::{BudgetSubject, BudgetSubjectKind};
use serde::{Deserialize, Serialize};

/// Built-in check implementation admitted by a repair-check policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepairCheckKind {
    /// Parse and audit one changed standard-CSS source without invoking an external process.
    StandardCssAudit,
    /// Parse and validate one exact canonical `PliegoCSS` token graph in process.
    TokenGraphIntegrity,
    /// Audit one changed CSS source against an exact versioned CSS budget policy in process.
    CssBudgetAudit,
    /// Validate exact evidence from the separate fixed-profile Rust test runner.
    TestSuiteEvidence,
    /// Validate exact evidence from the separate fixed `PliegoRS` Chromium runner.
    BrowserEvidence,
}

/// Browser-evidence requirement declared by a repair-check policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepairBrowserRequirement {
    /// The bounded verification policy does not require browser evidence.
    NotRequired,
    /// Browser evidence is required; schema 1 records a blocked result until it is supplied.
    Required,
}

/// One closed built-in check definition.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairCheckDefinition {
    pub(crate) id: String,
    pub(crate) kind: RepairCheckKind,
    pub(crate) source: String,
    pub(crate) compatibility_profile: CompatibilityProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) budget_policy: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) budget_subjects: Vec<BudgetSubject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) test_evidence: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) workspace_manifest: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) lockfile: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) browser_evidence_file: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) browser_profile_inputs: Vec<RepairArtifactIdentity>,
}

impl RepairCheckDefinition {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn validate(&self, schema_version: &str) -> Result<(), RepairContractError> {
        validate_dotted_id("checkPolicy.checks.id", &self.id)?;
        validate_logical_path("checkPolicy.checks.source", &self.source)?;
        match self.kind {
            RepairCheckKind::StandardCssAudit => {
                if self.compatibility_profile == CompatibilityProfile::None {
                    return Err(RepairContractError::new(
                        "standard-css-audit checks require `modern` or `baseline-widely` compatibility",
                    ));
                }
                self.require_no_supplementary_configuration("standard-css-audit")?;
            }
            RepairCheckKind::TokenGraphIntegrity => {
                if schema_version == LEGACY_REPAIR_CHECK_POLICY_SCHEMA_VERSION {
                    return Err(RepairContractError::new(
                        "repair-check policy schema 1.0.0 does not support token-graph-integrity",
                    ));
                }
                if self.compatibility_profile != CompatibilityProfile::None {
                    return Err(RepairContractError::new(
                        "token-graph-integrity checks require compatibilityProfile `none`",
                    ));
                }
                self.require_no_supplementary_configuration("token-graph-integrity")?;
            }
            RepairCheckKind::CssBudgetAudit => {
                if !matches!(
                    schema_version,
                    CSS_BUDGET_REPAIR_CHECK_POLICY_SCHEMA_VERSION
                        | TEST_EVIDENCE_REPAIR_CHECK_POLICY_SCHEMA_VERSION
                        | REPAIR_CHECK_POLICY_SCHEMA_VERSION
                ) {
                    return Err(RepairContractError::new(
                        "css-budget-audit requires repair-check policy schema 1.2.0, 1.3.0, or 1.4.0",
                    ));
                }
                if self.compatibility_profile == CompatibilityProfile::None {
                    return Err(RepairContractError::new(
                        "css-budget-audit checks require `modern` or `baseline-widely` compatibility",
                    ));
                }
                let budget_policy = self.budget_policy.as_ref().ok_or_else(|| {
                    RepairContractError::new(
                        "css-budget-audit checks require an exact budgetPolicy identity",
                    )
                })?;
                budget_policy.validate("checkPolicy.checks.budgetPolicy")?;
                if budget_policy.bytes == 0 {
                    return Err(RepairContractError::new(
                        "checkPolicy.checks.budgetPolicy.bytes must be greater than zero",
                    ));
                }
                if budget_policy.file == self.source {
                    return Err(RepairContractError::new(
                        "css-budget-audit source and budgetPolicy must be distinct files",
                    ));
                }
                if self.budget_subjects.len() > MAX_CHECKS {
                    return Err(RepairContractError::new(
                        "css-budget-audit cannot declare more than 256 budget subjects",
                    ));
                }
                for subject in &self.budget_subjects {
                    BudgetSubject::new(subject.kind(), subject.id()).map_err(|error| {
                        RepairContractError::new(format!("invalid budget subject: {error}"))
                    })?;
                    if !matches!(
                        subject.kind(),
                        BudgetSubjectKind::Package | BudgetSubjectKind::Route
                    ) {
                        return Err(RepairContractError::new(
                            "css-budget-audit budgetSubjects accept only package or route; file and layer are automatic",
                        ));
                    }
                }
                if !self
                    .budget_subjects
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                {
                    return Err(RepairContractError::new(
                        "css-budget-audit budgetSubjects must be sorted and unique",
                    ));
                }
                if self.test_evidence.is_some()
                    || self.workspace_manifest.is_some()
                    || self.lockfile.is_some()
                    || self.browser_evidence_file.is_some()
                    || !self.browser_profile_inputs.is_empty()
                {
                    return Err(RepairContractError::new(
                        "css-budget-audit does not accept test-suite or browser inputs",
                    ));
                }
            }
            RepairCheckKind::TestSuiteEvidence => {
                if !matches!(
                    schema_version,
                    TEST_EVIDENCE_REPAIR_CHECK_POLICY_SCHEMA_VERSION
                        | REPAIR_CHECK_POLICY_SCHEMA_VERSION
                ) {
                    return Err(RepairContractError::new(
                        "test-suite-evidence requires repair-check policy schema 1.3.0 or 1.4.0",
                    ));
                }
                if self.compatibility_profile != CompatibilityProfile::None {
                    return Err(RepairContractError::new(
                        "test-suite-evidence checks require compatibilityProfile `none`",
                    ));
                }
                if self.budget_policy.is_some() || !self.budget_subjects.is_empty() {
                    return Err(RepairContractError::new(
                        "test-suite-evidence does not accept budgetPolicy or budgetSubjects",
                    ));
                }
                if self.browser_evidence_file.is_some() || !self.browser_profile_inputs.is_empty() {
                    return Err(RepairContractError::new(
                        "test-suite-evidence does not accept browser inputs",
                    ));
                }
                let evidence = self.test_evidence.as_ref().ok_or_else(|| {
                    RepairContractError::new(
                        "test-suite-evidence checks require an exact testEvidence identity",
                    )
                })?;
                evidence.validate("checkPolicy.checks.testEvidence")?;
                if evidence.bytes == 0 {
                    return Err(RepairContractError::new(
                        "checkPolicy.checks.testEvidence.bytes must be greater than zero",
                    ));
                }
                if evidence.file == self.source {
                    return Err(RepairContractError::new(
                        "test-suite-evidence source and testEvidence must be distinct files",
                    ));
                }
                let manifest = self.workspace_manifest.as_ref().ok_or_else(|| {
                    RepairContractError::new(
                        "test-suite-evidence checks require an exact workspaceManifest identity",
                    )
                })?;
                manifest.validate("checkPolicy.checks.workspaceManifest")?;
                if manifest.file != "Cargo.toml" || manifest.bytes == 0 {
                    return Err(RepairContractError::new(
                        "test-suite-evidence workspaceManifest must identify nonempty `Cargo.toml`",
                    ));
                }
                let lockfile = self.lockfile.as_ref().ok_or_else(|| {
                    RepairContractError::new(
                        "test-suite-evidence checks require an exact lockfile identity",
                    )
                })?;
                lockfile.validate("checkPolicy.checks.lockfile")?;
                if lockfile.file != "Cargo.lock" || lockfile.bytes == 0 {
                    return Err(RepairContractError::new(
                        "test-suite-evidence lockfile must identify nonempty `Cargo.lock`",
                    ));
                }
            }
            RepairCheckKind::BrowserEvidence => {
                if schema_version != REPAIR_CHECK_POLICY_SCHEMA_VERSION {
                    return Err(RepairContractError::new(
                        "browser-evidence requires repair-check policy schema 1.4.0",
                    ));
                }
                if self.compatibility_profile != CompatibilityProfile::None {
                    return Err(RepairContractError::new(
                        "browser-evidence checks require compatibilityProfile `none`",
                    ));
                }
                if self.budget_policy.is_some()
                    || !self.budget_subjects.is_empty()
                    || self.test_evidence.is_some()
                    || self.workspace_manifest.is_some()
                    || self.lockfile.is_some()
                {
                    return Err(RepairContractError::new(
                        "browser-evidence does not accept budget or test-suite inputs",
                    ));
                }
                let evidence = self.browser_evidence_file.as_ref().ok_or_else(|| {
                    RepairContractError::new(
                        "browser-evidence checks require an exact browserEvidenceFile identity",
                    )
                })?;
                evidence.validate("checkPolicy.checks.browserEvidenceFile")?;
                if evidence.bytes == 0 || evidence.file == self.source {
                    return Err(RepairContractError::new(
                        "browserEvidenceFile must be nonempty and distinct from its source",
                    ));
                }
                validate_browser_profile_inputs(&self.browser_profile_inputs)?;
                if self
                    .browser_profile_inputs
                    .iter()
                    .any(|input| input.file == evidence.file)
                {
                    return Err(RepairContractError::new(
                        "browser profile inputs must be distinct from the evidence file",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn require_no_supplementary_configuration(
        &self,
        kind: &str,
    ) -> Result<(), RepairContractError> {
        if self.budget_policy.is_some()
            || !self.budget_subjects.is_empty()
            || self.test_evidence.is_some()
            || self.workspace_manifest.is_some()
            || self.lockfile.is_some()
            || self.browser_evidence_file.is_some()
            || !self.browser_profile_inputs.is_empty()
        {
            return Err(RepairContractError::new(format!(
                "{kind} does not accept budgetPolicy, budgetSubjects, test-suite inputs, or browser inputs"
            )));
        }
        Ok(())
    }
}

/// Closed policy selecting exact built-in post-change checks.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairCheckPolicy {
    pub(crate) schema_version: String,
    pub(crate) checks: Vec<RepairCheckDefinition>,
    pub(crate) browser_evidence: RepairBrowserRequirement,
}

impl RepairCheckPolicy {
    /// Serializes the validated canonical policy with one trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`RepairContractError`] when validation or serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, RepairContractError> {
        self.validate()?;
        let mut output = serde_json::to_string_pretty(self).map_err(|error| {
            RepairContractError::new(format!("cannot serialize repair-check policy: {error}"))
        })?;
        output.push('\n');
        if output.len() > MAX_DOCUMENT_BYTES {
            return Err(RepairContractError::new(
                "repair-check policy exceeds 16 MiB",
            ));
        }
        Ok(output)
    }

    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
        if !matches!(
            self.schema_version.as_str(),
            LEGACY_REPAIR_CHECK_POLICY_SCHEMA_VERSION
                | TOKEN_GRAPH_REPAIR_CHECK_POLICY_SCHEMA_VERSION
                | CSS_BUDGET_REPAIR_CHECK_POLICY_SCHEMA_VERSION
                | TEST_EVIDENCE_REPAIR_CHECK_POLICY_SCHEMA_VERSION
                | REPAIR_CHECK_POLICY_SCHEMA_VERSION
        ) {
            return Err(RepairContractError::new(format!(
                "unsupported repair-check policy schema `{}`; expected {LEGACY_REPAIR_CHECK_POLICY_SCHEMA_VERSION}, {TOKEN_GRAPH_REPAIR_CHECK_POLICY_SCHEMA_VERSION}, {CSS_BUDGET_REPAIR_CHECK_POLICY_SCHEMA_VERSION}, {TEST_EVIDENCE_REPAIR_CHECK_POLICY_SCHEMA_VERSION}, or {REPAIR_CHECK_POLICY_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        if self.checks.is_empty() || self.checks.len() > MAX_CHECKS {
            return Err(RepairContractError::new(
                "checkPolicy.checks must contain between 1 and 256 definitions",
            ));
        }
        for check in &self.checks {
            check.validate(&self.schema_version)?;
        }
        for pair in self.checks.windows(2) {
            if pair[0].id >= pair[1].id {
                return Err(RepairContractError::new(
                    "checkPolicy.checks must be sorted and unique by id",
                ));
            }
        }
        let browser_checks = self
            .checks
            .iter()
            .filter(|check| check.kind == RepairCheckKind::BrowserEvidence)
            .count();
        if browser_checks > 1 {
            return Err(RepairContractError::new(
                "checkPolicy may contain at most one browser-evidence check",
            ));
        }
        if self.browser_evidence == RepairBrowserRequirement::NotRequired && browser_checks != 0 {
            return Err(RepairContractError::new(
                "browser-evidence checks require browserEvidence `required`",
            ));
        }
        supplementary_repair_input_identities(self, std::iter::empty())?;
        Ok(())
    }
}

/// Parses and validates a canonical closed repair-check policy.
///
/// # Errors
///
/// Returns [`RepairContractError`] for malformed, noncanonical, unsupported, or over-limit input.
pub fn parse_repair_check_policy(source: &[u8]) -> Result<RepairCheckPolicy, RepairContractError> {
    if source.len() > MAX_DOCUMENT_BYTES {
        return Err(RepairContractError::new(
            "repair-check policy exceeds 16 MiB",
        ));
    }
    let policy: RepairCheckPolicy = serde_json::from_slice(source).map_err(|error| {
        RepairContractError::new(format!("invalid repair-check policy JSON: {error}"))
    })?;
    policy.validate()?;
    if policy.to_json_pretty()?.as_bytes() != source {
        return Err(RepairContractError::new(
            "repair-check policy bytes are not canonical pretty JSON",
        ));
    }
    Ok(policy)
}
