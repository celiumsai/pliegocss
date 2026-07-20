use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::validation::{
    validate_fingerprint, validate_logical_path, validate_sha256, validate_slug,
};
use super::{
    MAX_DOCUMENT_BYTES, MAX_EDITS, MAX_FILES, MAX_REPLACEMENT_BYTES, MAX_TOTAL_CHANGE_BYTES,
    REPAIR_PROPOSAL_SCHEMA_VERSION, RepairContractError, validate_checks, validate_edit_order,
};

/// Hard upper bounds authorized for one proposed change.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[allow(clippy::struct_field_names)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairChangeBudget {
    pub(crate) max_files: u32,
    pub(crate) max_edits: u32,
    pub(crate) max_inserted_bytes: u64,
    pub(crate) max_removed_bytes: u64,
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

    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
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
    pub(crate) finding_fingerprint: String,
    pub(crate) suggestion_id: String,
    pub(crate) file: String,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) replacement: String,
}

impl RepairProposalEdit {
    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
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
    pub(crate) schema_version: String,
    pub(crate) finding_document_sha256: String,
    pub(crate) change_budget: RepairChangeBudget,
    pub(crate) required_checks: Vec<String>,
    pub(crate) edits: Vec<RepairProposalEdit>,
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

    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
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
