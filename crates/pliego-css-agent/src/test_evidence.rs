use serde::{Deserialize, Serialize};

use pliego_css_build::artifacts::sha256_hex;

use super::validation::{validate_dotted_id, validate_sha256, validate_text, validate_version};
use super::{
    MAX_DOCUMENT_BYTES, REPAIR_TEST_EVIDENCE_SCHEMA_VERSION, RepairArtifactIdentity,
    RepairContractError,
};

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
    pub(crate) name: String,
    pub(crate) version: String,
}

impl RepairTestRunnerIdentity {
    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
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
    pub(crate) cargo_version: String,
    pub(crate) rustc_version: String,
    pub(crate) host: String,
}

impl RepairTestToolchainIdentity {
    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
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
    pub(crate) schema_version: String,
    pub(crate) evidence_sha256: String,
    pub(crate) change_receipt_sha256: String,
    pub(crate) check_id: String,
    pub(crate) profile: RepairTestProfile,
    pub(crate) runner: RepairTestRunnerIdentity,
    pub(crate) toolchain: RepairTestToolchainIdentity,
    pub(crate) workspace_manifest: RepairArtifactIdentity,
    pub(crate) lockfile: RepairArtifactIdentity,
    pub(crate) result: RepairTestEvidenceResult,
    pub(crate) exit_code: u8,
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

    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
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
    pub(crate) schema_version: &'a str,
    pub(crate) change_receipt_sha256: &'a str,
    pub(crate) check_id: &'a str,
    pub(crate) profile: RepairTestProfile,
    pub(crate) runner: &'a RepairTestRunnerIdentity,
    pub(crate) toolchain: &'a RepairTestToolchainIdentity,
    pub(crate) workspace_manifest: &'a RepairArtifactIdentity,
    pub(crate) lockfile: &'a RepairArtifactIdentity,
    pub(crate) result: RepairTestEvidenceResult,
    pub(crate) exit_code: u8,
}

pub(crate) fn repair_test_evidence_payload_sha256(
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
