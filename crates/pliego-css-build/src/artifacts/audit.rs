use std::collections::BTreeMap;

use lightningcss::media_query::{MediaFeatureName, QueryFeature};
use lightningcss::properties::Property;
use lightningcss::rules::container::ContainerCondition;
use lightningcss::rules::{CssRule, CssRuleList, Location};
use lightningcss::selector::{Component, PseudoClass};
use lightningcss::stylesheet::{ParserFlags, ParserOptions, PrinterOptions, StyleSheet};

use lightningcss::values::color::{CssColor, LABColor};

use super::{
    CompatibilityProfile, Finding, FindingCause, FindingDocument, FindingEvidence,
    FindingException, FindingRisk, FindingSeverity, FindingSource, FindingSuggestion, FindingTool,
    FindingVerification, sha256_hex,
};
use pliego_css_config::compatibility_data::{
    BASELINE_SNAPSHOT_DATE, BaselineFeatureStatus, WEB_FEATURES_DATA_SHA256, WEB_FEATURES_VERSION,
    WebFeatureSnapshot, web_feature_snapshot,
};
use pliego_css_config::{
    BudgetEvaluation, BudgetMetric, BudgetObservation, BudgetPolicy, BudgetSubject,
    BudgetSubjectKind, CssBudgetInventory, CssBudgetMetrics, evaluate_budget_policy,
    measure_css_budgets,
};

const MAX_CSS_BYTES: usize = 16 * 1024 * 1024;
const BACKEND_NAME: &str = "lightningcss";
const BACKEND_VERSION: &str = "1.0.0-alpha.71";
const CLASSIFIER_VERSION: &str = "css-rule-features-1";
const MAX_DUPLICATE_FINGERPRINT_EVIDENCE: usize = 256;
const MAX_DECLARATION_IDENTITY_EVIDENCE: usize = 4096;

/// Result of standards-first CSS ingestion and its canonical findings.
#[derive(Debug, Eq, PartialEq)]
pub struct CssAuditOutcome {
    document: FindingDocument,
    passed: bool,
    inventory: Option<CssBudgetInventory>,
}

impl CssAuditOutcome {
    /// Returns the canonical finding document shared by every renderer.
    #[must_use]
    pub const fn document(&self) -> &FindingDocument {
        &self.document
    }

    /// Returns whether ingestion and every enforced classified decision completed without error.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.passed
    }

    /// Returns canonical CSS budget measurements when syntax ingestion succeeded.
    #[must_use]
    pub const fn inventory(&self) -> Option<&CssBudgetInventory> {
        self.inventory.as_ref()
    }
}

#[derive(Default)]
struct Metrics {
    rules: usize,
    style_rules: usize,
    selectors: usize,
    style_declarations: usize,
    important_declarations: usize,
    max_specificity: u32,
    typed_declarations: usize,
    unparsed_declarations: usize,
    custom_declarations: usize,
    selector_components: usize,
    selector_combinators: usize,
    selector_attributes: usize,
    selector_pseudo_classes: usize,
    selector_pseudo_elements: usize,
}

#[derive(Clone, Debug)]
enum SourceMarker {
    Token(String),
    Selector,
}

#[derive(Clone, Debug)]
struct FeatureObservation {
    id: &'static str,
    loc: Location,
    marker: SourceMarker,
    occurrences: usize,
}

#[derive(Clone, Debug)]
struct UnclassifiedObservation {
    syntax: String,
    marker: String,
    loc: Location,
    occurrences: usize,
}

#[derive(Clone, Copy)]
enum CompatibilityDecision {
    Allowed,
    Rejected,
    Unmanaged,
}

struct DecisionPresentation {
    code: &'static str,
    severity: FindingSeverity,
    verification: FindingVerification,
    rule: &'static str,
    message: String,
    explanation: &'static str,
}

struct BudgetFindingPresentation {
    code: &'static str,
    severity: FindingSeverity,
    rule: &'static str,
    message: String,
    explanation: &'static str,
}

/// Parses standard CSS without transforming it and audits a bounded set of native CSS features
/// against one explicit compatibility profile.
///
/// # Errors
///
/// Returns an error for an unsafe logical path, an input larger than 16 MiB, or an internal finding
/// contract failure. CSS syntax and enforced compatibility failures are represented as canonical
/// findings, not tool errors.
pub fn audit_standard_css(
    logical_path: &str,
    css: &str,
    profile: CompatibilityProfile,
) -> Result<CssAuditOutcome, String> {
    audit_standard_css_with_budgets(logical_path, css, profile, None, &[])
}

/// Audit standard CSS and optionally evaluate a versioned budget policy against explicit artifact
/// ownership subjects plus automatically detected file/layer subjects.
///
/// # Errors
///
/// Returns the same contract errors as [`audit_standard_css`], plus invalid/duplicate budget
/// observations or serialization failures while producing canonical budget evidence.
#[allow(clippy::too_many_lines)]
pub fn audit_standard_css_with_budgets(
    logical_path: &str,
    css: &str,
    profile: CompatibilityProfile,
    budget_policy: Option<&BudgetPolicy>,
    budget_subjects: &[BudgetSubject],
) -> Result<CssAuditOutcome, String> {
    if css.len() > MAX_CSS_BYTES {
        return Err("CSS audit input exceeds 16 MiB".into());
    }
    if budget_policy.is_none() && !budget_subjects.is_empty() {
        return Err("budget subjects require a budget policy".into());
    }
    let source_hash = format!("sha256:{}", sha256_hex(css.as_bytes()));
    let options = ParserOptions {
        filename: logical_path.into(),
        flags: ParserFlags::NESTING,
        ..ParserOptions::default()
    };
    match StyleSheet::parse(css, options) {
        Ok(stylesheet) => {
            let mut metrics = Metrics::default();
            let mut features = Vec::new();
            let mut unclassified = Vec::new();
            inventory_rules(
                &stylesheet.rules,
                &mut metrics,
                &mut features,
                &mut unclassified,
            );
            let budget_inventory =
                measure_css_budgets(&stylesheet).map_err(|error| error.to_string())?;
            let budget_metrics = budget_inventory.file();
            let (end_line, end_column) = end_position(css);
            let source = FindingSource::new(logical_path, 0, css.len())
                .and_then(|source| source.with_position(1, 1, end_line, end_column))
                .map_err(|error| error.to_string())?;
            let mut finding = Finding::new(
                "PCSS-AUDIT-000",
                "inventory",
                FindingSeverity::Info,
                "standard CSS parsed and inventoried",
                FindingVerification::Verified,
                FindingCause::new(
                    "audit.ingestion",
                    "standard-css-parsed",
                    "the versioned backend parsed the complete source without recovery",
                )
                .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?
            .with_source(source.clone())
            .map_err(|error| error.to_string())?
            .with_context("backend", BACKEND_NAME)
            .map_err(|error| error.to_string())?
            .with_context("backend-version", BACKEND_VERSION)
            .map_err(|error| error.to_string())?
            .with_context("classifier-version", CLASSIFIER_VERSION)
            .map_err(|error| error.to_string())?
            .with_context("target-profile", profile.as_str())
            .map_err(|error| error.to_string())?;
            for (name, value, unit, evidence_source) in [
                (
                    "input-bytes",
                    css.len().to_string(),
                    Some("bytes"),
                    "source-bytes",
                ),
                (
                    "canonical-bytes",
                    budget_metrics.canonical_bytes().to_string(),
                    Some("bytes"),
                    "lightningcss-minified-ast",
                ),
                ("input-sha256", source_hash, None, "source-bytes"),
                (
                    "rules",
                    metrics.rules.to_string(),
                    Some("rules"),
                    "lightningcss-ast",
                ),
                (
                    "style-rules",
                    metrics.style_rules.to_string(),
                    Some("rules"),
                    "lightningcss-ast",
                ),
                (
                    "selectors",
                    metrics.selectors.to_string(),
                    Some("selectors"),
                    "lightningcss-ast",
                ),
                (
                    "selector-components",
                    metrics.selector_components.to_string(),
                    Some("components"),
                    "lightningcss-selector-ast",
                ),
                (
                    "selector-combinators",
                    metrics.selector_combinators.to_string(),
                    Some("combinators"),
                    "lightningcss-selector-ast",
                ),
                (
                    "selector-attributes",
                    metrics.selector_attributes.to_string(),
                    Some("attributes"),
                    "lightningcss-selector-ast",
                ),
                (
                    "selector-pseudo-classes",
                    metrics.selector_pseudo_classes.to_string(),
                    Some("pseudo-classes"),
                    "lightningcss-selector-ast",
                ),
                (
                    "selector-pseudo-elements",
                    metrics.selector_pseudo_elements.to_string(),
                    Some("pseudo-elements"),
                    "lightningcss-selector-ast",
                ),
                (
                    "style-declarations",
                    metrics.style_declarations.to_string(),
                    Some("declarations"),
                    "lightningcss-ast",
                ),
                (
                    "typed-declarations",
                    metrics.typed_declarations.to_string(),
                    Some("declarations"),
                    "lightningcss-typed-properties",
                ),
                (
                    "unparsed-declarations",
                    metrics.unparsed_declarations.to_string(),
                    Some("declarations"),
                    "lightningcss-unparsed-properties",
                ),
                (
                    "custom-declarations",
                    metrics.custom_declarations.to_string(),
                    Some("declarations"),
                    "lightningcss-custom-properties",
                ),
                (
                    "important-declarations",
                    metrics.important_declarations.to_string(),
                    Some("declarations"),
                    "lightningcss-ast",
                ),
                (
                    "maximum-specificity",
                    format_specificity(metrics.max_specificity),
                    Some("specificity"),
                    "lightningcss-ast",
                ),
                (
                    "semantic-duplicate-groups",
                    budget_metrics.semantic_duplicate_groups().to_string(),
                    Some("groups"),
                    "normalized-declaration-blocks",
                ),
                (
                    "semantic-duplicates",
                    budget_metrics.semantic_duplicates().to_string(),
                    Some("rules"),
                    "normalized-declaration-blocks",
                ),
                (
                    "classified-features",
                    features.len().to_string(),
                    Some("features"),
                    "pliegocss-classifier",
                ),
                (
                    "unclassified-syntax",
                    unclassified.len().to_string(),
                    Some("at-rules"),
                    "pliegocss-classifier",
                ),
            ] {
                let evidence = FindingEvidence::new("metric", name, value, evidence_source)
                    .and_then(|evidence| match unit {
                        Some(unit) => evidence.with_unit(unit),
                        None => Ok(evidence),
                    })
                    .map_err(|error| error.to_string())?;
                finding = finding
                    .with_evidence(evidence)
                    .map_err(|error| error.to_string())?;
            }
            finding = add_duplicate_fingerprint_evidence(finding, budget_metrics)?;
            finding = add_declaration_identity_evidence(finding, budget_metrics)?;
            let (mut compatibility, rejected) = compatibility_findings(
                logical_path,
                css,
                &source,
                profile,
                &features,
                &unclassified,
            )?;
            let mut findings = vec![finding];
            findings.append(&mut compatibility);
            let mut budget_failed = false;
            if let Some(policy) = budget_policy {
                let budget_outcome = evaluate_audit_budget_findings(
                    logical_path,
                    css,
                    &source,
                    policy,
                    budget_subjects,
                    &budget_inventory,
                )?;
                let (mut budget_findings, failed) = budget_outcome;
                findings.append(&mut budget_findings);
                budget_failed = failed;
            }
            outcome(
                findings,
                !rejected && !budget_failed,
                Some(budget_inventory),
            )
        }
        Err(error) => {
            let message = error.kind.to_string();
            let (byte_start, byte_end, line, column) = error.loc.map_or((0, 0, 1, 1), |location| {
                locate(css, location.line, location.column)
            });
            let source = FindingSource::new(logical_path, byte_start, byte_end)
                .and_then(|source| {
                    source.with_position(
                        line,
                        column,
                        line,
                        column + usize::from(byte_end > byte_start),
                    )
                })
                .map_err(|contract| contract.to_string())?;
            let finding = Finding::new(
                "PCSS-SYNTAX-001",
                "syntax",
                FindingSeverity::Error,
                "standard CSS could not be parsed",
                FindingVerification::Verified,
                FindingCause::new("css.syntax", "parser-error", message)
                    .map_err(|contract| contract.to_string())?,
            )
            .map_err(|contract| contract.to_string())?
            .with_source(source)
            .map_err(|contract| contract.to_string())?
            .with_context("backend", BACKEND_NAME)
            .map_err(|contract| contract.to_string())?
            .with_context("backend-version", BACKEND_VERSION)
            .map_err(|contract| contract.to_string())?
            .with_context("target-profile", profile.as_str())
            .map_err(|contract| contract.to_string())?
            .with_context("web-features-version", WEB_FEATURES_VERSION)
            .map_err(|contract| contract.to_string())?
            .with_evidence(
                FindingEvidence::new("metric", "input-sha256", source_hash, "source-bytes")
                    .map_err(|contract| contract.to_string())?,
            )
            .map_err(|contract| contract.to_string())?;
            outcome(vec![finding], false, None)
        }
    }
}

fn add_duplicate_fingerprint_evidence(
    mut finding: Finding,
    metrics: &CssBudgetMetrics,
) -> Result<Finding, String> {
    let duplicates = metrics.duplicate_fingerprints().collect::<Vec<_>>();
    let truncated = duplicates.len() > MAX_DUPLICATE_FINGERPRINT_EVIDENCE;
    for (index, (fingerprint, occurrences)) in duplicates
        .into_iter()
        .take(MAX_DUPLICATE_FINGERPRINT_EVIDENCE)
        .enumerate()
    {
        finding = finding
            .with_evidence(
                FindingEvidence::new(
                    "fingerprint",
                    format!("semantic-duplicate-{:04}", index + 1),
                    format!("{fingerprint};occurrences={occurrences}"),
                    "normalized-declaration-blocks",
                )
                .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
    }
    finding
        .with_evidence(
            FindingEvidence::new(
                "metric",
                "semantic-fingerprint-evidence-truncated",
                if truncated { "true" } else { "false" },
                "pliegocss-budget-engine",
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
}

fn add_declaration_identity_evidence(
    mut finding: Finding,
    metrics: &CssBudgetMetrics,
) -> Result<Finding, String> {
    let identities = metrics.declaration_identities().collect::<Vec<_>>();
    let truncated = identities.len() > MAX_DECLARATION_IDENTITY_EVIDENCE;
    for (index, (identity, occurrences)) in identities
        .into_iter()
        .take(MAX_DECLARATION_IDENTITY_EVIDENCE)
        .enumerate()
    {
        finding = finding
            .with_evidence(
                FindingEvidence::new(
                    "identity",
                    format!("generic-css-declaration-{:04}", index + 1),
                    format!("{identity};occurrences={occurrences}"),
                    "pliegocss-generic-css-declaration-v1",
                )
                .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
    }
    finding
        .with_evidence(
            FindingEvidence::new(
                "metric",
                "generic-css-declaration-identities",
                metrics.declaration_identities().count().to_string(),
                "pliegocss-budget-engine",
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?
        .with_evidence(
            FindingEvidence::new(
                "metric",
                "generic-css-declaration-identity-evidence-truncated",
                if truncated { "true" } else { "false" },
                "pliegocss-budget-engine",
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
}

fn evaluate_audit_budget_findings(
    logical_path: &str,
    css: &str,
    full_source: &FindingSource,
    policy: &BudgetPolicy,
    declared_subjects: &[BudgetSubject],
    inventory: &CssBudgetInventory,
) -> Result<(Vec<Finding>, bool), String> {
    let file_metrics = inventory.file();
    let file_subject = BudgetSubject::new(BudgetSubjectKind::File, logical_path)
        .map_err(|error| error.to_string())?;
    let mut observations = vec![(
        BudgetObservation::new(file_subject, file_metrics.measurements()),
        full_source.clone(),
    )];
    for subject in declared_subjects {
        if !matches!(
            subject.kind(),
            BudgetSubjectKind::Package | BudgetSubjectKind::Route
        ) {
            return Err(format!(
                "declared budget subject `{}:{}` must be `package` or `route`; file and layer subjects are automatic",
                subject.kind().as_str(),
                subject.id()
            ));
        }
        observations.push((
            BudgetObservation::new(subject.clone(), file_metrics.measurements()),
            full_source.clone(),
        ));
    }
    for layer in inventory.layers() {
        let subject = BudgetSubject::new(BudgetSubjectKind::Layer, layer.name())
            .map_err(|error| error.to_string())?;
        observations.push((
            BudgetObservation::new(subject, layer.metrics().measurements()),
            css_layer_finding_source(logical_path, css, layer.first_location())?,
        ));
    }
    evaluate_budget_observation_findings(policy, &observations, full_source)
}

/// Evaluate one closed budget policy against verified observations and their exact sources.
///
/// This projection is shared by single-artifact and Asset Plan audits so human/JSON/SARIF output
/// keeps one code, evidence, exception, and policy-hash contract.
///
/// # Errors
///
/// Returns an error for invalid/duplicate observations, a missing observation source, policy
/// serialization failure, or a finding-contract violation.
pub fn evaluate_budget_observation_findings(
    policy: &BudgetPolicy,
    observations: &[(BudgetObservation, FindingSource)],
    policy_source: &FindingSource,
) -> Result<(Vec<Finding>, bool), String> {
    let measurements = observations
        .iter()
        .map(|(observation, _)| observation.clone())
        .collect::<Vec<_>>();
    let report =
        evaluate_budget_policy(policy, &measurements).map_err(|error| error.to_string())?;
    let policy_json = policy.to_json_pretty().map_err(|error| error.to_string())?;
    let policy_hash = format!("sha256:{}", sha256_hex(policy_json.as_bytes()));
    let incomplete_coverage = report.matched_budgets() != policy.budget_count();
    let sources = observations
        .iter()
        .map(|(observation, source)| (observation.subject().clone(), source))
        .collect::<BTreeMap<_, _>>();
    let mut findings =
        Vec::with_capacity(report.evaluations().len() + usize::from(incomplete_coverage));
    if incomplete_coverage {
        findings.push(unmatched_budget_finding(
            policy_source,
            policy,
            &policy_hash,
            observations.len(),
            report.matched_budgets(),
        )?);
    }
    for evaluation in report.evaluations() {
        let source = sources
            .get(&evaluation.subject)
            .ok_or_else(|| "budget evaluation lost its observed source".to_owned())?;
        findings.push(budget_evaluation_finding(
            source,
            policy,
            &policy_hash,
            evaluation,
        )?);
    }
    Ok((findings, incomplete_coverage || report.failed()))
}

fn unmatched_budget_finding(
    source: &FindingSource,
    policy: &BudgetPolicy,
    policy_hash: &str,
    observed_subjects: usize,
    matched_budgets: usize,
) -> Result<Finding, String> {
    Finding::new(
        "PCSS-BUDGET-199",
        "budget",
        FindingSeverity::Error,
        "budget policy left one or more declared CSS subjects unobserved",
        FindingVerification::Verified,
        FindingCause::new(
            "budget.policy",
            "unmatched-policy",
            "every explicit budget definition must match one verified file, package, route, or detected layer subject",
        )
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?
    .with_source(source.clone())
    .map_err(|error| error.to_string())?
    .with_context("budget-policy-sha256", policy_hash)
    .map_err(|error| error.to_string())?
    .with_context("budget-policy-version", policy.policy_version().to_string())
    .map_err(|error| error.to_string())?
    .with_evidence(
        FindingEvidence::new(
            "metric",
            "declared-budgets",
            policy.budget_count().to_string(),
            "pliegocss-budget-engine",
        )
        .and_then(|evidence| evidence.with_unit("budgets"))
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?
    .with_evidence(
        FindingEvidence::new(
            "metric",
            "matched-budgets",
            matched_budgets.to_string(),
            "pliegocss-budget-engine",
        )
        .and_then(|evidence| evidence.with_unit("budgets"))
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?
    .with_evidence(
        FindingEvidence::new(
            "metric",
            "observed-subjects",
            observed_subjects.to_string(),
            "pliegocss-budget-engine",
        )
        .and_then(|evidence| evidence.with_unit("subjects"))
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn budget_evaluation_finding(
    source: &FindingSource,
    policy: &BudgetPolicy,
    policy_hash: &str,
    evaluation: &BudgetEvaluation,
) -> Result<Finding, String> {
    let presentation = budget_finding_presentation(evaluation);
    let mut finding = Finding::new(
        presentation.code,
        "budget",
        presentation.severity,
        presentation.message,
        FindingVerification::Verified,
        FindingCause::new("budget.policy", presentation.rule, presentation.explanation)
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?
    .with_source(source.clone())
    .map_err(|error| error.to_string())?
    .with_context("budget-id", evaluation.budget_id.clone())
    .map_err(|error| error.to_string())?
    .with_context("budget-policy-sha256", policy_hash)
    .map_err(|error| error.to_string())?
    .with_context("budget-policy-version", policy.policy_version().to_string())
    .map_err(|error| error.to_string())?
    .with_context("metric", evaluation.metric.as_str())
    .map_err(|error| error.to_string())?
    .with_context("subject-id", evaluation.subject.id())
    .map_err(|error| error.to_string())?
    .with_context("subject-kind", evaluation.subject.kind().as_str())
    .map_err(|error| error.to_string())?;
    for (name, value, unit) in [
        (
            "actual",
            Some(evaluation.actual.as_str()),
            Some(evaluation.metric.unit()),
        ),
        (
            "maximum",
            Some(evaluation.maximum.as_str()),
            Some(evaluation.metric.unit()),
        ),
        (
            "baseline",
            evaluation.baseline.as_deref(),
            Some(evaluation.metric.unit()),
        ),
        (
            "delta",
            evaluation.delta.as_deref(),
            Some(evaluation.metric.unit()),
        ),
        (
            "max-increase",
            evaluation.max_increase.as_deref(),
            Some(evaluation.metric.unit()),
        ),
        (
            "maximum-exceeded",
            Some(if evaluation.maximum_exceeded {
                "true"
            } else {
                "false"
            }),
            None,
        ),
        (
            "delta-exceeded",
            Some(if evaluation.delta_exceeded {
                "true"
            } else {
                "false"
            }),
            None,
        ),
    ] {
        if let Some(value) = value {
            let evidence = FindingEvidence::new("budget", name, value, "pliegocss-budget-engine")
                .map_err(|error| error.to_string())?;
            let evidence = match unit {
                Some(unit) => evidence
                    .with_unit(unit)
                    .map_err(|error| error.to_string())?,
                None => evidence,
            };
            finding = finding
                .with_evidence(evidence)
                .map_err(|error| error.to_string())?;
        }
    }
    if let Some(exception) = &evaluation.exception {
        finding = finding
            .with_exception(
                FindingException::new(&exception.id, &exception.justification)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
    }
    if evaluation.enforced() {
        finding = add_budget_suggestions(finding, evaluation.metric)?;
    }
    Ok(finding)
}

fn budget_finding_presentation(evaluation: &BudgetEvaluation) -> BudgetFindingPresentation {
    if evaluation.enforced() {
        BudgetFindingPresentation {
            code: "PCSS-BUDGET-101",
            severity: FindingSeverity::Error,
            rule: "limit-exceeded",
            message: format!(
                "budget `{}` exceeded `{}` for {} `{}`",
                evaluation.budget_id,
                evaluation.metric.as_str(),
                evaluation.subject.kind().as_str(),
                evaluation.subject.id()
            ),
            explanation: "the measured CSS artifact exceeds an absolute or regression budget without a reviewed exception",
        }
    } else if evaluation.violated() {
        BudgetFindingPresentation {
            code: "PCSS-BUDGET-102",
            severity: FindingSeverity::Warning,
            rule: "reviewed-exception",
            message: format!(
                "budget `{}` exceeded `{}` under a reviewed exception",
                evaluation.budget_id,
                evaluation.metric.as_str()
            ),
            explanation: "the measured CSS artifact exceeds a boundary, but the policy contains an explicit reviewed exception",
        }
    } else {
        BudgetFindingPresentation {
            code: "PCSS-BUDGET-100",
            severity: FindingSeverity::Info,
            rule: "within-limit",
            message: format!(
                "budget `{}` is within `{}` for {} `{}`",
                evaluation.budget_id,
                evaluation.metric.as_str(),
                evaluation.subject.kind().as_str(),
                evaluation.subject.id()
            ),
            explanation: "the measured CSS artifact is within both the absolute and configured regression boundaries",
        }
    }
}

fn add_budget_suggestions(finding: Finding, metric: BudgetMetric) -> Result<Finding, String> {
    finding
        .with_suggestion(
            FindingSuggestion::new(
                1,
                "reduce-measured-css-cost",
                format!(
                    "reduce `{}` in the attributed artifact without weakening ownership evidence",
                    metric.as_str()
                ),
                FindingRisk::Medium,
                "budget-subject",
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?
        .with_suggestion(
            FindingSuggestion::new(
                2,
                "review-budget-change",
                "change the limit or add an exception only with measured regression evidence and an explicit justification",
                FindingRisk::High,
                "budget-policy",
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
}

fn compatibility_findings(
    logical_path: &str,
    css: &str,
    full_source: &FindingSource,
    profile: CompatibilityProfile,
    observations: &[FeatureObservation],
    unclassified: &[UnclassifiedObservation],
) -> Result<(Vec<Finding>, bool), String> {
    let mut findings = vec![partial_coverage_finding(full_source, profile)?];
    let mut rejected = false;
    for observation in observations {
        let snapshot = web_feature_snapshot(observation.id)
            .ok_or_else(|| format!("missing frozen web-features entry `{}`", observation.id))?;
        let decision = feature_decision(profile, snapshot);
        rejected |= matches!(decision, CompatibilityDecision::Rejected);
        findings.push(feature_finding(
            logical_path,
            css,
            profile,
            snapshot,
            observation,
            decision,
        )?);
    }
    for observation in unclassified {
        let is_rejected = profile.rejects_unclassified_css();
        rejected |= is_rejected;
        findings.push(unclassified_finding(
            logical_path,
            css,
            profile,
            observation,
            is_rejected,
        )?);
    }
    Ok((findings, rejected))
}

fn partial_coverage_finding(
    source: &FindingSource,
    profile: CompatibilityProfile,
) -> Result<Finding, String> {
    Finding::new(
        "PCSS-COMPAT-001",
        "compatibility",
        FindingSeverity::Warning,
        "compatibility analysis is bounded to the versioned CSS rule classifier",
        FindingVerification::Unverified,
        FindingCause::new(
            "compatibility.classifier",
            "partial-coverage",
            "five representative declaration/value features and five selector features are classified; the complete surfaces remain unclassified",
        )
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?
    .with_source(source.clone())
    .map_err(|error| error.to_string())?
    .with_context("classifier-version", CLASSIFIER_VERSION)
    .map_err(|error| error.to_string())?
    .with_context("target-profile", profile.as_str())
    .map_err(|error| error.to_string())?
    .with_context("web-features-version", WEB_FEATURES_VERSION)
    .map_err(|error| error.to_string())?
    .with_evidence(
        FindingEvidence::new(
            "dataset",
            "web-features-data-sha256",
            WEB_FEATURES_DATA_SHA256,
            "web-features",
        )
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn feature_decision(
    profile: CompatibilityProfile,
    feature: &WebFeatureSnapshot,
) -> CompatibilityDecision {
    match profile {
        CompatibilityProfile::BaselineWidely => {
            if feature.baseline == BaselineFeatureStatus::Widely {
                CompatibilityDecision::Allowed
            } else {
                CompatibilityDecision::Rejected
            }
        }
        CompatibilityProfile::Modern => {
            if feature.supports_modern_profile() {
                CompatibilityDecision::Allowed
            } else {
                CompatibilityDecision::Rejected
            }
        }
        CompatibilityProfile::None => CompatibilityDecision::Unmanaged,
    }
}

#[allow(clippy::too_many_arguments)]
fn feature_finding(
    logical_path: &str,
    css: &str,
    profile: CompatibilityProfile,
    feature: &WebFeatureSnapshot,
    observation: &FeatureObservation,
    decision: CompatibilityDecision,
) -> Result<Finding, String> {
    let presentation = decision_presentation(profile, feature, decision);
    let source = source_for_location(logical_path, css, observation.loc, &observation.marker)?;
    let finding = Finding::new(
        presentation.code,
        "compatibility",
        presentation.severity,
        presentation.message,
        presentation.verification,
        FindingCause::new(
            "compatibility.web-features",
            presentation.rule,
            presentation.explanation,
        )
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?
    .with_source(source)
    .map_err(|error| error.to_string())?
    .with_context("compat-key", feature.compat_key)
    .map_err(|error| error.to_string())?
    .with_context("feature-id", feature.id)
    .map_err(|error| error.to_string())?
    .with_context("classifier-version", CLASSIFIER_VERSION)
    .map_err(|error| error.to_string())?
    .with_context("target-profile", profile.as_str())
    .map_err(|error| error.to_string())?
    .with_context("web-features-version", WEB_FEATURES_VERSION)
    .map_err(|error| error.to_string())?;
    let finding = add_feature_evidence(finding, feature, observation)?;
    add_rejection_suggestions(finding, decision)
}

fn decision_presentation(
    profile: CompatibilityProfile,
    feature: &WebFeatureSnapshot,
    decision: CompatibilityDecision,
) -> DecisionPresentation {
    match decision {
        CompatibilityDecision::Allowed => (
            "PCSS-COMPAT-100",
            FindingSeverity::Info,
            FindingVerification::Verified,
            "native-feature-allowed",
            format!(
                "{} is allowed by target profile `{}`",
                feature.name,
                profile.as_str()
            ),
            "the frozen compatibility dataset proves support inside the selected profile boundary",
        )
            .into(),
        CompatibilityDecision::Rejected => (
            "PCSS-COMPAT-101",
            FindingSeverity::Error,
            FindingVerification::Verified,
            "native-feature-rejected",
            format!(
                "{} is not allowed by target profile `{}`",
                feature.name,
                profile.as_str()
            ),
            "the frozen compatibility dataset does not prove native support across the selected target vector",
        )
            .into(),
        CompatibilityDecision::Unmanaged => (
            "PCSS-COMPAT-102",
            FindingSeverity::Info,
            FindingVerification::Unverified,
            "native-feature-unmanaged",
            format!("{} is unmanaged under target profile `none`", feature.name),
            "the selected profile intentionally makes no browser compatibility guarantee",
        )
            .into(),
    }
}

impl
    From<(
        &'static str,
        FindingSeverity,
        FindingVerification,
        &'static str,
        String,
        &'static str,
    )> for DecisionPresentation
{
    fn from(
        value: (
            &'static str,
            FindingSeverity,
            FindingVerification,
            &'static str,
            String,
            &'static str,
        ),
    ) -> Self {
        Self {
            code: value.0,
            severity: value.1,
            verification: value.2,
            rule: value.3,
            message: value.4,
            explanation: value.5,
        }
    }
}

fn add_feature_evidence(
    mut finding: Finding,
    feature: &WebFeatureSnapshot,
    observation: &FeatureObservation,
) -> Result<Finding, String> {
    for (name, value, unit) in [
        (
            "baseline-status",
            feature.baseline.as_str().to_owned(),
            None,
        ),
        (
            "baseline-low-date",
            feature
                .baseline_low_date
                .unwrap_or("not-applicable")
                .to_owned(),
            None,
        ),
        (
            "baseline-high-date",
            feature
                .baseline_high_date
                .unwrap_or("not-applicable")
                .to_owned(),
            None,
        ),
        (
            "occurrences",
            observation.occurrences.to_string(),
            Some("occurrences"),
        ),
        ("support-chrome", support(feature.chrome), None),
        ("support-edge", support(feature.edge), None),
        ("support-firefox", support(feature.firefox), None),
        ("support-safari", support(feature.safari), None),
        (
            "web-features-data-sha256",
            WEB_FEATURES_DATA_SHA256.to_owned(),
            None,
        ),
        (
            "compatibility-snapshot-date",
            BASELINE_SNAPSHOT_DATE.to_owned(),
            None,
        ),
    ] {
        let evidence = FindingEvidence::new("compatibility", name, value, "web-features")
            .and_then(|evidence| match unit {
                Some(unit) => evidence.with_unit(unit),
                None => Ok(evidence),
            })
            .map_err(|error| error.to_string())?;
        finding = finding
            .with_evidence(evidence)
            .map_err(|error| error.to_string())?;
    }
    Ok(finding)
}

fn add_rejection_suggestions(
    mut finding: Finding,
    decision: CompatibilityDecision,
) -> Result<Finding, String> {
    if matches!(decision, CompatibilityDecision::Rejected) {
        finding = finding
            .with_suggestion(
                FindingSuggestion::new(
                    1,
                    "use-compatible-alternative",
                    "replace or progressively enhance the feature with a target-compatible path",
                    FindingRisk::Medium,
                    "source-rule",
                )
                .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?
            .with_suggestion(
                FindingSuggestion::new(
                    2,
                    "delegate-compatibility-explicitly",
                    "select target profile `none` only when a reviewed downstream pipeline owns compatibility",
                    FindingRisk::High,
                    "project-policy",
                )
                .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(finding)
}

fn unclassified_finding(
    logical_path: &str,
    css: &str,
    profile: CompatibilityProfile,
    observation: &UnclassifiedObservation,
    rejected: bool,
) -> Result<Finding, String> {
    let marker = SourceMarker::Token(observation.marker.clone());
    let source = source_for_location(logical_path, css, observation.loc, &marker)?;
    let severity = if rejected {
        FindingSeverity::Error
    } else {
        FindingSeverity::Warning
    };
    Finding::new(
        "PCSS-COMPAT-199",
        "compatibility",
        severity,
        format!(
            "syntax `{}` has no compatibility classification",
            observation.syntax
        ),
        FindingVerification::Unverified,
        FindingCause::new(
            "compatibility.classifier",
            "unclassified-syntax",
            "successful parsing is not treated as browser compatibility evidence",
        )
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?
    .with_source(source)
    .map_err(|error| error.to_string())?
    .with_context("syntax", observation.syntax.clone())
    .map_err(|error| error.to_string())?
    .with_context("target-profile", profile.as_str())
    .map_err(|error| error.to_string())?
    .with_evidence(
        FindingEvidence::new(
            "metric",
            "occurrences",
            observation.occurrences.to_string(),
            "lightningcss-ast",
        )
        .and_then(|evidence| evidence.with_unit("occurrences"))
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn support(value: Option<&str>) -> String {
    value.unwrap_or("unsupported-or-unreported").to_owned()
}

fn outcome(
    findings: Vec<Finding>,
    passed: bool,
    inventory: Option<CssBudgetInventory>,
) -> Result<CssAuditOutcome, String> {
    Ok(CssAuditOutcome {
        document: FindingDocument::new(
            FindingTool::new("pliegocss", env!("CARGO_PKG_VERSION"))
                .map_err(|error| error.to_string())?,
            "audit",
            findings,
        )
        .map_err(|error| error.to_string())?,
        passed,
        inventory,
    })
}

fn inventory_rules(
    rules: &CssRuleList<'_>,
    metrics: &mut Metrics,
    features: &mut Vec<FeatureObservation>,
    unclassified: &mut Vec<UnclassifiedObservation>,
) {
    for rule in &rules.0 {
        metrics.rules += 1;
        match rule {
            CssRule::Style(rule) => inventory_style(rule, metrics, features, unclassified),
            CssRule::Nesting(rule) => {
                observe_feature(
                    features,
                    "nesting",
                    rule.loc,
                    SourceMarker::Token("@nest".into()),
                );
                inventory_style(&rule.style, metrics, features, unclassified);
            }
            CssRule::NestedDeclarations(rule) => {
                metrics.style_declarations += rule.declarations.declarations.len()
                    + rule.declarations.important_declarations.len();
                metrics.important_declarations += rule.declarations.important_declarations.len();
            }
            CssRule::Media(rule) => {
                inventory_rules(&rule.rules, metrics, features, unclassified);
            }
            CssRule::Supports(rule) => {
                inventory_rules(&rule.rules, metrics, features, unclassified);
            }
            CssRule::MozDocument(rule) => {
                inventory_rules(&rule.rules, metrics, features, unclassified);
            }
            CssRule::LayerStatement(rule) => {
                observe_feature(
                    features,
                    "cascade-layers",
                    rule.loc,
                    SourceMarker::Token("@layer".into()),
                );
            }
            CssRule::LayerBlock(rule) => {
                observe_feature(
                    features,
                    "cascade-layers",
                    rule.loc,
                    SourceMarker::Token("@layer".into()),
                );
                inventory_rules(&rule.rules, metrics, features, unclassified);
            }
            CssRule::Container(rule) => {
                if observe_container_condition(features, rule.condition.as_ref(), rule.loc) {
                    observe_unclassified(
                        unclassified,
                        "@container condition",
                        "@container",
                        rule.loc,
                    );
                }
                inventory_rules(&rule.rules, metrics, features, unclassified);
            }
            CssRule::Scope(rule) => {
                observe_feature(
                    features,
                    "scope",
                    rule.loc,
                    SourceMarker::Token("@scope".into()),
                );
                inventory_rules(&rule.rules, metrics, features, unclassified);
            }
            CssRule::StartingStyle(rule) => {
                observe_feature(
                    features,
                    "starting-style",
                    rule.loc,
                    SourceMarker::Token("@starting-style".into()),
                );
                inventory_rules(&rule.rules, metrics, features, unclassified);
            }
            CssRule::Property(rule) => observe_feature(
                features,
                "registered-custom-properties",
                rule.loc,
                SourceMarker::Token("@property".into()),
            ),
            CssRule::Unknown(rule) => {
                observe_unclassified(
                    unclassified,
                    &format!("@{}", rule.name),
                    &format!("@{}", rule.name),
                    rule.loc,
                );
            }
            _ => {}
        }
    }
}

fn inventory_style(
    rule: &lightningcss::rules::style::StyleRule<'_>,
    metrics: &mut Metrics,
    features: &mut Vec<FeatureObservation>,
    unclassified: &mut Vec<UnclassifiedObservation>,
) {
    metrics.style_rules += 1;
    metrics.selectors += rule.selectors.0.len();
    metrics.style_declarations +=
        rule.declarations.declarations.len() + rule.declarations.important_declarations.len();
    metrics.important_declarations += rule.declarations.important_declarations.len();
    inventory_declaration_shapes(
        rule.declarations
            .declarations
            .iter()
            .chain(&rule.declarations.important_declarations),
        metrics,
    );
    for declaration in rule
        .declarations
        .declarations
        .iter()
        .chain(&rule.declarations.important_declarations)
    {
        let serialized = declaration
            .to_css_string(false, PrinterOptions::default())
            .unwrap_or_default();
        if serialized.contains("color-mix(") {
            let id = if color_mix_has_three_or_more_colors(&serialized) {
                "color-mix-variadic"
            } else {
                "color-mix"
            };
            observe_feature(
                features,
                id,
                rule.loc,
                SourceMarker::Token("color-mix(".into()),
            );
            continue;
        }
        match declaration {
            Property::UserSelect(..) => observe_feature(
                features,
                "user-select",
                rule.loc,
                SourceMarker::Token("user-select".into()),
            ),
            Property::AspectRatio(..) => observe_feature(
                features,
                "aspect-ratio",
                rule.loc,
                SourceMarker::Token("aspect-ratio".into()),
            ),
            Property::Color(color) => observe_color_features(features, color, rule.loc),
            _ => {}
        }
    }
    for selector in &rule.selectors.0 {
        metrics.max_specificity = metrics.max_specificity.max(selector.specificity());
        inventory_selector_shapes(selector.iter_raw_match_order(), metrics);
        for component in selector.iter_raw_match_order() {
            match component {
                Component::NonTSPseudoClass(PseudoClass::FocusVisible) => {
                    observe_feature(features, "focus-visible", rule.loc, SourceMarker::Selector);
                }
                Component::Has(_) => {
                    observe_feature(features, "has", rule.loc, SourceMarker::Selector);
                }
                Component::Is(_) => {
                    observe_feature(features, "is", rule.loc, SourceMarker::Selector);
                }
                Component::Where(_) => {
                    observe_feature(features, "where", rule.loc, SourceMarker::Selector);
                }
                Component::Negation(selectors) if selectors.len() > 1 => {
                    observe_feature(features, "not", rule.loc, SourceMarker::Selector);
                }
                _ => {}
            }
        }
    }
    if !rule.rules.0.is_empty() {
        observe_feature(features, "nesting", rule.loc, SourceMarker::Selector);
    }
    inventory_rules(&rule.rules, metrics, features, unclassified);
}

fn color_mix_has_three_or_more_colors(serialized: &str) -> bool {
    let Some(start) = serialized.find("color-mix(") else {
        return false;
    };
    let mut depth = 0_u32;
    let mut commas = 0_u8;
    for character in serialized[start + "color-mix(".len()..].chars() {
        match character {
            '(' => depth += 1,
            ')' if depth == 0 => break,
            ')' => depth -= 1,
            ',' if depth == 0 => commas = commas.saturating_add(1),
            _ => {}
        }
    }
    commas >= 3
}

fn observe_color_features(features: &mut Vec<FeatureObservation>, color: &CssColor, loc: Location) {
    match color {
        CssColor::Predefined(_) => observe_feature(
            features,
            "color-function",
            loc,
            SourceMarker::Token("color(".into()),
        ),
        CssColor::LAB(color)
            if matches!(color.as_ref(), LABColor::OKLAB(_) | LABColor::OKLCH(_)) =>
        {
            observe_feature(
                features,
                "oklab",
                loc,
                SourceMarker::Token(if matches!(color.as_ref(), LABColor::OKLCH(_)) {
                    "oklch(".into()
                } else {
                    "oklab(".into()
                }),
            );
        }
        _ => {}
    }
}

fn inventory_selector_shapes<'i: 'a, 'a>(
    components: impl Iterator<Item = &'a Component<'i>>,
    metrics: &mut Metrics,
) {
    for component in components {
        metrics.selector_components += 1;
        match component {
            Component::Combinator(_) => metrics.selector_combinators += 1,
            Component::AttributeInNoNamespaceExists { .. }
            | Component::AttributeInNoNamespace { .. }
            | Component::AttributeOther(_) => metrics.selector_attributes += 1,
            Component::PseudoElement(_) | Component::Slotted(_) | Component::Part(_) => {
                metrics.selector_pseudo_elements += 1;
            }
            Component::Negation(_)
            | Component::Root
            | Component::Empty
            | Component::Scope
            | Component::Nth(_)
            | Component::NthOf(_)
            | Component::NonTSPseudoClass(_)
            | Component::Host(_)
            | Component::Where(_)
            | Component::Is(_)
            | Component::Any(_, _)
            | Component::Has(_) => metrics.selector_pseudo_classes += 1,
            _ => {}
        }
    }
}

fn inventory_declaration_shapes<'i: 'a, 'a>(
    declarations: impl Iterator<Item = &'a Property<'i>>,
    metrics: &mut Metrics,
) {
    for declaration in declarations {
        match declaration {
            Property::Unparsed(_) => metrics.unparsed_declarations += 1,
            Property::Custom(_) => metrics.custom_declarations += 1,
            _ => metrics.typed_declarations += 1,
        }
    }
}

fn observe_container_condition(
    observations: &mut Vec<FeatureObservation>,
    condition: Option<&ContainerCondition<'_>>,
    loc: Location,
) -> bool {
    let marker = || SourceMarker::Token("@container".into());
    match condition {
        Some(ContainerCondition::Style(_)) => {
            observe_feature(observations, "container-style-queries", loc, marker());
            false
        }
        Some(ContainerCondition::ScrollState(_)) => {
            observe_feature(
                observations,
                "container-scroll-state-queries",
                loc,
                marker(),
            );
            false
        }
        Some(ContainerCondition::Not(condition)) => {
            observe_container_condition(observations, Some(condition), loc)
        }
        Some(ContainerCondition::Operation { conditions, .. }) => {
            let mut unclassified = false;
            for condition in conditions {
                if observe_container_condition(observations, Some(condition), loc) {
                    unclassified = true;
                }
            }
            unclassified
        }
        Some(ContainerCondition::Feature(feature)) => {
            if container_feature_is_standard(feature) {
                observe_feature(observations, "container-queries", loc, marker());
                false
            } else {
                true
            }
        }
        Some(ContainerCondition::Unknown(_)) | None => true,
    }
}

fn container_feature_is_standard(
    feature: &lightningcss::rules::container::ContainerSizeFeature<'_>,
) -> bool {
    let name = match feature {
        QueryFeature::Plain { name, .. }
        | QueryFeature::Boolean { name }
        | QueryFeature::Range { name, .. }
        | QueryFeature::Interval { name, .. } => name,
    };
    matches!(name, MediaFeatureName::Standard(_))
}

fn observe_feature(
    observations: &mut Vec<FeatureObservation>,
    id: &'static str,
    loc: Location,
    marker: SourceMarker,
) {
    if let Some(existing) = observations.iter_mut().find(|item| item.id == id) {
        existing.occurrences += 1;
    } else {
        observations.push(FeatureObservation {
            id,
            loc,
            marker,
            occurrences: 1,
        });
    }
}

fn observe_unclassified(
    observations: &mut Vec<UnclassifiedObservation>,
    syntax: &str,
    marker: &str,
    loc: Location,
) {
    if let Some(existing) = observations.iter_mut().find(|item| item.syntax == syntax) {
        existing.occurrences += 1;
    } else {
        observations.push(UnclassifiedObservation {
            syntax: syntax.to_owned(),
            marker: marker.to_owned(),
            loc,
            occurrences: 1,
        });
    }
}

/// Maps a parsed layer location back to the exact `@layer` token in authored CSS.
///
/// # Errors
///
/// Returns an error when the resulting source contract is invalid.
pub fn css_layer_finding_source(
    logical_path: &str,
    css: &str,
    location: Location,
) -> Result<FindingSource, String> {
    source_for_location(
        logical_path,
        css,
        location,
        &SourceMarker::Token("@layer".into()),
    )
}

fn source_for_location(
    logical_path: &str,
    css: &str,
    loc: Location,
    marker: &SourceMarker,
) -> Result<FindingSource, String> {
    let (byte_start, one_char_end, line, column) = locate(css, loc.line, loc.column);
    let byte_end = match marker {
        SourceMarker::Token(token) if css[byte_start..].starts_with(token) => {
            byte_start + token.len()
        }
        SourceMarker::Selector => css[byte_start..]
            .find('{')
            .map(|relative| {
                let raw_end = byte_start + relative;
                byte_start
                    + css[byte_start..raw_end]
                        .trim_end_matches(char::is_whitespace)
                        .len()
            })
            .filter(|end| *end > byte_start)
            .unwrap_or(one_char_end),
        SourceMarker::Token(_) => one_char_end,
    };
    let (end_line, end_column) = advance_position(line, column, &css[byte_start..byte_end]);
    FindingSource::new(logical_path, byte_start, byte_end)
        .and_then(|source| source.with_position(line, column, end_line, end_column))
        .map_err(|error| error.to_string())
}

fn format_specificity(value: u32) -> String {
    format!(
        "{},{},{}",
        value >> 20,
        (value >> 10) & 0x3ff,
        value & 0x3ff
    )
}

fn end_position(css: &str) -> (usize, usize) {
    advance_position(1, 1, css)
}

fn advance_position(mut line: usize, mut column: usize, value: &str) -> (usize, usize) {
    for character in value.chars() {
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn locate(
    css: &str,
    zero_based_line: u32,
    one_based_utf16_column: u32,
) -> (usize, usize, usize, usize) {
    let target_line = usize::try_from(zero_based_line).unwrap_or(usize::MAX);
    let mut line_start = 0;
    let mut line_text = "";
    for (index, line) in css.split_inclusive('\n').enumerate() {
        if index == target_line {
            line_text = line.strip_suffix('\n').unwrap_or(line);
            break;
        }
        line_start += line.len();
    }
    if target_line > 0 && line_text.is_empty() && line_start >= css.len() {
        return (css.len(), css.len(), target_line + 1, 1);
    }
    let target_units = one_based_utf16_column.saturating_sub(1);
    let mut units = 0_u32;
    let mut relative = line_text.len();
    let mut scalar_column = 1;
    for (index, character) in line_text.char_indices() {
        if units >= target_units {
            relative = index;
            break;
        }
        let width = if character.len_utf16() == 1 { 1 } else { 2 };
        units = units.saturating_add(width);
        scalar_column += 1;
    }
    let byte_start = (line_start + relative).min(css.len());
    let byte_end = css[byte_start..]
        .chars()
        .next()
        .map_or(byte_start, |character| byte_start + character.len_utf8());
    (byte_start, byte_end, target_line + 1, scalar_column)
}
