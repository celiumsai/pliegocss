//! Deterministic bounded repair plans, explicit application, and verification evidence.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fs2::FileExt;
use pliego_css_build::artifacts::{
    CompatibilityProfile, Finding, FindingCause, FindingDocument, FindingRisk, FindingSeverity,
    FindingSource, FindingSuggestion, FindingTool, FindingVerification, audit_standard_css,
    audit_standard_css_with_budgets, parse_finding_document, sha256_hex,
};
use pliego_css_config::{BudgetSubject, BudgetSubjectKind, parse_budget_policy, parse_token_graph};
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

/// Semantic version of the repair-proposal document.
pub const REPAIR_PROPOSAL_SCHEMA_VERSION: &str = "1.0.0";
/// Semantic version of the repair-plan document.
pub const REPAIR_PLAN_SCHEMA_VERSION: &str = "1.0.0";
/// Semantic version of the dry-run report.
pub const REPAIR_DRY_RUN_SCHEMA_VERSION: &str = "1.0.0";
/// Semantic version of the change receipt.
pub const REPAIR_CHANGE_RECEIPT_SCHEMA_VERSION: &str = "1.0.0";
/// Semantic version of the closed repair-check policy.
pub const REPAIR_CHECK_POLICY_SCHEMA_VERSION: &str = "1.4.0";
/// Semantic version of the post-change verification receipt.
pub const REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION: &str = "1.4.0";
/// Semantic version of fixed-profile Rust test-suite evidence.
pub const REPAIR_TEST_EVIDENCE_SCHEMA_VERSION: &str = "1.0.0";
/// Semantic version of fixed-profile `PliegoRS` Chromium browser evidence.
pub const REPAIR_BROWSER_EVIDENCE_SCHEMA_VERSION: &str = "1.0.0";

const LEGACY_REPAIR_CHECK_POLICY_SCHEMA_VERSION: &str = "1.0.0";
const TOKEN_GRAPH_REPAIR_CHECK_POLICY_SCHEMA_VERSION: &str = "1.1.0";
const CSS_BUDGET_REPAIR_CHECK_POLICY_SCHEMA_VERSION: &str = "1.2.0";
const TEST_EVIDENCE_REPAIR_CHECK_POLICY_SCHEMA_VERSION: &str = "1.3.0";
const LEGACY_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION: &str = "1.0.0";
const TOKEN_GRAPH_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION: &str = "1.1.0";
const CSS_BUDGET_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION: &str = "1.2.0";
const TEST_EVIDENCE_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION: &str = "1.3.0";

const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_REPLACEMENT_BYTES: usize = 1024 * 1024;
const MAX_EDITS: usize = 65_535;
const MAX_FILES: usize = 65_535;
const MAX_CHECKS: usize = 256;
const MAX_TOTAL_CHANGE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_VERIFICATION_EVIDENCE_BYTES: usize = 64 * 1024 * 1024;
const REPAIR_BROWSER_PROFILE_FILES: [&str; 8] = [
    "Cargo.lock",
    "Cargo.toml",
    "integration-tests/pliegors-smoke/Cargo.toml",
    "integration-tests/pliegors-smoke/client/Cargo.toml",
    "integration-tests/pliegors-smoke/pliegors-contract.json",
    "scripts/check-pliegors-browser.mjs",
    "scripts/check-pliegors-integration.mjs",
    "scripts/pliegors-contract.mjs",
];

/// Human or canonical JSON projection selected by the repair CLI.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RepairCliFormat {
    /// Concise human projection.
    #[default]
    Text,
    /// Canonical machine-readable JSON.
    Json,
}

/// Parsed arguments for the read-only `plan` command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairPlanCliArgs {
    /// Canonical finding document path.
    pub findings: PathBuf,
    /// Exact agent-authored proposal path.
    pub proposal: PathBuf,
    /// Root against which proposal logical source paths resolve.
    pub source_root: PathBuf,
    /// Output projection; JSON is the command default.
    pub format: RepairCliFormat,
}

/// Explicit execution mode for `fix`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepairFixMode {
    /// Verify all bound inputs without mutation.
    DryRun,
    /// Apply exact edits with an external exact-plan authorization and emit a receipt.
    Apply {
        /// Literal `sha256:<planSha256>` authorization token.
        authorization: String,
        /// Required adjacent Change Receipt destination.
        receipt: PathBuf,
    },
}

/// Parsed arguments for the `fix` command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairFixCliArgs {
    /// Integrity-bound repair plan path.
    pub plan: PathBuf,
    /// Original finding document bound by the plan.
    pub findings: PathBuf,
    /// Root against which plan logical source paths resolve.
    pub source_root: PathBuf,
    /// Read-only or explicitly authorized apply mode.
    pub mode: RepairFixMode,
    /// Output projection; text is the command default.
    pub format: RepairCliFormat,
}

/// Closed repair command surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepairCliCommand {
    /// Create a dry-run-only plan.
    Plan(RepairPlanCliArgs),
    /// Verify or explicitly apply a plan.
    Fix(RepairFixCliArgs),
}

/// Parses the closed `plan` or `fix` option surface.
///
/// # Errors
///
/// Returns a stable invocation message for missing, duplicate, conflicting, or unknown options.
pub fn parse_repair_cli_arguments(
    command: &str,
    arguments: &[String],
) -> Result<RepairCliCommand, String> {
    let fix = match command {
        "plan" => false,
        "fix" => true,
        _ => return Err(format!("unknown repair command `{command}`")),
    };
    let mut findings = None;
    let mut proposal = None;
    let mut plan = None;
    let mut source_root = None;
    let mut dry_run = false;
    let mut apply = false;
    let mut authorization = None;
    let mut receipt = None;
    let mut format = None;
    let mut index = 0;
    while index < arguments.len() {
        let option = arguments[index].as_str();
        match option {
            "--findings" => set_cli_path(&mut findings, arguments, &mut index, option)?,
            "--proposal" if !fix => set_cli_path(&mut proposal, arguments, &mut index, option)?,
            "--plan" if fix => set_cli_path(&mut plan, arguments, &mut index, option)?,
            "--source-root" => set_cli_path(&mut source_root, arguments, &mut index, option)?,
            "--dry-run" if fix && !dry_run => {
                dry_run = true;
                index += 1;
            }
            "--dry-run" if fix => return Err("`--dry-run` may only be provided once".into()),
            "--apply" if fix && !apply => {
                apply = true;
                index += 1;
            }
            "--apply" if fix => return Err("`--apply` may only be provided once".into()),
            "--authorize" if fix => {
                if authorization.is_some() {
                    return Err("`--authorize` may only be provided once".into());
                }
                authorization = Some(cli_value(arguments, index, option)?.to_owned());
                index += 2;
            }
            "--receipt" if fix => set_cli_path(&mut receipt, arguments, &mut index, option)?,
            "--format" => {
                if format.is_some() {
                    return Err("`--format` may only be provided once".into());
                }
                format = Some(match cli_value(arguments, index, option)? {
                    "text" => RepairCliFormat::Text,
                    "json" => RepairCliFormat::Json,
                    value => {
                        return Err(format!(
                            "unknown repair format `{value}`; expected `text` or `json`"
                        ));
                    }
                });
                index += 2;
            }
            _ => return Err(format!("unknown {command} option `{option}`")),
        }
    }
    let source_root =
        source_root.ok_or_else(|| format!("`{command}` requires `--source-root DIR`"))?;
    if fix {
        let mode = match (dry_run, apply) {
            (true, false) if authorization.is_none() && receipt.is_none() => RepairFixMode::DryRun,
            (false, true) => RepairFixMode::Apply {
                authorization: authorization.ok_or_else(|| {
                    "`fix --apply` requires `--authorize sha256:PLAN_HASH`".to_owned()
                })?,
                receipt: receipt
                    .ok_or_else(|| "`fix --apply` requires `--receipt FILE`".to_owned())?,
            },
            (true, true) => return Err("`--dry-run` and `--apply` are mutually exclusive".into()),
            (true, false) => {
                return Err("`--authorize` and `--receipt` require `--apply`".into());
            }
            (false, false) => {
                return Err("`fix` requires exactly one of `--dry-run` or `--apply`".into());
            }
        };
        return Ok(RepairCliCommand::Fix(RepairFixCliArgs {
            plan: plan.ok_or_else(|| "`fix` requires `--plan FILE`".to_owned())?,
            findings: findings.ok_or_else(|| "`fix` requires `--findings FILE`".to_owned())?,
            source_root,
            mode,
            format: format.unwrap_or_default(),
        }));
    }
    Ok(RepairCliCommand::Plan(RepairPlanCliArgs {
        findings: findings.ok_or_else(|| "`plan` requires `--findings FILE`".to_owned())?,
        proposal: proposal.ok_or_else(|| "`plan` requires `--proposal FILE`".to_owned())?,
        source_root,
        format: format.unwrap_or(RepairCliFormat::Json),
    }))
}

fn set_cli_path(
    slot: &mut Option<PathBuf>,
    arguments: &[String],
    index: &mut usize,
    option: &str,
) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("`{option}` may only be provided once"));
    }
    *slot = Some(PathBuf::from(cli_value(arguments, *index, option)?));
    *index += 2;
    Ok(())
}

fn cli_value<'a>(arguments: &'a [String], index: usize, option: &str) -> Result<&'a str, String> {
    arguments
        .get(index + 1)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("`{option}` requires a value"))
}

/// Error returned when a proposal, plan, or source snapshot violates the closed contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairContractError {
    reason: String,
}

impl RepairContractError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl fmt::Display for RepairContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::error::Error for RepairContractError {}

/// Tool identity bound into a repair plan.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairTool {
    name: String,
    version: String,
}

impl RepairTool {
    /// Creates a validated tool identity.
    ///
    /// # Errors
    ///
    /// Returns [`RepairContractError`] for an invalid name or version.
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, RepairContractError> {
        let value = Self {
            name: name.into(),
            version: version.into(),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), RepairContractError> {
        validate_slug("tool.name", &self.name)?;
        validate_text("tool.version", &self.version, 128)
    }
}

/// Hard upper bounds authorized for one proposed change.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[allow(clippy::struct_field_names)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairChangeBudget {
    max_files: u32,
    max_edits: u32,
    max_inserted_bytes: u64,
    max_removed_bytes: u64,
}

impl RepairChangeBudget {
    /// Creates a bounded change budget.
    ///
    /// # Errors
    ///
    /// Returns [`RepairContractError`] for zero file/edit limits or defensive-limit overflow.
    pub fn new(
        max_files: u32,
        max_edits: u32,
        max_inserted_bytes: u64,
        max_removed_bytes: u64,
    ) -> Result<Self, RepairContractError> {
        let value = Self {
            max_files,
            max_edits,
            max_inserted_bytes,
            max_removed_bytes,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), RepairContractError> {
        if self.max_files == 0 || usize::try_from(self.max_files).map_or(true, |v| v > MAX_FILES) {
            return Err(RepairContractError::new(
                "changeBudget.maxFiles must be between 1 and 65,535",
            ));
        }
        if self.max_edits == 0 || usize::try_from(self.max_edits).map_or(true, |v| v > MAX_EDITS) {
            return Err(RepairContractError::new(
                "changeBudget.maxEdits must be between 1 and 65,535",
            ));
        }
        if self.max_inserted_bytes > MAX_TOTAL_CHANGE_BYTES
            || self.max_removed_bytes > MAX_TOTAL_CHANGE_BYTES
        {
            return Err(RepairContractError::new(
                "changeBudget byte limits must not exceed 64 MiB",
            ));
        }
        Ok(())
    }
}

/// One exact byte edit proposed against the source range of a finding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairProposalEdit {
    finding_fingerprint: String,
    suggestion_id: String,
    file: String,
    byte_start: usize,
    byte_end: usize,
    replacement: String,
}

impl RepairProposalEdit {
    fn validate(&self) -> Result<(), RepairContractError> {
        validate_fingerprint("edit.findingFingerprint", &self.finding_fingerprint)?;
        validate_slug("edit.suggestionId", &self.suggestion_id)?;
        validate_logical_path("edit.file", &self.file)?;
        if self.byte_end < self.byte_start {
            return Err(RepairContractError::new(
                "edit.byteEnd precedes edit.byteStart",
            ));
        }
        if self.replacement.len() > MAX_REPLACEMENT_BYTES {
            return Err(RepairContractError::new("edit.replacement exceeds 1 MiB"));
        }
        if self.replacement.contains('\0') {
            return Err(RepairContractError::new(
                "edit.replacement contains a NUL byte",
            ));
        }
        Ok(())
    }
}

/// Agent-authored exact edit proposal linked to an immutable finding document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairProposalDocument {
    schema_version: String,
    finding_document_sha256: String,
    change_budget: RepairChangeBudget,
    required_checks: Vec<String>,
    edits: Vec<RepairProposalEdit>,
}

impl RepairProposalDocument {
    /// Returns unique source paths in canonical order.
    #[must_use]
    pub fn files(&self) -> Vec<&str> {
        self.edits
            .iter()
            .map(|edit| edit.file.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn validate(&self) -> Result<(), RepairContractError> {
        if self.schema_version != REPAIR_PROPOSAL_SCHEMA_VERSION {
            return Err(RepairContractError::new(format!(
                "unsupported repair proposal schema `{}`; expected {REPAIR_PROPOSAL_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        validate_sha256(
            "proposal.findingDocumentSha256",
            &self.finding_document_sha256,
        )?;
        self.change_budget.validate()?;
        validate_checks(&self.required_checks)?;
        if self.edits.is_empty() || self.edits.len() > MAX_EDITS {
            return Err(RepairContractError::new(
                "proposal.edits must contain between 1 and 65,535 edits",
            ));
        }
        for edit in &self.edits {
            edit.validate()?;
        }
        validate_edit_order(&self.edits)?;
        Ok(())
    }
}

/// Parses and validates a closed repair-proposal document.
///
/// # Errors
///
/// Returns [`RepairContractError`] for malformed JSON, unknown fields, noncanonical order, unsafe
/// paths, invalid ranges, hashes, budgets, or defensive-limit violations.
pub fn parse_repair_proposal(source: &[u8]) -> Result<RepairProposalDocument, RepairContractError> {
    if source.len() > MAX_DOCUMENT_BYTES {
        return Err(RepairContractError::new("repair proposal exceeds 16 MiB"));
    }
    let document: RepairProposalDocument = serde_json::from_slice(source).map_err(|error| {
        RepairContractError::new(format!("invalid repair proposal JSON: {error}"))
    })?;
    document.validate()?;
    Ok(document)
}

/// Identity of an exact input artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairArtifactIdentity {
    file: String,
    bytes: usize,
    sha256: String,
}

impl RepairArtifactIdentity {
    fn validate(&self, role: &str) -> Result<(), RepairContractError> {
        validate_logical_path(&format!("{role}.file"), &self.file)?;
        if self.bytes > MAX_DOCUMENT_BYTES {
            return Err(RepairContractError::new(format!(
                "{role}.bytes exceeds 16 MiB"
            )));
        }
        validate_sha256(&format!("{role}.sha256"), &self.sha256)
    }
}

/// Fixed test-suite command profile executed outside the static verifier.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepairTestProfile {
    /// `cargo test --workspace --all-targets --locked --offline` from the project root.
    RustWorkspaceAllTargets,
}

/// Derived result of one fixed-profile test-suite execution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RepairTestEvidenceResult {
    /// The fixed runner exited successfully.
    Passed,
    /// The fixed runner returned a nonzero exit code.
    Failed,
}

/// Identity of the code that executed a fixed test profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairTestRunnerIdentity {
    name: String,
    version: String,
}

impl RepairTestRunnerIdentity {
    fn validate(&self) -> Result<(), RepairContractError> {
        if self.name != "pliego-css-agent" {
            return Err(RepairContractError::new(
                "testEvidence.runner.name must be `pliego-css-agent`",
            ));
        }
        validate_version("testEvidence.runner.version", &self.version)
    }
}

/// Exact toolchain identity observed by the fixed Rust test runner.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairTestToolchainIdentity {
    cargo_version: String,
    rustc_version: String,
    host: String,
}

impl RepairTestToolchainIdentity {
    fn validate(&self) -> Result<(), RepairContractError> {
        validate_text(
            "testEvidence.toolchain.cargoVersion",
            &self.cargo_version,
            256,
        )?;
        validate_text(
            "testEvidence.toolchain.rustcVersion",
            &self.rustc_version,
            256,
        )?;
        if !self.cargo_version.starts_with("cargo ") || !self.rustc_version.starts_with("rustc ") {
            return Err(RepairContractError::new(
                "testEvidence.toolchain versions must retain canonical cargo/rustc prefixes",
            ));
        }
        if self.host.is_empty()
            || self.host.len() > 128
            || !self
                .host
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(RepairContractError::new(
                "testEvidence.toolchain.host must be a bounded ASCII target triple",
            ));
        }
        Ok(())
    }
}

/// Canonical evidence emitted by the explicit fixed-profile Rust test runner.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairTestEvidence {
    schema_version: String,
    evidence_sha256: String,
    change_receipt_sha256: String,
    check_id: String,
    profile: RepairTestProfile,
    runner: RepairTestRunnerIdentity,
    toolchain: RepairTestToolchainIdentity,
    workspace_manifest: RepairArtifactIdentity,
    lockfile: RepairArtifactIdentity,
    result: RepairTestEvidenceResult,
    exit_code: u8,
}

impl RepairTestEvidence {
    /// Returns the result derived from the fixed runner exit code.
    #[must_use]
    pub const fn result(&self) -> RepairTestEvidenceResult {
        self.result
    }

    /// Serializes canonical two-space JSON with one trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`RepairContractError`] when validation or serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, RepairContractError> {
        self.validate()?;
        let mut output = serde_json::to_string_pretty(self).map_err(|error| {
            RepairContractError::new(format!("cannot serialize test evidence: {error}"))
        })?;
        output.push('\n');
        if output.len() > MAX_DOCUMENT_BYTES {
            return Err(RepairContractError::new("test evidence exceeds 16 MiB"));
        }
        Ok(output)
    }

    fn validate(&self) -> Result<(), RepairContractError> {
        if self.schema_version != REPAIR_TEST_EVIDENCE_SCHEMA_VERSION {
            return Err(RepairContractError::new(format!(
                "unsupported test evidence schema `{}`; expected {REPAIR_TEST_EVIDENCE_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        validate_sha256("testEvidence.evidenceSha256", &self.evidence_sha256)?;
        validate_sha256(
            "testEvidence.changeReceiptSha256",
            &self.change_receipt_sha256,
        )?;
        validate_dotted_id("testEvidence.checkId", &self.check_id)?;
        self.runner.validate()?;
        self.toolchain.validate()?;
        self.workspace_manifest
            .validate("testEvidence.workspaceManifest")?;
        self.lockfile.validate("testEvidence.lockfile")?;
        if self.workspace_manifest.file != "Cargo.toml" || self.workspace_manifest.bytes == 0 {
            return Err(RepairContractError::new(
                "testEvidence.workspaceManifest must identify nonempty `Cargo.toml`",
            ));
        }
        if self.lockfile.file != "Cargo.lock" || self.lockfile.bytes == 0 {
            return Err(RepairContractError::new(
                "testEvidence.lockfile must identify nonempty `Cargo.lock`",
            ));
        }
        let expected_result = if self.exit_code == 0 {
            RepairTestEvidenceResult::Passed
        } else {
            RepairTestEvidenceResult::Failed
        };
        if self.result != expected_result {
            return Err(RepairContractError::new(
                "testEvidence.result does not match exitCode",
            ));
        }
        let expected_hash = repair_test_evidence_payload_sha256(self)?;
        if self.evidence_sha256 != expected_hash {
            return Err(RepairContractError::new(format!(
                "testEvidence.evidenceSha256 `{}` does not match `{expected_hash}`",
                self.evidence_sha256
            )));
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RepairTestEvidencePayload<'a> {
    schema_version: &'a str,
    change_receipt_sha256: &'a str,
    check_id: &'a str,
    profile: RepairTestProfile,
    runner: &'a RepairTestRunnerIdentity,
    toolchain: &'a RepairTestToolchainIdentity,
    workspace_manifest: &'a RepairArtifactIdentity,
    lockfile: &'a RepairArtifactIdentity,
    result: RepairTestEvidenceResult,
    exit_code: u8,
}

fn repair_test_evidence_payload_sha256(
    evidence: &RepairTestEvidence,
) -> Result<String, RepairContractError> {
    let payload = RepairTestEvidencePayload {
        schema_version: &evidence.schema_version,
        change_receipt_sha256: &evidence.change_receipt_sha256,
        check_id: &evidence.check_id,
        profile: evidence.profile,
        runner: &evidence.runner,
        toolchain: &evidence.toolchain,
        workspace_manifest: &evidence.workspace_manifest,
        lockfile: &evidence.lockfile,
        result: evidence.result,
        exit_code: evidence.exit_code,
    };
    serde_json::to_vec(&payload)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(|error| RepairContractError::new(format!("cannot hash test evidence: {error}")))
}

/// Parses and validates canonical fixed-profile test-suite evidence.
///
/// # Errors
///
/// Returns [`RepairContractError`] for malformed, noncanonical, drifted, or unsupported input.
pub fn parse_repair_test_evidence(
    source: &[u8],
) -> Result<RepairTestEvidence, RepairContractError> {
    if source.len() > MAX_DOCUMENT_BYTES {
        return Err(RepairContractError::new("test evidence exceeds 16 MiB"));
    }
    let evidence: RepairTestEvidence = serde_json::from_slice(source).map_err(|error| {
        RepairContractError::new(format!("invalid test evidence JSON: {error}"))
    })?;
    evidence.validate()?;
    if evidence.to_json_pretty()?.as_bytes() != source {
        return Err(RepairContractError::new(
            "test evidence bytes are not canonical pretty JSON",
        ));
    }
    Ok(evidence)
}

/// Fixed browser profile currently admitted by repair verification.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepairBrowserProfile {
    /// The checked `PliegoRS` visit-counter fixture driven through Chromium CDP.
    PliegorsVisitCounterChromiumCdp,
}

/// Result of one fixed browser-profile execution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RepairBrowserEvidenceResult {
    /// Every closed observation in the fixed profile passed.
    Passed,
    /// The profile completed but at least one closed observation failed.
    Failed,
}

/// Identity of the process boundary that constructed browser evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairBrowserRunnerIdentity {
    name: String,
    version: String,
    node_version: String,
}

impl RepairBrowserRunnerIdentity {
    fn validate(&self) -> Result<(), RepairContractError> {
        if self.name != "pliego-css-agent" {
            return Err(RepairContractError::new(
                "browserEvidence.runner.name must be `pliego-css-agent`",
            ));
        }
        validate_version("browserEvidence.runner.version", &self.version)?;
        validate_text(
            "browserEvidence.runner.nodeVersion",
            &self.node_version,
            256,
        )?;
        if !self.node_version.starts_with('v') {
            return Err(RepairContractError::new(
                "browserEvidence.runner.nodeVersion must retain the canonical `v` prefix",
            ));
        }
        Ok(())
    }
}

/// Browser metadata observed through the fixed CDP session.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairBrowserIdentity {
    family: String,
    executable: String,
    product: String,
    revision: String,
    protocol_version: String,
    js_version: String,
    user_agent: String,
}

impl RepairBrowserIdentity {
    fn validate(&self) -> Result<(), RepairContractError> {
        if self.family != "chromium" {
            return Err(RepairContractError::new(
                "browserEvidence.browser.family must be `chromium`",
            ));
        }
        for (field, value, maximum) in [
            ("executable", self.executable.as_str(), 256),
            ("product", self.product.as_str(), 256),
            ("revision", self.revision.as_str(), 512),
            ("protocolVersion", self.protocol_version.as_str(), 64),
            ("jsVersion", self.js_version.as_str(), 128),
            ("userAgent", self.user_agent.as_str(), 1_024),
        ] {
            validate_text(&format!("browserEvidence.browser.{field}"), value, maximum)?;
        }
        if !(self.product.starts_with("Chrome/") || self.product.starts_with("HeadlessChrome/")) {
            return Err(RepairContractError::new(
                "browserEvidence.browser.product must identify Chrome or HeadlessChrome",
            ));
        }
        Ok(())
    }
}

/// Object-identity observations from the resumable `PliegoRS` island replay.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairBrowserNodeIdentity {
    document: bool,
    island: bool,
    button: bool,
    value: bool,
}

/// Closed observations produced by the `PliegoRS` visit-counter browser profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairBrowserObservation {
    node_identity: RepairBrowserNodeIdentity,
    island_count: u16,
    island_id: String,
    initial_minutes: u16,
    final_minutes: u16,
    increment: u16,
    initial_text: String,
    final_text: String,
    initial_class: String,
    final_class: String,
    event_count: u16,
    event_key: String,
    event_value: u16,
    module_script_count: u16,
    preload_entry_count: u16,
    preload_request_count: u16,
    client_request_count: u16,
    stylesheet_present: bool,
    wasm_ready: bool,
    relevant_event_count: u16,
    server_error_count: u16,
}

impl RepairBrowserObservation {
    fn contract_passed(&self) -> bool {
        self.node_identity
            == RepairBrowserNodeIdentity {
                document: true,
                island: true,
                button: true,
                value: true,
            }
            && self.island_count == 1
            && self.island_id == "visit-counter"
            && self.initial_minutes == 15
            && self.final_minutes == 20
            && self.increment == 5
            && self.initial_text == "15"
            && self.final_text == "20"
            && self.initial_class == self.final_class
            && valid_browser_style_classes(&self.initial_class)
            && self.event_count == 1
            && self.event_key == "minutes"
            && self.event_value == 20
            && self.module_script_count == 2
            && self.preload_entry_count == 1
            && self.preload_request_count == 1
            && self.client_request_count == 4
            && self.stylesheet_present
            && self.wasm_ready
            && self.relevant_event_count == 0
            && self.server_error_count == 0
    }

    fn validate(&self) -> Result<(), RepairContractError> {
        validate_text("browserEvidence.observation.islandId", &self.island_id, 256)?;
        validate_text(
            "browserEvidence.observation.initialText",
            &self.initial_text,
            256,
        )?;
        validate_text(
            "browserEvidence.observation.finalText",
            &self.final_text,
            256,
        )?;
        validate_text(
            "browserEvidence.observation.initialClass",
            &self.initial_class,
            4_096,
        )?;
        validate_text(
            "browserEvidence.observation.finalClass",
            &self.final_class,
            4_096,
        )?;
        validate_text("browserEvidence.observation.eventKey", &self.event_key, 256)
    }
}

fn valid_browser_style_classes(classes: &str) -> bool {
    !classes.is_empty()
        && classes.split_ascii_whitespace().all(|class| {
            let Some(suffix) = class.strip_prefix("pc_") else {
                return false;
            };
            !suffix.is_empty()
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

/// Canonical evidence emitted by the fixed `PliegoRS` Chromium runner.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairBrowserEvidence {
    schema_version: String,
    evidence_sha256: String,
    change_receipt_sha256: String,
    check_id: String,
    profile: RepairBrowserProfile,
    runner: RepairBrowserRunnerIdentity,
    profile_inputs: Vec<RepairArtifactIdentity>,
    browser: RepairBrowserIdentity,
    observation: RepairBrowserObservation,
    result: RepairBrowserEvidenceResult,
    exit_code: u8,
}

impl RepairBrowserEvidence {
    /// Returns the result derived from the fixed profile observations and exit status.
    #[must_use]
    pub const fn result(&self) -> RepairBrowserEvidenceResult {
        self.result
    }

    /// Serializes canonical two-space JSON with one trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`RepairContractError`] when validation or serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, RepairContractError> {
        self.validate()?;
        let mut output = serde_json::to_string_pretty(self).map_err(|error| {
            RepairContractError::new(format!("cannot serialize browser evidence: {error}"))
        })?;
        output.push('\n');
        if output.len() > MAX_DOCUMENT_BYTES {
            return Err(RepairContractError::new("browser evidence exceeds 16 MiB"));
        }
        Ok(output)
    }

    fn validate(&self) -> Result<(), RepairContractError> {
        if self.schema_version != REPAIR_BROWSER_EVIDENCE_SCHEMA_VERSION {
            return Err(RepairContractError::new(format!(
                "unsupported browser evidence schema `{}`; expected {REPAIR_BROWSER_EVIDENCE_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        validate_sha256("browserEvidence.evidenceSha256", &self.evidence_sha256)?;
        validate_sha256(
            "browserEvidence.changeReceiptSha256",
            &self.change_receipt_sha256,
        )?;
        validate_dotted_id("browserEvidence.checkId", &self.check_id)?;
        self.runner.validate()?;
        validate_browser_profile_inputs(&self.profile_inputs)?;
        self.browser.validate()?;
        self.observation.validate()?;
        let expected_result = if self.observation.contract_passed() && self.exit_code == 0 {
            RepairBrowserEvidenceResult::Passed
        } else {
            RepairBrowserEvidenceResult::Failed
        };
        if self.result != expected_result {
            return Err(RepairContractError::new(
                "browserEvidence.result does not match the fixed profile observations and exitCode",
            ));
        }
        let expected_hash = repair_browser_evidence_payload_sha256(self)?;
        if self.evidence_sha256 != expected_hash {
            return Err(RepairContractError::new(format!(
                "browserEvidence.evidenceSha256 `{}` does not match `{expected_hash}`",
                self.evidence_sha256
            )));
        }
        Ok(())
    }
}

fn validate_browser_profile_inputs(
    inputs: &[RepairArtifactIdentity],
) -> Result<(), RepairContractError> {
    if inputs.len() != REPAIR_BROWSER_PROFILE_FILES.len()
        || inputs
            .iter()
            .map(|identity| identity.file.as_str())
            .ne(REPAIR_BROWSER_PROFILE_FILES)
    {
        return Err(RepairContractError::new(
            "browserEvidence.profileInputs must identify the exact fixed PliegoRS browser profile inputs",
        ));
    }
    for identity in inputs {
        identity.validate("browserEvidence.profileInputs")?;
        if identity.bytes == 0 {
            return Err(RepairContractError::new(
                "browserEvidence.profileInputs entries must be nonempty",
            ));
        }
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RepairBrowserEvidencePayload<'a> {
    schema_version: &'a str,
    change_receipt_sha256: &'a str,
    check_id: &'a str,
    profile: RepairBrowserProfile,
    runner: &'a RepairBrowserRunnerIdentity,
    profile_inputs: &'a [RepairArtifactIdentity],
    browser: &'a RepairBrowserIdentity,
    observation: &'a RepairBrowserObservation,
    result: RepairBrowserEvidenceResult,
    exit_code: u8,
}

fn repair_browser_evidence_payload_sha256(
    evidence: &RepairBrowserEvidence,
) -> Result<String, RepairContractError> {
    let payload = RepairBrowserEvidencePayload {
        schema_version: &evidence.schema_version,
        change_receipt_sha256: &evidence.change_receipt_sha256,
        check_id: &evidence.check_id,
        profile: evidence.profile,
        runner: &evidence.runner,
        profile_inputs: &evidence.profile_inputs,
        browser: &evidence.browser,
        observation: &evidence.observation,
        result: evidence.result,
        exit_code: evidence.exit_code,
    };
    serde_json::to_vec(&payload)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(|error| RepairContractError::new(format!("cannot hash browser evidence: {error}")))
}

/// Parses and validates canonical fixed-profile browser evidence.
///
/// # Errors
///
/// Returns [`RepairContractError`] for malformed, noncanonical, drifted, or unsupported input.
pub fn parse_repair_browser_evidence(
    source: &[u8],
) -> Result<RepairBrowserEvidence, RepairContractError> {
    if source.len() > MAX_DOCUMENT_BYTES {
        return Err(RepairContractError::new("browser evidence exceeds 16 MiB"));
    }
    let evidence: RepairBrowserEvidence = serde_json::from_slice(source).map_err(|error| {
        RepairContractError::new(format!("invalid browser evidence JSON: {error}"))
    })?;
    evidence.validate()?;
    if evidence.to_json_pretty()?.as_bytes() != source {
        return Err(RepairContractError::new(
            "browser evidence bytes are not canonical pretty JSON",
        ));
    }
    Ok(evidence)
}

/// Before/after digest pair for one source file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairSourceTransition {
    file: String,
    before_bytes: usize,
    before_sha256: String,
    after_bytes: usize,
    after_sha256: String,
}

impl RepairSourceTransition {
    /// Returns the portable logical source path.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    fn validate(&self) -> Result<(), RepairContractError> {
        validate_logical_path("source.file", &self.file)?;
        if self.before_bytes > MAX_SOURCE_BYTES || self.after_bytes > MAX_SOURCE_BYTES {
            return Err(RepairContractError::new(
                "source transition exceeds the 16 MiB per-file boundary",
            ));
        }
        validate_sha256("source.beforeSha256", &self.before_sha256)?;
        validate_sha256("source.afterSha256", &self.after_sha256)
    }
}

/// Exact edit after it has been reconciled with a source snapshot and finding suggestion.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairPlannedEdit {
    finding_fingerprint: String,
    finding_code: String,
    suggestion_id: String,
    suggestion_rank: u16,
    suggestion_scope: String,
    file: String,
    byte_start: usize,
    byte_end: usize,
    removed: String,
    removed_sha256: String,
    replacement: String,
    replacement_sha256: String,
}

impl RepairPlannedEdit {
    fn validate(&self) -> Result<(), RepairContractError> {
        validate_fingerprint("edit.findingFingerprint", &self.finding_fingerprint)?;
        validate_finding_code(&self.finding_code)?;
        validate_slug("edit.suggestionId", &self.suggestion_id)?;
        if self.suggestion_rank == 0 {
            return Err(RepairContractError::new(
                "edit.suggestionRank must be one-based",
            ));
        }
        validate_slug("edit.suggestionScope", &self.suggestion_scope)?;
        validate_logical_path("edit.file", &self.file)?;
        if self.byte_end < self.byte_start {
            return Err(RepairContractError::new(
                "edit.byteEnd precedes edit.byteStart",
            ));
        }
        if self.removed.len() != self.byte_end - self.byte_start {
            return Err(RepairContractError::new(
                "edit.removed UTF-8 bytes do not match its source range",
            ));
        }
        if self.removed.len() > MAX_SOURCE_BYTES || self.replacement.len() > MAX_REPLACEMENT_BYTES {
            return Err(RepairContractError::new(
                "planned edit exceeds defensive byte limits",
            ));
        }
        if self.removed.contains('\0') || self.replacement.contains('\0') {
            return Err(RepairContractError::new("planned edit contains a NUL byte"));
        }
        if sha256_hex(self.removed.as_bytes()) != self.removed_sha256 {
            return Err(RepairContractError::new(
                "edit.removedSha256 does not match edit.removed",
            ));
        }
        if sha256_hex(self.replacement.as_bytes()) != self.replacement_sha256 {
            return Err(RepairContractError::new(
                "edit.replacementSha256 does not match edit.replacement",
            ));
        }
        Ok(())
    }
}

/// Derived changed-file and byte counts checked against the authorized budget.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairChangeSummary {
    files: u32,
    edits: u32,
    inserted_bytes: u64,
    removed_bytes: u64,
}

/// Exact deterministic patch projection over planned UTF-8 byte edits.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairPatch {
    format: String,
    sha256: String,
    text: String,
}

impl RepairPatch {
    fn validate(&self) -> Result<(), RepairContractError> {
        if self.format != "pliegocss-byte-edits/1" {
            return Err(RepairContractError::new(
                "patch.format must be `pliegocss-byte-edits/1`",
            ));
        }
        validate_sha256("patch.sha256", &self.sha256)?;
        if self.text.is_empty() || self.text.len() > MAX_DOCUMENT_BYTES {
            return Err(RepairContractError::new(
                "patch.text must be non-empty and at most 16 MiB",
            ));
        }
        if sha256_hex(self.text.as_bytes()) != self.sha256 {
            return Err(RepairContractError::new(
                "patch.sha256 does not match patch.text",
            ));
        }
        Ok(())
    }
}

/// Closed, integrity-bound, dry-run-only change plan.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairPlanDocument {
    schema_version: String,
    plan_sha256: String,
    mode: String,
    tool: RepairTool,
    finding_document: RepairArtifactIdentity,
    change_budget: RepairChangeBudget,
    change_summary: RepairChangeSummary,
    risk: FindingRisk,
    required_checks: Vec<String>,
    sources: Vec<RepairSourceTransition>,
    edits: Vec<RepairPlannedEdit>,
    patch: RepairPatch,
}

impl RepairPlanDocument {
    /// Returns the canonical plan payload hash.
    #[must_use]
    pub fn plan_sha256(&self) -> &str {
        &self.plan_sha256
    }

    /// Returns source transitions in canonical path order.
    #[must_use]
    pub fn sources(&self) -> &[RepairSourceTransition] {
        &self.sources
    }

    /// Serializes the validated plan with one trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`RepairContractError`] when validation or serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, RepairContractError> {
        self.validate()?;
        let mut output = serde_json::to_string_pretty(self)
            .map_err(|error| RepairContractError::new(format!("cannot serialize plan: {error}")))?;
        output.push('\n');
        if output.len() > MAX_DOCUMENT_BYTES {
            return Err(RepairContractError::new("repair plan exceeds 16 MiB"));
        }
        Ok(output)
    }

    /// Renders the bounded summary plus exact byte-edit patch.
    ///
    /// # Errors
    ///
    /// Returns [`RepairContractError`] when the plan is invalid.
    pub fn to_human(&self) -> Result<String, RepairContractError> {
        self.validate()?;
        Ok(format!(
            "Plan: sha256:{}\nMode: {}\nRisk: {}\nChanges: {} file(s), {} edit(s), {} inserted byte(s), {} removed byte(s)\nRequired checks: {}\n{}",
            self.plan_sha256,
            self.mode,
            self.risk.as_str(),
            self.change_summary.files,
            self.change_summary.edits,
            self.change_summary.inserted_bytes,
            self.change_summary.removed_bytes,
            self.required_checks.join(", "),
            self.patch.text
        ))
    }

    fn validate(&self) -> Result<(), RepairContractError> {
        if self.schema_version != REPAIR_PLAN_SCHEMA_VERSION {
            return Err(RepairContractError::new(format!(
                "unsupported repair plan schema `{}`; expected {REPAIR_PLAN_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        validate_sha256("plan.planSha256", &self.plan_sha256)?;
        if self.mode != "dry-run-only" {
            return Err(RepairContractError::new(
                "plan.mode must be `dry-run-only` in schema 1.0.0",
            ));
        }
        self.tool.validate()?;
        self.finding_document.validate("findingDocument")?;
        self.change_budget.validate()?;
        if self.risk != FindingRisk::Low {
            return Err(RepairContractError::new(
                "schema 1.0.0 accepts only low-risk repair plans",
            ));
        }
        validate_checks(&self.required_checks)?;
        if self.sources.is_empty() || self.sources.len() > MAX_FILES {
            return Err(RepairContractError::new(
                "plan.sources must contain between 1 and 65,535 files",
            ));
        }
        for source in &self.sources {
            source.validate()?;
        }
        validate_source_order(&self.sources)?;
        if self.edits.is_empty() || self.edits.len() > MAX_EDITS {
            return Err(RepairContractError::new(
                "plan.edits must contain between 1 and 65,535 edits",
            ));
        }
        for edit in &self.edits {
            edit.validate()?;
        }
        validate_planned_edit_order(&self.edits)?;
        let summary = summarize_edits(&self.edits)?;
        if summary != self.change_summary {
            return Err(RepairContractError::new(
                "plan.changeSummary does not match plan.edits",
            ));
        }
        enforce_budget(summary, self.change_budget)?;
        if usize::try_from(summary.files).ok() != Some(self.sources.len()) {
            return Err(RepairContractError::new(
                "plan.sources do not exactly cover edited files",
            ));
        }
        let source_files = self
            .sources
            .iter()
            .map(|source| source.file.as_str())
            .collect::<BTreeSet<_>>();
        if self
            .edits
            .iter()
            .any(|edit| !source_files.contains(edit.file.as_str()))
        {
            return Err(RepairContractError::new(
                "plan edit references a file absent from plan.sources",
            ));
        }
        let expected_patch = build_patch(&self.edits)?;
        if expected_patch != self.patch {
            return Err(RepairContractError::new(
                "plan.patch does not match plan.edits",
            ));
        }
        self.patch.validate()?;
        let expected_hash = plan_payload_sha256(self)?;
        if expected_hash != self.plan_sha256 {
            return Err(RepairContractError::new(format!(
                "plan.planSha256 `{}` does not match `{expected_hash}`",
                self.plan_sha256
            )));
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RepairPlanPayload<'a> {
    schema_version: &'a str,
    mode: &'a str,
    tool: &'a RepairTool,
    finding_document: &'a RepairArtifactIdentity,
    change_budget: RepairChangeBudget,
    change_summary: RepairChangeSummary,
    risk: FindingRisk,
    required_checks: &'a [String],
    sources: &'a [RepairSourceTransition],
    edits: &'a [RepairPlannedEdit],
    patch: &'a RepairPatch,
}

fn plan_payload_sha256(plan: &RepairPlanDocument) -> Result<String, RepairContractError> {
    let bytes = serde_json::to_vec(&RepairPlanPayload {
        schema_version: &plan.schema_version,
        mode: &plan.mode,
        tool: &plan.tool,
        finding_document: &plan.finding_document,
        change_budget: plan.change_budget,
        change_summary: plan.change_summary,
        risk: plan.risk,
        required_checks: &plan.required_checks,
        sources: &plan.sources,
        edits: &plan.edits,
        patch: &plan.patch,
    })
    .map_err(|error| RepairContractError::new(format!("cannot hash repair plan: {error}")))?;
    Ok(sha256_hex(&bytes))
}

/// Builds a canonical dry-run-only repair plan from exact finding, proposal, and source bytes.
///
/// # Errors
///
/// Returns [`RepairContractError`] unless every edit references a verified, unexcepted, low-risk
/// suggestion, stays inside its finding range, matches the source snapshot, remains non-overlapping,
/// and fits the declared change budget.
pub fn build_repair_plan(
    tool: RepairTool,
    finding_file: &str,
    finding_bytes: &[u8],
    proposal: &RepairProposalDocument,
    sources: &BTreeMap<String, Vec<u8>>,
) -> Result<RepairPlanDocument, RepairContractError> {
    proposal.validate()?;
    validate_logical_path("findingDocument.file", finding_file)?;
    if finding_bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(RepairContractError::new("finding document exceeds 16 MiB"));
    }
    let finding_sha256 = sha256_hex(finding_bytes);
    if finding_sha256 != proposal.finding_document_sha256 {
        return Err(RepairContractError::new(format!(
            "proposal finding-document hash `{}` does not match `{finding_sha256}`",
            proposal.finding_document_sha256
        )));
    }
    let finding_document = parse_finding_document(finding_bytes)
        .map_err(|error| RepairContractError::new(format!("invalid finding document: {error}")))?;
    let proposed_files = proposal.files();
    if proposed_files.len() != sources.len() {
        return Err(RepairContractError::new(
            "source snapshot set must exactly equal proposal edit files",
        ));
    }
    let planned_edits = reconcile_edits(&finding_document, proposal, sources)?;
    validate_planned_edit_order(&planned_edits)?;
    let summary = summarize_edits(&planned_edits)?;
    enforce_budget(summary, proposal.change_budget)?;
    let transitions = build_source_transitions(&proposed_files, sources, &planned_edits)?;
    let patch = build_patch(&planned_edits)?;
    let mut plan = RepairPlanDocument {
        schema_version: REPAIR_PLAN_SCHEMA_VERSION.into(),
        plan_sha256: String::new(),
        mode: "dry-run-only".into(),
        tool,
        finding_document: RepairArtifactIdentity {
            file: finding_file.into(),
            bytes: finding_bytes.len(),
            sha256: finding_sha256,
        },
        change_budget: proposal.change_budget,
        change_summary: summary,
        risk: FindingRisk::Low,
        required_checks: proposal.required_checks.clone(),
        sources: transitions,
        edits: planned_edits,
        patch,
    };
    plan.plan_sha256 = plan_payload_sha256(&plan)?;
    plan.validate()?;
    Ok(plan)
}

fn reconcile_edits(
    finding_document: &FindingDocument,
    proposal: &RepairProposalDocument,
    sources: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<RepairPlannedEdit>, RepairContractError> {
    let findings = finding_document
        .findings()
        .iter()
        .map(|finding| (finding.fingerprint(), finding))
        .collect::<BTreeMap<_, _>>();
    let mut planned_edits = Vec::with_capacity(proposal.edits.len());
    for edit in &proposal.edits {
        let finding = findings
            .get(edit.finding_fingerprint.as_str())
            .ok_or_else(|| {
                RepairContractError::new(format!(
                    "edit references unknown finding `{}`",
                    edit.finding_fingerprint
                ))
            })?;
        let suggestion = resolve_suggestion(finding, &edit.suggestion_id)?;
        validate_edit_authority(finding, suggestion, edit, &proposal.required_checks)?;
        let source = sources.get(&edit.file).ok_or_else(|| {
            RepairContractError::new(format!("missing source snapshot `{}`", edit.file))
        })?;
        if source.len() > MAX_SOURCE_BYTES {
            return Err(RepairContractError::new(format!(
                "source snapshot `{}` exceeds 16 MiB",
                edit.file
            )));
        }
        let source_text = std::str::from_utf8(source).map_err(|error| {
            RepairContractError::new(format!(
                "source snapshot `{}` is not UTF-8: {error}",
                edit.file
            ))
        })?;
        if source_text.contains('\0') {
            return Err(RepairContractError::new(format!(
                "source snapshot `{}` contains a NUL byte",
                edit.file
            )));
        }
        let removed = source_text
            .get(edit.byte_start..edit.byte_end)
            .ok_or_else(|| {
                RepairContractError::new(format!(
                    "edit range {}..{} is not on UTF-8 boundaries inside `{}`",
                    edit.byte_start, edit.byte_end, edit.file
                ))
            })?;
        planned_edits.push(RepairPlannedEdit {
            finding_fingerprint: edit.finding_fingerprint.clone(),
            finding_code: finding.code().to_owned(),
            suggestion_id: suggestion.id().to_owned(),
            suggestion_rank: suggestion.rank(),
            suggestion_scope: suggestion.scope().to_owned(),
            file: edit.file.clone(),
            byte_start: edit.byte_start,
            byte_end: edit.byte_end,
            removed: removed.to_owned(),
            removed_sha256: sha256_hex(removed.as_bytes()),
            replacement: edit.replacement.clone(),
            replacement_sha256: sha256_hex(edit.replacement.as_bytes()),
        });
    }
    Ok(planned_edits)
}

fn build_source_transitions(
    proposed_files: &[&str],
    sources: &BTreeMap<String, Vec<u8>>,
    planned_edits: &[RepairPlannedEdit],
) -> Result<Vec<RepairSourceTransition>, RepairContractError> {
    let mut transitions = Vec::with_capacity(sources.len());
    for (file, source) in sources {
        if !proposed_files.contains(&file.as_str()) {
            return Err(RepairContractError::new(format!(
                "unexpected source snapshot `{file}`"
            )));
        }
        let after = apply_file_edits(file, source, planned_edits)?;
        transitions.push(RepairSourceTransition {
            file: file.clone(),
            before_bytes: source.len(),
            before_sha256: sha256_hex(source),
            after_bytes: after.len(),
            after_sha256: sha256_hex(&after),
        });
    }
    Ok(transitions)
}

fn resolve_suggestion<'a>(
    finding: &'a Finding,
    suggestion_id: &str,
) -> Result<&'a FindingSuggestion, RepairContractError> {
    finding
        .suggestions()
        .iter()
        .find(|suggestion| suggestion.id() == suggestion_id)
        .ok_or_else(|| {
            RepairContractError::new(format!(
                "finding `{}` has no suggestion `{suggestion_id}`",
                finding.fingerprint()
            ))
        })
}

fn validate_edit_authority(
    finding: &Finding,
    suggestion: &FindingSuggestion,
    edit: &RepairProposalEdit,
    required_checks: &[String],
) -> Result<(), RepairContractError> {
    if finding.verification() != FindingVerification::Verified {
        return Err(RepairContractError::new(format!(
            "finding `{}` is not verified",
            finding.fingerprint()
        )));
    }
    if finding.is_excepted() {
        return Err(RepairContractError::new(format!(
            "finding `{}` has a reviewed exception",
            finding.fingerprint()
        )));
    }
    if suggestion.risk() != FindingRisk::Low {
        return Err(RepairContractError::new(format!(
            "suggestion `{}` is {}; schema 1.0.0 accepts only low-risk edits",
            suggestion.id(),
            suggestion.risk().as_str()
        )));
    }
    for prerequisite in suggestion.prerequisites() {
        if required_checks.binary_search(prerequisite).is_err() {
            return Err(RepairContractError::new(format!(
                "suggestion `{}` prerequisite `{prerequisite}` is absent from requiredChecks",
                suggestion.id()
            )));
        }
    }
    let source = finding.source().ok_or_else(|| {
        RepairContractError::new(format!(
            "finding `{}` has no exact source range",
            finding.fingerprint()
        ))
    })?;
    if source.file() != edit.file
        || edit.byte_start < source.byte_start()
        || edit.byte_end > source.byte_end()
    {
        return Err(RepairContractError::new(format!(
            "edit {}:{}..{} escapes finding source {}:{}..{}",
            edit.file,
            edit.byte_start,
            edit.byte_end,
            source.file(),
            source.byte_start(),
            source.byte_end()
        )));
    }
    Ok(())
}

/// Parses and validates a canonical repair-plan document.
///
/// # Errors
///
/// Returns [`RepairContractError`] for malformed JSON, an unsupported schema, hash drift,
/// noncanonical collections, invalid edits, summaries, budgets, or patch bytes.
pub fn parse_repair_plan(source: &[u8]) -> Result<RepairPlanDocument, RepairContractError> {
    if source.len() > MAX_DOCUMENT_BYTES {
        return Err(RepairContractError::new("repair plan exceeds 16 MiB"));
    }
    let plan: RepairPlanDocument = serde_json::from_slice(source)
        .map_err(|error| RepairContractError::new(format!("invalid repair plan JSON: {error}")))?;
    plan.validate()?;
    Ok(plan)
}

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
    const fn as_str(self) -> &'static str {
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
    schema_version: String,
    plan_sha256: String,
    state: RepairDryRunState,
    files: u32,
    edits: u32,
    patch_sha256: String,
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
struct RepairReceiptCheck {
    id: String,
    status: String,
}

/// Deterministic receipt for an explicitly authorized source transition.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairChangeReceipt {
    schema_version: String,
    receipt_sha256: String,
    plan_sha256: String,
    patch_sha256: String,
    authorization: String,
    state: RepairChangeState,
    result: String,
    finding_document: RepairArtifactIdentity,
    change_summary: RepairChangeSummary,
    sources: Vec<RepairSourceTransition>,
    checks: Vec<RepairReceiptCheck>,
    browser_evidence: String,
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

    fn validate(&self) -> Result<(), RepairContractError> {
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
    schema_version: &'a str,
    plan_sha256: &'a str,
    patch_sha256: &'a str,
    authorization: &'a str,
    state: RepairChangeState,
    result: &'a str,
    finding_document: &'a RepairArtifactIdentity,
    change_summary: RepairChangeSummary,
    sources: &'a [RepairSourceTransition],
    checks: &'a [RepairReceiptCheck],
    browser_evidence: &'a str,
}

fn receipt_payload_sha256(receipt: &RepairChangeReceipt) -> Result<String, RepairContractError> {
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
    id: String,
    kind: RepairCheckKind,
    source: String,
    compatibility_profile: CompatibilityProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    budget_policy: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    budget_subjects: Vec<BudgetSubject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    test_evidence: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workspace_manifest: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lockfile: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    browser_evidence_file: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    browser_profile_inputs: Vec<RepairArtifactIdentity>,
}

impl RepairCheckDefinition {
    #[allow(clippy::too_many_lines)]
    fn validate(&self, schema_version: &str) -> Result<(), RepairContractError> {
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

    fn require_no_supplementary_configuration(
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
    schema_version: String,
    checks: Vec<RepairCheckDefinition>,
    browser_evidence: RepairBrowserRequirement,
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

    fn validate(&self) -> Result<(), RepairContractError> {
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
struct RepairVerificationCheck {
    id: String,
    kind: RepairCheckKind,
    status: RepairVerificationCheckStatus,
    source: RepairArtifactIdentity,
    compatibility_profile: CompatibilityProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    budget_policy: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    test_evidence: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    browser_evidence_file: Option<RepairArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    browser_profile_inputs: Vec<RepairArtifactIdentity>,
    finding_document_bytes: usize,
    finding_document_sha256: String,
    finding_count: usize,
}

impl RepairVerificationCheck {
    #[allow(clippy::too_many_lines)]
    fn validate(&self, schema_version: &str) -> Result<(), RepairContractError> {
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
    schema_version: String,
    receipt_sha256: String,
    change_receipt: RepairArtifactIdentity,
    change_receipt_sha256: String,
    check_policy: RepairArtifactIdentity,
    result: RepairVerificationResult,
    checks: Vec<RepairVerificationCheck>,
    browser_evidence: RepairVerificationBrowserEvidence,
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

    fn validate(&self) -> Result<(), RepairContractError> {
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
    schema_version: &'a str,
    change_receipt: &'a RepairArtifactIdentity,
    change_receipt_sha256: &'a str,
    check_policy: &'a RepairArtifactIdentity,
    result: RepairVerificationResult,
    checks: &'a [RepairVerificationCheck],
    browser_evidence: RepairVerificationBrowserEvidence,
}

fn verification_receipt_payload_sha256(
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

fn derive_verification_result(
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

/// In-memory source transition and receipt prepared for one atomic publisher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedRepairApplication {
    receipt: RepairChangeReceipt,
    sources: BTreeMap<String, Vec<u8>>,
}

impl PreparedRepairApplication {
    /// Returns the receipt that must be published atomically with changed source files.
    #[must_use]
    pub const fn receipt(&self) -> &RepairChangeReceipt {
        &self.receipt
    }

    /// Returns exact after bytes keyed by canonical logical source path.
    #[must_use]
    pub const fn sources(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.sources
    }
}

/// Receipt and changed-destination count returned by atomic repair publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedRepairApplication {
    receipt: RepairChangeReceipt,
    receipt_bytes: Vec<u8>,
    changed_destinations: usize,
}

impl PublishedRepairApplication {
    /// Returns the exact published or preserved receipt.
    #[must_use]
    pub const fn receipt(&self) -> &RepairChangeReceipt {
        &self.receipt
    }

    /// Returns its exact canonical JSON bytes.
    #[must_use]
    pub fn receipt_bytes(&self) -> &[u8] {
        &self.receipt_bytes
    }

    /// Returns the number of source/receipt destinations whose bytes changed.
    #[must_use]
    pub const fn changed_destinations(&self) -> usize {
        self.changed_destinations
    }

    /// Renders canonical JSON or the concise human change report.
    #[must_use]
    pub fn render(&self, format: RepairCliFormat) -> String {
        match format {
            RepairCliFormat::Json => String::from_utf8_lossy(&self.receipt_bytes).into_owned(),
            RepairCliFormat::Text => format!(
                "{}Published destinations: {}\n",
                self.receipt.to_human(),
                self.changed_destinations
            ),
        }
    }
}

static REPAIR_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Resolves and validates a project-relative Change Receipt destination.
///
/// Parent components must already exist and may not traverse symbolic links, junctions, or reparse
/// points. The destination may be missing or a regular non-link file and may not alias any protected
/// input or repair source under portable ASCII case folding.
///
/// # Errors
///
/// Returns [`RepairContractError`] for unsafe components, missing/link-like parents, invalid final
/// file types, reserved lock names, or physical aliases.
pub fn resolve_repair_receipt_path<'a>(
    receipt: &Path,
    protected_inputs: impl IntoIterator<Item = &'a Path>,
    source_paths: &BTreeMap<String, PathBuf>,
) -> Result<PathBuf, RepairContractError> {
    if receipt.as_os_str().is_empty()
        || receipt.is_absolute()
        || receipt
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(RepairContractError::new(
            "change receipt must be a project-relative path without `.` or `..` components",
        ));
    }
    if receipt
        .file_name()
        .is_some_and(|name| name == ".pliegocss-repair.lock")
    {
        return Err(RepairContractError::new(
            "change receipt may not use `.pliegocss-repair.lock`",
        ));
    }
    let cwd = env::current_dir().map_err(|error| {
        RepairContractError::new(format!("cannot resolve change receipt: {error}"))
    })?;
    let mut current = cwd.clone();
    let components = receipt.components().collect::<Vec<_>>();
    for component in &components[..components.len() - 1] {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            RepairContractError::new(format!(
                "cannot inspect change receipt parent `{}`: {error}",
                current.display()
            ))
        })?;
        if is_repair_link_like(&metadata) || !metadata.is_dir() {
            return Err(RepairContractError::new(format!(
                "change receipt parent `{}` is not a regular non-link directory",
                current.display()
            )));
        }
    }
    let resolved = cwd.join(receipt);
    match fs::symlink_metadata(&resolved) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(RepairContractError::new(format!(
                "cannot inspect change receipt `{}`: {error}",
                resolved.display()
            )));
        }
        Ok(metadata) if is_repair_link_like(&metadata) || !metadata.is_file() => {
            return Err(RepairContractError::new(format!(
                "change receipt `{}` is not a regular non-link file",
                resolved.display()
            )));
        }
        Ok(_) => {}
    }
    let receipt_key = repair_path_key(&resolved)?;
    for input in protected_inputs {
        if receipt_key == repair_path_key(input)? {
            return Err(RepairContractError::new(format!(
                "change receipt `{}` aliases protected input `{}`",
                receipt.display(),
                input.display()
            )));
        }
    }
    for source in source_paths.values() {
        if receipt_key == repair_path_key(source)? {
            return Err(RepairContractError::new(format!(
                "change receipt `{}` aliases repair source `{}`",
                receipt.display(),
                source.display()
            )));
        }
    }
    Ok(resolved)
}

/// Resolves a project-relative receipt and atomically publishes a prepared repair.
///
/// # Errors
///
/// Returns [`RepairContractError`] for receipt-path or publication failure.
pub fn publish_repair_application_checked<'a>(
    prepared: &PreparedRepairApplication,
    source_root: &Path,
    source_paths: &BTreeMap<String, PathBuf>,
    verified_sources: &BTreeMap<String, Vec<u8>>,
    receipt: &Path,
    protected_inputs: impl IntoIterator<Item = &'a Path>,
) -> Result<PublishedRepairApplication, RepairContractError> {
    let receipt = resolve_repair_receipt_path(receipt, protected_inputs, source_paths)?;
    publish_repair_application(
        prepared,
        source_root,
        source_paths,
        verified_sources,
        &receipt,
    )
}

/// Verifies, prepares, resolves, and atomically publishes one explicitly authorized repair.
///
/// # Errors
///
/// Returns [`RepairContractError`] for any authorization, artifact, source, path, or publication
/// failure.
#[allow(clippy::too_many_arguments)]
pub fn apply_repair_plan_checked<'a>(
    plan: &RepairPlanDocument,
    finding_file: &str,
    finding_bytes: &[u8],
    verified_sources: &BTreeMap<String, Vec<u8>>,
    authorization: &str,
    source_root: &Path,
    source_paths: &BTreeMap<String, PathBuf>,
    receipt: &Path,
    protected_inputs: impl IntoIterator<Item = &'a Path>,
) -> Result<PublishedRepairApplication, RepairContractError> {
    let prepared = prepare_repair_application(
        plan,
        finding_file,
        finding_bytes,
        verified_sources,
        authorization,
    )?;
    publish_repair_application_checked(
        &prepared,
        source_root,
        source_paths,
        verified_sources,
        receipt,
        protected_inputs,
    )
}

fn repair_path_key(path: &Path) -> Result<String, RepairContractError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| RepairContractError::new(format!("cannot resolve path: {error}")))?
            .join(path)
    };
    let name = absolute
        .file_name()
        .ok_or_else(|| RepairContractError::new(format!("invalid path `{}`", path.display())))?;
    let parent = absolute
        .parent()
        .ok_or_else(|| RepairContractError::new(format!("invalid path `{}`", path.display())))?;
    let parent = fs::canonicalize(parent).map_err(|error| {
        RepairContractError::new(format!(
            "cannot canonicalize path parent `{}`: {error}",
            parent.display()
        ))
    })?;
    Ok(parent.join(name).to_string_lossy().to_ascii_lowercase())
}

fn is_repair_link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Atomically publishes prepared source bytes and their Change Receipt with rollback.
///
/// `source_paths` must map every logical prepared source to its already validated physical file;
/// `verified_sources` must contain the exact bytes used by [`prepare_repair_application`]. A
/// persistent `.pliegocss-repair.lock` under `source_root` coordinates cooperating writers. Source
/// permissions are preserved. An existing receipt for the same plan is preserved only when all
/// sources are already applied; any other receipt collision fails closed.
///
/// # Errors
///
/// Returns [`RepairContractError`] for path/set mismatch, file-type or byte drift, lock failure,
/// receipt collision, staging failure, or a publication/rollback failure.
pub fn publish_repair_application(
    prepared: &PreparedRepairApplication,
    source_root: &Path,
    source_paths: &BTreeMap<String, PathBuf>,
    verified_sources: &BTreeMap<String, Vec<u8>>,
    receipt_path: &Path,
) -> Result<PublishedRepairApplication, RepairContractError> {
    publish_repair_application_with(
        prepared,
        source_root,
        source_paths,
        verified_sources,
        receipt_path,
        |_, temporary, destination| fs::rename(temporary, destination),
    )
}

fn publish_repair_application_with(
    prepared: &PreparedRepairApplication,
    source_root: &Path,
    source_paths: &BTreeMap<String, PathBuf>,
    verified_sources: &BTreeMap<String, Vec<u8>>,
    receipt_path: &Path,
    publish: impl FnMut(usize, &Path, &Path) -> std::io::Result<()>,
) -> Result<PublishedRepairApplication, RepairContractError> {
    validate_publication_paths(
        prepared,
        source_root,
        source_paths,
        verified_sources,
        receipt_path,
    )?;
    let _lock = acquire_repair_lock(source_root)?;
    for (file, path) in source_paths {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            RepairContractError::new(format!(
                "cannot re-inspect repair source `{}` under lock: {error}",
                path.display()
            ))
        })?;
        if is_repair_link_like(&metadata) || !metadata.is_file() {
            return Err(RepairContractError::new(format!(
                "repair source `{}` changed file type under lock",
                path.display()
            )));
        }
        let actual = fs::read(path).map_err(|error| {
            RepairContractError::new(format!(
                "cannot re-read repair source `{}` under lock: {error}",
                path.display()
            ))
        })?;
        if verified_sources.get(file) != Some(&actual) {
            return Err(RepairContractError::new(format!(
                "repair source `{}` changed after plan verification",
                path.display()
            )));
        }
    }

    let existing_receipt = read_existing_receipt(receipt_path)?;
    let (receipt, receipt_bytes) = if let Some((receipt, bytes)) = existing_receipt {
        if receipt.plan_sha256 != prepared.receipt.plan_sha256 {
            return Err(RepairContractError::new(
                "change receipt destination already contains a different plan",
            ));
        }
        if prepared.receipt.state != RepairChangeState::AlreadyApplied {
            return Err(RepairContractError::new(
                "change receipt records this plan but sources returned to the before state",
            ));
        }
        (receipt, bytes)
    } else {
        let bytes = prepared.receipt.to_json_pretty()?.into_bytes();
        (prepared.receipt.clone(), bytes)
    };

    let mut writes = Vec::new();
    for (file, path) in source_paths {
        let after = prepared.sources.get(file).ok_or_else(|| {
            RepairContractError::new(format!("missing after bytes for repair source `{file}`"))
        })?;
        if verified_sources.get(file) != Some(after) {
            writes.push(prepare_source_write(path, after)?);
        }
    }
    if !receipt_path.exists() {
        writes.push(prepare_write(receipt_path, &receipt_bytes)?);
    }
    let changed_destinations = writes.len();
    commit_writes(writes, publish)?;
    Ok(PublishedRepairApplication {
        receipt,
        receipt_bytes,
        changed_destinations,
    })
}

fn validate_publication_paths(
    prepared: &PreparedRepairApplication,
    source_root: &Path,
    source_paths: &BTreeMap<String, PathBuf>,
    verified_sources: &BTreeMap<String, Vec<u8>>,
    receipt_path: &Path,
) -> Result<(), RepairContractError> {
    if source_paths.len() != verified_sources.len()
        || verified_sources.len() != prepared.sources.len()
        || source_paths.keys().ne(verified_sources.keys())
        || source_paths.keys().ne(prepared.sources.keys())
    {
        return Err(RepairContractError::new(
            "repair publication source sets do not match",
        ));
    }
    let root = fs::canonicalize(source_root).map_err(|error| {
        RepairContractError::new(format!(
            "cannot canonicalize repair source root `{}`: {error}",
            source_root.display()
        ))
    })?;
    let mut destinations = BTreeSet::new();
    for path in source_paths
        .values()
        .map(PathBuf::as_path)
        .chain(std::iter::once(receipt_path))
    {
        if path
            .file_name()
            .is_some_and(|name| name == ".pliegocss-repair.lock")
        {
            return Err(RepairContractError::new(
                "repair destinations may not use `.pliegocss-repair.lock`",
            ));
        }
        let parent = path.parent().ok_or_else(|| {
            RepairContractError::new(format!("invalid repair destination `{}`", path.display()))
        })?;
        let name = path.file_name().ok_or_else(|| {
            RepairContractError::new(format!("invalid repair destination `{}`", path.display()))
        })?;
        let parent = fs::canonicalize(parent).map_err(|error| {
            RepairContractError::new(format!(
                "cannot canonicalize repair destination parent `{}`: {error}",
                parent.display()
            ))
        })?;
        let key = parent.join(name).to_string_lossy().to_ascii_lowercase();
        if !destinations.insert(key) {
            return Err(RepairContractError::new(format!(
                "duplicate repair destination `{}`",
                path.display()
            )));
        }
    }
    for path in source_paths.values() {
        let canonical = fs::canonicalize(path).map_err(|error| {
            RepairContractError::new(format!(
                "cannot canonicalize repair source `{}`: {error}",
                path.display()
            ))
        })?;
        if !canonical.starts_with(&root) {
            return Err(RepairContractError::new(format!(
                "repair source `{}` escapes source root `{}`",
                path.display(),
                root.display()
            )));
        }
    }
    Ok(())
}

fn read_existing_receipt(
    path: &Path,
) -> Result<Option<(RepairChangeReceipt, Vec<u8>)>, RepairContractError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(RepairContractError::new(format!(
            "cannot inspect change receipt `{}`: {error}",
            path.display()
        ))),
        Ok(metadata) if is_repair_link_like(&metadata) || !metadata.is_file() => {
            Err(RepairContractError::new(format!(
                "change receipt `{}` is not a regular non-link file",
                path.display()
            )))
        }
        Ok(_) => {
            let bytes = fs::read(path).map_err(|error| {
                RepairContractError::new(format!(
                    "cannot read change receipt `{}`: {error}",
                    path.display()
                ))
            })?;
            let receipt = parse_repair_change_receipt(&bytes).map_err(|error| {
                RepairContractError::new(format!("invalid existing change receipt: {error}"))
            })?;
            Ok(Some((receipt, bytes)))
        }
    }
}

struct RepairLock(fs::File);

impl Drop for RepairLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

fn acquire_repair_lock(root: &Path) -> Result<RepairLock, RepairContractError> {
    let path = root.join(".pliegocss-repair.lock");
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(RepairContractError::new(format!(
                "cannot inspect repair lock `{}`: {error}",
                path.display()
            )));
        }
        Ok(metadata) if is_repair_link_like(&metadata) || !metadata.is_file() => {
            return Err(RepairContractError::new(format!(
                "repair lock `{}` is not a regular non-link file",
                path.display()
            )));
        }
        Ok(_) => {}
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| {
            RepairContractError::new(format!(
                "cannot open repair lock `{}`: {error}",
                path.display()
            ))
        })?;
    file.try_lock_exclusive().map_err(|error| {
        RepairContractError::new(format!(
            "cannot acquire repair lock `{}`; another repair may be active: {error}",
            path.display()
        ))
    })?;
    Ok(RepairLock(file))
}

struct PreparedRepairWrite {
    destination: PathBuf,
    temporary: Option<PathBuf>,
}

impl Drop for PreparedRepairWrite {
    fn drop(&mut self) {
        if let Some(temporary) = &self.temporary {
            let _ = fs::remove_file(temporary);
        }
    }
}

fn prepare_write(
    destination: &Path,
    bytes: &[u8],
) -> Result<PreparedRepairWrite, RepairContractError> {
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            RepairContractError::new(format!(
                "invalid repair destination `{}`",
                destination.display()
            ))
        })?;
    for _ in 0..32 {
        let sequence = REPAIR_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary = destination.with_file_name(format!(
            ".{name}.pliego-repair-{}-{sequence}.tmp",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(mut file) => {
                file.write_all(bytes)
                    .and_then(|()| file.sync_all())
                    .map_err(|error| {
                        let _ = fs::remove_file(&temporary);
                        RepairContractError::new(format!(
                            "cannot prepare `{}` through `{}`: {error}",
                            destination.display(),
                            temporary.display()
                        ))
                    })?;
                return Ok(PreparedRepairWrite {
                    destination: destination.to_path_buf(),
                    temporary: Some(temporary),
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(RepairContractError::new(format!(
                    "cannot prepare `{}` through `{}`: {error}",
                    destination.display(),
                    temporary.display()
                )));
            }
        }
    }
    Err(RepairContractError::new(format!(
        "cannot allocate a temporary file beside `{}`",
        destination.display()
    )))
}

fn prepare_source_write(
    destination: &Path,
    bytes: &[u8],
) -> Result<PreparedRepairWrite, RepairContractError> {
    let permissions = fs::metadata(destination)
        .map_err(|error| {
            RepairContractError::new(format!(
                "cannot read repair source permissions `{}`: {error}",
                destination.display()
            ))
        })?
        .permissions();
    let write = prepare_write(destination, bytes)?;
    let temporary = write
        .temporary
        .as_deref()
        .ok_or_else(|| RepairContractError::new("prepared repair source has no temporary file"))?;
    fs::set_permissions(temporary, permissions).map_err(|error| {
        RepairContractError::new(format!(
            "cannot preserve repair source permissions `{}`: {error}",
            destination.display()
        ))
    })?;
    Ok(write)
}

fn commit_writes(
    mut writes: Vec<PreparedRepairWrite>,
    mut publish: impl FnMut(usize, &Path, &Path) -> std::io::Result<()>,
) -> Result<(), RepairContractError> {
    let mut backups = vec![None; writes.len()];
    for (index, write) in writes.iter().enumerate() {
        match move_to_backup(&write.destination) {
            Ok(backup) => backups[index] = backup,
            Err(error) => {
                return Err(rollback_writes(
                    &writes,
                    &mut backups,
                    0,
                    &error.to_string(),
                ));
            }
        }
    }
    for index in 0..writes.len() {
        let Some(temporary) = writes[index].temporary.take() else {
            return Err(rollback_writes(
                &writes,
                &mut backups,
                index,
                "prepared repair write lost its temporary path",
            ));
        };
        if let Err(error) = publish(index, &temporary, &writes[index].destination) {
            let _ = fs::remove_file(&temporary);
            let primary = format!(
                "cannot publish `{}`: {error}",
                writes[index].destination.display()
            );
            return Err(rollback_writes(&writes, &mut backups, index, &primary));
        }
    }
    for backup in backups.into_iter().flatten() {
        if let Err(error) = fs::remove_file(&backup) {
            eprintln!(
                "warning: repair published but backup `{}` could not be removed: {error}",
                backup.display()
            );
        }
    }
    Ok(())
}

fn move_to_backup(destination: &Path) -> Result<Option<PathBuf>, RepairContractError> {
    match fs::symlink_metadata(destination) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(RepairContractError::new(format!(
                "cannot inspect repair destination `{}`: {error}",
                destination.display()
            )));
        }
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(RepairContractError::new(format!(
                "repair destination `{}` is not a regular non-link file",
                destination.display()
            )));
        }
        Ok(_) => {}
    }
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            RepairContractError::new(format!(
                "invalid repair destination `{}`",
                destination.display()
            ))
        })?;
    for _ in 0..32 {
        let sequence = REPAIR_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let backup = destination.with_file_name(format!(
            ".{name}.pliego-repair-{}-{sequence}.bak",
            std::process::id()
        ));
        if backup.exists() {
            continue;
        }
        fs::rename(destination, &backup).map_err(|error| {
            RepairContractError::new(format!(
                "cannot stage `{}` through `{}`: {error}",
                destination.display(),
                backup.display()
            ))
        })?;
        return Ok(Some(backup));
    }
    Err(RepairContractError::new(format!(
        "cannot allocate a backup beside `{}`",
        destination.display()
    )))
}

fn rollback_writes(
    writes: &[PreparedRepairWrite],
    backups: &mut [Option<PathBuf>],
    published: usize,
    primary: &str,
) -> RepairContractError {
    let mut errors = Vec::new();
    for index in (0..writes.len()).rev() {
        let destination = &writes[index].destination;
        if index < published {
            if let Err(error) = fs::remove_file(destination) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    errors.push(format!(
                        "cannot remove `{}`: {error}",
                        destination.display()
                    ));
                }
            }
        }
        if let Some(backup) = backups[index].take() {
            if let Err(error) = fs::rename(&backup, destination) {
                errors.push(format!(
                    "cannot restore `{}` from `{}`: {error}",
                    destination.display(),
                    backup.display()
                ));
            }
        }
    }
    if errors.is_empty() {
        RepairContractError::new(format!(
            "{primary}; all previous source bytes were restored"
        ))
    } else {
        RepairContractError::new(format!(
            "{primary}; rollback also failed: {}",
            errors.join("; ")
        ))
    }
}

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

struct PreparedRepairVerification {
    receipt: RepairVerificationReceipt,
    finding_documents: BTreeMap<String, Vec<u8>>,
}

fn prepare_repair_verification(
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

fn validate_repair_after_sources(
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

fn execute_repair_check(
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

fn bounded_verification_reason(reason: &str) -> String {
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

/// One complete adjacent `FindingDocument` published for an executed verification check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedRepairFindingDocument {
    check_id: String,
    file: String,
    bytes: Vec<u8>,
    changed: bool,
}

impl PublishedRepairFindingDocument {
    /// Returns the exact check ID whose findings this document records.
    #[must_use]
    pub fn check_id(&self) -> &str {
        &self.check_id
    }

    /// Returns the deterministic project-relative adjacent artifact path.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the complete canonical `FindingDocument` bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns whether this invocation newly linked the document destination.
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.changed
    }
}

/// Published post-change receipt plus every complete adjacent `FindingDocument`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedRepairVerification {
    receipt: RepairVerificationReceipt,
    receipt_bytes: Vec<u8>,
    finding_documents: Vec<PublishedRepairFindingDocument>,
    changed: bool,
}

impl PublishedRepairVerification {
    /// Returns the validated Verification Receipt.
    #[must_use]
    pub const fn receipt(&self) -> &RepairVerificationReceipt {
        &self.receipt
    }

    /// Returns the exact canonical receipt bytes.
    #[must_use]
    pub fn receipt_bytes(&self) -> &[u8] {
        &self.receipt_bytes
    }

    /// Returns complete `FindingDocument` values in sorted check-ID order.
    #[must_use]
    pub fn finding_documents(&self) -> &[PublishedRepairFindingDocument] {
        &self.finding_documents
    }

    /// Returns whether the receipt destination was newly published.
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.changed
    }

    /// Renders canonical JSON or the bounded human summary.
    #[must_use]
    pub fn render(&self, format: RepairCliFormat) -> String {
        match format {
            RepairCliFormat::Json => String::from_utf8_lossy(&self.receipt_bytes).into_owned(),
            RepairCliFormat::Text => format!(
                "{}Finding documents: {}\nPublished: {}\n",
                self.receipt.to_human(),
                self.finding_documents.len(),
                if self.changed { "yes" } else { "no" }
            ),
        }
    }
}

/// Test evidence published by the explicit fixed-profile Rust runner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedRepairTestEvidence {
    evidence: RepairTestEvidence,
    evidence_bytes: Vec<u8>,
    changed: bool,
}

impl PublishedRepairTestEvidence {
    /// Returns the validated canonical test evidence.
    #[must_use]
    pub const fn evidence(&self) -> &RepairTestEvidence {
        &self.evidence
    }

    /// Returns its exact canonical JSON bytes.
    #[must_use]
    pub fn evidence_bytes(&self) -> &[u8] {
        &self.evidence_bytes
    }

    /// Returns whether this invocation newly published the evidence destination.
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.changed
    }

    /// Renders a bounded human summary after the child process output.
    #[must_use]
    pub fn to_human(&self) -> String {
        format!(
            "Test evidence: {}\nProfile: rust-workspace-all-targets\nPublished: {}\n",
            match self.evidence.result {
                RepairTestEvidenceResult::Passed => "passed",
                RepairTestEvidenceResult::Failed => "failed",
            },
            if self.changed { "yes" } else { "no" }
        )
    }
}

/// Runs the one fixed Rust workspace test profile and publishes canonical evidence.
///
/// The command is exactly `cargo test --workspace --all-targets --locked --offline`; no program,
/// argument, environment override, or shell fragment is accepted from a plan, policy, or caller.
/// The source root must be the current project directory so every project-relative control path
/// shares one fail-closed path boundary. The Change Receipt after sources, root `Cargo.toml`, and
/// root `Cargo.lock` are read before and after execution under the repair lock.
///
/// # Errors
///
/// Returns [`RepairContractError`] for unsafe paths, source/control drift, missing Cargo inputs,
/// process-launch failure, nonnumeric termination, evidence collision, or publication failure. A
/// nonzero Cargo exit is published as valid failed evidence rather than returned as a tool error.
#[allow(clippy::too_many_lines)]
pub fn run_repair_rust_tests_checked(
    change_receipt_path: &Path,
    source_root: &Path,
    check_id: &str,
    evidence_path: &Path,
) -> Result<PublishedRepairTestEvidence, RepairContractError> {
    validate_dotted_id("testEvidence.checkId", check_id)?;
    let (change_path, _) = resolve_repair_project_input(change_receipt_path, "Change Receipt")?;
    let change_bytes = read_repair_document(&change_path, "Change Receipt")?;
    let change_receipt = parse_repair_change_receipt(&change_bytes)?;
    let root = resolve_repair_verification_root(source_root)?;
    let cwd = env::current_dir()
        .and_then(fs::canonicalize)
        .map_err(|error| {
            RepairContractError::new(format!("cannot resolve current project: {error}"))
        })?;
    if root != cwd {
        return Err(RepairContractError::new(
            "fixed test runner requires source-root to resolve to the current project directory",
        ));
    }
    let source_paths = resolve_repair_verification_sources(&root, &change_receipt.sources)?;
    let manifest_path =
        resolve_repair_verification_file(&root, "Cargo.toml", "test workspace manifest")?;
    let lockfile_path =
        resolve_repair_verification_file(&root, "Cargo.lock", "test workspace lockfile")?;
    let output = resolve_repair_receipt_path(
        evidence_path,
        [&change_path, &manifest_path, &lockfile_path].map(PathBuf::as_path),
        &source_paths,
    )?;

    let _lock = acquire_repair_lock(&root)?;
    require_repair_document_unchanged(&change_path, &change_bytes, "Change Receipt")?;
    let sources = read_repair_verification_sources(&source_paths)?;
    validate_repair_after_sources(&change_receipt, &sources)?;
    let manifest_bytes = read_repair_document(&manifest_path, "test workspace manifest")?;
    let lockfile_bytes = read_repair_document(&lockfile_path, "test workspace lockfile")?;
    let toolchain = read_repair_test_toolchain(&root)?;

    let status = Command::new("cargo")
        .current_dir(&root)
        .args([
            "test",
            "--workspace",
            "--all-targets",
            "--locked",
            "--offline",
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TERM_COLOR", "never")
        .env("RUSTC", "rustc")
        .env(
            "CARGO_TARGET_DIR",
            root.join("target/pliego-css-agent-tests"),
        )
        .status()
        .map_err(|error| {
            RepairContractError::new(format!("cannot launch fixed Cargo test profile: {error}"))
        })?;
    let exit_code = status.code().ok_or_else(|| {
        RepairContractError::new("fixed Cargo test profile terminated without a numeric exit code")
    })?;
    let exit_code = u8::try_from(exit_code).map_err(|_| {
        RepairContractError::new(format!(
            "fixed Cargo test profile returned unsupported exit code `{exit_code}`"
        ))
    })?;

    require_repair_document_unchanged(&change_path, &change_bytes, "Change Receipt")?;
    let repeated_sources = read_repair_verification_sources(&source_paths)?;
    if repeated_sources != sources {
        return Err(RepairContractError::new(
            "repair sources changed while the fixed Cargo test profile was executing",
        ));
    }
    require_repair_document_unchanged(&manifest_path, &manifest_bytes, "test workspace manifest")?;
    require_repair_document_unchanged(&lockfile_path, &lockfile_bytes, "test workspace lockfile")?;

    let mut evidence = RepairTestEvidence {
        schema_version: REPAIR_TEST_EVIDENCE_SCHEMA_VERSION.into(),
        evidence_sha256: "0".repeat(64),
        change_receipt_sha256: change_receipt.receipt_sha256.clone(),
        check_id: check_id.into(),
        profile: RepairTestProfile::RustWorkspaceAllTargets,
        runner: RepairTestRunnerIdentity {
            name: "pliego-css-agent".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        toolchain,
        workspace_manifest: RepairArtifactIdentity {
            file: "Cargo.toml".into(),
            bytes: manifest_bytes.len(),
            sha256: sha256_hex(&manifest_bytes),
        },
        lockfile: RepairArtifactIdentity {
            file: "Cargo.lock".into(),
            bytes: lockfile_bytes.len(),
            sha256: sha256_hex(&lockfile_bytes),
        },
        result: if exit_code == 0 {
            RepairTestEvidenceResult::Passed
        } else {
            RepairTestEvidenceResult::Failed
        },
        exit_code,
    };
    evidence.evidence_sha256 = repair_test_evidence_payload_sha256(&evidence)?;
    evidence.validate()?;
    let evidence_bytes = evidence.to_json_pretty()?.into_bytes();
    let changed = match read_existing_repair_test_evidence(&output)? {
        Some((existing, bytes)) if existing == evidence && bytes == evidence_bytes => false,
        Some(_) => {
            return Err(RepairContractError::new(
                "test evidence destination already contains different evidence",
            ));
        }
        None => {
            let writes = [prepare_write(&output, &evidence_bytes)?];
            commit_new_repair_documents(&writes, |_, temporary, destination| {
                fs::hard_link(temporary, destination)
            })?;
            true
        }
    };
    Ok(PublishedRepairTestEvidence {
        evidence,
        evidence_bytes,
        changed,
    })
}

/// Browser evidence published by the explicit fixed `PliegoRS` Chromium runner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedRepairBrowserEvidence {
    evidence: RepairBrowserEvidence,
    evidence_bytes: Vec<u8>,
    changed: bool,
}

impl PublishedRepairBrowserEvidence {
    /// Returns the validated canonical browser evidence.
    #[must_use]
    pub const fn evidence(&self) -> &RepairBrowserEvidence {
        &self.evidence
    }

    /// Returns its exact canonical JSON bytes.
    #[must_use]
    pub fn evidence_bytes(&self) -> &[u8] {
        &self.evidence_bytes
    }

    /// Returns whether this invocation newly published the evidence destination.
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.changed
    }

    /// Renders a bounded human summary after the Chromium replay.
    #[must_use]
    pub fn to_human(&self) -> String {
        format!(
            "Browser evidence: {}\nProfile: pliegors-visit-counter-chromium-cdp\nBrowser: {}\nPublished: {}\n",
            match self.evidence.result {
                RepairBrowserEvidenceResult::Passed => "passed",
                RepairBrowserEvidenceResult::Failed => "failed",
            },
            self.evidence.browser.product,
            if self.changed { "yes" } else { "no" }
        )
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RepairBrowserAgentReport {
    schema_version: String,
    passed: bool,
    browser: RepairBrowserIdentity,
    observation: RepairBrowserObservation,
}

impl RepairBrowserAgentReport {
    fn validate(&self) -> Result<(), RepairContractError> {
        if self.schema_version != "1.0.0" {
            return Err(RepairContractError::new(
                "fixed browser profile returned an unsupported agent report schema",
            ));
        }
        self.browser.validate()?;
        self.observation.validate()?;
        if self.passed != self.observation.contract_passed() {
            return Err(RepairContractError::new(
                "fixed browser profile result does not match its closed observations",
            ));
        }
        Ok(())
    }
}

/// Runs the fixed `PliegoRS` Chromium/CDP profile and publishes canonical evidence.
///
/// The runner invokes only `node scripts/check-pliegors-browser.mjs` with its private closed report
/// mode. A caller cannot provide a program, script, URL, selector, assertion, browser argument, or
/// shell fragment. The Change Receipt after sources and eight fixed profile inputs are read before
/// and after execution under the repair lock. `PLIEGOCSS_CHROME_PATH` may select the installed
/// Chromium executable and `PLIEGORS_ROOT` may select a checkout, but the latter must satisfy the
/// exact pinned revision and covered-source hash in the bound profile inputs. The observed browser
/// product and protocol are recorded, not cryptographically authenticated.
///
/// # Errors
///
/// Returns [`RepairContractError`] for unsafe paths, source/profile drift, missing Node/Chrome,
/// malformed runner output, inconsistent process status, evidence collision, or publication
/// failure. A completed failed observation is published as valid failed evidence.
#[allow(clippy::too_many_lines)]
pub fn run_repair_pliegors_browser_checked(
    change_receipt_path: &Path,
    source_root: &Path,
    check_id: &str,
    evidence_path: &Path,
) -> Result<PublishedRepairBrowserEvidence, RepairContractError> {
    validate_dotted_id("browserEvidence.checkId", check_id)?;
    let (change_path, _) = resolve_repair_project_input(change_receipt_path, "Change Receipt")?;
    let change_bytes = read_repair_document(&change_path, "Change Receipt")?;
    let change_receipt = parse_repair_change_receipt(&change_bytes)?;
    let root = resolve_repair_verification_root(source_root)?;
    let cwd = env::current_dir()
        .and_then(fs::canonicalize)
        .map_err(|error| {
            RepairContractError::new(format!("cannot resolve current project: {error}"))
        })?;
    if root != cwd {
        return Err(RepairContractError::new(
            "fixed browser runner requires source-root to resolve to the current project directory",
        ));
    }
    let source_paths = resolve_repair_verification_sources(&root, &change_receipt.sources)?;
    let profile_paths = REPAIR_BROWSER_PROFILE_FILES
        .iter()
        .map(|file| {
            resolve_repair_verification_file(&root, file, "browser profile input")
                .map(|path| ((*file).to_owned(), path))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let protected = std::iter::once(change_path.as_path())
        .chain(profile_paths.values().map(PathBuf::as_path))
        .collect::<Vec<_>>();
    let output = resolve_repair_receipt_path(evidence_path, protected, &source_paths)?;

    let _lock = acquire_repair_lock(&root)?;
    require_repair_document_unchanged(&change_path, &change_bytes, "Change Receipt")?;
    let sources = read_repair_verification_sources(&source_paths)?;
    validate_repair_after_sources(&change_receipt, &sources)?;
    let profile_bytes = read_repair_verification_files(&profile_paths, "browser profile input")?;
    let node_version = read_repair_browser_node_version(&root)?;
    if !profile_paths.contains_key("scripts/check-pliegors-browser.mjs") {
        return Err(RepairContractError::new(
            "fixed browser profile script is missing",
        ));
    }
    let output_result = Command::new("node")
        .current_dir(&root)
        .arg("scripts/check-pliegors-browser.mjs")
        .env("PLIEGOCSS_AGENT_REPORT", "1")
        .env("PLIEGOCSS_SKIP_PLIEGORS_BUILD", "0")
        .env("PLIEGOCSS_KEEP_PLIEGORS_SITE", "0")
        .env("NO_COLOR", "1")
        .output()
        .map_err(|error| {
            RepairContractError::new(format!(
                "cannot launch fixed PliegoRS Chromium profile: {error}"
            ))
        })?;
    if output_result.stdout.len() > MAX_DOCUMENT_BYTES
        || output_result.stderr.len() > MAX_DOCUMENT_BYTES
    {
        return Err(RepairContractError::new(
            "fixed browser profile output exceeds 16 MiB",
        ));
    }
    let exit_code = output_result.status.code().ok_or_else(|| {
        RepairContractError::new(
            "fixed PliegoRS Chromium profile terminated without a numeric exit code",
        )
    })?;
    let exit_code = u8::try_from(exit_code).map_err(|_| {
        RepairContractError::new(format!(
            "fixed PliegoRS Chromium profile returned unsupported exit code `{exit_code}`"
        ))
    })?;
    let report: RepairBrowserAgentReport =
        serde_json::from_slice(&output_result.stdout).map_err(|error| {
            let stderr =
                bounded_verification_reason(&String::from_utf8_lossy(&output_result.stderr));
            RepairContractError::new(format!(
                "fixed browser profile returned invalid agent JSON: {error}; stderr: {stderr}"
            ))
        })?;
    report.validate()?;
    if report.passed != (exit_code == 0) {
        return Err(RepairContractError::new(
            "fixed browser profile process status does not match its report result",
        ));
    }

    require_repair_document_unchanged(&change_path, &change_bytes, "Change Receipt")?;
    let repeated_sources = read_repair_verification_sources(&source_paths)?;
    if repeated_sources != sources {
        return Err(RepairContractError::new(
            "repair sources changed while the fixed browser profile was executing",
        ));
    }
    let repeated_profile = read_repair_verification_files(&profile_paths, "browser profile input")?;
    if repeated_profile != profile_bytes {
        return Err(RepairContractError::new(
            "browser profile inputs changed while the fixed profile was executing",
        ));
    }

    let profile_inputs = profile_bytes
        .iter()
        .map(|(file, bytes)| RepairArtifactIdentity {
            file: file.clone(),
            bytes: bytes.len(),
            sha256: sha256_hex(bytes),
        })
        .collect::<Vec<_>>();
    let mut evidence = RepairBrowserEvidence {
        schema_version: REPAIR_BROWSER_EVIDENCE_SCHEMA_VERSION.into(),
        evidence_sha256: "0".repeat(64),
        change_receipt_sha256: change_receipt.receipt_sha256.clone(),
        check_id: check_id.into(),
        profile: RepairBrowserProfile::PliegorsVisitCounterChromiumCdp,
        runner: RepairBrowserRunnerIdentity {
            name: "pliego-css-agent".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            node_version,
        },
        profile_inputs,
        browser: report.browser,
        observation: report.observation,
        result: if exit_code == 0 {
            RepairBrowserEvidenceResult::Passed
        } else {
            RepairBrowserEvidenceResult::Failed
        },
        exit_code,
    };
    evidence.evidence_sha256 = repair_browser_evidence_payload_sha256(&evidence)?;
    evidence.validate()?;
    let evidence_bytes = evidence.to_json_pretty()?.into_bytes();
    let changed = match read_existing_repair_browser_evidence(&output)? {
        Some((existing, bytes)) if existing == evidence && bytes == evidence_bytes => false,
        Some(_) => {
            return Err(RepairContractError::new(
                "browser evidence destination already contains different evidence",
            ));
        }
        None => {
            let writes = [prepare_write(&output, &evidence_bytes)?];
            commit_new_repair_documents(&writes, |_, temporary, destination| {
                fs::hard_link(temporary, destination)
            })?;
            true
        }
    };
    Ok(PublishedRepairBrowserEvidence {
        evidence,
        evidence_bytes,
        changed,
    })
}

fn read_repair_browser_node_version(root: &Path) -> Result<String, RepairContractError> {
    let version = read_repair_test_tool_output(root, "node", &["--version"])?;
    let mut lines = version.lines();
    let version = lines
        .next()
        .ok_or_else(|| RepairContractError::new("node --version returned no version line"))?
        .to_owned();
    if lines.next().is_some() || !version.starts_with('v') {
        return Err(RepairContractError::new(
            "node --version returned a noncanonical version",
        ));
    }
    Ok(version)
}

fn read_repair_test_toolchain(
    root: &Path,
) -> Result<RepairTestToolchainIdentity, RepairContractError> {
    let cargo = read_repair_test_tool_output(root, "cargo", &["--version"])?;
    let mut cargo_lines = cargo.lines();
    let cargo_version = cargo_lines
        .next()
        .ok_or_else(|| RepairContractError::new("cargo --version returned no version line"))?
        .to_owned();
    if cargo_lines.next().is_some() {
        return Err(RepairContractError::new(
            "cargo --version returned multiple lines",
        ));
    }
    let rustc = read_repair_test_tool_output(root, "rustc", &["-vV"])?;
    let rustc_version = rustc
        .lines()
        .next()
        .ok_or_else(|| RepairContractError::new("rustc -vV returned no version line"))?
        .to_owned();
    let host = rustc
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or_else(|| RepairContractError::new("rustc -vV returned no host triple"))?
        .to_owned();
    let toolchain = RepairTestToolchainIdentity {
        cargo_version,
        rustc_version,
        host,
    };
    toolchain.validate()?;
    Ok(toolchain)
}

fn read_repair_test_tool_output(
    root: &Path,
    program: &str,
    arguments: &[&str],
) -> Result<String, RepairContractError> {
    let output = Command::new(program)
        .current_dir(root)
        .args(arguments)
        .output()
        .map_err(|error| {
            RepairContractError::new(format!(
                "cannot inspect fixed test tool `{program}`: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(RepairContractError::new(format!(
            "fixed test tool `{program}` identity command failed"
        )));
    }
    if output.stdout.len() > 4_096 || output.stderr.len() > 4_096 {
        return Err(RepairContractError::new(format!(
            "fixed test tool `{program}` identity output exceeds 4 KiB"
        )));
    }
    if !output.stderr.is_empty() {
        return Err(RepairContractError::new(format!(
            "fixed test tool `{program}` identity command wrote unexpected stderr"
        )));
    }
    let text = std::str::from_utf8(&output.stdout).map_err(|error| {
        RepairContractError::new(format!(
            "fixed test tool `{program}` identity output is not UTF-8: {error}"
        ))
    })?;
    Ok(text.trim_end_matches(['\r', '\n']).to_owned())
}

/// Loads exact repair artifacts, executes built-in checks under the repair lock, and atomically
/// publishes a separate Verification Receipt.
///
/// Input and output artifacts must be project-relative regular non-link files. Source files are
/// resolved from the Change Receipt below `source_root`, reject link-like components, and are read
/// twice under `.pliegocss-repair.lock`. An existing byte-identical verification receipt is a no-op;
/// every other output collision fails closed.
///
/// # Errors
///
/// Returns [`RepairContractError`] for unsafe paths, artifact/source drift, check failure to execute,
/// receipt collision, lock failure, or publication failure. A completed check whose policy result is
/// `failed` or whose browser evidence is blocked is returned as a valid receipt, not a tool error.
pub fn verify_repair_change_checked(
    change_receipt_path: &Path,
    check_policy_path: &Path,
    source_root: &Path,
    verification_receipt_path: &Path,
) -> Result<PublishedRepairVerification, RepairContractError> {
    let (change_path, change_file) =
        resolve_repair_project_input(change_receipt_path, "Change Receipt")?;
    let (policy_path, policy_file) =
        resolve_repair_project_input(check_policy_path, "repair-check policy")?;
    let change_bytes = read_repair_document(&change_path, "Change Receipt")?;
    let policy_bytes = read_repair_document(&policy_path, "repair-check policy")?;
    let change_receipt = parse_repair_change_receipt(&change_bytes)?;
    let policy = parse_repair_check_policy(&policy_bytes)?;
    let root = resolve_repair_verification_root(source_root)?;
    let source_paths = resolve_repair_verification_sources(&root, &change_receipt.sources)?;
    let input_identities =
        supplementary_repair_input_identities(&policy, source_paths.keys().map(String::as_str))?;
    let input_paths = resolve_repair_verification_inputs(
        &root,
        input_identities.keys().map(String::as_str),
        &source_paths,
    )?;
    let mut protected_paths = source_paths.clone();
    protected_paths.extend(input_paths.clone());
    let output = resolve_repair_receipt_path(
        verification_receipt_path,
        [&change_path, &policy_path].map(PathBuf::as_path),
        &protected_paths,
    )?;
    let finding_outputs = resolve_repair_finding_document_paths(
        verification_receipt_path,
        &policy,
        &change_path,
        &policy_path,
        &output,
        &protected_paths,
    )?;

    let _lock = acquire_repair_lock(&root)?;
    require_repair_document_unchanged(&change_path, &change_bytes, "Change Receipt")?;
    require_repair_document_unchanged(&policy_path, &policy_bytes, "repair-check policy")?;
    let sources = read_repair_verification_sources(&source_paths)?;
    let inputs = read_repair_verification_files(&input_paths, "supplementary verification input")?;
    let prepared = prepare_repair_verification(
        &change_receipt,
        &change_file,
        &change_bytes,
        &policy_file,
        &policy_bytes,
        &policy,
        RepairVerificationInputs::new(&sources, &inputs),
    )?;
    let receipt_bytes = prepared.receipt.to_json_pretty()?.into_bytes();
    require_repair_document_unchanged(&change_path, &change_bytes, "Change Receipt")?;
    require_repair_document_unchanged(&policy_path, &policy_bytes, "repair-check policy")?;
    let repeated_sources = read_repair_verification_sources(&source_paths)?;
    if repeated_sources != sources {
        return Err(RepairContractError::new(
            "verification sources changed while built-in checks were executing",
        ));
    }
    let repeated_inputs =
        read_repair_verification_files(&input_paths, "supplementary verification input")?;
    if repeated_inputs != inputs {
        return Err(RepairContractError::new(
            "supplementary verification inputs changed while built-in checks were executing",
        ));
    }
    match read_existing_verification_receipt(&output)? {
        Some((existing, bytes)) if existing == prepared.receipt && bytes == receipt_bytes => {
            let finding_documents = collect_existing_repair_finding_documents(
                &prepared.finding_documents,
                &finding_outputs,
            )?;
            Ok(PublishedRepairVerification {
                receipt: existing,
                receipt_bytes: bytes,
                finding_documents,
                changed: false,
            })
        }
        Some(_) => Err(RepairContractError::new(
            "verification receipt destination already contains different evidence",
        )),
        None => {
            let finding_documents = publish_repair_verification_documents(
                &prepared.finding_documents,
                &finding_outputs,
                &output,
                &receipt_bytes,
            )?;
            Ok(PublishedRepairVerification {
                receipt: prepared.receipt,
                receipt_bytes,
                finding_documents,
                changed: true,
            })
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepairFindingDocumentOutput {
    path: PathBuf,
    file: String,
}

fn resolve_repair_finding_document_paths(
    receipt: &Path,
    policy: &RepairCheckPolicy,
    change_path: &Path,
    policy_path: &Path,
    receipt_path: &Path,
    source_paths: &BTreeMap<String, PathBuf>,
) -> Result<BTreeMap<String, RepairFindingDocumentOutput>, RepairContractError> {
    let receipt_file = receipt
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| RepairContractError::new("verification receipt path must be UTF-8"))?
        .join("/")
        .to_ascii_lowercase();
    validate_logical_path("verificationReceipt.file", &receipt_file)?;
    let parent = receipt.parent().unwrap_or_else(|| Path::new(""));
    let mut protected = vec![
        change_path.to_path_buf(),
        policy_path.to_path_buf(),
        receipt_path.to_path_buf(),
    ];
    let mut outputs = BTreeMap::new();
    for definition in &policy.checks {
        let key = sha256_hex(format!("{receipt_file}\0{}", definition.id).as_bytes());
        let relative = parent.join(format!("pliego-css-findings-{key}.json"));
        let file = relative
            .components()
            .map(|component| component.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| RepairContractError::new("derived finding document path must be UTF-8"))?
            .join("/");
        validate_logical_path("findingDocument.file", &file)?;
        let path = resolve_repair_receipt_path(
            &relative,
            protected.iter().map(PathBuf::as_path),
            source_paths,
        )?;
        protected.push(path.clone());
        outputs.insert(
            definition.id.clone(),
            RepairFindingDocumentOutput { path, file },
        );
    }
    Ok(outputs)
}

fn collect_existing_repair_finding_documents(
    documents: &BTreeMap<String, Vec<u8>>,
    outputs: &BTreeMap<String, RepairFindingDocumentOutput>,
) -> Result<Vec<PublishedRepairFindingDocument>, RepairContractError> {
    documents
        .iter()
        .map(|(check_id, bytes)| {
            let output = outputs.get(check_id).ok_or_else(|| {
                RepairContractError::new(format!(
                    "missing derived FindingDocument output for check `{check_id}`"
                ))
            })?;
            if !repair_document_matches(&output.path, bytes, "verification FindingDocument")? {
                return Err(RepairContractError::new(format!(
                    "verification receipt exists without FindingDocument `{}`",
                    output.file
                )));
            }
            Ok(PublishedRepairFindingDocument {
                check_id: check_id.clone(),
                file: output.file.clone(),
                bytes: bytes.clone(),
                changed: false,
            })
        })
        .collect()
}

fn publish_repair_verification_documents(
    documents: &BTreeMap<String, Vec<u8>>,
    outputs: &BTreeMap<String, RepairFindingDocumentOutput>,
    receipt_path: &Path,
    receipt_bytes: &[u8],
) -> Result<Vec<PublishedRepairFindingDocument>, RepairContractError> {
    publish_repair_verification_documents_with(
        documents,
        outputs,
        receipt_path,
        receipt_bytes,
        |_, temporary, destination| fs::hard_link(temporary, destination),
    )
}

fn publish_repair_verification_documents_with(
    documents: &BTreeMap<String, Vec<u8>>,
    outputs: &BTreeMap<String, RepairFindingDocumentOutput>,
    receipt_path: &Path,
    receipt_bytes: &[u8],
    publish: impl FnMut(usize, &Path, &Path) -> std::io::Result<()>,
) -> Result<Vec<PublishedRepairFindingDocument>, RepairContractError> {
    let mut published = Vec::with_capacity(documents.len());
    let mut writes = Vec::with_capacity(documents.len() + 1);
    for (check_id, bytes) in documents {
        let output = outputs.get(check_id).ok_or_else(|| {
            RepairContractError::new(format!(
                "missing derived FindingDocument output for check `{check_id}`"
            ))
        })?;
        let exists = repair_document_matches(&output.path, bytes, "verification FindingDocument")?;
        if !exists {
            writes.push(prepare_write(&output.path, bytes)?);
        }
        published.push(PublishedRepairFindingDocument {
            check_id: check_id.clone(),
            file: output.file.clone(),
            bytes: bytes.clone(),
            changed: !exists,
        });
    }
    writes.push(prepare_write(receipt_path, receipt_bytes)?);
    commit_new_repair_documents(&writes, publish)?;
    Ok(published)
}

fn repair_document_matches(
    path: &Path,
    expected: &[u8],
    role: &str,
) -> Result<bool, RepairContractError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(RepairContractError::new(format!(
            "cannot inspect {role} `{}`: {error}",
            path.display()
        ))),
        Ok(metadata) if is_repair_link_like(&metadata) || !metadata.is_file() => {
            Err(RepairContractError::new(format!(
                "{role} `{}` is not a regular non-link file",
                path.display()
            )))
        }
        Ok(_) => {
            let actual = read_repair_document(path, role)?;
            if actual != expected {
                return Err(RepairContractError::new(format!(
                    "{role} `{}` contains different evidence",
                    path.display()
                )));
            }
            parse_finding_document(&actual).map_err(|error| {
                RepairContractError::new(format!("invalid {role} `{}`: {error}", path.display()))
            })?;
            Ok(true)
        }
    }
}

fn commit_new_repair_documents(
    writes: &[PreparedRepairWrite],
    mut publish: impl FnMut(usize, &Path, &Path) -> std::io::Result<()>,
) -> Result<(), RepairContractError> {
    for (index, write) in writes.iter().enumerate() {
        let temporary = write.temporary.as_deref().ok_or_else(|| {
            RepairContractError::new("prepared verification evidence lost its temporary path")
        })?;
        if let Err(error) = publish(index, temporary, &write.destination) {
            let primary = format!(
                "cannot publish verification evidence `{}` without overwrite: {error}",
                write.destination.display()
            );
            return Err(rollback_new_repair_documents(writes, index, &primary));
        }
    }
    Ok(())
}

fn rollback_new_repair_documents(
    writes: &[PreparedRepairWrite],
    published: usize,
    primary: &str,
) -> RepairContractError {
    let mut errors = Vec::new();
    for write in writes[..published].iter().rev() {
        if let Err(error) = fs::remove_file(&write.destination) {
            if error.kind() != std::io::ErrorKind::NotFound {
                errors.push(format!(
                    "cannot remove `{}`: {error}",
                    write.destination.display()
                ));
            }
        }
    }
    if errors.is_empty() {
        RepairContractError::new(format!(
            "{primary}; all newly linked verification evidence was removed"
        ))
    } else {
        RepairContractError::new(format!(
            "{primary}; verification rollback also failed: {}",
            errors.join("; ")
        ))
    }
}

fn resolve_repair_project_input(
    path: &Path,
    role: &str,
) -> Result<(PathBuf, String), RepairContractError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(RepairContractError::new(format!(
            "{role} must be a project-relative path without `.` or `..` components"
        )));
    }
    let logical = path
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| RepairContractError::new(format!("{role} path must be UTF-8")))?
        .join("/");
    validate_logical_path(role, &logical)?;
    let cwd = env::current_dir()
        .map_err(|error| RepairContractError::new(format!("cannot resolve {role}: {error}")))?;
    let resolved = cwd.join(path);
    let mut current = cwd;
    for component in path.components() {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            RepairContractError::new(format!(
                "cannot inspect {role} `{}`: {error}",
                current.display()
            ))
        })?;
        let last = current == resolved;
        if is_repair_link_like(&metadata)
            || (last && !metadata.is_file())
            || (!last && !metadata.is_dir())
        {
            return Err(RepairContractError::new(format!(
                "{role} `{}` has a non-regular or link-like component",
                current.display()
            )));
        }
    }
    Ok((resolved, logical))
}

fn resolve_repair_verification_root(root: &Path) -> Result<PathBuf, RepairContractError> {
    let metadata = fs::symlink_metadata(root).map_err(|error| {
        RepairContractError::new(format!(
            "cannot inspect verification source root `{}`: {error}",
            root.display()
        ))
    })?;
    if is_repair_link_like(&metadata) || !metadata.is_dir() {
        return Err(RepairContractError::new(
            "verification source root must be a regular non-link directory",
        ));
    }
    fs::canonicalize(root).map_err(|error| {
        RepairContractError::new(format!(
            "cannot canonicalize verification source root `{}`: {error}",
            root.display()
        ))
    })
}

fn resolve_repair_verification_sources(
    root: &Path,
    transitions: &[RepairSourceTransition],
) -> Result<BTreeMap<String, PathBuf>, RepairContractError> {
    let mut sources = BTreeMap::new();
    for transition in transitions {
        let canonical =
            resolve_repair_verification_file(root, &transition.file, "verification source")?;
        sources.insert(transition.file.clone(), canonical);
    }
    Ok(sources)
}

fn supplementary_repair_input_identities<'a>(
    policy: &RepairCheckPolicy,
    source_files: impl Iterator<Item = &'a str>,
) -> Result<BTreeMap<String, RepairArtifactIdentity>, RepairContractError> {
    let source_files = source_files.collect::<BTreeSet<_>>();
    let mut inputs = BTreeMap::new();
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
            if source_files.contains(identity.file.as_str()) {
                continue;
            }
            match inputs.insert(identity.file.clone(), identity.clone()) {
                Some(previous) if previous != *identity => {
                    return Err(RepairContractError::new(format!(
                        "supplementary input `{}` has conflicting identities",
                        identity.file
                    )));
                }
                _ => {}
            }
        }
    }
    Ok(inputs)
}

fn resolve_repair_verification_inputs<'a>(
    root: &Path,
    files: impl Iterator<Item = &'a str>,
    source_paths: &BTreeMap<String, PathBuf>,
) -> Result<BTreeMap<String, PathBuf>, RepairContractError> {
    let mut physical = source_paths
        .values()
        .map(|path| path.to_string_lossy().to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut inputs = BTreeMap::new();
    for file in files {
        let canonical =
            resolve_repair_verification_file(root, file, "supplementary verification input")?;
        let key = canonical.to_string_lossy().to_ascii_lowercase();
        if !physical.insert(key) {
            return Err(RepairContractError::new(format!(
                "supplementary verification input `{file}` aliases another verification file"
            )));
        }
        inputs.insert(file.to_owned(), canonical);
    }
    Ok(inputs)
}

fn resolve_repair_verification_file(
    root: &Path,
    file: &str,
    role: &str,
) -> Result<PathBuf, RepairContractError> {
    validate_logical_path(role, file)?;
    let mut current = root.to_path_buf();
    let components = Path::new(file).components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            RepairContractError::new(format!(
                "cannot inspect {role} `{}`: {error}",
                current.display()
            ))
        })?;
        let last = index + 1 == components.len();
        if is_repair_link_like(&metadata)
            || (last && !metadata.is_file())
            || (!last && !metadata.is_dir())
        {
            return Err(RepairContractError::new(format!(
                "{role} `{}` has a non-regular or link-like component",
                current.display()
            )));
        }
    }
    let canonical = fs::canonicalize(&current).map_err(|error| {
        RepairContractError::new(format!(
            "cannot canonicalize {role} `{}`: {error}",
            current.display()
        ))
    })?;
    if !canonical.starts_with(root) {
        return Err(RepairContractError::new(format!(
            "{role} `{file}` escapes source root `{}`",
            root.display()
        )));
    }
    Ok(canonical)
}

fn read_repair_verification_sources(
    paths: &BTreeMap<String, PathBuf>,
) -> Result<BTreeMap<String, Vec<u8>>, RepairContractError> {
    read_repair_verification_files(paths, "verification source")
}

fn read_repair_verification_files(
    paths: &BTreeMap<String, PathBuf>,
    role: &str,
) -> Result<BTreeMap<String, Vec<u8>>, RepairContractError> {
    paths
        .iter()
        .map(|(file, path)| {
            let metadata = fs::symlink_metadata(path).map_err(|error| {
                RepairContractError::new(format!(
                    "cannot re-inspect {role} `{}`: {error}",
                    path.display()
                ))
            })?;
            if is_repair_link_like(&metadata) || !metadata.is_file() {
                return Err(RepairContractError::new(format!(
                    "{role} `{}` changed file type",
                    path.display()
                )));
            }
            let bytes = fs::read(path).map_err(|error| {
                RepairContractError::new(format!(
                    "cannot read {role} `{}`: {error}",
                    path.display()
                ))
            })?;
            if bytes.len() > MAX_SOURCE_BYTES {
                return Err(RepairContractError::new(format!(
                    "{role} `{file}` exceeds 16 MiB"
                )));
            }
            Ok((file.clone(), bytes))
        })
        .collect()
}

fn read_repair_document(path: &Path, role: &str) -> Result<Vec<u8>, RepairContractError> {
    let bytes = fs::read(path).map_err(|error| {
        RepairContractError::new(format!("cannot read {role} `{}`: {error}", path.display()))
    })?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(RepairContractError::new(format!("{role} exceeds 16 MiB")));
    }
    Ok(bytes)
}

fn require_repair_document_unchanged(
    path: &Path,
    expected: &[u8],
    role: &str,
) -> Result<(), RepairContractError> {
    if read_repair_document(path, role)? != expected {
        return Err(RepairContractError::new(format!(
            "{role} changed while verification was active"
        )));
    }
    Ok(())
}

fn read_existing_verification_receipt(
    path: &Path,
) -> Result<Option<(RepairVerificationReceipt, Vec<u8>)>, RepairContractError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(RepairContractError::new(format!(
            "cannot inspect verification receipt `{}`: {error}",
            path.display()
        ))),
        Ok(metadata) if is_repair_link_like(&metadata) || !metadata.is_file() => {
            Err(RepairContractError::new(format!(
                "verification receipt `{}` is not a regular non-link file",
                path.display()
            )))
        }
        Ok(_) => {
            let bytes = read_repair_document(path, "verification receipt")?;
            let receipt = parse_repair_verification_receipt(&bytes)?;
            Ok(Some((receipt, bytes)))
        }
    }
}

fn read_existing_repair_test_evidence(
    path: &Path,
) -> Result<Option<(RepairTestEvidence, Vec<u8>)>, RepairContractError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(RepairContractError::new(format!(
            "cannot inspect test evidence `{}`: {error}",
            path.display()
        ))),
        Ok(metadata) if is_repair_link_like(&metadata) || !metadata.is_file() => {
            Err(RepairContractError::new(format!(
                "test evidence `{}` is not a regular non-link file",
                path.display()
            )))
        }
        Ok(_) => {
            let bytes = read_repair_document(path, "test evidence")?;
            let evidence = parse_repair_test_evidence(&bytes)?;
            Ok(Some((evidence, bytes)))
        }
    }
}

fn read_existing_repair_browser_evidence(
    path: &Path,
) -> Result<Option<(RepairBrowserEvidence, Vec<u8>)>, RepairContractError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(RepairContractError::new(format!(
            "cannot inspect browser evidence `{}`: {error}",
            path.display()
        ))),
        Ok(metadata) if is_repair_link_like(&metadata) || !metadata.is_file() => {
            Err(RepairContractError::new(format!(
                "browser evidence `{}` is not a regular non-link file",
                path.display()
            )))
        }
        Ok(_) => {
            let bytes = read_repair_document(path, "browser evidence")?;
            let evidence = parse_repair_browser_evidence(&bytes)?;
            Ok(Some((evidence, bytes)))
        }
    }
}

fn apply_file_edits(
    file: &str,
    source: &[u8],
    edits: &[RepairPlannedEdit],
) -> Result<Vec<u8>, RepairContractError> {
    let source_text = std::str::from_utf8(source).map_err(|error| {
        RepairContractError::new(format!("source snapshot `{file}` is not UTF-8: {error}"))
    })?;
    let file_edits = edits
        .iter()
        .filter(|edit| edit.file == file)
        .collect::<Vec<_>>();
    if file_edits.is_empty() {
        return Err(RepairContractError::new(format!(
            "source snapshot `{file}` has no planned edits"
        )));
    }
    let mut output = source_text.to_owned();
    for edit in file_edits.into_iter().rev() {
        let actual = source_text
            .get(edit.byte_start..edit.byte_end)
            .ok_or_else(|| {
                RepairContractError::new(format!(
                    "plan edit range {}..{} is invalid for `{file}`",
                    edit.byte_start, edit.byte_end
                ))
            })?;
        if actual != edit.removed {
            return Err(RepairContractError::new(format!(
                "plan edit range {}..{} in `{file}` no longer matches removed bytes",
                edit.byte_start, edit.byte_end
            )));
        }
        output.replace_range(edit.byte_start..edit.byte_end, &edit.replacement);
    }
    Ok(output.into_bytes())
}

fn build_patch(edits: &[RepairPlannedEdit]) -> Result<RepairPatch, RepairContractError> {
    let mut text = String::new();
    let mut current_file = None;
    for edit in edits {
        if current_file != Some(edit.file.as_str()) {
            current_file = Some(&edit.file);
            text.push_str("--- a/");
            text.push_str(&edit.file);
            text.push_str("\n+++ b/");
            text.push_str(&edit.file);
            text.push('\n');
        }
        text.push_str("@@ bytes ");
        text.push_str(&edit.byte_start.to_string());
        text.push_str("..");
        text.push_str(&edit.byte_end.to_string());
        text.push_str(" finding ");
        text.push_str(&edit.finding_fingerprint);
        text.push_str(" suggestion ");
        text.push_str(&edit.suggestion_id);
        text.push_str(" @@\n-");
        text.push_str(&serde_json::to_string(&edit.removed).map_err(|error| {
            RepairContractError::new(format!("cannot render removed bytes: {error}"))
        })?);
        text.push_str("\n+");
        text.push_str(&serde_json::to_string(&edit.replacement).map_err(|error| {
            RepairContractError::new(format!("cannot render replacement bytes: {error}"))
        })?);
        text.push('\n');
    }
    Ok(RepairPatch {
        format: "pliegocss-byte-edits/1".into(),
        sha256: sha256_hex(text.as_bytes()),
        text,
    })
}

fn summarize_edits(
    edits: &[RepairPlannedEdit],
) -> Result<RepairChangeSummary, RepairContractError> {
    let files = edits
        .iter()
        .map(|edit| edit.file.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let inserted_bytes = edits.iter().try_fold(0u64, |total, edit| {
        let value = u64::try_from(edit.replacement.len())
            .map_err(|_| RepairContractError::new("inserted byte count overflows u64"))?;
        total
            .checked_add(value)
            .ok_or_else(|| RepairContractError::new("inserted byte count overflows u64"))
    })?;
    let removed_bytes = edits.iter().try_fold(0u64, |total, edit| {
        let value = u64::try_from(edit.removed.len())
            .map_err(|_| RepairContractError::new("removed byte count overflows u64"))?;
        total
            .checked_add(value)
            .ok_or_else(|| RepairContractError::new("removed byte count overflows u64"))
    })?;
    Ok(RepairChangeSummary {
        files: u32::try_from(files)
            .map_err(|_| RepairContractError::new("changed file count exceeds u32"))?,
        edits: u32::try_from(edits.len())
            .map_err(|_| RepairContractError::new("edit count exceeds u32"))?,
        inserted_bytes,
        removed_bytes,
    })
}

fn enforce_budget(
    summary: RepairChangeSummary,
    budget: RepairChangeBudget,
) -> Result<(), RepairContractError> {
    if summary.files > budget.max_files
        || summary.edits > budget.max_edits
        || summary.inserted_bytes > budget.max_inserted_bytes
        || summary.removed_bytes > budget.max_removed_bytes
    {
        return Err(RepairContractError::new(format!(
            "change summary files={} edits={} insertedBytes={} removedBytes={} exceeds budget files={} edits={} insertedBytes={} removedBytes={}",
            summary.files,
            summary.edits,
            summary.inserted_bytes,
            summary.removed_bytes,
            budget.max_files,
            budget.max_edits,
            budget.max_inserted_bytes,
            budget.max_removed_bytes
        )));
    }
    Ok(())
}

fn validate_edit_order(edits: &[RepairProposalEdit]) -> Result<(), RepairContractError> {
    for pair in edits.windows(2) {
        let left = (
            pair[0].file.as_str(),
            pair[0].byte_start,
            pair[0].byte_end,
            pair[0].finding_fingerprint.as_str(),
            pair[0].suggestion_id.as_str(),
        );
        let right = (
            pair[1].file.as_str(),
            pair[1].byte_start,
            pair[1].byte_end,
            pair[1].finding_fingerprint.as_str(),
            pair[1].suggestion_id.as_str(),
        );
        if left >= right {
            return Err(RepairContractError::new(
                "proposal.edits are not in canonical file/range/finding/suggestion order",
            ));
        }
        if pair[0].file == pair[1].file && pair[1].byte_start < pair[0].byte_end {
            return Err(RepairContractError::new(
                "proposal.edits contain overlapping source ranges",
            ));
        }
        if pair[0].file == pair[1].file
            && pair[0].byte_start == pair[0].byte_end
            && pair[1].byte_start == pair[0].byte_start
        {
            return Err(RepairContractError::new(
                "proposal.edits contain ambiguous insertions at the same byte",
            ));
        }
    }
    Ok(())
}

fn validate_planned_edit_order(edits: &[RepairPlannedEdit]) -> Result<(), RepairContractError> {
    for pair in edits.windows(2) {
        let left = (
            pair[0].file.as_str(),
            pair[0].byte_start,
            pair[0].byte_end,
            pair[0].finding_fingerprint.as_str(),
            pair[0].suggestion_id.as_str(),
        );
        let right = (
            pair[1].file.as_str(),
            pair[1].byte_start,
            pair[1].byte_end,
            pair[1].finding_fingerprint.as_str(),
            pair[1].suggestion_id.as_str(),
        );
        if left >= right {
            return Err(RepairContractError::new(
                "plan.edits are not in canonical file/range/finding/suggestion order",
            ));
        }
        if pair[0].file == pair[1].file && pair[1].byte_start < pair[0].byte_end {
            return Err(RepairContractError::new(
                "plan.edits contain overlapping source ranges",
            ));
        }
        if pair[0].file == pair[1].file
            && pair[0].byte_start == pair[0].byte_end
            && pair[1].byte_start == pair[0].byte_start
        {
            return Err(RepairContractError::new(
                "plan.edits contain ambiguous insertions at the same byte",
            ));
        }
    }
    Ok(())
}

fn validate_source_order(sources: &[RepairSourceTransition]) -> Result<(), RepairContractError> {
    for pair in sources.windows(2) {
        if pair[0].file >= pair[1].file {
            return Err(RepairContractError::new(
                "plan.sources are not in unique canonical path order",
            ));
        }
    }
    Ok(())
}

fn validate_checks(checks: &[String]) -> Result<(), RepairContractError> {
    if checks.is_empty() || checks.len() > MAX_CHECKS {
        return Err(RepairContractError::new(
            "requiredChecks must contain between 1 and 256 checks",
        ));
    }
    for check in checks {
        validate_dotted_id("requiredChecks entry", check)?;
    }
    for pair in checks.windows(2) {
        if pair[0] >= pair[1] {
            return Err(RepairContractError::new(
                "requiredChecks must be sorted and unique",
            ));
        }
    }
    Ok(())
}

fn validate_sha256(field: &str, value: &str) -> Result<(), RepairContractError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(RepairContractError::new(format!(
            "{field} must be 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn validate_version(field: &str, value: &str) -> Result<(), RepairContractError> {
    if value.is_empty()
        || value.len() > 64
        || !value.as_bytes()[0].is_ascii_digit()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
    {
        return Err(RepairContractError::new(format!(
            "{field} must be a bounded ASCII version beginning with a digit"
        )));
    }
    Ok(())
}

fn validate_fingerprint(field: &str, value: &str) -> Result<(), RepairContractError> {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| RepairContractError::new(format!("{field} must start with `sha256:`")))?;
    validate_sha256(field, digest)
}

fn validate_finding_code(value: &str) -> Result<(), RepairContractError> {
    if !(3..=64).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-')
        || !value.as_bytes()[0].is_ascii_uppercase()
        || value.ends_with('-')
        || value.contains("--")
    {
        return Err(RepairContractError::new(format!(
            "finding code `{value}` is not canonical"
        )));
    }
    Ok(())
}

fn validate_slug(field: &str, value: &str) -> Result<(), RepairContractError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || !value.as_bytes()[0].is_ascii_lowercase()
        || value.ends_with('-')
        || value.contains("--")
    {
        return Err(RepairContractError::new(format!(
            "{field} `{value}` must be lowercase kebab-case ASCII"
        )));
    }
    Ok(())
}

fn validate_dotted_id(field: &str, value: &str) -> Result<(), RepairContractError> {
    if value.is_empty() || value.len() > 256 {
        return Err(RepairContractError::new(format!(
            "{field} must be 1-256 bytes"
        )));
    }
    for segment in value.split('.') {
        validate_slug(field, segment)?;
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, max: usize) -> Result<(), RepairContractError> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(RepairContractError::new(format!(
            "{field} must be non-empty, at most {max} bytes, and contain no control characters"
        )));
    }
    Ok(())
}

fn validate_logical_path(field: &str, value: &str) -> Result<(), RepairContractError> {
    validate_text(field, value, 4_096)?;
    if value.starts_with('/')
        || value.contains('\\')
        || value.contains(':')
        || value
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(RepairContractError::new(format!(
            "{field} `{value}` must be a portable relative logical path"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pliego_css_build::artifacts::{
        FindingCause, FindingException, FindingSeverity, FindingSource, FindingSuggestion,
        FindingTool,
    };
    use pliego_css_config::{TOKEN_GRAPH_VERSION, TokenGraph, TokenGraphTheme, TokenGraphToken};

    fn token_graph_fixture() -> TokenGraph {
        let name = "unit";
        let mut token_id = 2_166_136_261_u32;
        for byte in name.bytes() {
            token_id ^= u32::from(byte);
            token_id = token_id.wrapping_mul(16_777_619);
        }
        TokenGraph {
            schema_version: 1,
            graph_version: TOKEN_GRAPH_VERSION.into(),
            adapter: None,
            sources: Vec::new(),
            themes: vec![TokenGraphTheme {
                name: "default".into(),
                selections: BTreeMap::new(),
                theme_id: "00000000000000000000000000000000".into(),
                dtcg_inventory: BTreeMap::new(),
                tokens: vec![TokenGraphToken {
                    kind: "spacing".into(),
                    token_id: format!("{token_id:08x}"),
                    name: name.into(),
                    source: None,
                    value: "1rem".into(),
                }],
            }],
            dtcg_inventory: BTreeMap::new(),
        }
    }

    fn budget_policy_fixture(file: &str, maximum: u64) -> Vec<u8> {
        let source = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "policyVersion": 1,
            "budgets": [{
                "id": "repaired-file-bytes",
                "subject": {"kind": "file", "id": file},
                "limits": {
                    "bytes": {
                        "maximum": maximum,
                        "baseline": null,
                        "maxIncrease": null
                    }
                }
            }],
            "exceptions": []
        }))
        .unwrap();
        parse_budget_policy(&source)
            .unwrap()
            .to_json_pretty()
            .unwrap()
            .into_bytes()
    }

    fn assert_token_graph_schema_1_1_compatibility(
        mut policy: RepairCheckPolicy,
        mut receipt: RepairVerificationReceipt,
    ) {
        policy.schema_version = TOKEN_GRAPH_REPAIR_CHECK_POLICY_SCHEMA_VERSION.into();
        let policy_bytes = policy.to_json_pretty().unwrap();
        assert_eq!(
            parse_repair_check_policy(policy_bytes.as_bytes()).unwrap(),
            policy
        );

        receipt.schema_version = TOKEN_GRAPH_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION.into();
        receipt.receipt_sha256 = verification_receipt_payload_sha256(&receipt).unwrap();
        let receipt_bytes = receipt.to_json_pretty().unwrap();
        assert_eq!(
            parse_repair_verification_receipt(receipt_bytes.as_bytes()).unwrap(),
            receipt
        );
    }

    fn fixture() -> (
        Vec<u8>,
        String,
        RepairProposalDocument,
        BTreeMap<String, Vec<u8>>,
    ) {
        fixture_with(FindingVerification::Verified, FindingRisk::Low, false)
    }

    fn fixture_with(
        verification: FindingVerification,
        risk: FindingRisk,
        excepted: bool,
    ) -> (
        Vec<u8>,
        String,
        RepairProposalDocument,
        BTreeMap<String, Vec<u8>>,
    ) {
        let css = b".button { color: #777; }\n".to_vec();
        let start = String::from_utf8(css.clone())
            .unwrap()
            .find("#777")
            .unwrap();
        let source = FindingSource::new("src/app.css", start, start + 4).unwrap();
        let finding = Finding::new(
            "A11Y001",
            "accessibility",
            FindingSeverity::Error,
            "declared contrast is below policy",
            verification,
            FindingCause::new(
                "accessibility.contrast",
                "minimum-ratio",
                "ratio is too low",
            )
            .unwrap(),
        )
        .unwrap()
        .with_source(source)
        .unwrap()
        .with_suggestion(
            FindingSuggestion::new(
                1,
                "replace-color",
                "replace the local literal",
                risk,
                "declaration",
            )
            .unwrap()
            .with_prerequisites(vec!["audit".into()])
            .unwrap(),
        )
        .unwrap();
        let finding = if excepted {
            finding
                .with_exception(FindingException::new("reviewed", "accepted by policy").unwrap())
                .unwrap()
        } else {
            finding
        };
        let fingerprint = finding.fingerprint().to_owned();
        let document = FindingDocument::new(
            FindingTool::new("pliegocss", "0.0.0").unwrap(),
            "audit",
            vec![finding],
        )
        .unwrap();
        let bytes = document.to_json_pretty().unwrap().into_bytes();
        let proposal = RepairProposalDocument {
            schema_version: REPAIR_PROPOSAL_SCHEMA_VERSION.into(),
            finding_document_sha256: sha256_hex(&bytes),
            change_budget: RepairChangeBudget::new(1, 1, 4, 4).unwrap(),
            required_checks: vec!["audit".into()],
            edits: vec![RepairProposalEdit {
                finding_fingerprint: fingerprint,
                suggestion_id: "replace-color".into(),
                file: "src/app.css".into(),
                byte_start: start,
                byte_end: start + 4,
                replacement: "#111".into(),
            }],
        };
        let sources = BTreeMap::from([("src/app.css".into(), css)]);
        (bytes, "findings.json".into(), proposal, sources)
    }

    #[test]
    fn plan_is_deterministic_integrity_bound_and_range_exact() {
        let (findings, path, proposal, sources) = fixture();
        let first = build_repair_plan(
            RepairTool::new("pliegocss", "0.0.0").unwrap(),
            &path,
            &findings,
            &proposal,
            &sources,
        )
        .unwrap();
        let second = build_repair_plan(
            RepairTool::new("pliegocss", "0.0.0").unwrap(),
            &path,
            &findings,
            &proposal,
            &sources,
        )
        .unwrap();
        assert_eq!(first, second);
        let json = first.to_json_pretty().unwrap();
        assert_eq!(parse_repair_plan(json.as_bytes()).unwrap(), first);
        assert!(first.patch.text.contains("-\"#777\"\n+\"#111\""));
        assert_eq!(first.change_summary.files, 1);
        assert_eq!(first.change_summary.inserted_bytes, 4);
        assert_eq!(first.change_summary.removed_bytes, 4);
    }

    #[test]
    fn dry_run_distinguishes_ready_already_applied_and_stale() {
        let (findings, path, proposal, sources) = fixture();
        let plan = build_repair_plan(
            RepairTool::new("pliegocss", "0.0.0").unwrap(),
            &path,
            &findings,
            &proposal,
            &sources,
        )
        .unwrap();
        assert_eq!(
            verify_repair_plan(&plan, &path, &findings, &sources)
                .unwrap()
                .state,
            RepairDryRunState::Ready
        );
        let applied =
            BTreeMap::from([("src/app.css".into(), b".button { color: #111; }\n".to_vec())]);
        assert_eq!(
            verify_repair_plan(&plan, &path, &findings, &applied)
                .unwrap()
                .state,
            RepairDryRunState::AlreadyApplied
        );
        let stale =
            BTreeMap::from([("src/app.css".into(), b".button { color: #999; }\n".to_vec())]);
        assert!(verify_repair_plan(&plan, &path, &findings, &stale).is_err());
    }

    #[test]
    fn explicit_apply_prepares_after_bytes_and_an_honest_receipt() {
        let (findings, path, proposal, sources) = fixture();
        let plan = build_repair_plan(
            RepairTool::new("pliegocss", "0.0.0").unwrap(),
            &path,
            &findings,
            &proposal,
            &sources,
        )
        .unwrap();
        let authorization = format!("sha256:{}", plan.plan_sha256());
        assert!(
            prepare_repair_application(&plan, &path, &findings, &sources, "sha256:wrong").is_err()
        );
        let prepared =
            prepare_repair_application(&plan, &path, &findings, &sources, &authorization).unwrap();
        assert_eq!(prepared.receipt().state(), RepairChangeState::Applied);
        assert_eq!(prepared.receipt().plan_sha256(), plan.plan_sha256());
        assert_eq!(
            prepared.sources()["src/app.css"],
            b".button { color: #111; }\n"
        );
        let receipt = prepared.receipt().to_json_pretty().unwrap();
        assert_eq!(
            parse_repair_change_receipt(receipt.as_bytes()).unwrap(),
            *prepared.receipt()
        );
        let value: serde_json::Value = serde_json::from_str(&receipt).unwrap();
        assert_eq!(value["result"], "checks-pending");
        assert_eq!(value["checks"][0]["status"], "not-run");
        assert_eq!(value["browserEvidence"], "not-collected");
        let compact = serde_json::to_vec(&value).unwrap();
        assert!(parse_repair_change_receipt(&compact).is_err());

        let already =
            prepare_repair_application(&plan, &path, &findings, prepared.sources(), &authorization)
                .unwrap();
        assert_eq!(already.receipt().state(), RepairChangeState::AlreadyApplied);
        assert_eq!(already.sources(), prepared.sources());
    }

    #[test]
    fn failed_receipt_publication_rolls_back_the_source() {
        let (findings, path, proposal, sources) = fixture();
        let plan = build_repair_plan(
            RepairTool::new("pliegocss", "0.0.0").unwrap(),
            &path,
            &findings,
            &proposal,
            &sources,
        )
        .unwrap();
        let authorization = format!("sha256:{}", plan.plan_sha256());
        let prepared =
            prepare_repair_application(&plan, &path, &findings, &sources, &authorization).unwrap();
        let root = std::env::temp_dir().join(format!(
            "pliegocss-agent-publish-{}-{}",
            std::process::id(),
            REPAIR_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let source_path = root.join("src/app.css");
        fs::write(&source_path, &sources["src/app.css"]).unwrap();
        let source_paths = BTreeMap::from([("src/app.css".into(), source_path.clone())]);
        let receipt_path = root.join("change-receipt.json");
        let error = publish_repair_application_with(
            &prepared,
            &root,
            &source_paths,
            &sources,
            &receipt_path,
            |index, temporary, destination| {
                if index == 1 {
                    Err(std::io::Error::other("injected receipt failure"))
                } else {
                    fs::rename(temporary, destination)
                }
            },
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("all previous source bytes were restored")
        );
        assert_eq!(fs::read(source_path).unwrap(), sources["src/app.css"]);
        assert!(!receipt_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn receipt_destination_is_project_relative_and_alias_safe() {
        let cwd = env::current_dir().unwrap();
        let source = cwd.join("Cargo.toml");
        let sources = BTreeMap::from([("src/app.css".into(), source.clone())]);
        assert!(
            resolve_repair_receipt_path(Path::new("../escape.json"), [], &BTreeMap::new()).is_err()
        );
        assert!(
            resolve_repair_receipt_path(Path::new(".pliegocss-repair.lock"), [], &BTreeMap::new())
                .is_err()
        );
        assert!(resolve_repair_receipt_path(Path::new("Cargo.toml"), [], &sources).is_err());

        let receipt = Path::new("pliegocss-agent-receipt-path-test.json");
        assert_eq!(
            resolve_repair_receipt_path(receipt, [source.as_path()], &BTreeMap::new()).unwrap(),
            cwd.join(receipt)
        );
    }

    #[test]
    fn required_checks_accept_finding_schema_dotted_identifiers() {
        assert!(validate_checks(&["token-graph.resolved".into()]).is_ok());
        assert!(validate_checks(&["token-graph..resolved".into()]).is_err());
        assert!(validate_checks(&["Token-Graph.resolved".into()]).is_err());
    }

    #[test]
    fn built_in_verification_is_closed_deterministic_and_browser_honest() {
        let (findings, path, proposal, sources) = fixture();
        let plan = build_repair_plan(
            RepairTool::new("pliegocss", "0.0.0").unwrap(),
            &path,
            &findings,
            &proposal,
            &sources,
        )
        .unwrap();
        let authorization = format!("sha256:{}", plan.plan_sha256());
        let prepared =
            prepare_repair_application(&plan, &path, &findings, &sources, &authorization).unwrap();
        let change_bytes = prepared.receipt().to_json_pretty().unwrap();
        let mut policy = RepairCheckPolicy {
            schema_version: REPAIR_CHECK_POLICY_SCHEMA_VERSION.into(),
            checks: vec![RepairCheckDefinition {
                id: "audit".into(),
                kind: RepairCheckKind::StandardCssAudit,
                source: "src/app.css".into(),
                compatibility_profile: CompatibilityProfile::Modern,
                budget_policy: None,
                budget_subjects: Vec::new(),
                test_evidence: None,
                workspace_manifest: None,
                lockfile: None,
                browser_evidence_file: None,
                browser_profile_inputs: Vec::new(),
            }],
            browser_evidence: RepairBrowserRequirement::NotRequired,
        };
        let policy_bytes = policy.to_json_pretty().unwrap();
        assert_eq!(
            parse_repair_check_policy(policy_bytes.as_bytes()).unwrap(),
            policy
        );
        assert!(
            parse_repair_check_policy(serde_json::to_vec(&policy).unwrap().as_slice()).is_err()
        );
        let verified = execute_repair_verification(
            prepared.receipt(),
            "pliego.css.change-receipt.json",
            change_bytes.as_bytes(),
            "pliego.css.check-policy.json",
            policy_bytes.as_bytes(),
            &policy,
            prepared.sources(),
        )
        .unwrap();
        assert_eq!(verified.result(), RepairVerificationResult::Passed);
        let bytes = verified.to_json_pretty().unwrap();
        assert_eq!(
            parse_repair_verification_receipt(bytes.as_bytes()).unwrap(),
            verified
        );
        let value: serde_json::Value = serde_json::from_str(&bytes).unwrap();
        assert_eq!(value["checks"][0]["kind"], "standard-css-audit");
        assert_eq!(value["checks"][0]["status"], "passed");
        assert_eq!(value["browserEvidence"], "not-required");

        let mut legacy = verified.clone();
        legacy.schema_version = LEGACY_REPAIR_VERIFICATION_RECEIPT_SCHEMA_VERSION.into();
        legacy.receipt_sha256 = verification_receipt_payload_sha256(&legacy).unwrap();
        let legacy_bytes = legacy.to_json_pretty().unwrap();
        assert_eq!(
            parse_repair_verification_receipt(legacy_bytes.as_bytes()).unwrap(),
            legacy
        );

        policy.browser_evidence = RepairBrowserRequirement::Required;
        let policy_bytes = policy.to_json_pretty().unwrap();
        let blocked = execute_repair_verification(
            prepared.receipt(),
            "pliego.css.change-receipt.json",
            change_bytes.as_bytes(),
            "pliego.css.check-policy.json",
            policy_bytes.as_bytes(),
            &policy,
            prepared.sources(),
        )
        .unwrap();
        assert_eq!(blocked.result(), RepairVerificationResult::Blocked);
    }

    #[test]
    fn built_in_verification_records_a_failed_audit_without_executing_commands() {
        let (findings, path, mut proposal, sources) = fixture();
        proposal.edits[0].replacement = "{".into();
        let plan = build_repair_plan(
            RepairTool::new("pliegocss", "0.0.0").unwrap(),
            &path,
            &findings,
            &proposal,
            &sources,
        )
        .unwrap();
        let authorization = format!("sha256:{}", plan.plan_sha256());
        let prepared =
            prepare_repair_application(&plan, &path, &findings, &sources, &authorization).unwrap();
        let change_bytes = prepared.receipt().to_json_pretty().unwrap();
        let policy = RepairCheckPolicy {
            schema_version: REPAIR_CHECK_POLICY_SCHEMA_VERSION.into(),
            checks: vec![RepairCheckDefinition {
                id: "audit".into(),
                kind: RepairCheckKind::StandardCssAudit,
                source: "src/app.css".into(),
                compatibility_profile: CompatibilityProfile::Modern,
                budget_policy: None,
                budget_subjects: Vec::new(),
                test_evidence: None,
                workspace_manifest: None,
                lockfile: None,
                browser_evidence_file: None,
                browser_profile_inputs: Vec::new(),
            }],
            browser_evidence: RepairBrowserRequirement::NotRequired,
        };
        let policy_bytes = policy.to_json_pretty().unwrap();
        let receipt = execute_repair_verification(
            prepared.receipt(),
            "pliego.css.change-receipt.json",
            change_bytes.as_bytes(),
            "pliego.css.check-policy.json",
            policy_bytes.as_bytes(),
            &policy,
            prepared.sources(),
        )
        .unwrap();
        assert_eq!(receipt.result(), RepairVerificationResult::Failed);
    }

    #[test]
    fn token_graph_integrity_is_a_closed_versioned_check() {
        assert_eq!(bounded_verification_reason("bad\nreason"), "bad reason");
        assert_eq!(
            bounded_verification_reason(""),
            "token graph validation failed"
        );
        let definition = RepairCheckDefinition {
            id: "token-graph.integrity".into(),
            kind: RepairCheckKind::TokenGraphIntegrity,
            source: "dist/pliego.tokens.json".into(),
            compatibility_profile: CompatibilityProfile::None,
            budget_policy: None,
            budget_subjects: Vec::new(),
            test_evidence: None,
            workspace_manifest: None,
            lockfile: None,
            browser_evidence_file: None,
            browser_profile_inputs: Vec::new(),
        };
        assert!(
            definition
                .validate(LEGACY_REPAIR_CHECK_POLICY_SCHEMA_VERSION)
                .is_err()
        );
        definition
            .validate(REPAIR_CHECK_POLICY_SCHEMA_VERSION)
            .unwrap();
        let graph = token_graph_fixture();
        let bytes = graph.to_canonical_json().unwrap();
        let change_receipt_sha256 = "0".repeat(64);
        let (passed, passed_document) = execute_repair_check(
            &definition,
            &bytes,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &change_receipt_sha256,
        )
        .unwrap();
        assert_eq!(passed.status, RepairVerificationCheckStatus::Passed);
        assert_eq!(passed.kind, RepairCheckKind::TokenGraphIntegrity);
        let document = parse_finding_document(&passed_document).unwrap();
        assert_eq!(document.findings()[0].code(), "PCSS-TOKEN-000");

        let mut noncanonical = bytes;
        noncanonical.insert(0, b' ');
        let (failed, failed_document) = execute_repair_check(
            &definition,
            &noncanonical,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &change_receipt_sha256,
        )
        .unwrap();
        assert_eq!(failed.status, RepairVerificationCheckStatus::Failed);
        let document = parse_finding_document(&failed_document).unwrap();
        assert_eq!(document.findings()[0].code(), "PCSS-TOKEN-001");

        let wrong_profile = RepairCheckDefinition {
            compatibility_profile: CompatibilityProfile::Modern,
            ..definition
        };
        assert!(
            wrong_profile
                .validate(REPAIR_CHECK_POLICY_SCHEMA_VERSION)
                .is_err()
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn token_graph_repair_reaches_passed_verification() {
        let definition = RepairCheckDefinition {
            id: "token-graph.integrity".into(),
            kind: RepairCheckKind::TokenGraphIntegrity,
            source: "dist/pliego.tokens.json".into(),
            compatibility_profile: CompatibilityProfile::None,
            budget_policy: None,
            budget_subjects: Vec::new(),
            test_evidence: None,
            workspace_manifest: None,
            lockfile: None,
            browser_evidence_file: None,
            browser_profile_inputs: Vec::new(),
        };
        let graph = token_graph_fixture();
        let mut before = vec![b' '];
        before.extend_from_slice(&graph.to_canonical_json().unwrap());
        let finding = Finding::new(
            "PCSS-TOKEN-001",
            "token-graph",
            FindingSeverity::Error,
            "token graph has a noncanonical prefix",
            FindingVerification::Verified,
            FindingCause::new(
                "token-graph.integrity",
                "noncanonical-prefix",
                "one leading byte prevents canonical token-graph parsing",
            )
            .unwrap(),
        )
        .unwrap()
        .with_source(FindingSource::new("dist/pliego.tokens.json", 0, 1).unwrap())
        .unwrap()
        .with_suggestion(
            FindingSuggestion::new(
                1,
                "remove-prefix",
                "remove the noncanonical leading byte",
                FindingRisk::Low,
                "document",
            )
            .unwrap()
            .with_prerequisites(vec!["token-graph.integrity".into()])
            .unwrap(),
        )
        .unwrap();
        let fingerprint = finding.fingerprint().to_owned();
        let findings = FindingDocument::new(
            FindingTool::new("pliegocss", "0.0.0").unwrap(),
            "audit",
            vec![finding],
        )
        .unwrap()
        .to_json_pretty()
        .unwrap()
        .into_bytes();
        let proposal = RepairProposalDocument {
            schema_version: REPAIR_PROPOSAL_SCHEMA_VERSION.into(),
            finding_document_sha256: sha256_hex(&findings),
            change_budget: RepairChangeBudget::new(1, 1, 0, 1).unwrap(),
            required_checks: vec!["token-graph.integrity".into()],
            edits: vec![RepairProposalEdit {
                finding_fingerprint: fingerprint,
                suggestion_id: "remove-prefix".into(),
                file: "dist/pliego.tokens.json".into(),
                byte_start: 0,
                byte_end: 1,
                replacement: String::new(),
            }],
        };
        let sources = BTreeMap::from([("dist/pliego.tokens.json".into(), before)]);
        let plan = build_repair_plan(
            RepairTool::new("pliegocss", "0.0.0").unwrap(),
            "findings.json",
            &findings,
            &proposal,
            &sources,
        )
        .unwrap();
        let authorization = format!("sha256:{}", plan.plan_sha256());
        let prepared =
            prepare_repair_application(&plan, "findings.json", &findings, &sources, &authorization)
                .unwrap();
        let policy = RepairCheckPolicy {
            schema_version: REPAIR_CHECK_POLICY_SCHEMA_VERSION.into(),
            checks: vec![definition.clone()],
            browser_evidence: RepairBrowserRequirement::NotRequired,
        };
        let change_bytes = prepared.receipt().to_json_pretty().unwrap();
        let policy_bytes = policy.to_json_pretty().unwrap();
        let receipt = execute_repair_verification(
            prepared.receipt(),
            "change.json",
            change_bytes.as_bytes(),
            "checks.json",
            policy_bytes.as_bytes(),
            &policy,
            prepared.sources(),
        )
        .unwrap();
        assert_eq!(receipt.result(), RepairVerificationResult::Passed);
        let receipt_value: serde_json::Value =
            serde_json::from_str(&receipt.to_json_pretty().unwrap()).unwrap();
        assert_eq!(receipt_value["schemaVersion"], "1.4.0");
        assert_eq!(receipt_value["checks"][0]["kind"], "token-graph-integrity");
        assert_token_graph_schema_1_1_compatibility(policy, receipt);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fixed_test_evidence_is_canonical_identity_bound_and_source_additive() {
        let (findings, path, mut proposal, sources) = fixture();
        proposal.required_checks.push("tests.workspace".into());
        let plan = build_repair_plan(
            RepairTool::new("pliegocss", "0.0.0").unwrap(),
            &path,
            &findings,
            &proposal,
            &sources,
        )
        .unwrap();
        let authorization = format!("sha256:{}", plan.plan_sha256());
        let prepared =
            prepare_repair_application(&plan, &path, &findings, &sources, &authorization).unwrap();
        let change_bytes = prepared.receipt().to_json_pretty().unwrap();
        let manifest_bytes = b"[workspace]\nresolver = \"2\"\n".to_vec();
        let lockfile_bytes = b"version = 4\n".to_vec();
        let manifest = RepairArtifactIdentity {
            file: "Cargo.toml".into(),
            bytes: manifest_bytes.len(),
            sha256: sha256_hex(&manifest_bytes),
        };
        let lockfile = RepairArtifactIdentity {
            file: "Cargo.lock".into(),
            bytes: lockfile_bytes.len(),
            sha256: sha256_hex(&lockfile_bytes),
        };
        let mut evidence = RepairTestEvidence {
            schema_version: REPAIR_TEST_EVIDENCE_SCHEMA_VERSION.into(),
            evidence_sha256: "0".repeat(64),
            change_receipt_sha256: prepared.receipt().receipt_sha256.clone(),
            check_id: "tests.workspace".into(),
            profile: RepairTestProfile::RustWorkspaceAllTargets,
            runner: RepairTestRunnerIdentity {
                name: "pliego-css-agent".into(),
                version: "0.0.0".into(),
            },
            toolchain: RepairTestToolchainIdentity {
                cargo_version: "cargo 1.85.0 (fixture)".into(),
                rustc_version: "rustc 1.85.0 (fixture)".into(),
                host: "x86_64-unknown-linux-gnu".into(),
            },
            workspace_manifest: manifest.clone(),
            lockfile: lockfile.clone(),
            result: RepairTestEvidenceResult::Passed,
            exit_code: 0,
        };
        evidence.evidence_sha256 = repair_test_evidence_payload_sha256(&evidence).unwrap();
        let evidence_bytes = evidence.to_json_pretty().unwrap().into_bytes();
        assert_eq!(
            parse_repair_test_evidence(&evidence_bytes).unwrap(),
            evidence
        );
        let evidence_identity = RepairArtifactIdentity {
            file: "pliego.css.test-evidence.json".into(),
            bytes: evidence_bytes.len(),
            sha256: sha256_hex(&evidence_bytes),
        };
        let policy = RepairCheckPolicy {
            schema_version: REPAIR_CHECK_POLICY_SCHEMA_VERSION.into(),
            checks: vec![
                RepairCheckDefinition {
                    id: "audit".into(),
                    kind: RepairCheckKind::StandardCssAudit,
                    source: "src/app.css".into(),
                    compatibility_profile: CompatibilityProfile::Modern,
                    budget_policy: None,
                    budget_subjects: Vec::new(),
                    test_evidence: None,
                    workspace_manifest: None,
                    lockfile: None,
                    browser_evidence_file: None,
                    browser_profile_inputs: Vec::new(),
                },
                RepairCheckDefinition {
                    id: "tests.workspace".into(),
                    kind: RepairCheckKind::TestSuiteEvidence,
                    source: "src/app.css".into(),
                    compatibility_profile: CompatibilityProfile::None,
                    budget_policy: None,
                    budget_subjects: Vec::new(),
                    test_evidence: Some(evidence_identity.clone()),
                    workspace_manifest: Some(manifest.clone()),
                    lockfile: Some(lockfile.clone()),
                    browser_evidence_file: None,
                    browser_profile_inputs: Vec::new(),
                },
            ],
            browser_evidence: RepairBrowserRequirement::NotRequired,
        };
        let policy_bytes = policy.to_json_pretty().unwrap();
        let inputs = BTreeMap::from([
            (evidence_identity.file.clone(), evidence_bytes.clone()),
            (manifest.file.clone(), manifest_bytes),
            (lockfile.file.clone(), lockfile_bytes),
        ]);
        let receipt = execute_repair_verification_with_inputs(
            prepared.receipt(),
            "pliego.css.change-receipt.json",
            change_bytes.as_bytes(),
            "pliego.css.check-policy.json",
            policy_bytes.as_bytes(),
            &policy,
            RepairVerificationInputs::new(prepared.sources(), &inputs),
        )
        .unwrap();
        assert_eq!(receipt.result(), RepairVerificationResult::Passed);
        assert_eq!(receipt.checks[1].kind, RepairCheckKind::TestSuiteEvidence);
        assert_eq!(receipt.checks[1].test_evidence, Some(evidence_identity));

        let mut stale = evidence;
        stale.change_receipt_sha256 = "f".repeat(64);
        stale.evidence_sha256 = repair_test_evidence_payload_sha256(&stale).unwrap();
        let stale_bytes = stale.to_json_pretty().unwrap().into_bytes();
        let mut stale_policy = policy;
        stale_policy.checks[1].test_evidence = Some(RepairArtifactIdentity {
            file: "pliego.css.test-evidence.json".into(),
            bytes: stale_bytes.len(),
            sha256: sha256_hex(&stale_bytes),
        });
        let stale_policy_bytes = stale_policy.to_json_pretty().unwrap();
        let stale_inputs = BTreeMap::from([
            ("pliego.css.test-evidence.json".into(), stale_bytes),
            (
                "Cargo.toml".into(),
                b"[workspace]\nresolver = \"2\"\n".to_vec(),
            ),
            ("Cargo.lock".into(), b"version = 4\n".to_vec()),
        ]);
        assert!(
            execute_repair_verification_with_inputs(
                prepared.receipt(),
                "pliego.css.change-receipt.json",
                change_bytes.as_bytes(),
                "pliego.css.check-policy.json",
                stale_policy_bytes.as_bytes(),
                &stale_policy,
                RepairVerificationInputs::new(prepared.sources(), &stale_inputs),
            )
            .is_err()
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fixed_browser_evidence_unblocks_required_verification_and_fails_closed() {
        let (findings, path, mut proposal, sources) = fixture();
        proposal.required_checks.push("browser.pliegors".into());
        let plan = build_repair_plan(
            RepairTool::new("pliegocss", "0.0.0").unwrap(),
            &path,
            &findings,
            &proposal,
            &sources,
        )
        .unwrap();
        let authorization = format!("sha256:{}", plan.plan_sha256());
        let prepared =
            prepare_repair_application(&plan, &path, &findings, &sources, &authorization).unwrap();
        let change_bytes = prepared.receipt().to_json_pretty().unwrap();
        let profile_bytes = REPAIR_BROWSER_PROFILE_FILES
            .iter()
            .map(|file| ((*file).to_owned(), format!("fixture:{file}\n").into_bytes()))
            .collect::<BTreeMap<_, _>>();
        let profile_inputs = profile_bytes
            .iter()
            .map(|(file, bytes)| RepairArtifactIdentity {
                file: file.clone(),
                bytes: bytes.len(),
                sha256: sha256_hex(bytes),
            })
            .collect::<Vec<_>>();
        let mut evidence = RepairBrowserEvidence {
            schema_version: REPAIR_BROWSER_EVIDENCE_SCHEMA_VERSION.into(),
            evidence_sha256: "0".repeat(64),
            change_receipt_sha256: prepared.receipt().receipt_sha256.clone(),
            check_id: "browser.pliegors".into(),
            profile: RepairBrowserProfile::PliegorsVisitCounterChromiumCdp,
            runner: RepairBrowserRunnerIdentity {
                name: "pliego-css-agent".into(),
                version: "0.0.0".into(),
                node_version: "v24.0.0".into(),
            },
            profile_inputs: profile_inputs.clone(),
            browser: RepairBrowserIdentity {
                family: "chromium".into(),
                executable: "chrome.exe".into(),
                product: "HeadlessChrome/138.0.0.0".into(),
                revision: "fixture".into(),
                protocol_version: "1.3".into(),
                js_version: "13.8.0".into(),
                user_agent: "fixture HeadlessChrome/138.0.0.0".into(),
            },
            observation: RepairBrowserObservation {
                node_identity: RepairBrowserNodeIdentity {
                    document: true,
                    island: true,
                    button: true,
                    value: true,
                },
                island_count: 1,
                island_id: "visit-counter".into(),
                initial_minutes: 15,
                final_minutes: 20,
                increment: 5,
                initial_text: "15".into(),
                final_text: "20".into(),
                initial_class: "pc_abc123".into(),
                final_class: "pc_abc123".into(),
                event_count: 1,
                event_key: "minutes".into(),
                event_value: 20,
                module_script_count: 2,
                preload_entry_count: 1,
                preload_request_count: 1,
                client_request_count: 4,
                stylesheet_present: true,
                wasm_ready: true,
                relevant_event_count: 0,
                server_error_count: 0,
            },
            result: RepairBrowserEvidenceResult::Passed,
            exit_code: 0,
        };
        evidence.evidence_sha256 = repair_browser_evidence_payload_sha256(&evidence).unwrap();
        let evidence_bytes = evidence.to_json_pretty().unwrap().into_bytes();
        assert_eq!(
            parse_repair_browser_evidence(&evidence_bytes).unwrap(),
            evidence
        );
        let evidence_identity = RepairArtifactIdentity {
            file: "pliego.css.browser-evidence.json".into(),
            bytes: evidence_bytes.len(),
            sha256: sha256_hex(&evidence_bytes),
        };
        let mut policy = RepairCheckPolicy {
            schema_version: REPAIR_CHECK_POLICY_SCHEMA_VERSION.into(),
            checks: vec![
                RepairCheckDefinition {
                    id: "audit".into(),
                    kind: RepairCheckKind::StandardCssAudit,
                    source: "src/app.css".into(),
                    compatibility_profile: CompatibilityProfile::Modern,
                    budget_policy: None,
                    budget_subjects: Vec::new(),
                    test_evidence: None,
                    workspace_manifest: None,
                    lockfile: None,
                    browser_evidence_file: None,
                    browser_profile_inputs: Vec::new(),
                },
                RepairCheckDefinition {
                    id: "browser.pliegors".into(),
                    kind: RepairCheckKind::BrowserEvidence,
                    source: "src/app.css".into(),
                    compatibility_profile: CompatibilityProfile::None,
                    budget_policy: None,
                    budget_subjects: Vec::new(),
                    test_evidence: None,
                    workspace_manifest: None,
                    lockfile: None,
                    browser_evidence_file: Some(evidence_identity.clone()),
                    browser_profile_inputs: profile_inputs.clone(),
                },
            ],
            browser_evidence: RepairBrowserRequirement::Required,
        };
        let mut inconsistent_policy = policy.clone();
        inconsistent_policy.browser_evidence = RepairBrowserRequirement::NotRequired;
        assert!(inconsistent_policy.to_json_pretty().is_err());
        let policy_bytes = policy.to_json_pretty().unwrap();
        let mut inputs = profile_bytes.clone();
        inputs.insert(evidence_identity.file.clone(), evidence_bytes);
        let receipt = execute_repair_verification_with_inputs(
            prepared.receipt(),
            "pliego.css.change-receipt.json",
            change_bytes.as_bytes(),
            "pliego.css.check-policy.json",
            policy_bytes.as_bytes(),
            &policy,
            RepairVerificationInputs::new(prepared.sources(), &inputs),
        )
        .unwrap();
        assert_eq!(receipt.result(), RepairVerificationResult::Passed);
        assert_eq!(
            receipt.browser_evidence,
            RepairVerificationBrowserEvidence::RequiredPassed
        );

        evidence.observation.final_minutes = 21;
        evidence.result = RepairBrowserEvidenceResult::Failed;
        evidence.exit_code = 1;
        evidence.evidence_sha256 = repair_browser_evidence_payload_sha256(&evidence).unwrap();
        let failed_bytes = evidence.to_json_pretty().unwrap().into_bytes();
        let failed_identity = RepairArtifactIdentity {
            file: "pliego.css.browser-evidence.json".into(),
            bytes: failed_bytes.len(),
            sha256: sha256_hex(&failed_bytes),
        };
        policy.checks[1].browser_evidence_file = Some(failed_identity.clone());
        let failed_policy_bytes = policy.to_json_pretty().unwrap();
        inputs.insert(failed_identity.file.clone(), failed_bytes);
        let failed = execute_repair_verification_with_inputs(
            prepared.receipt(),
            "pliego.css.change-receipt.json",
            change_bytes.as_bytes(),
            "pliego.css.check-policy.json",
            failed_policy_bytes.as_bytes(),
            &policy,
            RepairVerificationInputs::new(prepared.sources(), &inputs),
        )
        .unwrap();
        assert_eq!(failed.result(), RepairVerificationResult::Failed);
        assert_eq!(
            failed.browser_evidence,
            RepairVerificationBrowserEvidence::RequiredFailed
        );

        evidence.change_receipt_sha256 = "f".repeat(64);
        evidence.evidence_sha256 = repair_browser_evidence_payload_sha256(&evidence).unwrap();
        let stale_bytes = evidence.to_json_pretty().unwrap().into_bytes();
        policy.checks[1].browser_evidence_file = Some(RepairArtifactIdentity {
            file: "pliego.css.browser-evidence.json".into(),
            bytes: stale_bytes.len(),
            sha256: sha256_hex(&stale_bytes),
        });
        let stale_policy_bytes = policy.to_json_pretty().unwrap();
        inputs.insert("pliego.css.browser-evidence.json".into(), stale_bytes);
        assert!(
            execute_repair_verification_with_inputs(
                prepared.receipt(),
                "pliego.css.change-receipt.json",
                change_bytes.as_bytes(),
                "pliego.css.check-policy.json",
                stale_policy_bytes.as_bytes(),
                &policy,
                RepairVerificationInputs::new(prepared.sources(), &inputs),
            )
            .is_err()
        );
    }

    #[test]
    fn css_budget_audit_is_identity_bound_and_schema_closed() {
        let css = b".button { color: red; }\n".to_vec();
        let policy_bytes = budget_policy_fixture("src/app.css", 0);
        let definition = RepairCheckDefinition {
            id: "budget".into(),
            kind: RepairCheckKind::CssBudgetAudit,
            source: "src/app.css".into(),
            compatibility_profile: CompatibilityProfile::Modern,
            budget_policy: Some(RepairArtifactIdentity {
                file: "config/css-budgets.json".into(),
                bytes: policy_bytes.len(),
                sha256: sha256_hex(&policy_bytes),
            }),
            budget_subjects: Vec::new(),
            test_evidence: None,
            workspace_manifest: None,
            lockfile: None,
            browser_evidence_file: None,
            browser_profile_inputs: Vec::new(),
        };
        assert!(
            definition
                .validate(TOKEN_GRAPH_REPAIR_CHECK_POLICY_SCHEMA_VERSION)
                .is_err()
        );
        definition
            .validate(REPAIR_CHECK_POLICY_SCHEMA_VERSION)
            .unwrap();

        let inputs = BTreeMap::from([("config/css-budgets.json".into(), policy_bytes.clone())]);
        let change_receipt_sha256 = "0".repeat(64);
        let (check, document_bytes) = execute_repair_check(
            &definition,
            &css,
            &BTreeMap::new(),
            &inputs,
            &change_receipt_sha256,
        )
        .unwrap();
        assert_eq!(check.kind, RepairCheckKind::CssBudgetAudit);
        assert_eq!(check.status, RepairVerificationCheckStatus::Failed);
        assert_eq!(check.budget_policy, definition.budget_policy);
        let document = parse_finding_document(&document_bytes).unwrap();
        assert!(
            document
                .findings()
                .iter()
                .any(|finding| finding.code() == "PCSS-BUDGET-101")
        );

        let mut noncanonical = policy_bytes;
        noncanonical.insert(0, b' ');
        let noncanonical_inputs =
            BTreeMap::from([("config/css-budgets.json".into(), noncanonical)]);
        assert!(
            execute_repair_check(
                &definition,
                &css,
                &BTreeMap::new(),
                &noncanonical_inputs,
                &change_receipt_sha256,
            )
            .is_err()
        );

        let invalid_subject = RepairCheckDefinition {
            budget_subjects: vec![
                BudgetSubject::new(BudgetSubjectKind::File, "src/app.css").unwrap(),
            ],
            ..definition
        };
        assert!(
            invalid_subject
                .validate(REPAIR_CHECK_POLICY_SCHEMA_VERSION)
                .is_err()
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn repair_verification_executes_exact_supplementary_budget_policy() {
        let (findings, path, mut proposal, sources) = fixture();
        proposal.required_checks.push("budget".into());
        let plan = build_repair_plan(
            RepairTool::new("pliegocss", "0.0.0").unwrap(),
            &path,
            &findings,
            &proposal,
            &sources,
        )
        .unwrap();
        let authorization = format!("sha256:{}", plan.plan_sha256());
        let prepared =
            prepare_repair_application(&plan, &path, &findings, &sources, &authorization).unwrap();
        let change_bytes = prepared.receipt().to_json_pretty().unwrap();
        let budget_bytes = budget_policy_fixture("src/app.css", 0);
        let budget_identity = RepairArtifactIdentity {
            file: "config/css-budgets.json".into(),
            bytes: budget_bytes.len(),
            sha256: sha256_hex(&budget_bytes),
        };
        let policy = RepairCheckPolicy {
            schema_version: REPAIR_CHECK_POLICY_SCHEMA_VERSION.into(),
            checks: vec![
                RepairCheckDefinition {
                    id: "audit".into(),
                    kind: RepairCheckKind::StandardCssAudit,
                    source: "src/app.css".into(),
                    compatibility_profile: CompatibilityProfile::Modern,
                    budget_policy: None,
                    budget_subjects: Vec::new(),
                    test_evidence: None,
                    workspace_manifest: None,
                    lockfile: None,
                    browser_evidence_file: None,
                    browser_profile_inputs: Vec::new(),
                },
                RepairCheckDefinition {
                    id: "budget".into(),
                    kind: RepairCheckKind::CssBudgetAudit,
                    source: "src/app.css".into(),
                    compatibility_profile: CompatibilityProfile::Modern,
                    budget_policy: Some(budget_identity.clone()),
                    budget_subjects: Vec::new(),
                    test_evidence: None,
                    workspace_manifest: None,
                    lockfile: None,
                    browser_evidence_file: None,
                    browser_profile_inputs: Vec::new(),
                },
            ],
            browser_evidence: RepairBrowserRequirement::NotRequired,
        };
        let policy_bytes = policy.to_json_pretty().unwrap();
        let inputs = BTreeMap::from([(budget_identity.file.clone(), budget_bytes)]);

        assert!(
            execute_repair_verification(
                prepared.receipt(),
                "pliego.css.change-receipt.json",
                change_bytes.as_bytes(),
                "pliego.css.check-policy.json",
                policy_bytes.as_bytes(),
                &policy,
                prepared.sources(),
            )
            .is_err()
        );
        let receipt = execute_repair_verification_with_inputs(
            prepared.receipt(),
            "pliego.css.change-receipt.json",
            change_bytes.as_bytes(),
            "pliego.css.check-policy.json",
            policy_bytes.as_bytes(),
            &policy,
            RepairVerificationInputs::new(prepared.sources(), &inputs),
        )
        .unwrap();
        assert_eq!(receipt.result(), RepairVerificationResult::Failed);
        let receipt_bytes = receipt.to_json_pretty().unwrap();
        let receipt_value: serde_json::Value = serde_json::from_str(&receipt_bytes).unwrap();
        assert_eq!(receipt_value["schemaVersion"], "1.4.0");
        assert_eq!(
            receipt_value["checks"][1]["budgetPolicy"]["file"],
            "config/css-budgets.json"
        );
        assert_eq!(
            parse_repair_verification_receipt(receipt_bytes.as_bytes()).unwrap(),
            receipt
        );

        let mut drifted = inputs;
        drifted
            .get_mut("config/css-budgets.json")
            .unwrap()
            .push(b' ');
        assert!(
            execute_repair_verification_with_inputs(
                prepared.receipt(),
                "pliego.css.change-receipt.json",
                change_bytes.as_bytes(),
                "pliego.css.check-policy.json",
                policy_bytes.as_bytes(),
                &policy,
                RepairVerificationInputs::new(prepared.sources(), &drifted),
            )
            .is_err()
        );
    }

    #[test]
    fn checked_verification_publishes_once_under_the_repair_lock() {
        let (findings, path, proposal, sources) = fixture();
        let plan = build_repair_plan(
            RepairTool::new("pliegocss", "0.0.0").unwrap(),
            &path,
            &findings,
            &proposal,
            &sources,
        )
        .unwrap();
        let authorization = format!("sha256:{}", plan.plan_sha256());
        let prepared =
            prepare_repair_application(&plan, &path, &findings, &sources, &authorization).unwrap();
        let policy = RepairCheckPolicy {
            schema_version: REPAIR_CHECK_POLICY_SCHEMA_VERSION.into(),
            checks: vec![RepairCheckDefinition {
                id: "audit".into(),
                kind: RepairCheckKind::StandardCssAudit,
                source: "src/app.css".into(),
                compatibility_profile: CompatibilityProfile::Modern,
                budget_policy: None,
                budget_subjects: Vec::new(),
                test_evidence: None,
                workspace_manifest: None,
                lockfile: None,
                browser_evidence_file: None,
                browser_profile_inputs: Vec::new(),
            }],
            browser_evidence: RepairBrowserRequirement::NotRequired,
        };
        let directory_name = format!(
            "pliegocss-agent-verify-{}-{}",
            std::process::id(),
            REPAIR_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let cwd = env::current_dir().unwrap();
        let root = cwd.join(&directory_name);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/app.css"), &prepared.sources()["src/app.css"]).unwrap();
        fs::write(
            root.join("change.json"),
            prepared.receipt().to_json_pretty().unwrap(),
        )
        .unwrap();
        fs::write(root.join("checks.json"), policy.to_json_pretty().unwrap()).unwrap();
        let change = PathBuf::from(&directory_name).join("change.json");
        let checks = PathBuf::from(&directory_name).join("checks.json");
        let output = PathBuf::from(&directory_name).join("verified.json");
        let published = verify_repair_change_checked(&change, &checks, &root, &output).unwrap();
        assert!(published.changed());
        assert_eq!(
            published.receipt().result(),
            RepairVerificationResult::Passed
        );
        assert_eq!(
            fs::read(root.join("verified.json")).unwrap(),
            published.receipt_bytes()
        );
        assert_eq!(published.finding_documents().len(), 1);
        let document = &published.finding_documents()[0];
        assert_eq!(document.check_id(), "audit");
        assert!(document.changed());
        let document_path = cwd.join(document.file());
        assert_eq!(fs::read(&document_path).unwrap(), document.bytes());
        let parsed = parse_finding_document(document.bytes()).unwrap();
        let receipt_value: serde_json::Value =
            serde_json::from_slice(published.receipt_bytes()).unwrap();
        assert_eq!(
            receipt_value["checks"][0]["findingDocumentBytes"],
            document.bytes().len()
        );
        assert_eq!(
            receipt_value["checks"][0]["findingDocumentSha256"],
            sha256_hex(document.bytes())
        );
        assert_eq!(
            receipt_value["checks"][0]["findingCount"],
            parsed.findings().len()
        );
        let repeated = verify_repair_change_checked(&change, &checks, &root, &output).unwrap();
        assert!(!repeated.changed());
        assert_eq!(repeated.receipt_bytes(), published.receipt_bytes());
        assert!(!repeated.finding_documents()[0].changed());
        assert_eq!(
            repeated.finding_documents()[0].bytes(),
            published.finding_documents()[0].bytes()
        );
        fs::remove_file(root.join("verified.json")).unwrap();
        fs::write(&document_path, b"{}\n").unwrap();
        let collision = verify_repair_change_checked(&change, &checks, &root, &output).unwrap_err();
        assert!(
            collision
                .to_string()
                .contains("contains different evidence")
        );
        assert!(!root.join("verified.json").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verification_receipt_failure_rolls_back_new_finding_documents() {
        let documents = BTreeMap::from([(
            "audit".into(),
            FindingDocument::new(
                FindingTool::new("pliegocss", "0.0.0").unwrap(),
                "audit",
                Vec::new(),
            )
            .unwrap()
            .to_json_pretty()
            .unwrap()
            .into_bytes(),
        )]);
        let root = env::temp_dir().join(format!(
            "pliegocss-agent-verification-rollback-{}-{}",
            std::process::id(),
            REPAIR_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let finding_path = root.join("finding.json");
        let receipt_path = root.join("verified.json");
        let outputs = BTreeMap::from([(
            "audit".into(),
            RepairFindingDocumentOutput {
                path: finding_path.clone(),
                file: "finding.json".into(),
            },
        )]);
        let error = publish_repair_verification_documents_with(
            &documents,
            &outputs,
            &receipt_path,
            b"receipt\n",
            |index, temporary, destination| {
                if index == 1 {
                    Err(std::io::Error::other("injected receipt failure"))
                } else {
                    fs::hard_link(temporary, destination)
                }
            },
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("all newly linked verification evidence was removed")
        );
        assert!(!finding_path.exists());
        assert!(!receipt_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn proposal_rejects_budget_escape_overlap_and_hash_drift() {
        let (findings, path, mut proposal, sources) = fixture();
        proposal.change_budget = RepairChangeBudget::new(1, 1, 3, 4).unwrap();
        assert!(
            build_repair_plan(
                RepairTool::new("pliegocss", "0.0.0").unwrap(),
                &path,
                &findings,
                &proposal,
                &sources,
            )
            .unwrap_err()
            .to_string()
            .contains("exceeds budget")
        );
        proposal.change_budget = RepairChangeBudget::new(1, 2, 8, 8).unwrap();
        proposal.edits.push(proposal.edits[0].clone());
        assert!(proposal.validate().is_err());
        proposal.edits.pop();
        proposal.finding_document_sha256 = "0".repeat(64);
        assert!(
            build_repair_plan(
                RepairTool::new("pliegocss", "0.0.0").unwrap(),
                &path,
                &findings,
                &proposal,
                &sources,
            )
            .is_err()
        );
    }

    #[test]
    fn low_risk_verified_unexcepted_authority_is_mandatory() {
        for (verification, risk, excepted, expected) in [
            (
                FindingVerification::Verified,
                FindingRisk::Medium,
                false,
                "accepts only low-risk",
            ),
            (
                FindingVerification::Unverified,
                FindingRisk::Low,
                false,
                "is not verified",
            ),
            (
                FindingVerification::Verified,
                FindingRisk::Low,
                true,
                "reviewed exception",
            ),
        ] {
            let (findings, path, proposal, sources) = fixture_with(verification, risk, excepted);
            let error = build_repair_plan(
                RepairTool::new("pliegocss", "0.0.0").unwrap(),
                &path,
                &findings,
                &proposal,
                &sources,
            )
            .unwrap_err();
            assert!(error.to_string().contains(expected));
        }
    }

    #[test]
    fn edits_cannot_escape_the_finding_source_range() {
        let (findings, path, mut proposal, sources) = fixture();
        proposal.edits[0].byte_start -= 1;
        assert!(
            build_repair_plan(
                RepairTool::new("pliegocss", "0.0.0").unwrap(),
                &path,
                &findings,
                &proposal,
                &sources,
            )
            .is_err()
        );
    }

    #[test]
    fn tampered_plan_fails_validation() {
        let (findings, path, proposal, sources) = fixture();
        let plan = build_repair_plan(
            RepairTool::new("pliegocss", "0.0.0").unwrap(),
            &path,
            &findings,
            &proposal,
            &sources,
        )
        .unwrap();
        let mut value = serde_json::to_value(&plan).unwrap();
        value["patch"]["text"] = serde_json::json!("tampered\n");
        let tampered = serde_json::to_vec(&value).unwrap();
        assert!(parse_repair_plan(&tampered).is_err());
    }
}
