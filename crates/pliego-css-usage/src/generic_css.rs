//! Conservative observation contract for generic-CSS declaration identities.

use std::collections::{BTreeMap, BTreeSet};

use pliego_css_build::artifacts::sha256_hex;
use serde::{Deserialize, Serialize};

use crate::MAX_USAGE_DOCUMENT_BYTES;

/// Generic-CSS usage evidence schema version.
pub const GENERIC_CSS_USAGE_SCHEMA_VERSION: u8 = 1;
/// Conventional generic-CSS usage artifact filename.
pub const GENERIC_CSS_USAGE_FILE: &str = "pliego.css.generic-usage.json";

/// Conservative state for one authored generic-CSS declaration identity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GenericCssUsageStatus {
    /// Positive evidence observed this exact declaration identity.
    Observed,
    /// No positive evidence exists; absence never proves deadness.
    Unknown,
}

/// One declaration identity and its conservative observation state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GenericCssUsageEntry {
    identity: String,
    authored_occurrences: u64,
    status: GenericCssUsageStatus,
}

impl GenericCssUsageEntry {
    /// Returns the canonical generic declaration identity.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }
    /// Returns the conservative observation state.
    #[must_use]
    pub const fn status(&self) -> GenericCssUsageStatus {
        self.status
    }
}

/// Closed generic-CSS observation report.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GenericCssUsageReport {
    schema_version: u8,
    identity_format: String,
    inventory_sha256: String,
    observation_sha256: String,
    observation_scope: String,
    observed: u64,
    unknown: u64,
    entries: Vec<GenericCssUsageEntry>,
}

impl GenericCssUsageReport {
    /// Returns entries in canonical identity order.
    #[must_use]
    pub fn entries(&self) -> &[GenericCssUsageEntry] {
        &self.entries
    }
}

/// Builds a canonical report from the complete authored inventory and positive observations.
///
/// # Errors
/// Returns an error for malformed/duplicate identities, zero occurrences, observations outside the
/// inventory, invalid scope text, or an oversized report.
pub fn build_generic_css_usage_report(
    inventory: impl IntoIterator<Item = (String, usize)>,
    observed: impl IntoIterator<Item = String>,
    observation_scope: &str,
) -> Result<Vec<u8>, String> {
    validate_scope(observation_scope)?;
    let mut universe = BTreeMap::new();
    for (identity, occurrences) in inventory {
        validate_identity(&identity)?;
        if occurrences == 0 || universe.insert(identity.clone(), occurrences).is_some() {
            return Err("invalid generic CSS usage inventory".into());
        }
    }
    if universe.is_empty() {
        return Err("generic CSS usage inventory cannot be empty".into());
    }
    let mut positives = BTreeSet::new();
    for identity in observed {
        validate_identity(&identity)?;
        if !universe.contains_key(&identity) || !positives.insert(identity) {
            return Err("invalid generic CSS positive observation".into());
        }
    }
    let inventory_bytes = serde_json::to_vec(&universe).map_err(|error| error.to_string())?;
    let observation_bytes = serde_json::to_vec(&positives).map_err(|error| error.to_string())?;
    let entries = universe
        .into_iter()
        .map(|(identity, occurrences)| GenericCssUsageEntry {
            status: if positives.contains(&identity) {
                GenericCssUsageStatus::Observed
            } else {
                GenericCssUsageStatus::Unknown
            },
            identity,
            authored_occurrences: u64::try_from(occurrences).unwrap_or(u64::MAX),
        })
        .collect::<Vec<_>>();
    let observed_count = u64::try_from(positives.len()).unwrap_or(u64::MAX);
    let report = GenericCssUsageReport {
        schema_version: GENERIC_CSS_USAGE_SCHEMA_VERSION,
        identity_format: "pliegocss-generic-css-declaration-v1".into(),
        inventory_sha256: format!("sha256:{}", sha256_hex(&inventory_bytes)),
        observation_sha256: format!("sha256:{}", sha256_hex(&observation_bytes)),
        observation_scope: observation_scope.into(),
        observed: observed_count,
        unknown: u64::try_from(entries.len())
            .unwrap_or(u64::MAX)
            .saturating_sub(observed_count),
        entries,
    };
    let mut bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    if bytes.len() > MAX_USAGE_DOCUMENT_BYTES {
        return Err("generic CSS usage report exceeds 16 MiB".into());
    }
    Ok(bytes)
}

/// Parses and validates canonical generic-CSS usage evidence.
///
/// # Errors
/// Returns an error for malformed JSON or inconsistent counts/order/identities.
pub fn parse_generic_css_usage_report(bytes: &[u8]) -> Result<GenericCssUsageReport, String> {
    if bytes.len() > MAX_USAGE_DOCUMENT_BYTES {
        return Err("generic CSS usage report exceeds 16 MiB".into());
    }
    let report: GenericCssUsageReport =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if report.schema_version != 1
        || report.identity_format != "pliegocss-generic-css-declaration-v1"
    {
        return Err("unsupported generic CSS usage schema".into());
    }
    validate_scope(&report.observation_scope)?;
    let mut previous = None;
    let mut observed = 0_u64;
    for entry in &report.entries {
        validate_identity(&entry.identity)?;
        if entry.authored_occurrences == 0
            || previous.is_some_and(|value: &str| value >= entry.identity.as_str())
        {
            return Err("invalid generic CSS usage entries".into());
        }
        previous = Some(entry.identity.as_str());
        observed += u64::from(matches!(entry.status, GenericCssUsageStatus::Observed));
    }
    if observed != report.observed
        || report.unknown
            != u64::try_from(report.entries.len())
                .unwrap_or(u64::MAX)
                .saturating_sub(observed)
    {
        return Err("generic CSS usage summary mismatch".into());
    }
    Ok(report)
}

fn validate_identity(identity: &str) -> Result<(), String> {
    if identity.len() == 71
        && identity.starts_with("sha256:")
        && identity[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err("invalid generic CSS declaration identity".into())
    }
}
fn validate_scope(scope: &str) -> Result<(), String> {
    if !scope.is_empty() && scope.len() <= 4096 && !scope.chars().any(char::is_control) {
        Ok(())
    } else {
        Err("invalid generic CSS observation scope".into())
    }
}
