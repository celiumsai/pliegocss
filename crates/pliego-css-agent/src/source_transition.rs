use serde::{Deserialize, Serialize};

use super::validation::{validate_logical_path, validate_sha256};
use super::{MAX_SOURCE_BYTES, RepairContractError};

/// Before/after digest pair for one source file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairSourceTransition {
    pub(crate) file: String,
    pub(crate) before_bytes: usize,
    pub(crate) before_sha256: String,
    pub(crate) after_bytes: usize,
    pub(crate) after_sha256: String,
}

impl RepairSourceTransition {
    /// Returns the portable logical source path.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
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
