//! Versioned binary encoding for deterministic style identities.

use core::fmt;
use std::sync::OnceLock;

use pliego_css_ir::{
    Assignment, CascadeLayer, ColorValue, ContrastPreference, CssNumber, InvariantError, Keyword,
    Length, LengthUnit, MotionPreference, PseudoState, SemanticStyle, SemanticValue, Slot, StyleId,
    ThemeMode, TokenKind, Utility, ValueKind,
};
use pliego_css_theme::{THEME_ID_FORMAT_VERSION, ThemeRegistry};
use sha2::{Digest, Sha256};

/// Version of the canonical byte stream used to derive [`StyleId`].
pub const STYLE_ID_FORMAT_VERSION: u16 = 2;

const STREAM_DOMAIN: &[u8; 16] = b"pliego-style-id\0";

const STREAM_THEME: u8 = 0x01;
const STREAM_ASSIGNMENTS: u8 = 0x02;
const STREAM_ASSIGNMENT: u8 = 0x03;

pub(super) const RECORD_UTILITY: u8 = 0x11;
pub(super) const RECORD_SLOTS: u8 = 0x12;
pub(super) const RECORD_VALUE: u8 = 0x13;
pub(super) const RECORD_FLAGS: u8 = 0x14;
pub(super) const RECORD_CONDITION: u8 = 0x15;
pub(super) const RECORD_CONTAINER_BREAKPOINT: u8 = 0x16;
pub(super) const RECORD_CASCADE_LAYER: u8 = 0x17;

const PSEUDO_STATES: [PseudoState; 5] = [
    PseudoState::Hover,
    PseudoState::Focus,
    PseudoState::FocusVisible,
    PseudoState::Active,
    PseudoState::Disabled,
];

/// Failure to encode or derive an identity from semantic IR.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    /// The semantic style violates an IR structural invariant.
    InvalidSemanticStyle(InvariantError),
    /// A count or UTF-8 payload cannot fit in the version-2 length field.
    LengthOverflow {
        /// Identity field whose encoded length overflowed.
        field: &'static str,
        /// Observed number of entries or bytes.
        length: usize,
    },
    /// An interned reference was outside its corresponding table.
    ReferenceOutOfBounds {
        /// Referenced semantic table.
        table: &'static str,
        /// Referenced table index.
        index: usize,
        /// Available table length.
        length: usize,
    },
    /// A bitset contains a semantic variant without a version-2 tag.
    UnencodedBits {
        /// Semantic bitset that was not encoded exhaustively.
        field: &'static str,
        /// Complete observed bit representation.
        bits: u128,
    },
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSemanticStyle(error) => {
                write!(formatter, "cannot encode style identity: {error}")
            }
            Self::LengthOverflow { field, length } => write!(
                formatter,
                "identity field {field} has length {length}, exceeding the version-2 limit"
            ),
            Self::ReferenceOutOfBounds {
                table,
                index,
                length,
            } => write!(
                formatter,
                "identity reference {table}[{index}] is out of bounds for length {length}"
            ),
            Self::UnencodedBits { field, bits } => write!(
                formatter,
                "identity field {field} contains untagged semantic bits 0x{bits:032x}"
            ),
        }
    }
}

impl std::error::Error for IdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidSemanticStyle(error) => Some(error),
            Self::LengthOverflow { .. }
            | Self::ReferenceOutOfBounds { .. }
            | Self::UnencodedBits { .. } => None,
        }
    }
}

impl From<InvariantError> for IdentityError {
    fn from(error: InvariantError) -> Self {
        Self::InvalidSemanticStyle(error)
    }
}

fn seed_theme() -> &'static ThemeRegistry {
    static SEED: OnceLock<ThemeRegistry> = OnceLock::new();
    SEED.get_or_init(ThemeRegistry::seed)
}

/// Encodes the canonical version-2 identity stream under the built-in theme.
///
/// The stream excludes provenance spans and the previously derived `style.id`.
/// Assignment records are sorted by encoded semantic content, so authoring order
/// cannot change the resulting identity.
///
/// # Errors
///
/// Returns [`IdentityError`] when the style violates an IR invariant or an
/// encoded count exceeds the version-2 field width.
pub fn try_encode_style_identity(style: &SemanticStyle) -> Result<Vec<u8>, IdentityError> {
    try_encode_style_identity_with_theme(seed_theme(), style)
}

/// Encodes the canonical version-2 identity stream under an explicit theme.
///
/// The theme contributes only its already-versioned 128-bit identity. Interned
/// arbitrary values, custom properties, selectors, and arbitrary property names
/// are resolved to their UTF-8 content rather than encoded as table positions.
///
/// # Errors
///
/// Returns [`IdentityError`] when the style violates an IR invariant or an
/// encoded count exceeds the version-2 field width.
pub fn try_encode_style_identity_with_theme(
    theme: &ThemeRegistry,
    style: &SemanticStyle,
) -> Result<Vec<u8>, IdentityError> {
    style.validate()?;

    let mut records = Vec::with_capacity(style.assignments.len());
    for assignment in &style.assignments {
        records.push(encode_assignment(style, assignment)?);
    }
    records.sort_unstable();
    records.dedup();

    let mut stream = Vec::new();
    stream.extend_from_slice(STREAM_DOMAIN);
    stream.extend_from_slice(&STYLE_ID_FORMAT_VERSION.to_be_bytes());
    stream.push(STREAM_THEME);
    stream.extend_from_slice(&THEME_ID_FORMAT_VERSION.to_be_bytes());
    stream.extend_from_slice(&theme.id().get().to_be_bytes());
    stream.push(STREAM_ASSIGNMENTS);
    write_u32_length(&mut stream, records.len(), "assignments")?;
    for record in records {
        stream.push(STREAM_ASSIGNMENT);
        write_u32_length(&mut stream, record.len(), "assignment record")?;
        stream.extend_from_slice(&record);
    }
    Ok(stream)
}

/// Tries to derive a deterministic style identity under the built-in theme.
///
/// SHA-256 is computed over the version-2 stream and its first 128 bits, in
/// network byte order, become the compact [`StyleId`]. The all-zero value is
/// remapped to one because zero is reserved for unresolved styles.
///
/// # Errors
///
/// Returns [`IdentityError`] when the semantic style cannot be encoded.
pub fn try_derive_style_id(style: &SemanticStyle) -> Result<StyleId, IdentityError> {
    try_derive_style_id_with_theme(seed_theme(), style)
}

/// Tries to derive a deterministic style identity under an explicit theme.
///
/// # Errors
///
/// Returns [`IdentityError`] when the semantic style cannot be encoded.
pub fn try_derive_style_id_with_theme(
    theme: &ThemeRegistry,
    style: &SemanticStyle,
) -> Result<StyleId, IdentityError> {
    let stream = try_encode_style_identity_with_theme(theme, style)?;
    Ok(style_id_from_stream(&stream))
}

/// Derives a deterministic style identity under the built-in theme.
///
/// # Panics
///
/// Panics when `style` violates a semantic-IR invariant. Compiler lowering
/// validates the IR before invoking this convenience wrapper; callers handling
/// untrusted or manually assembled IR should use [`try_derive_style_id`].
#[must_use]
pub fn derive_style_id(style: &SemanticStyle) -> StyleId {
    try_derive_style_id(style).expect("normalized semantic style must encode an identity")
}

/// Derives a deterministic style identity under an explicit theme.
///
/// # Panics
///
/// Panics when `style` violates a semantic-IR invariant. Compiler lowering
/// validates the IR before invoking this convenience wrapper; callers handling
/// untrusted or manually assembled IR should use [`try_derive_style_id_with_theme`].
#[must_use]
pub fn derive_style_id_with_theme(theme: &ThemeRegistry, style: &SemanticStyle) -> StyleId {
    try_derive_style_id_with_theme(theme, style)
        .expect("normalized semantic style must encode an identity")
}

fn style_id_from_stream(stream: &[u8]) -> StyleId {
    let digest = Sha256::digest(stream);
    let mut truncated = [0_u8; 16];
    truncated.copy_from_slice(&digest[..16]);
    let value = u128::from_be_bytes(truncated);
    StyleId::new(if value == 0 { 1 } else { value })
}

pub(super) fn encode_assignment(
    style: &SemanticStyle,
    assignment: &Assignment,
) -> Result<Vec<u8>, IdentityError> {
    let mut record = Vec::new();

    record.push(RECORD_UTILITY);
    encode_utility(&mut record, style, assignment.utility)?;

    record.push(RECORD_SLOTS);
    let slot_count = assignment.slots.iter().count();
    if assignment.slots.bits().count_ones()
        != u32::try_from(slot_count).expect("a u128 bit count always fits in u32")
    {
        return Err(IdentityError::UnencodedBits {
            field: "assignment slots",
            bits: assignment.slots.bits(),
        });
    }
    write_u8_length(&mut record, slot_count, "assignment slots")?;
    for slot in assignment.slots.iter() {
        record.push(slot_tag(slot));
    }

    record.push(RECORD_VALUE);
    encode_semantic_value(&mut record, style, assignment.value)?;

    record.push(RECORD_FLAGS);
    record.push(u8::from(assignment.negative));
    record.push(u8::from(assignment.important));

    record.push(RECORD_CONDITION);
    let condition = table_entry(
        &style.conditions,
        assignment.condition.index(),
        "conditions",
    )?;
    match condition.breakpoint {
        None => record.push(0),
        Some(id) => {
            record.push(1);
            record.extend_from_slice(&id.get().to_be_bytes());
        }
    }
    if let Some(id) = condition.container_breakpoint {
        record.push(RECORD_CONTAINER_BREAKPOINT);
        record.extend_from_slice(&id.get().to_be_bytes());
    }
    if condition.layer != CascadeLayer::Unlayered {
        record.push(RECORD_CASCADE_LAYER);
        record.push(cascade_layer_tag(condition.layer));
    }
    record.push(theme_mode_tag(condition.theme));
    record.push(motion_preference_tag(condition.motion));
    record.push(contrast_preference_tag(condition.contrast));

    let state_count = PSEUDO_STATES
        .iter()
        .filter(|state| condition.states.contains(**state))
        .count();
    if condition.states.bits().count_ones()
        != u32::try_from(state_count).expect("a u16 bit count always fits in u32")
    {
        return Err(IdentityError::UnencodedBits {
            field: "condition states",
            bits: u128::from(condition.states.bits()),
        });
    }
    write_u8_length(&mut record, state_count, "condition states")?;
    for state in PSEUDO_STATES {
        if condition.states.contains(state) {
            record.push(pseudo_state_tag(state));
        }
    }

    write_u32_length(
        &mut record,
        condition.selectors.len(),
        "condition selectors",
    )?;
    for selector in &condition.selectors {
        let value = table_entry(&style.selectors, selector.index(), "selectors")?;
        write_text(&mut record, value, "selector")?;
    }

    Ok(record)
}

fn encode_utility(
    output: &mut Vec<u8>,
    style: &SemanticStyle,
    utility: Utility,
) -> Result<(), IdentityError> {
    output.push(utility_tag(utility));
    if let Utility::ArbitraryProperty(id) = utility {
        let property = table_entry(
            &style.arbitrary_properties,
            id.index(),
            "arbitrary properties",
        )?;
        write_text(output, &property.name, "arbitrary property")?;
    }
    Ok(())
}

fn encode_semantic_value(
    output: &mut Vec<u8>,
    style: &SemanticStyle,
    value: SemanticValue,
) -> Result<(), IdentityError> {
    match value {
        SemanticValue::Keyword(keyword) => {
            output.push(1);
            output.push(keyword_tag(keyword));
        }
        SemanticValue::Token(reference) => {
            output.push(2);
            output.push(token_kind_tag(reference.kind));
            output.extend_from_slice(&reference.id.get().to_be_bytes());
        }
        SemanticValue::Integer(value) => {
            output.push(3);
            output.extend_from_slice(&value.to_be_bytes());
        }
        SemanticValue::Number(number) => {
            output.push(4);
            encode_css_number(output, number);
        }
        SemanticValue::Length(length) => {
            output.push(5);
            encode_length(output, length);
        }
        SemanticValue::Percentage(percentage) => {
            output.push(6);
            output.extend_from_slice(&percentage.basis_points().to_be_bytes());
        }
        SemanticValue::Color(color) => {
            output.push(7);
            encode_color(output, color);
        }
        SemanticValue::Fraction {
            numerator,
            denominator,
        } => {
            output.push(8);
            output.extend_from_slice(&numerator.to_be_bytes());
            output.extend_from_slice(&denominator.to_be_bytes());
        }
        SemanticValue::Arbitrary(id) => {
            output.push(9);
            let value = table_entry(&style.arbitrary_values, id.index(), "arbitrary values")?;
            write_text(output, value, "arbitrary value")?;
        }
        SemanticValue::CustomProperty(reference) => {
            output.push(10);
            let name = table_entry(
                &style.custom_properties,
                reference.id.index(),
                "custom properties",
            )?;
            write_text(output, name, "custom property")?;
            output.push(value_kind_tag(reference.hint));
        }
    }
    Ok(())
}

fn encode_css_number(output: &mut Vec<u8>, number: CssNumber) {
    output.extend_from_slice(&number.coefficient().to_be_bytes());
    output.push(number.scale());
}

fn encode_length(output: &mut Vec<u8>, length: Length) {
    encode_css_number(output, length.number);
    output.push(length_unit_tag(length.unit));
}

fn encode_color(output: &mut Vec<u8>, color: ColorValue) {
    match color {
        ColorValue::Transparent => output.push(1),
        ColorValue::CurrentColor => output.push(2),
        ColorValue::Token { id, alpha } => {
            output.push(3);
            output.extend_from_slice(&id.get().to_be_bytes());
            match alpha {
                None => output.push(0),
                Some(alpha) => {
                    output.push(1);
                    output.extend_from_slice(&alpha.basis_points().to_be_bytes());
                }
            }
        }
        ColorValue::Srgba {
            red,
            green,
            blue,
            alpha,
        } => {
            output.push(4);
            output.extend_from_slice(&[red, green, blue, alpha]);
        }
    }
}

fn write_text(output: &mut Vec<u8>, value: &str, field: &'static str) -> Result<(), IdentityError> {
    write_u32_length(output, value.len(), field)?;
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_u32_length(
    output: &mut Vec<u8>,
    length: usize,
    field: &'static str,
) -> Result<(), IdentityError> {
    let encoded =
        u32::try_from(length).map_err(|_| IdentityError::LengthOverflow { field, length })?;
    output.extend_from_slice(&encoded.to_be_bytes());
    Ok(())
}

fn write_u8_length(
    output: &mut Vec<u8>,
    length: usize,
    field: &'static str,
) -> Result<(), IdentityError> {
    let encoded =
        u8::try_from(length).map_err(|_| IdentityError::LengthOverflow { field, length })?;
    output.push(encoded);
    Ok(())
}

fn table_entry<'a, T>(
    table: &'a [T],
    index: usize,
    name: &'static str,
) -> Result<&'a T, IdentityError> {
    table.get(index).ok_or(IdentityError::ReferenceOutOfBounds {
        table: name,
        index,
        length: table.len(),
    })
}

#[allow(clippy::too_many_lines)]
pub(super) const fn utility_tag(utility: Utility) -> u8 {
    match utility {
        Utility::Display => 1,
        Utility::Visibility => 2,
        Utility::Position => 3,
        Utility::Inset => 4,
        Utility::ZIndex => 5,
        Utility::Overflow => 6,
        Utility::Width => 7,
        Utility::MinWidth => 8,
        Utility::MaxWidth => 9,
        Utility::Height => 10,
        Utility::MinHeight => 11,
        Utility::MaxHeight => 12,
        Utility::AspectRatio => 13,
        Utility::Margin => 14,
        Utility::Padding => 15,
        Utility::Gap => 16,
        Utility::FlexDirection => 17,
        Utility::FlexWrap => 18,
        Utility::FlexGrow => 19,
        Utility::FlexShrink => 20,
        Utility::FlexBasis => 21,
        Utility::AlignItems => 22,
        Utility::AlignContent => 23,
        Utility::AlignSelf => 24,
        Utility::JustifyContent => 25,
        Utility::JustifyItems => 26,
        Utility::JustifySelf => 27,
        Utility::GridTemplateColumns => 28,
        Utility::GridTemplateRows => 29,
        Utility::GridColumn => 30,
        Utility::GridRow => 31,
        Utility::BackgroundColor => 32,
        Utility::BackgroundImage => 33,
        Utility::TextColor => 34,
        Utility::FontFamily => 35,
        Utility::FontSize => 36,
        Utility::FontWeight => 37,
        Utility::LineHeight => 38,
        Utility::LetterSpacing => 39,
        Utility::FontSmoothing => 40,
        Utility::TextAlign => 41,
        Utility::TextDecorationLine => 42,
        Utility::BorderWidth => 43,
        Utility::BorderColor => 44,
        Utility::BorderStyle => 45,
        Utility::BorderRadius => 46,
        Utility::OutlineWidth => 47,
        Utility::OutlineColor => 48,
        Utility::OutlineOffset => 49,
        Utility::OutlineStyle => 50,
        Utility::Opacity => 51,
        Utility::BoxShadow => 52,
        Utility::RingWidth => 53,
        Utility::RingColor => 54,
        Utility::Transform => 55,
        Utility::TransitionProperty => 56,
        Utility::Resize => 57,
        Utility::Cursor => 58,
        Utility::PointerEvents => 59,
        Utility::ArbitraryProperty(_) => 60,
        Utility::ContainerType => 61,
        Utility::WritingMode => 62,
    }
}

#[allow(clippy::too_many_lines)]
pub(super) const fn slot_tag(slot: Slot) -> u8 {
    match slot {
        Slot::DisplayMode => 1,
        Slot::Visibility => 2,
        Slot::PositionMode => 3,
        Slot::InsetTop => 4,
        Slot::InsetRight => 5,
        Slot::InsetBottom => 6,
        Slot::InsetLeft => 7,
        Slot::ZIndex => 8,
        Slot::OverflowX => 9,
        Slot::OverflowY => 10,
        Slot::Width => 11,
        Slot::MinWidth => 12,
        Slot::MaxWidth => 13,
        Slot::Height => 14,
        Slot::MinHeight => 15,
        Slot::MaxHeight => 16,
        Slot::AspectRatio => 17,
        Slot::MarginTop => 18,
        Slot::MarginRight => 19,
        Slot::MarginBottom => 20,
        Slot::MarginLeft => 21,
        Slot::PaddingTop => 22,
        Slot::PaddingRight => 23,
        Slot::PaddingBottom => 24,
        Slot::PaddingLeft => 25,
        Slot::GapRow => 26,
        Slot::GapColumn => 27,
        Slot::FlexDirection => 28,
        Slot::FlexWrap => 29,
        Slot::FlexGrow => 30,
        Slot::FlexShrink => 31,
        Slot::FlexBasis => 32,
        Slot::AlignItems => 33,
        Slot::AlignContent => 34,
        Slot::AlignSelf => 35,
        Slot::JustifyContent => 36,
        Slot::JustifyItems => 37,
        Slot::JustifySelf => 38,
        Slot::GridTemplateColumns => 39,
        Slot::GridTemplateRows => 40,
        Slot::GridColumn => 41,
        Slot::GridRow => 42,
        Slot::BackgroundColor => 43,
        Slot::BackgroundImage => 44,
        Slot::TextColor => 45,
        Slot::FontFamily => 46,
        Slot::FontSize => 47,
        Slot::FontWeight => 48,
        Slot::LineHeight => 49,
        Slot::LetterSpacing => 50,
        Slot::FontSmoothing => 51,
        Slot::TextAlign => 52,
        Slot::TextDecorationLine => 53,
        Slot::BorderTopWidth => 54,
        Slot::BorderRightWidth => 55,
        Slot::BorderBottomWidth => 56,
        Slot::BorderLeftWidth => 57,
        Slot::BorderTopColor => 58,
        Slot::BorderRightColor => 59,
        Slot::BorderBottomColor => 60,
        Slot::BorderLeftColor => 61,
        Slot::BorderTopStyle => 62,
        Slot::BorderRightStyle => 63,
        Slot::BorderBottomStyle => 64,
        Slot::BorderLeftStyle => 65,
        Slot::BorderTopLeftRadius => 66,
        Slot::BorderTopRightRadius => 67,
        Slot::BorderBottomRightRadius => 68,
        Slot::BorderBottomLeftRadius => 69,
        Slot::OutlineWidth => 70,
        Slot::OutlineColor => 71,
        Slot::OutlineOffset => 72,
        Slot::OutlineStyle => 73,
        Slot::Opacity => 74,
        Slot::BoxShadow => 75,
        Slot::RingWidth => 76,
        Slot::RingColor => 77,
        Slot::Transform => 78,
        Slot::TransitionProperty => 79,
        Slot::Resize => 80,
        Slot::Cursor => 81,
        Slot::PointerEvents => 82,
        Slot::ArbitraryProperty => 83,
        Slot::ContainerType => 84,
        Slot::WritingMode => 85,
    }
}

#[allow(clippy::too_many_lines)]
pub(super) const fn keyword_tag(keyword: Keyword) -> u8 {
    match keyword {
        Keyword::None => 1,
        Keyword::Auto => 2,
        Keyword::Normal => 3,
        Keyword::Block => 4,
        Keyword::Inline => 5,
        Keyword::InlineBlock => 6,
        Keyword::Flex => 7,
        Keyword::InlineFlex => 8,
        Keyword::Grid => 9,
        Keyword::InlineGrid => 10,
        Keyword::Visible => 11,
        Keyword::Hidden => 12,
        Keyword::Collapse => 13,
        Keyword::Static => 14,
        Keyword::Relative => 15,
        Keyword::Absolute => 16,
        Keyword::Fixed => 17,
        Keyword::Sticky => 18,
        Keyword::Clip => 19,
        Keyword::Scroll => 20,
        Keyword::Row => 21,
        Keyword::RowReverse => 22,
        Keyword::Column => 23,
        Keyword::ColumnReverse => 24,
        Keyword::Wrap => 25,
        Keyword::WrapReverse => 26,
        Keyword::NoWrap => 27,
        Keyword::Start => 28,
        Keyword::End => 29,
        Keyword::Center => 30,
        Keyword::SpaceBetween => 31,
        Keyword::SpaceAround => 32,
        Keyword::SpaceEvenly => 33,
        Keyword::Stretch => 34,
        Keyword::Baseline => 35,
        Keyword::MinContent => 36,
        Keyword::MaxContent => 37,
        Keyword::FitContent => 38,
        Keyword::Content => 39,
        Keyword::Left => 40,
        Keyword::Right => 41,
        Keyword::Justify => 42,
        Keyword::Bold => 43,
        Keyword::Underline => 44,
        Keyword::Overline => 45,
        Keyword::LineThrough => 46,
        Keyword::Solid => 47,
        Keyword::Dashed => 48,
        Keyword::Dotted => 49,
        Keyword::Double => 50,
        Keyword::Pointer => 51,
        Keyword::Default => 52,
        Keyword::Wait => 53,
        Keyword::NotAllowed => 54,
        Keyword::Text => 55,
        Keyword::Move => 56,
        Keyword::Antialiased => 57,
        Keyword::Colors => 58,
        Keyword::Vertical => 59,
        Keyword::InlineSize => 60,
        Keyword::HorizontalTb => 61,
        Keyword::VerticalLr => 62,
        Keyword::VerticalRl => 63,
    }
}

pub(super) const fn cascade_layer_tag(layer: CascadeLayer) -> u8 {
    match layer {
        CascadeLayer::Unlayered => 0,
        CascadeLayer::Base => 1,
        CascadeLayer::Components => 2,
        CascadeLayer::Utilities => 3,
        CascadeLayer::Overrides => 4,
    }
}

pub(super) const fn token_kind_tag(kind: TokenKind) -> u8 {
    match kind {
        TokenKind::Spacing => 1,
        TokenKind::Color => 2,
        TokenKind::FontFamily => 3,
        TokenKind::FontSize => 4,
        TokenKind::FontWeight => 5,
        TokenKind::LineHeight => 6,
        TokenKind::LetterSpacing => 7,
        TokenKind::Radius => 8,
        TokenKind::Shadow => 9,
        TokenKind::ZIndex => 10,
    }
}

pub(super) const fn length_unit_tag(unit: LengthUnit) -> u8 {
    match unit {
        LengthUnit::Px => 1,
        LengthUnit::Rem => 2,
        LengthUnit::Em => 3,
        LengthUnit::Percent => 4,
        LengthUnit::Ch => 5,
        LengthUnit::Vw => 6,
        LengthUnit::Vh => 7,
        LengthUnit::Dvw => 8,
        LengthUnit::Dvh => 9,
    }
}

pub(super) const fn value_kind_tag(kind: ValueKind) -> u8 {
    match kind {
        ValueKind::Any => 1,
        ValueKind::Keyword => 2,
        ValueKind::Token => 3,
        ValueKind::Integer => 4,
        ValueKind::Number => 5,
        ValueKind::Length => 6,
        ValueKind::Percentage => 7,
        ValueKind::Color => 8,
        ValueKind::Fraction => 9,
        ValueKind::Shadow => 10,
        ValueKind::Image => 11,
        ValueKind::GridTemplate => 12,
        ValueKind::Transform => 13,
    }
}

pub(super) const fn theme_mode_tag(mode: ThemeMode) -> u8 {
    match mode {
        ThemeMode::Any => 1,
        ThemeMode::Light => 2,
        ThemeMode::Dark => 3,
    }
}

pub(super) const fn motion_preference_tag(preference: MotionPreference) -> u8 {
    match preference {
        MotionPreference::Any => 1,
        MotionPreference::Safe => 2,
        MotionPreference::Reduce => 3,
    }
}

pub(super) const fn contrast_preference_tag(preference: ContrastPreference) -> u8 {
    match preference {
        ContrastPreference::Any => 1,
        ContrastPreference::More => 2,
        ContrastPreference::Less => 3,
    }
}

pub(super) const fn pseudo_state_tag(state: PseudoState) -> u8 {
    match state {
        PseudoState::Hover => 1,
        PseudoState::Focus => 2,
        PseudoState::FocusVisible => 3,
        PseudoState::Active => 4,
        PseudoState::Disabled => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pliego_css_ir::{
        ArbitraryPropertyId, ArbitraryValueId, BreakpointId, Condition, ConditionId,
        CustomPropertyId, SelectorId, SlotSet, Span, StateSet, TokenId, TokenRef,
    };
    use pliego_css_parser::parse_style_list;

    fn hex(bytes: &[u8]) -> String {
        use core::fmt::Write as _;

        let mut encoded = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(&mut encoded, "{byte:02x}").expect("must succeed");
        }
        encoded
    }

    fn encoded_value(style: &SemanticStyle, value: SemanticValue) -> String {
        let mut bytes = Vec::new();
        encode_semantic_value(&mut bytes, style, value).expect("must succeed");
        hex(&bytes)
    }

    fn assignment(utility: Utility, slot: Slot, value: SemanticValue, source: Span) -> Assignment {
        Assignment {
            utility,
            slots: SlotSet::from_slot(slot),
            value,
            negative: false,
            condition: ConditionId::new(0),
            important: false,
            source,
        }
    }

    #[test]
    fn empty_stream_layout_is_frozen() {
        let style = SemanticStyle::default();
        let theme = ThemeRegistry::seed();
        let stream = try_encode_style_identity_with_theme(&theme, &style).expect("must succeed");

        let mut expected = Vec::new();
        expected.extend_from_slice(STREAM_DOMAIN);
        expected.extend_from_slice(&2_u16.to_be_bytes());
        expected.push(STREAM_THEME);
        expected.extend_from_slice(&THEME_ID_FORMAT_VERSION.to_be_bytes());
        expected.extend_from_slice(&theme.id().get().to_be_bytes());
        expected.push(STREAM_ASSIGNMENTS);
        expected.extend_from_slice(&0_u32.to_be_bytes());
        assert_eq!(stream, expected);
    }

    #[test]
    fn simple_assignment_record_uses_explicit_tags() {
        let style = SemanticStyle::default();
        let record = encode_assignment(
            &style,
            &assignment(
                Utility::Display,
                Slot::DisplayMode,
                SemanticValue::Keyword(Keyword::Flex),
                Span::new(0, 4),
            ),
        )
        .expect("must succeed");

        assert_eq!(
            record,
            [
                RECORD_UTILITY,
                1,
                RECORD_SLOTS,
                1,
                1,
                RECORD_VALUE,
                1,
                7,
                RECORD_FLAGS,
                0,
                0,
                RECORD_CONDITION,
                0,
                1,
                1,
                1,
                0,
                0,
                0,
                0,
                0,
            ]
        );
    }

    #[test]
    fn assignment_order_duplicates_and_provenance_do_not_change_identity() {
        let display = assignment(
            Utility::Display,
            Slot::DisplayMode,
            SemanticValue::Keyword(Keyword::Flex),
            Span::new(0, 4),
        );
        let visibility = assignment(
            Utility::Visibility,
            Slot::Visibility,
            SemanticValue::Keyword(Keyword::Hidden),
            Span::new(5, 11),
        );
        let left = SemanticStyle {
            assignments: vec![display, visibility],
            ..SemanticStyle::default()
        };

        let right = SemanticStyle {
            assignments: vec![
                Assignment {
                    source: Span::new(100, 106),
                    ..visibility
                },
                Assignment {
                    source: Span::new(200, 204),
                    ..display
                },
            ],
            ..SemanticStyle::default()
        };

        assert_eq!(
            try_derive_style_id(&left).expect("must succeed"),
            try_derive_style_id(&right).expect("must succeed")
        );

        let duplicate = SemanticStyle {
            assignments: vec![
                display,
                Assignment {
                    source: Span::new(300, 304),
                    ..display
                },
                visibility,
            ],
            ..SemanticStyle::default()
        };
        assert_eq!(
            try_derive_style_id(&left).expect("must succeed"),
            try_derive_style_id(&duplicate).expect("must succeed")
        );
    }

    #[test]
    fn slot_tags_follow_the_frozen_slot_catalog() {
        for (index, slot) in Slot::ALL.into_iter().enumerate() {
            let expected = u8::try_from(index + 1).expect("must succeed");
            assert_eq!(slot_tag(slot), expected);
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn every_v2_enum_tag_is_explicit_unique_and_frozen() {
        let utilities = [
            Utility::Display,
            Utility::Visibility,
            Utility::Position,
            Utility::Inset,
            Utility::ZIndex,
            Utility::Overflow,
            Utility::Width,
            Utility::MinWidth,
            Utility::MaxWidth,
            Utility::Height,
            Utility::MinHeight,
            Utility::MaxHeight,
            Utility::AspectRatio,
            Utility::Margin,
            Utility::Padding,
            Utility::Gap,
            Utility::FlexDirection,
            Utility::FlexWrap,
            Utility::FlexGrow,
            Utility::FlexShrink,
            Utility::FlexBasis,
            Utility::AlignItems,
            Utility::AlignContent,
            Utility::AlignSelf,
            Utility::JustifyContent,
            Utility::JustifyItems,
            Utility::JustifySelf,
            Utility::GridTemplateColumns,
            Utility::GridTemplateRows,
            Utility::GridColumn,
            Utility::GridRow,
            Utility::BackgroundColor,
            Utility::BackgroundImage,
            Utility::TextColor,
            Utility::FontFamily,
            Utility::FontSize,
            Utility::FontWeight,
            Utility::LineHeight,
            Utility::LetterSpacing,
            Utility::FontSmoothing,
            Utility::TextAlign,
            Utility::TextDecorationLine,
            Utility::BorderWidth,
            Utility::BorderColor,
            Utility::BorderStyle,
            Utility::BorderRadius,
            Utility::OutlineWidth,
            Utility::OutlineColor,
            Utility::OutlineOffset,
            Utility::OutlineStyle,
            Utility::Opacity,
            Utility::BoxShadow,
            Utility::RingWidth,
            Utility::RingColor,
            Utility::Transform,
            Utility::TransitionProperty,
            Utility::Resize,
            Utility::Cursor,
            Utility::PointerEvents,
            Utility::ArbitraryProperty(ArbitraryPropertyId::new(0)),
            Utility::ContainerType,
            Utility::WritingMode,
        ];
        let keywords = [
            Keyword::None,
            Keyword::Auto,
            Keyword::Normal,
            Keyword::Block,
            Keyword::Inline,
            Keyword::InlineBlock,
            Keyword::Flex,
            Keyword::InlineFlex,
            Keyword::Grid,
            Keyword::InlineGrid,
            Keyword::Visible,
            Keyword::Hidden,
            Keyword::Collapse,
            Keyword::Static,
            Keyword::Relative,
            Keyword::Absolute,
            Keyword::Fixed,
            Keyword::Sticky,
            Keyword::Clip,
            Keyword::Scroll,
            Keyword::Row,
            Keyword::RowReverse,
            Keyword::Column,
            Keyword::ColumnReverse,
            Keyword::Wrap,
            Keyword::WrapReverse,
            Keyword::NoWrap,
            Keyword::Start,
            Keyword::End,
            Keyword::Center,
            Keyword::SpaceBetween,
            Keyword::SpaceAround,
            Keyword::SpaceEvenly,
            Keyword::Stretch,
            Keyword::Baseline,
            Keyword::MinContent,
            Keyword::MaxContent,
            Keyword::FitContent,
            Keyword::Content,
            Keyword::Left,
            Keyword::Right,
            Keyword::Justify,
            Keyword::Bold,
            Keyword::Underline,
            Keyword::Overline,
            Keyword::LineThrough,
            Keyword::Solid,
            Keyword::Dashed,
            Keyword::Dotted,
            Keyword::Double,
            Keyword::Pointer,
            Keyword::Default,
            Keyword::Wait,
            Keyword::NotAllowed,
            Keyword::Text,
            Keyword::Move,
            Keyword::Antialiased,
            Keyword::Colors,
            Keyword::Vertical,
            Keyword::InlineSize,
            Keyword::HorizontalTb,
            Keyword::VerticalLr,
            Keyword::VerticalRl,
        ];
        let token_kinds = [
            TokenKind::Spacing,
            TokenKind::Color,
            TokenKind::FontFamily,
            TokenKind::FontSize,
            TokenKind::FontWeight,
            TokenKind::LineHeight,
            TokenKind::LetterSpacing,
            TokenKind::Radius,
            TokenKind::Shadow,
            TokenKind::ZIndex,
        ];
        let length_units = [
            LengthUnit::Px,
            LengthUnit::Rem,
            LengthUnit::Em,
            LengthUnit::Percent,
            LengthUnit::Ch,
            LengthUnit::Vw,
            LengthUnit::Vh,
            LengthUnit::Dvw,
            LengthUnit::Dvh,
        ];
        let value_kinds = [
            ValueKind::Any,
            ValueKind::Keyword,
            ValueKind::Token,
            ValueKind::Integer,
            ValueKind::Number,
            ValueKind::Length,
            ValueKind::Percentage,
            ValueKind::Color,
            ValueKind::Fraction,
            ValueKind::Shadow,
            ValueKind::Image,
            ValueKind::GridTemplate,
            ValueKind::Transform,
        ];
        let themes = [ThemeMode::Any, ThemeMode::Light, ThemeMode::Dark];
        let motions = [
            MotionPreference::Any,
            MotionPreference::Safe,
            MotionPreference::Reduce,
        ];
        let contrasts = [
            ContrastPreference::Any,
            ContrastPreference::More,
            ContrastPreference::Less,
        ];

        for (index, value) in utilities.into_iter().enumerate() {
            assert_eq!(
                utility_tag(value),
                u8::try_from(index + 1).expect("must succeed")
            );
        }
        for (index, value) in keywords.into_iter().enumerate() {
            assert_eq!(
                keyword_tag(value),
                u8::try_from(index + 1).expect("must succeed")
            );
        }
        for (index, value) in token_kinds.into_iter().enumerate() {
            assert_eq!(
                token_kind_tag(value),
                u8::try_from(index + 1).expect("must succeed")
            );
        }
        for (index, value) in length_units.into_iter().enumerate() {
            assert_eq!(
                length_unit_tag(value),
                u8::try_from(index + 1).expect("must succeed")
            );
        }
        for (index, value) in value_kinds.into_iter().enumerate() {
            assert_eq!(
                value_kind_tag(value),
                u8::try_from(index + 1).expect("must succeed")
            );
        }
        for (index, value) in themes.into_iter().enumerate() {
            assert_eq!(
                theme_mode_tag(value),
                u8::try_from(index + 1).expect("must succeed")
            );
        }
        for (index, value) in motions.into_iter().enumerate() {
            assert_eq!(
                motion_preference_tag(value),
                u8::try_from(index + 1).expect("must succeed")
            );
        }
        for (index, value) in contrasts.into_iter().enumerate() {
            assert_eq!(
                contrast_preference_tag(value),
                u8::try_from(index + 1).expect("must succeed")
            );
        }
        for (index, value) in PSEUDO_STATES.into_iter().enumerate() {
            assert_eq!(
                pseudo_state_tag(value),
                u8::try_from(index + 1).expect("must succeed")
            );
        }
    }

    #[test]
    fn every_v2_semantic_value_payload_is_framed_and_frozen() {
        let mut style = SemanticStyle::default();
        style.arbitrary_values.push("a|:\0café😀".into());
        style.custom_properties.push("--café😀|:".into());
        let half = pliego_css_ir::Percentage::from_basis_points(5_000).expect("must succeed");

        let vectors = [
            (SemanticValue::Keyword(Keyword::Flex), "0107"),
            (
                SemanticValue::Token(TokenRef {
                    kind: TokenKind::Color,
                    id: TokenId::new(0x0102_0304),
                }),
                "020201020304",
            ),
            (SemanticValue::Integer(-2), "03fffffffe"),
            (
                SemanticValue::Number(CssNumber::new(-125, 2).expect("must succeed")),
                "04ffffffffffffff8302",
            ),
            (
                SemanticValue::Length(Length {
                    number: CssNumber::new(15, 1).expect("must succeed"),
                    unit: LengthUnit::Rem,
                }),
                "05000000000000000f0102",
            ),
            (SemanticValue::Percentage(half), "061388"),
            (SemanticValue::Color(ColorValue::Transparent), "0701"),
            (SemanticValue::Color(ColorValue::CurrentColor), "0702"),
            (
                SemanticValue::Color(ColorValue::Token {
                    id: TokenId::new(0x0102_0304),
                    alpha: None,
                }),
                "07030102030400",
            ),
            (
                SemanticValue::Color(ColorValue::Token {
                    id: TokenId::new(0x0102_0304),
                    alpha: Some(half),
                }),
                "070301020304011388",
            ),
            (
                SemanticValue::Color(ColorValue::Srgba {
                    red: 1,
                    green: 2,
                    blue: 3,
                    alpha: 4,
                }),
                "070401020304",
            ),
            (
                SemanticValue::Fraction {
                    numerator: 2,
                    denominator: 3,
                },
                "0800020003",
            ),
            (
                SemanticValue::Arbitrary(ArbitraryValueId::new(0)),
                "090000000d617c3a00636166c3a9f09f9880",
            ),
            (
                SemanticValue::CustomProperty(pliego_css_ir::CustomPropertyRef {
                    id: CustomPropertyId::new(0),
                    hint: ValueKind::Color,
                }),
                "0a0000000d2d2d636166c3a9f09f98807c3a08",
            ),
        ];

        for (value, expected) in vectors {
            assert_eq!(encoded_value(&style, value), expected);
        }
    }

    #[test]
    fn rich_v2_stream_digest_style_id_and_class_are_frozen() {
        let theme = ThemeRegistry::seed();
        let syntax = parse_style_list(
            "dark:motion-reduce:contrast-more:hover:[&>p]:bg-accent/50! -mt-[1.25rem] [content:'café|:😀']",
        )
        .expect("must succeed");
        let style = crate::lower_style_with_theme(&theme, &syntax).expect("must succeed");
        let stream = try_encode_style_identity_with_theme(&theme, &style).expect("must succeed");

        assert_eq!(
            hex(&stream),
            "706c6965676f2d7374796c652d6964000002010002b3d5ad77175995c2b8f51ef7c0d419910200000003030000001f110e120112130900000007312e323572656d140100150001010100000000000300000024112012012b13070313e3e9b7011388140001150003030201010000000100000003263e700300000030113c00000007636f6e74656e7412015313090000000d27636166c3a97c3af09f98802714000015000101010000000000"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&stream)),
            "f22fe79baa3e45505636d5a9e245d3d0934f8a45afbe662db9f841683277cc04"
        );
        assert_eq!(
            format!("{:032x}", style.id.get()),
            "f22fe79baa3e45505636d5a9e245d3d0"
        );
        assert_eq!(style.id.to_class_name(), "pc_ec64pw6t451sjonk2yafopa6o");
    }

    #[test]
    fn v2_condition_and_error_paths_are_explicit() {
        let mut style = SemanticStyle::default();
        style.selectors.push("&>p".into());
        style.conditions[0] = Condition {
            breakpoint: Some(BreakpointId::new(0x0102)),
            container_breakpoint: None,
            layer: CascadeLayer::Unlayered,
            theme: ThemeMode::Dark,
            motion: MotionPreference::Reduce,
            contrast: ContrastPreference::More,
            states: StateSet::EMPTY
                .with(PseudoState::Hover)
                .with(PseudoState::Disabled),
            selectors: vec![SelectorId::new(0)],
        };
        let record = encode_assignment(
            &style,
            &Assignment {
                negative: true,
                important: true,
                ..assignment(
                    Utility::Display,
                    Slot::DisplayMode,
                    SemanticValue::Keyword(Keyword::Flex),
                    Span::new(9, 13),
                )
            },
        )
        .expect("must succeed");
        assert_eq!(
            hex(&record),
            "1101120101130107140101150101020303020201050000000100000003263e70"
        );

        let missing = encode_semantic_value(
            &mut Vec::new(),
            &SemanticStyle::default(),
            SemanticValue::Arbitrary(ArbitraryValueId::new(7)),
        )
        .expect_err("must reject");
        assert!(matches!(
            missing,
            IdentityError::ReferenceOutOfBounds {
                table: "arbitrary values",
                index: 7,
                length: 0
            }
        ));
        assert!(matches!(
            write_u8_length(&mut Vec::new(), 256, "test"),
            Err(IdentityError::LengthOverflow {
                field: "test",
                length: 256
            })
        ));
    }
}
