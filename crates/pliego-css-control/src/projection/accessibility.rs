//! Deterministic accessibility projection over canonical policy, token graph, and CSS ASTs.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use lightningcss::properties::{Property, PropertyId};
use lightningcss::rules::{CssRule, CssRuleList, Location};
use lightningcss::stylesheet::{ParserFlags, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::traits::ToCss;
use lightningcss::values::color::{ColorGamut, CssColor, SRGB};
use pliego_css_build::artifacts::{
    Finding, FindingCause, FindingEvidence, FindingException, FindingSeverity, FindingSource,
    FindingVerification,
};
use pliego_css_config::{TokenGraph, TokenGraphTheme};
use sha2::{Digest, Sha256};

use crate::accessibility::{
    AccessibilityCheck, AccessibilityColorEndpoint, AccessibilityContrastPair,
    AccessibilityEnforcement, AccessibilityPolicy, parse_accessibility_policy,
};

/// One CSS artifact reparsed inside the accessibility analysis boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessibilityStylesheetInput<'a> {
    /// Portable project-relative path used in finding source ranges and subject identities.
    pub logical_path: &'a str,
    /// Exact authored or generated CSS text being audited.
    pub css: &'a str,
}

/// Exact accessibility policy input and its portable source identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessibilityPolicySource<'a> {
    /// Portable project-relative policy path.
    pub logical_path: &'a str,
    /// Exact policy JSON bytes.
    pub bytes: &'a [u8],
}

/// Accessibility findings and the resulting policy-gate decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessibilityAuditOutcome {
    /// Canonical finding values ready for the shared finding document.
    pub findings: Vec<Finding>,
    /// False only when an unexcepted result is configured as `fail`.
    pub passed: bool,
    /// Number of declared contrast relationships, independent of theme fan-out.
    pub contrast_pairs: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ResultClass {
    Pass,
    Violation,
    Unverified,
    ManualRequired,
}

impl ResultClass {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Violation => "violation",
            Self::Unverified => "unverified",
            Self::ManualRequired => "manual-required",
        }
    }

    const fn verification(self) -> FindingVerification {
        match self {
            Self::Pass | Self::Violation => FindingVerification::Verified,
            Self::Unverified => FindingVerification::Unverified,
            Self::ManualRequired => FindingVerification::ManualRequired,
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::Pass => 0,
            Self::Unverified => 1,
            Self::ManualRequired => 2,
            Self::Violation => 3,
        }
    }
}

struct FindingSpec {
    check: AccessibilityCheck,
    class: ResultClass,
    subject_id: String,
    message: String,
    rule: &'static str,
    explanation: &'static str,
    source: Option<FindingSource>,
    context: BTreeMap<String, String>,
    evidence: Vec<FindingEvidence>,
}

/// Reparse every CSS input and evaluate the closed accessibility policy without a wall clock.
///
/// # Errors
///
/// Returns an error for invalid policy, input identity, source range, or finding data.
#[allow(clippy::too_many_lines)]
pub fn evaluate_accessibility(
    stylesheets: &[AccessibilityStylesheetInput<'_>],
    policy_source: AccessibilityPolicySource<'_>,
    token_graph: Option<&TokenGraph>,
) -> Result<AccessibilityAuditOutcome, String> {
    let policy =
        parse_accessibility_policy(policy_source.bytes).map_err(|error| error.to_string())?;
    validate_input_source(policy_source.logical_path, policy_source.bytes.len())?;

    let mut inputs = stylesheets.to_vec();
    inputs.sort_by(|left, right| left.logical_path.cmp(right.logical_path));
    if inputs
        .windows(2)
        .any(|window| window[0].logical_path == window[1].logical_path)
    {
        return Err("an accessibility stylesheet logical path was supplied more than once".into());
    }
    for input in &inputs {
        validate_input_source(input.logical_path, input.css.len())?;
    }

    let mut findings = Vec::new();
    let mut passed = true;
    let mut used_exceptions = BTreeSet::new();

    evaluate_contrast(
        &policy,
        policy_source,
        token_graph,
        &mut used_exceptions,
        &mut findings,
        &mut passed,
    )?;

    let mut snapshots = Vec::new();
    for input in &inputs {
        match StyleSheet::parse(
            input.css,
            ParserOptions {
                filename: input.logical_path.to_owned(),
                flags: ParserFlags::NESTING,
                ..ParserOptions::default()
            },
        ) {
            Ok(stylesheet) => collect_css_rules(
                &stylesheet.rules,
                input.logical_path,
                input.css,
                MotionContext::Unrestricted,
                false,
                &mut snapshots,
            )?,
            Err(error) => emit_css_parse_findings(
                &policy,
                input,
                &single_line(&error.to_string()),
                &mut used_exceptions,
                &mut findings,
                &mut passed,
            )?,
        }
    }
    snapshots.sort_by(|left, right| {
        (
            left.logical_path.as_str(),
            left.byte_start,
            left.byte_end,
            left.selector.as_str(),
        )
            .cmp(&(
                right.logical_path.as_str(),
                right.byte_start,
                right.byte_end,
                right.selector.as_str(),
            ))
    });

    evaluate_motion(
        &policy,
        &snapshots,
        &mut used_exceptions,
        &mut findings,
        &mut passed,
    )?;
    evaluate_focus_visibility(
        &policy,
        &snapshots,
        &mut used_exceptions,
        &mut findings,
        &mut passed,
    )?;
    evaluate_forced_colors(
        &policy,
        &snapshots,
        &mut used_exceptions,
        &mut findings,
        &mut passed,
    )?;
    evaluate_input_modality(
        &policy,
        &snapshots,
        &mut used_exceptions,
        &mut findings,
        &mut passed,
    )?;

    emit_unmatched_exceptions(&policy, policy_source, &used_exceptions, &mut findings)?;
    emit_summary(&policy, passed, &mut findings)?;

    Ok(AccessibilityAuditOutcome {
        findings,
        passed,
        contrast_pairs: policy.contrast_pair_count(),
    })
}

fn validate_input_source(logical_path: &str, bytes: usize) -> Result<(), String> {
    FindingSource::new(logical_path, 0, bytes)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn evaluate_contrast(
    policy: &AccessibilityPolicy,
    policy_source: AccessibilityPolicySource<'_>,
    token_graph: Option<&TokenGraph>,
    used_exceptions: &mut BTreeSet<String>,
    findings: &mut Vec<Finding>,
    passed: &mut bool,
) -> Result<(), String> {
    for pair in policy.contrast_pairs() {
        match token_graph {
            Some(graph) => {
                if let Some(selections) = pair.selections() {
                    if let Some(theme) = graph.theme(selections) {
                        evaluate_contrast_theme(
                            policy,
                            policy_source,
                            pair,
                            Some(theme),
                            &theme.name,
                            &theme.selections,
                            used_exceptions,
                            findings,
                            passed,
                        )?;
                    } else {
                        emit_missing_contrast_theme(
                            policy,
                            policy_source,
                            pair,
                            selections,
                            used_exceptions,
                            findings,
                            passed,
                        )?;
                    }
                } else if graph.themes.is_empty() {
                    emit_missing_contrast_graph(
                        policy,
                        policy_source,
                        pair,
                        "the token graph contains no resolved themes",
                        used_exceptions,
                        findings,
                        passed,
                    )?;
                } else {
                    let mut themes = graph.themes.iter().collect::<Vec<_>>();
                    themes.sort_by(|left, right| left.name.cmp(&right.name));
                    for theme in themes {
                        evaluate_contrast_theme(
                            policy,
                            policy_source,
                            pair,
                            Some(theme),
                            &theme.name,
                            &theme.selections,
                            used_exceptions,
                            findings,
                            passed,
                        )?;
                    }
                }
            }
            None if pair.selections().is_none() && endpoints_are_literals(pair) => {
                evaluate_contrast_theme(
                    policy,
                    policy_source,
                    pair,
                    None,
                    "literal",
                    &BTreeMap::new(),
                    used_exceptions,
                    findings,
                    passed,
                )?;
            }
            None => emit_missing_contrast_graph(
                policy,
                policy_source,
                pair,
                "a token graph is required by this contrast relationship",
                used_exceptions,
                findings,
                passed,
            )?,
        }
    }
    Ok(())
}

fn endpoints_are_literals(pair: &AccessibilityContrastPair) -> bool {
    matches!(
        pair.foreground(),
        AccessibilityColorEndpoint::Literal { .. }
    ) && matches!(
        pair.background(),
        AccessibilityColorEndpoint::Literal { .. }
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn evaluate_contrast_theme(
    policy: &AccessibilityPolicy,
    policy_source: AccessibilityPolicySource<'_>,
    pair: &AccessibilityContrastPair,
    theme: Option<&TokenGraphTheme>,
    theme_name: &str,
    selections: &BTreeMap<String, String>,
    used_exceptions: &mut BTreeSet<String>,
    findings: &mut Vec<Finding>,
    passed: &mut bool,
) -> Result<(), String> {
    let source = policy_item_source(policy_source, "contrastPairs", pair.id())?;
    let foreground = resolve_endpoint(pair.foreground(), theme);
    let background = resolve_endpoint(pair.background(), theme);
    let foreground_claim = color_resolution_subject_value(&foreground);
    let background_claim = color_resolution_subject_value(&background);

    let (foreground, background) = match (foreground, background) {
        (Ok(foreground), Ok(background)) => (foreground, background),
        (left, right) => {
            let (class, reason) = merge_color_resolution_errors(left.err(), right.err());
            let subject_id = contrast_subject_id(
                pair,
                selections,
                theme_name,
                class,
                &foreground_claim,
                &background_claim,
                &reason,
            )?;
            return emit_spec(
                policy,
                FindingSpec {
                    check: AccessibilityCheck::Contrast,
                    class,
                    subject_id: subject_id.clone(),
                    message: format!(
                        "Contrast pair `{}` could not be verified for theme `{theme_name}`: {reason}.",
                        pair.id()
                    ),
                    rule: "contrast-ratio",
                    explanation: "Explicit foreground/background relationships must have complete, statically resolved color evidence in every selected theme.",
                    source: Some(source),
                    context: contrast_context(pair, theme_name, selections, &subject_id)?,
                    evidence: vec![evidence(
                        "analysis",
                        "resolution",
                        &reason,
                        "token-graph",
                        None,
                    )?],
                },
                used_exceptions,
                findings,
                passed,
            );
        }
    };

    let foreground_claim = format!("resolved:{foreground}");
    let background_claim = format!("resolved:{background}");
    let foreground = parse_resolved_color(&foreground);
    let background = parse_resolved_color(&background);
    let (foreground, background) = match (foreground, background) {
        (Ok(foreground), Ok(background)) => (foreground, background),
        (left, right) => {
            let (class, reason) = merge_color_resolution_errors(left.err(), right.err());
            let subject_id = contrast_subject_id(
                pair,
                selections,
                theme_name,
                class,
                &foreground_claim,
                &background_claim,
                &reason,
            )?;
            return emit_spec(
                policy,
                FindingSpec {
                    check: AccessibilityCheck::Contrast,
                    class,
                    subject_id: subject_id.clone(),
                    message: format!(
                        "Contrast pair `{}` could not be verified for theme `{theme_name}`: {reason}.",
                        pair.id()
                    ),
                    rule: "contrast-ratio",
                    explanation: "Contrast evidence is verified only for opaque, in-gamut, statically resolved sRGB colors.",
                    source: Some(source),
                    context: contrast_context(pair, theme_name, selections, &subject_id)?,
                    evidence: vec![evidence(
                        "analysis",
                        "color-resolution",
                        &reason,
                        "lightningcss",
                        None,
                    )?],
                },
                used_exceptions,
                findings,
                passed,
            );
        }
    };

    let ratio = contrast_ratio(foreground, background);
    let minimum = f64::from(pair.minimum_ratio_milli());
    let class = if ratio * 1000.0 >= minimum {
        ResultClass::Pass
    } else {
        ResultClass::Violation
    };
    let subject_id = contrast_subject_id(
        pair,
        selections,
        theme_name,
        class,
        &foreground_claim,
        &background_claim,
        &format!("ratio:{ratio:.12}"),
    )?;
    let relation = if class == ResultClass::Pass {
        "meets"
    } else {
        "does not meet"
    };
    emit_spec(
        policy,
        FindingSpec {
            check: AccessibilityCheck::Contrast,
            class,
            subject_id: subject_id.clone(),
            message: format!(
                "Contrast pair `{}` {relation} its {:.3}:1 minimum in theme `{theme_name}`.",
                pair.id(),
                minimum / 1000.0
            ),
            rule: "contrast-ratio",
            explanation: "WCAG 2.2 relative luminance and contrast are evaluated for every explicitly selected resolved theme.",
            source: Some(source),
            context: contrast_context(pair, theme_name, selections, &subject_id)?,
            evidence: vec![
                evidence(
                    "measurement",
                    "contrast-ratio",
                    &format!("{ratio:.9}"),
                    "wcag-22",
                    Some("ratio"),
                )?,
                evidence(
                    "policy",
                    "minimum-ratio",
                    &pair.minimum_ratio_milli().to_string(),
                    "accessibility-policy",
                    Some("milli-ratio"),
                )?,
            ],
        },
        used_exceptions,
        findings,
        passed,
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_missing_contrast_theme(
    policy: &AccessibilityPolicy,
    policy_source: AccessibilityPolicySource<'_>,
    pair: &AccessibilityContrastPair,
    selections: &BTreeMap<String, String>,
    used_exceptions: &mut BTreeSet<String>,
    findings: &mut Vec<Finding>,
    passed: &mut bool,
) -> Result<(), String> {
    let subject_id = contrast_subject_id(
        pair,
        selections,
        "missing-theme",
        ResultClass::Unverified,
        pair.foreground().display_value(),
        pair.background().display_value(),
        "theme-selection-absent",
    )?;
    let context = contrast_context(pair, "missing-theme", selections, &subject_id)?;
    emit_spec(
        policy,
        FindingSpec {
            check: AccessibilityCheck::Contrast,
            class: ResultClass::Unverified,
            subject_id,
            message: format!(
                "Contrast pair `{}` selects a theme permutation absent from the token graph.",
                pair.id()
            ),
            rule: "contrast-ratio",
            explanation: "Exact resolver selections must resolve to one graph theme before contrast can be verified.",
            source: Some(policy_item_source(
                policy_source,
                "contrastPairs",
                pair.id(),
            )?),
            context,
            evidence: vec![evidence(
                "analysis",
                "theme-selection",
                &serde_json::to_string(selections).map_err(|error| error.to_string())?,
                "token-graph",
                None,
            )?],
        },
        used_exceptions,
        findings,
        passed,
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_missing_contrast_graph(
    policy: &AccessibilityPolicy,
    policy_source: AccessibilityPolicySource<'_>,
    pair: &AccessibilityContrastPair,
    reason: &str,
    used_exceptions: &mut BTreeSet<String>,
    findings: &mut Vec<Finding>,
    passed: &mut bool,
) -> Result<(), String> {
    let selections = pair.selections().cloned().unwrap_or_default();
    let subject_id = contrast_subject_id(
        pair,
        &selections,
        "missing-graph",
        ResultClass::Unverified,
        pair.foreground().display_value(),
        pair.background().display_value(),
        reason,
    )?;
    let context = contrast_context(pair, "missing-graph", &selections, &subject_id)?;
    emit_spec(
        policy,
        FindingSpec {
            check: AccessibilityCheck::Contrast,
            class: ResultClass::Unverified,
            subject_id,
            message: format!("Contrast pair `{}` is unverified: {reason}.", pair.id()),
            rule: "contrast-ratio",
            explanation: "Token-backed contrast requires the canonical resolved token graph; missing evidence cannot be treated as compliance.",
            source: Some(policy_item_source(
                policy_source,
                "contrastPairs",
                pair.id(),
            )?),
            context,
            evidence: vec![evidence(
                "analysis",
                "token-graph",
                reason,
                "accessibility-audit",
                None,
            )?],
        },
        used_exceptions,
        findings,
        passed,
    )
}

fn contrast_context(
    pair: &AccessibilityContrastPair,
    theme_name: &str,
    selections: &BTreeMap<String, String>,
    subject_id: &str,
) -> Result<BTreeMap<String, String>, String> {
    Ok(BTreeMap::from([
        (
            "background".into(),
            pair.background().display_value().to_owned(),
        ),
        (
            "foreground".into(),
            pair.foreground().display_value().to_owned(),
        ),
        ("pair".into(), pair.id().to_owned()),
        ("subject-id".into(), subject_id.to_owned()),
        ("theme".into(), theme_name.to_owned()),
        (
            "theme-selections".into(),
            serde_json::to_string(selections).map_err(|error| error.to_string())?,
        ),
    ]))
}

fn contrast_subject_id(
    pair: &AccessibilityContrastPair,
    selections: &BTreeMap<String, String>,
    theme_name: &str,
    class: ResultClass,
    foreground_evidence: &str,
    background_evidence: &str,
    outcome_evidence: &str,
) -> Result<String, String> {
    let selections = serde_json::to_string(selections).map_err(|error| error.to_string())?;
    let foreground = serde_json::to_string(pair.foreground()).map_err(|error| error.to_string())?;
    let background = serde_json::to_string(pair.background()).map_err(|error| error.to_string())?;
    let minimum = pair.minimum_ratio_milli().to_string();
    Ok(subject_id(&[
        AccessibilityCheck::Contrast.as_str(),
        pair.id(),
        &foreground,
        &background,
        &minimum,
        &selections,
        theme_name,
        class.as_str(),
        foreground_evidence,
        background_evidence,
        outcome_evidence,
    ]))
}

fn color_resolution_subject_value(value: &Result<String, ColorResolutionError>) -> String {
    match value {
        Ok(value) => format!("resolved:{value}"),
        Err(error) => format!("{}:{}", error.class.as_str(), error.reason),
    }
}

fn resolve_endpoint(
    endpoint: &AccessibilityColorEndpoint,
    theme: Option<&TokenGraphTheme>,
) -> Result<String, ColorResolutionError> {
    match endpoint {
        AccessibilityColorEndpoint::Literal { value } => Ok(value.clone()),
        AccessibilityColorEndpoint::Token { name } => {
            let theme = theme.ok_or_else(|| {
                ColorResolutionError::unverified(format!(
                    "token `{name}` has no resolved theme evidence"
                ))
            })?;
            let token_name = name.strip_prefix("color.").unwrap_or(name);
            theme
                .tokens
                .iter()
                .find(|token| token.kind == "color" && token.name == token_name)
                .map(|token| token.value.clone())
                .ok_or_else(|| {
                    ColorResolutionError::unverified(format!(
                        "token `{name}` is absent from theme `{}`",
                        theme.name
                    ))
                })
        }
    }
}

#[derive(Debug)]
struct ColorResolutionError {
    class: ResultClass,
    reason: String,
}

impl ColorResolutionError {
    fn unverified(reason: impl Into<String>) -> Self {
        Self {
            class: ResultClass::Unverified,
            reason: reason.into(),
        }
    }

    fn manual(reason: impl Into<String>) -> Self {
        Self {
            class: ResultClass::ManualRequired,
            reason: reason.into(),
        }
    }
}

fn merge_color_resolution_errors(
    left: Option<ColorResolutionError>,
    right: Option<ColorResolutionError>,
) -> (ResultClass, String) {
    let mut errors = [left, right].into_iter().flatten().collect::<Vec<_>>();
    errors.sort_by(|left, right| left.reason.cmp(&right.reason));
    let class = errors
        .iter()
        .max_by_key(|error| error.class.rank())
        .map_or(ResultClass::Unverified, |error| error.class);
    let reason = errors
        .into_iter()
        .map(|error| error.reason)
        .collect::<Vec<_>>()
        .join("; ");
    (class, reason)
}

// Exact equality is intentional: automatic contrast proof requires full opacity.
#[allow(clippy::float_cmp)]
fn parse_resolved_color(value: &str) -> Result<SRGB, ColorResolutionError> {
    let property = Property::parse_string(PropertyId::Color, value, ParserOptions::default())
        .map_err(|error| {
            ColorResolutionError::unverified(format!(
                "`{value}` is not parseable as CSS color: {}",
                single_line(&error.to_string())
            ))
        })?;
    let color = match property {
        Property::Color(color) => color,
        Property::Unparsed(_) if contains_dynamic_css(value) => {
            return Err(ColorResolutionError::manual(format!(
                "`{value}` requires runtime CSS resolution"
            )));
        }
        Property::Unparsed(_) => {
            return Err(ColorResolutionError::unverified(format!(
                "`{value}` did not resolve to a typed CSS color"
            )));
        }
        _ => {
            return Err(ColorResolutionError::unverified(format!(
                "`{value}` did not parse as the color property"
            )));
        }
    };
    if matches!(
        color,
        CssColor::CurrentColor | CssColor::LightDark(..) | CssColor::System(..)
    ) {
        return Err(ColorResolutionError::manual(format!(
            "`{value}` depends on runtime color context"
        )));
    }
    let color = SRGB::try_from(&color).map_err(|()| {
        ColorResolutionError::manual(format!("`{value}` could not be dynamically resolved"))
    })?;
    if ![color.r, color.g, color.b, color.alpha]
        .into_iter()
        .all(f32::is_finite)
    {
        return Err(ColorResolutionError::unverified(format!(
            "`{value}` contains unresolved color channels"
        )));
    }
    if color.alpha != 1.0 {
        return Err(ColorResolutionError::unverified(format!(
            "`{value}` is not fully opaque"
        )));
    }
    if !color.in_gamut() {
        return Err(ColorResolutionError::unverified(format!(
            "`{value}` is outside the sRGB gamut"
        )));
    }
    Ok(color)
}

fn contrast_ratio(foreground: SRGB, background: SRGB) -> f64 {
    let foreground = relative_luminance(foreground);
    let background = relative_luminance(background);
    (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05)
}

fn relative_luminance(color: SRGB) -> f64 {
    0.2126 * linear_channel(color.r)
        + 0.7152 * linear_channel(color.g)
        + 0.0722 * linear_channel(color.b)
}

fn linear_channel(value: f32) -> f64 {
    let value = f64::from(value);
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CssDeclaration {
    name: String,
    value: String,
    important: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MotionContext {
    Unrestricted,
    Reduce,
    NoPreference,
    Ambiguous,
}

impl MotionContext {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unrestricted => "unrestricted",
            Self::Reduce => "reduce",
            Self::NoPreference => "no-preference",
            Self::Ambiguous => "ambiguous",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CssSnapshot {
    logical_path: String,
    selector: String,
    selector_group: String,
    declarations: Vec<CssDeclaration>,
    motion_context: MotionContext,
    structural_ambiguity: bool,
    source: FindingSource,
    byte_start: usize,
    byte_end: usize,
}

fn collect_css_rules(
    rules: &CssRuleList<'_>,
    logical_path: &str,
    css: &str,
    motion_context: MotionContext,
    structural_ambiguity: bool,
    output: &mut Vec<CssSnapshot>,
) -> Result<(), String> {
    for rule in &rules.0 {
        match rule {
            CssRule::Style(rule) => {
                collect_style_rule(
                    rule,
                    logical_path,
                    css,
                    motion_context,
                    structural_ambiguity,
                    output,
                )?;
                collect_css_rules(&rule.rules, logical_path, css, motion_context, true, output)?;
            }
            CssRule::Nesting(rule) => {
                collect_style_rule(&rule.style, logical_path, css, motion_context, true, output)?;
                collect_css_rules(
                    &rule.style.rules,
                    logical_path,
                    css,
                    motion_context,
                    true,
                    output,
                )?;
            }
            CssRule::Media(rule) => {
                let query = canonical_css(&rule.query, "media query")?;
                collect_css_rules(
                    &rule.rules,
                    logical_path,
                    css,
                    motion_context_for_query(motion_context, &query),
                    structural_ambiguity,
                    output,
                )?;
            }
            CssRule::Supports(rule) => {
                collect_css_rules(&rule.rules, logical_path, css, motion_context, true, output)?;
            }
            CssRule::MozDocument(rule) => {
                collect_css_rules(&rule.rules, logical_path, css, motion_context, true, output)?;
            }
            CssRule::LayerBlock(rule) => {
                collect_css_rules(&rule.rules, logical_path, css, motion_context, true, output)?;
            }
            CssRule::Container(rule) => {
                collect_css_rules(&rule.rules, logical_path, css, motion_context, true, output)?;
            }
            CssRule::Scope(rule) => {
                collect_css_rules(&rule.rules, logical_path, css, motion_context, true, output)?;
            }
            CssRule::StartingStyle(rule) => {
                collect_css_rules(&rule.rules, logical_path, css, motion_context, true, output)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn collect_style_rule(
    rule: &lightningcss::rules::style::StyleRule<'_>,
    logical_path: &str,
    css: &str,
    motion_context: MotionContext,
    structural_ambiguity: bool,
    output: &mut Vec<CssSnapshot>,
) -> Result<(), String> {
    let (source, byte_start, byte_end) = selector_source(logical_path, css, rule.loc)?;
    let mut effective = BTreeMap::new();
    for (property, important) in rule.declarations.iter() {
        let name = property
            .property_id()
            .to_css_string(minified_printer())
            .map_err(|error| format!("cannot serialize CSS property name: {error}"))?;
        let value = property
            .value_to_css_string(minified_printer())
            .map_err(|error| format!("cannot serialize CSS property value: {error}"))?;
        let name = normalize_property_name(&name);
        let declaration = CssDeclaration {
            name: name.clone(),
            value,
            important,
        };
        effective
            .entry(name)
            .and_modify(|current: &mut CssDeclaration| {
                if important || !current.important {
                    current.clone_from(&declaration);
                }
            })
            .or_insert(declaration);
    }
    let declarations = effective.into_values().collect::<Vec<_>>();
    let selectors = rule
        .selectors
        .0
        .iter()
        .map(|selector| canonical_css(selector, "selector"))
        .collect::<Result<Vec<_>, _>>()?;
    let selector_group = selectors.join(",");
    for selector in selectors {
        output.push(CssSnapshot {
            logical_path: logical_path.to_owned(),
            selector,
            selector_group: selector_group.clone(),
            declarations: declarations.clone(),
            motion_context,
            structural_ambiguity,
            source: source.clone(),
            byte_start,
            byte_end,
        });
    }
    Ok(())
}

fn canonical_css(value: &impl ToCss, label: &str) -> Result<String, String> {
    value
        .to_css_string(minified_printer())
        .map_err(|error| format!("cannot serialize canonical {label}: {error}"))
}

fn minified_printer() -> PrinterOptions<'static> {
    PrinterOptions {
        minify: true,
        ..PrinterOptions::default()
    }
}

fn motion_context_for_query(parent: MotionContext, query: &str) -> MotionContext {
    let child = match query {
        "(prefers-reduced-motion:reduce)" => MotionContext::Reduce,
        "(prefers-reduced-motion:no-preference)" => MotionContext::NoPreference,
        _ if query.contains("prefers-reduced-motion") => MotionContext::Ambiguous,
        _ => return parent,
    };
    match (parent, child) {
        (MotionContext::Unrestricted, child) => child,
        (parent, child) if parent == child => parent,
        _ => MotionContext::Ambiguous,
    }
}

fn emit_css_parse_findings(
    policy: &AccessibilityPolicy,
    input: &AccessibilityStylesheetInput<'_>,
    parse_error: &str,
    used_exceptions: &mut BTreeSet<String>,
    findings: &mut Vec<Finding>,
    passed: &mut bool,
) -> Result<(), String> {
    for check in [
        AccessibilityCheck::Motion,
        AccessibilityCheck::FocusVisibility,
        AccessibilityCheck::ForcedColors,
        AccessibilityCheck::InputModality,
    ] {
        let subject_id = subject_id(&[check.as_str(), input.logical_path, "css-parse", input.css]);
        emit_spec(
            policy,
            FindingSpec {
                check,
                class: ResultClass::Unverified,
                subject_id: subject_id.clone(),
                message: format!(
                    "The `{}` accessibility check is unverified because `{}` could not be parsed.",
                    check.as_str(),
                    input.logical_path
                ),
                rule: "css-parse",
                explanation: "Automatic CSS accessibility checks require a successfully parsed typed stylesheet AST.",
                source: Some(source_for_range(
                    input.logical_path,
                    input.css,
                    0,
                    input.css.len(),
                )?),
                context: BTreeMap::from([
                    ("stylesheet".into(), input.logical_path.to_owned()),
                    ("subject-id".into(), subject_id),
                ]),
                evidence: vec![evidence(
                    "parser",
                    "parse-error",
                    parse_error,
                    "lightningcss",
                    None,
                )?],
            },
            used_exceptions,
            findings,
            passed,
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum MotionFamily {
    Animation,
    Transition,
    Scroll,
}

impl MotionFamily {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Animation => "animation",
            Self::Transition => "transition",
            Self::Scroll => "scroll",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MotionState {
    Active,
    Safe,
    Dynamic,
}

#[allow(clippy::too_many_lines)]
fn evaluate_motion(
    policy: &AccessibilityPolicy,
    snapshots: &[CssSnapshot],
    used_exceptions: &mut BTreeSet<String>,
    findings: &mut Vec<Finding>,
    passed: &mut bool,
) -> Result<(), String> {
    let mut reduced_alternatives = BTreeSet::new();
    let mut related_rules = BTreeMap::new();
    for snapshot in snapshots {
        for family in [
            MotionFamily::Animation,
            MotionFamily::Transition,
            MotionFamily::Scroll,
        ] {
            let state = motion_state(snapshot, family);
            if snapshot.motion_context == MotionContext::Reduce && state == Some(MotionState::Safe)
            {
                reduced_alternatives.insert((
                    snapshot.logical_path.clone(),
                    snapshot.selector.clone(),
                    family,
                ));
            }
            if state.is_some()
                && !(snapshot.motion_context == MotionContext::Reduce
                    && state == Some(MotionState::Safe))
            {
                *related_rules
                    .entry((
                        snapshot.logical_path.clone(),
                        snapshot.selector.clone(),
                        family,
                    ))
                    .or_insert(0_usize) += 1;
            }
        }
    }

    let mut observations = BTreeMap::<String, FindingSpec>::new();
    for snapshot in snapshots {
        for family in [
            MotionFamily::Animation,
            MotionFamily::Transition,
            MotionFamily::Scroll,
        ] {
            let Some(state) = motion_state(snapshot, family) else {
                continue;
            };
            if state == MotionState::Safe {
                continue;
            }
            let has_alternative = reduced_alternatives.contains(&(
                snapshot.logical_path.clone(),
                snapshot.selector.clone(),
                family,
            ));
            let has_competitor = related_rules
                .get(&(
                    snapshot.logical_path.clone(),
                    snapshot.selector.clone(),
                    family,
                ))
                .is_some_and(|count| *count > 1);
            let class = match (state, snapshot.motion_context) {
                (MotionState::Dynamic, _) | (_, MotionContext::Ambiguous) => {
                    ResultClass::ManualRequired
                }
                (MotionState::Active, MotionContext::Reduce) => ResultClass::Violation,
                _ if snapshot.structural_ambiguity || snapshot.selector.contains('&') => {
                    ResultClass::ManualRequired
                }
                _ if has_competitor => ResultClass::ManualRequired,
                (MotionState::Active, MotionContext::NoPreference) => ResultClass::Pass,
                (MotionState::Active, MotionContext::Unrestricted) if has_alternative => {
                    ResultClass::ManualRequired
                }
                (MotionState::Active, MotionContext::Unrestricted) => ResultClass::Violation,
                _ => unreachable!(),
            };
            let correlation =
                format!("reduced-alternative={has_alternative};competing-rule={has_competitor}");
            let subject = css_subject_id(
                AccessibilityCheck::Motion,
                snapshot,
                family.as_str(),
                class,
                &correlation,
            );
            let message = match class {
                ResultClass::Pass => format!(
                    "Selector `{}` enables {} only under exact no-preference scoping.",
                    snapshot.selector,
                    family.as_str()
                ),
                ResultClass::Violation if snapshot.motion_context == MotionContext::Reduce => {
                    format!(
                        "Selector `{}` enables {} while reduced motion is requested.",
                        snapshot.selector,
                        family.as_str()
                    )
                }
                ResultClass::Violation => format!(
                    "Selector `{}` enables {} without exact no-preference scoping.",
                    snapshot.selector,
                    family.as_str()
                ),
                ResultClass::ManualRequired => format!(
                    "Selector `{}` requires manual {} verification; its value or context is not an exact proof.",
                    snapshot.selector,
                    family.as_str()
                ),
                ResultClass::Unverified => unreachable!(),
            };
            insert_observation(
                &mut observations,
                FindingSpec {
                    check: AccessibilityCheck::Motion,
                    class,
                    subject_id: subject.clone(),
                    message,
                    rule: "reduced-motion",
                    explanation: "User-visible animation, transition, and smooth scrolling require a statically verifiable reduced-motion alternative.",
                    source: Some(snapshot.source.clone()),
                    context: BTreeMap::from([
                        ("family".into(), family.as_str().into()),
                        (
                            "motion-context".into(),
                            snapshot.motion_context.as_str().into(),
                        ),
                        ("selector".into(), snapshot.selector.clone()),
                        ("stylesheet".into(), snapshot.logical_path.clone()),
                        ("subject-id".into(), subject),
                    ]),
                    evidence: vec![evidence(
                        "css-ast",
                        "declarations",
                        &declaration_evidence(snapshot, family.as_str()),
                        "lightningcss",
                        None,
                    )?],
                },
            );
        }
    }
    emit_observations(policy, observations, used_exceptions, findings, passed)
}

fn motion_state(snapshot: &CssSnapshot, family: MotionFamily) -> Option<MotionState> {
    let declaration = |name: &str| {
        snapshot
            .declarations
            .iter()
            .find(|declaration| declaration.name == name)
            .map(|declaration| declaration.value.as_str())
    };
    match family {
        MotionFamily::Animation => {
            let shorthand = declaration("animation");
            let name = declaration("animation-name");
            let duration = declaration("animation-duration");
            if [shorthand, name, duration]
                .into_iter()
                .flatten()
                .any(contains_dynamic_css)
            {
                return Some(MotionState::Dynamic);
            }
            if shorthand.is_some() && (name.is_some() || duration.is_some()) {
                return Some(MotionState::Dynamic);
            }
            if let Some(value) = shorthand {
                return Some(if value.trim() == "none" || !contains_nonzero_time(value) {
                    MotionState::Safe
                } else {
                    MotionState::Active
                });
            }
            match (name, duration) {
                (Some(name), Some(duration))
                    if name.trim() != "none" && contains_nonzero_time(duration) =>
                {
                    Some(MotionState::Active)
                }
                (Some(_), Some(_)) => Some(MotionState::Safe),
                (Some(_), None) | (None, Some(_)) => Some(MotionState::Dynamic),
                _ => None,
            }
        }
        MotionFamily::Transition => {
            let shorthand = declaration("transition");
            let property = declaration("transition-property");
            let duration = declaration("transition-duration");
            if [shorthand, property, duration]
                .into_iter()
                .flatten()
                .any(contains_dynamic_css)
            {
                return Some(MotionState::Dynamic);
            }
            if shorthand.is_some() && (property.is_some() || duration.is_some()) {
                return Some(MotionState::Dynamic);
            }
            if let Some(value) = shorthand {
                return Some(if value.trim() == "none" || !contains_nonzero_time(value) {
                    MotionState::Safe
                } else {
                    MotionState::Active
                });
            }
            match (property, duration) {
                (Some("none"), _) => Some(MotionState::Safe),
                (_, Some(duration)) if contains_nonzero_time(duration) => Some(MotionState::Active),
                (Some(_) | None, Some(_)) => Some(MotionState::Safe),
                (Some(_), None) => Some(MotionState::Dynamic),
                _ => None,
            }
        }
        MotionFamily::Scroll => declaration("scroll-behavior").map(|value| {
            if contains_dynamic_css(value) {
                MotionState::Dynamic
            } else if value.trim() == "smooth" {
                MotionState::Active
            } else {
                MotionState::Safe
            }
        }),
    }
}

fn contains_nonzero_time(value: &str) -> bool {
    value
        .split(|character: char| {
            character.is_ascii_whitespace() || matches!(character, ',' | '/' | '(' | ')')
        })
        .filter_map(|part| {
            let part = part.trim();
            let number = part.strip_suffix("ms").or_else(|| part.strip_suffix('s'))?;
            number.parse::<f64>().ok()
        })
        .any(|number| number != 0.0)
}

fn evaluate_focus_visibility(
    policy: &AccessibilityPolicy,
    snapshots: &[CssSnapshot],
    used_exceptions: &mut BTreeSet<String>,
    findings: &mut Vec<Finding>,
    passed: &mut bool,
) -> Result<(), String> {
    let mut observations = BTreeMap::<String, FindingSpec>::new();
    for snapshot in snapshots
        .iter()
        .filter(|snapshot| selector_has_focus(&snapshot.selector))
    {
        let outlines = snapshot
            .declarations
            .iter()
            .filter(|declaration| {
                matches!(
                    declaration.name.as_str(),
                    "outline" | "outline-width" | "outline-style"
                )
            })
            .collect::<Vec<_>>();
        let class = if outlines
            .iter()
            .any(|declaration| outline_is_suppressed(&declaration.name, &declaration.value))
        {
            ResultClass::Violation
        } else {
            ResultClass::ManualRequired
        };
        let subject = css_subject_id(
            AccessibilityCheck::FocusVisibility,
            snapshot,
            "outline",
            class,
            "focus-rule-local-evidence",
        );
        let message = match class {
            ResultClass::Violation => {
                format!(
                    "Selector `{}` suppresses the focus outline.",
                    snapshot.selector
                )
            }
            ResultClass::ManualRequired => format!(
                "Selector `{}` requires manual focus verification because the full computed cascade was not proven.",
                snapshot.selector
            ),
            ResultClass::Pass | ResultClass::Unverified => unreachable!(),
        };
        insert_observation(
            &mut observations,
            FindingSpec {
                check: AccessibilityCheck::FocusVisibility,
                class,
                subject_id: subject.clone(),
                message,
                rule: "focus-indicator",
                explanation: "Keyboard focus styles must not statically suppress the user-agent outline without independently verified visible replacement evidence.",
                source: Some(snapshot.source.clone()),
                context: BTreeMap::from([
                    ("selector".into(), snapshot.selector.clone()),
                    ("stylesheet".into(), snapshot.logical_path.clone()),
                    ("subject-id".into(), subject),
                ]),
                evidence: vec![evidence(
                    "css-ast",
                    "outline",
                    &if outlines.is_empty() {
                        "not-declared".into()
                    } else {
                        outlines
                            .iter()
                            .map(|declaration| declaration_text(declaration))
                            .collect::<Vec<_>>()
                            .join(";")
                    },
                    "lightningcss",
                    None,
                )?],
            },
        );
    }
    emit_observations(policy, observations, used_exceptions, findings, passed)
}

fn outline_is_suppressed(name: &str, value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    match name {
        "outline" => {
            value == "none"
                || is_zero_css_dimension(&value)
                || value
                    .split_ascii_whitespace()
                    .next()
                    .is_some_and(is_zero_css_dimension)
        }
        "outline-width" => is_zero_css_dimension(&value),
        "outline-style" => value == "none",
        _ => false,
    }
}

fn is_zero_css_dimension(value: &str) -> bool {
    let value = value.trim();
    if value == "0" {
        return true;
    }
    ["px", "em", "rem", "pt", "pc", "in", "cm", "mm", "q"]
        .into_iter()
        .filter_map(|unit| value.strip_suffix(unit))
        .any(|number| number.parse::<f64>().ok() == Some(0.0))
}

fn evaluate_forced_colors(
    policy: &AccessibilityPolicy,
    snapshots: &[CssSnapshot],
    used_exceptions: &mut BTreeSet<String>,
    findings: &mut Vec<Finding>,
    passed: &mut bool,
) -> Result<(), String> {
    let mut observations = BTreeMap::<String, FindingSpec>::new();
    for snapshot in snapshots {
        let Some(declaration) = snapshot
            .declarations
            .iter()
            .find(|declaration| declaration.name == "forced-color-adjust")
        else {
            continue;
        };
        let class = if contains_dynamic_css(&declaration.value) {
            ResultClass::ManualRequired
        } else if declaration.value.trim().eq_ignore_ascii_case("none") {
            ResultClass::Violation
        } else {
            ResultClass::Pass
        };
        let subject = css_subject_id(
            AccessibilityCheck::ForcedColors,
            snapshot,
            "forced-color-adjust",
            class,
            "declaration-local-evidence",
        );
        let message = match class {
            ResultClass::Pass => format!(
                "Selector `{}` preserves forced-colors adaptation.",
                snapshot.selector
            ),
            ResultClass::Violation => format!(
                "Selector `{}` disables forced-colors adaptation with `forced-color-adjust:none`.",
                snapshot.selector
            ),
            ResultClass::ManualRequired => format!(
                "Selector `{}` requires manual forced-colors verification because its value is dynamic.",
                snapshot.selector
            ),
            ResultClass::Unverified => unreachable!(),
        };
        insert_observation(
            &mut observations,
            FindingSpec {
                check: AccessibilityCheck::ForcedColors,
                class,
                subject_id: subject.clone(),
                message,
                rule: "forced-colors",
                explanation: "Disabling user-agent forced-color adjustment can remove required system color adaptation.",
                source: Some(snapshot.source.clone()),
                context: BTreeMap::from([
                    ("selector".into(), snapshot.selector.clone()),
                    ("stylesheet".into(), snapshot.logical_path.clone()),
                    ("subject-id".into(), subject),
                ]),
                evidence: vec![evidence(
                    "css-ast",
                    "forced-color-adjust",
                    &declaration_text(declaration),
                    "lightningcss",
                    None,
                )?],
            },
        );
    }
    emit_observations(policy, observations, used_exceptions, findings, passed)
}

#[allow(clippy::too_many_lines)]
fn evaluate_input_modality(
    policy: &AccessibilityPolicy,
    snapshots: &[CssSnapshot],
    used_exceptions: &mut BTreeSet<String>,
    findings: &mut Vec<Finding>,
    passed: &mut bool,
) -> Result<(), String> {
    let mut observations = BTreeMap::<String, FindingSpec>::new();
    for snapshot in snapshots
        .iter()
        .filter(|snapshot| selector_has_hover(&snapshot.selector))
    {
        let base = interaction_base(&snapshot.selector);
        let mut possible_alternative = false;
        let mut exact_alternative = false;
        let mut candidate_evidence = Vec::new();
        for candidate in snapshots.iter().filter(|candidate| {
            selector_has_focus(&candidate.selector)
                && !selector_has_hover(&candidate.selector)
                && interaction_base(&candidate.selector) == base
        }) {
            possible_alternative = true;
            candidate_evidence.push(format!(
                "{}|{}|{}|{}|{}",
                candidate.logical_path,
                candidate.selector,
                candidate.selector_group,
                candidate.motion_context.as_str(),
                candidate
                    .declarations
                    .iter()
                    .map(declaration_text)
                    .collect::<Vec<_>>()
                    .join(";")
            ));
            if candidate.logical_path == snapshot.logical_path
                && candidate.byte_start == snapshot.byte_start
                && candidate.byte_end == snapshot.byte_end
                && candidate.declarations == snapshot.declarations
                && !candidate.structural_ambiguity
                && !selector_is_ambiguous(&candidate.selector)
            {
                exact_alternative = true;
            }
        }
        let ambiguous = snapshot.structural_ambiguity
            || selector_is_ambiguous(&snapshot.selector)
            || selector_has_focus(&snapshot.selector);
        let class = if ambiguous {
            ResultClass::ManualRequired
        } else if exact_alternative {
            ResultClass::Pass
        } else if possible_alternative {
            ResultClass::ManualRequired
        } else {
            ResultClass::Violation
        };
        candidate_evidence.sort();
        let correlation = format!(
            "possible={possible_alternative};exact={exact_alternative};candidates={}",
            candidate_evidence.join("\n")
        );
        let subject = css_subject_id(
            AccessibilityCheck::InputModality,
            snapshot,
            "hover-equivalence",
            class,
            &correlation,
        );
        let message = match class {
            ResultClass::Pass => format!(
                "Hover selector `{}` has an alternative focus selector in the same style rule.",
                snapshot.selector
            ),
            ResultClass::Violation => format!(
                "Hover selector `{}` has no focus or focus-visible equivalent.",
                snapshot.selector
            ),
            ResultClass::ManualRequired => format!(
                "Functional hover selector `{}` requires manual keyboard-equivalence verification.",
                snapshot.selector
            ),
            ResultClass::Unverified => unreachable!(),
        };
        insert_observation(
            &mut observations,
            FindingSpec {
                check: AccessibilityCheck::InputModality,
                class,
                subject_id: subject.clone(),
                message,
                rule: "input-modality",
                explanation: "Hover-only interaction styling must expose an equivalent keyboard-focus selector or explicit manual evidence.",
                source: Some(snapshot.source.clone()),
                context: BTreeMap::from([
                    ("interaction-base".into(), base),
                    ("selector".into(), snapshot.selector.clone()),
                    ("stylesheet".into(), snapshot.logical_path.clone()),
                    ("subject-id".into(), subject),
                ]),
                evidence: vec![evidence(
                    "css-ast",
                    "selector",
                    &snapshot.selector,
                    "lightningcss",
                    Some("selector"),
                )?],
            },
        );
    }
    emit_observations(policy, observations, used_exceptions, findings, passed)
}

fn selector_has_focus(selector: &str) -> bool {
    selector_has_pseudo(selector, ":focus-visible") || selector_has_pseudo(selector, ":focus")
}

fn selector_has_hover(selector: &str) -> bool {
    selector_has_pseudo(selector, ":hover")
}

fn selector_has_pseudo(selector: &str, pseudo: &str) -> bool {
    !selector_pseudo_positions(selector, pseudo).is_empty()
}

fn selector_is_ambiguous(selector: &str) -> bool {
    selector.contains('&')
        || [":is(", ":where(", ":has(", ":not("]
            .into_iter()
            .any(|pseudo| selector.contains(pseudo))
}

fn interaction_base(selector: &str) -> String {
    [":focus-visible", ":focus", ":hover"]
        .into_iter()
        .fold(selector.to_owned(), |base, pseudo| {
            let mut output = base;
            for start in selector_pseudo_positions(&output, pseudo).into_iter().rev() {
                output.replace_range(start..start + pseudo.len(), "");
            }
            output
        })
}

fn selector_pseudo_positions(selector: &str, pseudo: &str) -> Vec<usize> {
    let (bytes, needle) = (selector.as_bytes(), pseudo.as_bytes());
    let (mut quote, mut brackets, mut escaped) = (None, 0_usize, false);
    let mut positions = Vec::new();
    for index in 0..bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
            continue;
        }
        if byte == b'\\' {
            escaped = true;
            continue;
        }
        if let Some(expected) = quote {
            if byte == expected {
                quote = None;
            }
            continue;
        }
        if matches!(byte, b'\'' | b'"') {
            quote = Some(byte);
            continue;
        }
        match byte {
            b'[' => brackets += 1,
            b']' => brackets = brackets.saturating_sub(1),
            _ if brackets == 0
                && bytes[index..].starts_with(needle)
                && index
                    .checked_sub(1)
                    .is_none_or(|before| bytes[before] != b':')
                && bytes[index + needle.len()..].first().is_none_or(|next| {
                    !next.is_ascii_alphanumeric() && !matches!(next, b'-' | b'_')
                }) =>
            {
                positions.push(index);
            }
            _ => {}
        }
    }
    positions
}

fn insert_observation(observations: &mut BTreeMap<String, FindingSpec>, candidate: FindingSpec) {
    observations
        .entry(candidate.subject_id.clone())
        .and_modify(|existing| {
            if candidate.class.rank() > existing.class.rank() {
                *existing = FindingSpec {
                    check: candidate.check,
                    class: candidate.class,
                    subject_id: candidate.subject_id.clone(),
                    message: candidate.message.clone(),
                    rule: candidate.rule,
                    explanation: candidate.explanation,
                    source: candidate.source.clone(),
                    context: candidate.context.clone(),
                    evidence: candidate.evidence.clone(),
                };
            }
        })
        .or_insert(candidate);
}

fn emit_observations(
    policy: &AccessibilityPolicy,
    observations: BTreeMap<String, FindingSpec>,
    used_exceptions: &mut BTreeSet<String>,
    findings: &mut Vec<Finding>,
    passed: &mut bool,
) -> Result<(), String> {
    for (_, spec) in observations {
        emit_spec(policy, spec, used_exceptions, findings, passed)?;
    }
    Ok(())
}

fn emit_spec(
    policy: &AccessibilityPolicy,
    mut spec: FindingSpec,
    used_exceptions: &mut BTreeSet<String>,
    findings: &mut Vec<Finding>,
    passed: &mut bool,
) -> Result<(), String> {
    spec.context
        .entry("subject-id".into())
        .or_insert_with(|| spec.subject_id.clone());
    spec.context
        .entry("check".into())
        .or_insert_with(|| spec.check.as_str().to_owned());
    let exception = (spec.class != ResultClass::Pass)
        .then(|| {
            policy.exceptions().iter().find(|exception| {
                exception.check() == spec.check && exception.subject_id() == spec.subject_id
            })
        })
        .flatten();
    let excepted = exception.is_some();
    if let Some(exception) = exception {
        used_exceptions.insert(exception.id().to_owned());
    }

    let enforcement = match spec.class {
        ResultClass::Pass => AccessibilityEnforcement::Warn,
        ResultClass::Violation => policy.check_policy(spec.check).violation(),
        ResultClass::Unverified => policy.check_policy(spec.check).unverified(),
        ResultClass::ManualRequired => policy.check_policy(spec.check).manual_required(),
    };
    if spec.class != ResultClass::Pass && !excepted && enforcement == AccessibilityEnforcement::Fail
    {
        *passed = false;
    }
    let severity = if excepted {
        FindingSeverity::Warning
    } else {
        match spec.class {
            ResultClass::Pass => FindingSeverity::Info,
            _ if enforcement == AccessibilityEnforcement::Fail => FindingSeverity::Error,
            _ => FindingSeverity::Warning,
        }
    };
    let message = if excepted {
        format!("Reviewed exception: {}", spec.message)
    } else {
        spec.message
    };
    let cause = FindingCause::new("pliegocss.accessibility.v1", spec.rule, spec.explanation)
        .map_err(|error| error.to_string())?;
    let mut finding = Finding::new(
        finding_code(spec.check, spec.class, excepted),
        "accessibility",
        severity,
        message,
        spec.class.verification(),
        cause,
    )
    .map_err(|error| error.to_string())?;
    if let Some(source) = spec.source {
        finding = finding
            .with_source(source)
            .map_err(|error| error.to_string())?;
    }
    for (name, value) in spec.context {
        finding = finding
            .with_context(name, value)
            .map_err(|error| error.to_string())?;
    }
    for item in spec.evidence {
        finding = finding
            .with_evidence(item)
            .map_err(|error| error.to_string())?;
    }
    if let Some(exception) = exception {
        let reviewed = FindingException::new(exception.id(), exception.justification())
            .map_err(|error| error.to_string())?
            .expiring_on(exception.expires_on())
            .map_err(|error| error.to_string())?;
        finding = finding
            .with_exception(reviewed)
            .map_err(|error| error.to_string())?;
    }
    findings.push(finding);
    Ok(())
}

fn finding_code(check: AccessibilityCheck, class: ResultClass, excepted: bool) -> &'static str {
    if excepted {
        return match check {
            AccessibilityCheck::Contrast => "PCSS-A11Y-102",
            AccessibilityCheck::Motion => "PCSS-A11Y-202",
            AccessibilityCheck::FocusVisibility => "PCSS-A11Y-302",
            AccessibilityCheck::ForcedColors => "PCSS-A11Y-402",
            AccessibilityCheck::InputModality => "PCSS-A11Y-502",
        };
    }
    match (check, class) {
        (AccessibilityCheck::Contrast, ResultClass::Pass) => "PCSS-A11Y-100",
        (AccessibilityCheck::Contrast, ResultClass::Violation) => "PCSS-A11Y-101",
        (AccessibilityCheck::Contrast, ResultClass::Unverified | ResultClass::ManualRequired) => {
            "PCSS-A11Y-108"
        }
        (AccessibilityCheck::Motion, ResultClass::Pass) => "PCSS-A11Y-200",
        (AccessibilityCheck::Motion, ResultClass::Violation) => "PCSS-A11Y-201",
        (AccessibilityCheck::Motion, ResultClass::Unverified) => "PCSS-A11Y-208",
        (AccessibilityCheck::Motion, ResultClass::ManualRequired) => "PCSS-A11Y-209",
        (AccessibilityCheck::FocusVisibility, ResultClass::Pass) => "PCSS-A11Y-300",
        (AccessibilityCheck::FocusVisibility, ResultClass::Violation) => "PCSS-A11Y-301",
        (AccessibilityCheck::FocusVisibility, ResultClass::Unverified) => "PCSS-A11Y-308",
        (AccessibilityCheck::FocusVisibility, ResultClass::ManualRequired) => "PCSS-A11Y-309",
        (AccessibilityCheck::ForcedColors, ResultClass::Pass) => "PCSS-A11Y-400",
        (AccessibilityCheck::ForcedColors, ResultClass::Violation) => "PCSS-A11Y-401",
        (
            AccessibilityCheck::ForcedColors,
            ResultClass::Unverified | ResultClass::ManualRequired,
        ) => "PCSS-A11Y-409",
        (AccessibilityCheck::InputModality, ResultClass::Pass) => "PCSS-A11Y-500",
        (AccessibilityCheck::InputModality, ResultClass::Violation) => "PCSS-A11Y-501",
        (AccessibilityCheck::InputModality, ResultClass::Unverified) => "PCSS-A11Y-508",
        (AccessibilityCheck::InputModality, ResultClass::ManualRequired) => "PCSS-A11Y-509",
    }
}

fn emit_unmatched_exceptions(
    policy: &AccessibilityPolicy,
    policy_source: AccessibilityPolicySource<'_>,
    used_exceptions: &BTreeSet<String>,
    findings: &mut Vec<Finding>,
) -> Result<(), String> {
    for exception in policy
        .exceptions()
        .iter()
        .filter(|exception| !used_exceptions.contains(exception.id()))
    {
        let cause = FindingCause::new(
            "pliegocss.accessibility.v1",
            "exception-match",
            "Reviewed accessibility exceptions must match a current non-pass observation by exact check and subjectId.",
        )
        .map_err(|error| error.to_string())?;
        let mut finding = Finding::new(
            "PCSS-A11Y-999",
            "accessibility",
            FindingSeverity::Warning,
            format!(
                "Accessibility exception `{}` did not match a current non-pass observation.",
                exception.id()
            ),
            FindingVerification::Verified,
            cause,
        )
        .map_err(|error| error.to_string())?
        .with_source(policy_item_source(
            policy_source,
            "exceptions",
            exception.id(),
        )?)
        .map_err(|error| error.to_string())?;
        for (name, value) in [
            ("check", exception.check().as_str()),
            ("exception-id", exception.id()),
            ("subject-id", exception.subject_id()),
        ] {
            finding = finding
                .with_context(name, value)
                .map_err(|error| error.to_string())?;
        }
        findings.push(finding);
    }
    Ok(())
}

fn emit_summary(
    policy: &AccessibilityPolicy,
    passed: bool,
    findings: &mut Vec<Finding>,
) -> Result<(), String> {
    let cause = FindingCause::new(
        "pliegocss.accessibility.v1",
        "audit-summary",
        "The summary reports only the configured static-analysis gate; it is not a global accessibility compliance claim.",
    )
    .map_err(|error| error.to_string())?;
    let mut finding = Finding::new(
        "PCSS-A11Y-000",
        "accessibility",
        FindingSeverity::Info,
        if passed {
            "The configured accessibility static-analysis gate passed."
        } else {
            "The configured accessibility static-analysis gate failed."
        },
        FindingVerification::Verified,
        cause,
    )
    .map_err(|error| error.to_string())?;
    for (name, value) in [
        ("passed", passed.to_string()),
        ("contrast-pairs", policy.contrast_pair_count().to_string()),
        (
            "accessibility-policy-version",
            policy.policy_version().to_string(),
        ),
    ] {
        finding = finding
            .with_context(name, value)
            .map_err(|error| error.to_string())?;
    }
    findings.push(finding);
    Ok(())
}

fn evidence(
    kind: &str,
    name: &str,
    value: &str,
    source: &str,
    unit: Option<&str>,
) -> Result<FindingEvidence, String> {
    let item = FindingEvidence::new(kind, name, single_line(value), source)
        .map_err(|error| error.to_string())?;
    match unit {
        Some(unit) => item.with_unit(unit).map_err(|error| error.to_string()),
        None => Ok(item),
    }
}

fn declaration_evidence(snapshot: &CssSnapshot, prefix: &str) -> String {
    let values = snapshot
        .declarations
        .iter()
        .filter(|declaration| declaration.name.starts_with(prefix))
        .map(declaration_text)
        .collect::<Vec<_>>()
        .join(";");
    if values.is_empty() {
        "not-declared".into()
    } else {
        values
    }
}

fn declaration_text(declaration: &CssDeclaration) -> String {
    format!(
        "{}:{}{}",
        declaration.name,
        declaration.value,
        if declaration.important {
            "!important"
        } else {
            ""
        }
    )
}

fn css_subject_id(
    check: AccessibilityCheck,
    snapshot: &CssSnapshot,
    discriminator: &str,
    class: ResultClass,
    correlation_evidence: &str,
) -> String {
    let declarations = snapshot
        .declarations
        .iter()
        .map(declaration_text)
        .collect::<Vec<_>>()
        .join(";");
    subject_id(&[
        check.as_str(),
        &snapshot.logical_path,
        &snapshot.selector,
        &snapshot.selector_group,
        snapshot.motion_context.as_str(),
        if snapshot.structural_ambiguity {
            "ambiguous-ancestor"
        } else {
            "flat-ancestor"
        },
        discriminator,
        class.as_str(),
        correlation_evidence,
        &declarations,
    ])
}

fn contains_dynamic_css(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    ["var(", "env(", "attr(", "currentcolor", "inherit", "revert"]
        .into_iter()
        .any(|needle| value.contains(needle))
}

fn normalize_property_name(name: &str) -> String {
    let name = name.trim().to_ascii_lowercase();
    for prefix in ["-webkit-", "-moz-", "-ms-", "-o-"] {
        if let Some(unprefixed) = name.strip_prefix(prefix) {
            return unprefixed.to_owned();
        }
    }
    name
}

fn subject_id(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"pliegocss-accessibility-subject-v1\0");
    for part in parts {
        let length = u64::try_from(part.len()).unwrap_or(u64::MAX);
        hasher.update(length.to_be_bytes());
        hasher.update(part.as_bytes());
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(71);
    encoded.push_str("sha256:");
    for byte in digest {
        use fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn policy_item_source(
    policy_source: AccessibilityPolicySource<'_>,
    array_name: &str,
    id: &str,
) -> Result<FindingSource, String> {
    let text = std::str::from_utf8(policy_source.bytes).map_err(|error| error.to_string())?;
    let escaped = serde_json::to_string(id).map_err(|error| error.to_string())?;
    let (start, end) = json_array_span(text, array_name)
        .and_then(|(array_start, array_end)| {
            json_id_span(&text[array_start..array_end], &escaped)
                .map(|(start, end)| (array_start + start, array_start + end))
        })
        .unwrap_or((0, text.len()));
    source_for_range(policy_source.logical_path, text, start, end)
}

fn json_array_span(text: &str, name: &str) -> Option<(usize, usize)> {
    let key = serde_json::to_string(name).ok()?;
    let mut search_start = 0_usize;
    let start = loop {
        let relative = text[search_start..].find(&key)?;
        let key_start = search_start + relative;
        let mut index = skip_json_whitespace(text, key_start + key.len());
        if text.as_bytes().get(index) == Some(&b':') {
            index = skip_json_whitespace(text, index + 1);
            if text.as_bytes().get(index) == Some(&b'[') {
                break index;
            }
        }
        search_start = key_start + key.len();
    };
    let mut depth = 0_usize;
    let mut quote = false;
    let mut escaped = false;
    for (offset, byte) in text.as_bytes()[start..].iter().copied().enumerate() {
        if quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quote = false;
            }
            continue;
        }
        match byte {
            b'"' => quote = true,
            b'[' => depth += 1,
            b']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some((start, start + offset + 1));
                }
            }
            _ => {}
        }
    }
    None
}

fn json_id_span(array: &str, escaped_id: &str) -> Option<(usize, usize)> {
    let mut search_start = 0_usize;
    while let Some(relative) = array[search_start..].find("\"id\"") {
        let key_start = search_start + relative;
        let mut index = skip_json_whitespace(array, key_start + 4);
        if array.as_bytes().get(index) != Some(&b':') {
            search_start = key_start + 4;
            continue;
        }
        index = skip_json_whitespace(array, index + 1);
        if array[index..].starts_with(escaped_id) {
            return Some((index, index + escaped_id.len()));
        }
        search_start = key_start + 4;
    }
    None
}

fn skip_json_whitespace(text: &str, mut index: usize) -> usize {
    while text
        .as_bytes()
        .get(index)
        .is_some_and(u8::is_ascii_whitespace)
    {
        index += 1;
    }
    index
}

fn selector_source(
    logical_path: &str,
    css: &str,
    location: Location,
) -> Result<(FindingSource, usize, usize), String> {
    let start = location_to_byte(css, location)?;
    let brace = find_rule_open_brace(css, start)
        .ok_or_else(|| format!("style rule in `{logical_path}` has no source opening brace"))?;
    let prelude = &css[start..brace];
    let trimmed_start = prelude.len() - prelude.trim_start().len();
    let trimmed_end = prelude.trim_end().len();
    let byte_start = start + trimmed_start;
    let byte_end = start + trimmed_end;
    Ok((
        source_for_range(logical_path, css, byte_start, byte_end)?,
        byte_start,
        byte_end,
    ))
}

fn find_rule_open_brace(css: &str, start: usize) -> Option<usize> {
    let bytes = css.as_bytes();
    let mut index = start;
    let mut quote = None;
    let mut escaped = false;
    let mut comment = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if comment {
            if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                comment = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == delimiter {
                quote = None;
            }
            index += 1;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            comment = true;
            index += 2;
            continue;
        }
        match byte {
            b'\'' | b'"' => quote = Some(byte),
            b'\\' => index = index.saturating_add(1),
            b'{' => return Some(index),
            _ => {}
        }
        index += 1;
    }
    None
}

fn location_to_byte(css: &str, location: Location) -> Result<usize, String> {
    let target_line =
        usize::try_from(location.line).map_err(|_| "CSS source line exceeds usize".to_owned())?;
    let target_utf16 = usize::try_from(location.column.saturating_sub(1))
        .map_err(|_| "CSS source column exceeds usize".to_owned())?;
    let mut line = 0_usize;
    let mut line_start = 0_usize;
    for (offset, byte) in css.bytes().enumerate() {
        if line == target_line {
            break;
        }
        if byte == b'\n' {
            line += 1;
            line_start = offset + 1;
        }
    }
    if line != target_line {
        return Err("Lightning CSS location references a line outside the source".into());
    }
    let line_end = css[line_start..]
        .find('\n')
        .map_or(css.len(), |offset| line_start + offset);
    let mut utf16 = 0_usize;
    for (offset, character) in css[line_start..line_end].char_indices() {
        if utf16 == target_utf16 {
            return Ok(line_start + offset);
        }
        utf16 += character.len_utf16();
        if utf16 > target_utf16 {
            return Err("Lightning CSS location splits a UTF-16 surrogate pair".into());
        }
    }
    if utf16 == target_utf16 {
        Ok(line_end)
    } else {
        Err("Lightning CSS location references a column outside the source".into())
    }
}

fn source_for_range(
    logical_path: &str,
    text: &str,
    byte_start: usize,
    byte_end: usize,
) -> Result<FindingSource, String> {
    if byte_end > text.len()
        || byte_start > byte_end
        || !text.is_char_boundary(byte_start)
        || !text.is_char_boundary(byte_end)
    {
        return Err(format!(
            "source range {byte_start}..{byte_end} is invalid for `{logical_path}`"
        ));
    }
    let (start_line, start_column) = line_column(text, byte_start);
    let (end_line, end_column) = line_column(text, byte_end);
    FindingSource::new(logical_path, byte_start, byte_end)
        .map_err(|error| error.to_string())?
        .with_position(start_line, start_column, end_line, end_column)
        .map_err(|error| error.to_string())
}

fn line_column(text: &str, byte_offset: usize) -> (usize, usize) {
    let mut line = 1_usize;
    let mut column = 1_usize;
    for character in text[..byte_offset].chars() {
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn single_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(all(test, not(feature = "package-verify")))]
#[path = "../../tests/internal/accessibility_projection.rs"]
mod tests;
