//! Versioned CSS budget policy and deterministic evaluator.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
#[cfg(feature = "css-analysis")]
use sha2::{Digest, Sha256};

#[cfg(feature = "css-analysis")]
use lightningcss::rules::{CssRule, CssRuleList, Location};
#[cfg(feature = "css-analysis")]
use lightningcss::stylesheet::{PrinterOptions, StyleSheet};
#[cfg(feature = "css-analysis")]
use lightningcss::traits::ToCss;

const MAX_POLICY_BYTES: usize = 1024 * 1024;
const MAX_ITEMS: usize = 4096;

/// Error returned when a budget policy or observation violates its closed contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetError {
    reason: String,
}

impl BudgetError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl fmt::Display for BudgetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::error::Error for BudgetError {}

/// Stable ownership dimension to which one CSS artifact is attributed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BudgetSubjectKind {
    /// One portable project-relative CSS file.
    File,
    /// One explicitly attested package.
    Package,
    /// One explicitly attested route.
    Route,
    /// One detected named layer or the aggregate of anonymous cascade layers.
    Layer,
}

impl BudgetSubjectKind {
    /// Return the stable wire identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Package => "package",
            Self::Route => "route",
            Self::Layer => "layer",
        }
    }
}

/// Canonical budget subject.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BudgetSubject {
    kind: BudgetSubjectKind,
    id: String,
}

impl BudgetSubject {
    /// Construct and validate a subject.
    ///
    /// # Errors
    ///
    /// Returns [`BudgetError`] for an unsafe or noncanonical identifier.
    pub fn new(kind: BudgetSubjectKind, id: impl Into<String>) -> Result<Self, BudgetError> {
        let subject = Self {
            kind,
            id: id.into(),
        };
        subject.validate()?;
        Ok(subject)
    }

    /// Return the ownership dimension.
    #[must_use]
    pub const fn kind(&self) -> BudgetSubjectKind {
        self.kind
    }

    /// Return the stable subject identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    fn validate(&self) -> Result<(), BudgetError> {
        match self.kind {
            BudgetSubjectKind::File => validate_logical_path("budget subject file", &self.id),
            BudgetSubjectKind::Package => validate_package_id(&self.id),
            BudgetSubjectKind::Route => validate_route_id(&self.id),
            BudgetSubjectKind::Layer => validate_layer_id(&self.id),
        }
    }
}

/// Budget metric identifiers shared by limits, exceptions, and findings.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BudgetMetric {
    /// Canonical minified CSS bytes.
    Bytes,
    /// Recursive CSS rule count.
    Rules,
    /// Selector count.
    Selectors,
    /// Highest selector specificity tuple.
    Specificity,
    /// Additional occurrences of normalized declaration blocks.
    SemanticDuplicates,
}

impl BudgetMetric {
    /// Return the stable wire identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bytes => "bytes",
            Self::Rules => "rules",
            Self::Selectors => "selectors",
            Self::Specificity => "specificity",
            Self::SemanticDuplicates => "semantic-duplicates",
        }
    }

    /// Return the stable evidence unit.
    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Self::Bytes => "bytes",
            Self::Rules | Self::SemanticDuplicates => "rules",
            Self::Selectors => "selectors",
            Self::Specificity => "specificity",
        }
    }
}

/// CSS specificity represented as the standard `(id, class, type)` tuple.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CssSpecificity {
    ids: u16,
    classes: u16,
    types: u16,
}

impl CssSpecificity {
    /// Construct a tuple. Each component is bounded to Lightning CSS' 10-bit representation.
    ///
    /// # Errors
    ///
    /// Returns [`BudgetError`] if a component exceeds 1,023.
    pub fn new(ids: u16, classes: u16, types: u16) -> Result<Self, BudgetError> {
        if ids > 1023 || classes > 1023 || types > 1023 {
            return Err(BudgetError::new(
                "specificity components must each be between 0 and 1023",
            ));
        }
        Ok(Self {
            ids,
            classes,
            types,
        })
    }

    /// Decode Lightning CSS' packed specificity value.
    #[must_use]
    pub fn from_packed(value: u32) -> Self {
        Self {
            ids: u16::try_from(value >> 20).unwrap_or(1023),
            classes: u16::try_from((value >> 10) & 0x3ff).unwrap_or(1023),
            types: u16::try_from(value & 0x3ff).unwrap_or(1023),
        }
    }

    fn checked_add(self, increase: Self) -> Self {
        Self {
            ids: self.ids.saturating_add(increase.ids).min(1023),
            classes: self.classes.saturating_add(increase.classes).min(1023),
            types: self.types.saturating_add(increase.types).min(1023),
        }
    }

    fn signed_delta(self, baseline: Self) -> String {
        format!(
            "{:+},{:+},{:+}",
            i32::from(self.ids) - i32::from(baseline.ids),
            i32::from(self.classes) - i32::from(baseline.classes),
            i32::from(self.types) - i32::from(baseline.types)
        )
    }
}

impl fmt::Display for CssSpecificity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{},{},{}", self.ids, self.classes, self.types)
    }
}

impl Serialize for CssSpecificity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for CssSpecificity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        parse_specificity(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CountLimit {
    maximum: u64,
    baseline: Option<u64>,
    max_increase: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SpecificityLimit {
    maximum: CssSpecificity,
    baseline: Option<CssSpecificity>,
    max_increase: Option<CssSpecificity>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BudgetLimits {
    bytes: Option<CountLimit>,
    rules: Option<CountLimit>,
    selectors: Option<CountLimit>,
    specificity: Option<SpecificityLimit>,
    semantic_duplicates: Option<CountLimit>,
}

impl BudgetLimits {
    fn metrics(&self) -> impl Iterator<Item = BudgetMetric> + '_ {
        [
            (BudgetMetric::Bytes, self.bytes.is_some()),
            (BudgetMetric::Rules, self.rules.is_some()),
            (BudgetMetric::Selectors, self.selectors.is_some()),
            (BudgetMetric::Specificity, self.specificity.is_some()),
            (
                BudgetMetric::SemanticDuplicates,
                self.semantic_duplicates.is_some(),
            ),
        ]
        .into_iter()
        .filter_map(|(metric, present)| present.then_some(metric))
    }

    fn validate(&self, budget_id: &str) -> Result<(), BudgetError> {
        if self.metrics().next().is_none() {
            return Err(BudgetError::new(format!(
                "budget `{budget_id}` must configure at least one limit"
            )));
        }
        for (metric, limit) in [
            (BudgetMetric::Bytes, self.bytes.as_ref()),
            (BudgetMetric::Rules, self.rules.as_ref()),
            (BudgetMetric::Selectors, self.selectors.as_ref()),
            (
                BudgetMetric::SemanticDuplicates,
                self.semantic_duplicates.as_ref(),
            ),
        ] {
            if limit.is_some_and(|limit| limit.max_increase.is_some() && limit.baseline.is_none()) {
                return Err(BudgetError::new(format!(
                    "budget `{budget_id}` metric `{}` requires `baseline` when `maxIncrease` is set",
                    metric.as_str()
                )));
            }
        }
        if self
            .specificity
            .as_ref()
            .is_some_and(|limit| limit.max_increase.is_some() && limit.baseline.is_none())
        {
            return Err(BudgetError::new(format!(
                "budget `{budget_id}` metric `specificity` requires `baseline` when `maxIncrease` is set"
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BudgetDefinition {
    id: String,
    subject: BudgetSubject,
    limits: BudgetLimits,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BudgetException {
    id: String,
    budget: String,
    metrics: Vec<BudgetMetric>,
    justification: String,
}

/// Closed and canonical budget policy schema 1.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BudgetPolicy {
    schema_version: u8,
    policy_version: u8,
    budgets: Vec<BudgetDefinition>,
    #[serde(default)]
    exceptions: Vec<BudgetException>,
}

impl BudgetPolicy {
    /// Serialize the semantic policy in canonical order with one trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`BudgetError`] if serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, BudgetError> {
        self.validate()?;
        let mut output = serde_json::to_string_pretty(self).map_err(|error| {
            BudgetError::new(format!("cannot serialize budget policy: {error}"))
        })?;
        output.push('\n');
        Ok(output)
    }

    /// Return the policy decision version.
    #[must_use]
    pub const fn policy_version(&self) -> u8 {
        self.policy_version
    }

    /// Return whether any budget targets the requested ownership dimension.
    #[must_use]
    pub fn uses_subject_kind(&self, kind: BudgetSubjectKind) -> bool {
        self.budgets
            .iter()
            .any(|budget| budget.subject.kind == kind)
    }

    /// Return the number of declared budget subjects that must be observed.
    #[must_use]
    pub fn budget_count(&self) -> usize {
        self.budgets.len()
    }

    fn canonicalize(&mut self) {
        self.budgets
            .sort_by(|left, right| (&left.subject, &left.id).cmp(&(&right.subject, &right.id)));
        for exception in &mut self.exceptions {
            exception.metrics.sort_unstable();
        }
        self.exceptions
            .sort_by(|left, right| left.id.cmp(&right.id));
    }

    fn validate(&self) -> Result<(), BudgetError> {
        if self.schema_version != 1 {
            return Err(BudgetError::new(format!(
                "unsupported budget schema `{}`; expected `1`",
                self.schema_version
            )));
        }
        if self.policy_version != 1 {
            return Err(BudgetError::new(format!(
                "unsupported budget policy version `{}`; expected `1`",
                self.policy_version
            )));
        }
        if self.budgets.is_empty() || self.budgets.len() > MAX_ITEMS {
            return Err(BudgetError::new(
                "budget policy must contain between 1 and 4,096 budgets",
            ));
        }
        if self.exceptions.len() > MAX_ITEMS {
            return Err(BudgetError::new(
                "budget policy cannot contain more than 4,096 exceptions",
            ));
        }
        validate_budget_definitions(&self.budgets)?;
        validate_exceptions(&self.budgets, &self.exceptions)
    }
}

/// Parse, validate, and canonicalize budget policy JSON.
///
/// # Errors
///
/// Returns [`BudgetError`] for oversized/malformed JSON, unknown fields, unsupported versions,
/// invalid limits/subjects/exceptions, or ambiguous duplicate ownership.
pub fn parse_budget_policy(source: &[u8]) -> Result<BudgetPolicy, BudgetError> {
    if source.len() > MAX_POLICY_BYTES {
        return Err(BudgetError::new("budget policy exceeds 1 MiB"));
    }
    let mut policy: BudgetPolicy = serde_json::from_slice(source)
        .map_err(|error| BudgetError::new(format!("invalid budget policy JSON: {error}")))?;
    policy.canonicalize();
    policy.validate()?;
    Ok(policy)
}

/// Measurements attributed to one subject.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetMeasurements {
    bytes: u64,
    rules: u64,
    selectors: u64,
    specificity: CssSpecificity,
    semantic_duplicates: u64,
}

impl BudgetMeasurements {
    /// Construct a complete measurement vector.
    #[must_use]
    pub const fn new(
        bytes: u64,
        rules: u64,
        selectors: u64,
        specificity: CssSpecificity,
        semantic_duplicates: u64,
    ) -> Self {
        Self {
            bytes,
            rules,
            selectors,
            specificity,
            semantic_duplicates,
        }
    }
}

/// One subject and its measured CSS artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetObservation {
    subject: BudgetSubject,
    measurements: BudgetMeasurements,
}

impl BudgetObservation {
    /// Construct an observation from a validated subject and complete metrics.
    #[must_use]
    pub const fn new(subject: BudgetSubject, measurements: BudgetMeasurements) -> Self {
        Self {
            subject,
            measurements,
        }
    }

    /// Return the attributed subject.
    #[must_use]
    pub const fn subject(&self) -> &BudgetSubject {
        &self.subject
    }
}

/// Reviewed exception attached to an exceeded metric.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedBudgetException {
    /// Stable exception ID.
    pub id: String,
    /// Human review justification.
    pub justification: String,
}

/// Deterministic evaluation of one configured metric.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetEvaluation {
    /// Stable budget ID.
    pub budget_id: String,
    /// Matched ownership subject.
    pub subject: BudgetSubject,
    /// Evaluated metric.
    pub metric: BudgetMetric,
    /// Current measurement rendered in its canonical unit.
    pub actual: String,
    /// Absolute maximum rendered in its canonical unit.
    pub maximum: String,
    /// Optional previous measurement.
    pub baseline: Option<String>,
    /// Signed current-minus-baseline delta.
    pub delta: Option<String>,
    /// Optional permitted positive regression.
    pub max_increase: Option<String>,
    /// Whether the absolute maximum was exceeded.
    pub maximum_exceeded: bool,
    /// Whether the permitted regression delta was exceeded.
    pub delta_exceeded: bool,
    /// Reviewed exception, present only for an exceeded metric.
    pub exception: Option<AppliedBudgetException>,
}

impl BudgetEvaluation {
    /// Return whether either configured enforcement boundary was exceeded.
    #[must_use]
    pub const fn violated(&self) -> bool {
        self.maximum_exceeded || self.delta_exceeded
    }

    /// Return whether the violation remains enforced after reviewed exceptions.
    #[must_use]
    pub const fn enforced(&self) -> bool {
        self.violated() && self.exception.is_none()
    }
}

/// Evaluations plus policy-match coverage for one audit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetEvaluationReport {
    evaluations: Vec<BudgetEvaluation>,
    matched_budgets: usize,
}

impl BudgetEvaluationReport {
    /// Return the evaluations in policy/metric canonical order.
    #[must_use]
    pub fn evaluations(&self) -> &[BudgetEvaluation] {
        &self.evaluations
    }

    /// Return how many budget definitions matched observed subjects.
    #[must_use]
    pub const fn matched_budgets(&self) -> usize {
        self.matched_budgets
    }

    /// Return whether an unexcepted limit failed.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.evaluations.iter().any(BudgetEvaluation::enforced)
    }
}

/// Evaluate a validated policy against uniquely attributed observations.
///
/// # Errors
///
/// Returns [`BudgetError`] if the same subject is observed more than once.
pub fn evaluate_budget_policy(
    policy: &BudgetPolicy,
    observations: &[BudgetObservation],
) -> Result<BudgetEvaluationReport, BudgetError> {
    policy.validate()?;
    let mut observed = BTreeMap::new();
    for observation in observations {
        if observed
            .insert(&observation.subject, observation.measurements)
            .is_some()
        {
            return Err(BudgetError::new(format!(
                "budget subject `{}:{}` was observed more than once",
                observation.subject.kind.as_str(),
                observation.subject.id
            )));
        }
    }
    let exceptions = exception_index(&policy.exceptions);
    let mut evaluations = Vec::new();
    let mut matched_budgets = 0;
    for budget in &policy.budgets {
        let Some(measurements) = observed.get(&budget.subject) else {
            continue;
        };
        matched_budgets += 1;
        evaluate_definition(budget, *measurements, &exceptions, &mut evaluations);
    }
    Ok(BudgetEvaluationReport {
        evaluations,
        matched_budgets,
    })
}

/// Complete CSS budget measurements produced from one parsed stylesheet.
#[cfg(feature = "css-analysis")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssBudgetInventory {
    file: CssBudgetMetrics,
    layers: Vec<CssLayerBudgetMetrics>,
}

#[cfg(feature = "css-analysis")]
impl CssBudgetInventory {
    /// Return the measurements for the complete stylesheet.
    #[must_use]
    pub const fn file(&self) -> &CssBudgetMetrics {
        &self.file
    }

    /// Return named layer measurements and anonymous-layer aggregates in canonical order.
    #[must_use]
    pub fn layers(&self) -> &[CssLayerBudgetMetrics] {
        &self.layers
    }
}

/// Canonical CSS measurements and normalized declaration fingerprints.
#[cfg(feature = "css-analysis")]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CssBudgetMetrics {
    canonical_bytes: usize,
    rules: usize,
    selectors: usize,
    declarations: usize,
    max_specificity: u32,
    declaration_fingerprints: BTreeMap<String, usize>,
    declaration_identities: BTreeMap<String, usize>,
}

#[cfg(feature = "css-analysis")]
impl CssBudgetMetrics {
    /// Return canonical minified CSS bytes.
    #[must_use]
    pub const fn canonical_bytes(&self) -> usize {
        self.canonical_bytes
    }

    /// Return the recursive CSS rule count.
    #[must_use]
    pub const fn rules(&self) -> usize {
        self.rules
    }

    /// Return the selector count.
    #[must_use]
    pub const fn selectors(&self) -> usize {
        self.selectors
    }

    /// Return the normal plus important declaration count.
    #[must_use]
    pub const fn declarations(&self) -> usize {
        self.declarations
    }

    /// Return maximum packed selector specificity.
    #[must_use]
    pub const fn max_specificity(&self) -> u32 {
        self.max_specificity
    }

    /// Return additional normalized declaration-block occurrences after each first occurrence.
    #[must_use]
    pub fn semantic_duplicates(&self) -> usize {
        self.declaration_fingerprints
            .values()
            .map(|occurrences| occurrences.saturating_sub(1))
            .sum()
    }

    /// Return how many declaration fingerprints occur more than once.
    #[must_use]
    pub fn semantic_duplicate_groups(&self) -> usize {
        self.declaration_fingerprints
            .values()
            .filter(|occurrences| **occurrences > 1)
            .count()
    }

    /// Iterate over duplicate fingerprints and their occurrence counts in hash order.
    pub fn duplicate_fingerprints(&self) -> impl Iterator<Item = (&str, usize)> {
        self.declaration_fingerprints
            .iter()
            .filter(|(_, occurrences)| **occurrences > 1)
            .map(|(fingerprint, occurrences)| (fingerprint.as_str(), *occurrences))
    }

    /// Iterate over generic-CSS declaration identities and occurrence counts in hash order.
    ///
    /// Each identity binds conditional/layer context, selector list, importance, and canonical
    /// declaration bytes. It identifies authored declarations without implying runtime usage.
    pub fn declaration_identities(&self) -> impl Iterator<Item = (&str, usize)> {
        self.declaration_identities
            .iter()
            .map(|(identity, occurrences)| (identity.as_str(), *occurrences))
    }

    /// Convert the AST metrics into the backend-independent policy vector.
    #[must_use]
    pub fn measurements(&self) -> BudgetMeasurements {
        BudgetMeasurements::new(
            usize_to_u64(self.canonical_bytes),
            usize_to_u64(self.rules),
            usize_to_u64(self.selectors),
            CssSpecificity::from_packed(self.max_specificity),
            usize_to_u64(self.semantic_duplicates()),
        )
    }

    /// Merge another independently parsed CSS artifact into this aggregate.
    ///
    /// Canonical bytes, rules, selectors, and fingerprint occurrences are added; specificity uses
    /// the lexicographic maximum. Fingerprints are merged before duplicate counts are derived, so
    /// equivalent declaration blocks in separate bundles are detected.
    ///
    /// # Errors
    ///
    /// Returns [`BudgetError`] if any aggregate counter would overflow the host representation.
    pub fn try_merge(&mut self, other: &Self) -> Result<(), BudgetError> {
        let mut merged = self.clone();
        merged.canonical_bytes = checked_metric_add(
            merged.canonical_bytes,
            other.canonical_bytes,
            "canonical bytes",
        )?;
        merged.rules = checked_metric_add(merged.rules, other.rules, "rules")?;
        merged.selectors = checked_metric_add(merged.selectors, other.selectors, "selectors")?;
        merged.declarations =
            checked_metric_add(merged.declarations, other.declarations, "declarations")?;
        merged.max_specificity = merged.max_specificity.max(other.max_specificity);
        for (fingerprint, occurrences) in &other.declaration_fingerprints {
            let current = merged
                .declaration_fingerprints
                .entry(fingerprint.clone())
                .or_default();
            *current = checked_metric_add(*current, *occurrences, "fingerprint occurrences")?;
        }
        for (identity, occurrences) in &other.declaration_identities {
            let current = merged
                .declaration_identities
                .entry(identity.clone())
                .or_default();
            *current =
                checked_metric_add(*current, *occurrences, "declaration identity occurrences")?;
        }
        *self = merged;
        Ok(())
    }
}

/// Metrics for one canonical full cascade-layer name.
#[cfg(feature = "css-analysis")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssLayerBudgetMetrics {
    name: String,
    metrics: CssBudgetMetrics,
    first_loc: Location,
}

#[cfg(feature = "css-analysis")]
impl CssLayerBudgetMetrics {
    /// Return the canonical dot-qualified layer name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return aggregate measurements for all blocks with this full name.
    #[must_use]
    pub const fn metrics(&self) -> &CssBudgetMetrics {
        &self.metrics
    }

    /// Return the first source location for exact finding attachment.
    #[must_use]
    pub const fn first_location(&self) -> Location {
        self.first_loc
    }
}

/// Measure one already-parsed stylesheet using canonical Lightning CSS serialization.
///
/// # Errors
///
/// Returns [`BudgetError`] if canonical rule, condition, selector, layer, or declaration
/// serialization fails.
#[cfg(feature = "css-analysis")]
pub fn measure_css_budgets(
    stylesheet: &StyleSheet<'_, '_>,
) -> Result<CssBudgetInventory, BudgetError> {
    let file = budget_metrics_for_stylesheet(stylesheet)?;
    let layers = collect_layer_budget_metrics(&stylesheet.rules)?
        .into_iter()
        .map(|(name, (metrics, first_loc))| CssLayerBudgetMetrics {
            name,
            metrics,
            first_loc,
        })
        .collect();
    Ok(CssBudgetInventory { file, layers })
}

#[cfg(feature = "css-analysis")]
fn budget_metrics_for_stylesheet(
    stylesheet: &StyleSheet<'_, '_>,
) -> Result<CssBudgetMetrics, BudgetError> {
    let mut metrics = CssBudgetMetrics {
        canonical_bytes: stylesheet
            .to_css(PrinterOptions {
                minify: true,
                ..PrinterOptions::default()
            })
            .map_err(|error| {
                BudgetError::new(format!(
                    "cannot serialize canonical CSS budget input: {error}"
                ))
            })?
            .code
            .len(),
        ..CssBudgetMetrics::default()
    };
    collect_budget_rule_counts(&stylesheet.rules, &mut metrics, "")?;
    Ok(metrics)
}

#[cfg(feature = "css-analysis")]
fn budget_metrics_for_rules(rules: &CssRuleList<'_>) -> Result<CssBudgetMetrics, BudgetError> {
    let mut metrics = CssBudgetMetrics {
        canonical_bytes: rules
            .to_css_string(minified_printer())
            .map_err(|error| {
                BudgetError::new(format!(
                    "cannot serialize canonical CSS layer budget: {error}"
                ))
            })?
            .len(),
        ..CssBudgetMetrics::default()
    };
    collect_budget_rule_counts(rules, &mut metrics, "")?;
    Ok(metrics)
}

#[cfg(feature = "css-analysis")]
fn collect_budget_rule_counts(
    rules: &CssRuleList<'_>,
    metrics: &mut CssBudgetMetrics,
    context: &str,
) -> Result<(), BudgetError> {
    for rule in &rules.0 {
        metrics.rules += 1;
        match rule {
            CssRule::Style(rule) => collect_style_budget_metrics(rule, metrics, context)?,
            CssRule::Nesting(rule) => collect_style_budget_metrics(&rule.style, metrics, context)?,
            CssRule::Media(rule) => {
                let nested = extend_budget_context(
                    context,
                    "media",
                    &canonical_css(&rule.query, "media query")?,
                );
                collect_budget_rule_counts(&rule.rules, metrics, &nested)?;
            }
            CssRule::Supports(rule) => {
                let nested = extend_budget_context(
                    context,
                    "supports",
                    &canonical_css(&rule.condition, "supports condition")?,
                );
                collect_budget_rule_counts(&rule.rules, metrics, &nested)?;
            }
            CssRule::MozDocument(rule) => {
                let nested = extend_budget_context(context, "moz-document", "url-prefix()");
                collect_budget_rule_counts(&rule.rules, metrics, &nested)?;
            }
            CssRule::LayerBlock(rule) => {
                let name = rule
                    .name
                    .as_ref()
                    .map(|name| canonical_css(name, "layer name"))
                    .transpose()?
                    .unwrap_or_else(|| "anonymous".into());
                let nested = extend_budget_context(context, "layer", &name);
                collect_budget_rule_counts(&rule.rules, metrics, &nested)?;
            }
            CssRule::Container(rule) => {
                let name = rule
                    .name
                    .as_ref()
                    .map(|name| canonical_css(name, "container name"))
                    .transpose()?
                    .unwrap_or_else(|| "none".into());
                let condition = rule
                    .condition
                    .as_ref()
                    .map(|condition| canonical_css(condition, "container condition"))
                    .transpose()?
                    .unwrap_or_else(|| "none".into());
                let nested =
                    extend_budget_context(context, "container", &format!("{name}|{condition}"));
                collect_budget_rule_counts(&rule.rules, metrics, &nested)?;
            }
            CssRule::Scope(rule) => {
                let start = rule
                    .scope_start
                    .as_ref()
                    .map(|selectors| canonical_css(selectors, "scope start"))
                    .transpose()?
                    .unwrap_or_else(|| "none".into());
                let end = rule
                    .scope_end
                    .as_ref()
                    .map(|selectors| canonical_css(selectors, "scope end"))
                    .transpose()?
                    .unwrap_or_else(|| "none".into());
                let nested = extend_budget_context(context, "scope", &format!("{start}|{end}"));
                collect_budget_rule_counts(&rule.rules, metrics, &nested)?;
            }
            CssRule::StartingStyle(rule) => {
                let nested = extend_budget_context(context, "starting-style", "");
                collect_budget_rule_counts(&rule.rules, metrics, &nested)?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(feature = "css-analysis")]
fn collect_style_budget_metrics(
    rule: &lightningcss::rules::style::StyleRule<'_>,
    metrics: &mut CssBudgetMetrics,
    context: &str,
) -> Result<(), BudgetError> {
    metrics.selectors += rule.selectors.0.len();
    metrics.declarations += rule.declarations.declarations.len();
    metrics.declarations += rule.declarations.important_declarations.len();
    for selector in &rule.selectors.0 {
        metrics.max_specificity = metrics.max_specificity.max(selector.specificity());
    }
    let selectors = canonical_css(&rule.selectors, "selector list")?;
    for (important, declarations) in [
        (false, &rule.declarations.declarations),
        (true, &rule.declarations.important_declarations),
    ] {
        for declaration in declarations {
            let canonical = declaration
                .to_css_string(important, minified_printer())
                .map_err(|error| {
                    BudgetError::new(format!("cannot normalize declaration identity: {error}"))
                })?;
            let input = format!(
                "pliegocss-generic-css-declaration-v1\0{context}\0{selectors}\0{canonical}"
            );
            let identity = format!("sha256:{}", sha256_hex(input.as_bytes()));
            *metrics.declaration_identities.entry(identity).or_default() += 1;
        }
    }
    if !rule.declarations.declarations.is_empty()
        || !rule.declarations.important_declarations.is_empty()
    {
        let normalized = rule
            .declarations
            .to_css_string(minified_printer())
            .map_err(|error| {
                BudgetError::new(format!("cannot normalize declaration block: {error}"))
            })?;
        let fingerprint_input = format!("{context}\0{normalized}");
        let fingerprint = format!("sha256:{}", sha256_hex(fingerprint_input.as_bytes()));
        *metrics
            .declaration_fingerprints
            .entry(fingerprint)
            .or_default() += 1;
    }
    let nested = extend_budget_context(context, "selector-parent", &selectors);
    collect_budget_rule_counts(&rule.rules, metrics, &nested)
}

#[cfg(feature = "css-analysis")]
fn collect_layer_budget_metrics(
    rules: &CssRuleList<'_>,
) -> Result<BTreeMap<String, (CssBudgetMetrics, Location)>, BudgetError> {
    let mut layers = BTreeMap::new();
    discover_layer_budget_metrics(rules, None, &mut layers)?;
    Ok(layers)
}

#[cfg(feature = "css-analysis")]
fn discover_layer_budget_metrics(
    rules: &CssRuleList<'_>,
    parent: Option<&str>,
    layers: &mut BTreeMap<String, (CssBudgetMetrics, Location)>,
) -> Result<(), BudgetError> {
    for rule in &rules.0 {
        match rule {
            CssRule::LayerBlock(rule) => {
                let local = rule
                    .name
                    .as_ref()
                    .map(|name| canonical_css(name, "layer name"))
                    .transpose()?
                    .unwrap_or_else(|| "anonymous".into());
                let full = parent
                    .map(|parent| format!("{parent}.{local}"))
                    .unwrap_or(local);
                let measured = budget_metrics_for_rules(&rule.rules)?;
                if let Some((existing, _)) = layers.get_mut(&full) {
                    existing.try_merge(&measured)?;
                } else {
                    layers.insert(full.clone(), (measured, rule.loc));
                }
                discover_layer_budget_metrics(&rule.rules, Some(&full), layers)?;
            }
            CssRule::Style(rule) => {
                discover_layer_budget_metrics(&rule.rules, parent, layers)?;
            }
            CssRule::Nesting(rule) => {
                discover_layer_budget_metrics(&rule.style.rules, parent, layers)?;
            }
            CssRule::Media(rule) => discover_layer_budget_metrics(&rule.rules, parent, layers)?,
            CssRule::Supports(rule) => discover_layer_budget_metrics(&rule.rules, parent, layers)?,
            CssRule::MozDocument(rule) => {
                discover_layer_budget_metrics(&rule.rules, parent, layers)?;
            }
            CssRule::Container(rule) => {
                discover_layer_budget_metrics(&rule.rules, parent, layers)?;
            }
            CssRule::Scope(rule) => discover_layer_budget_metrics(&rule.rules, parent, layers)?,
            CssRule::StartingStyle(rule) => {
                discover_layer_budget_metrics(&rule.rules, parent, layers)?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(feature = "css-analysis")]
fn canonical_css(value: &impl ToCss, label: &str) -> Result<String, BudgetError> {
    value
        .to_css_string(minified_printer())
        .map_err(|error| BudgetError::new(format!("cannot serialize canonical {label}: {error}")))
}

#[cfg(feature = "css-analysis")]
fn minified_printer() -> PrinterOptions<'static> {
    PrinterOptions {
        minify: true,
        ..PrinterOptions::default()
    }
}

#[cfg(feature = "css-analysis")]
fn extend_budget_context(parent: &str, kind: &str, value: &str) -> String {
    format!("{parent}\0{kind}:{value}")
}

#[cfg(feature = "css-analysis")]
fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        use fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

#[cfg(feature = "css-analysis")]
fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(feature = "css-analysis")]
fn checked_metric_add(left: usize, right: usize, metric: &str) -> Result<usize, BudgetError> {
    left.checked_add(right)
        .ok_or_else(|| BudgetError::new(format!("CSS budget aggregate `{metric}` overflowed")))
}

fn evaluate_definition(
    budget: &BudgetDefinition,
    measurements: BudgetMeasurements,
    exceptions: &BTreeMap<(&str, BudgetMetric), &BudgetException>,
    output: &mut Vec<BudgetEvaluation>,
) {
    for (metric, actual, limit) in [
        (
            BudgetMetric::Bytes,
            measurements.bytes,
            &budget.limits.bytes,
        ),
        (
            BudgetMetric::Rules,
            measurements.rules,
            &budget.limits.rules,
        ),
        (
            BudgetMetric::Selectors,
            measurements.selectors,
            &budget.limits.selectors,
        ),
        (
            BudgetMetric::SemanticDuplicates,
            measurements.semantic_duplicates,
            &budget.limits.semantic_duplicates,
        ),
    ] {
        if let Some(limit) = limit {
            output.push(evaluate_count(budget, metric, actual, limit, exceptions));
        }
    }
    if let Some(limit) = &budget.limits.specificity {
        output.push(evaluate_specificity(
            budget,
            measurements.specificity,
            limit,
            exceptions,
        ));
    }
    output.sort_by(|left, right| {
        (&left.subject, &left.budget_id, left.metric).cmp(&(
            &right.subject,
            &right.budget_id,
            right.metric,
        ))
    });
}

fn evaluate_count(
    budget: &BudgetDefinition,
    metric: BudgetMetric,
    actual: u64,
    limit: &CountLimit,
    exceptions: &BTreeMap<(&str, BudgetMetric), &BudgetException>,
) -> BudgetEvaluation {
    let maximum_exceeded = actual > limit.maximum;
    let delta_exceeded = limit
        .baseline
        .zip(limit.max_increase)
        .is_some_and(|(baseline, increase)| actual > baseline.saturating_add(increase));
    let violated = maximum_exceeded || delta_exceeded;
    BudgetEvaluation {
        budget_id: budget.id.clone(),
        subject: budget.subject.clone(),
        metric,
        actual: actual.to_string(),
        maximum: limit.maximum.to_string(),
        baseline: limit.baseline.map(|value| value.to_string()),
        delta: limit
            .baseline
            .map(|baseline| signed_count_delta(actual, baseline)),
        max_increase: limit.max_increase.map(|value| value.to_string()),
        maximum_exceeded,
        delta_exceeded,
        exception: applied_exception(exceptions, &budget.id, metric, violated),
    }
}

fn evaluate_specificity(
    budget: &BudgetDefinition,
    actual: CssSpecificity,
    limit: &SpecificityLimit,
    exceptions: &BTreeMap<(&str, BudgetMetric), &BudgetException>,
) -> BudgetEvaluation {
    let maximum_exceeded = actual > limit.maximum;
    let delta_exceeded = limit
        .baseline
        .zip(limit.max_increase)
        .is_some_and(|(baseline, increase)| actual > baseline.checked_add(increase));
    let violated = maximum_exceeded || delta_exceeded;
    BudgetEvaluation {
        budget_id: budget.id.clone(),
        subject: budget.subject.clone(),
        metric: BudgetMetric::Specificity,
        actual: actual.to_string(),
        maximum: limit.maximum.to_string(),
        baseline: limit.baseline.map(|value| value.to_string()),
        delta: limit.baseline.map(|baseline| actual.signed_delta(baseline)),
        max_increase: limit.max_increase.map(|value| value.to_string()),
        maximum_exceeded,
        delta_exceeded,
        exception: applied_exception(exceptions, &budget.id, BudgetMetric::Specificity, violated),
    }
}

fn applied_exception(
    exceptions: &BTreeMap<(&str, BudgetMetric), &BudgetException>,
    budget_id: &str,
    metric: BudgetMetric,
    violated: bool,
) -> Option<AppliedBudgetException> {
    violated
        .then(|| exceptions.get(&(budget_id, metric)))
        .flatten()
        .map(|exception| AppliedBudgetException {
            id: exception.id.clone(),
            justification: exception.justification.clone(),
        })
}

fn exception_index(
    exceptions: &[BudgetException],
) -> BTreeMap<(&str, BudgetMetric), &BudgetException> {
    let mut index = BTreeMap::new();
    for exception in exceptions {
        for metric in &exception.metrics {
            index.insert((exception.budget.as_str(), *metric), exception);
        }
    }
    index
}

fn validate_budget_definitions(budgets: &[BudgetDefinition]) -> Result<(), BudgetError> {
    let mut ids = BTreeSet::new();
    let mut subjects = BTreeSet::new();
    for budget in budgets {
        validate_slug("budget id", &budget.id)?;
        budget.subject.validate()?;
        budget.limits.validate(&budget.id)?;
        if !ids.insert(&budget.id) {
            return Err(BudgetError::new(format!(
                "duplicate budget id `{}`",
                budget.id
            )));
        }
        if !subjects.insert(&budget.subject) {
            return Err(BudgetError::new(format!(
                "budget subject `{}:{}` is configured more than once",
                budget.subject.kind.as_str(),
                budget.subject.id
            )));
        }
    }
    Ok(())
}

fn validate_exceptions(
    budgets: &[BudgetDefinition],
    exceptions: &[BudgetException],
) -> Result<(), BudgetError> {
    let definitions = budgets
        .iter()
        .map(|budget| (budget.id.as_str(), &budget.limits))
        .collect::<BTreeMap<_, _>>();
    let mut ids = BTreeSet::new();
    let mut coverage = BTreeSet::new();
    for exception in exceptions {
        validate_slug("budget exception id", &exception.id)?;
        validate_text("budget exception justification", &exception.justification)?;
        if !ids.insert(&exception.id) {
            return Err(BudgetError::new(format!(
                "duplicate budget exception id `{}`",
                exception.id
            )));
        }
        let Some(limits) = definitions.get(exception.budget.as_str()) else {
            return Err(BudgetError::new(format!(
                "budget exception `{}` references unknown budget `{}`",
                exception.id, exception.budget
            )));
        };
        if exception.metrics.is_empty() {
            return Err(BudgetError::new(format!(
                "budget exception `{}` must name at least one metric",
                exception.id
            )));
        }
        for metric in &exception.metrics {
            if !limits.metrics().any(|candidate| candidate == *metric) {
                return Err(BudgetError::new(format!(
                    "budget exception `{}` references unconfigured metric `{}`",
                    exception.id,
                    metric.as_str()
                )));
            }
            if !coverage.insert((exception.budget.as_str(), *metric)) {
                return Err(BudgetError::new(format!(
                    "budget `{}` metric `{}` has more than one exception",
                    exception.budget,
                    metric.as_str()
                )));
            }
        }
    }
    Ok(())
}

fn parse_specificity(value: &str) -> Result<CssSpecificity, BudgetError> {
    let mut parts = value.split(',');
    let parse = |part: Option<&str>| {
        part.ok_or_else(|| BudgetError::new("specificity must use `id,class,type`"))?
            .parse::<u16>()
            .map_err(|_| BudgetError::new("specificity must use `id,class,type` integers"))
    };
    let result = CssSpecificity::new(
        parse(parts.next())?,
        parse(parts.next())?,
        parse(parts.next())?,
    )?;
    if parts.next().is_some() {
        return Err(BudgetError::new("specificity must use `id,class,type`"));
    }
    Ok(result)
}

fn signed_count_delta(actual: u64, baseline: u64) -> String {
    match actual.cmp(&baseline) {
        Ordering::Greater => format!("+{}", actual - baseline),
        Ordering::Equal => "+0".into(),
        Ordering::Less => format!("-{}", baseline - actual),
    }
}

fn validate_slug(field: &str, value: &str) -> Result<(), BudgetError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || !value.as_bytes()[0].is_ascii_lowercase()
        || value.ends_with('-')
        || value.contains("--")
    {
        return Err(BudgetError::new(format!(
            "{field} `{value}` must be lowercase kebab-case ASCII"
        )));
    }
    Ok(())
}

fn validate_package_id(value: &str) -> Result<(), BudgetError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(BudgetError::new(format!(
            "budget subject package `{value}` must be 1-128 ASCII letters, digits, hyphens, or underscores"
        )));
    }
    Ok(())
}

fn validate_layer_id(value: &str) -> Result<(), BudgetError> {
    validate_text("budget subject layer", value)?;
    if value.trim() != value {
        return Err(BudgetError::new(format!(
            "budget subject layer `{value}` must not have leading or trailing whitespace"
        )));
    }
    Ok(())
}

fn validate_route_id(value: &str) -> Result<(), BudgetError> {
    validate_text("budget subject route", value)?;
    if !value.starts_with('/')
        || value.contains('\\')
        || value.contains('?')
        || value.contains('#')
        || value.chars().any(char::is_whitespace)
        || value.contains("//")
        || value
            .trim_start_matches('/')
            .split('/')
            .any(|segment| matches!(segment, "." | ".."))
    {
        return Err(BudgetError::new(format!(
            "budget subject route `{value}` must be a canonical absolute route path without query, fragment, whitespace, or traversal segments"
        )));
    }
    Ok(())
}

fn validate_logical_path(field: &str, value: &str) -> Result<(), BudgetError> {
    validate_text(field, value)?;
    if value.starts_with('/')
        || value.contains('\\')
        || value.contains(':')
        || value
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(BudgetError::new(format!(
            "{field} `{value}` must be a portable relative logical path"
        )));
    }
    Ok(())
}

fn validate_text(field: &str, value: &str) -> Result<(), BudgetError> {
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(BudgetError::new(format!(
            "{field} must be non-empty, at most 4,096 bytes, and contain no control characters"
        )));
    }
    Ok(())
}

#[cfg(all(test, feature = "css-analysis"))]
mod analysis_tests {
    use lightningcss::stylesheet::{ParserOptions, StyleSheet};

    use super::*;

    #[test]
    fn generic_declaration_identities_bind_selector_context_importance_and_bytes() {
        let stylesheet = StyleSheet::parse(
            ".a { color: red; color: red !important; } .b { color: red; }",
            ParserOptions::default(),
        )
        .expect("stylesheet");
        let metrics = measure_css_budgets(&stylesheet).expect("metrics").file;
        let identities = metrics.declaration_identities().collect::<Vec<_>>();
        assert_eq!(identities.len(), 3);
        assert!(identities.iter().all(|(identity, count)| {
            identity.starts_with("sha256:") && identity.len() == 71 && *count == 1
        }));

        let repeated =
            StyleSheet::parse(".a { color: red; color: red; }", ParserOptions::default())
                .expect("repeated");
        let repeated = measure_css_budgets(&repeated).expect("metrics").file;
        assert_eq!(
            repeated.declaration_identities().collect::<Vec<_>>().len(),
            1
        );
        assert_eq!(repeated.declaration_identities().next().unwrap().1, 2);
    }

    #[test]
    fn bundle_aggregation_detects_cross_artifact_duplicates_and_is_atomic_on_overflow() {
        let left = StyleSheet::parse(".a { color: red; }", ParserOptions::default()).expect("left");
        let right =
            StyleSheet::parse(".b { color: red; }", ParserOptions::default()).expect("right");
        let mut aggregate = measure_css_budgets(&left).expect("left metrics").file;
        let right = measure_css_budgets(&right).expect("right metrics").file;
        let expected_bytes = aggregate.canonical_bytes() + right.canonical_bytes();
        aggregate.try_merge(&right).expect("merge");
        assert_eq!(aggregate.canonical_bytes(), expected_bytes);
        assert_eq!(aggregate.semantic_duplicate_groups(), 1);
        assert_eq!(aggregate.semantic_duplicates(), 1);

        let mut maximum = CssBudgetMetrics {
            canonical_bytes: usize::MAX,
            ..CssBudgetMetrics::default()
        };
        let before = maximum.clone();
        assert!(maximum.try_merge(&right).is_err());
        assert_eq!(maximum, before);
    }
}
