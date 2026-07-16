//! Closed, deterministic accessibility policy schema and parser.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

const MAX_POLICY_BYTES: usize = 1024 * 1024;
const MAX_CONTRAST_PAIRS: usize = 256;
const MAX_EXCEPTIONS: usize = 4096;

/// Error returned when an accessibility policy violates its closed contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessibilityPolicyError {
    reason: String,
}

impl AccessibilityPolicyError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl fmt::Display for AccessibilityPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::error::Error for AccessibilityPolicyError {}

/// Accessibility checks that have an explicit schema-1 enforcement decision.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AccessibilityCheck {
    Contrast,
    Motion,
    FocusVisibility,
    ForcedColors,
    InputModality,
}

impl AccessibilityCheck {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Contrast => "contrast",
            Self::Motion => "motion",
            Self::FocusVisibility => "focusVisibility",
            Self::ForcedColors => "forcedColors",
            Self::InputModality => "inputModality",
        }
    }
}

/// Whether one policy outcome fails the audit or is emitted as a warning.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AccessibilityEnforcement {
    Fail,
    Warn,
}

/// Enforcement for the three non-pass result classes of one check.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AccessibilityCheckPolicy {
    violation: AccessibilityEnforcement,
    unverified: AccessibilityEnforcement,
    manual_required: AccessibilityEnforcement,
}

impl AccessibilityCheckPolicy {
    pub(crate) const fn violation(self) -> AccessibilityEnforcement {
        self.violation
    }

    pub(crate) const fn unverified(self) -> AccessibilityEnforcement {
        self.unverified
    }

    pub(crate) const fn manual_required(self) -> AccessibilityEnforcement {
        self.manual_required
    }
}

/// Mandatory enforcement vector for every schema-1 accessibility check.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AccessibilityChecks {
    contrast: AccessibilityCheckPolicy,
    motion: AccessibilityCheckPolicy,
    focus_visibility: AccessibilityCheckPolicy,
    forced_colors: AccessibilityCheckPolicy,
    input_modality: AccessibilityCheckPolicy,
}

impl AccessibilityChecks {
    const fn get(self, check: AccessibilityCheck) -> AccessibilityCheckPolicy {
        match check {
            AccessibilityCheck::Contrast => self.contrast,
            AccessibilityCheck::Motion => self.motion,
            AccessibilityCheck::FocusVisibility => self.focus_visibility,
            AccessibilityCheck::ForcedColors => self.forced_colors,
            AccessibilityCheck::InputModality => self.input_modality,
        }
    }
}

/// One explicit color token reference or literal CSS color.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
pub(crate) enum AccessibilityColorEndpoint {
    Token { name: String },
    Literal { value: String },
}

impl AccessibilityColorEndpoint {
    pub(crate) fn display_value(&self) -> &str {
        match self {
            Self::Token { name } => name,
            Self::Literal { value } => value,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct AccessibilitySelections(BTreeMap<String, String>);

impl<'de> Deserialize<'de> for AccessibilitySelections {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(AccessibilitySelectionsVisitor)
    }
}

struct AccessibilitySelectionsVisitor;

impl<'de> Visitor<'de> for AccessibilitySelectionsVisitor {
    type Value = AccessibilitySelections;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("unique accessibility selection keys")
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut selections = BTreeMap::new();
        while let Some((name, value)) = object.next_entry::<String, String>()? {
            if selections.contains_key(&name) {
                return Err(de::Error::custom(format!(
                    "duplicate JSON object key `{name}`"
                )));
            }
            selections.insert(name, value);
        }
        Ok(AccessibilitySelections(selections))
    }
}

/// One explicit foreground/background relationship evaluated for contrast.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AccessibilityContrastPair {
    id: String,
    foreground: AccessibilityColorEndpoint,
    background: AccessibilityColorEndpoint,
    minimum_ratio_milli: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selections: Option<AccessibilitySelections>,
}

impl AccessibilityContrastPair {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) const fn foreground(&self) -> &AccessibilityColorEndpoint {
        &self.foreground
    }

    pub(crate) const fn background(&self) -> &AccessibilityColorEndpoint {
        &self.background
    }

    pub(crate) const fn minimum_ratio_milli(&self) -> u16 {
        self.minimum_ratio_milli
    }

    pub(crate) const fn selections(&self) -> Option<&BTreeMap<String, String>> {
        match &self.selections {
            Some(selections) => Some(&selections.0),
            None => None,
        }
    }
}

/// One reviewed exception bound to a stable observation identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AccessibilityException {
    id: String,
    check: AccessibilityCheck,
    subject_id: String,
    justification: String,
    expires_on: String,
}

impl AccessibilityException {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) const fn check(&self) -> AccessibilityCheck {
        self.check
    }

    pub(crate) fn subject_id(&self) -> &str {
        &self.subject_id
    }

    pub(crate) fn justification(&self) -> &str {
        &self.justification
    }

    pub(crate) fn expires_on(&self) -> &str {
        &self.expires_on
    }
}

/// Closed and canonical accessibility policy schema 1.
///
/// Values are constructed by [`parse_accessibility_policy`]. Deliberately omitting a public
/// [`Deserialize`] implementation keeps duplicate-aware parsing and canonicalization mandatory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AccessibilityPolicy {
    schema_version: u8,
    policy_version: u8,
    checks: AccessibilityChecks,
    #[serde(default)]
    contrast_pairs: Vec<AccessibilityContrastPair>,
    #[serde(default)]
    exceptions: Vec<AccessibilityException>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AccessibilityPolicyWire {
    schema_version: u8,
    policy_version: u8,
    checks: AccessibilityChecks,
    #[serde(default)]
    contrast_pairs: Vec<AccessibilityContrastPair>,
    #[serde(default)]
    exceptions: Vec<AccessibilityException>,
}

impl AccessibilityPolicy {
    /// Serialize the validated semantic policy in canonical order with one trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`AccessibilityPolicyError`] when validation or serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, AccessibilityPolicyError> {
        self.validate()?;
        let mut output = serde_json::to_string_pretty(self).map_err(|error| {
            AccessibilityPolicyError::new(format!("cannot serialize accessibility policy: {error}"))
        })?;
        output.push('\n');
        Ok(output)
    }

    /// Return the interpretation-affecting policy version.
    #[must_use]
    pub const fn policy_version(&self) -> u8 {
        self.policy_version
    }

    /// Return how many explicit contrast relationships are declared.
    #[must_use]
    pub fn contrast_pair_count(&self) -> u64 {
        u64::try_from(self.contrast_pairs.len()).unwrap_or(u64::MAX)
    }

    pub(crate) const fn check_policy(&self, check: AccessibilityCheck) -> AccessibilityCheckPolicy {
        self.checks.get(check)
    }

    pub(crate) fn contrast_pairs(&self) -> &[AccessibilityContrastPair] {
        &self.contrast_pairs
    }

    pub(crate) fn exceptions(&self) -> &[AccessibilityException] {
        &self.exceptions
    }

    fn canonicalize(&mut self) {
        self.contrast_pairs
            .sort_by(|left, right| left.id.cmp(&right.id));
        self.exceptions
            .sort_by(|left, right| left.id.cmp(&right.id));
    }

    fn validate(&self) -> Result<(), AccessibilityPolicyError> {
        if self.schema_version != 1 {
            return Err(AccessibilityPolicyError::new(format!(
                "unsupported accessibility schema `{}`; expected `1`",
                self.schema_version
            )));
        }
        if self.policy_version != 1 {
            return Err(AccessibilityPolicyError::new(format!(
                "unsupported accessibility policy version `{}`; expected `1`",
                self.policy_version
            )));
        }
        if self.contrast_pairs.len() > MAX_CONTRAST_PAIRS {
            return Err(AccessibilityPolicyError::new(
                "accessibility policy cannot contain more than 256 contrast pairs",
            ));
        }
        if self.exceptions.len() > MAX_EXCEPTIONS {
            return Err(AccessibilityPolicyError::new(
                "accessibility policy cannot contain more than 4,096 exceptions",
            ));
        }
        validate_contrast_pairs(&self.contrast_pairs)?;
        validate_exceptions(&self.exceptions)
    }
}

/// Parse, validate, and canonicalize accessibility policy JSON.
///
/// # Errors
///
/// Returns [`AccessibilityPolicyError`] for oversized or malformed JSON, unknown fields,
/// unsupported versions, invalid contrast relationships, or overlapping exceptions.
pub fn parse_accessibility_policy(
    source: &[u8],
) -> Result<AccessibilityPolicy, AccessibilityPolicyError> {
    if source.len() > MAX_POLICY_BYTES {
        return Err(AccessibilityPolicyError::new(
            "accessibility policy exceeds 1 MiB",
        ));
    }
    let wire: AccessibilityPolicyWire = serde_json::from_slice(source).map_err(|error| {
        AccessibilityPolicyError::new(format!("invalid accessibility policy JSON: {error}"))
    })?;
    let mut policy = AccessibilityPolicy {
        schema_version: wire.schema_version,
        policy_version: wire.policy_version,
        checks: wire.checks,
        contrast_pairs: wire.contrast_pairs,
        exceptions: wire.exceptions,
    };
    policy.canonicalize();
    policy.validate()?;
    Ok(policy)
}

fn validate_contrast_pairs(
    pairs: &[AccessibilityContrastPair],
) -> Result<(), AccessibilityPolicyError> {
    let mut ids = BTreeSet::new();
    for pair in pairs {
        validate_slug("contrast pair id", &pair.id)?;
        if !ids.insert(pair.id.as_str()) {
            return Err(AccessibilityPolicyError::new(format!(
                "duplicate contrast pair id `{}`",
                pair.id
            )));
        }
        validate_endpoint(&pair.id, "foreground", &pair.foreground)?;
        validate_endpoint(&pair.id, "background", &pair.background)?;
        if !(1000..=21000).contains(&pair.minimum_ratio_milli) {
            return Err(AccessibilityPolicyError::new(format!(
                "contrast pair `{}` minimumRatioMilli must be between 1,000 and 21,000",
                pair.id
            )));
        }
        if let Some(AccessibilitySelections(selections)) = &pair.selections {
            if selections.is_empty() {
                return Err(AccessibilityPolicyError::new(format!(
                    "contrast pair `{}` selections must not be empty when present",
                    pair.id
                )));
            }
            if selections.len() > 64 {
                return Err(AccessibilityPolicyError::new(format!(
                    "contrast pair `{}` cannot select more than 64 resolver modifiers",
                    pair.id
                )));
            }
            for (name, value) in selections {
                validate_selection_text(&pair.id, "name", name)?;
                validate_selection_text(&pair.id, "value", value)?;
            }
        }
    }
    Ok(())
}

fn validate_endpoint(
    pair_id: &str,
    role: &str,
    endpoint: &AccessibilityColorEndpoint,
) -> Result<(), AccessibilityPolicyError> {
    match endpoint {
        AccessibilityColorEndpoint::Token { name } => {
            let Some(name) = name.strip_prefix("color.") else {
                return Err(AccessibilityPolicyError::new(format!(
                    "contrast pair `{pair_id}` {role} token must start with `color.`"
                )));
            };
            if !is_kebab_ascii(name, 256) {
                return Err(AccessibilityPolicyError::new(format!(
                    "contrast pair `{pair_id}` {role} token must use canonical `color.<kebab-name>` syntax"
                )));
            }
        }
        AccessibilityColorEndpoint::Literal { value } => {
            validate_text(&format!("contrast pair `{pair_id}` {role} literal"), value)?;
            if value.trim() != value {
                return Err(AccessibilityPolicyError::new(format!(
                    "contrast pair `{pair_id}` {role} literal must not have surrounding whitespace"
                )));
            }
        }
    }
    Ok(())
}

fn validate_exceptions(
    exceptions: &[AccessibilityException],
) -> Result<(), AccessibilityPolicyError> {
    let mut ids = BTreeSet::new();
    let mut coverage = BTreeSet::new();
    for exception in exceptions {
        validate_slug("accessibility exception id", &exception.id)?;
        validate_sha256_subject(&exception.subject_id)?;
        validate_text(
            "accessibility exception justification",
            &exception.justification,
        )?;
        validate_iso_date(&exception.expires_on)?;
        if !ids.insert(exception.id.as_str()) {
            return Err(AccessibilityPolicyError::new(format!(
                "duplicate accessibility exception id `{}`",
                exception.id
            )));
        }
        if !coverage.insert((exception.check, exception.subject_id.as_str())) {
            return Err(AccessibilityPolicyError::new(format!(
                "accessibility check `{}` subject `{}` has more than one exception",
                exception.check.as_str(),
                exception.subject_id
            )));
        }
    }
    Ok(())
}

fn validate_slug(field: &str, value: &str) -> Result<(), AccessibilityPolicyError> {
    if !is_slug(value) || value.len() > 128 {
        return Err(AccessibilityPolicyError::new(format!(
            "{field} `{value}` must be lowercase kebab-case ASCII"
        )));
    }
    Ok(())
}

fn is_slug(value: &str) -> bool {
    is_kebab_ascii(value, 128)
}

fn is_kebab_ascii(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_bytes
        && value.as_bytes()[0].is_ascii_lowercase()
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn validate_selection_text(
    pair_id: &str,
    role: &str,
    value: &str,
) -> Result<(), AccessibilityPolicyError> {
    if value.is_empty()
        || value.len() > 128
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(AccessibilityPolicyError::new(format!(
            "contrast pair `{pair_id}` selection {role} must be 1-128 bytes without controls or surrounding whitespace"
        )));
    }
    Ok(())
}

fn validate_text(field: &str, value: &str) -> Result<(), AccessibilityPolicyError> {
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(AccessibilityPolicyError::new(format!(
            "{field} must be non-empty, at most 4,096 bytes, and contain no control characters"
        )));
    }
    Ok(())
}

fn validate_sha256_subject(value: &str) -> Result<(), AccessibilityPolicyError> {
    let valid = value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    });
    if !valid {
        return Err(AccessibilityPolicyError::new(format!(
            "accessibility exception subjectId `{value}` must be a canonical prefixed SHA-256"
        )));
    }
    Ok(())
}

fn validate_iso_date(value: &str) -> Result<(), AccessibilityPolicyError> {
    let bytes = value.as_bytes();
    let canonical = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit());
    if !canonical {
        return Err(AccessibilityPolicyError::new(format!(
            "accessibility exception expiresOn `{value}` must use a real `YYYY-MM-DD` date"
        )));
    }
    let year = parse_date_component(&bytes[0..4]);
    let month = parse_date_component(&bytes[5..7]);
    let day = parse_date_component(&bytes[8..10]);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let maximum_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    if year == 0 || day == 0 || day > maximum_day {
        return Err(AccessibilityPolicyError::new(format!(
            "accessibility exception expiresOn `{value}` must use a real `YYYY-MM-DD` date"
        )));
    }
    Ok(())
}

fn parse_date_component(bytes: &[u8]) -> u16 {
    bytes
        .iter()
        .fold(0_u16, |value, byte| value * 10 + u16::from(byte - b'0'))
}

#[cfg(all(test, not(feature = "package-verify")))]
#[path = "../tests/internal/accessibility_policy.rs"]
mod tests;
