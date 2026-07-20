use serde::{Deserialize, Serialize};

use super::validation::{validate_logical_path, validate_sha256};
use super::{MAX_DOCUMENT_BYTES, RepairContractError};

/// Identity of an exact input artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairArtifactIdentity {
    pub(crate) file: String,
    pub(crate) bytes: usize,
    pub(crate) sha256: String,
}

impl RepairArtifactIdentity {
    pub(crate) fn validate(&self, role: &str) -> Result<(), RepairContractError> {
        validate_logical_path(&format!("{role}.file"), &self.file)?;
        if self.bytes > MAX_DOCUMENT_BYTES {
            return Err(RepairContractError::new(format!(
                "{role}.bytes exceeds 16 MiB"
            )));
        }
        validate_sha256(&format!("{role}.sha256"), &self.sha256)
    }
}
