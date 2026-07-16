//! Closed control-manifest and build-receipt wire contracts.
//!
//! The control manifest describes what `PliegoCSS` read, decided, measured, and emitted. The build
//! receipt hashes the exact manifest bytes and records verification results. Keeping the manifest
//! free of a receipt hash makes the integrity graph acyclic.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod accessibility;

#[cfg(feature = "projection")]
pub mod projection;

/// Fixed control-manifest filename.
pub const CONTROL_MANIFEST_FILE: &str = "pliego.css.manifest.json";
/// Fixed build-receipt filename.
pub const BUILD_RECEIPT_FILE: &str = "pliego.css.receipt.json";
/// Fixed canonical token-graph filename.
pub const TOKEN_GRAPH_FILE: &str = "pliego.tokens.json";
/// Control-manifest document discriminator.
pub const CONTROL_MANIFEST_KIND: &str = "pliego-css-control-manifest";
/// Build-receipt document discriminator.
pub const BUILD_RECEIPT_KIND: &str = "pliego-css-build-receipt";
/// Current semantic wire version shared by both control artifacts.
pub const CONTROL_SCHEMA_VERSION: &str = "1.0.0";

const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_ITEMS: usize = 65_536;
const MAX_TEXT_BYTES: usize = 1024 * 1024;
const TOKEN_GRAPH_VERSION: &str = "pliegocss-token-graph/1";
const TOKEN_GRAPH_INTEGRITY_CHECK: &str = "token-graph-integrity";

/// Error returned when a control artifact violates its closed contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractError {
    reason: String,
}

impl ContractError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::error::Error for ContractError {}

/// Tool identity and interpretation-affecting contract versions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ToolIdentity {
    /// Stable executable or product name.
    pub name: String,
    /// Exact `PliegoCSS` version.
    pub version: String,
    /// Named versions of contracts that affect interpretation.
    pub contracts: BTreeMap<String, String>,
}

/// Integrity-bound reference to a logical artifact.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ArtifactReference {
    /// Portable path relative to the declared logical root.
    pub file: String,
    /// Exact serialized byte length.
    pub bytes: u64,
    /// SHA-256 of the exact serialized bytes.
    pub sha256: String,
}

impl ArtifactReference {
    /// Creates an integrity reference from exact artifact bytes.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when `file` is not a portable logical path.
    pub fn from_bytes(file: impl Into<String>, bytes: &[u8]) -> Result<Self, ContractError> {
        let value = Self {
            file: file.into(),
            bytes: u64::try_from(bytes.len())
                .map_err(|_| ContractError::new("artifact byte length exceeds u64"))?,
            sha256: content_hash(bytes),
        };
        value.validate("artifact")?;
        Ok(value)
    }

    fn validate(&self, field: &str) -> Result<(), ContractError> {
        validate_logical_path(&format!("{field}.file"), &self.file)?;
        validate_hash(&format!("{field}.sha256"), &self.sha256)
    }
}

/// One individually hashed source, policy, configuration, or adapter input.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputFile {
    /// Portable logical path.
    pub file: String,
    /// Stable role such as `source`, `config`, `policy`, or `adapter`.
    pub role: String,
    /// SHA-256 of the exact input bytes.
    pub sha256: String,
}

/// Declared adapter identity participating in semantic interpretation.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AdapterIdentity {
    /// Stable adapter name.
    pub name: String,
    /// Exact adapter version.
    pub version: String,
}

/// Merkle-like identity of all build inputs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ManifestInputs {
    /// Portable identity of the source root, never an absolute host path.
    pub source_root: String,
    /// Canonical hash of the ordered source-input set.
    pub source_hash: String,
    /// Canonical hash of configuration and policy inputs.
    pub config_hash: String,
    /// Individually hashed inputs in logical-path order.
    pub files: Vec<InputFile>,
    /// Optional typed source-manifest reference.
    pub source_manifest: Option<ArtifactReference>,
    /// Optional Project Index reference.
    pub project_index: Option<ArtifactReference>,
    /// Declared adapters in name/version order.
    pub adapters: Vec<AdapterIdentity>,
}

/// Downstream CSS backend identity and deterministic stage configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BackendIdentity {
    /// Backend name, distinct from the `PliegoCSS` tool identity.
    pub name: String,
    /// Exact backend version.
    pub version: String,
    /// Enabled backend stages in canonical order.
    pub stages: Vec<String>,
    /// Hash of deterministic backend configuration.
    pub config_hash: String,
}

/// Official compatibility-data snapshot identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CompatibilityData {
    /// Official dataset source identifier.
    pub source: String,
    /// Exact dataset version.
    pub version: String,
    /// Evidence date in `YYYY-MM-DD` form.
    pub date: String,
}

/// Explicit target and policy boundary used for the build.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TargetIdentity {
    /// Selected policy profile.
    pub profile: String,
    /// Explicit canonical browser vector; empty only for the `none` profile.
    pub browsers: Vec<String>,
    /// Official compatibility-data snapshot.
    pub compatibility: CompatibilityData,
    /// Selected reset policy.
    pub reset_policy: String,
    /// Selected style-scope policy.
    pub scope_policy: String,
    /// Version of the capability-decision contract.
    pub capability_decisions_version: String,
}

/// One emitted artifact with role, media type, and graph relationships.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OutputArtifact {
    /// Exact output reference.
    pub artifact: ArtifactReference,
    /// Stable output role.
    pub role: String,
    /// Canonical media type.
    pub media_type: String,
    /// Optional source-map reference.
    pub source_map: Option<ArtifactReference>,
    /// Specialized manifest, Asset Plan, or Project Index relationships.
    pub relationships: Vec<String>,
}

/// Canonical CSS measurements and attribution counts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RuleMeasurements {
    /// Whether the semantic measurement completed or was unavailable.
    pub observation: MeasurementState,
    /// Deterministic reason when measurement was unavailable.
    pub unavailable_reason: Option<String>,
    /// Total canonical CSS bytes.
    pub total_bytes: u64,
    /// Total rule count.
    pub rules: u64,
    /// Total selector count.
    pub selectors: u64,
    /// Total declaration count.
    pub declarations: u64,
    /// Maximum encoded specificity when measured.
    pub maximum_specificity: Option<String>,
    /// Semantic duplicate count.
    pub semantic_duplicates: u64,
    /// Counts keyed by canonical layer, including explicit `unknown` when needed.
    pub by_layer: BTreeMap<String, u64>,
    /// Counts keyed by package, including explicit `unknown` when needed.
    pub by_package: BTreeMap<String, u64>,
    /// Counts keyed by route, including explicit `unknown` when needed.
    pub by_route: BTreeMap<String, u64>,
    /// Counts keyed by component, including explicit `unknown` when needed.
    pub by_component: BTreeMap<String, u64>,
    /// Counts keyed by state, including explicit `unknown` when needed.
    pub by_state: BTreeMap<String, u64>,
    /// Counts keyed by rule type.
    pub by_rule_type: BTreeMap<String, u64>,
}

/// Token-graph identity, health, and coverage measurements.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TokenMeasurements {
    /// Whether a typed token graph was measured or unavailable.
    pub observation: MeasurementState,
    /// Deterministic reason when a token graph was unavailable.
    pub unavailable_reason: Option<String>,
    /// Internal token-graph contract version when measured.
    pub graph_version: Option<String>,
    /// Canonical token-graph hash when measured.
    pub graph_hash: Option<String>,
    /// Declared token count.
    pub tokens: u64,
    /// Alias count.
    pub aliases: u64,
    /// Derived-value count.
    pub derived_values: u64,
    /// Theme count.
    pub themes: u64,
    /// Percentage basis points covered by tokens when measured, from 0 through 10,000.
    pub coverage_basis_points: Option<u16>,
    /// Detected token-cycle count.
    pub cycles: u64,
    /// Declared contrast-pair count.
    pub contrast_pairs: u64,
    /// Deprecated token count.
    pub deprecations: u64,
    /// Optional DTCG adapter identity.
    pub dtcg_adapter: Option<AdapterIdentity>,
    /// Canonical counts keyed by DTCG token type.
    pub dtcg_inventory: BTreeMap<String, u64>,
}

/// Observation state for analyzer-fed manifest sections.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MeasurementState {
    /// The declared analyzer produced complete values for this section.
    Measured,
    /// The values could not be observed inside the declared boundary.
    Unavailable,
}

/// Action selected for one capability or policy decision.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DecisionAction {
    /// Preserve author intent without transformation.
    Preserved,
    /// Apply a deterministic transformation.
    Transformed,
    /// Emit a supported fallback.
    Fallback,
    /// Continue with an explicitly degraded result.
    Degraded,
    /// Allow an explicitly experimental behavior.
    Experimental,
    /// Block unsupported or policy-forbidden output.
    Blocked,
}

impl DecisionAction {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Preserved => "preserved",
            Self::Transformed => "transformed",
            Self::Fallback => "fallback",
            Self::Degraded => "degraded",
            Self::Experimental => "experimental",
            Self::Blocked => "blocked",
        }
    }
}

/// Deterministic backend or policy decision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Decision {
    /// Stable decision ID.
    pub id: String,
    /// Feature or policy being decided.
    pub feature: String,
    /// Selected action.
    pub action: DecisionAction,
    /// Human-readable deterministic reason.
    pub reason: String,
    /// Stable evidence identifiers.
    pub evidence: Vec<String>,
    /// Logical output paths affected by the decision.
    pub affected_outputs: Vec<String>,
    /// SHA-256 over the canonical decision content excluding this field.
    pub fingerprint: String,
}

impl Decision {
    /// Refreshes the fingerprint after canonical decision fields change.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when the decision cannot be serialized.
    pub fn refresh_fingerprint(&mut self) -> Result<(), ContractError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct FingerprintInput<'a> {
            id: &'a str,
            feature: &'a str,
            action: DecisionAction,
            reason: &'a str,
            evidence: &'a [String],
            affected_outputs: &'a [String],
        }

        let bytes = serde_json::to_vec(&FingerprintInput {
            id: &self.id,
            feature: &self.feature,
            action: self.action,
            reason: &self.reason,
            evidence: &self.evidence,
            affected_outputs: &self.affected_outputs,
        })
        .map_err(|error| ContractError::new(format!("cannot fingerprint decision: {error}")))?;
        self.fingerprint = content_hash(&bytes);
        Ok(())
    }
}

/// Binding to the canonical finding document and renderer parity evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ViolationSummary {
    /// Canonical finding schema version.
    pub finding_schema_version: String,
    /// Integrity-bound canonical finding document.
    pub document: ArtifactReference,
    /// Finding counts by stable code.
    pub counts_by_code: BTreeMap<String, u64>,
    /// Finding counts by severity.
    pub counts_by_severity: BTreeMap<String, u64>,
    /// Finding counts by verification state.
    pub counts_by_verification: BTreeMap<String, u64>,
    /// Canonically ordered active exception IDs.
    pub active_exception_ids: Vec<String>,
    /// Whether human, JSON, and SARIF parity was verified.
    pub parity_passed: bool,
}

/// Acyclic receipt expectation embedded in the control manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReceiptExpectation {
    /// Expected receipt schema version.
    pub schema_version: String,
    /// Fixed adjacent receipt path.
    pub file: String,
    /// Stable IDs of checks that must appear in the adjacent receipt.
    pub required_checks: Vec<String>,
    /// Result expected from the completed verification run.
    pub expected_result: ReceiptResult,
}

/// Unified control manifest for one `PliegoCSS` output group.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ControlManifest {
    /// Exact document discriminator.
    pub document_kind: String,
    /// Semantic wire version.
    pub schema_version: String,
    /// Tool and contract identities.
    pub tool: ToolIdentity,
    /// Merkle-like input identity.
    pub inputs: ManifestInputs,
    /// Downstream backend identity.
    pub backend: BackendIdentity,
    /// Explicit target boundary.
    pub targets: TargetIdentity,
    /// Canonical emitted outputs.
    pub outputs: Vec<OutputArtifact>,
    /// Canonical CSS measurements.
    pub rules: RuleMeasurements,
    /// Token-graph measurements.
    pub tokens: TokenMeasurements,
    /// Canonical capability and policy decisions.
    pub decisions: Vec<Decision>,
    /// Canonical finding-document binding.
    pub violations: ViolationSummary,
    /// Acyclic expectation for the adjacent receipt.
    pub receipt: ReceiptExpectation,
}

impl ControlManifest {
    /// Validates every schema-1 invariant without writing.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] for unsupported versions, unsafe paths, invalid hashes,
    /// noncanonical order, duplicate identities, inconsistent references, or defensive limits.
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_header(
            &self.document_kind,
            CONTROL_MANIFEST_KIND,
            &self.schema_version,
        )?;
        validate_tool(&self.tool)?;
        validate_manifest_inputs(&self.inputs)?;
        validate_backend(&self.backend)?;
        validate_targets(&self.targets)?;
        validate_outputs(&self.outputs)?;
        validate_rules(&self.rules)?;
        validate_tokens(&self.tokens)?;
        validate_token_graph_output(&self.tokens, &self.outputs, &self.receipt)?;
        validate_decisions(&self.decisions, &self.outputs)?;
        validate_violations(&self.violations)?;
        validate_receipt_expectation(&self.receipt)?;
        Ok(())
    }

    /// Serializes the validated manifest as deterministic pretty JSON ending in one newline.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when validation or serialization fails.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, ContractError> {
        self.validate()?;
        canonical_json(self)
    }
}

/// Minimal input identity repeated by the build receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReceiptInputs {
    /// Canonical source-input set hash.
    pub source_hash: String,
    /// Canonical configuration and policy hash.
    pub config_hash: String,
}

/// Canonical decision counts repeated by the build receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReceiptDecisionSummary {
    /// Total decision count.
    pub total: u64,
    /// Decision counts keyed by action.
    pub by_action: BTreeMap<String, u64>,
}

/// Canonical finding summary repeated by the build receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReceiptFindingSummary {
    /// Canonical finding-document reference.
    pub document: ArtifactReference,
    /// Total finding count.
    pub total: u64,
    /// Count of error findings without an active exception.
    pub unexcepted_errors: u64,
    /// Canonically ordered active exception IDs.
    pub active_exception_ids: Vec<String>,
}

/// Status of one receipt check.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    /// The check completed successfully with appropriate evidence.
    Passed,
    /// The check ran and failed.
    Failed,
    /// The check did not run.
    NotRun,
}

/// One integrity, policy, budget, or browser verification check.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReceiptCheck {
    /// Stable check ID.
    pub id: String,
    /// Derived execution status.
    pub status: CheckStatus,
    /// Evidence kind such as `static`, `browser`, or `integrity`.
    pub evidence_kind: String,
    /// Optional integrity-bound evidence artifact.
    pub evidence_artifact: Option<ArtifactReference>,
    /// Whether this check is required for a passed receipt.
    pub required: bool,
}

/// Derived result of the complete receipt verification boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReceiptResult {
    /// Every required condition passed.
    Passed,
    /// At least one required condition failed or was not run.
    Failed,
}

/// Integrity and evidence receipt adjacent to the control manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BuildReceipt {
    /// Exact document discriminator.
    pub document_kind: String,
    /// Semantic wire version.
    pub schema_version: String,
    /// Tool and contract identities.
    pub tool: ToolIdentity,
    /// Repeated source and configuration hashes.
    pub inputs: ReceiptInputs,
    /// Repeated backend identity.
    pub backend: BackendIdentity,
    /// Repeated target identity.
    pub targets: TargetIdentity,
    /// Hash and byte length of the exact complete control manifest.
    pub manifest: ArtifactReference,
    /// Integrity references to every emitted output.
    pub outputs: Vec<ArtifactReference>,
    /// Decision summary.
    pub decisions: ReceiptDecisionSummary,
    /// Finding and exception summary.
    pub findings: ReceiptFindingSummary,
    /// Canonical verification checks.
    pub checks: Vec<ReceiptCheck>,
    /// Derived overall result.
    pub result: ReceiptResult,
}

impl BuildReceipt {
    /// Validates the receipt against the exact manifest bytes it claims to bind.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] for schema violations, hash drift, missing checks, invalid browser
    /// evidence, inconsistent repeated data, or a result that is not correctly derived.
    pub fn validate(&self, manifest_bytes: &[u8]) -> Result<(), ContractError> {
        validate_header(
            &self.document_kind,
            BUILD_RECEIPT_KIND,
            &self.schema_version,
        )?;
        validate_tool(&self.tool)?;
        validate_hash("inputs.sourceHash", &self.inputs.source_hash)?;
        validate_hash("inputs.configHash", &self.inputs.config_hash)?;
        validate_backend(&self.backend)?;
        validate_targets(&self.targets)?;
        validate_manifest_reference(&self.manifest, manifest_bytes)?;
        let manifest = parse_control_manifest(manifest_bytes)?;
        validate_receipt_identity(self, &manifest)?;
        validate_receipt_outputs(&self.outputs, &manifest.outputs)?;
        validate_receipt_decisions(&self.decisions, &manifest.decisions)?;
        validate_receipt_findings(&self.findings, &manifest.violations)?;
        validate_checks(&self.checks)?;
        validate_token_graph_receipt(self, &manifest)?;
        validate_receipt_result(self, &manifest.receipt)
    }

    /// Serializes the validated receipt as deterministic pretty JSON ending in one newline.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when validation or serialization fails.
    pub fn to_canonical_json(&self, manifest_bytes: &[u8]) -> Result<Vec<u8>, ContractError> {
        self.validate(manifest_bytes)?;
        canonical_json(self)
    }
}

fn validate_manifest_reference(
    reference: &ArtifactReference,
    manifest_bytes: &[u8],
) -> Result<(), ContractError> {
    reference.validate("manifest")?;
    if reference.file != CONTROL_MANIFEST_FILE {
        return Err(ContractError::new(format!(
            "manifest.file must be `{CONTROL_MANIFEST_FILE}`"
        )));
    }
    let bytes = u64::try_from(manifest_bytes.len())
        .map_err(|_| ContractError::new("manifest byte length exceeds u64"))?;
    if reference.bytes != bytes || reference.sha256 != content_hash(manifest_bytes) {
        return Err(ContractError::new(
            "manifest reference does not match the supplied exact bytes",
        ));
    }
    Ok(())
}

fn validate_receipt_identity(
    receipt: &BuildReceipt,
    manifest: &ControlManifest,
) -> Result<(), ContractError> {
    if receipt.tool != manifest.tool
        || receipt.inputs.source_hash != manifest.inputs.source_hash
        || receipt.inputs.config_hash != manifest.inputs.config_hash
        || receipt.backend != manifest.backend
        || receipt.targets != manifest.targets
    {
        return Err(ContractError::new(
            "receipt repeats tool, input, backend, or target data inconsistent with the manifest",
        ));
    }
    Ok(())
}

fn validate_receipt_outputs(
    receipt: &[ArtifactReference],
    manifest: &[OutputArtifact],
) -> Result<(), ContractError> {
    validate_artifact_references("outputs", receipt)?;
    let expected: Vec<ArtifactReference> = manifest
        .iter()
        .map(|output| output.artifact.clone())
        .collect();
    if receipt != expected {
        return Err(ContractError::new(
            "receipt outputs do not exactly match manifest outputs",
        ));
    }
    Ok(())
}

fn validate_receipt_decisions(
    receipt: &ReceiptDecisionSummary,
    manifest: &[Decision],
) -> Result<(), ContractError> {
    validate_count_map("decisions.byAction", &receipt.by_action)?;
    let action_total = checked_count_total("decisions.byAction", &receipt.by_action)?;
    if action_total != receipt.total {
        return Err(ContractError::new(
            "decisions.total does not equal decisions.byAction",
        ));
    }
    let mut expected = BTreeMap::<String, u64>::new();
    for decision in manifest {
        let count = expected
            .entry(decision.action.as_str().to_owned())
            .or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| ContractError::new("decision count overflows u64"))?;
    }
    let manifest_total = u64::try_from(manifest.len())
        .map_err(|_| ContractError::new("decision count exceeds u64"))?;
    if receipt.total != manifest_total || receipt.by_action != expected {
        return Err(ContractError::new(
            "receipt decision summary does not match manifest decisions",
        ));
    }
    Ok(())
}

fn validate_receipt_findings(
    receipt: &ReceiptFindingSummary,
    manifest: &ViolationSummary,
) -> Result<(), ContractError> {
    receipt.document.validate("findings.document")?;
    validate_sorted_unique_ids("findings.activeExceptionIds", &receipt.active_exception_ids)?;
    let finding_total =
        checked_count_total("violations.countsBySeverity", &manifest.counts_by_severity)?;
    let error_total = manifest
        .counts_by_severity
        .get("error")
        .copied()
        .unwrap_or(0);
    if receipt.document != manifest.document
        || receipt.total != finding_total
        || receipt.unexcepted_errors > error_total
        || receipt.active_exception_ids != manifest.active_exception_ids
    {
        return Err(ContractError::new(
            "receipt finding summary does not match manifest violations",
        ));
    }
    Ok(())
}

fn validate_receipt_result(
    receipt: &BuildReceipt,
    expectation: &ReceiptExpectation,
) -> Result<(), ContractError> {
    let required_checks: Vec<String> = receipt
        .checks
        .iter()
        .filter(|check| check.required)
        .map(|check| check.id.clone())
        .collect();
    if required_checks != expectation.required_checks {
        return Err(ContractError::new(
            "receipt required checks do not match the manifest expectation",
        ));
    }
    let passed = receipt.findings.unexcepted_errors == 0
        && receipt
            .checks
            .iter()
            .filter(|check| check.required)
            .all(|check| check.status == CheckStatus::Passed);
    let derived = if passed {
        ReceiptResult::Passed
    } else {
        ReceiptResult::Failed
    };
    if receipt.result != derived {
        return Err(ContractError::new(
            "receipt result does not match required checks and unexcepted errors",
        ));
    }
    if receipt.result != expectation.expected_result {
        return Err(ContractError::new(
            "receipt result does not match the manifest expectation",
        ));
    }
    Ok(())
}

/// Computes the canonical lowercase prefixed SHA-256 form used by schema 1.
#[must_use]
pub fn content_hash(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// Parses and validates a closed control-manifest JSON document.
///
/// # Errors
///
/// Returns [`ContractError`] for oversized input, malformed JSON, unknown fields, unsupported
/// versions, noncanonical collections, unsafe paths, invalid hashes, or inconsistent references.
pub fn parse_control_manifest(bytes: &[u8]) -> Result<ControlManifest, ContractError> {
    validate_document_size(bytes)?;
    let manifest: ControlManifest = serde_json::from_slice(bytes)
        .map_err(|error| ContractError::new(format!("invalid control manifest JSON: {error}")))?;
    manifest.validate()?;
    Ok(manifest)
}

/// Parses and validates a closed build receipt against exact control-manifest bytes.
///
/// # Errors
///
/// Returns [`ContractError`] for oversized input, malformed JSON, unknown fields, unsupported
/// versions, noncanonical collections, invalid integrity edges, or an incorrectly derived result.
pub fn parse_build_receipt(
    bytes: &[u8],
    manifest_bytes: &[u8],
) -> Result<BuildReceipt, ContractError> {
    validate_document_size(bytes)?;
    validate_document_size(manifest_bytes)?;
    let receipt: BuildReceipt = serde_json::from_slice(bytes)
        .map_err(|error| ContractError::new(format!("invalid build receipt JSON: {error}")))?;
    receipt.validate(manifest_bytes)?;
    Ok(receipt)
}

fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, ContractError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        ContractError::new(format!("cannot serialize control artifact: {error}"))
    })?;
    bytes.push(b'\n');
    validate_document_size(&bytes)?;
    Ok(bytes)
}

fn validate_document_size(bytes: &[u8]) -> Result<(), ContractError> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(ContractError::new(format!(
            "control artifact exceeds {MAX_DOCUMENT_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_header(kind: &str, expected: &str, version: &str) -> Result<(), ContractError> {
    if kind != expected {
        return Err(ContractError::new(format!(
            "documentKind must be `{expected}`"
        )));
    }
    if version != CONTROL_SCHEMA_VERSION {
        return Err(ContractError::new(format!(
            "unsupported schemaVersion `{version}`; expected `{CONTROL_SCHEMA_VERSION}`"
        )));
    }
    Ok(())
}

fn validate_tool(tool: &ToolIdentity) -> Result<(), ContractError> {
    validate_id("tool.name", &tool.name)?;
    validate_text("tool.version", &tool.version)?;
    validate_version_map("tool.contracts", &tool.contracts)
}

fn validate_manifest_inputs(inputs: &ManifestInputs) -> Result<(), ContractError> {
    validate_root_identity("inputs.sourceRoot", &inputs.source_root)?;
    validate_hash("inputs.sourceHash", &inputs.source_hash)?;
    validate_hash("inputs.configHash", &inputs.config_hash)?;
    validate_len("inputs.files", inputs.files.len())?;
    ensure_sorted_unique_by("inputs.files", &inputs.files, |item| item.file.as_str())?;
    for input in &inputs.files {
        validate_logical_path("inputs.files.file", &input.file)?;
        validate_id("inputs.files.role", &input.role)?;
        validate_hash("inputs.files.sha256", &input.sha256)?;
    }
    if let Some(reference) = &inputs.source_manifest {
        reference.validate("inputs.sourceManifest")?;
    }
    if let Some(reference) = &inputs.project_index {
        reference.validate("inputs.projectIndex")?;
    }
    validate_len("inputs.adapters", inputs.adapters.len())?;
    ensure_sorted_unique_by("inputs.adapters", &inputs.adapters, |item| {
        item.name.as_str()
    })?;
    for adapter in &inputs.adapters {
        validate_id("inputs.adapters.name", &adapter.name)?;
        validate_text("inputs.adapters.version", &adapter.version)?;
    }
    Ok(())
}

fn validate_backend(backend: &BackendIdentity) -> Result<(), ContractError> {
    validate_id("backend.name", &backend.name)?;
    validate_text("backend.version", &backend.version)?;
    validate_sorted_unique_ids("backend.stages", &backend.stages)?;
    validate_hash("backend.configHash", &backend.config_hash)
}

fn validate_targets(targets: &TargetIdentity) -> Result<(), ContractError> {
    validate_id("targets.profile", &targets.profile)?;
    validate_sorted_unique_text("targets.browsers", &targets.browsers)?;
    if targets.browsers.is_empty() != (targets.profile == "none") {
        return Err(ContractError::new(
            "targets.browsers must be empty exactly when targets.profile is `none`",
        ));
    }
    validate_id(
        "targets.compatibility.source",
        &targets.compatibility.source,
    )?;
    validate_text(
        "targets.compatibility.version",
        &targets.compatibility.version,
    )?;
    validate_date("targets.compatibility.date", &targets.compatibility.date)?;
    validate_id("targets.resetPolicy", &targets.reset_policy)?;
    validate_id("targets.scopePolicy", &targets.scope_policy)?;
    validate_text(
        "targets.capabilityDecisionsVersion",
        &targets.capability_decisions_version,
    )
}

fn validate_outputs(outputs: &[OutputArtifact]) -> Result<(), ContractError> {
    validate_len("outputs", outputs.len())?;
    ensure_sorted_unique_by("outputs", outputs, |output| output.artifact.file.as_str())?;
    for output in outputs {
        output.artifact.validate("outputs.artifact")?;
        validate_id("outputs.role", &output.role)?;
        validate_media_type("outputs.mediaType", &output.media_type)?;
        if let Some(source_map) = &output.source_map {
            if output.media_type != "text/css" {
                return Err(ContractError::new(
                    "outputs.sourceMap can only be attached to a `text/css` output",
                ));
            }
            source_map.validate("outputs.sourceMap")?;
            if source_map.file == output.artifact.file {
                return Err(ContractError::new(
                    "outputs.sourceMap cannot reference its owning output",
                ));
            }
            let target = outputs
                .binary_search_by(|candidate| candidate.artifact.file.cmp(&source_map.file))
                .ok()
                .map(|index| &outputs[index])
                .ok_or_else(|| {
                    ContractError::new(format!(
                        "outputs.sourceMap `{}` does not resolve to an output artifact",
                        source_map.file
                    ))
                })?;
            if target.artifact != *source_map {
                return Err(ContractError::new(format!(
                    "outputs.sourceMap `{}` does not match the referenced output bytes or hash",
                    source_map.file
                )));
            }
            if target.role != "css-source-map" {
                return Err(ContractError::new(format!(
                    "outputs.sourceMap `{}` must reference a `css-source-map` output",
                    source_map.file
                )));
            }
            if target.media_type != "application/json" {
                return Err(ContractError::new(format!(
                    "outputs.sourceMap `{}` must reference an `application/json` output",
                    source_map.file
                )));
            }
            if !output.relationships.contains(&source_map.file) {
                return Err(ContractError::new(format!(
                    "outputs.sourceMap `{}` must be listed in its owning output relationships",
                    source_map.file
                )));
            }
            if !target.relationships.contains(&output.artifact.file) {
                return Err(ContractError::new(format!(
                    "outputs.sourceMap `{}` must relate back to its owning output",
                    source_map.file
                )));
            }
        }
        validate_sorted_unique_text("outputs.relationships", &output.relationships)?;
        for relationship in &output.relationships {
            validate_logical_path("outputs.relationships", relationship)?;
        }
    }
    for output in outputs {
        if output.role == "css-source-map"
            && !outputs
                .iter()
                .any(|owner| owner.source_map.as_ref() == Some(&output.artifact))
        {
            return Err(ContractError::new(format!(
                "css-source-map output `{}` is not referenced by any owning output",
                output.artifact.file
            )));
        }
    }
    Ok(())
}

fn validate_rules(rules: &RuleMeasurements) -> Result<(), ContractError> {
    match (rules.observation, &rules.unavailable_reason) {
        (MeasurementState::Measured, None) => {}
        (MeasurementState::Unavailable, Some(reason)) => {
            validate_text("rules.unavailableReason", reason)?;
        }
        (MeasurementState::Measured, Some(_)) => {
            return Err(ContractError::new(
                "measured rules cannot carry unavailableReason",
            ));
        }
        (MeasurementState::Unavailable, None) => {
            return Err(ContractError::new(
                "unavailable rules require unavailableReason",
            ));
        }
    }
    if let Some(specificity) = &rules.maximum_specificity {
        validate_text("rules.maximumSpecificity", specificity)?;
    }
    if rules.observation == MeasurementState::Unavailable {
        let counters_are_zero = rules.total_bytes == 0
            && rules.rules == 0
            && rules.selectors == 0
            && rules.declarations == 0
            && rules.semantic_duplicates == 0;
        let maps_are_empty = rules.by_layer.is_empty()
            && rules.by_package.is_empty()
            && rules.by_route.is_empty()
            && rules.by_component.is_empty()
            && rules.by_state.is_empty()
            && rules.by_rule_type.is_empty();
        if !counters_are_zero || !maps_are_empty || rules.maximum_specificity.is_some() {
            return Err(ContractError::new(
                "unavailable rules cannot claim measurements or attribution",
            ));
        }
        return Ok(());
    }
    if rules.maximum_specificity.is_none() {
        return Err(ContractError::new(
            "measured rules require maximumSpecificity",
        ));
    }
    for (field, counts) in [
        ("rules.byLayer", &rules.by_layer),
        ("rules.byPackage", &rules.by_package),
        ("rules.byRoute", &rules.by_route),
        ("rules.byComponent", &rules.by_component),
        ("rules.byState", &rules.by_state),
        ("rules.byRuleType", &rules.by_rule_type),
    ] {
        validate_count_map(field, counts)?;
        if checked_count_total(field, counts)? != rules.rules {
            return Err(ContractError::new(format!(
                "{field} total does not equal rules.rules"
            )));
        }
    }
    Ok(())
}

fn validate_tokens(tokens: &TokenMeasurements) -> Result<(), ContractError> {
    match (tokens.observation, &tokens.unavailable_reason) {
        (MeasurementState::Measured, None) => {}
        (MeasurementState::Unavailable, Some(reason)) => {
            validate_text("tokens.unavailableReason", reason)?;
        }
        (MeasurementState::Measured, Some(_)) => {
            return Err(ContractError::new(
                "measured tokens cannot carry unavailableReason",
            ));
        }
        (MeasurementState::Unavailable, None) => {
            return Err(ContractError::new(
                "unavailable tokens require unavailableReason",
            ));
        }
    }
    match (&tokens.graph_version, &tokens.graph_hash) {
        (Some(version), Some(hash)) => {
            validate_text("tokens.graphVersion", version)?;
            validate_hash("tokens.graphHash", hash)?;
        }
        (None, None) => {}
        _ => {
            return Err(ContractError::new(
                "tokens graphVersion and graphHash must be both present or both absent",
            ));
        }
    }
    if tokens
        .coverage_basis_points
        .is_some_and(|coverage| coverage > 10_000)
    {
        return Err(ContractError::new(
            "tokens.coverageBasisPoints must be between 0 and 10000",
        ));
    }
    if tokens.observation == MeasurementState::Unavailable {
        let counters_are_zero = tokens.tokens == 0
            && tokens.aliases == 0
            && tokens.derived_values == 0
            && tokens.themes == 0
            && tokens.cycles == 0
            && tokens.contrast_pairs == 0
            && tokens.deprecations == 0;
        if !counters_are_zero
            || tokens.graph_version.is_some()
            || tokens.graph_hash.is_some()
            || tokens.coverage_basis_points.is_some()
            || tokens.dtcg_adapter.is_some()
            || !tokens.dtcg_inventory.is_empty()
        {
            return Err(ContractError::new(
                "unavailable tokens cannot claim graph identity, measurements, or adapters",
            ));
        }
        return Ok(());
    }
    if tokens.graph_version.is_none()
        || tokens.graph_hash.is_none()
        || tokens.coverage_basis_points.is_none()
    {
        return Err(ContractError::new(
            "measured tokens require graphVersion, graphHash, and coverageBasisPoints",
        ));
    }
    if let Some(adapter) = &tokens.dtcg_adapter {
        validate_id("tokens.dtcgAdapter.name", &adapter.name)?;
        validate_text("tokens.dtcgAdapter.version", &adapter.version)?;
    }
    validate_count_map("tokens.dtcgInventory", &tokens.dtcg_inventory)
}

fn validate_token_graph_output(
    tokens: &TokenMeasurements,
    outputs: &[OutputArtifact],
    receipt: &ReceiptExpectation,
) -> Result<(), ContractError> {
    let candidates = outputs
        .iter()
        .filter(|output| output.role == "token-graph" || output.artifact.file == TOKEN_GRAPH_FILE)
        .collect::<Vec<_>>();
    if tokens.graph_version.as_deref() != Some(TOKEN_GRAPH_VERSION) {
        if receipt
            .required_checks
            .iter()
            .any(|check| check == TOKEN_GRAPH_INTEGRITY_CHECK)
        {
            return Err(ContractError::new(format!(
                "receipt.requiredChecks cannot contain `{TOKEN_GRAPH_INTEGRITY_CHECK}` without tokens.graphVersion `{TOKEN_GRAPH_VERSION}`"
            )));
        }
        if candidates.is_empty() {
            return Ok(());
        }
        return Err(ContractError::new(format!(
            "`{TOKEN_GRAPH_FILE}` token-graph output requires tokens.graphVersion `{TOKEN_GRAPH_VERSION}`"
        )));
    }

    if candidates.len() != 1 {
        return Err(ContractError::new(format!(
            "tokens.graphVersion `{TOKEN_GRAPH_VERSION}` requires exactly one `{TOKEN_GRAPH_FILE}` token-graph output"
        )));
    }
    if !receipt
        .required_checks
        .iter()
        .any(|check| check == TOKEN_GRAPH_INTEGRITY_CHECK)
    {
        return Err(ContractError::new(format!(
            "tokens.graphVersion `{TOKEN_GRAPH_VERSION}` requires receipt.requiredChecks `{TOKEN_GRAPH_INTEGRITY_CHECK}`"
        )));
    }
    let output = candidates[0];
    if output.artifact.file != TOKEN_GRAPH_FILE {
        return Err(ContractError::new(format!(
            "token-graph output file must be `{TOKEN_GRAPH_FILE}`"
        )));
    }
    if output.role != "token-graph" {
        return Err(ContractError::new(
            "pliego.tokens.json output role must be `token-graph`",
        ));
    }
    if output.source_map.is_some() {
        return Err(ContractError::new(
            "token-graph output cannot carry outputs.sourceMap",
        ));
    }
    if output.media_type != "application/json" {
        return Err(ContractError::new(
            "token-graph output mediaType must be `application/json`",
        ));
    }
    if Some(&output.artifact.sha256) != tokens.graph_hash.as_ref() {
        return Err(ContractError::new(
            "token-graph output sha256 must equal tokens.graphHash",
        ));
    }

    for related in outputs
        .iter()
        .filter(|candidate| matches!(candidate.role.as_str(), "generated-css" | "style-manifest"))
    {
        if !output.relationships.contains(&related.artifact.file) {
            return Err(ContractError::new(format!(
                "token-graph output must relate to {} output `{}`",
                related.role, related.artifact.file
            )));
        }
        if !related
            .relationships
            .iter()
            .any(|relationship| relationship == TOKEN_GRAPH_FILE)
        {
            return Err(ContractError::new(format!(
                "{} output `{}` must relate back to `{TOKEN_GRAPH_FILE}`",
                related.role, related.artifact.file
            )));
        }
    }
    Ok(())
}

fn validate_token_graph_receipt(
    receipt: &BuildReceipt,
    manifest: &ControlManifest,
) -> Result<(), ContractError> {
    let graph_output = manifest
        .outputs
        .iter()
        .find(|output| output.artifact.file == TOKEN_GRAPH_FILE);
    let graph_check = receipt
        .checks
        .iter()
        .find(|check| check.id == TOKEN_GRAPH_INTEGRITY_CHECK);

    if manifest.tokens.graph_version.as_deref() != Some(TOKEN_GRAPH_VERSION) {
        if graph_check.is_some() {
            return Err(ContractError::new(format!(
                "receipt check `{TOKEN_GRAPH_INTEGRITY_CHECK}` is forbidden without tokens.graphVersion `{TOKEN_GRAPH_VERSION}`"
            )));
        }
        return Ok(());
    }

    let output = graph_output.ok_or_else(|| {
        ContractError::new(format!(
            "tokens.graphVersion `{TOKEN_GRAPH_VERSION}` has no `{TOKEN_GRAPH_FILE}` output"
        ))
    })?;
    let check = graph_check.ok_or_else(|| {
        ContractError::new(format!(
            "receipt is missing required check `{TOKEN_GRAPH_INTEGRITY_CHECK}`"
        ))
    })?;
    if !check.required {
        return Err(ContractError::new(format!(
            "receipt check `{TOKEN_GRAPH_INTEGRITY_CHECK}` must be required"
        )));
    }
    if check.status != CheckStatus::Passed {
        return Err(ContractError::new(format!(
            "receipt check `{TOKEN_GRAPH_INTEGRITY_CHECK}` must pass for a published canonical token graph"
        )));
    }
    if check.evidence_kind != "integrity" {
        return Err(ContractError::new(format!(
            "receipt check `{TOKEN_GRAPH_INTEGRITY_CHECK}` evidenceKind must be `integrity`"
        )));
    }
    if check.evidence_artifact.as_ref() != Some(&output.artifact) {
        return Err(ContractError::new(format!(
            "receipt check `{TOKEN_GRAPH_INTEGRITY_CHECK}` must evidence the exact `{TOKEN_GRAPH_FILE}` output"
        )));
    }
    Ok(())
}

fn validate_decisions(
    decisions: &[Decision],
    outputs: &[OutputArtifact],
) -> Result<(), ContractError> {
    validate_len("decisions", decisions.len())?;
    ensure_sorted_unique_by("decisions", decisions, |decision| decision.id.as_str())?;
    let output_files: BTreeSet<&str> = outputs
        .iter()
        .map(|output| output.artifact.file.as_str())
        .collect();
    for decision in decisions {
        validate_id("decisions.id", &decision.id)?;
        validate_id("decisions.feature", &decision.feature)?;
        validate_text("decisions.reason", &decision.reason)?;
        validate_sorted_unique_text("decisions.evidence", &decision.evidence)?;
        validate_sorted_unique_text("decisions.affectedOutputs", &decision.affected_outputs)?;
        for output in &decision.affected_outputs {
            validate_logical_path("decisions.affectedOutputs", output)?;
            if !output_files.contains(output.as_str()) {
                return Err(ContractError::new(format!(
                    "decision `{}` references unknown output `{output}`",
                    decision.id
                )));
            }
        }
        validate_hash("decisions.fingerprint", &decision.fingerprint)?;
        let mut expected = decision.clone();
        expected.refresh_fingerprint()?;
        if expected.fingerprint != decision.fingerprint {
            return Err(ContractError::new(format!(
                "decision `{}` fingerprint does not match canonical content",
                decision.id
            )));
        }
    }
    Ok(())
}

fn validate_violations(violations: &ViolationSummary) -> Result<(), ContractError> {
    validate_text(
        "violations.findingSchemaVersion",
        &violations.finding_schema_version,
    )?;
    violations.document.validate("violations.document")?;
    validate_count_map("violations.countsByCode", &violations.counts_by_code)?;
    validate_count_map(
        "violations.countsBySeverity",
        &violations.counts_by_severity,
    )?;
    validate_count_map(
        "violations.countsByVerification",
        &violations.counts_by_verification,
    )?;
    let by_code = checked_count_total("violations.countsByCode", &violations.counts_by_code)?;
    let by_severity = checked_count_total(
        "violations.countsBySeverity",
        &violations.counts_by_severity,
    )?;
    let by_verification = checked_count_total(
        "violations.countsByVerification",
        &violations.counts_by_verification,
    )?;
    if by_code != by_severity || by_code != by_verification {
        return Err(ContractError::new(
            "violation totals differ across code, severity, and verification",
        ));
    }
    validate_sorted_unique_ids(
        "violations.activeExceptionIds",
        &violations.active_exception_ids,
    )
}

fn validate_receipt_expectation(receipt: &ReceiptExpectation) -> Result<(), ContractError> {
    if receipt.schema_version != CONTROL_SCHEMA_VERSION {
        return Err(ContractError::new(format!(
            "receipt.schemaVersion must be `{CONTROL_SCHEMA_VERSION}`"
        )));
    }
    if receipt.file != BUILD_RECEIPT_FILE {
        return Err(ContractError::new(format!(
            "receipt.file must be `{BUILD_RECEIPT_FILE}`"
        )));
    }
    validate_sorted_unique_ids("receipt.requiredChecks", &receipt.required_checks)?;
    if receipt.required_checks.is_empty() {
        return Err(ContractError::new(
            "receipt.requiredChecks must contain at least one check",
        ));
    }
    Ok(())
}

fn validate_artifact_references(
    field: &str,
    references: &[ArtifactReference],
) -> Result<(), ContractError> {
    validate_len(field, references.len())?;
    ensure_sorted_unique_by(field, references, |reference| reference.file.as_str())?;
    for reference in references {
        reference.validate(field)?;
    }
    Ok(())
}

fn validate_checks(checks: &[ReceiptCheck]) -> Result<(), ContractError> {
    validate_len("checks", checks.len())?;
    ensure_sorted_unique_by("checks", checks, |check| check.id.as_str())?;
    for check in checks {
        validate_id("checks.id", &check.id)?;
        validate_id("checks.evidenceKind", &check.evidence_kind)?;
        if let Some(artifact) = &check.evidence_artifact {
            artifact.validate("checks.evidenceArtifact")?;
        }
        if check.evidence_kind == "browser"
            && check.status == CheckStatus::Passed
            && check.evidence_artifact.is_none()
        {
            return Err(ContractError::new(format!(
                "browser check `{}` cannot pass without an evidence artifact",
                check.id
            )));
        }
    }
    Ok(())
}

fn validate_count_map(field: &str, counts: &BTreeMap<String, u64>) -> Result<(), ContractError> {
    validate_len(field, counts.len())?;
    for key in counts.keys() {
        validate_id(field, key)?;
    }
    Ok(())
}

fn validate_version_map(
    field: &str,
    versions: &BTreeMap<String, String>,
) -> Result<(), ContractError> {
    validate_len(field, versions.len())?;
    for (key, value) in versions {
        validate_id(field, key)?;
        validate_text(field, value)?;
    }
    Ok(())
}

fn checked_count_total(field: &str, counts: &BTreeMap<String, u64>) -> Result<u64, ContractError> {
    counts.values().try_fold(0_u64, |total, count| {
        total
            .checked_add(*count)
            .ok_or_else(|| ContractError::new(format!("{field} count total overflows u64")))
    })
}

fn validate_hash(field: &str, value: &str) -> Result<(), ContractError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(ContractError::new(format!(
            "{field} must use sha256:<64 lowercase hex>"
        )));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ContractError::new(format!(
            "{field} must use sha256:<64 lowercase hex>"
        )));
    }
    Ok(())
}

fn validate_logical_path(field: &str, value: &str) -> Result<(), ContractError> {
    validate_text(field, value)?;
    if value.starts_with('/')
        || value.starts_with('\\')
        || value.contains('\\')
        || value.contains(':')
        || value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(ContractError::new(format!(
            "{field} must be a portable relative logical path"
        )));
    }
    Ok(())
}

fn validate_root_identity(field: &str, value: &str) -> Result<(), ContractError> {
    validate_logical_path(field, value)
}

fn validate_id(field: &str, value: &str) -> Result<(), ContractError> {
    validate_text(field, value)?;
    if !value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/' | b'@')
    }) {
        return Err(ContractError::new(format!(
            "{field} contains characters outside the canonical identifier set"
        )));
    }
    Ok(())
}

fn validate_media_type(field: &str, value: &str) -> Result<(), ContractError> {
    validate_text(field, value)?;
    let Some((kind, subtype)) = value.split_once('/') else {
        return Err(ContractError::new(format!(
            "{field} must be a canonical media type"
        )));
    };
    if kind.is_empty()
        || subtype.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'/' | b'-' | b'+' | b'.')
        })
    {
        return Err(ContractError::new(format!(
            "{field} must be a canonical media type"
        )));
    }
    Ok(())
}

fn validate_date(field: &str, value: &str) -> Result<(), ContractError> {
    if value.len() != 10
        || value.as_bytes()[4] != b'-'
        || value.as_bytes()[7] != b'-'
        || !value
            .bytes()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return Err(ContractError::new(format!("{field} must use YYYY-MM-DD")));
    }
    let year = value[0..4].parse::<u16>().unwrap_or(0);
    let month = value[5..7].parse::<u8>().unwrap_or(0);
    let day = value[8..10].parse::<u8>().unwrap_or(0);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let maximum_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    if year == 0 || day == 0 || day > maximum_day {
        return Err(ContractError::new(format!(
            "{field} is not a valid canonical date"
        )));
    }
    Ok(())
}

fn validate_text(field: &str, value: &str) -> Result<(), ContractError> {
    if value.is_empty() || value.len() > MAX_TEXT_BYTES {
        return Err(ContractError::new(format!(
            "{field} must contain 1..={MAX_TEXT_BYTES} UTF-8 bytes"
        )));
    }
    if value.chars().any(|character| {
        character.is_control() || character == '\u{2028}' || character == '\u{2029}'
    }) {
        return Err(ContractError::new(format!(
            "{field} contains forbidden control characters"
        )));
    }
    Ok(())
}

fn validate_len(field: &str, len: usize) -> Result<(), ContractError> {
    if len > MAX_ITEMS {
        return Err(ContractError::new(format!(
            "{field} exceeds the {MAX_ITEMS}-item defensive limit"
        )));
    }
    Ok(())
}

fn validate_sorted_unique_ids(field: &str, values: &[String]) -> Result<(), ContractError> {
    validate_sorted_unique_text(field, values)?;
    for value in values {
        validate_id(field, value)?;
    }
    Ok(())
}

fn validate_sorted_unique_text(field: &str, values: &[String]) -> Result<(), ContractError> {
    validate_len(field, values.len())?;
    for value in values {
        validate_text(field, value)?;
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ContractError::new(format!(
            "{field} must be in canonical sorted unique order"
        )));
    }
    Ok(())
}

fn ensure_sorted_unique_by<T, F>(field: &str, values: &[T], key: F) -> Result<(), ContractError>
where
    F: Fn(&T) -> &str,
{
    if values.windows(2).any(|pair| key(&pair[0]) >= key(&pair[1])) {
        return Err(ContractError::new(format!(
            "{field} must be in canonical sorted unique order"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> String {
        content_hash(label.as_bytes())
    }

    fn map(entries: &[(&str, u64)]) -> BTreeMap<String, u64> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_owned(), *value))
            .collect()
    }

    fn tool() -> ToolIdentity {
        ToolIdentity {
            name: "pliegocss".to_owned(),
            version: "0.0.0".to_owned(),
            contracts: BTreeMap::from([
                ("finding".to_owned(), "1.0.0".to_owned()),
                ("style-manifest".to_owned(), "5".to_owned()),
            ]),
        }
    }

    fn backend() -> BackendIdentity {
        BackendIdentity {
            name: "lightningcss".to_owned(),
            version: "1.0.0-alpha.71".to_owned(),
            stages: vec!["minify".to_owned(), "prefix".to_owned()],
            config_hash: hash("backend"),
        }
    }

    fn targets() -> TargetIdentity {
        TargetIdentity {
            profile: "baseline-widely".to_owned(),
            browsers: vec!["chrome>=111".to_owned(), "firefox>=111".to_owned()],
            compatibility: CompatibilityData {
                source: "web-features".to_owned(),
                version: "2.70.0".to_owned(),
                date: "2026-07-10".to_owned(),
            },
            reset_policy: "opt-in".to_owned(),
            scope_policy: "component".to_owned(),
            capability_decisions_version: "1.0.0".to_owned(),
        }
    }

    fn manifest() -> ControlManifest {
        let css = ArtifactReference::from_bytes("assets/app.css", b"a{}\n").unwrap();
        let findings = ArtifactReference::from_bytes("reports/findings.json", b"{}\n").unwrap();
        let mut decision = Decision {
            id: "compat.nesting".to_owned(),
            feature: "css-nesting".to_owned(),
            action: DecisionAction::Transformed,
            reason: "target vector requires lowering".to_owned(),
            evidence: vec!["web-features@2.70.0".to_owned()],
            affected_outputs: vec!["assets/app.css".to_owned()],
            fingerprint: String::new(),
        };
        decision.refresh_fingerprint().unwrap();
        ControlManifest {
            document_kind: CONTROL_MANIFEST_KIND.to_owned(),
            schema_version: CONTROL_SCHEMA_VERSION.to_owned(),
            tool: tool(),
            inputs: ManifestInputs {
                source_root: "workspace".to_owned(),
                source_hash: hash("sources"),
                config_hash: hash("config"),
                files: vec![InputFile {
                    file: "src/app.rs".to_owned(),
                    role: "source".to_owned(),
                    sha256: hash("source"),
                }],
                source_manifest: None,
                project_index: None,
                adapters: vec![AdapterIdentity {
                    name: "dtcg".to_owned(),
                    version: "2025.10".to_owned(),
                }],
            },
            backend: backend(),
            targets: targets(),
            outputs: vec![OutputArtifact {
                artifact: css,
                role: "stylesheet".to_owned(),
                media_type: "text/css".to_owned(),
                source_map: None,
                relationships: vec!["assets/app.css.manifest.json".to_owned()],
            }],
            rules: RuleMeasurements {
                observation: MeasurementState::Measured,
                unavailable_reason: None,
                total_bytes: 4,
                rules: 1,
                selectors: 1,
                declarations: 0,
                maximum_specificity: Some("0,0,1".to_owned()),
                semantic_duplicates: 0,
                by_layer: map(&[("utilities", 1)]),
                by_package: map(&[("app", 1)]),
                by_route: map(&[("root", 1)]),
                by_component: map(&[("app", 1)]),
                by_state: map(&[("base", 1)]),
                by_rule_type: map(&[("style", 1)]),
            },
            tokens: TokenMeasurements {
                observation: MeasurementState::Measured,
                unavailable_reason: None,
                graph_version: Some("1".to_owned()),
                graph_hash: Some(hash("tokens")),
                tokens: 2,
                aliases: 1,
                derived_values: 0,
                themes: 1,
                coverage_basis_points: Some(10_000),
                cycles: 0,
                contrast_pairs: 1,
                deprecations: 0,
                dtcg_adapter: Some(AdapterIdentity {
                    name: "dtcg".to_owned(),
                    version: "2025.10".to_owned(),
                }),
                dtcg_inventory: map(&[("color", 2)]),
            },
            decisions: vec![decision],
            violations: ViolationSummary {
                finding_schema_version: "1.0.0".to_owned(),
                document: findings,
                counts_by_code: map(&[("PCSS-COMPAT-001", 1)]),
                counts_by_severity: map(&[("warning", 1)]),
                counts_by_verification: map(&[("verified", 1)]),
                active_exception_ids: Vec::new(),
                parity_passed: true,
            },
            receipt: ReceiptExpectation {
                schema_version: CONTROL_SCHEMA_VERSION.to_owned(),
                file: BUILD_RECEIPT_FILE.to_owned(),
                required_checks: vec!["integrity".to_owned()],
                expected_result: ReceiptResult::Passed,
            },
        }
    }

    fn token_graph_manifest() -> ControlManifest {
        let mut value = manifest();
        let token_graph = ArtifactReference::from_bytes(TOKEN_GRAPH_FILE, b"{}\n").unwrap();
        value.tokens.graph_version = Some(TOKEN_GRAPH_VERSION.to_owned());
        value.tokens.graph_hash = Some(token_graph.sha256.clone());

        let css = &mut value.outputs[0];
        css.role = "generated-css".to_owned();
        css.relationships.push(TOKEN_GRAPH_FILE.to_owned());
        css.relationships.sort();

        value.outputs.push(OutputArtifact {
            artifact: ArtifactReference::from_bytes("assets/app.css.manifest.json", b"{}\n")
                .unwrap(),
            role: "style-manifest".to_owned(),
            media_type: "application/json".to_owned(),
            source_map: None,
            relationships: vec!["assets/app.css".to_owned(), TOKEN_GRAPH_FILE.to_owned()],
        });
        value.outputs.push(OutputArtifact {
            artifact: token_graph,
            role: "token-graph".to_owned(),
            media_type: "application/json".to_owned(),
            source_map: None,
            relationships: vec![
                "assets/app.css".to_owned(),
                "assets/app.css.manifest.json".to_owned(),
            ],
        });
        value
            .outputs
            .sort_by(|left, right| left.artifact.file.cmp(&right.artifact.file));
        value
            .receipt
            .required_checks
            .push(TOKEN_GRAPH_INTEGRITY_CHECK.to_owned());
        value.receipt.required_checks.sort();
        value
    }

    fn token_graph_receipt(manifest: &ControlManifest, manifest_bytes: &[u8]) -> BuildReceipt {
        let mut value = receipt(manifest_bytes);
        value.outputs = manifest
            .outputs
            .iter()
            .map(|output| output.artifact.clone())
            .collect();
        let token_graph = value
            .outputs
            .iter()
            .find(|output| output.file == TOKEN_GRAPH_FILE)
            .expect("token graph output")
            .clone();
        value.checks.push(ReceiptCheck {
            id: TOKEN_GRAPH_INTEGRITY_CHECK.to_owned(),
            status: CheckStatus::Passed,
            evidence_kind: "integrity".to_owned(),
            evidence_artifact: Some(token_graph),
            required: true,
        });
        value.checks.sort_by(|left, right| left.id.cmp(&right.id));
        value
    }

    fn receipt(manifest_bytes: &[u8]) -> BuildReceipt {
        BuildReceipt {
            document_kind: BUILD_RECEIPT_KIND.to_owned(),
            schema_version: CONTROL_SCHEMA_VERSION.to_owned(),
            tool: tool(),
            inputs: ReceiptInputs {
                source_hash: hash("sources"),
                config_hash: hash("config"),
            },
            backend: backend(),
            targets: targets(),
            manifest: ArtifactReference::from_bytes(CONTROL_MANIFEST_FILE, manifest_bytes).unwrap(),
            outputs: vec![ArtifactReference::from_bytes("assets/app.css", b"a{}\n").unwrap()],
            decisions: ReceiptDecisionSummary {
                total: 1,
                by_action: map(&[("transformed", 1)]),
            },
            findings: ReceiptFindingSummary {
                document: ArtifactReference::from_bytes("reports/findings.json", b"{}\n").unwrap(),
                total: 1,
                unexcepted_errors: 0,
                active_exception_ids: Vec::new(),
            },
            checks: vec![ReceiptCheck {
                id: "integrity".to_owned(),
                status: CheckStatus::Passed,
                evidence_kind: "integrity".to_owned(),
                evidence_artifact: None,
                required: true,
            }],
            result: ReceiptResult::Passed,
        }
    }

    #[test]
    fn manifest_and_receipt_have_an_acyclic_integrity_edge() {
        let manifest = manifest();
        let manifest_bytes = manifest.to_canonical_json().unwrap();
        let receipt = receipt(&manifest_bytes);
        let receipt_bytes = receipt.to_canonical_json(&manifest_bytes).unwrap();

        assert_eq!(parse_control_manifest(&manifest_bytes).unwrap(), manifest);
        assert_eq!(
            parse_build_receipt(&receipt_bytes, &manifest_bytes).unwrap(),
            receipt
        );
        let manifest_json = String::from_utf8(manifest_bytes).unwrap();
        assert!(!manifest_json.contains("receiptHash"));
        assert!(
            String::from_utf8(receipt_bytes)
                .unwrap()
                .contains(&content_hash(manifest_json.as_bytes()))
        );
        assert_eq!(
            content_hash(&receipt.to_canonical_json(manifest_json.as_bytes()).unwrap()),
            "sha256:08c9cc69b3eefd4c04f7f4362b3c4f385f3ac57ed59d2b2fc660fdc65def7eeb"
        );
    }

    #[test]
    fn canonical_bytes_are_stable_and_end_in_one_newline() {
        let first = manifest().to_canonical_json().unwrap();
        let reparsed = parse_control_manifest(&first).unwrap();
        let second = reparsed.to_canonical_json().unwrap();
        assert_eq!(first, second);
        assert!(first.ends_with(b"\n"));
        assert!(!first.ends_with(b"\n\n"));
        assert_eq!(content_hash(&first), content_hash(&second));
        assert_eq!(
            content_hash(&first),
            "sha256:ccb4d20e8c443e51c9264f9927427d6177efa487b0f35b3e82a59a47c8382ee1"
        );
    }

    #[test]
    fn decoder_fails_closed_on_unknown_fields_order_and_tampering() {
        let bytes = manifest().to_canonical_json().unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["unexpected"] = serde_json::json!(true);
        assert!(parse_control_manifest(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut unsorted = manifest();
        unsorted.inputs.files.push(InputFile {
            file: "a.rs".to_owned(),
            role: "source".to_owned(),
            sha256: hash("a"),
        });
        assert!(unsorted.validate().is_err());

        let mut tampered = manifest();
        tampered.decisions[0].reason = "changed without fingerprint refresh".to_owned();
        assert!(tampered.validate().is_err());

        let mut impossible_date = manifest();
        impossible_date.targets.compatibility.date = "2026-02-30".to_owned();
        assert!(impossible_date.validate().is_err());

        let mut no_targets = manifest();
        no_targets.targets.profile = "none".to_owned();
        no_targets.targets.browsers.clear();
        assert!(no_targets.validate().is_ok());
        no_targets.targets.profile = "modern".to_owned();
        assert!(no_targets.validate().is_err());

        let mut unavailable = manifest();
        unavailable.tokens = TokenMeasurements {
            observation: MeasurementState::Unavailable,
            unavailable_reason: Some("no typed token source was supplied".to_owned()),
            graph_version: None,
            graph_hash: None,
            tokens: 0,
            aliases: 0,
            derived_values: 0,
            themes: 0,
            coverage_basis_points: None,
            cycles: 0,
            contrast_pairs: 0,
            deprecations: 0,
            dtcg_adapter: None,
            dtcg_inventory: BTreeMap::new(),
        };
        assert!(unavailable.validate().is_ok());
        unavailable.tokens.tokens = 1;
        assert!(unavailable.validate().is_err());
    }

    #[test]
    fn source_map_references_resolve_exactly_to_dedicated_outputs() {
        let mut value = manifest();
        let source_map = ArtifactReference::from_bytes(
            "assets/app.css.map",
            br#"{"version":3,"sources":["src/app.rs"],"names":[],"mappings":"AAAA"}
"#,
        )
        .unwrap();
        value.outputs[0].source_map = Some(source_map.clone());
        value.outputs[0]
            .relationships
            .push("assets/app.css.map".to_owned());
        value.outputs[0].relationships.sort();
        value.outputs.push(OutputArtifact {
            artifact: source_map.clone(),
            role: "css-source-map".to_owned(),
            media_type: "application/json".to_owned(),
            source_map: None,
            relationships: vec!["assets/app.css".to_owned()],
        });
        value
            .outputs
            .sort_by(|left, right| left.artifact.file.cmp(&right.artifact.file));
        assert!(value.validate().is_ok());

        let mut missing = value.clone();
        missing
            .outputs
            .retain(|output| output.role != "css-source-map");
        assert!(missing.validate().is_err());

        let mut drifted = value.clone();
        let css = drifted
            .outputs
            .iter_mut()
            .find(|output| output.role == "stylesheet")
            .unwrap();
        css.source_map.as_mut().unwrap().bytes += 1;
        assert!(drifted.validate().is_err());

        let mut wrong_role = value;
        wrong_role
            .outputs
            .iter_mut()
            .find(|output| output.role == "css-source-map")
            .unwrap()
            .role = "metadata".to_owned();
        assert!(wrong_role.validate().is_err());
    }

    #[test]
    fn canonical_token_graph_requires_one_integrity_bound_output() {
        let value = token_graph_manifest();
        assert!(value.validate().is_ok());

        let mut missing_expectation = token_graph_manifest();
        missing_expectation
            .receipt
            .required_checks
            .retain(|check| check != TOKEN_GRAPH_INTEGRITY_CHECK);
        assert!(missing_expectation.validate().is_err());

        let mut legacy = manifest();
        legacy.tokens.graph_version = Some("pliegocss-flat-token-graph/1".to_owned());
        assert!(legacy.validate().is_ok());

        let mut legacy_with_graph_check = legacy.clone();
        legacy_with_graph_check
            .receipt
            .required_checks
            .push(TOKEN_GRAPH_INTEGRITY_CHECK.to_owned());
        legacy_with_graph_check.receipt.required_checks.sort();
        assert!(legacy_with_graph_check.validate().is_err());

        let mut mislabeled = token_graph_manifest();
        mislabeled.tokens.graph_version = Some("pliegocss-flat-token-graph/1".to_owned());
        assert!(mislabeled.validate().is_err());

        let mut unavailable = manifest();
        unavailable.tokens = TokenMeasurements {
            observation: MeasurementState::Unavailable,
            unavailable_reason: Some("no typed token source was supplied".to_owned()),
            graph_version: None,
            graph_hash: None,
            tokens: 0,
            aliases: 0,
            derived_values: 0,
            themes: 0,
            coverage_basis_points: None,
            cycles: 0,
            contrast_pairs: 0,
            deprecations: 0,
            dtcg_adapter: None,
            dtcg_inventory: BTreeMap::new(),
        };
        assert!(unavailable.validate().is_ok());
    }

    #[test]
    fn canonical_token_graph_rejects_missing_duplicate_and_hash_drift() {
        let mut missing = token_graph_manifest();
        missing
            .outputs
            .retain(|output| output.role != "token-graph");
        assert!(missing.validate().is_err());

        let mut duplicate = token_graph_manifest();
        duplicate.outputs.push(OutputArtifact {
            artifact: ArtifactReference::from_bytes("extra.tokens.json", b"{}\n").unwrap(),
            role: "token-graph".to_owned(),
            media_type: "application/json".to_owned(),
            source_map: None,
            relationships: vec![
                "assets/app.css".to_owned(),
                "assets/app.css.manifest.json".to_owned(),
            ],
        });
        duplicate
            .outputs
            .sort_by(|left, right| left.artifact.file.cmp(&right.artifact.file));
        assert!(duplicate.validate().is_err());

        let mut drifted = token_graph_manifest();
        drifted
            .outputs
            .iter_mut()
            .find(|output| output.role == "token-graph")
            .unwrap()
            .artifact
            .sha256 = hash("drifted token graph");
        assert!(drifted.validate().is_err());
    }

    #[test]
    fn canonical_token_graph_rejects_wrong_role_media_type_and_source_map() {
        let mut wrong_role = token_graph_manifest();
        wrong_role
            .outputs
            .iter_mut()
            .find(|output| output.artifact.file == TOKEN_GRAPH_FILE)
            .unwrap()
            .role = "metadata".to_owned();
        assert!(wrong_role.validate().is_err());

        let mut wrong_media = token_graph_manifest();
        wrong_media
            .outputs
            .iter_mut()
            .find(|output| output.role == "token-graph")
            .unwrap()
            .media_type = "text/plain".to_owned();
        assert!(wrong_media.validate().is_err());

        let mut mapped = token_graph_manifest();
        let source_map = ArtifactReference::from_bytes("pliego.tokens.json.map", b"{}\n").unwrap();
        let graph = mapped
            .outputs
            .iter_mut()
            .find(|output| output.role == "token-graph")
            .unwrap();
        graph.media_type = "text/css".to_owned();
        graph.source_map = Some(source_map.clone());
        graph.relationships.push(source_map.file.clone());
        graph.relationships.sort();
        mapped.outputs.push(OutputArtifact {
            artifact: source_map,
            role: "css-source-map".to_owned(),
            media_type: "application/json".to_owned(),
            source_map: None,
            relationships: vec![TOKEN_GRAPH_FILE.to_owned()],
        });
        mapped
            .outputs
            .sort_by(|left, right| left.artifact.file.cmp(&right.artifact.file));
        let error = mapped.validate().unwrap_err();
        assert!(error.to_string().contains("cannot carry outputs.sourceMap"));
    }

    #[test]
    fn canonical_token_graph_requires_complete_reciprocal_relationships() {
        let mut missing_forward = token_graph_manifest();
        missing_forward
            .outputs
            .iter_mut()
            .find(|output| output.role == "token-graph")
            .unwrap()
            .relationships
            .retain(|relationship| relationship != "assets/app.css");
        assert!(missing_forward.validate().is_err());

        let mut missing_backlink = token_graph_manifest();
        missing_backlink
            .outputs
            .iter_mut()
            .find(|output| output.role == "style-manifest")
            .unwrap()
            .relationships
            .retain(|relationship| relationship != TOKEN_GRAPH_FILE);
        assert!(missing_backlink.validate().is_err());
    }

    #[test]
    fn receipt_requires_exact_canonical_token_graph_integrity_evidence() {
        let graph_manifest = token_graph_manifest();
        let manifest_bytes = graph_manifest.to_canonical_json().unwrap();
        let valid = token_graph_receipt(&graph_manifest, &manifest_bytes);
        assert!(valid.validate(&manifest_bytes).is_ok());

        let mut missing = valid.clone();
        missing
            .checks
            .retain(|check| check.id != TOKEN_GRAPH_INTEGRITY_CHECK);
        assert!(missing.validate(&manifest_bytes).is_err());

        let mut unrelated = valid.clone();
        let css_evidence = unrelated
            .outputs
            .iter()
            .find(|output| output.file == "assets/app.css")
            .unwrap()
            .clone();
        unrelated
            .checks
            .iter_mut()
            .find(|check| check.id == TOKEN_GRAPH_INTEGRITY_CHECK)
            .unwrap()
            .evidence_artifact = Some(css_evidence);
        assert!(unrelated.validate(&manifest_bytes).is_err());

        let mut failed = valid.clone();
        failed
            .checks
            .iter_mut()
            .find(|check| check.id == TOKEN_GRAPH_INTEGRITY_CHECK)
            .unwrap()
            .status = CheckStatus::Failed;
        assert!(failed.validate(&manifest_bytes).is_err());

        let legacy_manifest_bytes = manifest().to_canonical_json().unwrap();
        let mut legacy = receipt(&legacy_manifest_bytes);
        legacy.checks.push(ReceiptCheck {
            id: TOKEN_GRAPH_INTEGRITY_CHECK.to_owned(),
            status: CheckStatus::Passed,
            evidence_kind: "integrity".to_owned(),
            evidence_artifact: Some(legacy.outputs[0].clone()),
            required: false,
        });
        legacy.checks.sort_by(|left, right| left.id.cmp(&right.id));
        assert!(legacy.validate(&legacy_manifest_bytes).is_err());
    }

    #[test]
    fn receipt_rejects_hash_drift_failed_derivation_and_unevidenced_browser_pass() {
        let manifest_bytes = manifest().to_canonical_json().unwrap();
        let valid = receipt(&manifest_bytes);
        assert!(valid.validate(&manifest_bytes).is_ok());
        assert!(valid.validate(b"different manifest\n").is_err());

        let mut inconsistent = valid.clone();
        inconsistent.backend.version = "different".to_owned();
        assert!(inconsistent.validate(&manifest_bytes).is_err());

        let mut missing_output = valid.clone();
        missing_output.outputs.clear();
        assert!(missing_output.validate(&manifest_bytes).is_err());

        let mut wrong_summary = valid.clone();
        wrong_summary.decisions.by_action = map(&[("preserved", 1)]);
        assert!(wrong_summary.validate(&manifest_bytes).is_err());

        let mut wrong_result = valid.clone();
        wrong_result.checks[0].status = CheckStatus::Failed;
        assert!(wrong_result.validate(&manifest_bytes).is_err());

        let mut browser = valid;
        browser.checks[0].evidence_kind = "browser".to_owned();
        assert!(browser.validate(&manifest_bytes).is_err());
    }
}
