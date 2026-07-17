use core::{cmp::Ordering, fmt};
use std::collections::{BTreeMap, BTreeSet};

use pliego_css_ir::{
    Assignment, CASCADE_LAYER_ORDER_CSS, CascadeLayer, ColorValue, Condition, ConditionId,
    ContrastPreference, CssNumber, Keyword, Length, LengthUnit, MotionPreference, Percentage,
    SemanticStyle, SemanticValue, Slot, SlotSet, StyleId, ThemeMode, TokenKind, TokenRef, Utility,
};
use pliego_css_parser::{
    is_valid_arbitrary_property_name, is_valid_custom_property_name, validate_arbitrary_value_text,
};
use pliego_css_theme::ThemeRegistry;

use super::{derive_style_id_with_theme, seed_theme, validate_selector_transform_text};

/// Error returned while turning valid semantic IR into CSS.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmitError {
    /// The style has not received a semantic identity.
    UnresolvedStyleId,
    /// Semantic IR invariants are broken.
    InvalidIr(String),
    /// A token ID is not present in the active theme registry.
    UnknownToken {
        /// Namespace of the unresolved token.
        kind: TokenKind,
        /// Stable numeric token identifier.
        id: u32,
    },
    /// A condition references an unknown breakpoint.
    UnknownBreakpoint(u16),
    /// The style identity was derived under a different theme or hash contract.
    StyleIdentityMismatch {
        /// Identity required by the active theme and semantic payload.
        expected: u128,
        /// Identity carried by the semantic style.
        actual: u128,
    },
}

impl fmt::Display for EmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedStyleId => formatter.write_str("style ID is unresolved"),
            Self::InvalidIr(error) => write!(formatter, "invalid semantic IR: {error}"),
            Self::UnknownToken { kind, id } => write!(formatter, "unknown {kind:?} token ID {id}"),
            Self::UnknownBreakpoint(id) => write!(formatter, "unknown breakpoint ID {id}"),
            Self::StyleIdentityMismatch { expected, actual } => write!(
                formatter,
                "style identity does not match the active theme (expected {expected:032x}, got {actual:032x})"
            ),
        }
    }
}

impl std::error::Error for EmitError {}

/// Encodes a semantic style ID as a compact CSS-safe class name.
#[must_use]
pub fn class_name(id: StyleId) -> String {
    id.to_class_name()
}

/// Physical-declaration lineage for one emitted style.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleLineage {
    /// Semantic identity carried by the emitted style.
    pub style_id: StyleId,
    /// Qualified CSS rules in emitted byte order.
    pub rules: Vec<RuleLineage>,
}

/// Physical-declaration lineage for one qualified CSS rule.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleLineage {
    /// Declarations in emitted byte order.
    pub declarations: Vec<DeclarationLineage>,
}

/// Semantic owners and emission flags for one physical CSS declaration.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationLineage {
    /// Zero-based canonical semantic-assignment ordinals that caused this declaration.
    pub semantic_ordinals: Vec<u32>,
    /// Whether the physical declaration carries `!important`.
    pub important: bool,
    /// Whether the emitter synthesized the declaration to compose semantic effects.
    pub generated: bool,
}

/// Reuses deterministic CSS fragments for unchanged canonical semantic streams.
///
/// This cache is an implementation surface for incremental tooling. Callers remain responsible for
/// deriving collision-checked canonical streams and pruning entries after each complete snapshot.
#[doc(hidden)]
#[derive(Default)]
pub struct CssFragmentCache {
    fragments: BTreeMap<Vec<u8>, String>,
}

impl CssFragmentCache {
    /// Returns the number of retained canonical fragments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fragments.len()
    }

    /// Returns whether no canonical fragments are retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fragments.is_empty()
    }

    /// Returns one cached or newly emitted fragment and whether it was a cache hit.
    ///
    /// # Errors
    ///
    /// Returns the ordinary emitter error when a missing fragment cannot be emitted.
    pub fn emit<'a>(
        &'a mut self,
        stream: &[u8],
        theme: &ThemeRegistry,
        style: &SemanticStyle,
    ) -> Result<(&'a str, bool), EmitError> {
        use std::collections::btree_map::Entry;

        match self.fragments.entry(stream.to_vec()) {
            Entry::Occupied(entry) => Ok((entry.into_mut().as_str(), true)),
            Entry::Vacant(entry) => {
                let css = emit_css_with_theme(theme, style)?;
                Ok((entry.insert(css).as_str(), false))
            }
        }
    }

    /// Removes fragments that are not part of the latest complete semantic snapshot.
    pub fn retain<'a, I>(&mut self, streams: I) -> usize
    where
        I: IntoIterator<Item = &'a Vec<u8>>,
    {
        let active = streams
            .into_iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        let before = self.fragments.len();
        self.fragments.retain(|stream, _| active.contains(stream));
        before - self.fragments.len()
    }
}

/// Emits deterministic minified CSS for one validated semantic style.
///
/// # Errors
///
/// Returns an error for unresolved IDs, invalid IR, unknown tokens, or unknown breakpoints.
pub fn emit_css(style: &SemanticStyle) -> Result<String, EmitError> {
    emit_css_with_theme(seed_theme(), style)
}

/// Emits deterministic minified CSS using an explicit theme registry.
///
/// # Errors
///
/// Returns an error for unresolved IDs, invalid IR, unknown tokens, or unknown breakpoints.
pub fn emit_css_with_theme(
    theme: &ThemeRegistry,
    style: &SemanticStyle,
) -> Result<String, EmitError> {
    emit_css_with_theme_inner::<false>(theme, style).map(|(css, _)| css)
}

/// Emits CSS together with physical-declaration lineage using an explicit theme registry.
///
/// # Errors
///
/// Returns an error for unresolved IDs, invalid IR, unknown tokens, or unknown breakpoints.
#[doc(hidden)]
pub fn emit_css_with_theme_traced(
    theme: &ThemeRegistry,
    style: &SemanticStyle,
) -> Result<(String, StyleLineage), EmitError> {
    emit_css_with_theme_inner::<true>(theme, style)
}

fn emit_css_with_theme_inner<const TRACE: bool>(
    theme: &ThemeRegistry,
    style: &SemanticStyle,
) -> Result<(String, StyleLineage), EmitError> {
    if style.id.is_unresolved() {
        return Err(EmitError::UnresolvedStyleId);
    }
    style
        .validate()
        .map_err(|error| EmitError::InvalidIr(error.to_string()))?;

    let class = class_name(style.id);
    let mut effect_ordinals = Vec::new();
    let mut needs_effect_initialization = false;
    let mut groups: BTreeMap<ConditionId, Vec<(u32, &Assignment)>> = BTreeMap::new();
    for (ordinal, assignment) in style.assignments.iter().enumerate() {
        let effect = matches!(
            assignment.utility,
            Utility::BoxShadow | Utility::RingWidth | Utility::RingColor
        );
        needs_effect_initialization |= effect;
        let ordinal = if TRACE {
            u32::try_from(ordinal).map_err(|_| {
                EmitError::InvalidIr("semantic assignment ordinal exceeds u32".into())
            })?
        } else {
            0
        };
        if TRACE && effect {
            effect_ordinals.push(ordinal);
        }
        groups
            .entry(assignment.condition)
            .or_default()
            .push((ordinal, assignment));
    }
    let mut groups = groups
        .into_iter()
        .map(|(id, assignments)| (&style.conditions[id.index()], assignments))
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| compare_resolved_conditions(style, left.0, right.0));

    let mut css = String::new();
    let mut rules = Vec::new();
    let has_layer = groups
        .iter()
        .any(|(condition, _)| condition.layer != CascadeLayer::Unlayered);
    if has_layer {
        css.push_str(CASCADE_LAYER_ORDER_CSS);
    }
    let has_base_group = groups.iter().any(|(condition, _)| condition.is_base());
    if needs_effect_initialization && (has_layer || !has_base_group) {
        let initialization = format!(".{class}{{{EFFECT_INITIALIZERS}}}");
        if has_layer {
            css.push_str("@layer pliego.base{");
            css.push_str(&initialization);
            css.push('}');
        } else {
            css.push_str(&initialization);
        }
        if TRACE {
            rules.push(RuleLineage {
                declarations: generated_lineage(&effect_ordinals, 3),
            });
        }
    }
    for (condition, assignments) in groups {
        let selector = selector(&class, condition, style)?;
        let (mut declarations, mut declaration_lineage) =
            declarations::<TRACE>(theme, style, &assignments)?;
        if needs_effect_initialization && !has_layer && *condition == Condition::default() {
            declarations.insert_str(0, EFFECT_INITIALIZERS);
            if TRACE {
                let mut initializers = generated_lineage(&effect_ordinals, 3);
                initializers.append(&mut declaration_lineage);
                declaration_lineage = initializers;
            }
        }
        let rule = format!("{selector}{{{declarations}}}");
        css.push_str(&wrap_condition(theme, rule, condition)?);
        if TRACE {
            rules.push(RuleLineage {
                declarations: declaration_lineage,
            });
        }
    }
    let expected = derive_style_id_with_theme(theme, style);
    if style.id != expected {
        return Err(EmitError::StyleIdentityMismatch {
            expected: expected.get(),
            actual: style.id.get(),
        });
    }
    Ok((
        css,
        StyleLineage {
            style_id: style.id,
            rules,
        },
    ))
}

const EFFECT_INITIALIZERS: &str =
    "--pc-shadow:0 0 #0000;--pc-ring-width:0;--pc-ring-color:currentColor;";

fn generated_lineage(ordinals: &[u32], count: usize) -> Vec<DeclarationLineage> {
    (0..count)
        .map(|_| DeclarationLineage {
            semantic_ordinals: ordinals.to_vec(),
            important: false,
            generated: true,
        })
        .collect()
}

/// Emits the seed design tokens used by the Gate-A catalog.
#[must_use]
pub fn emit_seed_theme() -> String {
    emit_theme(seed_theme())
}

/// Emits deterministic CSS custom properties required by registry-backed values.
#[must_use]
pub fn emit_theme(theme: &ThemeRegistry) -> String {
    emit_theme_subset(theme, None)
}

/// Emits only custom properties referenced by the supplied retained semantic styles.
#[must_use]
pub fn emit_used_theme<'a>(
    theme: &ThemeRegistry,
    styles: impl IntoIterator<Item = &'a SemanticStyle>,
) -> String {
    let used = styles
        .into_iter()
        .flat_map(|style| style.assignments.iter())
        .filter_map(|assignment| match assignment.value {
            SemanticValue::Token(reference) => Some(reference),
            SemanticValue::Color(ColorValue::Token { id, .. }) => Some(TokenRef {
                kind: TokenKind::Color,
                id,
            }),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    emit_theme_subset(theme, Some(&used))
}

fn emit_theme_subset(theme: &ThemeRegistry, used: Option<&BTreeSet<TokenRef>>) -> String {
    let mut declarations = theme
        .tokens()
        .iter()
        .filter_map(|token| {
            if used.is_some_and(|used| {
                !used.contains(&TokenRef {
                    kind: token.kind,
                    id: token.id,
                })
            }) {
                return None;
            }
            let prefix = match token.kind {
                TokenKind::Color
                    if token.name != "transparent"
                        && token.name != "current"
                        && token.name != "white" =>
                {
                    "color"
                }
                TokenKind::FontFamily => "font",
                _ => return None,
            };
            Some((format!("--{prefix}-{}", token.name), token.value.as_str()))
        })
        .collect::<Vec<_>>();
    declarations.sort_unstable_by(|left, right| left.0.cmp(&right.0));

    let mut css = String::from(":root{");
    for (name, value) in declarations {
        css.push_str(&name);
        css.push(':');
        css.push_str(value);
        css.push(';');
    }
    css.push('}');
    css
}

fn selector(
    class: &str,
    condition: &Condition,
    style: &SemanticStyle,
) -> Result<String, EmitError> {
    let mut selector = format!(".{class}");
    for state in [
        pliego_css_ir::PseudoState::Hover,
        pliego_css_ir::PseudoState::Focus,
        pliego_css_ir::PseudoState::FocusVisible,
        pliego_css_ir::PseudoState::Active,
        pliego_css_ir::PseudoState::Disabled,
    ] {
        if condition.states.contains(state) {
            selector.push(':');
            selector.push_str(match state {
                pliego_css_ir::PseudoState::Hover => "hover",
                pliego_css_ir::PseudoState::Focus => "focus",
                pliego_css_ir::PseudoState::FocusVisible => "focus-visible",
                pliego_css_ir::PseudoState::Active => "active",
                pliego_css_ir::PseudoState::Disabled => "disabled",
            });
        }
    }
    for transform in &condition.selectors {
        let transform = &style.selectors[transform.index()];
        validate_selector_transform_text(transform)
            .map_err(|error| EmitError::InvalidIr(error.to_string()))?;
        selector.push_str(
            transform
                .strip_prefix('&')
                .expect("validated selector transform must start with an anchor"),
        );
    }
    Ok(match condition.theme {
        ThemeMode::Any => selector,
        ThemeMode::Light => format!("[data-theme=light] {selector}"),
        ThemeMode::Dark => format!("[data-theme=dark] {selector}"),
    })
}

fn compare_resolved_conditions(
    style: &SemanticStyle,
    left: &Condition,
    right: &Condition,
) -> Ordering {
    left.layer
        .cmp(&right.layer)
        .then_with(|| left.breakpoint.cmp(&right.breakpoint))
        .then_with(|| left.container_breakpoint.cmp(&right.container_breakpoint))
        .then_with(|| left.theme.cmp(&right.theme))
        .then_with(|| left.motion.cmp(&right.motion))
        .then_with(|| left.contrast.cmp(&right.contrast))
        .then_with(|| left.states.cmp(&right.states))
        .then_with(|| {
            left.selectors
                .iter()
                .map(|selector| style.selectors[selector.index()].as_str())
                .cmp(
                    right
                        .selectors
                        .iter()
                        .map(|selector| style.selectors[selector.index()].as_str()),
                )
        })
}

fn wrap_condition(
    theme: &ThemeRegistry,
    mut rule: String,
    condition: &Condition,
) -> Result<String, EmitError> {
    let mut media = Vec::new();
    if let Some(breakpoint) = condition.breakpoint {
        let definition = theme
            .breakpoint_by_id(breakpoint)
            .ok_or_else(|| EmitError::UnknownBreakpoint(breakpoint.get()))?;
        media.push(format!("(min-width:{})", definition.min_width));
    }
    match condition.motion {
        MotionPreference::Any => {}
        MotionPreference::Safe => media.push("(prefers-reduced-motion:no-preference)".into()),
        MotionPreference::Reduce => media.push("(prefers-reduced-motion:reduce)".into()),
    }
    match condition.contrast {
        ContrastPreference::Any => {}
        ContrastPreference::More => media.push("(prefers-contrast:more)".into()),
        ContrastPreference::Less => media.push("(prefers-contrast:less)".into()),
    }
    if let Some(breakpoint) = condition.container_breakpoint {
        let definition = theme
            .breakpoint_by_id(breakpoint)
            .ok_or_else(|| EmitError::UnknownBreakpoint(breakpoint.get()))?;
        rule = format!("@container (min-width:{}){{{rule}}}", definition.min_width);
    }
    if !media.is_empty() {
        rule = format!("@media {}{{{rule}}}", media.join(" and "));
    }
    if let Some(layer) = condition.layer.css_name() {
        rule = format!("@layer {layer}{{{rule}}}");
    }
    Ok(rule)
}

// Keeping the exhaustive utility mapping together makes unsupported IR fail visibly.
#[allow(clippy::too_many_lines)]
fn declarations<const TRACE: bool>(
    theme: &ThemeRegistry,
    style: &SemanticStyle,
    assignments: &[(u32, &Assignment)],
) -> Result<(String, Vec<DeclarationLineage>), EmitError> {
    let mut declarations = String::new();
    let mut lineage = Vec::new();
    let mut effects = false;
    let mut effect_ordinals = Vec::new();
    for &(ordinal, assignment) in assignments {
        let value = value(theme, style, assignment)?;
        let important = if assignment.important {
            "!important"
        } else {
            ""
        };
        let mut push = |property: &str, value: &str| {
            declarations.push_str(property);
            declarations.push(':');
            declarations.push_str(value);
            declarations.push_str(important);
            declarations.push(';');
            if TRACE {
                lineage.push(DeclarationLineage {
                    semantic_ordinals: vec![ordinal],
                    important: assignment.important,
                    generated: false,
                });
            }
        };
        match assignment.utility {
            Utility::Display => push("display", &value),
            Utility::ContainerType => push("container-type", &value),
            Utility::WritingMode => push("writing-mode", &value),
            Utility::Width => push("width", &value),
            Utility::MinWidth => push("min-width", &value),
            Utility::MaxWidth => push("max-width", &value),
            Utility::Height => push("height", &value),
            Utility::MinHeight => push("min-height", &value),
            Utility::MaxHeight => push("max-height", &value),
            Utility::AspectRatio => {
                if let SemanticValue::Fraction {
                    numerator,
                    denominator,
                } = assignment.value
                {
                    push("aspect-ratio", &format!("{numerator}/{denominator}"));
                } else {
                    push("aspect-ratio", &value);
                }
            }
            Utility::FlexDirection => push("flex-direction", &value),
            Utility::FlexWrap => push("flex-wrap", &value),
            Utility::FlexGrow => push("flex-grow", &value),
            Utility::FlexShrink => push("flex-shrink", &value),
            Utility::FlexBasis => push("flex-basis", &value),
            Utility::AlignItems => push("align-items", &value),
            Utility::JustifyContent => push("justify-content", &value),
            Utility::GridTemplateColumns => {
                if let SemanticValue::Integer(columns) = assignment.value {
                    push(
                        "grid-template-columns",
                        &format!("repeat({columns},minmax(0,1fr))"),
                    );
                } else {
                    push("grid-template-columns", &value);
                }
            }
            Utility::GridColumn => {
                if let SemanticValue::Integer(span) = assignment.value {
                    push("grid-column", &format!("span {span}/span {span}"));
                } else {
                    push("grid-column", &value);
                }
            }
            Utility::Gap => push_slots(
                &mut push,
                assignment.slots,
                SlotSet::GAP,
                "gap",
                &[(Slot::GapRow, "row-gap"), (Slot::GapColumn, "column-gap")],
                &value,
            ),
            Utility::Padding => push_slots(
                &mut push,
                assignment.slots,
                SlotSet::PADDING,
                "padding",
                &[
                    (Slot::PaddingTop, "padding-top"),
                    (Slot::PaddingRight, "padding-right"),
                    (Slot::PaddingBottom, "padding-bottom"),
                    (Slot::PaddingLeft, "padding-left"),
                ],
                &value,
            ),
            Utility::Margin => push_slots(
                &mut push,
                assignment.slots,
                SlotSet::MARGIN,
                "margin",
                &[
                    (Slot::MarginTop, "margin-top"),
                    (Slot::MarginRight, "margin-right"),
                    (Slot::MarginBottom, "margin-bottom"),
                    (Slot::MarginLeft, "margin-left"),
                ],
                &value,
            ),
            Utility::BackgroundColor => push("background-color", &value),
            Utility::TextColor => push("color", &value),
            Utility::FontFamily => push("font-family", &value),
            Utility::FontSize => {
                push("font-size", &value);
                if let SemanticValue::Token(reference) = assignment.value {
                    let line_height = match token_name(theme, reference) {
                        Some("xs") => Some("1rem"),
                        Some("sm") => Some("1.25rem"),
                        Some("base") => Some("1.5rem"),
                        Some("lg" | "xl") => Some("1.75rem"),
                        Some("2xl") => Some("2rem"),
                        Some("3xl") => Some("2.25rem"),
                        _ => None,
                    };
                    if let Some(line_height) = line_height {
                        push("line-height", line_height);
                    }
                }
            }
            Utility::FontWeight => push("font-weight", &value),
            Utility::LineHeight => push("line-height", &value),
            Utility::LetterSpacing => push("letter-spacing", &value),
            Utility::FontSmoothing => {
                push("-webkit-font-smoothing", "antialiased");
                push("-moz-osx-font-smoothing", "grayscale");
            }
            Utility::BorderWidth => push_slots(
                &mut push,
                assignment.slots,
                SlotSet::BORDER_WIDTH,
                "border-width",
                &[
                    (Slot::BorderTopWidth, "border-top-width"),
                    (Slot::BorderRightWidth, "border-right-width"),
                    (Slot::BorderBottomWidth, "border-bottom-width"),
                    (Slot::BorderLeftWidth, "border-left-width"),
                ],
                &value,
            ),
            Utility::BorderColor => push_slots(
                &mut push,
                assignment.slots,
                SlotSet::BORDER_COLOR,
                "border-color",
                &[
                    (Slot::BorderTopColor, "border-top-color"),
                    (Slot::BorderRightColor, "border-right-color"),
                    (Slot::BorderBottomColor, "border-bottom-color"),
                    (Slot::BorderLeftColor, "border-left-color"),
                ],
                &value,
            ),
            Utility::BorderRadius => push_slots(
                &mut push,
                assignment.slots,
                SlotSet::BORDER_RADIUS,
                "border-radius",
                &[
                    (Slot::BorderTopLeftRadius, "border-top-left-radius"),
                    (Slot::BorderTopRightRadius, "border-top-right-radius"),
                    (Slot::BorderBottomRightRadius, "border-bottom-right-radius"),
                    (Slot::BorderBottomLeftRadius, "border-bottom-left-radius"),
                ],
                &value,
            ),
            Utility::Opacity => push("opacity", &value),
            Utility::OutlineWidth => {
                push("outline-width", &value);
                push("outline-style", "solid");
            }
            Utility::OutlineColor => push("outline-color", &value),
            Utility::OutlineOffset => push("outline-offset", &value),
            Utility::OutlineStyle => push("outline-style", &value),
            Utility::TransitionProperty => {
                push(
                    "transition-property",
                    "color,background-color,border-color,text-decoration-color,fill,stroke",
                );
                push("transition-duration", ".15s");
                push("transition-timing-function", "cubic-bezier(.4,0,.2,1)");
            }
            Utility::Resize => push("resize", &value),
            Utility::Cursor => push("cursor", &value),
            Utility::BoxShadow => {
                push("--pc-shadow", &value);
                effects = true;
                if TRACE {
                    effect_ordinals.push(ordinal);
                }
            }
            Utility::RingWidth => {
                push("--pc-ring-width", &value);
                effects = true;
                if TRACE {
                    effect_ordinals.push(ordinal);
                }
            }
            Utility::RingColor => {
                push("--pc-ring-color", &value);
                effects = true;
                if TRACE {
                    effect_ordinals.push(ordinal);
                }
            }
            Utility::ArbitraryProperty(id) => {
                let name = &style.arbitrary_properties[id.index()].name;
                if !is_valid_arbitrary_property_name(name) {
                    return Err(EmitError::InvalidIr(
                        "unsafe arbitrary property name".into(),
                    ));
                }
                push(name, &value);
            }
            utility => {
                return Err(EmitError::InvalidIr(format!(
                    "Gate-A emitter does not support {utility:?}"
                )));
            }
        }
        if assignment.utility == Utility::BorderWidth {
            push_slots(
                &mut push,
                assignment.slots,
                SlotSet::BORDER_WIDTH,
                "border-style",
                &[
                    (Slot::BorderTopWidth, "border-top-style"),
                    (Slot::BorderRightWidth, "border-right-style"),
                    (Slot::BorderBottomWidth, "border-bottom-style"),
                    (Slot::BorderLeftWidth, "border-left-style"),
                ],
                "solid",
            );
        }
    }
    if effects {
        declarations.push_str(
            "box-shadow:0 0 0 var(--pc-ring-width,0) var(--pc-ring-color,currentColor),var(--pc-shadow,0 0 #0000);",
        );
        if TRACE {
            lineage.push(DeclarationLineage {
                semantic_ordinals: effect_ordinals,
                important: false,
                generated: true,
            });
        }
    }
    Ok((declarations, lineage))
}

fn push_slots(
    push: &mut impl FnMut(&str, &str),
    actual: SlotSet,
    complete: SlotSet,
    shorthand: &str,
    longhands: &[(Slot, &str)],
    value: &str,
) {
    if actual == complete {
        push(shorthand, value);
    } else {
        for (slot, property) in longhands {
            if actual.contains(*slot) {
                push(property, value);
            }
        }
    }
}

fn value(
    theme: &ThemeRegistry,
    style: &SemanticStyle,
    assignment: &Assignment,
) -> Result<String, EmitError> {
    let raw = match assignment.value {
        SemanticValue::Keyword(value) => render_keyword(value).into(),
        SemanticValue::Token(reference) => token_value(theme, assignment.utility, reference)?,
        SemanticValue::Integer(value) => value.to_string(),
        SemanticValue::Number(value) => render_number(value),
        SemanticValue::Length(value) => render_length(value),
        SemanticValue::Percentage(value) => render_percentage(value),
        SemanticValue::Color(value) => render_color(theme, value)?,
        SemanticValue::Fraction {
            numerator,
            denominator,
        } => format!("calc({numerator}/{denominator}*100%)"),
        SemanticValue::Arbitrary(id) => {
            let value = &style.arbitrary_values[id.index()];
            validate_arbitrary_value_text(value)
                .map_err(|error| EmitError::InvalidIr(error.to_string()))?;
            value.clone()
        }
        SemanticValue::CustomProperty(reference) => {
            let name = &style.custom_properties[reference.id.index()];
            if !is_valid_custom_property_name(name) {
                return Err(EmitError::InvalidIr("unsafe custom property name".into()));
            }
            format!("var({name})")
        }
    };
    if assignment.negative {
        Ok(format!("calc({raw}*-1)"))
    } else {
        Ok(raw)
    }
}

fn token_value(
    theme: &ThemeRegistry,
    utility: Utility,
    reference: TokenRef,
) -> Result<String, EmitError> {
    let definition =
        theme
            .token_by_id(reference.kind, reference.id)
            .ok_or(EmitError::UnknownToken {
                kind: reference.kind,
                id: reference.id.get(),
            })?;
    let name = definition.name.as_str();
    Ok(match reference.kind {
        TokenKind::Spacing => spacing_value(utility, name, &definition.value),
        TokenKind::Color => match name {
            "transparent" | "current" | "white" => definition.value.clone(),
            _ => format!("var(--color-{name})"),
        },
        TokenKind::FontFamily => format!("var(--font-{name})"),
        TokenKind::FontSize
        | TokenKind::FontWeight
        | TokenKind::LineHeight
        | TokenKind::LetterSpacing
        | TokenKind::Radius
        | TokenKind::Shadow
        | TokenKind::ZIndex => definition.value.clone(),
    })
}

fn token_name(theme: &ThemeRegistry, reference: TokenRef) -> Option<&str> {
    theme
        .token_by_id(reference.kind, reference.id)
        .map(|definition| definition.name.as_str())
}

fn spacing_value(utility: Utility, name: &str, registered_value: &str) -> String {
    match (utility, name) {
        (Utility::Height | Utility::MinHeight | Utility::MaxHeight, "screen")
            if registered_value == "100vw" =>
        {
            "100vh".into()
        }
        _ => registered_value.into(),
    }
}

const fn render_keyword(value: Keyword) -> &'static str {
    match value {
        Keyword::None => "none",
        Keyword::Auto => "auto",
        Keyword::Normal => "normal",
        Keyword::Block => "block",
        Keyword::Inline => "inline",
        Keyword::InlineBlock => "inline-block",
        Keyword::Flex => "flex",
        Keyword::InlineFlex => "inline-flex",
        Keyword::Grid => "grid",
        Keyword::InlineGrid => "inline-grid",
        Keyword::Visible => "visible",
        Keyword::Hidden => "hidden",
        Keyword::Collapse => "collapse",
        Keyword::Static => "static",
        Keyword::Relative => "relative",
        Keyword::Absolute => "absolute",
        Keyword::Fixed => "fixed",
        Keyword::Sticky => "sticky",
        Keyword::Clip => "clip",
        Keyword::Scroll => "scroll",
        Keyword::Row => "row",
        Keyword::RowReverse => "row-reverse",
        Keyword::Column => "column",
        Keyword::ColumnReverse => "column-reverse",
        Keyword::Wrap => "wrap",
        Keyword::WrapReverse => "wrap-reverse",
        Keyword::NoWrap => "nowrap",
        Keyword::Start => "start",
        Keyword::End => "end",
        Keyword::Center => "center",
        Keyword::SpaceBetween => "space-between",
        Keyword::SpaceAround => "space-around",
        Keyword::SpaceEvenly => "space-evenly",
        Keyword::Stretch => "stretch",
        Keyword::Baseline => "baseline",
        Keyword::MinContent => "min-content",
        Keyword::MaxContent => "max-content",
        Keyword::FitContent => "fit-content",
        Keyword::Content => "content",
        Keyword::Left => "left",
        Keyword::Right => "right",
        Keyword::Justify => "justify",
        Keyword::Bold => "bold",
        Keyword::Underline => "underline",
        Keyword::Overline => "overline",
        Keyword::LineThrough => "line-through",
        Keyword::Solid => "solid",
        Keyword::Dashed => "dashed",
        Keyword::Dotted => "dotted",
        Keyword::Double => "double",
        Keyword::Pointer => "pointer",
        Keyword::Default => "default",
        Keyword::Wait => "wait",
        Keyword::NotAllowed => "not-allowed",
        Keyword::Text => "text",
        Keyword::Move => "move",
        Keyword::Antialiased => "antialiased",
        Keyword::Colors => "colors",
        Keyword::Vertical => "vertical",
        Keyword::InlineSize => "inline-size",
        Keyword::HorizontalTb => "horizontal-tb",
        Keyword::VerticalLr => "vertical-lr",
        Keyword::VerticalRl => "vertical-rl",
    }
}

fn render_number(value: CssNumber) -> String {
    let coefficient = value.coefficient();
    let scale = usize::from(value.scale());
    if scale == 0 {
        return coefficient.to_string();
    }
    let negative = coefficient < 0;
    let mut digits = coefficient.unsigned_abs().to_string();
    if digits.len() <= scale {
        digits.insert_str(0, &"0".repeat(scale + 1 - digits.len()));
    }
    let split = digits.len() - scale;
    digits.insert(split, '.');
    if negative {
        digits.insert(0, '-');
    }
    digits
}

fn render_length(value: Length) -> String {
    let unit = match value.unit {
        LengthUnit::Px => "px",
        LengthUnit::Rem => "rem",
        LengthUnit::Em => "em",
        LengthUnit::Percent => "%",
        LengthUnit::Ch => "ch",
        LengthUnit::Vw => "vw",
        LengthUnit::Vh => "vh",
        LengthUnit::Dvw => "dvw",
        LengthUnit::Dvh => "dvh",
    };
    format!("{}{unit}", render_number(value.number))
}

fn render_percentage(value: Percentage) -> String {
    let basis_points = value.basis_points();
    if basis_points % 100 == 0 {
        format!("{}%", basis_points / 100)
    } else {
        format!("{}.{:02}%", basis_points / 100, basis_points % 100)
    }
}

fn render_color(theme: &ThemeRegistry, value: ColorValue) -> Result<String, EmitError> {
    match value {
        ColorValue::Transparent => Ok("transparent".into()),
        ColorValue::CurrentColor => Ok("currentColor".into()),
        ColorValue::Token { id, alpha } => {
            let definition =
                theme
                    .token_by_id(TokenKind::Color, id)
                    .ok_or(EmitError::UnknownToken {
                        kind: TokenKind::Color,
                        id: id.get(),
                    })?;
            let base = if matches!(
                definition.name.as_str(),
                "transparent" | "current" | "white"
            ) {
                definition.value.clone()
            } else {
                format!("var(--color-{})", definition.name)
            };
            Ok(alpha.map_or(base.clone(), |alpha| {
                format!(
                    "color-mix(in oklab,{base} {},transparent)",
                    render_percentage(alpha)
                )
            }))
        }
        ColorValue::Srgba {
            red,
            green,
            blue,
            alpha,
        } => {
            if alpha == u8::MAX {
                Ok(format!("rgb({red} {green} {blue})"))
            } else {
                let basis_points = u16::try_from((u32::from(alpha) * 10_000 + 127) / 255)
                    .map_err(|_| EmitError::InvalidIr("sRGBA alpha is out of range".into()))?;
                let percentage = Percentage::from_basis_points(basis_points)
                    .map(render_percentage)
                    .ok_or_else(|| EmitError::InvalidIr("sRGBA alpha is out of range".into()))?;
                Ok(format!("rgb({red} {green} {blue}/{percentage})"))
            }
        }
    }
}
