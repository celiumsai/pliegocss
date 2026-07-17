//! Closed usage-analysis, observation, and retention contracts for `PliegoCSS`.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

pub use pliego_css_build::artifacts::AssetRuleSelection;
use pliego_css_build::artifacts::{
    MAX_DOCUMENT_BYTES, ReachabilityDocument, ReachabilityIndex, parse_reachability_document,
    sha256_hex, validate_asset_bundle_id,
};
use pliego_css_ir::StyleId;
use serde::{Deserialize, Serialize};

mod token_usage;

pub use token_usage::{
    TOKEN_USAGE_FILE, TOKEN_USAGE_SCHEMA_VERSION, TokenUsageConsumerInput, TokenUsageReport,
    TokenUsageStatus, build_token_usage_report, collect_selected_bundle_token_references,
    collect_selected_token_references, collect_selected_token_usage_consumers,
    collect_token_usage_consumers, explain_token_usage, explain_token_usage_text,
    parse_token_usage_report,
};

/// Maximum accepted size of one usage, observation, or retention document (16 MiB).
pub const MAX_USAGE_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;

const MAX_ITEMS: usize = 65_535;
const MAX_TEXT_BYTES: usize = 4 * 1024;
const MAX_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_RETENTION_ID_BYTES: usize = 128;
const INVALID_ANALYSIS: &str = "invalid usage analysis";
const INVALID_OBSERVATION: &str = "invalid usage observation";
const INVALID_RETENTION: &str = "invalid usage retention";

/// One exact compiler origin in the all-compiled usage universe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageOriginInput {
    source: String,
    file: String,
    byte_start: usize,
    byte_end: usize,
    macro_kind: String,
    reason: String,
}

impl UsageOriginInput {
    /// Creates one exact source-origin input.
    #[must_use]
    pub fn new(
        source: impl Into<String>,
        file: impl Into<String>,
        byte_start: usize,
        byte_end: usize,
        macro_kind: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            file: file.into(),
            byte_start,
            byte_end,
            macro_kind: macro_kind.into(),
            reason: reason.into(),
        }
    }
}

/// One bundle-qualified complete `StyleId` occurrence in the all-compiled universe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageStyleInput {
    bundle_id: String,
    style_id: String,
    class_name: String,
    origins: Vec<UsageOriginInput>,
}

impl UsageStyleInput {
    /// Creates one bundle-qualified style-universe input.
    #[must_use]
    pub fn new(
        bundle_id: impl Into<String>,
        style_id: impl Into<String>,
        class_name: impl Into<String>,
        origins: Vec<UsageOriginInput>,
    ) -> Self {
        Self {
            bundle_id: bundle_id.into(),
            style_id: style_id.into(),
            class_name: class_name.into(),
            origins,
        }
    }
}

/// Deterministic collector for compiler candidates that share one `StyleId`.
pub struct UsageStyleCollector {
    bundle_id: String,
    styles: BTreeMap<String, (String, BTreeSet<UsageOriginInput>)>,
}

/// Borrowed compiler-candidate projection accepted by [`collect_usage_style_inputs`].
pub struct UsageCandidateInput<'a> {
    style_id: String,
    class_name: String,
    source: &'a str,
    file: Option<&'a str>,
    byte_start: Option<usize>,
    byte_end: Option<usize>,
    macro_kind: &'a str,
    reason: &'a str,
}

impl<'a> UsageCandidateInput<'a> {
    /// Creates a candidate projection; collection rejects missing source ranges.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        style_id: impl Into<String>,
        class_name: impl Into<String>,
        source: &'a str,
        file: Option<&'a str>,
        byte_start: Option<usize>,
        byte_end: Option<usize>,
        macro_kind: &'a str,
        reason: &'a str,
    ) -> Self {
        Self {
            style_id: style_id.into(),
            class_name: class_name.into(),
            source,
            file,
            byte_start,
            byte_end,
            macro_kind,
            reason,
        }
    }
}

/// Collects compiler candidates into canonical bundle-qualified style inputs.
///
/// # Errors
///
/// Returns an error for invalid identities, missing source ranges, conflicting class names, or
/// duplicate exact origins.
pub fn collect_usage_style_inputs<'a>(
    bundle_id: impl Into<String>,
    candidates: impl IntoIterator<Item = UsageCandidateInput<'a>>,
) -> Result<Vec<UsageStyleInput>, String> {
    let mut styles = UsageStyleCollector::new(bundle_id)?;
    for candidate in candidates {
        let (Some(file), Some(byte_start), Some(byte_end)) =
            (candidate.file, candidate.byte_start, candidate.byte_end)
        else {
            return Err("usage origin lacks a source range".into());
        };
        styles.push(
            candidate.style_id,
            candidate.class_name,
            UsageOriginInput::new(
                candidate.source,
                file,
                byte_start,
                byte_end,
                candidate.macro_kind,
                candidate.reason,
            ),
        )?;
    }
    styles.finish()
}

impl UsageStyleCollector {
    /// Creates a collector bound to one bundle.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid portable bundle identifier.
    pub fn new(bundle_id: impl Into<String>) -> Result<Self, String> {
        let bundle_id = bundle_id.into();
        validate_asset_bundle_id(&bundle_id)?;
        Ok(Self {
            bundle_id,
            styles: BTreeMap::new(),
        })
    }

    /// Adds one compiler candidate and its exact origin.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identity, class-name drift, or a duplicate exact origin.
    pub fn push(
        &mut self,
        style_id: impl Into<String>,
        class_name: impl Into<String>,
        origin: UsageOriginInput,
    ) -> Result<(), String> {
        let (style_id, class_name) = (style_id.into(), class_name.into());
        validate_style_identity(&style_id, &class_name, INVALID_ANALYSIS)?;
        validate_origin_input(&origin)?;
        let entry = self
            .styles
            .entry(style_id.clone())
            .or_insert_with(|| (class_name.clone(), BTreeSet::new()));
        if entry.0 != class_name {
            return Err(format!(
                "usage StyleId `{style_id}` maps to inconsistent class names"
            ));
        }
        if !entry.1.insert(origin) {
            return Err(format!(
                "duplicate usage origin for `{}/{style_id}`",
                self.bundle_id
            ));
        }
        Ok(())
    }

    /// Finishes canonical bundle-qualified style inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when a collected origin becomes invalid during canonicalization.
    pub fn finish(self) -> Result<Vec<UsageStyleInput>, String> {
        self.styles
            .into_iter()
            .map(|(style_id, (class_name, origins))| {
                Ok(UsageStyleInput::new(
                    &self.bundle_id,
                    style_id,
                    class_name,
                    origins.into_iter().collect(),
                ))
            })
            .collect()
    }
}

impl Ord for UsageOriginInput {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        origin_input_cmp(self, other)
    }
}

impl PartialOrd for UsageOriginInput {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Declared coverage of one explicit usage-observation snapshot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UsageObservationCoverage {
    /// The producer exercised a finite, declared sample. Absence is only `unobserved`.
    Sampled,
    /// The producer attests that its declared observation space is complete.
    ProducerAttestedComplete,
}

/// One bundle-qualified positive style hit supplied by an observation producer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageObservedStyleInput {
    bundle_id: String,
    style_id: String,
}

impl UsageObservedStyleInput {
    /// Creates one stable positive style hit.
    #[must_use]
    pub fn new(bundle_id: impl Into<String>, style_id: impl Into<String>) -> Self {
        Self {
            bundle_id: bundle_id.into(),
            style_id: style_id.into(),
        }
    }
}

/// Finite routes, islands, themes, browsers, viewports, and states exercised by a producer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageObservationScopeInput {
    routes: Vec<String>,
    islands: Vec<String>,
    themes: Vec<String>,
    browsers: Vec<String>,
    viewports: Vec<String>,
    states: Vec<String>,
}

impl UsageObservationScopeInput {
    /// Creates one explicit observation scope. Arrays are canonicalized but duplicates fail.
    #[must_use]
    pub fn new(
        routes: Vec<String>,
        islands: Vec<String>,
        themes: Vec<String>,
        browsers: Vec<String>,
        viewports: Vec<String>,
        states: Vec<String>,
    ) -> Self {
        Self {
            routes,
            islands,
            themes,
            browsers,
            viewports,
            states,
        }
    }
}

/// Adapter-facing input used to build a canonical observation sidecar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageObservationInput {
    universe_sha256: String,
    reachability_sha256: String,
    coverage: UsageObservationCoverage,
    producer_name: String,
    producer_version: String,
    contexts: UsageObservationScopeInput,
    unknown_dynamic_inputs: Vec<String>,
    observed_styles: Vec<UsageObservedStyleInput>,
}

impl UsageObservationInput {
    /// Creates one exact observation snapshot input.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        universe_sha256: impl Into<String>,
        reachability_sha256: impl Into<String>,
        coverage: UsageObservationCoverage,
        producer_name: impl Into<String>,
        producer_version: impl Into<String>,
        contexts: UsageObservationScopeInput,
        unknown_dynamic_inputs: Vec<String>,
        observed_styles: Vec<UsageObservedStyleInput>,
    ) -> Self {
        Self {
            universe_sha256: universe_sha256.into(),
            reachability_sha256: reachability_sha256.into(),
            coverage,
            producer_name: producer_name.into(),
            producer_version: producer_version.into(),
            contexts,
            unknown_dynamic_inputs,
            observed_styles,
        }
    }
}

/// One explicit bundle-qualified retention grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageRetentionEntryInput {
    id: String,
    bundle_id: String,
    style_id: String,
    justification: String,
}

impl UsageRetentionEntryInput {
    /// Creates one reviewed retention entry.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        bundle_id: impl Into<String>,
        style_id: impl Into<String>,
        justification: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            bundle_id: bundle_id.into(),
            style_id: style_id.into(),
            justification: justification.into(),
        }
    }
}

/// Adapter-facing input used to build a canonical retention sidecar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageRetentionInput {
    universe_sha256: String,
    reachability_sha256: String,
    entries: Vec<UsageRetentionEntryInput>,
}

impl UsageRetentionInput {
    /// Creates one exact retention-policy snapshot input.
    #[must_use]
    pub fn new(
        universe_sha256: impl Into<String>,
        reachability_sha256: impl Into<String>,
        entries: Vec<UsageRetentionEntryInput>,
    ) -> Self {
        Self {
            universe_sha256: universe_sha256.into(),
            reachability_sha256: reachability_sha256.into(),
            entries,
        }
    }
}

/// Parsed closed schema-1 observation sidecar.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UsageObservationDocument {
    schema_version: u8,
    universe_sha256: String,
    reachability_sha256: String,
    coverage: UsageObservationCoverage,
    producer: ObservationProducer,
    contexts: ObservationContexts,
    unknown_dynamic_inputs: Vec<String>,
    observed_styles: Vec<ObservedStyle>,
}

/// Parsed closed schema-1 retention sidecar.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UsageRetentionDocument {
    schema_version: u8,
    universe_sha256: String,
    reachability_sha256: String,
    entries: Vec<RetentionEntry>,
}

/// Parsed and validated schema-1 or schema-2 usage-analysis artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UsageAnalysis {
    schema_version: u8,
    state_model_version: u8,
    analysis_unit: String,
    origin_coverage: String,
    rule_selection: AssetRuleSelection,
    universe_sha256: String,
    application: ApplicationEvidence,
    observation: ObservationEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    retention: Option<RetentionEvidence>,
    summary: UsageSummary,
    styles: Vec<UsageStyle>,
}

impl UsageAnalysis {
    /// Returns the wire schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u8 {
        self.schema_version
    }

    /// Returns the canonical all-compiled universe digest bound by sidecars.
    #[must_use]
    pub fn universe_sha256(&self) -> &str {
        &self.universe_sha256
    }

    /// Returns the number of bundle-qualified style occurrences in the analysis.
    #[must_use]
    pub fn style_count(&self) -> usize {
        self.styles.len()
    }
}

/// Immutable, fully classified source for compiler selection and usage-analysis bytes.
#[derive(Debug)]
pub struct PreparedUsageAnalysis {
    rule_selection: AssetRuleSelection,
    universe_sha256: String,
    application: ApplicationEvidence,
    observation: ObservationEvidence,
    retention: Option<RetentionEvidence>,
    styles: Vec<UsageStyle>,
    selection: UsageSelection,
}

impl PreparedUsageAnalysis {
    /// Returns the canonical pre-selection universe digest.
    #[must_use]
    pub fn universe_sha256(&self) -> &str {
        &self.universe_sha256
    }

    /// Returns the exact rule-selection contract derived during preparation.
    #[must_use]
    pub const fn rule_selection(&self) -> AssetRuleSelection {
        self.rule_selection
    }

    /// Returns the immutable bundle-qualified compiler selection.
    #[must_use]
    pub const fn selection(&self) -> &UsageSelection {
        &self.selection
    }

    /// Builds canonical schema-1 or schema-2 analysis bytes from the prepared state.
    ///
    /// # Errors
    ///
    /// Returns an error only if the prepared state cannot be validated or serialized.
    pub fn build_analysis(&self) -> Result<Vec<u8>, String> {
        let schema_version =
            if self.rule_selection == AssetRuleSelection::ReachableOrRetainedStyleIds {
                2
            } else {
                1
            };
        let analysis = UsageAnalysis {
            schema_version,
            state_model_version: schema_version,
            analysis_unit: "bundle-style-id".into(),
            origin_coverage: "compiler-verified-complete".into(),
            rule_selection: self.rule_selection,
            universe_sha256: self.universe_sha256.clone(),
            application: self.application.clone(),
            observation: self.observation.clone(),
            retention: self.retention.clone(),
            summary: summarize(&self.styles, schema_version == 2),
            styles: self.styles.clone(),
        };
        analysis.validate_for(schema_version)?;
        let mut output = serde_json::to_vec_pretty(&analysis)
            .map_err(|error| format!("cannot serialize usage analysis: {error}"))?;
        output.push(b'\n');
        require_analysis(output.len() <= MAX_USAGE_DOCUMENT_BYTES)?;
        Ok(output)
    }
}

/// Opaque bundle-qualified style selection derived from exact evidence and policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageSelection {
    selected: BTreeMap<String, BTreeSet<String>>,
    policy_retained: BTreeMap<String, BTreeSet<String>>,
}

impl UsageSelection {
    /// Returns whether the complete `StyleId` is selected in this exact bundle.
    #[must_use]
    pub fn contains(&self, bundle_id: &str, style_id: &str) -> bool {
        self.selected
            .get(bundle_id)
            .is_some_and(|styles| styles.contains(style_id))
    }

    /// Returns the number of selected bundle-qualified `StyleId` entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.selected.values().map(BTreeSet::len).sum()
    }

    /// Returns whether explicit policy, rather than reachability, retained this entry.
    #[must_use]
    pub fn is_policy_retained(&self, bundle_id: &str, style_id: &str) -> bool {
        self.policy_retained
            .get(bundle_id)
            .is_some_and(|styles| styles.contains(style_id))
    }

    /// Returns whether the derived selection is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ObservationProducer {
    name: String,
    version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ObservationContexts {
    routes: Vec<String>,
    islands: Vec<String>,
    themes: Vec<String>,
    browsers: Vec<String>,
    viewports: Vec<String>,
    states: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ObservedStyle {
    bundle_id: String,
    style_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RetentionEntry {
    id: String,
    bundle_id: String,
    style_id: String,
    justification: String,
}

impl From<UsageRetentionEntryInput> for RetentionEntry {
    fn from(input: UsageRetentionEntryInput) -> Self {
        Self {
            id: input.id,
            bundle_id: input.bundle_id,
            style_id: input.style_id,
            justification: input.justification,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    tag = "state",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum ApplicationEvidence {
    Unavailable,
    Bound {
        coverage: String,
        input_bytes: usize,
        input_sha256: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[allow(clippy::large_enum_variant)]
#[serde(
    deny_unknown_fields,
    tag = "state",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum ObservationEvidence {
    Unavailable,
    Bound {
        coverage: UsageObservationCoverage,
        producer: ObservationProducer,
        contexts: ObservationContexts,
        unknown_dynamic_inputs: Vec<String>,
        input_bytes: usize,
        input_sha256: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    tag = "state",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum RetentionEvidence {
    Bound {
        input_bytes: usize,
        input_sha256: String,
        entries: usize,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum StaticReachability {
    Reachable,
    Unreachable,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ObservationState {
    Observed,
    Unobserved,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum UsageState {
    Observed,
    Unobserved,
    Dead,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RemovalDisposition {
    Retain,
    Blocked,
    Candidate,
    Removed,
    PolicyRetained,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    tag = "state",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum StyleRetention {
    NotRetained,
    Retained {
        entry_id: String,
        justification: String,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UsageSummary {
    entries: usize,
    selected: usize,
    static_reachable: usize,
    static_unreachable: usize,
    static_unknown: usize,
    observation_observed: usize,
    observation_unobserved: usize,
    observation_unavailable: usize,
    usage_observed: usize,
    usage_unobserved: usize,
    usage_dead: usize,
    usage_unknown: usize,
    removal_retain: usize,
    removal_blocked: usize,
    removal_candidate: usize,
    removal_removed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    removal_policy_retained: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UsageStyle {
    bundle_id: String,
    style_id: String,
    class_name: String,
    selected: bool,
    static_reachability: StaticReachability,
    observation_state: ObservationState,
    usage_state: UsageState,
    #[serde(skip_serializing_if = "Option::is_none")]
    retention: Option<StyleRetention>,
    removal_disposition: RemovalDisposition,
    origins: Vec<UsageOrigin>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UsageOrigin {
    source: String,
    file: String,
    byte_start: usize,
    byte_end: usize,
    macro_kind: String,
    reason: String,
    static_reachability: StaticReachability,
    component_ids: Vec<String>,
    route_ids: Vec<String>,
    island_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UniverseStyle {
    bundle_id: String,
    style_id: String,
    class_name: String,
    origins: Vec<UniverseOrigin>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UniverseOrigin {
    source: String,
    file: String,
    byte_start: usize,
    byte_end: usize,
    macro_kind: String,
    reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageUniverse<'a> {
    schema_version: u8,
    analysis_unit: &'static str,
    styles: &'a [UniverseStyle],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SchemaProbe {
    schema_version: u8,
}

struct ClassifiedStyle {
    bundle_id: String,
    style_id: String,
    class_name: String,
    static_reachability: StaticReachability,
    origins: Vec<UsageOrigin>,
}

/// Parses and canonicalizes one closed schema-1 usage-observation document.
///
/// # Errors
///
/// Returns an error for malformed JSON, unknown fields, invalid identifiers, duplicates, unsafe
/// text, or defensive-limit violations.
pub fn parse_usage_observation(source: &[u8]) -> Result<UsageObservationDocument, String> {
    require_observation(source.len() <= MAX_USAGE_DOCUMENT_BYTES)?;
    let mut document: UsageObservationDocument =
        serde_json::from_slice(source).map_err(|error| error.to_string())?;
    document.validate_and_canonicalize()?;
    Ok(document)
}

/// Builds one canonical closed schema-1 usage-observation sidecar.
///
/// # Errors
///
/// Returns an error for invalid input, defensive-limit overflow, or serialization failure.
pub fn build_usage_observation(input: UsageObservationInput) -> Result<Vec<u8>, String> {
    let mut document = UsageObservationDocument {
        schema_version: 1,
        universe_sha256: input.universe_sha256,
        reachability_sha256: input.reachability_sha256,
        coverage: input.coverage,
        producer: ObservationProducer {
            name: input.producer_name,
            version: input.producer_version,
        },
        contexts: ObservationContexts {
            routes: input.contexts.routes,
            islands: input.contexts.islands,
            themes: input.contexts.themes,
            browsers: input.contexts.browsers,
            viewports: input.contexts.viewports,
            states: input.contexts.states,
        },
        unknown_dynamic_inputs: input.unknown_dynamic_inputs,
        observed_styles: input
            .observed_styles
            .into_iter()
            .map(|style| ObservedStyle {
                bundle_id: style.bundle_id,
                style_id: style.style_id,
            })
            .collect(),
    };
    document.validate_and_canonicalize()?;
    serialize_pretty(&document, "usage observation", INVALID_OBSERVATION)
}

/// Parses and canonicalizes one closed schema-1 usage-retention document.
///
/// # Errors
///
/// Returns an error for malformed JSON, unknown fields, invalid entries, duplicates, an empty
/// policy, or defensive-limit violations.
pub fn parse_usage_retention(source: &[u8]) -> Result<UsageRetentionDocument, String> {
    require_retention(source.len() <= MAX_USAGE_DOCUMENT_BYTES)?;
    let mut document: UsageRetentionDocument =
        serde_json::from_slice(source).map_err(|error| error.to_string())?;
    document.validate_and_canonicalize()?;
    Ok(document)
}

/// Builds one canonical closed schema-1 usage-retention sidecar.
///
/// # Errors
///
/// Returns an error for invalid or duplicate entries, an empty policy, defensive-limit overflow,
/// or serialization failure.
pub fn build_usage_retention(input: UsageRetentionInput) -> Result<Vec<u8>, String> {
    let mut document = UsageRetentionDocument {
        schema_version: 1,
        universe_sha256: input.universe_sha256,
        reachability_sha256: input.reachability_sha256,
        entries: input
            .entries
            .into_iter()
            .map(RetentionEntry::from)
            .collect(),
    };
    document.validate_and_canonicalize()?;
    serialize_pretty(&document, "usage retention", INVALID_RETENTION)
}

/// Parses and structurally validates one closed schema-1 or schema-2 usage-analysis artifact.
///
/// This validates the artifact's closed shape, derived state, canonical arrays, and self-contained
/// universe digest. It cannot authenticate the external evidence named only by input digests. Use
/// [`verify_usage_analysis`] when the exact compiler universe and sidecar bytes are available.
///
/// # Errors
///
/// Returns an error for an unsupported version, malformed JSON, unknown fields, inconsistent
/// derived state, noncanonical arrays, self-contained digest drift, or defensive-limit violations.
pub fn parse_usage_analysis(source: &[u8]) -> Result<UsageAnalysis, String> {
    require_analysis(source.len() <= MAX_USAGE_DOCUMENT_BYTES)?;
    let probe: SchemaProbe = serde_json::from_slice(source).map_err(|error| error.to_string())?;
    match probe.schema_version {
        1 | 2 => {
            let analysis: UsageAnalysis =
                serde_json::from_slice(source).map_err(|error| error.to_string())?;
            analysis.validate_for(probe.schema_version)?;
            Ok(analysis)
        }
        _ => Err("unsupported usage analysis schemaVersion".into()),
    }
}

/// Verifies an analysis artifact against the exact compiler universe and external evidence bytes.
///
/// The expected artifact is rederived through the same preparation path used by the compiler and
/// must match `source` byte for byte. This verifies reachability, observation, and retention input
/// digests as well as every derived state and policy field.
///
/// # Errors
///
/// Returns an error when the artifact is structurally invalid, any supplied input is invalid or
/// stale, or the canonical artifact rederived from those exact inputs differs from `source`.
pub fn verify_usage_analysis(
    source: &[u8],
    styles: &[UsageStyleInput],
    reachability_source: Option<&[u8]>,
    observation_source: Option<&[u8]>,
    retention_source: Option<&[u8]>,
) -> Result<UsageAnalysis, String> {
    let analysis = parse_usage_analysis(source)?;
    let expected = prepare_usage_analysis(
        styles,
        reachability_source,
        observation_source,
        retention_source,
        analysis.rule_selection,
    )?
    .build_analysis()?;
    require_analysis(expected == source)?;
    Ok(analysis)
}

/// Classifies a complete universe once and derives the exact compiler selection and report state.
///
/// `reachability_source`, `observation_source`, and `retention_source` are exact raw bytes. The
/// third rule-selection mode requires both reachability and a non-empty retention sidecar.
///
/// # Errors
///
/// Returns an error for invalid identities or origins, missing evidence, stale sidecar hashes,
/// invalid retention targets, contradictory observation, or defensive-limit overflow.
#[allow(clippy::too_many_lines)]
pub fn prepare_usage_analysis(
    styles: &[UsageStyleInput],
    reachability_source: Option<&[u8]>,
    observation_source: Option<&[u8]>,
    retention_source: Option<&[u8]>,
    rule_selection: AssetRuleSelection,
) -> Result<PreparedUsageAnalysis, String> {
    require_analysis(!styles.is_empty() && styles.len() <= MAX_ITEMS)?;
    match rule_selection {
        AssetRuleSelection::AllCompiled => {
            if retention_source.is_some() {
                return Err("usage retention requires retained-style pruning".into());
            }
        }
        AssetRuleSelection::ReachableStyleIds => {
            if reachability_source.is_none() {
                return Err(
                    "reachable style selection requires exact reachability evidence".into(),
                );
            }
            if retention_source.is_some() {
                return Err("usage retention requires reachable-or-retained selection".into());
            }
        }
        AssetRuleSelection::ReachableOrRetainedStyleIds => {
            if reachability_source.is_none() {
                return Err("usage retention requires exact reachability evidence".into());
            }
            if retention_source.is_none() {
                return Err("reachable-or-retained selection requires usage retention".into());
            }
        }
    }

    let prepared = prepare_styles(styles)?;
    let universe = universe_from_inputs(&prepared);
    let universe_sha256 = universe_digest(&universe)?;
    let reachability = reachability_source
        .map(parse_reachability_document)
        .transpose()?;
    let index = reachability
        .as_ref()
        .map(ReachabilityDocument::source_index);
    let application = reachability_source.map_or(ApplicationEvidence::Unavailable, |source| {
        ApplicationEvidence::Bound {
            coverage: "adapter-attested-complete".into(),
            input_bytes: source.len(),
            input_sha256: sha256_hex(source),
        }
    });

    let classified = prepared
        .into_iter()
        .map(|style| {
            let origins = style
                .origins
                .into_iter()
                .map(|origin| classify_origin(origin, index.as_ref()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ClassifiedStyle {
                bundle_id: style.bundle_id,
                style_id: style.style_id,
                class_name: style.class_name,
                static_reachability: aggregate_reachability(&origins),
                origins,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let states = classified
        .iter()
        .map(|style| {
            (
                (style.bundle_id.clone(), style.style_id.clone()),
                style.static_reachability,
            )
        })
        .collect::<BTreeMap<_, _>>();

    let (mut grants, retention) = if let (Some(source), Some(document)) = (
        retention_source,
        retention_source.map(parse_usage_retention).transpose()?,
    ) {
        if document.universe_sha256 != universe_sha256 {
            return Err("usage retention universeSha256 does not match current universe".into());
        }
        let exact_reachability = reachability_source
            .map(sha256_hex)
            .ok_or_else(|| "usage retention requires exact reachability evidence".to_owned())?;
        if document.reachability_sha256 != exact_reachability {
            return Err(
                "usage retention reachabilitySha256 does not match exact reachability input".into(),
            );
        }
        let entry_count = document.entries.len();
        let mut grants = BTreeMap::new();
        for entry in document.entries {
            let key = (entry.bundle_id.clone(), entry.style_id.clone());
            let state = states.get(&key).ok_or_else(|| {
                format!(
                    "usage retention entry `{}` references unknown style `{}/{}`",
                    entry.id, entry.bundle_id, entry.style_id
                )
            })?;
            if *state != StaticReachability::Unreachable {
                return Err(format!(
                    "usage retention entry `{}` requires a structurally unreachable style",
                    entry.id
                ));
            }
            grants.insert(
                key,
                StyleRetention::Retained {
                    entry_id: entry.id,
                    justification: entry.justification,
                },
            );
        }
        (
            grants,
            Some(RetentionEvidence::Bound {
                input_bytes: source.len(),
                input_sha256: sha256_hex(source),
                entries: entry_count,
            }),
        )
    } else {
        (BTreeMap::new(), None)
    };

    let observation_document = observation_source
        .map(parse_usage_observation)
        .transpose()?;
    if observation_document.is_some() && reachability_source.is_none() {
        return Err("usage observation requires exact reachability evidence".into());
    }
    if let Some(document) = &observation_document {
        if document.universe_sha256 != universe_sha256 {
            return Err("usage observation universeSha256 does not match current universe".into());
        }
        let exact_reachability = reachability_source
            .map(sha256_hex)
            .ok_or_else(|| "usage observation requires exact reachability evidence".to_owned())?;
        if document.reachability_sha256 != exact_reachability {
            return Err(
                "usage observation reachabilitySha256 does not match exact reachability input"
                    .into(),
            );
        }
    }
    let observed = observation_document
        .as_ref()
        .map(|document| {
            document
                .observed_styles
                .iter()
                .map(|style| (style.bundle_id.clone(), style.style_id.clone()))
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    if let Some(key) = observed.iter().find(|key| !states.contains_key(*key)) {
        return Err(format!(
            "usage observation references style `{}/{}` outside the bound universe",
            key.0, key.1
        ));
    }
    let observation = match (observation_source, observation_document.as_ref()) {
        (Some(source), Some(document)) => ObservationEvidence::Bound {
            coverage: document.coverage,
            producer: document.producer.clone(),
            contexts: document.contexts.clone(),
            unknown_dynamic_inputs: document.unknown_dynamic_inputs.clone(),
            input_bytes: source.len(),
            input_sha256: sha256_hex(source),
        },
        _ => ObservationEvidence::Unavailable,
    };

    let mut selected_keys = BTreeMap::<String, BTreeSet<String>>::new();
    let mut retained_keys = BTreeMap::<String, BTreeSet<String>>::new();
    let mut output_styles = Vec::with_capacity(classified.len());
    for style in classified {
        let key = (style.bundle_id.clone(), style.style_id.clone());
        let observation_state = if observation_document.is_none() {
            ObservationState::Unavailable
        } else if observed.contains(&key) {
            ObservationState::Observed
        } else {
            ObservationState::Unobserved
        };
        if style.static_reachability == StaticReachability::Unreachable
            && observation_state == ObservationState::Observed
        {
            return Err(format!(
                "usage observation contradicts unreachable style `{}/{}`",
                style.bundle_id, style.style_id
            ));
        }
        let policy = if rule_selection == AssetRuleSelection::ReachableOrRetainedStyleIds {
            Some(grants.remove(&key).unwrap_or(StyleRetention::NotRetained))
        } else {
            None
        };
        let policy_retained = matches!(policy, Some(StyleRetention::Retained { .. }));
        let selected = match rule_selection {
            AssetRuleSelection::AllCompiled => true,
            AssetRuleSelection::ReachableStyleIds => {
                style.static_reachability == StaticReachability::Reachable
            }
            AssetRuleSelection::ReachableOrRetainedStyleIds => {
                style.static_reachability == StaticReachability::Reachable || policy_retained
            }
        };
        if selected {
            selected_keys
                .entry(key.0.clone())
                .or_default()
                .insert(key.1.clone());
        }
        if policy_retained {
            retained_keys.entry(key.0).or_default().insert(key.1);
        }
        output_styles.push(UsageStyle {
            bundle_id: style.bundle_id,
            style_id: style.style_id,
            class_name: style.class_name,
            selected,
            static_reachability: style.static_reachability,
            observation_state,
            usage_state: derive_usage(style.static_reachability, observation_state),
            retention: policy,
            removal_disposition: derive_removal(
                style.static_reachability,
                selected,
                policy_retained,
                rule_selection,
            )?,
            origins: style.origins,
        });
    }
    require_analysis(grants.is_empty())?;
    Ok(PreparedUsageAnalysis {
        rule_selection,
        universe_sha256,
        application,
        observation,
        retention,
        styles: output_styles,
        selection: UsageSelection {
            selected: selected_keys,
            policy_retained: retained_keys,
        },
    })
}

/// Builds schema-1 analysis bytes for all-compiled or reachable-only selection.
///
/// # Errors
///
/// Returns an error for invalid inputs or when schema-2 retained selection is requested without
/// the explicit preparation API.
pub fn build_usage_analysis(
    styles: &[UsageStyleInput],
    reachability_source: Option<&[u8]>,
    observation_source: Option<&[u8]>,
    rule_selection: AssetRuleSelection,
) -> Result<Vec<u8>, String> {
    prepare_usage_analysis(
        styles,
        reachability_source,
        observation_source,
        None,
        rule_selection,
    )?
    .build_analysis()
}

fn prepare_styles(styles: &[UsageStyleInput]) -> Result<Vec<UsageStyleInput>, String> {
    let mut prepared = styles.to_vec();
    for style in &mut prepared {
        validate_asset_bundle_id(&style.bundle_id)?;
        validate_style_identity(&style.style_id, &style.class_name, INVALID_ANALYSIS)?;
        require_analysis(!style.origins.is_empty())?;
        style.origins.iter().try_for_each(validate_origin_input)?;
        style.origins.sort_by(origin_input_cmp);
        require_analysis(!style.origins.windows(2).any(|pair| pair[0] == pair[1]))?;
    }
    prepared.sort_by(|left, right| {
        (&left.bundle_id, &left.style_id).cmp(&(&right.bundle_id, &right.style_id))
    });
    require_analysis(!prepared.windows(2).any(|pair| {
        pair[0].bundle_id == pair[1].bundle_id && pair[0].style_id == pair[1].style_id
    }))?;
    let origins = prepared.iter().try_fold(0_usize, |count, style| {
        count.checked_add(style.origins.len())
    });
    require_analysis(origins.is_some_and(|count| count <= MAX_ITEMS))?;
    Ok(prepared)
}

fn universe_from_inputs(styles: &[UsageStyleInput]) -> Vec<UniverseStyle> {
    styles
        .iter()
        .map(|style| UniverseStyle {
            bundle_id: style.bundle_id.clone(),
            style_id: style.style_id.clone(),
            class_name: style.class_name.clone(),
            origins: style.origins.iter().map(UniverseOrigin::from).collect(),
        })
        .collect()
}

fn universe_from_styles(styles: &[UsageStyle]) -> Vec<UniverseStyle> {
    styles
        .iter()
        .map(|style| UniverseStyle {
            bundle_id: style.bundle_id.clone(),
            style_id: style.style_id.clone(),
            class_name: style.class_name.clone(),
            origins: style.origins.iter().map(UniverseOrigin::from).collect(),
        })
        .collect()
}

impl From<&UsageOriginInput> for UniverseOrigin {
    fn from(origin: &UsageOriginInput) -> Self {
        Self {
            source: origin.source.clone(),
            file: origin.file.clone(),
            byte_start: origin.byte_start,
            byte_end: origin.byte_end,
            macro_kind: origin.macro_kind.clone(),
            reason: origin.reason.clone(),
        }
    }
}

impl From<&UsageOrigin> for UniverseOrigin {
    fn from(origin: &UsageOrigin) -> Self {
        Self {
            source: origin.source.clone(),
            file: origin.file.clone(),
            byte_start: origin.byte_start,
            byte_end: origin.byte_end,
            macro_kind: origin.macro_kind.clone(),
            reason: origin.reason.clone(),
        }
    }
}

fn universe_digest(styles: &[UniverseStyle]) -> Result<String, String> {
    serde_json::to_vec(&UsageUniverse {
        schema_version: 1,
        analysis_unit: "bundle-style-id",
        styles,
    })
    .map(|bytes| sha256_hex(&bytes))
    .map_err(|error| format!("cannot serialize usage universe: {error}"))
}

fn origin_input_cmp(left: &UsageOriginInput, right: &UsageOriginInput) -> std::cmp::Ordering {
    (
        &left.file,
        left.byte_start,
        left.byte_end,
        &left.macro_kind,
        &left.reason,
        &left.source,
    )
        .cmp(&(
            &right.file,
            right.byte_start,
            right.byte_end,
            &right.macro_kind,
            &right.reason,
            &right.source,
        ))
}

fn classify_origin(
    origin: UsageOriginInput,
    index: Option<&ReachabilityIndex>,
) -> Result<UsageOrigin, String> {
    let (static_reachability, component_ids, route_ids, island_ids) = if let Some(index) = index {
        let evidence = index
            .origin_evidence(&origin.file, origin.byte_start, origin.byte_end)
            .map_err(|error| {
                if error == "origin has no component" {
                    "usage origin has no component".to_owned()
                } else {
                    error
                }
            })?;
        (
            if evidence.is_reachable() {
                StaticReachability::Reachable
            } else {
                StaticReachability::Unreachable
            },
            evidence.component_ids().to_vec(),
            evidence.route_ids().to_vec(),
            evidence.island_ids().to_vec(),
        )
    } else {
        (
            StaticReachability::Unknown,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };
    Ok(UsageOrigin {
        source: origin.source,
        file: origin.file,
        byte_start: origin.byte_start,
        byte_end: origin.byte_end,
        macro_kind: origin.macro_kind,
        reason: origin.reason,
        static_reachability,
        component_ids,
        route_ids,
        island_ids,
    })
}

fn aggregate_reachability(origins: &[UsageOrigin]) -> StaticReachability {
    if origins
        .iter()
        .any(|origin| origin.static_reachability == StaticReachability::Reachable)
    {
        StaticReachability::Reachable
    } else if origins
        .iter()
        .all(|origin| origin.static_reachability == StaticReachability::Unreachable)
    {
        StaticReachability::Unreachable
    } else {
        StaticReachability::Unknown
    }
}

const fn derive_usage(
    reachability: StaticReachability,
    observation: ObservationState,
) -> UsageState {
    match (reachability, observation) {
        (_, ObservationState::Observed) => UsageState::Observed,
        (StaticReachability::Unreachable, _) => UsageState::Dead,
        (_, ObservationState::Unobserved) => UsageState::Unobserved,
        _ => UsageState::Unknown,
    }
}

fn derive_removal(
    reachability: StaticReachability,
    selected: bool,
    policy_retained: bool,
    rule_selection: AssetRuleSelection,
) -> Result<RemovalDisposition, String> {
    match rule_selection {
        AssetRuleSelection::AllCompiled => {
            require_analysis(selected && !policy_retained)?;
            Ok(match reachability {
                StaticReachability::Reachable => RemovalDisposition::Retain,
                StaticReachability::Unreachable => RemovalDisposition::Candidate,
                StaticReachability::Unknown => RemovalDisposition::Blocked,
            })
        }
        AssetRuleSelection::ReachableStyleIds => {
            require_analysis(reachability != StaticReachability::Unknown && !policy_retained)?;
            require_analysis(selected == (reachability == StaticReachability::Reachable))?;
            Ok(if selected {
                RemovalDisposition::Retain
            } else {
                RemovalDisposition::Removed
            })
        }
        AssetRuleSelection::ReachableOrRetainedStyleIds => {
            require_analysis(reachability != StaticReachability::Unknown)?;
            require_analysis(!policy_retained || reachability == StaticReachability::Unreachable)?;
            require_analysis(
                selected == (reachability == StaticReachability::Reachable || policy_retained),
            )?;
            Ok(if policy_retained {
                RemovalDisposition::PolicyRetained
            } else if selected {
                RemovalDisposition::Retain
            } else {
                RemovalDisposition::Removed
            })
        }
    }
}

impl UsageObservationDocument {
    fn validate_and_canonicalize(&mut self) -> Result<(), String> {
        require_observation(self.schema_version == 1)?;
        validate_digest(&self.universe_sha256, INVALID_OBSERVATION)?;
        validate_digest(&self.reachability_sha256, INVALID_OBSERVATION)?;
        validate_text(&self.producer.name, 256, INVALID_OBSERVATION)?;
        validate_text(&self.producer.version, 256, INVALID_OBSERVATION)?;
        for values in [
            &mut self.contexts.routes,
            &mut self.contexts.islands,
            &mut self.contexts.themes,
            &mut self.contexts.browsers,
            &mut self.contexts.viewports,
            &mut self.contexts.states,
            &mut self.unknown_dynamic_inputs,
        ] {
            values
                .iter()
                .try_for_each(|value| validate_text(value, MAX_TEXT_BYTES, INVALID_OBSERVATION))?;
            values.sort();
            require_observation(!values.windows(2).any(|pair| pair[0] == pair[1]))?;
        }
        for style in &self.observed_styles {
            validate_asset_bundle_id(&style.bundle_id)
                .map_err(|_| INVALID_OBSERVATION.to_owned())?;
            validate_style_id_for(&style.style_id, INVALID_OBSERVATION)?;
        }
        self.observed_styles.sort();
        require_observation(
            !self
                .observed_styles
                .windows(2)
                .any(|pair| pair[0] == pair[1]),
        )?;
        let count = [
            self.contexts.routes.len(),
            self.contexts.islands.len(),
            self.contexts.themes.len(),
            self.contexts.browsers.len(),
            self.contexts.viewports.len(),
            self.contexts.states.len(),
            self.unknown_dynamic_inputs.len(),
            self.observed_styles.len(),
        ]
        .into_iter()
        .try_fold(0_usize, usize::checked_add);
        require_observation(count.is_some_and(|count| count <= MAX_ITEMS))
    }
}

impl UsageRetentionDocument {
    fn validate_and_canonicalize(&mut self) -> Result<(), String> {
        require_retention(
            self.schema_version == 1 && !self.entries.is_empty() && self.entries.len() <= MAX_ITEMS,
        )?;
        validate_digest(&self.universe_sha256, INVALID_RETENTION)?;
        validate_digest(&self.reachability_sha256, INVALID_RETENTION)?;
        let mut ids = BTreeSet::new();
        for entry in &self.entries {
            validate_retention_id(&entry.id)?;
            require_retention(ids.insert(entry.id.clone()))?;
            validate_asset_bundle_id(&entry.bundle_id).map_err(|_| INVALID_RETENTION.to_owned())?;
            validate_style_id_for(&entry.style_id, INVALID_RETENTION)?;
            validate_text(&entry.justification, MAX_TEXT_BYTES, INVALID_RETENTION)?;
        }
        self.entries.sort_by(|left, right| {
            (&left.bundle_id, &left.style_id).cmp(&(&right.bundle_id, &right.style_id))
        });
        require_retention(!self.entries.windows(2).any(|pair| {
            pair[0].bundle_id == pair[1].bundle_id && pair[0].style_id == pair[1].style_id
        }))
    }
}

impl UsageAnalysis {
    #[allow(clippy::too_many_lines)]
    fn validate_for(&self, schema_version: u8) -> Result<(), String> {
        require_analysis(
            self.schema_version == schema_version
                && matches!(schema_version, 1 | 2)
                && self.state_model_version == schema_version
                && self.analysis_unit == "bundle-style-id"
                && self.origin_coverage == "compiler-verified-complete"
                && !self.styles.is_empty()
                && self.styles.len() <= MAX_ITEMS,
        )?;
        match schema_version {
            1 => require_analysis(
                self.rule_selection != AssetRuleSelection::ReachableOrRetainedStyleIds
                    && self.retention.is_none()
                    && self.summary.removal_policy_retained.is_none(),
            )?,
            2 => require_analysis(
                self.rule_selection == AssetRuleSelection::ReachableOrRetainedStyleIds
                    && self.retention.is_some()
                    && self.summary.removal_policy_retained.is_some(),
            )?,
            _ => return Err(INVALID_ANALYSIS.into()),
        }
        validate_digest(&self.universe_sha256, INVALID_ANALYSIS)?;
        validate_application(&self.application)?;
        validate_observation(&self.observation)?;
        if let Some(retention) = &self.retention {
            validate_retention_evidence(retention)?;
        }
        let application_available = matches!(&self.application, ApplicationEvidence::Bound { .. });
        let observation_available = matches!(&self.observation, ObservationEvidence::Bound { .. });
        require_analysis(!observation_available || application_available)?;
        if schema_version == 2 {
            require_analysis(application_available)?;
        }

        let origin_count = self.styles.iter().try_fold(0_usize, |count, style| {
            count.checked_add(style.origins.len())
        });
        require_analysis(origin_count.is_some_and(|count| count <= MAX_ITEMS))?;
        let mut previous = None;
        let mut retained_ids = BTreeSet::new();
        let mut retained_count = 0_usize;
        for style in &self.styles {
            validate_asset_bundle_id(&style.bundle_id).map_err(|_| INVALID_ANALYSIS.to_owned())?;
            validate_style_identity(&style.style_id, &style.class_name, INVALID_ANALYSIS)?;
            require_analysis(!style.origins.is_empty())?;
            let key = (&style.bundle_id, &style.style_id);
            require_analysis(previous.is_none_or(|last| last < key))?;
            previous = Some(key);
            require_analysis(style.static_reachability == aggregate_reachability(&style.origins))?;
            require_analysis(
                application_available == (style.static_reachability != StaticReachability::Unknown),
            )?;
            require_analysis(
                observation_available == (style.observation_state != ObservationState::Unavailable),
            )?;
            require_analysis(
                style.usage_state
                    == derive_usage(style.static_reachability, style.observation_state),
            )?;
            if style.static_reachability == StaticReachability::Unreachable {
                require_analysis(style.observation_state != ObservationState::Observed)?;
            }
            let policy_retained = match &style.retention {
                None => {
                    require_analysis(schema_version == 1)?;
                    false
                }
                Some(StyleRetention::NotRetained) => {
                    require_analysis(schema_version == 2)?;
                    false
                }
                Some(StyleRetention::Retained {
                    entry_id,
                    justification,
                }) => {
                    require_analysis(schema_version == 2)?;
                    validate_retention_id_for(entry_id, INVALID_ANALYSIS)?;
                    validate_text(justification, MAX_TEXT_BYTES, INVALID_ANALYSIS)?;
                    require_analysis(retained_ids.insert(entry_id.clone()))?;
                    require_analysis(style.static_reachability == StaticReachability::Unreachable)?;
                    retained_count += 1;
                    true
                }
            };
            require_analysis(
                style.removal_disposition
                    == derive_removal(
                        style.static_reachability,
                        style.selected,
                        policy_retained,
                        self.rule_selection,
                    )?,
            )?;
            require_analysis(
                style
                    .origins
                    .windows(2)
                    .all(|pair| usage_origin_cmp(&pair[0], &pair[1]).is_lt()),
            )?;
            style.origins.iter().try_for_each(validate_usage_origin)?;
        }
        match &self.retention {
            Some(RetentionEvidence::Bound { entries, .. }) => {
                require_analysis(retained_count == *entries)?;
            }
            None => require_analysis(retained_count == 0)?,
        }
        let universe = universe_from_styles(&self.styles);
        require_analysis(universe_digest(&universe)? == self.universe_sha256)?;
        require_analysis(self.summary == summarize(&self.styles, schema_version == 2))
    }
}

fn validate_application(evidence: &ApplicationEvidence) -> Result<(), String> {
    match evidence {
        ApplicationEvidence::Unavailable => Ok(()),
        ApplicationEvidence::Bound {
            coverage,
            input_bytes,
            input_sha256,
        } => {
            require_analysis(
                coverage == "adapter-attested-complete"
                    && *input_bytes > 0
                    && *input_bytes <= MAX_DOCUMENT_BYTES,
            )?;
            validate_digest(input_sha256, INVALID_ANALYSIS)
        }
    }
}

fn validate_observation(evidence: &ObservationEvidence) -> Result<(), String> {
    match evidence {
        ObservationEvidence::Unavailable => Ok(()),
        ObservationEvidence::Bound {
            producer,
            contexts,
            unknown_dynamic_inputs,
            input_bytes,
            input_sha256,
            ..
        } => {
            require_analysis(*input_bytes > 0 && *input_bytes <= MAX_USAGE_DOCUMENT_BYTES)?;
            validate_digest(input_sha256, INVALID_ANALYSIS)?;
            validate_text(&producer.name, 256, INVALID_ANALYSIS)?;
            validate_text(&producer.version, 256, INVALID_ANALYSIS)?;
            let item_count = [
                contexts.routes.len(),
                contexts.islands.len(),
                contexts.themes.len(),
                contexts.browsers.len(),
                contexts.viewports.len(),
                contexts.states.len(),
                unknown_dynamic_inputs.len(),
            ]
            .into_iter()
            .try_fold(0_usize, usize::checked_add);
            require_analysis(item_count.is_some_and(|count| count <= MAX_ITEMS))?;
            for values in [
                &contexts.routes,
                &contexts.islands,
                &contexts.themes,
                &contexts.browsers,
                &contexts.viewports,
                &contexts.states,
                unknown_dynamic_inputs,
            ] {
                validate_sorted_unique(values)?;
            }
            Ok(())
        }
    }
}

fn validate_retention_evidence(evidence: &RetentionEvidence) -> Result<(), String> {
    let RetentionEvidence::Bound {
        input_bytes,
        input_sha256,
        entries,
    } = evidence;
    require_analysis(
        *input_bytes > 0
            && *input_bytes <= MAX_USAGE_DOCUMENT_BYTES
            && *entries > 0
            && *entries <= MAX_ITEMS,
    )?;
    validate_digest(input_sha256, INVALID_ANALYSIS)
}

fn validate_usage_origin(origin: &UsageOrigin) -> Result<(), String> {
    validate_path(&origin.file)?;
    require_analysis(origin.byte_start < origin.byte_end)?;
    validate_utility_source(&origin.source)?;
    validate_text(&origin.macro_kind, MAX_TEXT_BYTES, INVALID_ANALYSIS)?;
    validate_text(&origin.reason, MAX_TEXT_BYTES, INVALID_ANALYSIS)?;
    let item_count = [
        origin.component_ids.len(),
        origin.route_ids.len(),
        origin.island_ids.len(),
    ]
    .into_iter()
    .try_fold(0_usize, usize::checked_add);
    require_analysis(item_count.is_some_and(|count| count <= MAX_ITEMS))?;
    validate_sorted_unique(&origin.component_ids)?;
    validate_sorted_unique(&origin.route_ids)?;
    validate_sorted_unique(&origin.island_ids)?;
    match origin.static_reachability {
        StaticReachability::Reachable => require_analysis(
            !origin.component_ids.is_empty()
                && (!origin.route_ids.is_empty() || !origin.island_ids.is_empty()),
        ),
        StaticReachability::Unreachable => require_analysis(
            !origin.component_ids.is_empty()
                && origin.route_ids.is_empty()
                && origin.island_ids.is_empty(),
        ),
        StaticReachability::Unknown => require_analysis(
            origin.component_ids.is_empty()
                && origin.route_ids.is_empty()
                && origin.island_ids.is_empty(),
        ),
    }
}

fn usage_origin_cmp(left: &UsageOrigin, right: &UsageOrigin) -> std::cmp::Ordering {
    (
        &left.file,
        left.byte_start,
        left.byte_end,
        &left.macro_kind,
        &left.reason,
        &left.source,
    )
        .cmp(&(
            &right.file,
            right.byte_start,
            right.byte_end,
            &right.macro_kind,
            &right.reason,
            &right.source,
        ))
}

fn summarize(styles: &[UsageStyle], schema_2: bool) -> UsageSummary {
    let mut summary = UsageSummary {
        entries: styles.len(),
        removal_policy_retained: schema_2.then_some(0),
        ..UsageSummary::default()
    };
    for style in styles {
        summary.selected += usize::from(style.selected);
        match style.static_reachability {
            StaticReachability::Reachable => summary.static_reachable += 1,
            StaticReachability::Unreachable => summary.static_unreachable += 1,
            StaticReachability::Unknown => summary.static_unknown += 1,
        }
        match style.observation_state {
            ObservationState::Observed => summary.observation_observed += 1,
            ObservationState::Unobserved => summary.observation_unobserved += 1,
            ObservationState::Unavailable => summary.observation_unavailable += 1,
        }
        match style.usage_state {
            UsageState::Observed => summary.usage_observed += 1,
            UsageState::Unobserved => summary.usage_unobserved += 1,
            UsageState::Dead => summary.usage_dead += 1,
            UsageState::Unknown => summary.usage_unknown += 1,
        }
        match style.removal_disposition {
            RemovalDisposition::Retain => summary.removal_retain += 1,
            RemovalDisposition::Blocked => summary.removal_blocked += 1,
            RemovalDisposition::Candidate => summary.removal_candidate += 1,
            RemovalDisposition::Removed => summary.removal_removed += 1,
            RemovalDisposition::PolicyRetained => {
                *summary
                    .removal_policy_retained
                    .as_mut()
                    .expect("schema 2 summary has policy count") += 1;
            }
        }
    }
    summary
}

fn serialize_pretty<T: Serialize>(
    value: &T,
    label: &str,
    size_error: &str,
) -> Result<Vec<u8>, String> {
    let mut output = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("cannot serialize {label}: {error}"))?;
    output.push(b'\n');
    require_for(output.len() <= MAX_USAGE_DOCUMENT_BYTES, size_error)?;
    Ok(output)
}

fn validate_origin_input(origin: &UsageOriginInput) -> Result<(), String> {
    validate_path(&origin.file)?;
    require_analysis(origin.byte_start < origin.byte_end)?;
    validate_utility_source(&origin.source)?;
    validate_text(&origin.macro_kind, MAX_TEXT_BYTES, INVALID_ANALYSIS)?;
    validate_text(&origin.reason, MAX_TEXT_BYTES, INVALID_ANALYSIS)
}

fn validate_path(path: &str) -> Result<(), String> {
    validate_text(path, MAX_TEXT_BYTES, INVALID_ANALYSIS)?;
    require_analysis(
        !(path.starts_with('/')
            || path.contains('\\')
            || path.split('/').any(|segment| !portable_segment(segment))),
    )
}

fn portable_segment(segment: &str) -> bool {
    if segment.is_empty()
        || matches!(segment, "." | "..")
        || segment.ends_with('.')
        || segment.ends_with(' ')
        || segment.bytes().any(|byte| b"<>:\"|?*".contains(&byte))
    {
        return false;
    }
    let stem = segment.split('.').next().unwrap_or(segment);
    if ["con", "prn", "aux", "nul"]
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return false;
    }
    let prefix = stem.get(..3).is_some_and(|prefix| {
        prefix.eq_ignore_ascii_case("com") || prefix.eq_ignore_ascii_case("lpt")
    });
    let port = stem.get(3..).is_some_and(|suffix| {
        (suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
            || matches!(suffix, "¹" | "²" | "³")
    });
    !(prefix && port)
}

fn validate_style_identity(style_id: &str, class_name: &str, error: &str) -> Result<(), String> {
    validate_style_id_for(style_id, error)?;
    let value = u128::from_str_radix(style_id, 16).map_err(|_| error.to_owned())?;
    require_for(StyleId::new(value).to_class_name() == class_name, error)
}

fn validate_style_id_for(value: &str, error: &str) -> Result<(), String> {
    require_for(
        value.len() == 32
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')),
        error,
    )
}

fn validate_retention_id(value: &str) -> Result<(), String> {
    validate_retention_id_for(value, INVALID_RETENTION)
}

fn validate_retention_id_for(value: &str, error: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    require_for(
        !bytes.is_empty()
            && bytes.len() <= MAX_RETENTION_ID_BYTES
            && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
            && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
            && !value.contains("--")
            && bytes
                .iter()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-'),
        error,
    )
}

fn validate_digest(value: &str, error: &str) -> Result<(), String> {
    require_for(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')),
        error,
    )
}

fn validate_text(value: &str, maximum: usize, error: &str) -> Result<(), String> {
    require_for(
        !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control),
        error,
    )
}

fn validate_utility_source(value: &str) -> Result<(), String> {
    require_analysis(
        !value.is_empty()
            && value.len() <= MAX_SOURCE_BYTES
            && !value.chars().any(|character| {
                character.is_control() && !matches!(character, '\t' | '\n' | '\r')
            }),
    )
}

fn validate_sorted_unique(values: &[String]) -> Result<(), String> {
    values
        .iter()
        .try_for_each(|value| validate_text(value, MAX_TEXT_BYTES, INVALID_ANALYSIS))?;
    require_analysis(values.windows(2).all(|pair| pair[0] < pair[1]))
}

fn require_analysis(condition: bool) -> Result<(), String> {
    require_for(condition, INVALID_ANALYSIS)
}

fn require_observation(condition: bool) -> Result<(), String> {
    require_for(condition, INVALID_OBSERVATION)
}

fn require_retention(condition: bool) -> Result<(), String> {
    require_for(condition, INVALID_RETENTION)
}

fn require_for(condition: bool, error: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| error.to_owned())
}
