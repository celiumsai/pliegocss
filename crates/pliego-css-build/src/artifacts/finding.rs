//! Canonical agent-ready finding contract.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::sha256_hex;

/// Semantic version of the canonical finding document.
pub const FINDING_SCHEMA_VERSION: &str = "1.0.0";

const MAX_FINDING_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_FIELD_BYTES: usize = 1024 * 1024;
const MAX_FINDINGS: usize = 65_536;
const MAX_EVIDENCE: usize = 1_024;
const MAX_SUGGESTIONS: usize = 256;

/// Error returned when a finding document violates its closed schema contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FindingContractError {
    reason: String,
}

impl FindingContractError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl fmt::Display for FindingContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::error::Error for FindingContractError {}

/// Tool identity carried by a canonical finding document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FindingTool {
    name: String,
    version: String,
}

impl FindingTool {
    /// Creates a validated tool identity.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] when either field is empty or unsafe for a wire artifact.
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, FindingContractError> {
        let value = Self {
            name: name.into(),
            version: version.into(),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), FindingContractError> {
        validate_slug("tool.name", &self.name)?;
        validate_text("tool.version", &self.version)
    }
}

/// Severity shared by human, JSON, and SARIF projections.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingSeverity {
    /// Informational evidence that does not fail a policy.
    Info,
    /// Actionable policy warning.
    Warning,
    /// Policy violation that fails the selected gate.
    Error,
}

impl FindingSeverity {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// Verification boundary for one finding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FindingVerification {
    /// The recorded evidence proves the finding inside the declared analysis boundary.
    Verified,
    /// Available evidence is insufficient to prove the finding.
    Unverified,
    /// Static or automated analysis cannot close the finding without a manual check.
    ManualRequired,
}

impl FindingVerification {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Unverified => "unverified",
            Self::ManualRequired => "manual-required",
        }
    }
}

/// Risk assigned to a ranked suggestion.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingRisk {
    /// Mechanically local change with strong evidence.
    Low,
    /// Change that requires review of related declarations or states.
    Medium,
    /// Change with broad, behavioral, or browser-dependent impact.
    High,
}

impl FindingRisk {
    /// Returns the canonical wire label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// Optional generated-artifact range linked from an authored source span.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FindingSourceMap {
    file: String,
    byte_start: usize,
    byte_end: usize,
}

impl FindingSourceMap {
    /// Creates a validated generated-artifact range.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] for a non-portable path or invalid half-open range.
    pub fn new(
        generated_file: impl Into<String>,
        generated_byte_start: usize,
        generated_byte_end: usize,
    ) -> Result<Self, FindingContractError> {
        let value = Self {
            file: generated_file.into(),
            byte_start: generated_byte_start,
            byte_end: generated_byte_end,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), FindingContractError> {
        validate_logical_path("source.sourceMap.file", &self.file)?;
        validate_range("source.sourceMap.byteRange", self.byte_start, self.byte_end)
    }
}

/// Exact authored source range associated with a finding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FindingSource {
    file: String,
    byte_start: usize,
    byte_end: usize,
    start_line: Option<usize>,
    start_column: Option<usize>,
    end_line: Option<usize>,
    end_column: Option<usize>,
    source_map: Option<FindingSourceMap>,
}

impl FindingSource {
    /// Creates a source with an exact zero-based, half-open UTF-8 byte range.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] for a non-portable path or invalid byte range.
    pub fn new(
        file: impl Into<String>,
        byte_start: usize,
        byte_end: usize,
    ) -> Result<Self, FindingContractError> {
        let value = Self {
            file: file.into(),
            byte_start,
            byte_end,
            start_line: None,
            start_column: None,
            end_line: None,
            end_column: None,
            source_map: None,
        };
        value.validate()?;
        Ok(value)
    }

    /// Adds one-based Unicode-scalar line and column coordinates.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] when a position is zero or ends before it starts.
    pub fn with_position(
        mut self,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
    ) -> Result<Self, FindingContractError> {
        self.start_line = Some(start_line);
        self.start_column = Some(start_column);
        self.end_line = Some(end_line);
        self.end_column = Some(end_column);
        self.validate()?;
        Ok(self)
    }

    /// Links the authored span to an exact generated-artifact range.
    #[must_use]
    pub fn with_source_map(mut self, source_map: FindingSourceMap) -> Self {
        self.source_map = Some(source_map);
        self
    }

    /// Returns the portable logical source path.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the zero-based UTF-8 byte start.
    #[must_use]
    pub const fn byte_start(&self) -> usize {
        self.byte_start
    }

    /// Returns the exclusive zero-based UTF-8 byte end.
    #[must_use]
    pub const fn byte_end(&self) -> usize {
        self.byte_end
    }

    fn validate(&self) -> Result<(), FindingContractError> {
        validate_logical_path("source.file", &self.file)?;
        validate_range("source.byteRange", self.byte_start, self.byte_end)?;
        match (
            self.start_line,
            self.start_column,
            self.end_line,
            self.end_column,
        ) {
            (None, None, None, None) => {}
            (Some(start_line), Some(start_column), Some(end_line), Some(end_column)) => {
                if [start_line, start_column, end_line, end_column].contains(&0) {
                    return Err(FindingContractError::new(
                        "source line and column coordinates are one-based",
                    ));
                }
                if (end_line, end_column) < (start_line, start_column) {
                    return Err(FindingContractError::new(
                        "source end position precedes its start position",
                    ));
                }
            }
            _ => {
                return Err(FindingContractError::new(
                    "source line and column coordinates must be all present or all null",
                ));
            }
        }
        if let Some(source_map) = &self.source_map {
            source_map.validate()?;
        }
        Ok(())
    }
}

/// Policy and semantic rule that caused a finding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FindingCause {
    policy: String,
    rule: String,
    explanation: String,
}

impl FindingCause {
    /// Creates a validated cause.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] for invalid identifiers or empty explanation.
    pub fn new(
        policy: impl Into<String>,
        rule: impl Into<String>,
        explanation: impl Into<String>,
    ) -> Result<Self, FindingContractError> {
        let value = Self {
            policy: policy.into(),
            rule: rule.into(),
            explanation: explanation.into(),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), FindingContractError> {
        validate_dotted_id("cause.policy", &self.policy)?;
        validate_slug("cause.rule", &self.rule)?;
        validate_text("cause.explanation", &self.explanation)
    }
}

/// One deterministic piece of evidence supporting a finding.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FindingEvidence {
    kind: String,
    name: String,
    value: String,
    unit: Option<String>,
    source: String,
}

impl FindingEvidence {
    /// Creates validated evidence.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] when identifiers or values are empty or unsafe.
    pub fn new(
        kind: impl Into<String>,
        name: impl Into<String>,
        value: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<Self, FindingContractError> {
        let item = Self {
            kind: kind.into(),
            name: name.into(),
            value: value.into(),
            unit: None,
            source: source.into(),
        };
        item.validate()?;
        Ok(item)
    }

    /// Adds a stable unit label such as `ratio`, `bytes`, or `selector`.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] for an invalid unit identifier.
    pub fn with_unit(mut self, unit: impl Into<String>) -> Result<Self, FindingContractError> {
        self.unit = Some(unit.into());
        self.validate()?;
        Ok(self)
    }

    fn validate(&self) -> Result<(), FindingContractError> {
        validate_slug("evidence.kind", &self.kind)?;
        validate_slug("evidence.name", &self.name)?;
        validate_text("evidence.value", &self.value)?;
        validate_slug("evidence.source", &self.source)?;
        if let Some(unit) = &self.unit {
            validate_slug("evidence.unit", unit)?;
        }
        Ok(())
    }
}

/// Ranked, non-authoritative repair suggestion.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FindingSuggestion {
    rank: u16,
    id: String,
    message: String,
    risk: FindingRisk,
    scope: String,
    prerequisites: Vec<String>,
}

impl FindingSuggestion {
    /// Creates a ranked suggestion. Rank is one-based and must be contiguous inside a finding.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] for rank zero or invalid identifiers/text.
    pub fn new(
        rank: u16,
        id: impl Into<String>,
        message: impl Into<String>,
        risk: FindingRisk,
        scope: impl Into<String>,
    ) -> Result<Self, FindingContractError> {
        let value = Self {
            rank,
            id: id.into(),
            message: message.into(),
            risk,
            scope: scope.into(),
            prerequisites: Vec::new(),
        };
        value.validate()?;
        Ok(value)
    }

    /// Replaces the prerequisite list.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] for duplicates or invalid prerequisite identifiers.
    pub fn with_prerequisites(
        mut self,
        prerequisites: impl IntoIterator<Item = String>,
    ) -> Result<Self, FindingContractError> {
        self.prerequisites = prerequisites.into_iter().collect();
        self.prerequisites.sort();
        self.validate()?;
        Ok(self)
    }

    /// Returns the one-based rank inside the owning finding.
    #[must_use]
    pub const fn rank(&self) -> u16 {
        self.rank
    }

    /// Returns the stable suggestion identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the declared repair risk.
    #[must_use]
    pub const fn risk(&self) -> FindingRisk {
        self.risk
    }

    /// Returns the bounded change scope.
    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// Returns sorted prerequisite identifiers.
    #[must_use]
    pub fn prerequisites(&self) -> &[String] {
        &self.prerequisites
    }

    fn validate(&self) -> Result<(), FindingContractError> {
        if self.rank == 0 {
            return Err(FindingContractError::new(
                "suggestion.rank must be one-based",
            ));
        }
        validate_slug("suggestion.id", &self.id)?;
        validate_text("suggestion.message", &self.message)?;
        validate_slug("suggestion.scope", &self.scope)?;
        validate_sorted_unique_ids("suggestion.prerequisites", &self.prerequisites)
    }
}

/// Reviewed policy exception attached to a finding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FindingException {
    id: String,
    justification: String,
    expires_on: Option<String>,
}

impl FindingException {
    /// Creates a non-expiring exception record.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] for an invalid ID or empty justification.
    pub fn new(
        id: impl Into<String>,
        justification: impl Into<String>,
    ) -> Result<Self, FindingContractError> {
        let value = Self {
            id: id.into(),
            justification: justification.into(),
            expires_on: None,
        };
        value.validate()?;
        Ok(value)
    }

    /// Adds an ISO `YYYY-MM-DD` expiry date.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] when the date is not canonical.
    pub fn expiring_on(mut self, date: impl Into<String>) -> Result<Self, FindingContractError> {
        self.expires_on = Some(date.into());
        self.validate()?;
        Ok(self)
    }

    fn validate(&self) -> Result<(), FindingContractError> {
        validate_slug("exception.id", &self.id)?;
        validate_text("exception.justification", &self.justification)?;
        if let Some(date) = &self.expires_on {
            if !is_iso_date(date) {
                return Err(FindingContractError::new(
                    "exception.expiresOn must use YYYY-MM-DD",
                ));
            }
        }
        Ok(())
    }
}

/// Canonical finding from which human, JSON, and SARIF renderers derive.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Finding {
    fingerprint: String,
    code: String,
    category: String,
    severity: FindingSeverity,
    message: String,
    verification: FindingVerification,
    deterministic: bool,
    source: Option<FindingSource>,
    cause: FindingCause,
    context: BTreeMap<String, String>,
    evidence: Vec<FindingEvidence>,
    suggestions: Vec<FindingSuggestion>,
    exception: Option<FindingException>,
}

impl Finding {
    /// Creates a finding with no source, evidence, suggestions, or exception.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] when the stable code, category, message, or cause is invalid.
    pub fn new(
        code: impl Into<String>,
        category: impl Into<String>,
        severity: FindingSeverity,
        message: impl Into<String>,
        verification: FindingVerification,
        cause: FindingCause,
    ) -> Result<Self, FindingContractError> {
        let mut value = Self {
            fingerprint: String::new(),
            code: code.into(),
            category: category.into(),
            severity,
            message: message.into(),
            verification,
            deterministic: true,
            source: None,
            cause,
            context: BTreeMap::new(),
            evidence: Vec::new(),
            suggestions: Vec::new(),
            exception: None,
        };
        value.refresh_fingerprint()?;
        value.validate()?;
        Ok(value)
    }

    /// Adds the exact authored source range.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] if refreshing the deterministic fingerprint fails.
    pub fn with_source(mut self, source: FindingSource) -> Result<Self, FindingContractError> {
        self.source = Some(source);
        self.refresh_fingerprint()?;
        self.validate()?;
        Ok(self)
    }

    /// Adds or replaces a deterministic context value.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] for an invalid key/value or fingerprint failure.
    pub fn with_context(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, FindingContractError> {
        let name = name.into();
        let value = value.into();
        validate_slug("context key", &name)?;
        validate_text("context value", &value)?;
        self.context.insert(name, value);
        self.refresh_fingerprint()?;
        self.validate()?;
        Ok(self)
    }

    /// Adds evidence and canonicalizes evidence order.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] for duplicates, limits, or fingerprint failure.
    pub fn with_evidence(
        mut self,
        evidence: FindingEvidence,
    ) -> Result<Self, FindingContractError> {
        self.evidence.push(evidence);
        self.evidence.sort();
        self.refresh_fingerprint()?;
        self.validate()?;
        Ok(self)
    }

    /// Adds a ranked suggestion and canonicalizes rank order.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] unless ranks remain unique and contiguous from one.
    pub fn with_suggestion(
        mut self,
        suggestion: FindingSuggestion,
    ) -> Result<Self, FindingContractError> {
        self.suggestions.push(suggestion);
        self.suggestions.sort_by_key(|item| item.rank);
        self.validate()?;
        Ok(self)
    }

    /// Attaches a reviewed exception.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] for an invalid exception.
    pub fn with_exception(
        mut self,
        exception: FindingException,
    ) -> Result<Self, FindingContractError> {
        self.exception = Some(exception);
        self.validate()?;
        Ok(self)
    }

    /// Returns the stable instance fingerprint.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Returns the stable finding code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the finding verification boundary.
    #[must_use]
    pub const fn verification(&self) -> FindingVerification {
        self.verification
    }

    /// Returns the exact authored source range when one exists.
    #[must_use]
    pub const fn source(&self) -> Option<&FindingSource> {
        self.source.as_ref()
    }

    /// Returns ranked, non-authoritative suggestions.
    #[must_use]
    pub fn suggestions(&self) -> &[FindingSuggestion] {
        &self.suggestions
    }

    /// Returns whether a reviewed exception is attached.
    #[must_use]
    pub const fn is_excepted(&self) -> bool {
        self.exception.is_some()
    }

    fn refresh_fingerprint(&mut self) -> Result<(), FindingContractError> {
        let input = FindingFingerprint {
            schema_version: FINDING_SCHEMA_VERSION,
            code: &self.code,
            category: &self.category,
            severity: self.severity,
            verification: self.verification,
            source: self.source.as_ref(),
            cause: &self.cause,
            context: &self.context,
            evidence: &self.evidence,
        };
        let bytes = serde_json::to_vec(&input).map_err(|error| {
            FindingContractError::new(format!("cannot fingerprint finding: {error}"))
        })?;
        self.fingerprint = format!("sha256:{}", sha256_hex(&bytes));
        Ok(())
    }

    fn validate(&self) -> Result<(), FindingContractError> {
        validate_code(&self.code)?;
        validate_slug("finding.category", &self.category)?;
        validate_text("finding.message", &self.message)?;
        if !self.deterministic {
            return Err(FindingContractError::new(
                "finding.deterministic must be true in schema 1.0.0",
            ));
        }
        if let Some(source) = &self.source {
            source.validate()?;
        }
        self.cause.validate()?;
        for (name, value) in &self.context {
            validate_slug("context key", name)?;
            validate_text("context value", value)?;
        }
        if self.evidence.len() > MAX_EVIDENCE {
            return Err(FindingContractError::new(
                "finding contains more than 1,024 evidence entries",
            ));
        }
        for evidence in &self.evidence {
            evidence.validate()?;
        }
        validate_sorted_unique("finding.evidence", &self.evidence)?;
        if self.suggestions.len() > MAX_SUGGESTIONS {
            return Err(FindingContractError::new(
                "finding contains more than 256 suggestions",
            ));
        }
        for (index, suggestion) in self.suggestions.iter().enumerate() {
            suggestion.validate()?;
            let expected = u16::try_from(index + 1)
                .map_err(|_| FindingContractError::new("suggestion rank exceeds schema limits"))?;
            if suggestion.rank != expected {
                return Err(FindingContractError::new(
                    "suggestion ranks must be unique and contiguous from one",
                ));
            }
        }
        if let Some(exception) = &self.exception {
            exception.validate()?;
        }
        let mut expected = self.clone();
        expected.refresh_fingerprint()?;
        if expected.fingerprint != self.fingerprint {
            return Err(FindingContractError::new(format!(
                "finding fingerprint `{}` does not match `{}`",
                self.fingerprint, expected.fingerprint
            )));
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FindingFingerprint<'a> {
    schema_version: &'static str,
    code: &'a str,
    category: &'a str,
    severity: FindingSeverity,
    verification: FindingVerification,
    source: Option<&'a FindingSource>,
    cause: &'a FindingCause,
    context: &'a BTreeMap<String, String>,
    evidence: &'a [FindingEvidence],
}

/// Closed document that carries canonical findings for one command execution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FindingDocument {
    schema_version: String,
    tool: FindingTool,
    command: String,
    findings: Vec<Finding>,
}

impl FindingDocument {
    /// Creates a canonical document and sorts findings by source, code, and fingerprint.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] for invalid fields, duplicate findings, or size limits.
    pub fn new(
        tool: FindingTool,
        command: impl Into<String>,
        mut findings: Vec<Finding>,
    ) -> Result<Self, FindingContractError> {
        sort_findings(&mut findings);
        let document = Self {
            schema_version: FINDING_SCHEMA_VERSION.into(),
            tool,
            command: command.into(),
            findings,
        };
        document.validate()?;
        Ok(document)
    }

    /// Returns canonical findings in their wire order.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Serializes the closed JSON contract with one trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] if validation or serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, FindingContractError> {
        self.validate()?;
        let mut output = serde_json::to_string_pretty(self).map_err(|error| {
            FindingContractError::new(format!("cannot serialize finding document: {error}"))
        })?;
        output.push('\n');
        if output.len() > MAX_FINDING_DOCUMENT_BYTES {
            return Err(FindingContractError::new("finding document exceeds 16 MiB"));
        }
        Ok(output)
    }

    /// Renders the same findings for humans without changing code, severity, or evidence.
    ///
    /// # Errors
    ///
    /// Returns [`FindingContractError`] if the document is not valid.
    pub fn to_human(&self) -> Result<String, FindingContractError> {
        self.validate()?;
        if self.findings.is_empty() {
            return Ok("no findings\n".into());
        }
        let mut output = String::new();
        for finding in &self.findings {
            output.push_str(finding.severity.as_str());
            output.push('[');
            output.push_str(&finding.code);
            output.push_str("]: ");
            output.push_str(&finding.message);
            output.push('\n');
            if let Some(source) = &finding.source {
                output.push_str("  --> ");
                output.push_str(&source.file);
                if let (Some(line), Some(column)) = (source.start_line, source.start_column) {
                    output.push(':');
                    output.push_str(&line.to_string());
                    output.push(':');
                    output.push_str(&column.to_string());
                } else {
                    output.push_str(" [bytes ");
                    output.push_str(&source.byte_start.to_string());
                    output.push_str("..");
                    output.push_str(&source.byte_end.to_string());
                    output.push(')');
                }
                output.push('\n');
            }
            output.push_str("  cause: ");
            output.push_str(&finding.cause.policy);
            output.push('/');
            output.push_str(&finding.cause.rule);
            output.push_str(" - ");
            output.push_str(&finding.cause.explanation);
            output.push('\n');
            output.push_str("  verification: ");
            output.push_str(finding.verification.as_str());
            output.push('\n');
            for evidence in &finding.evidence {
                output.push_str("  evidence[");
                output.push_str(&evidence.kind);
                output.push_str("]: ");
                output.push_str(&evidence.name);
                output.push_str(" = ");
                output.push_str(&evidence.value);
                if let Some(unit) = &evidence.unit {
                    output.push(' ');
                    output.push_str(unit);
                }
                output.push_str(" (");
                output.push_str(&evidence.source);
                output.push_str(")\n");
            }
            for suggestion in &finding.suggestions {
                output.push_str("  help[");
                output.push_str(&suggestion.rank.to_string());
                output.push('/');
                output.push_str(suggestion.risk.as_str());
                output.push_str("]: ");
                output.push_str(&suggestion.message);
                output.push('\n');
            }
            if let Some(exception) = &finding.exception {
                output.push_str("  exception: ");
                output.push_str(&exception.id);
                output.push_str(" - ");
                output.push_str(&exception.justification);
                output.push('\n');
            }
        }
        Ok(output)
    }

    fn validate(&self) -> Result<(), FindingContractError> {
        if self.schema_version != FINDING_SCHEMA_VERSION {
            return Err(FindingContractError::new(format!(
                "unsupported finding schema `{}`; expected `{FINDING_SCHEMA_VERSION}`",
                self.schema_version
            )));
        }
        self.tool.validate()?;
        validate_slug("command", &self.command)?;
        if self.findings.len() > MAX_FINDINGS {
            return Err(FindingContractError::new(
                "finding document contains more than 65,536 findings",
            ));
        }
        for finding in &self.findings {
            finding.validate()?;
        }
        let mut canonical = self.findings.clone();
        sort_findings(&mut canonical);
        if canonical != self.findings {
            return Err(FindingContractError::new(
                "findings are not in canonical source/code/fingerprint order",
            ));
        }
        for pair in self.findings.windows(2) {
            if pair[0].fingerprint == pair[1].fingerprint {
                return Err(FindingContractError::new(format!(
                    "duplicate finding fingerprint `{}`",
                    pair[0].fingerprint
                )));
            }
        }
        Ok(())
    }
}

/// Parses and validates a canonical finding JSON document.
///
/// # Errors
///
/// Returns [`FindingContractError`] for malformed JSON, an unsupported schema, noncanonical order,
/// duplicate findings, fingerprint tampering, unsafe fields, or defensive-limit violations.
pub fn parse_finding_document(source: &[u8]) -> Result<FindingDocument, FindingContractError> {
    if source.len() > MAX_FINDING_DOCUMENT_BYTES {
        return Err(FindingContractError::new("finding document exceeds 16 MiB"));
    }
    let document: FindingDocument = serde_json::from_slice(source)
        .map_err(|error| FindingContractError::new(format!("invalid finding JSON: {error}")))?;
    document.validate()?;
    Ok(document)
}

fn sort_findings(findings: &mut [Finding]) {
    findings.sort_by(compare_findings);
}

fn compare_findings(left: &Finding, right: &Finding) -> Ordering {
    let source_order =
        match (&left.source, &right.source) {
            (Some(left), Some(right)) => (left.file.as_str(), left.byte_start, left.byte_end)
                .cmp(&(right.file.as_str(), right.byte_start, right.byte_end)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
    source_order
        .then_with(|| left.code.cmp(&right.code))
        .then_with(|| left.fingerprint.cmp(&right.fingerprint))
}

fn validate_range(
    field: &str,
    byte_start: usize,
    byte_end: usize,
) -> Result<(), FindingContractError> {
    if byte_end < byte_start {
        return Err(FindingContractError::new(format!(
            "{field} end precedes its start"
        )));
    }
    Ok(())
}

fn validate_code(code: &str) -> Result<(), FindingContractError> {
    if !(3..=64).contains(&code.len())
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-')
        || !code.as_bytes()[0].is_ascii_uppercase()
        || code.ends_with('-')
        || code.contains("--")
    {
        return Err(FindingContractError::new(format!(
            "finding.code `{code}` must be 3-64 uppercase ASCII letters, digits, or single hyphens"
        )));
    }
    Ok(())
}

fn validate_slug(field: &str, value: &str) -> Result<(), FindingContractError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || !value.as_bytes()[0].is_ascii_lowercase()
        || value.ends_with('-')
        || value.contains("--")
    {
        return Err(FindingContractError::new(format!(
            "{field} `{value}` must be lowercase kebab-case ASCII"
        )));
    }
    Ok(())
}

fn validate_dotted_id(field: &str, value: &str) -> Result<(), FindingContractError> {
    if value.is_empty() || value.len() > 256 {
        return Err(FindingContractError::new(format!(
            "{field} must be 1-256 bytes"
        )));
    }
    for segment in value.split('.') {
        validate_slug(field, segment)?;
    }
    Ok(())
}

fn validate_text(field: &str, value: &str) -> Result<(), FindingContractError> {
    if value.is_empty() || value.len() > MAX_FIELD_BYTES || value.chars().any(char::is_control) {
        return Err(FindingContractError::new(format!(
            "{field} must be non-empty, at most 1 MiB, and contain no control characters"
        )));
    }
    Ok(())
}

fn validate_logical_path(field: &str, value: &str) -> Result<(), FindingContractError> {
    validate_text(field, value)?;
    if value.starts_with('/')
        || value.contains('\\')
        || value.contains(':')
        || value
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(FindingContractError::new(format!(
            "{field} `{value}` must be a portable relative logical path"
        )));
    }
    Ok(())
}

fn validate_sorted_unique<T: Ord>(field: &str, values: &[T]) -> Result<(), FindingContractError> {
    for pair in values.windows(2) {
        match pair[0].cmp(&pair[1]) {
            Ordering::Less => {}
            Ordering::Equal => {
                return Err(FindingContractError::new(format!(
                    "{field} contains a duplicate"
                )));
            }
            Ordering::Greater => {
                return Err(FindingContractError::new(format!(
                    "{field} is not in canonical order"
                )));
            }
        }
    }
    Ok(())
}

fn validate_sorted_unique_ids(field: &str, values: &[String]) -> Result<(), FindingContractError> {
    for value in values {
        validate_dotted_id(field, value)?;
    }
    validate_sorted_unique(field, values)
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return false;
    }
    let year = u16::from(bytes[0] - b'0') * 1_000
        + u16::from(bytes[1] - b'0') * 100
        + u16::from(bytes[2] - b'0') * 10
        + u16::from(bytes[3] - b'0');
    let month = (bytes[5] - b'0') * 10 + (bytes[6] - b'0');
    let day = (bytes[8] - b'0') * 10 + (bytes[9] - b'0');
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => return false,
    };
    year != 0 && day != 0 && day <= days_in_month
}
