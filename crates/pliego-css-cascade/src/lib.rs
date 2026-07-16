//! Fail-closed standard-CSS cascade explanation.

#![forbid(unsafe_code)]

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;

use lightningcss::properties::{Property, PropertyId};
use lightningcss::rules::layer::LayerName;
use lightningcss::rules::style::StyleRule;
use lightningcss::rules::{CssRule, CssRuleList, Location};
use lightningcss::selector::{Component, Selector};
use lightningcss::stylesheet::{ParserFlags, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::traits::ToCss;
use serde::Serialize;

const CASCADE_SCHEMA_VERSION: u8 = 1;
const MAX_CSS_BYTES: usize = 16 * 1024 * 1024;
const MAX_QUERY_BYTES: usize = 1_024;

/// Outcome of one bounded author-stylesheet cascade query.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CascadeStatus {
    /// One declaration wins inside the documented static boundary.
    Resolved,
    /// No declaration in the stylesheet matches the element and property.
    NoMatch,
    /// Runtime or unsupported context prevents a sound static winner.
    BrowserRequired,
}

/// Exact source range for one authored declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CascadeSource {
    /// Logical stylesheet path supplied by the caller.
    pub path: String,
    /// Zero-based UTF-8 byte start.
    pub byte_start: usize,
    /// Exclusive zero-based UTF-8 byte end.
    pub byte_end: usize,
    /// One-based source line.
    pub start_line: u32,
    /// One-based UTF-16 source column, matching Lightning CSS locations.
    pub start_column: u32,
    /// One-based source line containing the exclusive end.
    pub end_line: u32,
    /// One-based UTF-16 source column at the exclusive end.
    pub end_column: u32,
}

/// Standard selector-specificity tuple.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CascadeSpecificity {
    /// ID selector count.
    pub ids: u16,
    /// Class, attribute, and pseudo-class selector count.
    pub classes: u16,
    /// Type and pseudo-element selector count.
    pub types: u16,
}

impl CascadeSpecificity {
    fn from_packed(value: u32) -> Self {
        Self {
            ids: ((value >> 20) & 0x3ff) as u16,
            classes: ((value >> 10) & 0x3ff) as u16,
            types: (value & 0x3ff) as u16,
        }
    }
}

impl fmt::Display for CascadeSpecificity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{},{},{}", self.ids, self.classes, self.types)
    }
}

/// Specificity spread among declarations that can participate in the result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CascadeSpecificityEscalation {
    /// Whether the resolved winner exceeds the least-specific competing declaration.
    pub winner_exceeds_minimum: bool,
    /// Lowest specificity among matching candidates.
    pub minimum: CascadeSpecificity,
    /// Highest specificity among matching candidates.
    pub maximum: CascadeSpecificity,
    /// Winner specificity, or `None` for a non-resolved result.
    pub winner: Option<CascadeSpecificity>,
}

/// Normalized HTML element descriptor used by the static selector matcher.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CascadeElement {
    /// Canonical compound selector supplied for the target element.
    pub selector: String,
    /// Optional lowercase HTML local name.
    pub local_name: Option<String>,
    /// Optional case-sensitive element ID.
    pub id: Option<String>,
    /// Sorted, unique case-sensitive classes.
    pub classes: Vec<String>,
}

/// One matching declaration and every static precedence dimension used to rank it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CascadeCandidate {
    /// Stable query-local candidate ID.
    pub id: String,
    /// Most-specific matching selector from the declaration's selector list.
    pub selector: String,
    /// Exact authored declaration text, excluding a trailing semicolon.
    pub declaration: String,
    /// Canonical query-property declaration after extracting a shorthand when necessary.
    pub effective_declaration: String,
    /// Whether the authored declaration is important.
    pub important: bool,
    /// Named top-level cascade layer, or `None` for an unlayered declaration.
    pub layer: Option<String>,
    /// Zero-based first-appearance order for a named layer.
    pub layer_order: Option<u32>,
    /// Specificity of the matching selector used for this declaration.
    pub specificity: CascadeSpecificity,
    /// Zero-based UTF-8 byte start used as deterministic source order.
    pub source_order: usize,
    /// Exact authored declaration range.
    pub source: CascadeSource,
    /// `winner` or `overridden` for resolved output; `candidate` otherwise.
    pub disposition: String,
    /// First precedence dimension by which the winner defeats this declaration.
    pub decisive_criterion: Option<String>,
}

/// One reason the static boundary cannot identify a sound winner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CascadeBlocker {
    /// Stable blocker code.
    pub code: String,
    /// Human-readable explanation.
    pub message: String,
    /// Selector associated with the blocker when available.
    pub selector: Option<String>,
    /// One-based source line when available.
    pub line: Option<u32>,
    /// One-based UTF-16 source column when available.
    pub column: Option<u32>,
}

/// Schema-1 bounded author-stylesheet cascade explanation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CascadeExplanation {
    /// Document schema version.
    pub schema_version: u8,
    /// Fixed analysis boundary; this is not a computed-style result.
    pub scope: String,
    /// Logical stylesheet path.
    pub input: String,
    /// Canonical longhand property queried.
    pub property: String,
    /// Normalized target element.
    pub element: CascadeElement,
    /// Static resolution status.
    pub status: CascadeStatus,
    /// Winning candidate ID only when status is `resolved`.
    pub winner: Option<String>,
    /// Matching candidates in source order.
    pub candidates: Vec<CascadeCandidate>,
    /// Specificity escalation summary when candidates exist.
    pub specificity_escalation: Option<CascadeSpecificityEscalation>,
    /// Reasons browser/runtime evidence is required.
    pub blockers: Vec<CascadeBlocker>,
    /// Explicit limitations that always apply to this schema.
    pub limitations: Vec<String>,
}

/// Invalid input or internal source-reconciliation error from cascade explanation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CascadeExplainError {
    message: String,
}

impl CascadeExplainError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CascadeExplainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CascadeExplainError {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ElementDescriptor {
    output: CascadeElement,
    classes: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectorMatch {
    Match,
    NoMatch,
    Unsupported,
}

#[derive(Clone, Debug)]
struct MatchedSelector {
    css: String,
    specificity: CascadeSpecificity,
}

#[derive(Clone, Debug)]
struct LocatedDeclaration<'a> {
    property: Property<'a>,
    important: bool,
    source: CascadeSource,
    authored: String,
}

struct Collector<'a> {
    logical_path: &'a str,
    css: &'a str,
    lines: LineIndex,
    element: &'a ElementDescriptor,
    target: &'a PropertyId<'a>,
    target_name: &'a str,
    layers: Vec<String>,
    candidates: Vec<CascadeCandidate>,
    blockers: Vec<CascadeBlocker>,
}

#[derive(Clone, Debug, Default)]
struct RuleContext {
    layer: Option<String>,
    dynamic: Option<(&'static str, &'static str, Location)>,
}

/// Explains the direct author-stylesheet winner for one HTML element and longhand property.
///
/// The static boundary accepts only one complete UTF-8 stylesheet, a simple compound HTML element
/// selector (`type`, `#id`, `.class`, or `*`), and a closed direction-independent longhand set.
/// It resolves top-level named layers, importance, selector specificity, and source order. Imports,
/// conditional/nested/scoped rules, potentially matching complex selectors, animation/transition
/// origins, `all`, and `revert*` produce `browser-required` rather than an assumed winner.
///
/// # Errors
///
/// Returns an error for invalid CSS, an invalid element/property query, oversized input, or an
/// inability to reconcile parsed declarations with exact source ranges.
pub fn explain_stylesheet_cascade<'a>(
    logical_path: &'a str,
    css: &'a str,
    element: &'a str,
    property: &'a str,
) -> Result<CascadeExplanation, CascadeExplainError> {
    validate_query(logical_path, css, element, property)?;
    let target = PropertyId::from(property);
    let target_name = canonical_property(&target)?;
    if matches!(target, PropertyId::Custom(_) | PropertyId::All) || target.is_shorthand() {
        return Err(CascadeExplainError::new(
            "cascade explanation requires a supported standard longhand property",
        ));
    }
    if !supported_longhand(&target_name) {
        return Err(CascadeExplainError::new(format!(
            "cascade explanation does not yet support longhand `{target_name}`"
        )));
    }

    let element = parse_element(element)?;
    let stylesheet = StyleSheet::parse(
        css,
        ParserOptions {
            filename: logical_path.into(),
            flags: ParserFlags::NESTING,
            ..ParserOptions::default()
        },
    )
    .map_err(|error| CascadeExplainError::new(format!("invalid CSS: {error}")))?;
    let layers = discover_layers(&stylesheet.rules)?;
    let mut collector = Collector {
        logical_path,
        css,
        lines: LineIndex::new(css),
        element: &element,
        target: &target,
        target_name: &target_name,
        layers,
        candidates: Vec::new(),
        blockers: Vec::new(),
    };
    collector.collect_rules(&stylesheet.rules, &RuleContext::default())?;
    Ok(collector.finish(element.output.clone()))
}

impl<'a> Collector<'a> {
    #[allow(clippy::too_many_lines)]
    fn collect_rules(
        &mut self,
        rules: &CssRuleList<'a>,
        context: &RuleContext,
    ) -> Result<(), CascadeExplainError> {
        for rule in &rules.0 {
            match rule {
                CssRule::Style(style) => self.collect_style(style, context)?,
                CssRule::LayerStatement(statement) => {
                    if statement.names.iter().any(|name| name.0.len() != 1)
                        && rules_may_set_property(rules, self.target)
                    {
                        self.block(
                            "nested-layer-order",
                            "nested layer order statements are outside schema 1",
                            None,
                            Some(statement.loc),
                        );
                    }
                }
                CssRule::LayerBlock(layer) => {
                    let Some(name) = &layer.name else {
                        if rules_may_set_property(&layer.rules, self.target) {
                            self.block(
                                "anonymous-layer",
                                "anonymous or nested layer precedence is outside schema 1",
                                None,
                                Some(layer.loc),
                            );
                        }
                        continue;
                    };
                    if context.layer.is_some() || name.0.len() != 1 {
                        if rules_may_set_property(&layer.rules, self.target) {
                            self.block(
                                "nested-layer",
                                "nested layer precedence is outside schema 1",
                                None,
                                Some(layer.loc),
                            );
                        }
                        continue;
                    }
                    let layer_name = layer_name(name)?;
                    self.collect_rules(
                        &layer.rules,
                        &RuleContext {
                            layer: Some(layer_name),
                            dynamic: context.dynamic,
                        },
                    )?;
                }
                CssRule::Media(media) => self.collect_dynamic(
                    &media.rules,
                    context,
                    "media-context",
                    "media-query evaluation requires browser evidence",
                    media.loc,
                )?,
                CssRule::Supports(rule) => self.collect_dynamic(
                    &rule.rules,
                    context,
                    "supports-context",
                    "feature-support evaluation requires target browser evidence",
                    rule.loc,
                )?,
                CssRule::Container(rule) => self.collect_dynamic(
                    &rule.rules,
                    context,
                    "container-context",
                    "container-query evaluation requires layout evidence",
                    rule.loc,
                )?,
                CssRule::Scope(rule) => self.collect_dynamic(
                    &rule.rules,
                    context,
                    "scope-context",
                    "scope proximity requires DOM evidence",
                    rule.loc,
                )?,
                CssRule::StartingStyle(rule) => self.collect_dynamic(
                    &rule.rules,
                    context,
                    "starting-style-context",
                    "starting-style state requires runtime evidence",
                    rule.loc,
                )?,
                CssRule::MozDocument(rule) => self.collect_dynamic(
                    &rule.rules,
                    context,
                    "document-context",
                    "document-condition evaluation requires browser evidence",
                    rule.loc,
                )?,
                CssRule::Nesting(rule) => {
                    if style_may_set_property(&rule.style, self.target) {
                        self.block(
                            "nested-selector",
                            "nested selector matching requires DOM evidence",
                            selector_list_css(&rule.style.selectors).ok(),
                            Some(rule.style.loc),
                        );
                    }
                }
                CssRule::NestedDeclarations(rule) => {
                    if declarations_may_set_property(&rule.declarations, self.target) {
                        self.block(
                            "nested-declarations",
                            "nested declaration ownership requires parent selector evidence",
                            None,
                            None,
                        );
                    }
                }
                CssRule::Import(rule) => self.block(
                    "imported-stylesheet",
                    "an imported stylesheet is not present in this single-file query",
                    None,
                    Some(rule.loc),
                ),
                CssRule::Keyframes(rule) => {
                    if rule.keyframes.iter().any(|frame| {
                        declarations_may_set_property(&frame.declarations, self.target)
                    }) {
                        self.block(
                            "animation-origin",
                            "keyframes can override the author declaration at runtime",
                            None,
                            Some(rule.loc),
                        );
                    }
                }
                CssRule::Namespace(rule) => {
                    if rules_may_set_property(rules, self.target) {
                        self.block(
                            "namespace-context",
                            "namespace rules are outside the schema-1 HTML selector matcher",
                            None,
                            Some(rule.loc),
                        );
                    }
                }
                CssRule::Unknown(rule) => self.block(
                    "unknown-at-rule",
                    "an unknown at-rule may contain unsupported cascade context",
                    None,
                    Some(rule.loc),
                ),
                CssRule::Custom(_) => self.block(
                    "custom-at-rule",
                    "a custom at-rule may contain unsupported cascade context",
                    None,
                    None,
                ),
                CssRule::FontFace(_)
                | CssRule::FontPaletteValues(_)
                | CssRule::FontFeatureValues(_)
                | CssRule::Page(_)
                | CssRule::CounterStyle(_)
                | CssRule::Viewport(_)
                | CssRule::CustomMedia(_)
                | CssRule::Property(_)
                | CssRule::ViewTransition(_)
                | CssRule::Ignored => {}
            }
        }
        Ok(())
    }

    fn collect_dynamic(
        &mut self,
        rules: &CssRuleList<'a>,
        context: &RuleContext,
        code: &'static str,
        message: &'static str,
        location: Location,
    ) -> Result<(), CascadeExplainError> {
        self.collect_rules(
            rules,
            &RuleContext {
                layer: context.layer.clone(),
                dynamic: Some((code, message, location)),
            },
        )
    }

    #[allow(clippy::too_many_lines)]
    fn collect_style(
        &mut self,
        style: &StyleRule<'a>,
        context: &RuleContext,
    ) -> Result<(), CascadeExplainError> {
        let located = locate_declarations(self.logical_path, self.css, &self.lines, style)?;
        let affects_query = located
            .iter()
            .any(|item| declaration_effect(&item.property, self.target).is_some());
        let has_all = located
            .iter()
            .any(|item| matches!(item.property.property_id(), PropertyId::All));
        let has_transition = located.iter().any(|item| {
            matches!(
                item.property.property_id().name(),
                "transition" | "transition-property"
            )
        });
        if !affects_query && !has_all && !has_transition {
            if !style.rules.0.is_empty() {
                self.collect_rules(
                    &style.rules,
                    &RuleContext {
                        layer: context.layer.clone(),
                        dynamic: Some((
                            "nested-selector",
                            "nested selector matching requires DOM evidence",
                            style.loc,
                        )),
                    },
                )?;
            }
            return Ok(());
        }

        let mut matched = Vec::new();
        let mut unsupported = Vec::new();
        for selector in &style.selectors.0 {
            let css = selector_css(selector)?;
            match match_selector(selector, self.element) {
                SelectorMatch::Match => matched.push(MatchedSelector {
                    css,
                    specificity: CascadeSpecificity::from_packed(selector.specificity()),
                }),
                SelectorMatch::Unsupported => unsupported.push(css),
                SelectorMatch::NoMatch => {}
            }
        }
        if !unsupported.is_empty() {
            for selector in unsupported {
                self.block(
                    "unsupported-selector",
                    "selector may match but requires DOM or runtime state",
                    Some(selector),
                    Some(style.loc),
                );
            }
        }
        let Some(best) = matched.into_iter().max_by(|left, right| {
            left.specificity
                .cmp(&right.specificity)
                .then_with(|| right.css.cmp(&left.css))
        }) else {
            return Ok(());
        };

        if let Some((code, message, location)) = context.dynamic {
            self.block(code, message, Some(best.css.clone()), Some(location));
        }
        if !style.rules.0.is_empty() {
            self.block(
                "nested-selector",
                "nested rules can add declarations under DOM-dependent selectors",
                Some(best.css.clone()),
                Some(style.loc),
            );
            self.collect_rules(
                &style.rules,
                &RuleContext {
                    layer: context.layer.clone(),
                    dynamic: Some((
                        "nested-selector",
                        "nested selector matching requires DOM evidence",
                        style.loc,
                    )),
                },
            )?;
        }
        if has_transition {
            self.block(
                "transition-origin",
                "a matching transition declaration can outrank author declarations at runtime",
                Some(best.css.clone()),
                Some(style.loc),
            );
        }
        if has_all {
            self.block(
                "all-property",
                "the `all` shorthand requires full CSS-wide keyword resolution",
                Some(best.css.clone()),
                Some(style.loc),
            );
        }

        let layer_order = context
            .layer
            .as_ref()
            .and_then(|name| self.layers.iter().position(|candidate| candidate == name))
            .map(u32::try_from)
            .transpose()
            .map_err(|_| CascadeExplainError::new("cascade layer count exceeds u32"))?;
        for declaration in located {
            let Some(effective) = declaration_effect(&declaration.property, self.target) else {
                continue;
            };
            let effective_declaration = effective
                .to_css_string(false, PrinterOptions::default())
                .map_err(|_| {
                CascadeExplainError::new("cannot serialize effective declaration")
            })?;
            if effective_value(&effective_declaration)
                .is_some_and(|value| matches!(value, "revert" | "revert-layer"))
            {
                self.block(
                    "revert-keyword",
                    "`revert` and `revert-layer` require broader origin/layer history",
                    Some(best.css.clone()),
                    Some(style.loc),
                );
            }
            let id = format!("candidate-{:04}", self.candidates.len() + 1);
            self.candidates.push(CascadeCandidate {
                id,
                selector: best.css.clone(),
                declaration: declaration.authored,
                effective_declaration,
                important: declaration.important,
                layer: context.layer.clone(),
                layer_order,
                specificity: best.specificity,
                source_order: declaration.source.byte_start,
                source: declaration.source,
                disposition: "candidate".into(),
                decisive_criterion: None,
            });
        }
        Ok(())
    }

    fn block(
        &mut self,
        code: &str,
        message: &str,
        selector: Option<String>,
        location: Option<Location>,
    ) {
        let blocker = CascadeBlocker {
            code: code.into(),
            message: message.into(),
            selector,
            line: location.map(|value| value.line + 1),
            column: location.map(|value| value.column),
        };
        if !self.blockers.contains(&blocker) {
            self.blockers.push(blocker);
        }
    }

    fn finish(mut self, element: CascadeElement) -> CascadeExplanation {
        self.candidates
            .sort_by_key(|candidate| candidate.source_order);
        let winner_index = if self.blockers.is_empty() && !self.candidates.is_empty() {
            self.candidates
                .iter()
                .enumerate()
                .max_by(|(_, left), (_, right)| compare_candidates(left, right))
                .map(|(index, _)| index)
        } else {
            None
        };
        if let Some(index) = winner_index {
            let winner = self.candidates[index].clone();
            for candidate in &mut self.candidates {
                if candidate.id == winner.id {
                    candidate.disposition = "winner".into();
                } else {
                    candidate.disposition = "overridden".into();
                    candidate.decisive_criterion = Some(decisive_criterion(&winner, candidate));
                }
            }
        }
        let specificity_escalation = specificity_escalation(
            &self.candidates,
            winner_index.map(|index| self.candidates[index].specificity),
        );
        let status = if !self.blockers.is_empty() {
            CascadeStatus::BrowserRequired
        } else if self.candidates.is_empty() {
            CascadeStatus::NoMatch
        } else {
            CascadeStatus::Resolved
        };
        CascadeExplanation {
            schema_version: CASCADE_SCHEMA_VERSION,
            scope: "single-author-stylesheet-direct-element-declarations".into(),
            input: self.logical_path.into(),
            property: self.target_name.into(),
            element,
            status,
            winner: winner_index.map(|index| self.candidates[index].id.clone()),
            candidates: self.candidates,
            specificity_escalation,
            blockers: self.blockers,
            limitations: vec![
                "does not include inline styles, other stylesheets, user or user-agent origins"
                    .into(),
                "does not compute inheritance, custom-property substitution, layout, or used values"
                    .into(),
                "browser-required means no static winner is claimed".into(),
            ],
        }
    }
}

fn validate_query(
    logical_path: &str,
    css: &str,
    element: &str,
    property: &str,
) -> Result<(), CascadeExplainError> {
    if logical_path.is_empty() || logical_path.chars().any(char::is_control) {
        return Err(CascadeExplainError::new(
            "cascade input path must be non-empty text without control characters",
        ));
    }
    if css.len() > MAX_CSS_BYTES {
        return Err(CascadeExplainError::new(
            "cascade explanation input exceeds 16 MiB",
        ));
    }
    if element.is_empty() || element.len() > MAX_QUERY_BYTES {
        return Err(CascadeExplainError::new(
            "cascade element selector must contain 1..=1024 bytes",
        ));
    }
    if property.is_empty() || property.len() > MAX_QUERY_BYTES {
        return Err(CascadeExplainError::new(
            "cascade property must contain 1..=1024 bytes",
        ));
    }
    Ok(())
}

fn canonical_property(property: &PropertyId<'_>) -> Result<String, CascadeExplainError> {
    property
        .to_css_string(PrinterOptions::default())
        .map_err(|_| CascadeExplainError::new("cannot serialize cascade property"))
}

fn supported_longhand(property: &str) -> bool {
    matches!(
        property,
        "background-color"
            | "color"
            | "display"
            | "font-family"
            | "font-size"
            | "font-style"
            | "font-weight"
            | "line-height"
            | "opacity"
            | "order"
            | "overflow-x"
            | "overflow-y"
            | "pointer-events"
            | "position"
            | "text-align"
            | "text-decoration-color"
            | "text-decoration-line"
            | "text-decoration-style"
            | "text-transform"
            | "visibility"
            | "white-space"
            | "z-index"
    )
}

fn parse_element(value: &str) -> Result<ElementDescriptor, CascadeExplainError> {
    let wrapped = format!("{value}{{}} ");
    let stylesheet = StyleSheet::parse(&wrapped, ParserOptions::default())
        .map_err(|error| CascadeExplainError::new(format!("invalid element selector: {error}")))?;
    let [CssRule::Style(style)] = stylesheet.rules.0.as_slice() else {
        return Err(CascadeExplainError::new(
            "element selector must contain exactly one simple compound selector",
        ));
    };
    let [selector] = style.selectors.0.as_slice() else {
        return Err(CascadeExplainError::new(
            "element selector must contain exactly one selector",
        ));
    };
    let mut local_name = None;
    let mut id = None;
    let mut classes = BTreeSet::new();
    for component in selector.iter_raw_match_order() {
        match component {
            Component::ExplicitUniversalType => {}
            Component::LocalName(name) => {
                if local_name.is_some() {
                    return Err(CascadeExplainError::new(
                        "element selector may contain at most one type",
                    ));
                }
                local_name = Some(name.lower_name.0.as_ref().to_owned());
            }
            Component::ID(identifier) => {
                if id.replace(identifier.0.as_ref().to_owned()).is_some() {
                    return Err(CascadeExplainError::new(
                        "element selector may contain at most one ID",
                    ));
                }
            }
            Component::Class(identifier) => {
                classes.insert(identifier.0.as_ref().to_owned());
            }
            _ => {
                return Err(CascadeExplainError::new(
                    "element selector supports only type, #id, .class, and * components",
                ));
            }
        }
    }
    let canonical = selector_css(selector)?;
    Ok(ElementDescriptor {
        output: CascadeElement {
            selector: canonical,
            local_name,
            id,
            classes: classes.iter().cloned().collect(),
        },
        classes,
    })
}

fn match_selector(selector: &Selector<'_>, element: &ElementDescriptor) -> SelectorMatch {
    let mut unsupported = false;
    let mut mismatch = false;
    for component in selector.iter_raw_match_order() {
        match component {
            Component::ExplicitUniversalType => {}
            Component::LocalName(name) => {
                if let Some(local) = &element.output.local_name {
                    mismatch |= !local.eq_ignore_ascii_case(name.lower_name.0.as_ref());
                } else {
                    unsupported = true;
                }
            }
            Component::ID(identifier) => {
                mismatch |= element
                    .output
                    .id
                    .as_ref()
                    .is_none_or(|id| id != identifier.0.as_ref());
            }
            Component::Class(identifier) => {
                mismatch |= !element.classes.contains(identifier.0.as_ref());
            }
            _ => unsupported = true,
        }
    }
    if mismatch {
        SelectorMatch::NoMatch
    } else if unsupported {
        SelectorMatch::Unsupported
    } else {
        SelectorMatch::Match
    }
}

fn discover_layers(rules: &CssRuleList<'_>) -> Result<Vec<String>, CascadeExplainError> {
    let mut layers = Vec::new();
    for rule in &rules.0 {
        match rule {
            CssRule::LayerStatement(statement) => {
                for name in &statement.names {
                    if name.0.len() != 1 {
                        continue;
                    }
                    push_layer(&mut layers, layer_name(name)?);
                }
            }
            CssRule::LayerBlock(block) => {
                if let Some(name) = &block.name {
                    if name.0.len() == 1 {
                        push_layer(&mut layers, layer_name(name)?);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(layers)
}

fn push_layer(layers: &mut Vec<String>, name: String) {
    if !layers.contains(&name) {
        layers.push(name);
    }
}

fn layer_name(name: &LayerName<'_>) -> Result<String, CascadeExplainError> {
    name.to_css_string(PrinterOptions::default())
        .map_err(|_| CascadeExplainError::new("cannot serialize cascade layer name"))
}

fn selector_css(selector: &Selector<'_>) -> Result<String, CascadeExplainError> {
    selector
        .to_css_string(PrinterOptions::default())
        .map_err(|_| CascadeExplainError::new("cannot serialize selector"))
}

fn selector_list_css(
    selectors: &lightningcss::selector::SelectorList<'_>,
) -> Result<String, CascadeExplainError> {
    selectors
        .to_css_string(PrinterOptions::default())
        .map_err(|_| CascadeExplainError::new("cannot serialize selector list"))
}

fn declaration_effect<'i>(
    property: &Property<'i>,
    target: &PropertyId<'i>,
) -> Option<Property<'i>> {
    if property.property_id() == *target {
        Some(property.clone())
    } else {
        property.longhand(target)
    }
}

fn declarations_may_set_property(
    declarations: &lightningcss::declaration::DeclarationBlock<'_>,
    target: &PropertyId<'_>,
) -> bool {
    declarations.iter().any(|(property, _)| {
        matches!(property.property_id(), PropertyId::All)
            || property.property_id() == *target
            || property.longhand(target).is_some()
    })
}

fn style_may_set_property(style: &StyleRule<'_>, target: &PropertyId<'_>) -> bool {
    declarations_may_set_property(&style.declarations, target)
        || rules_may_set_property(&style.rules, target)
}

fn rules_may_set_property(rules: &CssRuleList<'_>, target: &PropertyId<'_>) -> bool {
    rules.0.iter().any(|rule| match rule {
        CssRule::Style(style) => style_may_set_property(style, target),
        CssRule::Nesting(rule) => style_may_set_property(&rule.style, target),
        CssRule::NestedDeclarations(rule) => {
            declarations_may_set_property(&rule.declarations, target)
        }
        CssRule::Media(rule) => rules_may_set_property(&rule.rules, target),
        CssRule::Supports(rule) => rules_may_set_property(&rule.rules, target),
        CssRule::MozDocument(rule) => rules_may_set_property(&rule.rules, target),
        CssRule::LayerBlock(rule) => rules_may_set_property(&rule.rules, target),
        CssRule::Container(rule) => rules_may_set_property(&rule.rules, target),
        CssRule::Scope(rule) => rules_may_set_property(&rule.rules, target),
        CssRule::StartingStyle(rule) => rules_may_set_property(&rule.rules, target),
        CssRule::Keyframes(rule) => rule
            .keyframes
            .iter()
            .any(|frame| declarations_may_set_property(&frame.declarations, target)),
        _ => false,
    })
}

fn locate_declarations<'a>(
    logical_path: &str,
    css: &'a str,
    lines: &LineIndex,
    style: &StyleRule<'a>,
) -> Result<Vec<LocatedDeclaration<'a>>, CascadeExplainError> {
    let count = style.declarations.len();
    if style.loc.source_index != 0 {
        return Err(CascadeExplainError::new(
            "cascade source locations must refer to the single input stylesheet",
        ));
    }
    let rule_start = lines.offset(css, style.loc.line, style.loc.column)?;
    let rule_css = css
        .get(rule_start..)
        .ok_or_else(|| CascadeExplainError::new("invalid style-rule source start"))?;
    let rule_lines = LineIndex::new(rule_css);
    let mut located_style = style.clone();
    located_style.loc = Location {
        source_index: 0,
        line: 0,
        column: 1,
    };
    let mut source = Vec::with_capacity(count);
    for index in 0..count {
        let (property, value) = located_style
            .property_location(rule_css, index)
            .map_err(|_| CascadeExplainError::new("cannot locate authored declaration"))?;
        let byte_start =
            rule_start + rule_lines.offset(rule_css, property.start.line, property.start.column)?;
        let property_end =
            rule_start + rule_lines.offset(rule_css, property.end.line, property.end.column)?;
        let value_start =
            rule_start + rule_lines.offset(rule_css, value.start.line, value.start.column)?;
        let byte_end =
            rule_start + rule_lines.offset(rule_css, value.end.line, value.end.column)?;
        let authored = css
            .get(byte_start..byte_end)
            .ok_or_else(|| CascadeExplainError::new("invalid authored declaration range"))?
            .trim_end()
            .to_owned();
        let raw_name = css
            .get(byte_start..property_end)
            .ok_or_else(|| CascadeExplainError::new("invalid property-name range"))?
            .to_owned();
        let important = value_has_important(
            css.get(value_start..byte_end)
                .ok_or_else(|| CascadeExplainError::new("invalid property-value range"))?,
        );
        let (start_line, start_column) = lines.position(css, byte_start)?;
        let (end_line, end_column) = lines.position(css, byte_end)?;
        source.push((
            raw_name,
            important,
            false,
            CascadeSource {
                path: logical_path.into(),
                byte_start,
                byte_end,
                start_line,
                start_column,
                end_line,
                end_column,
            },
            authored,
        ));
    }

    let mut output = Vec::with_capacity(count);
    for (property, important) in style.declarations.iter() {
        let name = canonical_property(&property.property_id())?;
        let Some((_, _, used, exact, authored)) = source
            .iter_mut()
            .find(|item| !item.2 && item.1 == important && item.0.eq_ignore_ascii_case(&name))
        else {
            return Err(CascadeExplainError::new(
                "cannot reconcile parsed declaration with authored source",
            ));
        };
        *used = true;
        output.push(LocatedDeclaration {
            property: property.clone(),
            important,
            source: exact.clone(),
            authored: authored.clone(),
        });
    }
    output.sort_by_key(|item| item.source.byte_start);
    Ok(output)
}

fn value_has_important(value: &str) -> bool {
    value
        .trim_end_matches(|character: char| character.is_ascii_whitespace())
        .to_ascii_lowercase()
        .ends_with("!important")
}

fn effective_value(declaration: &str) -> Option<&str> {
    declaration
        .split_once(':')
        .map(|(_, value)| value.trim_matches(|character: char| character.is_ascii_whitespace()))
}

fn compare_candidates(left: &CascadeCandidate, right: &CascadeCandidate) -> Ordering {
    left.important
        .cmp(&right.important)
        .then_with(|| layer_precedence(left).cmp(&layer_precedence(right)))
        .then_with(|| left.specificity.cmp(&right.specificity))
        .then_with(|| left.source_order.cmp(&right.source_order))
}

fn layer_precedence(candidate: &CascadeCandidate) -> i64 {
    match (candidate.important, candidate.layer_order) {
        (false, None) => i64::MAX,
        (false, Some(order)) => i64::from(order),
        (true, None) => i64::MIN,
        (true, Some(order)) => -i64::from(order),
    }
}

fn decisive_criterion(winner: &CascadeCandidate, loser: &CascadeCandidate) -> String {
    if winner.important != loser.important {
        "importance".into()
    } else if layer_precedence(winner) != layer_precedence(loser) {
        "layer-order".into()
    } else if winner.specificity != loser.specificity {
        "specificity".into()
    } else {
        "source-order".into()
    }
}

fn specificity_escalation(
    candidates: &[CascadeCandidate],
    winner: Option<CascadeSpecificity>,
) -> Option<CascadeSpecificityEscalation> {
    let minimum = candidates.iter().map(|item| item.specificity).min()?;
    let maximum = candidates.iter().map(|item| item.specificity).max()?;
    Some(CascadeSpecificityEscalation {
        winner_exceeds_minimum: winner.is_some_and(|value| value > minimum),
        minimum,
        maximum,
        winner,
    })
}

struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(css: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            css.bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        );
        Self { starts }
    }

    fn offset(&self, css: &str, line: u32, column: u32) -> Result<usize, CascadeExplainError> {
        let start = *self
            .starts
            .get(line as usize)
            .ok_or_else(|| CascadeExplainError::new("invalid source line"))?;
        let units = column
            .checked_sub(1)
            .ok_or_else(|| CascadeExplainError::new("invalid source column"))?;
        let mut consumed = 0_u32;
        for (relative, character) in css[start..].char_indices() {
            if consumed == units {
                return Ok(start + relative);
            }
            if matches!(character, '\n' | '\r') {
                break;
            }
            consumed = consumed
                .checked_add(
                    u32::try_from(character.len_utf16())
                        .map_err(|_| CascadeExplainError::new("source column overflow"))?,
                )
                .ok_or_else(|| CascadeExplainError::new("source column overflow"))?;
            if consumed > units {
                return Err(CascadeExplainError::new(
                    "source column splits a UTF-16 character",
                ));
            }
        }
        if consumed == units {
            Ok(css
                .len()
                .min(start + css[start..].find(['\r', '\n']).unwrap_or(css.len() - start)))
        } else {
            Err(CascadeExplainError::new("source column is out of bounds"))
        }
    }

    fn position(&self, css: &str, offset: usize) -> Result<(u32, u32), CascadeExplainError> {
        if offset > css.len() || !css.is_char_boundary(offset) {
            return Err(CascadeExplainError::new("invalid UTF-8 source offset"));
        }
        let line = self.starts.partition_point(|start| *start <= offset) - 1;
        let start = self.starts[line];
        let column = css[start..offset]
            .chars()
            .try_fold(1_u32, |column, character| {
                let units = u32::try_from(character.len_utf16()).ok()?;
                column.checked_add(units)
            })
            .ok_or_else(|| CascadeExplainError::new("source column overflow"))?;
        let line = u32::try_from(line)
            .ok()
            .and_then(|line| line.checked_add(1))
            .ok_or_else(|| CascadeExplainError::new("source line overflow"))?;
        Ok((line, column))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_importance_layers_specificity_and_source_order() {
        let css = r"
@layer base, overrides;
@layer overrides { button.action { color: purple !important; } }
@layer base { #save { color: green !important; } }
button { color: black; }
.action { color: blue; }
button.action { color: red; }
";
        let explanation =
            explain_stylesheet_cascade("app.css", css, "button#save.action", "color").unwrap();
        assert_eq!(explanation.status, CascadeStatus::Resolved);
        let winner = explanation
            .candidates
            .iter()
            .find(|candidate| Some(&candidate.id) == explanation.winner.as_ref())
            .unwrap();
        assert_eq!(winner.declaration, "color: green !important");
        assert_eq!(winner.layer.as_deref(), Some("base"));
        assert_eq!(winner.disposition, "winner");
        assert_eq!(explanation.candidates.len(), 5);
        assert!(
            explanation
                .specificity_escalation
                .as_ref()
                .unwrap()
                .winner_exceeds_minimum
        );
    }

    #[test]
    fn extracts_shorthand_and_preserves_exact_mixed_importance_ranges() {
        let css =
            ".card { color: red !important; font: italic 700 18px/1.5 sans-serif; color: blue; }";
        let explanation = explain_stylesheet_cascade("app.css", css, ".card", "font-size").unwrap();
        assert_eq!(explanation.status, CascadeStatus::Resolved);
        assert_eq!(explanation.candidates.len(), 1);
        let candidate = &explanation.candidates[0];
        assert_eq!(
            candidate.declaration,
            "font: italic 700 18px/1.5 sans-serif"
        );
        assert!(candidate.effective_declaration.starts_with("font-size:"));
        assert_eq!(
            &css[candidate.source.byte_start..candidate.source.byte_end],
            candidate.declaration
        );
    }

    #[test]
    fn normal_unlayered_declaration_outranks_every_named_layer() {
        let css = r"
@layer low, high;
@layer high { #save.action { color: red; } }
@layer low { #save.action { color: green; } }
.action { color: blue; }
";
        let explanation =
            explain_stylesheet_cascade("app.css", css, "button#save.action", "color").unwrap();
        let winner = explanation
            .candidates
            .iter()
            .find(|candidate| Some(&candidate.id) == explanation.winner.as_ref())
            .unwrap();
        assert_eq!(winner.declaration, "color: blue");
        assert_eq!(winner.layer, None);
        assert!(
            explanation
                .candidates
                .iter()
                .filter(|candidate| candidate.id != winner.id)
                .all(|candidate| candidate.decisive_criterion.as_deref() == Some("layer-order"))
        );
    }

    #[test]
    fn fails_closed_for_runtime_selector_and_conditional_context() {
        let css = r"
.card:hover { color: red; }
@media (width > 40rem) { .card { color: blue; } }
";
        let explanation =
            explain_stylesheet_cascade("app.css", css, "article.card", "color").unwrap();
        assert_eq!(explanation.status, CascadeStatus::BrowserRequired);
        assert!(explanation.winner.is_none());
        assert_eq!(explanation.candidates.len(), 1);
        assert_eq!(
            explanation
                .blockers
                .iter()
                .map(|blocker| blocker.code.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["media-context", "unsupported-selector"])
        );
    }

    #[test]
    fn known_selector_mismatch_does_not_require_browser() {
        let explanation =
            explain_stylesheet_cascade("app.css", ".other:hover { color: red; }", ".card", "color")
                .unwrap();
        assert_eq!(explanation.status, CascadeStatus::NoMatch);
        assert!(explanation.blockers.is_empty());
    }

    #[test]
    fn missing_target_type_and_nested_layer_order_fail_closed() {
        let css = r"
@layer framework.base, app;
@layer app { button.card { color: blue; } }
.card { color: red; }
";
        let explanation = explain_stylesheet_cascade("app.css", css, ".card", "color").unwrap();
        assert_eq!(explanation.status, CascadeStatus::BrowserRequired);
        assert!(explanation.winner.is_none());
        let codes = explanation
            .blockers
            .iter()
            .map(|blocker| blocker.code.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            codes,
            BTreeSet::from(["nested-layer-order", "unsupported-selector"])
        );
    }

    #[test]
    fn rejects_complex_element_and_unbounded_property() {
        assert!(
            explain_stylesheet_cascade("app.css", ".card { color: red; }", "main .card", "color")
                .unwrap_err()
                .to_string()
                .contains("supports only")
        );
        assert!(
            explain_stylesheet_cascade("app.css", ".card { width: 1px; }", ".card", "width")
                .unwrap_err()
                .to_string()
                .contains("does not yet support")
        );
    }
}
