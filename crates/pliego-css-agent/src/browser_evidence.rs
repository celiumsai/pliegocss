use serde::{Deserialize, Serialize};

use pliego_css_build::artifacts::sha256_hex;

use super::validation::{validate_dotted_id, validate_sha256, validate_text, validate_version};
use super::{
    MAX_DOCUMENT_BYTES, REPAIR_BROWSER_EVIDENCE_SCHEMA_VERSION, REPAIR_BROWSER_PROFILE_FILES,
    RepairArtifactIdentity, RepairContractError,
};

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
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) node_version: String,
}

impl RepairBrowserRunnerIdentity {
    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
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
    pub(crate) family: String,
    pub(crate) executable: String,
    pub(crate) product: String,
    pub(crate) revision: String,
    pub(crate) protocol_version: String,
    pub(crate) js_version: String,
    pub(crate) user_agent: String,
}

impl RepairBrowserIdentity {
    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
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
    pub(crate) document: bool,
    pub(crate) island: bool,
    pub(crate) button: bool,
    pub(crate) value: bool,
}

/// Closed observations produced by the `PliegoRS` visit-counter browser profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairBrowserObservation {
    pub(crate) node_identity: RepairBrowserNodeIdentity,
    pub(crate) island_count: u16,
    pub(crate) island_id: String,
    pub(crate) initial_minutes: u16,
    pub(crate) final_minutes: u16,
    pub(crate) increment: u16,
    pub(crate) initial_text: String,
    pub(crate) final_text: String,
    pub(crate) initial_class: String,
    pub(crate) final_class: String,
    pub(crate) event_count: u16,
    pub(crate) event_key: String,
    pub(crate) event_value: u16,
    pub(crate) module_script_count: u16,
    pub(crate) preload_entry_count: u16,
    pub(crate) preload_request_count: u16,
    pub(crate) client_request_count: u16,
    pub(crate) stylesheet_present: bool,
    pub(crate) wasm_ready: bool,
    pub(crate) relevant_event_count: u16,
    pub(crate) server_error_count: u16,
}

impl RepairBrowserObservation {
    pub(crate) fn contract_passed(&self) -> bool {
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

    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
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
    pub(crate) schema_version: String,
    pub(crate) evidence_sha256: String,
    pub(crate) change_receipt_sha256: String,
    pub(crate) check_id: String,
    pub(crate) profile: RepairBrowserProfile,
    pub(crate) runner: RepairBrowserRunnerIdentity,
    pub(crate) profile_inputs: Vec<RepairArtifactIdentity>,
    pub(crate) browser: RepairBrowserIdentity,
    pub(crate) observation: RepairBrowserObservation,
    pub(crate) result: RepairBrowserEvidenceResult,
    pub(crate) exit_code: u8,
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

    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
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

pub(crate) fn validate_browser_profile_inputs(
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
    pub(crate) schema_version: &'a str,
    pub(crate) change_receipt_sha256: &'a str,
    pub(crate) check_id: &'a str,
    pub(crate) profile: RepairBrowserProfile,
    pub(crate) runner: &'a RepairBrowserRunnerIdentity,
    pub(crate) profile_inputs: &'a [RepairArtifactIdentity],
    pub(crate) browser: &'a RepairBrowserIdentity,
    pub(crate) observation: &'a RepairBrowserObservation,
    pub(crate) result: RepairBrowserEvidenceResult,
    pub(crate) exit_code: u8,
}

pub(crate) fn repair_browser_evidence_payload_sha256(
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
