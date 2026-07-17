use std::collections::{BTreeMap, BTreeSet};

use lightningcss::rules::{CssRule, CssRuleList, Location};
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::Targets;
use lightningcss::traits::ToCss;
use serde::Serialize;

const MAX_TRACE_ITEMS: usize = 65_535;
const MAX_TRACE_CSS_BYTES: usize = 16 * 1024 * 1024;
const INVALID: &str = "physical CSS trace reconciliation failed";

/// Neutral lineage for one semantic style emitted before CSS postprocessing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceStyle {
    style_id: u128,
    rules: Vec<TraceRule>,
}

impl TraceStyle {
    /// Creates lineage for one style and its qualified rules in emitted order.
    #[must_use]
    pub const fn new(style_id: u128, rules: Vec<TraceRule>) -> Self {
        Self { style_id, rules }
    }
}

/// Neutral lineage for one qualified rule before CSS postprocessing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceRule {
    declarations: Vec<TraceDeclaration>,
}

impl TraceRule {
    /// Creates lineage for declarations in their raw emitted order.
    #[must_use]
    pub const fn new(declarations: Vec<TraceDeclaration>) -> Self {
        Self { declarations }
    }
}

/// Neutral semantic ownership for one declaration before CSS postprocessing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceDeclaration {
    semantic_ordinals: Vec<u32>,
    important: bool,
    generated: bool,
}

impl TraceDeclaration {
    /// Creates one raw declaration lineage record.
    #[must_use]
    pub const fn new(semantic_ordinals: Vec<u32>, important: bool, generated: bool) -> Self {
        Self {
            semantic_ordinals,
            important,
            generated,
        }
    }
}

/// Complete physical projection over the final CSS artifact.
pub struct PhysicalProjection {
    pub(super) producers: Vec<PhysicalProducer>,
    pub(super) rules: Vec<PhysicalRule>,
    pub(super) declarations: Vec<PhysicalDeclaration>,
    pub(super) edges: Vec<PhysicalEdge>,
}

/// A non-semantic producer of final CSS declarations.
#[derive(Serialize)]
pub struct PhysicalProducer {
    pub(super) id: String,
    kind: &'static str,
}

/// A final qualified rule or supported at-rule.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalRule {
    pub(super) id: String,
    ordinal: u32,
    kind: &'static str,
    byte_start: u64,
    byte_end: u64,
    header_byte_start: u64,
    header_byte_end: u64,
}

/// One declaration in the final CSS artifact.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalDeclaration {
    pub(super) id: String,
    ordinal: u32,
    property: String,
    important: bool,
    generated: bool,
    byte_start: u64,
    byte_end: u64,
    property_byte_start: u64,
    property_byte_end: u64,
    value_byte_start: u64,
    value_byte_end: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct PhysicalEdge {
    pub(super) kind: &'static str,
    pub(super) from: String,
    pub(super) to: String,
}

#[derive(Clone)]
enum Producer {
    Theme,
    Semantic {
        style_id: u128,
        ordinals: Vec<u32>,
        generated: bool,
    },
}

struct QualifiedPlan {
    style_id: Option<u128>,
    declarations: Option<Vec<TraceDeclaration>>,
}

struct ExpectedDeclaration {
    text: String,
    producer: Producer,
}

struct ExpectedRule {
    kind: &'static str,
    header: String,
    parent: Option<u32>,
    declarations: Vec<ExpectedDeclaration>,
}

struct ActualDeclaration {
    text: String,
    property: String,
    important: bool,
    byte_start: usize,
    byte_end: usize,
    property_start: usize,
    property_end: usize,
    value_start: usize,
    value_end: usize,
}

struct ActualRule {
    kind: &'static str,
    header: String,
    parent: Option<u32>,
    byte_start: usize,
    byte_end: usize,
    header_end: usize,
    declarations: Vec<ActualDeclaration>,
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

    fn offset(&self, css: &str, line: u32, column: u32) -> Result<usize, String> {
        let start = *self.starts.get(line as usize).ok_or(INVALID)?;
        let units = column.checked_sub(1).ok_or(INVALID)?;
        let mut consumed = 0_u32;
        for (relative, character) in css[start..].char_indices() {
            if consumed == units {
                return Ok(start + relative);
            }
            if character == '\n' || character == '\r' {
                break;
            }
            consumed = consumed
                .checked_add(u32::try_from(character.len_utf16()).map_err(|_| INVALID)?)
                .ok_or(INVALID)?;
            if consumed > units {
                return Err(INVALID.into());
            }
        }
        if consumed == units {
            Ok(css
                .len()
                .min(start + css[start..].find(['\r', '\n']).unwrap_or(css.len() - start)))
        } else {
            Err(INVALID.into())
        }
    }
}

/// Reconciles raw emitter lineage with the final Lightning CSS artifact.
///
/// # Errors
///
/// Returns an error unless every supported rule and declaration can be mapped
/// exactly, all semantic producers remain covered, every range addresses the
/// final UTF-8 bytes, and all trace budgets remain within their hard limits.
#[allow(clippy::too_many_lines)]
pub fn build_physical_projection(
    raw_css: &str,
    final_css: &str,
    targets: Targets,
    minify: bool,
    include_theme: bool,
    styles: &[TraceStyle],
) -> Result<PhysicalProjection, String> {
    valid(raw_css.len() <= MAX_TRACE_CSS_BYTES && final_css.len() <= MAX_TRACE_CSS_BYTES)?;
    let raw = StyleSheet::parse(raw_css, ParserOptions::default()).map_err(|_| INVALID)?;
    let final_sheet =
        StyleSheet::parse(final_css, ParserOptions::default()).map_err(|_| INVALID)?;
    let plans = qualified_plans(include_theme, styles)?;
    let mut expected = Vec::new();
    let mut plan_index = 0;
    collect_expected_rules(
        &raw.rules,
        None,
        targets,
        minify,
        &plans,
        &mut plan_index,
        &mut expected,
    )
    .map_err(|error| format!("cannot reconcile raw CSS structure: {error}"))?;
    if plan_index != plans.len() {
        return Err("raw CSS does not consume every lineage rule".into());
    }

    let lines = LineIndex::new(final_css);
    let mut actual = Vec::new();
    collect_actual_rules(&final_sheet.rules, None, final_css, &lines, &mut actual)
        .map_err(|error| format!("cannot inspect final CSS structure: {error}"))?;
    if expected.len() != actual.len() || expected.len() > MAX_TRACE_ITEMS {
        return Err("raw and final CSS rule counts differ".into());
    }
    validate_actual_ranges(final_css, &actual)?;

    let theme_producer = "producer:theme".to_owned();
    let theme_has_declarations = include_theme
        && expected
            .first()
            .is_some_and(|rule| !rule.declarations.is_empty());
    let producers = theme_has_declarations
        .then(|| PhysicalProducer {
            id: theme_producer.clone(),
            kind: "theme",
        })
        .into_iter()
        .collect();
    let mut rules = Vec::with_capacity(actual.len());
    let mut declarations = Vec::new();
    let mut edges = BTreeSet::new();
    let mut covered_semantic = BTreeSet::new();

    for (rule_ordinal, (expected_rule, actual_rule)) in expected.iter().zip(&actual).enumerate() {
        if expected_rule.kind != actual_rule.kind
            || expected_rule.header != actual_rule.header
            || expected_rule.parent != actual_rule.parent
            || expected_rule.declarations.len() != actual_rule.declarations.len()
        {
            return Err(format!(
                "physical CSS rule {rule_ordinal} does not match raw lineage"
            ));
        }
        let rule_ordinal = u32::try_from(rule_ordinal).map_err(|_| INVALID)?;
        let rule_id = format!("css-rule:{rule_ordinal:08x}");
        if let Some(parent) = actual_rule.parent {
            edge(
                &mut edges,
                "ruleNestedInRule",
                rule_id.clone(),
                format!("css-rule:{parent:08x}"),
            )?;
        }
        for (ordinal, (expected_declaration, actual_declaration)) in expected_rule
            .declarations
            .iter()
            .zip(&actual_rule.declarations)
            .enumerate()
        {
            if expected_declaration.text != actual_declaration.text {
                return Err(format!(
                    "physical declaration {ordinal} in rule {rule_ordinal} does not match raw lineage"
                ));
            }
            let ordinal = u32::try_from(ordinal).map_err(|_| INVALID)?;
            let declaration_id = format!("css-decl:{rule_ordinal:08x}:{ordinal:08x}");
            edge(
                &mut edges,
                "physicalDeclarationBelongsToRule",
                declaration_id.clone(),
                rule_id.clone(),
            )?;
            let generated = match &expected_declaration.producer {
                Producer::Theme => {
                    edge(
                        &mut edges,
                        "syntheticProducerProducesPhysicalDeclaration",
                        theme_producer.clone(),
                        declaration_id.clone(),
                    )?;
                    false
                }
                Producer::Semantic {
                    style_id,
                    ordinals,
                    generated,
                } => {
                    for ordinal in ordinals {
                        let semantic = format!("decl:{style_id:032x}:{ordinal:08x}");
                        covered_semantic.insert(semantic.clone());
                        edge(
                            &mut edges,
                            "declarationContributesToPhysicalDeclaration",
                            semantic,
                            declaration_id.clone(),
                        )?;
                    }
                    *generated
                }
            };
            declarations.push(PhysicalDeclaration {
                id: declaration_id,
                ordinal,
                property: actual_declaration.property.clone(),
                important: actual_declaration.important,
                generated,
                byte_start: to_u64(actual_declaration.byte_start)?,
                byte_end: to_u64(actual_declaration.byte_end)?,
                property_byte_start: to_u64(actual_declaration.property_start)?,
                property_byte_end: to_u64(actual_declaration.property_end)?,
                value_byte_start: to_u64(actual_declaration.value_start)?,
                value_byte_end: to_u64(actual_declaration.value_end)?,
            });
        }
        rules.push(PhysicalRule {
            id: rule_id,
            ordinal: rule_ordinal,
            kind: actual_rule.kind,
            byte_start: to_u64(actual_rule.byte_start)?,
            byte_end: to_u64(actual_rule.byte_end)?,
            header_byte_start: to_u64(actual_rule.byte_start)?,
            header_byte_end: to_u64(actual_rule.header_end)?,
        });
    }

    valid(
        rules
            .len()
            .checked_add(declarations.len())
            .is_some_and(|count| count <= MAX_TRACE_ITEMS)
            && edges.len() <= MAX_TRACE_ITEMS,
    )?;
    let expected_semantic = styles
        .iter()
        .flat_map(|style| {
            style.rules.iter().flat_map(move |rule| {
                rule.declarations.iter().flat_map(move |declaration| {
                    declaration
                        .semantic_ordinals
                        .iter()
                        .map(move |ordinal| format!("decl:{:032x}:{ordinal:08x}", style.style_id))
                })
            })
        })
        .collect::<BTreeSet<_>>();
    valid(covered_semantic == expected_semantic)?;
    Ok(PhysicalProjection {
        producers,
        rules,
        declarations,
        edges: edges.into_iter().collect(),
    })
}

fn qualified_plans(
    include_theme: bool,
    styles: &[TraceStyle],
) -> Result<Vec<QualifiedPlan>, String> {
    let mut plans = Vec::new();
    if include_theme {
        plans.push(QualifiedPlan {
            style_id: None,
            declarations: None,
        });
    }
    let mut ids = BTreeSet::new();
    for style in styles {
        valid(ids.insert(style.style_id))?;
        for rule in &style.rules {
            for declaration in &rule.declarations {
                valid(
                    !declaration.semantic_ordinals.is_empty()
                        && declaration
                            .semantic_ordinals
                            .windows(2)
                            .all(|pair| pair[0] < pair[1]),
                )?;
            }
            plans.push(QualifiedPlan {
                style_id: Some(style.style_id),
                declarations: Some(rule.declarations.clone()),
            });
        }
    }
    valid(plans.len() <= MAX_TRACE_ITEMS)?;
    Ok(plans)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn collect_expected_rules(
    source: &CssRuleList<'_>,
    parent: Option<u32>,
    targets: Targets,
    minify: bool,
    plans: &[QualifiedPlan],
    plan_index: &mut usize,
    output: &mut Vec<ExpectedRule>,
) -> Result<(), String> {
    for rule in &source.0 {
        valid(output.len() < MAX_TRACE_ITEMS)?;
        match rule {
            CssRule::LayerStatement(layer) => {
                output.push(ExpectedRule {
                    kind: "layer-order",
                    header: format!(
                        "@layer {}",
                        layer
                            .names
                            .to_css_string(printer(minify, targets))
                            .map_err(|_| INVALID)?
                    ),
                    parent,
                    declarations: Vec::new(),
                });
            }
            CssRule::LayerBlock(layer) => {
                let ordinal = u32::try_from(output.len()).map_err(|_| INVALID)?;
                let mut header = String::from("@layer");
                if let Some(name) = &layer.name {
                    header.push(' ');
                    header.push_str(
                        &name
                            .to_css_string(printer(minify, targets))
                            .map_err(|_| INVALID)?,
                    );
                }
                output.push(ExpectedRule {
                    kind: "layer",
                    header,
                    parent,
                    declarations: Vec::new(),
                });
                collect_expected_rules(
                    &layer.rules,
                    Some(ordinal),
                    targets,
                    minify,
                    plans,
                    plan_index,
                    output,
                )?;
            }
            CssRule::Media(media) => {
                let ordinal = u32::try_from(output.len()).map_err(|_| INVALID)?;
                output.push(ExpectedRule {
                    kind: "media",
                    header: format!(
                        "@media {}",
                        media
                            .query
                            .to_css_string(printer(minify, targets))
                            .map_err(|_| INVALID)?
                    ),
                    parent,
                    declarations: Vec::new(),
                });
                collect_expected_rules(
                    &media.rules,
                    Some(ordinal),
                    targets,
                    minify,
                    plans,
                    plan_index,
                    output,
                )?;
            }
            CssRule::Container(container) => {
                let ordinal = u32::try_from(output.len()).map_err(|_| INVALID)?;
                let mut header = String::from("@container");
                if let Some(name) = &container.name {
                    header.push(' ');
                    header.push_str(
                        &name
                            .to_css_string(printer(minify, targets))
                            .map_err(|_| INVALID)?,
                    );
                }
                if let Some(condition) = &container.condition {
                    header.push(' ');
                    header.push_str(
                        &condition
                            .to_css_string(printer(minify, targets))
                            .map_err(|_| INVALID)?,
                    );
                }
                output.push(ExpectedRule {
                    kind: "container",
                    header,
                    parent,
                    declarations: Vec::new(),
                });
                collect_expected_rules(
                    &container.rules,
                    Some(ordinal),
                    targets,
                    minify,
                    plans,
                    plan_index,
                    output,
                )?;
            }
            CssRule::Style(style) => {
                valid(style.rules.0.is_empty())?;
                let plan = plans.get(*plan_index).ok_or(INVALID)?;
                *plan_index += 1;
                let mut lineage = plan.declarations.clone().unwrap_or_else(|| {
                    style
                        .declarations
                        .iter()
                        .map(|(_, important)| TraceDeclaration::new(Vec::new(), important, false))
                        .collect()
                });
                lineage.sort_by_key(|item| item.important);
                let properties = style.declarations.iter().collect::<Vec<_>>();
                valid(lineage.len() == properties.len())?;
                let mut declarations = Vec::new();
                for ((property, important), lineage) in properties.into_iter().zip(lineage) {
                    valid(important == lineage.important)?;
                    let serialized = property
                        .to_css_string(important, printer(minify, targets))
                        .map_err(|_| INVALID)?;
                    let producer = if plan.style_id.is_some() {
                        Producer::Semantic {
                            style_id: plan.style_id.ok_or(INVALID)?,
                            ordinals: lineage.semantic_ordinals,
                            generated: lineage.generated,
                        }
                    } else {
                        Producer::Theme
                    };
                    for text in split_declarations(&serialized)? {
                        declarations.push(ExpectedDeclaration {
                            text: text.to_owned(),
                            producer: producer.clone(),
                        });
                        valid(declarations.len() <= MAX_TRACE_ITEMS)?;
                    }
                }
                output.push(ExpectedRule {
                    kind: "qualified",
                    header: style
                        .selectors
                        .to_css_string(printer(minify, targets))
                        .map_err(|_| INVALID)?,
                    parent,
                    declarations,
                });
            }
            _ => {
                return Err(
                    "physical trace supports only qualified, media, container, and layer rules"
                        .into(),
                );
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn collect_actual_rules<'a>(
    source: &CssRuleList<'a>,
    parent: Option<u32>,
    css: &'a str,
    lines: &LineIndex,
    output: &mut Vec<ActualRule>,
) -> Result<(), String> {
    for rule in &source.0 {
        valid(output.len() < MAX_TRACE_ITEMS)?;
        match rule {
            CssRule::LayerStatement(layer) => {
                let start = location_offset(lines, css, layer.loc)?;
                let semicolon = css
                    .get(start..)
                    .and_then(|value| value.find(';').map(|offset| start + offset))
                    .ok_or(INVALID)?;
                let header_end = trim_ascii_end(css, start, semicolon);
                output.push(ActualRule {
                    kind: "layer-order",
                    header: css[start..header_end].to_owned(),
                    parent,
                    byte_start: start,
                    byte_end: semicolon + 1,
                    header_end,
                    declarations: Vec::new(),
                });
            }
            CssRule::LayerBlock(layer) => {
                let ordinal = u32::try_from(output.len()).map_err(|_| INVALID)?;
                let start = location_offset(lines, css, layer.loc)?;
                output.push(ActualRule {
                    kind: "layer",
                    header: String::new(),
                    parent,
                    byte_start: start,
                    byte_end: 0,
                    header_end: 0,
                    declarations: Vec::new(),
                });
                let first_child = output.len();
                collect_actual_rules(&layer.rules, Some(ordinal), css, lines, output)?;
                valid(output.len() > first_child)?;
                let child_start = output[first_child].byte_start;
                let child_end = output[first_child..]
                    .iter()
                    .rev()
                    .find(|rule| rule.parent == Some(ordinal))
                    .ok_or(INVALID)?
                    .byte_end;
                let open = find_open(css, start, child_start)?;
                let end = find_close(css, child_end)?;
                let header_end = trim_ascii_end(css, start, open);
                css[start..header_end].clone_into(&mut output[ordinal as usize].header);
                output[ordinal as usize].header_end = header_end;
                output[ordinal as usize].byte_end = end;
            }
            CssRule::Media(media) => {
                let ordinal = u32::try_from(output.len()).map_err(|_| INVALID)?;
                let start = location_offset(lines, css, media.loc)?;
                output.push(ActualRule {
                    kind: "media",
                    header: String::new(),
                    parent,
                    byte_start: start,
                    byte_end: 0,
                    header_end: 0,
                    declarations: Vec::new(),
                });
                let first_child = output.len();
                collect_actual_rules(&media.rules, Some(ordinal), css, lines, output)?;
                valid(output.len() > first_child)?;
                let child_start = output[first_child].byte_start;
                let child_end = output[first_child..]
                    .iter()
                    .rev()
                    .find(|rule| rule.parent == Some(ordinal))
                    .ok_or(INVALID)?
                    .byte_end;
                let open = find_open(css, start, child_start)?;
                let end = find_close(css, child_end)?;
                let header_end = trim_ascii_end(css, start, open);
                css[start..header_end].clone_into(&mut output[ordinal as usize].header);
                output[ordinal as usize].header_end = header_end;
                output[ordinal as usize].byte_end = end;
            }
            CssRule::Container(container) => {
                let ordinal = u32::try_from(output.len()).map_err(|_| INVALID)?;
                let start = location_offset(lines, css, container.loc)?;
                output.push(ActualRule {
                    kind: "container",
                    header: String::new(),
                    parent,
                    byte_start: start,
                    byte_end: 0,
                    header_end: 0,
                    declarations: Vec::new(),
                });
                let first_child = output.len();
                collect_actual_rules(&container.rules, Some(ordinal), css, lines, output)?;
                valid(output.len() > first_child)?;
                let child_start = output[first_child].byte_start;
                let child_end = output[first_child..]
                    .iter()
                    .rev()
                    .find(|rule| rule.parent == Some(ordinal))
                    .ok_or(INVALID)?
                    .byte_end;
                let open = find_open(css, start, child_start)?;
                let end = find_close(css, child_end)?;
                let header_end = trim_ascii_end(css, start, open);
                css[start..header_end].clone_into(&mut output[ordinal as usize].header);
                output[ordinal as usize].header_end = header_end;
                output[ordinal as usize].byte_end = end;
            }
            CssRule::Style(style) => {
                let rule_ordinal = output.len();
                valid(style.rules.0.is_empty())?;
                let start = location_offset(lines, css, style.loc).map_err(|error| {
                    format!("cannot locate qualified rule {rule_ordinal}: {error}")
                })?;
                let rule_css = css.get(start..).ok_or(INVALID)?;
                let rule_lines = LineIndex::new(rule_css);
                let mut located = style.clone();
                located.loc = Location {
                    source_index: 0,
                    line: 0,
                    column: 1,
                };
                let declarations = style
                    .declarations
                    .iter()
                    .enumerate()
                    .map(|(index, (_, important))| {
                        let (property, value) = located
                            .property_location(rule_css, index)
                            .map_err(|_| {
                                format!(
                                    "cannot locate declaration {index} in qualified rule {rule_ordinal}"
                                )
                            })?;
                        let property_start = start
                            + rule_lines.offset(
                                rule_css,
                                property.start.line,
                                property.start.column,
                            )?;
                        let property_end = start
                            + rule_lines.offset(
                                rule_css,
                                property.end.line,
                                property.end.column,
                            )?;
                        let value_start = start
                            + rule_lines.offset(rule_css, value.start.line, value.start.column)?;
                        let full_end = start
                            + rule_lines.offset(rule_css, value.end.line, value.end.column)?;
                        let value_end = semantic_value_end(css, value_start, full_end, important)?;
                        if !(start <= property_start
                            && property_start < property_end
                            && property_end <= value_start
                            && value_start < value_end
                            && value_end <= full_end
                            && full_end <= css.len())
                        {
                            return Err(format!(
                                "invalid declaration {index} range in qualified rule {rule_ordinal}: rule={start}, property={property_start}..{property_end}, value={value_start}..{value_end}, full={full_end}"
                            ));
                        }
                        Ok(ActualDeclaration {
                            text: css[property_start..full_end].to_owned(),
                            property: css[property_start..property_end].to_owned(),
                            important,
                            byte_start: property_start,
                            byte_end: full_end,
                            property_start,
                            property_end,
                            value_start,
                            value_end,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let anchor = declarations.last().map_or(start, |item| item.byte_end);
                let open = if let Some(first) = declarations.first() {
                    find_open(css, start, first.byte_start)?
                } else {
                    css.get(start..)
                        .and_then(|value| value.find('{').map(|offset| start + offset))
                        .ok_or(INVALID)?
                };
                let end = find_close(css, anchor.max(open + 1)).map_err(|error| {
                    format!("cannot close qualified rule {rule_ordinal}: {error}")
                })?;
                let header_end = trim_ascii_end(css, start, open);
                output.push(ActualRule {
                    kind: "qualified",
                    header: css[start..header_end].to_owned(),
                    parent,
                    byte_start: start,
                    byte_end: end,
                    header_end,
                    declarations,
                });
            }
            _ => {
                return Err(
                    "physical trace supports only qualified, media, container, and layer rules"
                        .into(),
                );
            }
        }
    }
    Ok(())
}

fn validate_actual_ranges(css: &str, rules: &[ActualRule]) -> Result<(), String> {
    let mut previous_start = None;
    let mut sibling_ends = BTreeMap::new();
    for (index, rule) in rules.iter().enumerate() {
        valid(
            rule.byte_start < rule.header_end
                && rule.header_end < rule.byte_end
                && rule.byte_end <= css.len()
                && css.is_char_boundary(rule.byte_start)
                && css.is_char_boundary(rule.header_end)
                && css.is_char_boundary(rule.byte_end),
        )
        .map_err(|_| format!("invalid physical rule range {index}"))?;
        if let Some(start) = previous_start {
            valid(start < rule.byte_start)
                .map_err(|_| format!("physical rule {index} is not ordered"))?;
        }
        previous_start = Some(rule.byte_start);
        if let Some(end) = sibling_ends.insert(rule.parent, rule.byte_end) {
            valid(end <= rule.byte_start && ascii_whitespace(css, end, rule.byte_start))
                .map_err(|_| format!("physical rule {index} overlaps a sibling"))?;
        }
        if let Some(parent) = rule.parent {
            let parent_index = usize::try_from(parent).map_err(|_| INVALID)?;
            let parent = rules.get(parent_index).ok_or(INVALID)?;
            valid(
                matches!(parent.kind, "media" | "container" | "layer")
                    && parent_index < index
                    && parent.byte_start < rule.byte_start
                    && rule.byte_end < parent.byte_end,
            )
            .map_err(|_| {
                format!(
                    "physical rule {index} ({kind} {start}..{end}) escapes parent {parent_index} ({parent_kind} {parent_start}..{parent_end})",
                    kind = rule.kind,
                    start = rule.byte_start,
                    end = rule.byte_end,
                    parent_kind = parent.kind,
                    parent_start = parent.byte_start,
                    parent_end = parent.byte_end,
                )
            })?;
        }
        if matches!(rule.kind, "media" | "container" | "layer" | "layer-order") {
            valid(rule.declarations.is_empty())
                .map_err(|_| format!("physical at-rule {index} owns declarations"))?;
            continue;
        }
        valid(rule.kind == "qualified")
            .map_err(|_| format!("unsupported physical rule kind at {index}"))?;
        let mut previous_end = None;
        for declaration in &rule.declarations {
            valid(
                previous_end.unwrap_or(rule.header_end) <= declaration.byte_start
                    && rule.header_end < declaration.byte_start
                    && declaration.byte_start < declaration.byte_end
                    && declaration.byte_end < rule.byte_end
                    && css.is_char_boundary(declaration.byte_start)
                    && css.is_char_boundary(declaration.byte_end)
                    && css.is_char_boundary(declaration.property_start)
                    && css.is_char_boundary(declaration.property_end)
                    && css.is_char_boundary(declaration.value_start)
                    && css.is_char_boundary(declaration.value_end),
            )
            .map_err(|_| format!("invalid declaration range in physical rule {index}"))?;
            if let Some(end) = previous_end {
                valid(declaration_separator(css, end, declaration.byte_start))?;
            }
            previous_end = Some(declaration.byte_end);
        }
    }
    let mut top_level_end = 0;
    for rule in rules.iter().filter(|rule| rule.parent.is_none()) {
        valid(
            top_level_end <= rule.byte_start
                && ascii_whitespace(css, top_level_end, rule.byte_start),
        )
        .map_err(|_| {
            "top-level physical rules overlap or contain non-whitespace gaps".to_owned()
        })?;
        top_level_end = rule.byte_end;
    }
    valid(ascii_whitespace(css, top_level_end, css.len()))
        .map_err(|_| "final physical rule is followed by non-whitespace CSS".to_owned())?;
    Ok(())
}

fn ascii_whitespace(css: &str, start: usize, end: usize) -> bool {
    css.get(start..end)
        .is_some_and(|value| value.bytes().all(|byte| byte.is_ascii_whitespace()))
}

fn declaration_separator(css: &str, start: usize, end: usize) -> bool {
    css.get(start..end).is_some_and(|value| {
        value
            .bytes()
            .all(|byte| byte == b';' || byte.is_ascii_whitespace())
    })
}

fn split_declarations(value: &str) -> Result<Vec<&str>, String> {
    let bytes = value.as_bytes();
    let (mut start, mut index, mut depth) = (0, 0, 0_u32);
    let mut quote = None;
    let mut comment = false;
    let mut parts = Vec::new();
    while index < bytes.len() {
        let byte = bytes[index];
        if comment {
            if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(delimiter) = quote {
            if byte == b'\\' {
                index = (index + 2).min(bytes.len());
            } else {
                if byte == delimiter {
                    quote = None;
                }
                index += 1;
            }
            continue;
        }
        match byte {
            b'\\' => {
                valid(index + 1 < bytes.len())?;
                index += 2;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                comment = true;
                index += 2;
            }
            b'\'' | b'"' => {
                quote = Some(byte);
                index += 1;
            }
            b'(' | b'[' | b'{' => {
                depth = depth.checked_add(1).ok_or(INVALID)?;
                index += 1;
            }
            b')' | b']' | b'}' => {
                depth = depth.checked_sub(1).ok_or(INVALID)?;
                index += 1;
            }
            b';' if depth == 0 => {
                if let Some(part) = trimmed(&value[start..index]) {
                    parts.push(part);
                }
                start = index + 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    valid(!comment && quote.is_none() && depth == 0)?;
    if let Some(part) = trimmed(&value[start..]) {
        parts.push(part);
    }
    valid(!parts.is_empty())?;
    Ok(parts)
}

fn semantic_value_end(
    css: &str,
    start: usize,
    end: usize,
    important: bool,
) -> Result<usize, String> {
    if !important {
        return Ok(end);
    }
    let trimmed_end = trim_ascii_end(css, start, end);
    let value = &css[start..trimmed_end];
    let suffix = "!important";
    valid(
        value.len() >= suffix.len()
            && value[value.len() - suffix.len()..].eq_ignore_ascii_case(suffix),
    )?;
    Ok(trim_ascii_end(css, start, trimmed_end - suffix.len()))
}

fn find_open(css: &str, start: usize, bound: usize) -> Result<usize, String> {
    css.get(start..bound)
        .and_then(|value| value.rfind('{').map(|offset| start + offset))
        .ok_or_else(|| INVALID.to_owned())
}

fn find_close(css: &str, mut start: usize) -> Result<usize, String> {
    while let Some(byte) = css.as_bytes().get(start) {
        if *byte == b'}' {
            return Ok(start + 1);
        }
        if !byte.is_ascii_whitespace() && *byte != b';' {
            return Err(INVALID.into());
        }
        start += 1;
    }
    Err(INVALID.into())
}

fn trim_ascii_end(css: &str, start: usize, mut end: usize) -> usize {
    while end > start && css.as_bytes()[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    end
}

fn trimmed(value: &str) -> Option<&str> {
    let value = value.trim_matches(|character: char| character.is_ascii_whitespace());
    (!value.is_empty()).then_some(value)
}

fn location_offset(lines: &LineIndex, css: &str, location: Location) -> Result<usize, String> {
    valid(location.source_index == 0)?;
    lines.offset(css, location.line, location.column)
}

fn printer(minify: bool, targets: Targets) -> PrinterOptions<'static> {
    PrinterOptions {
        minify,
        targets,
        ..PrinterOptions::default()
    }
}

fn edge(
    edges: &mut BTreeSet<PhysicalEdge>,
    kind: &'static str,
    from: String,
    to: String,
) -> Result<(), String> {
    edges.insert(PhysicalEdge { kind, from, to });
    valid(edges.len() <= MAX_TRACE_ITEMS)
}

fn to_u64(value: usize) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| INVALID.to_owned())
}

fn valid(condition: bool) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| INVALID.to_owned())
}

#[cfg(all(test, not(feature = "package-verify")))]
#[path = "../../tests/internal/physical_trace.rs"]
mod tests;
