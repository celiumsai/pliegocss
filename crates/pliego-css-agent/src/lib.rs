//! Deterministic bounded repair plans, explicit application, and verification evidence.

#![forbid(unsafe_code)]

mod artifact_identity;
mod browser_evidence;
mod check_policy;
mod cli;
mod plan;
mod proposal;
mod publication;
mod receipt;
mod repair_state;
mod source_transition;
mod static_verification;
mod test_evidence;
mod validation;
mod verification_receipt;

pub use artifact_identity::RepairArtifactIdentity;
pub use browser_evidence::{
    RepairBrowserEvidence, RepairBrowserEvidenceResult, RepairBrowserIdentity,
    RepairBrowserNodeIdentity, RepairBrowserObservation, RepairBrowserProfile,
    RepairBrowserRunnerIdentity, parse_repair_browser_evidence,
};
use browser_evidence::{repair_browser_evidence_payload_sha256, validate_browser_profile_inputs};
pub use check_policy::{
    RepairBrowserRequirement, RepairCheckDefinition, RepairCheckKind, RepairCheckPolicy,
    parse_repair_check_policy,
};
pub use cli::{
    RepairCliCommand, RepairCliFormat, RepairFixCliArgs, RepairFixMode, RepairPlanCliArgs,
    parse_repair_cli_arguments,
};
pub use plan::{
    RepairChangeSummary, RepairPatch, RepairPlanDocument, RepairPlannedEdit, build_repair_plan,
    parse_repair_plan,
};
pub use proposal::{
    RepairChangeBudget, RepairProposalDocument, RepairProposalEdit, parse_repair_proposal,
};
#[cfg(test)]
use publication::REPAIR_TEMP_COUNTER;
#[cfg(test)]
use publication::publish_repair_application_with;
pub use publication::{
    PreparedRepairApplication, PublishedRepairApplication, apply_repair_plan_checked,
    publish_repair_application, publish_repair_application_checked, resolve_repair_receipt_path,
};
use publication::{PreparedRepairWrite, acquire_repair_lock, is_repair_link_like, prepare_write};
pub use receipt::{
    RepairChangeReceipt, RepairChangeState, RepairDryRunReport, RepairDryRunState,
    parse_repair_change_receipt,
};
use receipt::{RepairReceiptCheck, receipt_payload_sha256};
pub use repair_state::{prepare_repair_application, verify_repair_plan};
pub use source_transition::RepairSourceTransition;

#[cfg(test)]
use static_verification::execute_repair_check;
pub use static_verification::{
    RepairVerificationInputs, execute_repair_verification, execute_repair_verification_with_inputs,
};
use static_verification::{
    bounded_verification_reason, prepare_repair_verification, validate_repair_after_sources,
};
use test_evidence::repair_test_evidence_payload_sha256;
pub use test_evidence::{
    RepairTestEvidence, RepairTestEvidenceResult, RepairTestProfile, RepairTestRunnerIdentity,
    RepairTestToolchainIdentity, parse_repair_test_evidence,
};
use validation::{validate_dotted_id, validate_logical_path, validate_slug, validate_text};
pub use verification_receipt::{
    RepairVerificationBrowserEvidence, RepairVerificationCheckStatus, RepairVerificationReceipt,
    RepairVerificationResult, parse_repair_verification_receipt,
};
use verification_receipt::{
    RepairVerificationCheck, derive_verification_result, verification_receipt_payload_sha256,
};

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
#[cfg(test)]
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use pliego_css_build::artifacts::{parse_finding_document, sha256_hex};
use serde::{Deserialize, Serialize};

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
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(20);
const REPAIR_BROWSER_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const REPAIR_RUST_TEST_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const TOOL_IDENTITY_TIMEOUT: Duration = Duration::from_secs(10);
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

    let mut command = Command::new("cargo");
    command
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
        );
    let test_output = run_bounded_command(
        &mut command,
        "fixed Cargo test profile",
        REPAIR_RUST_TEST_TIMEOUT,
        MAX_DOCUMENT_BYTES,
    )?;
    let exit_code = test_output.status.code().ok_or_else(|| {
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
    let mut command = Command::new("node");
    command
        .current_dir(&root)
        .arg("scripts/check-pliegors-browser.mjs")
        .env("PLIEGOCSS_AGENT_REPORT", "1")
        .env("PLIEGOCSS_SKIP_PLIEGORS_BUILD", "0")
        .env("PLIEGOCSS_KEEP_PLIEGORS_SITE", "0")
        .env("NO_COLOR", "1");
    let output_result = run_bounded_command(
        &mut command,
        "fixed PliegoRS Chromium profile",
        REPAIR_BROWSER_TIMEOUT,
        MAX_DOCUMENT_BYTES,
    )?;
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
    let mut command = Command::new(program);
    command.current_dir(root).args(arguments);
    let output = run_bounded_command(
        &mut command,
        &format!("fixed test tool `{program}` identity command"),
        TOOL_IDENTITY_TIMEOUT,
        4_096,
    )?;
    if !output.status.success() {
        return Err(RepairContractError::new(format!(
            "fixed test tool `{program}` identity command failed"
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

fn run_bounded_command(
    command: &mut Command,
    operation: &str,
    timeout: Duration,
    maximum_stream_bytes: usize,
) -> Result<Output, RepairContractError> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| RepairContractError::new(format!("cannot launch {operation}: {error}")))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| RepairContractError::new(format!("{operation} stdout is unavailable")))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| RepairContractError::new(format!("{operation} stderr is unavailable")))?;
    let stdout_reader =
        thread::spawn(move || read_bounded_process_stream(&mut stdout, maximum_stream_bytes));
    let stderr_reader =
        thread::spawn(move || read_bounded_process_stream(&mut stderr, maximum_stream_bytes));
    let deadline = Instant::now() + timeout;

    loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            RepairContractError::new(format!("cannot poll {operation}: {error}"))
        })? {
            return Ok(Output {
                status,
                stdout: join_process_stream(stdout_reader, operation, "stdout")?,
                stderr: join_process_stream(stderr_reader, operation, "stderr")?,
            });
        }
        if Instant::now() >= deadline {
            child.kill().map_err(|error| {
                RepairContractError::new(format!("cannot stop timed-out {operation}: {error}"))
            })?;
            child.wait().map_err(|error| {
                RepairContractError::new(format!("cannot reap timed-out {operation}: {error}"))
            })?;
            let _ = join_process_stream(stdout_reader, operation, "stdout");
            let _ = join_process_stream(stderr_reader, operation, "stderr");
            return Err(RepairContractError::new(format!(
                "{operation} exceeded its {} second deadline",
                timeout.as_secs()
            )));
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

fn read_bounded_process_stream(reader: &mut impl Read, maximum: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take((maximum + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(io::Error::other(format!(
            "process stream exceeds {maximum} bytes"
        )));
    }
    Ok(bytes)
}

fn join_process_stream(
    reader: thread::JoinHandle<io::Result<Vec<u8>>>,
    operation: &str,
    stream: &str,
) -> Result<Vec<u8>, RepairContractError> {
    reader
        .join()
        .map_err(|_| RepairContractError::new(format!("{operation} {stream} reader panicked")))?
        .map_err(|error| {
            RepairContractError::new(format!("cannot read {operation} {stream}: {error}"))
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_process_streams_are_bounded() {
        let mut within = io::Cursor::new(vec![b'x'; 8]);
        assert_eq!(
            read_bounded_process_stream(&mut within, 8).unwrap().len(),
            8
        );

        let mut excessive = io::Cursor::new(vec![b'x'; 9]);
        assert!(
            read_bounded_process_stream(&mut excessive, 8)
                .unwrap_err()
                .to_string()
                .contains("exceeds 8 bytes")
        );
    }
    use pliego_css_build::artifacts::{
        CompatibilityProfile, Finding, FindingCause, FindingDocument, FindingException,
        FindingRisk, FindingSeverity, FindingSource, FindingSuggestion, FindingTool,
        FindingVerification,
    };
    use pliego_css_config::{
        BudgetSubject, BudgetSubjectKind, TOKEN_GRAPH_VERSION, TokenGraph, TokenGraphTheme,
        TokenGraphToken, parse_budget_policy,
    };

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
