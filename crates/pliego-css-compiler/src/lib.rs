//! Semantic lowering and conflict analysis for `PliegoCSS`.
//!
//! This is an exact-version implementation crate. IR mutation, lowering, identity encoding,
//! composition, emission, and the Rust catalog are not the minimal application API.

#![forbid(unsafe_code)]

mod emitter;
mod engine;
mod identity;
mod ir_binary;

pub use emitter::{
    CssFragmentCache, DeclarationLineage, EmitError, RuleLineage, StyleLineage, class_name,
    emit_css, emit_css_with_theme, emit_css_with_theme_traced, emit_seed_theme, emit_theme,
    emit_theme_references, emit_used_theme, referenced_tokens, theme_custom_property_name,
};
pub use engine::{
    AnalysisCacheStats, AnalysisHost, CompileError, CompileInput, CompileRequest, CompileResult,
    CompiledStyle, MAX_PCX_COMBINATIONS, PcxAnalysis, PcxCombination, PcxError, PcxRequest,
    PhysicalRulePlan, PhysicalRulePlanner,
};
pub use identity::{
    IdentityError, STYLE_ID_FORMAT_VERSION, derive_style_id, derive_style_id_with_theme,
    try_derive_style_id, try_derive_style_id_with_theme, try_encode_style_identity,
    try_encode_style_identity_with_theme,
};
pub use ir_binary::{
    IR_BINARY_FORMAT_VERSION, IR_BINARY_MAGIC, IrBinaryError, MAX_IR_BINARY_BYTES, MAX_IR_RECORDS,
    MAX_IR_SELECTORS_PER_CONDITION, MAX_IR_TEXT_BYTES, MAX_IR_TOTAL_SELECTORS, decode_style_ir,
    decode_style_ir_with_theme, encode_style_ir, encode_style_ir_with_theme,
};

use std::sync::OnceLock;

#[cfg(test)]
use pliego_css_ir::is_classified_selector_transform;
use pliego_css_ir::{
    ArbitraryProperty, ArbitraryPropertyId, ArbitraryValueId, Assignment, CASCADE_LAYER_VARIANTS,
    CLASSIFIED_SELECTOR_VARIANTS, CandidateKind, CascadeLayer, ColorValue, Condition, ConditionId,
    ContrastPreference, CssNumber, CustomPropertyId, CustomPropertyRef, Diagnostic, DiagnosticCode,
    Keyword, Length, LengthUnit, MotionPreference, Percentage, PseudoState, SelectorId,
    SemanticStyle, SemanticValue, Slot, SlotSet, SourceSpan, Span, StyleItem, StyleList, ThemeMode,
    TokenKind, TokenRef, Utility, ValueKind, VariantKind, cascade_layer_for_variant,
    classified_attribute_name, classified_selector_for_variant, is_typed_attribute_variant,
};
use pliego_css_parser::{
    NamedCandidateSyntax, OperandKind, OperandSyntax, parse_named_candidate,
    parse_named_candidate_at,
};
use pliego_css_theme::ThemeRegistry;

/// The authoring shape exposed by one catalog entry.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UtilityForm {
    /// A complete utility spelling with no operand.
    Fixed,
    /// A named family followed by an operand.
    Parameterized,
    /// A parser-level arbitrary CSS declaration.
    ArbitraryProperty,
}

/// The semantic operand domain accepted by one utility entry.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UtilityDomain {
    /// No operand is accepted.
    None,
    /// An arbitrary CSS property and value pair.
    ArbitraryProperty,
    /// A color token, arbitrary color, or color custom property.
    Color,
    /// A font-family or font-weight token.
    FontFamilyOrWeight,
    /// A font-size or color token, including the supported modifiers and custom properties.
    FontSizeOrColor,
    /// A grid column span in the supported integer range.
    GridColumnSpan,
    /// A grid template expressed as a count, arbitrary value, or custom property.
    GridTemplate,
    /// A line-height token, arbitrary value, or custom property.
    LineHeight,
    /// A spacing token or `auto`, including arbitrary values and custom properties.
    Margin,
    /// An integer percentage from zero through one hundred.
    OpacityPercentage,
    /// A radius token, arbitrary value, or custom property.
    Radius,
    /// A non-negative integer pixel width or color token.
    BorderWidthOrColor,
    /// A non-negative integer pixel width or color token for a focus ring.
    RingWidthOrColor,
    /// A shadow token, arbitrary value, or custom property.
    Shadow,
    /// A size token or `auto`, including arbitrary values and custom properties.
    Size,
    /// A spacing token, arbitrary value, or custom property.
    Spacing,
}

/// Read-only metadata for one accepted utility spelling or family.
///
/// Descriptors are compiler-owned and cannot be constructed by downstream crates. New metadata
/// fields or catalog forms may be added without making the current surface exhaustive.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UtilityDescriptor {
    pattern: &'static str,
    match_name: &'static str,
    example: &'static str,
    summary: &'static str,
    domain: UtilityDomain,
    capabilities: u8,
    resolver: CatalogResolver,
}

impl UtilityDescriptor {
    /// Returns the documented spelling or parameterized pattern.
    #[must_use]
    pub const fn pattern(&self) -> &'static str {
        self.pattern
    }

    /// Returns the exact fixed spelling or parameterized prefix used by longest-match resolution.
    /// Arbitrary-property descriptors return an empty string because they are recognized by the
    /// parser before named catalog lookup.
    #[must_use]
    pub const fn match_name(&self) -> &'static str {
        self.match_name
    }

    /// Returns one complete source example accepted by the compiler.
    #[must_use]
    pub const fn example(&self) -> &'static str {
        self.example
    }

    /// Returns a short description of the resulting CSS semantics.
    #[must_use]
    pub const fn summary(&self) -> &'static str {
        self.summary
    }

    /// Returns the operand domain validated by the semantic compiler.
    #[must_use]
    pub const fn domain(&self) -> UtilityDomain {
        self.domain
    }

    /// Returns the authoring shape of this catalog entry.
    #[must_use]
    pub const fn form(&self) -> UtilityForm {
        match self.resolver {
            CatalogResolver::Fixed(_) => UtilityForm::Fixed,
            CatalogResolver::Family(_) => UtilityForm::Parameterized,
            CatalogResolver::ArbitraryProperty => UtilityForm::ArbitraryProperty,
        }
    }

    /// Returns whether the canonical leading-minus form is supported.
    #[must_use]
    pub const fn allows_negative(&self) -> bool {
        self.capabilities & CAP_NEGATIVE != 0
    }

    /// Returns whether square-bracket arbitrary values are supported.
    #[must_use]
    pub const fn accepts_arbitrary_value(&self) -> bool {
        self.capabilities & CAP_ARBITRARY != 0
    }

    /// Returns whether CSS custom-property operands are supported.
    #[must_use]
    pub const fn accepts_custom_property(&self) -> bool {
        self.capabilities & CAP_CUSTOM_PROPERTY != 0
    }

    /// Returns whether a slash modifier is supported for at least one value in the domain.
    #[must_use]
    pub const fn accepts_modifier(&self) -> bool {
        self.capabilities & CAP_MODIFIER != 0
    }

    const fn fixed(
        spelling: &'static str,
        example: &'static str,
        summary: &'static str,
        resolver: FixedResolver,
        capabilities: u8,
    ) -> Self {
        Self {
            pattern: spelling,
            match_name: spelling,
            example,
            summary,
            domain: UtilityDomain::None,
            capabilities,
            resolver: CatalogResolver::Fixed(resolver),
        }
    }

    const fn family(
        pattern: &'static str,
        match_name: &'static str,
        example: &'static str,
        summary: &'static str,
        domain: UtilityDomain,
        resolver: FamilyResolver,
        capabilities: u8,
    ) -> Self {
        Self {
            pattern,
            match_name,
            example,
            summary,
            domain,
            capabilities,
            resolver: CatalogResolver::Family(resolver),
        }
    }

    const fn arbitrary_property() -> Self {
        Self {
            pattern: "[property:value]",
            match_name: "",
            example: "[mask-type:luminance]",
            summary: "Emit one parsed arbitrary CSS declaration.",
            domain: UtilityDomain::ArbitraryProperty,
            capabilities: CAP_ARBITRARY,
            resolver: CatalogResolver::ArbitraryProperty,
        }
    }
}

const CAP_NEGATIVE: u8 = 1;
const CAP_ARBITRARY: u8 = 1 << 1;
const CAP_CUSTOM_PROPERTY: u8 = 1 << 2;
const CAP_MODIFIER: u8 = 1 << 3;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum CatalogResolver {
    Fixed(FixedResolver),
    Family(FamilyResolver),
    ArbitraryProperty,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum FixedResolver {
    Single {
        utility: Utility,
        slots: SlotSet,
        value: FixedValue,
    },
    FlexOne,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum FixedValue {
    Keyword(Keyword),
    Pixel(i64),
    Fraction { numerator: u16, denominator: u16 },
    ThemeToken { kind: TokenKind, name: &'static str },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum FamilyResolver {
    BackgroundColor,
    Border,
    BorderRadius,
    BoxShadow,
    Font,
    Gap,
    GridColumn,
    GridTemplateColumns,
    Height,
    Leading,
    Margin,
    MarginTop,
    MarginX,
    MaxWidth,
    MinHeight,
    MinWidth,
    Opacity,
    Padding,
    PaddingBottom,
    PaddingX,
    PaddingY,
    Ring,
    Text,
    Width,
}

macro_rules! fixed_keyword {
    ($spelling:literal, $example:literal, $summary:literal, $utility:expr, $slots:expr, $keyword:expr) => {
        UtilityDescriptor::fixed(
            $spelling,
            $example,
            $summary,
            FixedResolver::Single {
                utility: $utility,
                slots: $slots,
                value: FixedValue::Keyword($keyword),
            },
            0,
        )
    };
}

const CATALOG: &[UtilityDescriptor] = &[
    UtilityDescriptor::arbitrary_property(),
    fixed_keyword!(
        "antialiased",
        "antialiased",
        "Enable platform font smoothing.",
        Utility::FontSmoothing,
        SlotSet::from_slot(Slot::FontSmoothing),
        Keyword::Antialiased
    ),
    UtilityDescriptor::fixed(
        "aspect-square",
        "aspect-square",
        "Set a one-to-one aspect ratio.",
        FixedResolver::Single {
            utility: Utility::AspectRatio,
            slots: SlotSet::from_slot(Slot::AspectRatio),
            value: FixedValue::Fraction {
                numerator: 1,
                denominator: 1,
            },
        },
        0,
    ),
    UtilityDescriptor::family(
        "bg-{color}",
        "bg",
        "bg-accent/20",
        "Set the background color with optional named-token alpha.",
        UtilityDomain::Color,
        FamilyResolver::BackgroundColor,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY | CAP_MODIFIER,
    ),
    fixed_keyword!(
        "block",
        "block",
        "Use block display.",
        Utility::Display,
        SlotSet::from_slot(Slot::DisplayMode),
        Keyword::Block
    ),
    UtilityDescriptor::fixed(
        "border",
        "border",
        "Set all border widths to one pixel with solid style.",
        FixedResolver::Single {
            utility: Utility::BorderWidth,
            slots: SlotSet::BORDER_WIDTH,
            value: FixedValue::Pixel(1),
        },
        0,
    ),
    UtilityDescriptor::fixed(
        "border-b",
        "border-b",
        "Set the bottom border width to one pixel with solid style.",
        FixedResolver::Single {
            utility: Utility::BorderWidth,
            slots: SlotSet::from_slot(Slot::BorderBottomWidth),
            value: FixedValue::Pixel(1),
        },
        0,
    ),
    UtilityDescriptor::family(
        "border-{width-or-color}",
        "border",
        "border-2",
        "Set all border widths in pixels or set the border color.",
        UtilityDomain::BorderWidthOrColor,
        FamilyResolver::Border,
        0,
    ),
    UtilityDescriptor::family(
        "col-span-{n}",
        "col-span",
        "col-span-2",
        "Span one through twelve grid columns.",
        UtilityDomain::GridColumnSpan,
        FamilyResolver::GridColumn,
        0,
    ),
    fixed_keyword!(
        "container-inline",
        "container-inline",
        "Establish an inline-size query container.",
        Utility::ContainerType,
        SlotSet::from_slot(Slot::ContainerType),
        Keyword::InlineSize
    ),
    fixed_keyword!(
        "container-normal",
        "container-normal",
        "Disable size-query containment.",
        Utility::ContainerType,
        SlotSet::from_slot(Slot::ContainerType),
        Keyword::Normal
    ),
    fixed_keyword!(
        "cursor-not-allowed",
        "cursor-not-allowed",
        "Use the not-allowed cursor.",
        Utility::Cursor,
        SlotSet::from_slot(Slot::Cursor),
        Keyword::NotAllowed
    ),
    fixed_keyword!(
        "cursor-pointer",
        "cursor-pointer",
        "Use the pointer cursor.",
        Utility::Cursor,
        SlotSet::from_slot(Slot::Cursor),
        Keyword::Pointer
    ),
    fixed_keyword!(
        "flex",
        "flex",
        "Use flex display.",
        Utility::Display,
        SlotSet::from_slot(Slot::DisplayMode),
        Keyword::Flex
    ),
    UtilityDescriptor::fixed(
        "flex-1",
        "flex-1",
        "Set flex grow and shrink to one with a zero-percent basis.",
        FixedResolver::FlexOne,
        0,
    ),
    fixed_keyword!(
        "flex-col",
        "flex-col",
        "Lay out flex items in a column.",
        Utility::FlexDirection,
        SlotSet::from_slot(Slot::FlexDirection),
        Keyword::Column
    ),
    fixed_keyword!(
        "flex-row",
        "flex-row",
        "Lay out flex items in a row.",
        Utility::FlexDirection,
        SlotSet::from_slot(Slot::FlexDirection),
        Keyword::Row
    ),
    fixed_keyword!(
        "flex-wrap",
        "flex-wrap",
        "Allow flex items to wrap.",
        Utility::FlexWrap,
        SlotSet::from_slot(Slot::FlexWrap),
        Keyword::Wrap
    ),
    UtilityDescriptor::family(
        "font-{family-or-weight}",
        "font",
        "font-semibold",
        "Set a registered font family or font weight.",
        UtilityDomain::FontFamilyOrWeight,
        FamilyResolver::Font,
        0,
    ),
    UtilityDescriptor::family(
        "gap-{space}",
        "gap",
        "gap-4",
        "Set row and column gap from one spacing value.",
        UtilityDomain::Spacing,
        FamilyResolver::Gap,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    fixed_keyword!(
        "grid",
        "grid",
        "Use grid display.",
        Utility::Display,
        SlotSet::from_slot(Slot::DisplayMode),
        Keyword::Grid
    ),
    UtilityDescriptor::family(
        "grid-cols-{value}",
        "grid-cols",
        "grid-cols-[1fr 2fr]",
        "Set a grid column count or explicit template.",
        UtilityDomain::GridTemplate,
        FamilyResolver::GridTemplateColumns,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    UtilityDescriptor::family(
        "h-{size}",
        "h",
        "h-full",
        "Set height from the size domain.",
        UtilityDomain::Size,
        FamilyResolver::Height,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    fixed_keyword!(
        "hidden",
        "hidden",
        "Use none display.",
        Utility::Display,
        SlotSet::from_slot(Slot::DisplayMode),
        Keyword::None
    ),
    fixed_keyword!(
        "inline-block",
        "inline-block",
        "Use inline-block display.",
        Utility::Display,
        SlotSet::from_slot(Slot::DisplayMode),
        Keyword::InlineBlock
    ),
    fixed_keyword!(
        "inline-flex",
        "inline-flex",
        "Use inline-flex display.",
        Utility::Display,
        SlotSet::from_slot(Slot::DisplayMode),
        Keyword::InlineFlex
    ),
    fixed_keyword!(
        "items-center",
        "items-center",
        "Center items on the cross axis.",
        Utility::AlignItems,
        SlotSet::from_slot(Slot::AlignItems),
        Keyword::Center
    ),
    fixed_keyword!(
        "items-end",
        "items-end",
        "Align items to the cross-axis end.",
        Utility::AlignItems,
        SlotSet::from_slot(Slot::AlignItems),
        Keyword::End
    ),
    fixed_keyword!(
        "items-start",
        "items-start",
        "Align items to the cross-axis start.",
        Utility::AlignItems,
        SlotSet::from_slot(Slot::AlignItems),
        Keyword::Start
    ),
    fixed_keyword!(
        "justify-between",
        "justify-between",
        "Distribute items with space between them.",
        Utility::JustifyContent,
        SlotSet::from_slot(Slot::JustifyContent),
        Keyword::SpaceBetween
    ),
    fixed_keyword!(
        "justify-center",
        "justify-center",
        "Center items on the main axis.",
        Utility::JustifyContent,
        SlotSet::from_slot(Slot::JustifyContent),
        Keyword::Center
    ),
    fixed_keyword!(
        "justify-end",
        "justify-end",
        "Align items to the main-axis end.",
        Utility::JustifyContent,
        SlotSet::from_slot(Slot::JustifyContent),
        Keyword::End
    ),
    fixed_keyword!(
        "justify-start",
        "justify-start",
        "Align items to the main-axis start.",
        Utility::JustifyContent,
        SlotSet::from_slot(Slot::JustifyContent),
        Keyword::Start
    ),
    UtilityDescriptor::family(
        "leading-{line-height}",
        "leading",
        "leading-6",
        "Set line height from the line-height domain.",
        UtilityDomain::LineHeight,
        FamilyResolver::Leading,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    UtilityDescriptor::family(
        "m-{value}",
        "m",
        "-m-4",
        "Set all margins from spacing or auto.",
        UtilityDomain::Margin,
        FamilyResolver::Margin,
        CAP_NEGATIVE | CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    UtilityDescriptor::family(
        "max-w-{size}",
        "max-w",
        "max-w-6xl",
        "Set maximum width from the size domain.",
        UtilityDomain::Size,
        FamilyResolver::MaxWidth,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    UtilityDescriptor::family(
        "min-h-{size}",
        "min-h",
        "min-h-0",
        "Set minimum height from the size domain.",
        UtilityDomain::Size,
        FamilyResolver::MinHeight,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    UtilityDescriptor::family(
        "min-w-{size}",
        "min-w",
        "min-w-0",
        "Set minimum width from the size domain.",
        UtilityDomain::Size,
        FamilyResolver::MinWidth,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    UtilityDescriptor::family(
        "mt-{value}",
        "mt",
        "-mt-4",
        "Set top margin from spacing or auto.",
        UtilityDomain::Margin,
        FamilyResolver::MarginTop,
        CAP_NEGATIVE | CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    UtilityDescriptor::family(
        "mx-{value}",
        "mx",
        "-mx-4",
        "Set horizontal margins from spacing or auto.",
        UtilityDomain::Margin,
        FamilyResolver::MarginX,
        CAP_NEGATIVE | CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    UtilityDescriptor::family(
        "opacity-{percentage}",
        "opacity",
        "opacity-50",
        "Set opacity from zero through one hundred percent.",
        UtilityDomain::OpacityPercentage,
        FamilyResolver::Opacity,
        0,
    ),
    UtilityDescriptor::fixed(
        "outline-2",
        "outline-2",
        "Set outline width to two pixels.",
        FixedResolver::Single {
            utility: Utility::OutlineWidth,
            slots: SlotSet::from_slot(Slot::OutlineWidth),
            value: FixedValue::Pixel(2),
        },
        0,
    ),
    UtilityDescriptor::fixed(
        "outline-accent",
        "outline-accent",
        "Set outline color from the accent token.",
        FixedResolver::Single {
            utility: Utility::OutlineColor,
            slots: SlotSet::from_slot(Slot::OutlineColor),
            value: FixedValue::ThemeToken {
                kind: TokenKind::Color,
                name: "accent",
            },
        },
        0,
    ),
    fixed_keyword!(
        "outline-none",
        "outline-none",
        "Disable the outline style.",
        Utility::OutlineStyle,
        SlotSet::from_slot(Slot::OutlineStyle),
        Keyword::None
    ),
    UtilityDescriptor::fixed(
        "outline-offset-2",
        "outline-offset-2",
        "Offset the outline by two pixels.",
        FixedResolver::Single {
            utility: Utility::OutlineOffset,
            slots: SlotSet::from_slot(Slot::OutlineOffset),
            value: FixedValue::Pixel(2),
        },
        0,
    ),
    UtilityDescriptor::family(
        "p-{space}",
        "p",
        "p-4",
        "Set all padding sides from one spacing value.",
        UtilityDomain::Spacing,
        FamilyResolver::Padding,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    UtilityDescriptor::family(
        "pb-{space}",
        "pb",
        "pb-4",
        "Set bottom padding from one spacing value.",
        UtilityDomain::Spacing,
        FamilyResolver::PaddingBottom,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    UtilityDescriptor::family(
        "px-{space}",
        "px",
        "px-4",
        "Set horizontal padding from one spacing value.",
        UtilityDomain::Spacing,
        FamilyResolver::PaddingX,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    UtilityDescriptor::family(
        "py-{space}",
        "py",
        "py-4",
        "Set vertical padding from one spacing value.",
        UtilityDomain::Spacing,
        FamilyResolver::PaddingY,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    fixed_keyword!(
        "resize-y",
        "resize-y",
        "Allow vertical resizing.",
        Utility::Resize,
        SlotSet::from_slot(Slot::Resize),
        Keyword::Vertical
    ),
    UtilityDescriptor::family(
        "ring-{width-or-color}",
        "ring",
        "ring-accent/20",
        "Set focus-ring width in pixels or set its color.",
        UtilityDomain::RingWidthOrColor,
        FamilyResolver::Ring,
        CAP_MODIFIER,
    ),
    UtilityDescriptor::family(
        "rounded-{radius}",
        "rounded",
        "rounded-lg",
        "Set all corner radii from one radius value.",
        UtilityDomain::Radius,
        FamilyResolver::BorderRadius,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    UtilityDescriptor::family(
        "shadow-{shadow}",
        "shadow",
        "shadow-md",
        "Set the composed box-shadow value.",
        UtilityDomain::Shadow,
        FamilyResolver::BoxShadow,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    UtilityDescriptor::family(
        "text-{size-or-color}",
        "text",
        "text-sm/6",
        "Set font size or text color with the supported modifier.",
        UtilityDomain::FontSizeOrColor,
        FamilyResolver::Text,
        CAP_CUSTOM_PROPERTY | CAP_MODIFIER,
    ),
    UtilityDescriptor::fixed(
        "tracking-tight",
        "-tracking-tight",
        "Set tight letter spacing, optionally negated.",
        FixedResolver::Single {
            utility: Utility::LetterSpacing,
            slots: SlotSet::from_slot(Slot::LetterSpacing),
            value: FixedValue::ThemeToken {
                kind: TokenKind::LetterSpacing,
                name: "tight",
            },
        },
        CAP_NEGATIVE,
    ),
    fixed_keyword!(
        "transition-colors",
        "transition-colors",
        "Transition the compiler-defined color property bundle.",
        Utility::TransitionProperty,
        SlotSet::from_slot(Slot::TransitionProperty),
        Keyword::Colors
    ),
    UtilityDescriptor::family(
        "w-{size}",
        "w",
        "w-full",
        "Set width from the size domain.",
        UtilityDomain::Size,
        FamilyResolver::Width,
        CAP_ARBITRARY | CAP_CUSTOM_PROPERTY,
    ),
    fixed_keyword!(
        "writing-horizontal",
        "writing-horizontal",
        "Use horizontal top-to-bottom writing mode.",
        Utility::WritingMode,
        SlotSet::from_slot(Slot::WritingMode),
        Keyword::HorizontalTb
    ),
    fixed_keyword!(
        "writing-vertical-lr",
        "writing-vertical-lr",
        "Use vertical writing with left-to-right block flow.",
        Utility::WritingMode,
        SlotSet::from_slot(Slot::WritingMode),
        Keyword::VerticalLr
    ),
    fixed_keyword!(
        "writing-vertical-rl",
        "writing-vertical-rl",
        "Use vertical writing with right-to-left block flow.",
        Utility::WritingMode,
        SlotSet::from_slot(Slot::WritingMode),
        Keyword::VerticalRl
    ),
];

/// Returns the compiler-owned utility catalog in stable lexical pattern order.
#[must_use]
pub const fn utility_catalog() -> &'static [UtilityDescriptor] {
    CATALOG
}

/// Resolves the compiler-owned catalog descriptor for one parsed style item.
///
/// This read-only bridge lets editor and inspection adapters explain the exact utility accepted by
/// lowering without reimplementing longest-prefix catalog resolution.
#[must_use]
pub fn utility_descriptor_for_style_item(item: &StyleItem) -> Option<&'static UtilityDescriptor> {
    match &item.candidate.kind {
        CandidateKind::ArbitraryProperty { .. } => utility_catalog()
            .iter()
            .find(|descriptor| descriptor.form() == UtilityForm::ArbitraryProperty),
        CandidateKind::Named(candidate) => {
            let body = parse_named_candidate(candidate).ok()?.body;
            utility_catalog()
                .iter()
                .find(|descriptor| {
                    descriptor.form() == UtilityForm::Fixed && descriptor.match_name() == body
                })
                .or_else(|| {
                    utility_catalog()
                        .iter()
                        .filter(|descriptor| descriptor.form() == UtilityForm::Parameterized)
                        .filter(|descriptor| {
                            body.strip_prefix(descriptor.match_name())
                                .is_some_and(|suffix| suffix.starts_with('-'))
                        })
                        .max_by_key(|descriptor| descriptor.match_name().len())
                })
        }
    }
}

fn seed_theme() -> &'static ThemeRegistry {
    static SEED: OnceLock<ThemeRegistry> = OnceLock::new();
    SEED.get_or_init(ThemeRegistry::seed)
}

/// Lowers a syntax-level utility list into portable semantic IR.
///
/// # Errors
///
/// Returns the first catalog, condition, value-domain, or conflict diagnostic.
pub fn lower_style(style: &StyleList) -> Result<SemanticStyle, Diagnostic> {
    lower_style_with_theme(seed_theme(), style)
}

/// Lowers a syntax-level utility list against an explicit theme registry.
///
/// # Errors
///
/// Returns the first catalog, condition, value-domain, or conflict diagnostic.
pub fn lower_style_with_theme(
    theme: &ThemeRegistry,
    style: &StyleList,
) -> Result<SemanticStyle, Diagnostic> {
    let mut output = SemanticStyle::default();
    for item in &style.items {
        lower_item(theme, &mut output, item)?;
    }
    canonicalize_conditions(&mut output);
    canonicalize_assignments(&mut output);
    output.validate().map_err(|error| {
        diagnostic(
            DiagnosticCode::InvalidDomain,
            format!("semantic IR invariant failed: {error}"),
            SourceSpan::new(0, 0),
            None,
        )
    })?;
    output.id = derive_style_id_with_theme(theme, &output);
    Ok(output)
}

/// Composes one compiler-validated branch over a compiler-validated base style by semantic slot.
///
/// Assignments in the branch replace overlapping base slots only within the same normalized
/// condition. Non-overlapping base footprints remain in the returned style. Use
/// [`try_compose_style_override`] for semantic IR from an untrusted or mutable source.
///
/// # Panics
///
/// Panics when either style violates a semantic-IR invariant. Compiler lowering produces validated
/// styles; callers handling manually assembled or decoded mutable IR must use the fallible variant.
#[must_use]
pub fn compose_style_override(base: SemanticStyle, branch: SemanticStyle) -> SemanticStyle {
    try_compose_style_override(base, branch)
        .expect("compiler-produced semantic IR must remain valid during composition")
}

/// Composes one compiler-validated branch over a compiler-validated base style under a theme.
///
/// Use [`try_compose_style_override_with_theme`] for semantic IR from an untrusted or mutable
/// source.
///
/// # Panics
///
/// Panics when either style violates a semantic-IR invariant. Compiler lowering produces validated
/// styles; callers handling manually assembled or decoded mutable IR must use the fallible variant.
#[must_use]
pub fn compose_style_override_with_theme(
    theme: &ThemeRegistry,
    base: SemanticStyle,
    branch: SemanticStyle,
) -> SemanticStyle {
    try_compose_style_override_with_theme(theme, base, branch)
        .expect("compiler-produced semantic IR must remain valid during composition")
}

/// Validates and composes a branch over a base style by semantic slot.
///
/// # Errors
///
/// Returns the first structural invariant error in the base, branch, or composed result.
pub fn try_compose_style_override(
    base: SemanticStyle,
    branch: SemanticStyle,
) -> Result<SemanticStyle, pliego_css_ir::InvariantError> {
    try_compose_style_override_with_theme(seed_theme(), base, branch)
}

/// Validates and composes a branch over a base style under an explicit theme.
///
/// Validation occurs before any intern-table indexing, so malformed public semantic IR is returned
/// as a stable structural error instead of panicking.
///
/// # Errors
///
/// Returns the first structural invariant error in the base, branch, or composed result.
pub fn try_compose_style_override_with_theme(
    theme: &ThemeRegistry,
    mut base: SemanticStyle,
    branch: SemanticStyle,
) -> Result<SemanticStyle, pliego_css_ir::InvariantError> {
    base.validate()?;
    branch.validate()?;
    let selector_map = branch
        .selectors
        .iter()
        .map(|selector| base.intern_selector(selector.clone()))
        .collect::<Vec<SelectorId>>();
    let arbitrary_value_map = branch
        .arbitrary_values
        .iter()
        .map(|value| base.intern_arbitrary_value(value.clone()))
        .collect::<Vec<ArbitraryValueId>>();
    let custom_property_map = branch
        .custom_properties
        .iter()
        .map(|property| base.intern_custom_property(property.clone()))
        .collect::<Vec<CustomPropertyId>>();
    let arbitrary_property_map = branch
        .arbitrary_properties
        .iter()
        .cloned()
        .map(|property| base.intern_arbitrary_property(property))
        .collect::<Vec<ArbitraryPropertyId>>();
    let condition_map = branch
        .conditions
        .iter()
        .cloned()
        .map(|mut condition| {
            condition.selectors = condition
                .selectors
                .iter()
                .map(|id| selector_map[id.index()])
                .collect();
            base.intern_condition(condition)
        })
        .collect::<Vec<ConditionId>>();

    let imported = branch
        .assignments
        .into_iter()
        .map(|mut assignment| {
            assignment.condition = condition_map[assignment.condition.index()];
            assignment.value =
                remap_composed_value(assignment.value, &arbitrary_value_map, &custom_property_map);
            if let Utility::ArbitraryProperty(id) = assignment.utility {
                assignment.utility = Utility::ArbitraryProperty(arbitrary_property_map[id.index()]);
            }
            assignment
        })
        .collect::<Vec<_>>();

    for existing in &mut base.assignments {
        let overridden = imported
            .iter()
            .filter(|assignment| {
                assignment.condition == existing.condition
                    && assignments_overlap_in_style(
                        &base.arbitrary_properties,
                        existing,
                        assignment,
                    )
            })
            .fold(SlotSet::EMPTY, |slots, assignment| {
                slots.union(assignment.slots)
            });
        existing.slots = SlotSet::from_bits(existing.slots.bits() & !overridden.bits())
            .unwrap_or(SlotSet::EMPTY);
    }
    base.assignments
        .retain(|assignment| !assignment.slots.is_empty());
    base.assignments.extend(imported);
    canonicalize_conditions(&mut base);
    canonicalize_assignments(&mut base);
    base.validate()?;
    base.id = derive_style_id_with_theme(theme, &base);
    Ok(base)
}

/// One ambiguous overlap between independently selectable conditional clauses.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrossClauseConflict {
    /// Zero-based index of the first conflicting clause.
    pub left_clause: usize,
    /// Zero-based branch index inside the first clause.
    pub left_branch: usize,
    /// Zero-based index of the second conflicting clause.
    pub right_clause: usize,
    /// Zero-based branch index inside the second clause.
    pub right_branch: usize,
    /// Semantic slots both branches can assign under the same condition.
    pub slots: SlotSet,
}

/// Rejects assignment overlap between branches of independently selectable clauses.
///
/// Branches inside one clause are mutually exclusive and are not compared. Conditions are compared
/// structurally, including resolved selector text rather than style-local selector IDs. Exact
/// duplicate assignments are also rejected so independently authored clauses never establish
/// implicit precedence.
///
/// The input styles are expected to be normalized, structurally valid compiler output produced
/// under one shared theme registry.
///
/// # Errors
///
/// Returns the first conflict in clause, branch, and canonical assignment order.
pub fn analyze_cross_clause_conflicts(
    clauses: &[Vec<SemanticStyle>],
) -> Result<(), CrossClauseConflict> {
    for left_clause in 0..clauses.len() {
        for right_clause in (left_clause + 1)..clauses.len() {
            for (left_branch, left) in clauses[left_clause].iter().enumerate() {
                for (right_branch, right) in clauses[right_clause].iter().enumerate() {
                    if let Some(slots) = overlapping_slots_under_same_condition(left, right) {
                        return Err(CrossClauseConflict {
                            left_clause,
                            left_branch,
                            right_clause,
                            right_branch,
                            slots,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn overlapping_slots_under_same_condition(
    left: &SemanticStyle,
    right: &SemanticStyle,
) -> Option<SlotSet> {
    let mut overlap = SlotSet::EMPTY;
    for left_assignment in &left.assignments {
        for right_assignment in &right.assignments {
            if structurally_equal_conditions(
                left,
                left_assignment.condition.index(),
                right,
                right_assignment.condition.index(),
            ) && assignments_overlap_across_styles(
                left,
                left_assignment,
                right,
                right_assignment,
            ) {
                overlap = overlap.union(left_assignment.slots.intersection(right_assignment.slots));
            }
        }
    }
    (!overlap.is_empty()).then_some(overlap)
}

fn assignments_overlap_in_style(
    arbitrary_properties: &[ArbitraryProperty],
    left: &Assignment,
    right: &Assignment,
) -> bool {
    match (left.utility, right.utility) {
        (Utility::ArbitraryProperty(left_id), Utility::ArbitraryProperty(right_id)) => {
            arbitrary_properties.get(left_id.index()) == arbitrary_properties.get(right_id.index())
        }
        _ => left.slots.intersects(right.slots),
    }
}

fn assignments_overlap_across_styles(
    left_style: &SemanticStyle,
    left: &Assignment,
    right_style: &SemanticStyle,
    right: &Assignment,
) -> bool {
    match (left.utility, right.utility) {
        (Utility::ArbitraryProperty(left_id), Utility::ArbitraryProperty(right_id)) => {
            left_style.arbitrary_properties.get(left_id.index())
                == right_style.arbitrary_properties.get(right_id.index())
        }
        _ => left.slots.intersects(right.slots),
    }
}

fn structurally_equal_conditions(
    left: &SemanticStyle,
    left_index: usize,
    right: &SemanticStyle,
    right_index: usize,
) -> bool {
    let (Some(left_condition), Some(right_condition)) = (
        left.conditions.get(left_index),
        right.conditions.get(right_index),
    ) else {
        return false;
    };
    left_condition.breakpoint == right_condition.breakpoint
        && left_condition.container_breakpoint == right_condition.container_breakpoint
        && left_condition.layer == right_condition.layer
        && left_condition.theme == right_condition.theme
        && left_condition.motion == right_condition.motion
        && left_condition.contrast == right_condition.contrast
        && left_condition.states == right_condition.states
        && left_condition.selectors.len() == right_condition.selectors.len()
        && left_condition
            .selectors
            .iter()
            .zip(&right_condition.selectors)
            .all(|(left_id, right_id)| {
                left.selectors.get(left_id.index()) == right.selectors.get(right_id.index())
            })
}

fn remap_composed_value(
    value: SemanticValue,
    arbitrary_values: &[ArbitraryValueId],
    custom_properties: &[CustomPropertyId],
) -> SemanticValue {
    match value {
        SemanticValue::Arbitrary(id) => SemanticValue::Arbitrary(arbitrary_values[id.index()]),
        SemanticValue::CustomProperty(reference) => {
            SemanticValue::CustomProperty(CustomPropertyRef {
                id: custom_properties[reference.id.index()],
                hint: reference.hint,
            })
        }
        value => value,
    }
}

fn lower_item(
    theme: &ThemeRegistry,
    output: &mut SemanticStyle,
    item: &StyleItem,
) -> Result<(), Diagnostic> {
    let condition = lower_condition(theme, output, item)?;
    let condition_id = output.intern_condition(condition);
    let source = Span::try_from(item.span).map_err(|error| {
        diagnostic(
            DiagnosticCode::MalformedItem,
            error.to_string(),
            item.span,
            None,
        )
    })?;

    let assignments = match &item.candidate.kind {
        CandidateKind::Named(candidate) => {
            let syntax = parse_named_candidate_at(candidate, item.candidate.span.start)?;
            lower_named(theme, output, item, &syntax, condition_id, source)?
        }
        CandidateKind::ArbitraryProperty { property, value } => {
            if item.negative {
                return Err(diagnostic(
                    DiagnosticCode::InvalidNegative,
                    "arbitrary properties cannot use the negative utility form",
                    item.span,
                    None,
                ));
            }
            let property_id = output.intern_arbitrary_property(ArbitraryProperty {
                name: property.clone(),
            });
            let value_id = output.intern_arbitrary_value(value.clone());
            vec![Assignment {
                utility: Utility::ArbitraryProperty(property_id),
                slots: SlotSet::from_slot(Slot::ArbitraryProperty),
                value: SemanticValue::Arbitrary(value_id),
                negative: false,
                condition: condition_id,
                important: item.important,
                source,
            }]
        }
    };

    for assignment in assignments {
        insert_assignment(output, assignment, item.span)?;
    }
    Ok(())
}

// Keeping built-in condition dimensions together makes contradictory chains explicit.
#[allow(clippy::too_many_lines)]
fn lower_condition(
    theme: &ThemeRegistry,
    output: &mut SemanticStyle,
    item: &StyleItem,
) -> Result<Condition, Diagnostic> {
    let mut condition = Condition::BASE;
    for variant in &item.variants {
        let name = match &variant.kind {
            VariantKind::Named(name) => name,
            VariantKind::ArbitrarySelector(selector) => {
                validate_selector_transform(selector, variant.span)?;
                push_selector_transform(output, &mut condition, selector, variant.span, false)?;
                continue;
            }
        };
        if let Some(container_name) = name.strip_prefix("cq-") {
            if condition.container_breakpoint.is_some() {
                return Err(invalid_variant(
                    "a condition accepts one container breakpoint",
                    variant.span,
                ));
            }
            let Some(breakpoint) = theme.breakpoint_by_name(container_name) else {
                let breakpoint_names = theme
                    .breakpoints()
                    .iter()
                    .map(|breakpoint| breakpoint.name.as_str())
                    .collect::<Vec<_>>();
                let suggestion = nearest(container_name, &breakpoint_names)
                    .map(|candidate| format!("cq-{candidate}"));
                return Err(diagnostic(
                    DiagnosticCode::UnknownName,
                    format!("unknown container breakpoint variant `{name}`"),
                    variant.span,
                    suggestion,
                ));
            };
            condition.container_breakpoint = Some(breakpoint.id);
            continue;
        }
        if let Some(layer) = cascade_layer_for_variant(name) {
            if condition.layer != CascadeLayer::Unlayered {
                return Err(invalid_variant(
                    "a condition accepts one cascade layer",
                    variant.span,
                ));
            }
            condition.layer = layer;
            continue;
        }
        if let Some(breakpoint) = theme.breakpoint_by_name(name) {
            if condition.breakpoint.is_some() {
                return Err(invalid_variant(
                    "a condition accepts one breakpoint",
                    variant.span,
                ));
            }
            condition.breakpoint = Some(breakpoint.id);
            continue;
        }
        if let Some(selector) = classified_selector_for_variant(name) {
            push_selector_transform(
                output,
                &mut condition,
                selector.as_ref(),
                variant.span,
                true,
            )?;
            continue;
        }
        if is_typed_attribute_variant(name) {
            return Err(invalid_variant(
                "typed attribute variants require canonical `aria-[name=value]`, `data-[name]`, or `data-[name=value]` syntax",
                variant.span,
            ));
        }
        match name.as_str() {
            "dark" | "light" => {
                if condition.theme != ThemeMode::Any {
                    return Err(invalid_variant(
                        "theme variants cannot be combined",
                        variant.span,
                    ));
                }
                condition.theme = if name == "dark" {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                };
            }
            "motion-safe" | "motion-reduce" => {
                if condition.motion != MotionPreference::Any {
                    return Err(invalid_variant(
                        "motion variants cannot be combined",
                        variant.span,
                    ));
                }
                condition.motion = if name == "motion-safe" {
                    MotionPreference::Safe
                } else {
                    MotionPreference::Reduce
                };
            }
            "contrast-more" | "contrast-less" => {
                if condition.contrast != ContrastPreference::Any {
                    return Err(invalid_variant(
                        "contrast variants cannot be combined",
                        variant.span,
                    ));
                }
                condition.contrast = if name == "contrast-more" {
                    ContrastPreference::More
                } else {
                    ContrastPreference::Less
                };
            }
            "hover" | "focus" | "focus-visible" | "active" | "disabled" => {
                let state = match name.as_str() {
                    "hover" => PseudoState::Hover,
                    "focus" => PseudoState::Focus,
                    "focus-visible" => PseudoState::FocusVisible,
                    "active" => PseudoState::Active,
                    "disabled" => PseudoState::Disabled,
                    _ => unreachable!(),
                };
                if condition.states.contains(state) {
                    return Err(invalid_variant("variant is duplicated", variant.span));
                }
                condition.states = condition.states.with(state);
            }
            "placeholder" => {
                push_selector_transform(
                    output,
                    &mut condition,
                    "&::placeholder",
                    variant.span,
                    true,
                )?;
            }
            _ => {
                return Err(diagnostic(
                    DiagnosticCode::UnknownName,
                    format!("unknown variant `{name}`"),
                    variant.span,
                    nearest(name, &known_variants(theme)),
                ));
            }
        }
    }
    Ok(condition)
}

fn selector_family(selector: &str) -> Option<&str> {
    if matches!(selector, "&:dir(ltr)" | "&:dir(rtl)") {
        Some("writing-direction")
    } else {
        classified_attribute_name(selector)
    }
}

fn push_selector_transform(
    output: &mut SemanticStyle,
    condition: &mut Condition,
    selector: &str,
    span: SourceSpan,
    reject_duplicate: bool,
) -> Result<(), Diagnostic> {
    let family = selector_family(selector);
    for id in &condition.selectors {
        let existing = &output.selectors[id.index()];
        if existing == selector {
            if reject_duplicate {
                return Err(invalid_variant("selector variant is duplicated", span));
            }
            continue;
        }
        match family {
            Some(family) if selector_family(existing) == Some(family) => {
                return Err(invalid_variant(
                    format!("{family} variants cannot be combined"),
                    span,
                ));
            }
            _ => {}
        }
    }
    let selector = output.intern_selector(selector.to_owned());
    condition.selectors.push(selector);
    Ok(())
}

fn validate_selector_transform(selector: &str, span: SourceSpan) -> Result<(), Diagnostic> {
    if selector.contains("/*")
        || selector.contains("*/")
        || selector
            .chars()
            .any(|character| matches!(character, '\0' | '\n' | '\r' | '\u{000c}'))
    {
        return Err(invalid_variant(
            "arbitrary selector variants cannot contain CSS comments or raw control line breaks",
            span,
        ));
    }
    let mut quote = None;
    let mut escaped = false;
    let mut delimiters = Vec::new();
    let mut anchor_count = 0_u32;
    let mut characters = selector.chars().peekable();

    while let Some(character) = characters.next() {
        if escaped {
            escaped = false;
            continue;
        }
        if let Some(active_quote) = quote {
            match character {
                '\\' => escaped = true,
                current if current == active_quote => quote = None,
                _ => {}
            }
            continue;
        }

        match character {
            '\'' | '"' => quote = Some(character),
            '\\' => escaped = true,
            '&' => anchor_count += 1,
            '[' | '(' => delimiters.push(character),
            ']' | ')' => {
                let expected = if character == ']' { '[' } else { '(' };
                if delimiters.pop() != Some(expected) {
                    return Err(invalid_variant(
                        "arbitrary selector variants must have correctly nested delimiters",
                        span,
                    ));
                }
            }
            ',' if delimiters.is_empty() => {
                return Err(invalid_variant(
                    "arbitrary selector variants cannot contain a top-level selector list",
                    span,
                ));
            }
            '@' => {
                return Err(invalid_variant(
                    "at-rules are not allowed in arbitrary selector variants",
                    span,
                ));
            }
            '{' | '}' | ';' => {
                return Err(invalid_variant(
                    "blocks and declarations are not allowed in arbitrary selector variants",
                    span,
                ));
            }
            '/' if characters.peek() == Some(&'*') => {
                return Err(invalid_variant(
                    "comments are not allowed in arbitrary selector variants",
                    span,
                ));
            }
            '<' | '\0' | '\n' | '\r' => {
                return Err(invalid_variant(
                    "unsafe characters are not allowed in arbitrary selector variants",
                    span,
                ));
            }
            _ => {}
        }
    }

    if escaped || quote.is_some() || !delimiters.is_empty() {
        return Err(invalid_variant(
            "arbitrary selector variants must have balanced syntax",
            span,
        ));
    }
    if !selector.starts_with('&') || anchor_count != 1 {
        return Err(invalid_variant(
            "an arbitrary selector variant must start with exactly one unquoted `&` anchor",
            span,
        ));
    }
    Ok(())
}

/// Validates one decoded selector transform using the compiler's authoring safety rules.
///
/// # Errors
///
/// Returns a diagnostic when the selector does not start with exactly one `&` anchor, contains an
/// unsafe selector-list, at-rule, comment, declaration, or control syntax, or has unbalanced quotes
/// or delimiters.
pub fn validate_selector_transform_text(selector: &str) -> Result<(), Diagnostic> {
    validate_selector_transform(selector, SourceSpan::new(0, selector.len()))
}

fn lower_named(
    theme: &ThemeRegistry,
    output: &mut SemanticStyle,
    item: &StyleItem,
    syntax: &NamedCandidateSyntax,
    condition: pliego_css_ir::ConditionId,
    source: Span,
) -> Result<Vec<Assignment>, Diagnostic> {
    if let Some(descriptor) = fixed_utility(&syntax.body) {
        return lower_fixed(theme, descriptor, item, syntax, condition, source);
    }

    let (descriptor, operand_text) = resolve_family(&syntax.body).ok_or_else(|| {
        diagnostic(
            DiagnosticCode::UnknownUtility,
            format!("unknown utility `{}`", syntax.body),
            syntax.body_span,
            nearest(&syntax.body, &known_candidates()),
        )
    })?;
    let operand_offset = syntax.body_span.end - operand_text.len();
    let operand = pliego_css_parser::parse_operand_at(operand_text, operand_offset)?;
    let CatalogResolver::Family(family) = descriptor.resolver else {
        unreachable!("family lookup returned a non-family descriptor")
    };
    if item.negative && !descriptor.allows_negative() {
        return Err(diagnostic(
            DiagnosticCode::InvalidNegative,
            format!(
                "`{}` cannot use the negative utility form",
                descriptor.match_name
            ),
            item.span,
            None,
        ));
    }
    if syntax.modifier.is_some() && !descriptor.accepts_modifier() {
        require_no_modifier(syntax.modifier.as_ref())?;
    }
    let resolved = resolve_family_value(theme, output, family, &operand, syntax.modifier.as_ref())?;

    let mut assignments = Vec::with_capacity(resolved.len());
    for (utility, slots, value) in resolved {
        if item.negative && matches!(value, SemanticValue::Keyword(_)) {
            return Err(diagnostic(
                DiagnosticCode::InvalidNegative,
                format!("`{}` cannot negate a keyword value", descriptor.match_name),
                item.span,
                None,
            ));
        }
        assignments.push(Assignment {
            utility,
            slots,
            value,
            negative: item.negative,
            condition,
            important: item.important,
            source,
        });
    }
    Ok(assignments)
}

fn fixed_utility(body: &str) -> Option<&'static UtilityDescriptor> {
    utility_catalog().iter().find(|descriptor| {
        descriptor.match_name == body && matches!(descriptor.resolver, CatalogResolver::Fixed(_))
    })
}

fn lower_fixed(
    theme: &ThemeRegistry,
    descriptor: &UtilityDescriptor,
    item: &StyleItem,
    syntax: &NamedCandidateSyntax,
    condition: pliego_css_ir::ConditionId,
    source: Span,
) -> Result<Vec<Assignment>, Diagnostic> {
    require_no_modifier(syntax.modifier.as_ref())?;
    let CatalogResolver::Fixed(fixed_kind) = descriptor.resolver else {
        unreachable!("fixed lookup returned a non-fixed descriptor")
    };
    let resolved = match fixed_kind {
        FixedResolver::Single {
            utility,
            slots,
            value,
        } => vec![(
            utility,
            slots,
            match value {
                FixedValue::Keyword(value) => SemanticValue::Keyword(value),
                FixedValue::Pixel(value) => pixel_value(value),
                FixedValue::Fraction {
                    numerator,
                    denominator,
                } => SemanticValue::Fraction {
                    numerator,
                    denominator,
                },
                FixedValue::ThemeToken { kind, name } => {
                    required_theme_token(theme, kind, name, syntax.body_span)?
                }
            },
        )],
        FixedResolver::FlexOne => vec![
            (
                Utility::FlexGrow,
                SlotSet::from_slot(Slot::FlexGrow),
                SemanticValue::Integer(1),
            ),
            (
                Utility::FlexShrink,
                SlotSet::from_slot(Slot::FlexShrink),
                SemanticValue::Integer(1),
            ),
            (
                Utility::FlexBasis,
                SlotSet::from_slot(Slot::FlexBasis),
                SemanticValue::Percentage(Percentage::ZERO),
            ),
        ],
    };
    if item.negative && !descriptor.allows_negative() {
        return Err(diagnostic(
            DiagnosticCode::InvalidNegative,
            format!(
                "`{}` cannot use the negative utility form",
                descriptor.pattern
            ),
            item.span,
            None,
        ));
    }
    if item.negative
        && resolved
            .iter()
            .any(|(_, _, value)| matches!(value, SemanticValue::Keyword(_)))
    {
        return Err(diagnostic(
            DiagnosticCode::InvalidNegative,
            format!("`{}` cannot negate a keyword value", descriptor.pattern),
            item.span,
            None,
        ));
    }
    Ok(resolved
        .into_iter()
        .map(|(utility, slots, value)| Assignment {
            utility,
            slots,
            value,
            negative: item.negative,
            condition,
            important: item.important,
            source,
        })
        .collect())
}

fn resolve_family(body: &str) -> Option<(&'static UtilityDescriptor, &str)> {
    utility_catalog()
        .iter()
        .filter(|descriptor| matches!(descriptor.resolver, CatalogResolver::Family(_)))
        .filter_map(|descriptor| {
            body.strip_prefix(descriptor.match_name)
                .and_then(|rest| rest.strip_prefix('-'))
                .filter(|operand| !operand.is_empty())
                .map(|operand| (descriptor, operand))
        })
        .max_by_key(|(descriptor, _)| descriptor.match_name.len())
}

// The exhaustive family table remains centralized so catalog coverage is auditable.
#[allow(clippy::too_many_lines)]
fn resolve_family_value(
    theme: &ThemeRegistry,
    output: &mut SemanticStyle,
    family: FamilyResolver,
    operand: &OperandSyntax,
    modifier: Option<&OperandSyntax>,
) -> Result<Vec<(Utility, SlotSet, SemanticValue)>, Diagnostic> {
    let one = SlotSet::from_slot;
    match family {
        FamilyResolver::GridTemplateColumns => Ok(vec![(
            Utility::GridTemplateColumns,
            one(Slot::GridTemplateColumns),
            grid_template(output, operand)?,
        )]),
        FamilyResolver::GridColumn => Ok(vec![(
            Utility::GridColumn,
            one(Slot::GridColumn),
            integer_operand(operand, 1, 12)?,
        )]),
        FamilyResolver::Gap => Ok(vec![(
            Utility::Gap,
            SlotSet::GAP,
            spacing(theme, output, operand)?,
        )]),
        FamilyResolver::Padding => Ok(vec![(
            Utility::Padding,
            SlotSet::PADDING,
            spacing(theme, output, operand)?,
        )]),
        FamilyResolver::PaddingX => Ok(vec![(
            Utility::Padding,
            SlotSet::of(&[Slot::PaddingLeft, Slot::PaddingRight]),
            spacing(theme, output, operand)?,
        )]),
        FamilyResolver::PaddingY => Ok(vec![(
            Utility::Padding,
            SlotSet::of(&[Slot::PaddingTop, Slot::PaddingBottom]),
            spacing(theme, output, operand)?,
        )]),
        FamilyResolver::PaddingBottom => Ok(vec![(
            Utility::Padding,
            one(Slot::PaddingBottom),
            spacing(theme, output, operand)?,
        )]),
        FamilyResolver::Margin => Ok(vec![(
            Utility::Margin,
            SlotSet::MARGIN,
            margin(theme, output, operand)?,
        )]),
        FamilyResolver::MarginX => Ok(vec![(
            Utility::Margin,
            SlotSet::of(&[Slot::MarginLeft, Slot::MarginRight]),
            margin(theme, output, operand)?,
        )]),
        FamilyResolver::MarginTop => Ok(vec![(
            Utility::Margin,
            one(Slot::MarginTop),
            margin(theme, output, operand)?,
        )]),
        FamilyResolver::Width => Ok(vec![(
            Utility::Width,
            one(Slot::Width),
            size(theme, output, operand)?,
        )]),
        FamilyResolver::Height => Ok(vec![(
            Utility::Height,
            one(Slot::Height),
            size(theme, output, operand)?,
        )]),
        FamilyResolver::MaxWidth => Ok(vec![(
            Utility::MaxWidth,
            one(Slot::MaxWidth),
            size(theme, output, operand)?,
        )]),
        FamilyResolver::MinWidth => Ok(vec![(
            Utility::MinWidth,
            one(Slot::MinWidth),
            size(theme, output, operand)?,
        )]),
        FamilyResolver::MinHeight => Ok(vec![(
            Utility::MinHeight,
            one(Slot::MinHeight),
            size(theme, output, operand)?,
        )]),
        FamilyResolver::BackgroundColor => Ok(vec![(
            Utility::BackgroundColor,
            one(Slot::BackgroundColor),
            color(theme, output, operand, modifier)?,
        )]),
        FamilyResolver::Text => text(theme, output, operand, modifier),
        FamilyResolver::Font => font(theme, operand),
        FamilyResolver::Leading => Ok(vec![(
            Utility::LineHeight,
            one(Slot::LineHeight),
            catalog_token(theme, output, operand, TokenKind::LineHeight)?,
        )]),
        FamilyResolver::Border => Ok(vec![border(theme, output, operand)?]),
        FamilyResolver::BorderRadius => Ok(vec![(
            Utility::BorderRadius,
            SlotSet::BORDER_RADIUS,
            catalog_token(theme, output, operand, TokenKind::Radius)?,
        )]),
        FamilyResolver::BoxShadow => Ok(vec![(
            Utility::BoxShadow,
            one(Slot::BoxShadow),
            catalog_token(theme, output, operand, TokenKind::Shadow)?,
        )]),
        FamilyResolver::Ring => Ok(vec![ring(theme, output, operand, modifier)?]),
        FamilyResolver::Opacity => Ok(vec![(
            Utility::Opacity,
            one(Slot::Opacity),
            percentage_operand(operand)?,
        )]),
    }
}

fn spacing(
    theme: &ThemeRegistry,
    output: &mut SemanticStyle,
    operand: &OperandSyntax,
) -> Result<SemanticValue, Diagnostic> {
    catalog_token(theme, output, operand, TokenKind::Spacing).map_err(|error| {
        if error.code == DiagnosticCode::InvalidDomain {
            domain_error(operand, "spacing token")
        } else {
            error
        }
    })
}

fn margin(
    theme: &ThemeRegistry,
    output: &mut SemanticStyle,
    operand: &OperandSyntax,
) -> Result<SemanticValue, Diagnostic> {
    if matches!(&operand.kind, OperandKind::Bare(value) if value == "auto") {
        Ok(SemanticValue::Keyword(Keyword::Auto))
    } else {
        spacing(theme, output, operand)
    }
}

fn size(
    theme: &ThemeRegistry,
    output: &mut SemanticStyle,
    operand: &OperandSyntax,
) -> Result<SemanticValue, Diagnostic> {
    if matches!(&operand.kind, OperandKind::Bare(value) if value == "auto") {
        Ok(SemanticValue::Keyword(Keyword::Auto))
    } else {
        catalog_token(theme, output, operand, TokenKind::Spacing).map_err(|error| {
            if error.code == DiagnosticCode::InvalidDomain {
                domain_error(operand, "size token")
            } else {
                error
            }
        })
    }
}

fn text(
    theme: &ThemeRegistry,
    output: &mut SemanticStyle,
    operand: &OperandSyntax,
    modifier: Option<&OperandSyntax>,
) -> Result<Vec<(Utility, SlotSet, SemanticValue)>, Diagnostic> {
    let one = SlotSet::from_slot;
    match &operand.kind {
        OperandKind::Bare(value) if theme.token_by_name(TokenKind::FontSize, value).is_some() => {
            let mut assignments = vec![(
                Utility::FontSize,
                one(Slot::FontSize),
                required_theme_token(theme, TokenKind::FontSize, value, operand.span)?,
            )];
            if let Some(line_height) = modifier {
                assignments.push((
                    Utility::LineHeight,
                    one(Slot::LineHeight),
                    catalog_token(theme, output, line_height, TokenKind::LineHeight)?,
                ));
            }
            Ok(assignments)
        }
        OperandKind::Bare(value) if theme.token_by_name(TokenKind::Color, value).is_some() => {
            Ok(vec![(
                Utility::TextColor,
                one(Slot::TextColor),
                color(theme, output, operand, modifier)?,
            )])
        }
        OperandKind::CustomVariable {
            type_hint: None, ..
        } => Err(diagnostic(
            DiagnosticCode::AmbiguousNamespace,
            "`text-(--value)` is ambiguous between font size and color",
            operand.span,
            Some("use `text-(color:--value)` or an explicit font-size hint".into()),
        )),
        OperandKind::CustomVariable {
            type_hint: Some(hint),
            ..
        } if hint == "color" => Ok(vec![(
            Utility::TextColor,
            one(Slot::TextColor),
            custom_property(output, operand, ValueKind::Color)?,
        )]),
        _ => Err(domain_error(operand, "font-size or color token")),
    }
}

fn font(
    theme: &ThemeRegistry,
    operand: &OperandSyntax,
) -> Result<Vec<(Utility, SlotSet, SemanticValue)>, Diagnostic> {
    let one = SlotSet::from_slot;
    match &operand.kind {
        OperandKind::Bare(value) if theme.token_by_name(TokenKind::FontFamily, value).is_some() => {
            Ok(vec![(
                Utility::FontFamily,
                one(Slot::FontFamily),
                required_theme_token(theme, TokenKind::FontFamily, value, operand.span)?,
            )])
        }
        OperandKind::Bare(value) if theme.token_by_name(TokenKind::FontWeight, value).is_some() => {
            Ok(vec![(
                Utility::FontWeight,
                one(Slot::FontWeight),
                required_theme_token(theme, TokenKind::FontWeight, value, operand.span)?,
            )])
        }
        _ => Err(domain_error(operand, "font-family or font-weight token")),
    }
}

fn border(
    theme: &ThemeRegistry,
    output: &mut SemanticStyle,
    operand: &OperandSyntax,
) -> Result<(Utility, SlotSet, SemanticValue), Diagnostic> {
    match &operand.kind {
        OperandKind::Bare(value) if theme.token_by_name(TokenKind::Color, value).is_some() => Ok((
            Utility::BorderColor,
            SlotSet::BORDER_COLOR,
            color(theme, output, operand, None)?,
        )),
        OperandKind::Bare(value) if value.parse::<i64>().is_ok_and(|value| value >= 0) => Ok((
            Utility::BorderWidth,
            SlotSet::BORDER_WIDTH,
            pixel_value(value.parse().expect("guarded integer border width")),
        )),
        OperandKind::CustomVariable {
            type_hint: None, ..
        } => Err(diagnostic(
            DiagnosticCode::AmbiguousNamespace,
            "`border-(--value)` is ambiguous between width and color",
            operand.span,
            Some("add a `color:` or `length:` type hint".into()),
        )),
        _ => Err(domain_error(operand, "border width or color token")),
    }
}

fn ring(
    theme: &ThemeRegistry,
    output: &mut SemanticStyle,
    operand: &OperandSyntax,
    modifier: Option<&OperandSyntax>,
) -> Result<(Utility, SlotSet, SemanticValue), Diagnostic> {
    match &operand.kind {
        OperandKind::Bare(value) if theme.token_by_name(TokenKind::Color, value).is_some() => Ok((
            Utility::RingColor,
            SlotSet::from_slot(Slot::RingColor),
            color(theme, output, operand, modifier)?,
        )),
        OperandKind::Bare(value) if value.parse::<i64>().is_ok_and(|value| value >= 0) => Ok((
            Utility::RingWidth,
            SlotSet::from_slot(Slot::RingWidth),
            pixel_value(value.parse().expect("guarded integer ring width")),
        )),
        OperandKind::CustomVariable {
            type_hint: None, ..
        } => Err(diagnostic(
            DiagnosticCode::AmbiguousNamespace,
            "`ring-(--value)` is ambiguous between width and color",
            operand.span,
            Some("add a `color:` or `length:` type hint".into()),
        )),
        _ => Err(domain_error(operand, "ring width or color token")),
    }
}

fn pixel_value(value: i64) -> SemanticValue {
    SemanticValue::Length(Length {
        number: CssNumber::new(value, 0).expect("integer pixels are canonical CSS numbers"),
        unit: LengthUnit::Px,
    })
}

fn color(
    theme: &ThemeRegistry,
    output: &mut SemanticStyle,
    operand: &OperandSyntax,
    modifier: Option<&OperandSyntax>,
) -> Result<SemanticValue, Diagnostic> {
    match &operand.kind {
        OperandKind::Bare(value) if value == "transparent" => {
            require_no_modifier(modifier)?;
            Ok(SemanticValue::Color(ColorValue::Transparent))
        }
        OperandKind::Bare(value) if value == "current" => {
            require_no_modifier(modifier)?;
            Ok(SemanticValue::Color(ColorValue::CurrentColor))
        }
        OperandKind::Bare(value) if theme.token_by_name(TokenKind::Color, value).is_some() => {
            let alpha = modifier.map(percentage).transpose()?;
            Ok(SemanticValue::Color(ColorValue::Token {
                id: theme
                    .token_by_name(TokenKind::Color, value)
                    .expect("guarded theme token")
                    .id,
                alpha,
            }))
        }
        OperandKind::ArbitraryValue(value) => {
            require_no_modifier(modifier)?;
            let id = output.intern_arbitrary_value(value.clone());
            Ok(SemanticValue::Arbitrary(id))
        }
        OperandKind::CustomVariable { .. } => custom_property(output, operand, ValueKind::Color),
        OperandKind::Bare(_) => Err(domain_error(operand, "color token")),
    }
}

fn catalog_token(
    theme: &ThemeRegistry,
    output: &mut SemanticStyle,
    operand: &OperandSyntax,
    kind: TokenKind,
) -> Result<SemanticValue, Diagnostic> {
    match &operand.kind {
        OperandKind::Bare(value) if theme.token_by_name(kind, value).is_some() => {
            required_theme_token(theme, kind, value, operand.span)
        }
        OperandKind::ArbitraryValue(value) => {
            let id = output.intern_arbitrary_value(value.clone());
            Ok(SemanticValue::Arbitrary(id))
        }
        OperandKind::CustomVariable { .. } => custom_property(output, operand, ValueKind::Any),
        OperandKind::Bare(value) if token_exists_outside(theme, kind, value) => {
            Err(domain_error(operand, &format!("{kind:?} token")))
        }
        OperandKind::Bare(value) => Err(diagnostic(
            DiagnosticCode::UnknownName,
            format!("unknown {kind:?} token `{value}`"),
            operand.span,
            nearest(value, &token_names(theme, kind)),
        )),
    }
}

fn token_exists_outside(theme: &ThemeRegistry, kind: TokenKind, value: &str) -> bool {
    theme
        .tokens()
        .iter()
        .any(|token| token.kind != kind && token.name == value)
}

fn custom_property(
    output: &mut SemanticStyle,
    operand: &OperandSyntax,
    default_hint: ValueKind,
) -> Result<SemanticValue, Diagnostic> {
    let OperandKind::CustomVariable { type_hint, name } = &operand.kind else {
        return Err(domain_error(operand, "custom property"));
    };
    let hint = type_hint
        .as_deref()
        .map(value_kind)
        .transpose()?
        .unwrap_or(default_hint);
    let id = output.intern_custom_property(name.clone());
    Ok(SemanticValue::CustomProperty(CustomPropertyRef {
        id,
        hint,
    }))
}

fn value_kind(value: &str) -> Result<ValueKind, Diagnostic> {
    match value {
        "color" => Ok(ValueKind::Color),
        "length" => Ok(ValueKind::Length),
        "number" => Ok(ValueKind::Number),
        "percentage" => Ok(ValueKind::Percentage),
        _ => Err(diagnostic(
            DiagnosticCode::InvalidDomain,
            format!("unknown custom-property type hint `{value}`"),
            SourceSpan::new(0, 0),
            Some("use `color`, `length`, `number`, or `percentage`".into()),
        )),
    }
}

fn integer_operand(
    operand: &OperandSyntax,
    minimum: i32,
    maximum: i32,
) -> Result<SemanticValue, Diagnostic> {
    let OperandKind::Bare(value) = &operand.kind else {
        return Err(domain_error(operand, "integer"));
    };
    let number = value
        .parse::<i32>()
        .map_err(|_| domain_error(operand, "integer"))?;
    if !(minimum..=maximum).contains(&number) {
        return Err(domain_error(
            operand,
            &format!("integer {minimum}..={maximum}"),
        ));
    }
    Ok(SemanticValue::Integer(number))
}

fn grid_template(
    output: &mut SemanticStyle,
    operand: &OperandSyntax,
) -> Result<SemanticValue, Diagnostic> {
    match &operand.kind {
        OperandKind::Bare(_) => integer_operand(operand, 1, 12),
        OperandKind::ArbitraryValue(value) => {
            let id = output.intern_arbitrary_value(value.clone());
            Ok(SemanticValue::Arbitrary(id))
        }
        OperandKind::CustomVariable { .. } => {
            custom_property(output, operand, ValueKind::GridTemplate)
        }
    }
}

fn percentage_operand(operand: &OperandSyntax) -> Result<SemanticValue, Diagnostic> {
    Ok(SemanticValue::Percentage(percentage(operand)?))
}

fn percentage(operand: &OperandSyntax) -> Result<Percentage, Diagnostic> {
    let OperandKind::Bare(value) = &operand.kind else {
        return Err(domain_error(operand, "percentage 0..100"));
    };
    let percent = value
        .parse::<u16>()
        .map_err(|_| domain_error(operand, "percentage 0..100"))?;
    Percentage::from_basis_points(percent.saturating_mul(100))
        .ok_or_else(|| domain_error(operand, "percentage 0..100"))
}

fn require_no_modifier(modifier: Option<&OperandSyntax>) -> Result<(), Diagnostic> {
    if let Some(modifier) = modifier {
        Err(domain_error(modifier, "no modifier"))
    } else {
        Ok(())
    }
}

fn required_theme_token(
    theme: &ThemeRegistry,
    kind: TokenKind,
    name: &str,
    span: SourceSpan,
) -> Result<SemanticValue, Diagnostic> {
    theme
        .token_by_name(kind, name)
        .map(|definition| {
            SemanticValue::Token(TokenRef {
                kind,
                id: definition.id,
            })
        })
        .ok_or_else(|| {
            diagnostic(
                DiagnosticCode::UnknownName,
                format!("unknown {kind:?} token `{name}`"),
                span,
                nearest(name, &token_names(theme, kind)),
            )
        })
}

fn token_names(theme: &ThemeRegistry, kind: TokenKind) -> Vec<&str> {
    theme
        .tokens()
        .iter()
        .filter(|token| token.kind == kind)
        .map(|token| token.name.as_str())
        .collect()
}

fn known_variants(theme: &ThemeRegistry) -> Vec<&str> {
    let mut variants = theme
        .breakpoints()
        .iter()
        .map(|breakpoint| breakpoint.name.as_str())
        .collect::<Vec<_>>();
    variants.extend([
        "dark",
        "light",
        "motion-safe",
        "motion-reduce",
        "contrast-more",
        "contrast-less",
        "hover",
        "focus",
        "focus-visible",
        "active",
        "disabled",
        "placeholder",
    ]);
    variants.extend(CLASSIFIED_SELECTOR_VARIANTS.iter().map(|(name, _)| *name));
    variants.extend(CASCADE_LAYER_VARIANTS.iter().map(|(name, _)| *name));
    variants
}

fn insert_assignment(
    output: &mut SemanticStyle,
    assignment: Assignment,
    source: SourceSpan,
) -> Result<(), Diagnostic> {
    for existing in &output.assignments {
        if existing.condition != assignment.condition
            || !existing.slots.intersects(assignment.slots)
        {
            continue;
        }
        if matches!(existing.utility, Utility::ArbitraryProperty(_))
            && matches!(assignment.utility, Utility::ArbitraryProperty(_))
            && existing.utility != assignment.utility
        {
            continue;
        }
        if existing.utility == assignment.utility
            && existing.slots == assignment.slots
            && existing.value == assignment.value
            && existing.negative == assignment.negative
            && existing.important == assignment.important
        {
            return Ok(());
        }
        if assignment.slots.is_proper_subset(existing.slots)
            || existing.slots.is_proper_subset(assignment.slots)
        {
            let (broader, narrower) = if assignment.slots.is_proper_subset(existing.slots) {
                (existing, &assignment)
            } else {
                (&assignment, existing)
            };
            if broader.important && !narrower.important {
                return Err(diagnostic(
                    DiagnosticCode::Conflict,
                    "a non-important refinement cannot override an important shorthand",
                    source,
                    Some("mark the narrower utility important or remove important from the broader utility".into()),
                ));
            }
            continue;
        }
        return Err(diagnostic(
            DiagnosticCode::Conflict,
            "utilities assign conflicting values in the same condition",
            source,
            Some("remove one utility or move it to a narrower variant".into()),
        ));
    }
    output.assignments.push(assignment);
    Ok(())
}

fn canonicalize_conditions(style: &mut SemanticStyle) {
    let old_selectors = core::mem::take(&mut style.selectors);
    style.selectors.clone_from(&old_selectors);
    style.selectors.sort();
    for condition in &mut style.conditions {
        for selector in &mut condition.selectors {
            let value = &old_selectors[selector.index()];
            let index = style
                .selectors
                .binary_search(value)
                .expect("interned selector must remain present while canonicalizing");
            *selector = SelectorId::from_index(index)
                .expect("selector table exceeded its compact ID capacity");
        }
    }

    let old_conditions = core::mem::take(&mut style.conditions);
    style.conditions.clone_from(&old_conditions);
    style.conditions.sort();
    let base_index = style
        .conditions
        .iter()
        .position(Condition::is_base)
        .expect("semantic IR must retain its base condition");
    style.conditions.swap(0, base_index);

    let condition_ids = style
        .conditions
        .iter()
        .enumerate()
        .map(|(index, condition)| {
            (
                condition,
                ConditionId::from_index(index)
                    .expect("condition table exceeded its compact ID capacity"),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    for assignment in &mut style.assignments {
        let condition = &old_conditions[assignment.condition.index()];
        assignment.condition = *condition_ids
            .get(condition)
            .expect("interned condition must remain present while canonicalizing");
    }

    let old_arbitrary_properties = core::mem::take(&mut style.arbitrary_properties);
    style
        .arbitrary_properties
        .clone_from(&old_arbitrary_properties);
    style.arbitrary_properties.sort();
    for assignment in &mut style.assignments {
        let Utility::ArbitraryProperty(id) = assignment.utility else {
            continue;
        };
        let property = &old_arbitrary_properties[id.index()];
        let index = style
            .arbitrary_properties
            .binary_search(property)
            .expect("interned arbitrary property must remain present while canonicalizing");
        assignment.utility = Utility::ArbitraryProperty(
            ArbitraryPropertyId::from_index(index)
                .expect("arbitrary-property table exceeded its compact ID capacity"),
        );
    }
}

fn canonicalize_assignments(style: &mut SemanticStyle) {
    style.assignments.sort_by_key(|assignment| {
        (
            assignment.condition,
            core::cmp::Reverse(assignment.slots.bits().count_ones()),
            assignment.slots,
            assignment.utility,
            assignment.value,
            assignment.negative,
            assignment.important,
        )
    });
}

fn known_candidates() -> Vec<&'static str> {
    let mut candidates = utility_catalog()
        .iter()
        .filter(|descriptor| descriptor.form() != UtilityForm::ArbitraryProperty)
        .map(|descriptor| descriptor.match_name)
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

fn domain_error(operand: &OperandSyntax, expected: &str) -> Diagnostic {
    diagnostic(
        DiagnosticCode::InvalidDomain,
        format!("value is outside this utility domain; expected {expected}"),
        operand.span,
        None,
    )
}

fn invalid_variant(message: impl Into<String>, span: SourceSpan) -> Diagnostic {
    diagnostic(DiagnosticCode::InvalidVariantChain, message, span, None)
}

fn diagnostic(
    code: DiagnosticCode,
    message: impl Into<String>,
    span: SourceSpan,
    suggestion: Option<String>,
) -> Diagnostic {
    Diagnostic {
        code,
        message: message.into(),
        span,
        suggestion,
    }
}

fn nearest(value: &str, candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .map(|candidate| (*candidate, edit_distance(value, candidate)))
        .min_by_key(|(_, distance)| *distance)
        .filter(|(_, distance)| *distance <= 3)
        .map(|(candidate, _)| candidate.to_owned())
}

fn edit_distance(left: &str, right: &str) -> usize {
    let mut previous: Vec<usize> = (0..=right.chars().count()).collect();
    let mut current = vec![0; previous.len()];
    for (left_index, left_character) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_character) in right.chars().enumerate() {
            current[right_index + 1] = core::cmp::min(
                core::cmp::min(current[right_index] + 1, previous[right_index + 1] + 1),
                previous[right_index] + usize::from(left_character != right_character),
            );
        }
        core::mem::swap(&mut previous, &mut current);
    }
    previous[right.chars().count()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use pliego_css_parser::parse_style_list;
    use pliego_css_theme::{BreakpointDefinition, TokenDefinition};

    fn customized_seed(accent: &str, medium: &str) -> ThemeRegistry {
        let seed = ThemeRegistry::seed();
        let mut tokens = seed.tokens().to_vec();
        let mut breakpoints = seed.breakpoints().to_vec();
        tokens
            .iter_mut()
            .find(|token| token.kind == TokenKind::Color && token.name == "accent")
            .expect("must succeed")
            .value = accent.into();
        breakpoints
            .iter_mut()
            .find(|breakpoint| breakpoint.name == "md")
            .expect("must succeed")
            .min_width = medium.into();
        ThemeRegistry::from_definitions(tokens, breakpoints).expect("must succeed")
    }

    fn lower(source: &str) -> Result<SemanticStyle, Diagnostic> {
        lower_style(&parse_style_list(source).expect("must succeed"))
    }

    fn compatibility_theme() -> ThemeRegistry {
        ThemeRegistry::from_definitions(
            [
                TokenDefinition::new(TokenKind::Color, "brand", "#36f"),
                TokenDefinition::new(TokenKind::Spacing, "gutter", "1.5rem"),
            ],
            [BreakpointDefinition::new(
                pliego_css_ir::BreakpointId::new(7),
                0,
                "tablet",
                "52rem",
            )],
        )
        .expect("must succeed")
    }

    #[test]
    fn v2_style_identity_and_class_are_frozen() {
        let theme = compatibility_theme();
        let syntax = parse_style_list("flex gap-gutter tablet:grid").expect("must succeed");
        let style = lower_style_with_theme(&theme, &syntax).expect("must succeed");

        assert_eq!(STYLE_ID_FORMAT_VERSION, 2);
        assert_eq!(pliego_css_ir::CLASS_NAME_FORMAT_VERSION, 1);
        assert_eq!(
            format!("{:032x}", theme.id().get()),
            "eda25b5ed8fa8ce973662d4f18a46bdc"
        );
        assert_eq!(
            format!("{:032x}", style.id.get()),
            "b742ceb589d4f412c6ba77e81f632f53"
        );
        assert_eq!(style.id.to_class_name(), "pc_aukxxmkm8bmauj8duf8zpjdcj");
    }

    #[test]
    fn candidate_seed_style_vector_is_explicit() {
        let theme = ThemeRegistry::seed();
        let syntax = parse_style_list("flex gap-4").expect("must succeed");
        let style = lower_style_with_theme(&theme, &syntax).expect("must succeed");

        assert_eq!(
            format!("{:032x}", theme.id().get()),
            "b98b78da29201938d135b8bb94717788"
        );
        assert_eq!(
            format!("{:032x}", style.id.get()),
            "fe3a92576be2bb53e3240249bc45b829"
        );
        assert_eq!(style.id.to_class_name(), "pc_f1u1l7d58kemkdqdjie56hdex");
    }

    #[test]
    fn catalog_patterns_and_match_surfaces_are_unique_and_sorted() {
        use std::collections::BTreeSet;

        let catalog = utility_catalog();
        assert!(!catalog.is_empty());

        let mut patterns = BTreeSet::new();
        let mut fixed_names = BTreeSet::new();
        let mut family_names = BTreeSet::new();
        for descriptor in catalog {
            assert!(patterns.insert(descriptor.pattern()), "duplicate pattern");
            assert!(!descriptor.example().is_empty());
            assert!(!descriptor.summary().is_empty());
            match descriptor.form() {
                UtilityForm::Fixed => assert!(
                    fixed_names.insert(descriptor.match_name),
                    "duplicate fixed spelling `{}`",
                    descriptor.match_name
                ),
                UtilityForm::Parameterized => assert!(
                    family_names.insert(descriptor.match_name),
                    "duplicate family prefix `{}`",
                    descriptor.match_name
                ),
                UtilityForm::ArbitraryProperty => {
                    assert!(descriptor.match_name.is_empty());
                }
            }
        }
        for pair in catalog.windows(2) {
            assert!(
                pair[0].pattern() < pair[1].pattern(),
                "catalog is not sorted: `{}` before `{}`",
                pair[0].pattern(),
                pair[1].pattern()
            );
        }
    }

    #[test]
    fn every_catalog_example_parses_lowers_and_emits_with_seed() {
        let theme = ThemeRegistry::seed();
        for descriptor in utility_catalog() {
            let syntax = parse_style_list(descriptor.example()).unwrap_or_else(|error| {
                panic!(
                    "catalog example `{}` for `{}` did not parse: {error}",
                    descriptor.example(),
                    descriptor.pattern()
                )
            });
            let style = lower_style_with_theme(&theme, &syntax).unwrap_or_else(|error| {
                panic!(
                    "catalog example `{}` for `{}` did not lower: {error}",
                    descriptor.example(),
                    descriptor.pattern()
                )
            });
            let css = emit_css_with_theme(&theme, &style).unwrap_or_else(|error| {
                panic!(
                    "catalog example `{}` for `{}` did not emit: {error}",
                    descriptor.example(),
                    descriptor.pattern()
                )
            });
            assert!(!css.is_empty(), "empty CSS for `{}`", descriptor.pattern());
        }
    }

    #[test]
    fn every_catalog_example_resolves_through_its_own_descriptor() {
        for descriptor in utility_catalog() {
            let style = parse_style_list(descriptor.example()).expect("must succeed");
            assert_eq!(style.items.len(), 1, "`{}`", descriptor.pattern());
            match (&style.items[0].candidate.kind, descriptor.form()) {
                (CandidateKind::ArbitraryProperty { .. }, UtilityForm::ArbitraryProperty) => {}
                (CandidateKind::Named(candidate), UtilityForm::Fixed) => {
                    let syntax = parse_named_candidate_at(candidate, 0).expect("must succeed");
                    let resolved = fixed_utility(&syntax.body).expect("must succeed");
                    assert_eq!(resolved, descriptor, "`{candidate}`");
                }
                (CandidateKind::Named(candidate), UtilityForm::Parameterized) => {
                    let syntax = parse_named_candidate_at(candidate, 0).expect("must succeed");
                    let (resolved, _) = resolve_family(&syntax.body).expect("must succeed");
                    assert_eq!(resolved, descriptor, "`{candidate}`");
                }
                (candidate, form) => panic!(
                    "catalog example `{}` parsed as {candidate:?}, not {form:?}",
                    descriptor.example()
                ),
            }
        }
    }

    #[test]
    fn unsupported_modifiers_fail_instead_of_being_silently_ignored() {
        for source in ["p-4/50", "opacity-50/x", "font-semibold/x"] {
            let error = lower_style(&parse_style_list(source).expect("must succeed"))
                .expect_err("must reject");
            assert_eq!(error.code, DiagnosticCode::InvalidDomain, "{source}");
            assert!(error.message.contains("no modifier"), "{source}");
        }
    }

    #[test]
    fn resolves_static_and_longest_match_families() {
        let style = lower("inline-flex items-center max-w-6xl").expect("must succeed");
        assert_eq!(style.assignments.len(), 3);
        assert!(
            style
                .assignments
                .iter()
                .any(|assignment| assignment.utility == Utility::Display)
        );
        assert!(
            style
                .assignments
                .iter()
                .any(|assignment| assignment.utility == Utility::MaxWidth)
        );
    }

    #[test]
    fn resolves_gate_a_layout_primitives() {
        let style = lower("flex-1 min-w-0 pb-4 border-b tracking-tight cursor-pointer justify-end")
            .expect("must succeed");
        assert_eq!(style.assignments.len(), 9);
        assert!(
            style
                .assignments
                .iter()
                .any(|assignment| assignment.utility == Utility::FlexBasis)
        );
        assert!(
            style
                .assignments
                .iter()
                .any(|assignment| assignment.utility == Utility::LetterSpacing)
        );
    }

    #[test]
    fn resolves_complete_core_fixture_primitives() {
        let style = lower("antialiased transition-colors aspect-square outline-none resize-y")
            .expect("must succeed");
        assert_eq!(style.assignments.len(), 5);
    }

    #[test]
    fn resolves_full_fixture_outline_and_placeholder_contracts() {
        let style = lower(
            "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent placeholder:text-muted",
        )
        .expect("must succeed");
        assert_eq!(style.assignments.len(), 4);
        assert!(
            style
                .selectors
                .iter()
                .any(|selector| selector == "&::placeholder")
        );
    }

    #[test]
    fn rejects_unknown_utility_with_suggestion() {
        let error = lower("flec").expect_err("must reject");
        assert_eq!(error.code, DiagnosticCode::UnknownUtility);
        assert_eq!(error.suggestion.as_deref(), Some("flex"));
    }

    #[test]
    fn rejects_cross_domain_token() {
        let error = lower("p-accent").expect_err("must reject");
        assert_eq!(error.code, DiagnosticCode::InvalidDomain);
    }

    #[test]
    fn detects_same_condition_conflict() {
        let error = lower("flex grid").expect_err("must reject");
        assert_eq!(error.code, DiagnosticCode::Conflict);
    }

    #[test]
    fn allows_order_independent_footprint_refinement() {
        let left = lower("p-4 px-2").expect("must succeed");
        let right = lower("px-2 p-4").expect("must succeed");
        let semantics = |style: &SemanticStyle| {
            style
                .assignments
                .iter()
                .map(|assignment| {
                    (
                        assignment.utility,
                        assignment.slots,
                        assignment.value,
                        assignment.condition,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(semantics(&left), semantics(&right));
        assert_eq!(left.id, right.id);
    }

    #[test]
    fn canonicalizes_commuting_variant_order() {
        let left = lower("md:hover:bg-accent").expect("must succeed");
        let right = lower("hover:md:bg-accent").expect("must succeed");
        assert_eq!(left.conditions, right.conditions);
        assert_eq!(left.assignments, right.assignments);
        assert_eq!(left.id, right.id);
    }

    #[test]
    fn lowers_arbitrary_selectors_and_preserves_transform_order() {
        let forward = lower("[&>svg]:[& path]:block").expect("must succeed");
        let reverse = lower("[& path]:[&>svg]:block").expect("must succeed");

        assert_eq!(forward.selectors, ["& path", "&>svg"]);
        let chain = forward.conditions[1]
            .selectors
            .iter()
            .map(|id| forward.selectors[id.index()].as_str())
            .collect::<Vec<_>>();
        assert_eq!(chain, ["&>svg", "& path"]);
        assert_ne!(forward.id, reverse.id);
    }

    #[test]
    fn arbitrary_selectors_compose_with_commuting_dimensions() {
        let left = lower("[&[data-state=open]]:md:hover:block").expect("must succeed");
        let right = lower("hover:md:[&[data-state=open]]:block").expect("must succeed");

        assert_eq!(left.conditions, right.conditions);
        assert_eq!(left.assignments, right.assignments);
        assert_eq!(left.id, right.id);
    }

    #[test]
    fn typed_attribute_and_direction_variants_are_canonical_native_selectors() {
        let typed = lower("rtl:aria-expanded:data-state-open:hover:block").expect("must succeed");
        let reordered =
            lower("hover:rtl:aria-expanded:data-state-open:block").expect("must succeed");
        assert_eq!(typed, reordered);

        let css = emit_css(&typed).expect("must succeed");
        assert!(css.contains(":hover:dir(rtl)[aria-expanded=true][data-state=open]"));

        let arbitrary = lower("[&[aria-expanded=true]]:block").expect("must succeed");
        let named = lower("aria-expanded:block").expect("must succeed");
        assert_eq!(named.id, arbitrary.id);
        assert_eq!(emit_css(&named), emit_css(&arbitrary));
        assert!(is_classified_selector_transform("&[aria-expanded=true]"));
        assert!(!is_classified_selector_transform("&>svg"));
    }

    #[test]
    fn typed_selector_families_reject_duplicates_and_contradictions() {
        for source in [
            "rtl:ltr:block",
            "data-state-open:data-state-closed:block",
            "aria-expanded:aria-expanded:block",
        ] {
            let error = lower(source).expect_err("must reject");
            assert_eq!(error.code, DiagnosticCode::InvalidVariantChain, "{source}");
        }
    }

    #[test]
    fn canonicalizes_selector_and_condition_tables_across_item_order() {
        let left = lower("[&>svg]:block [&>path]:hidden").expect("must succeed");
        let right = lower("[&>path]:hidden [&>svg]:block").expect("must succeed");

        assert_eq!(left.selectors, right.selectors);
        assert_eq!(left.conditions, right.conditions);
        assert_eq!(left.id, right.id);
    }

    #[test]
    fn rejects_unsafe_arbitrary_selector_payloads() {
        for source in [
            "[&@media]:block",
            "[&{color:red}]:block",
            "[&,body]:block",
            "[&/*comment*/>svg]:block",
            "[&<style]:block",
        ] {
            let error = lower(source).expect_err("must reject");
            assert_eq!(error.code, DiagnosticCode::InvalidVariantChain, "{source}");
        }
    }

    #[test]
    fn distinguishes_text_domains_and_color_alpha() {
        let style = lower("text-sm text-ink bg-accent/20").expect("must succeed");
        assert_eq!(style.assignments.len(), 3);
        assert!(
            style
                .assignments
                .iter()
                .any(|assignment| assignment.utility == Utility::FontSize)
        );
        assert!(
            style
                .assignments
                .iter()
                .any(|assignment| assignment.utility == Utility::TextColor)
        );
    }

    #[test]
    fn rejects_negative_non_negatable_family() {
        let error = lower("-p-4").expect_err("must reject");
        assert_eq!(error.code, DiagnosticCode::InvalidNegative);
    }

    #[test]
    fn rejects_negative_keyword_value() {
        let error = lower("-m-auto").expect_err("must reject");
        assert_eq!(error.code, DiagnosticCode::InvalidNegative);
    }

    #[test]
    fn rejects_refinement_blocked_by_important_shorthand() {
        let error = lower("p-4! px-2").expect_err("must reject");
        assert_eq!(error.code, DiagnosticCode::Conflict);

        lower("p-4 px-2!").expect("must succeed");
        lower("p-4! px-2!").expect("must succeed");
    }

    #[test]
    fn deduplicates_exact_assignments() {
        let style = lower("flex flex").expect("must succeed");
        assert_eq!(style.assignments.len(), 1);
    }

    #[test]
    fn distinct_arbitrary_properties_coexist_deterministically() {
        let left = lower("[text-wrap:balance] [mask-type:luminance]").expect("must succeed");
        let right = lower("[mask-type:luminance] [text-wrap:balance]").expect("must succeed");

        assert_eq!(left.arbitrary_properties, right.arbitrary_properties);
        assert_eq!(left.id, right.id);
        let left_css = emit_css(&left).expect("must succeed");
        let right_css = emit_css(&right).expect("must succeed");
        assert_eq!(left_css, right_css);
        assert!(left_css.contains("mask-type:luminance;text-wrap:balance;"));

        let duplicate = lower("[mask-type:luminance] [mask-type:luminance]").expect("must succeed");
        assert_eq!(duplicate.assignments.len(), 1);
        assert_eq!(
            lower("[mask-type:alpha] [mask-type:luminance]")
                .expect_err("must reject")
                .code,
            DiagnosticCode::Conflict
        );
    }

    #[test]
    fn style_id_ignores_utility_order_and_source_spans() {
        let left = lower("p-4 px-2 flex").expect("must succeed");
        let right = lower("flex px-2 p-4").expect("must succeed");
        assert!(!left.id.is_unresolved());
        assert_eq!(left.id, right.id);
    }

    #[test]
    fn explicit_seed_wrappers_preserve_lowering_and_emission_contracts() {
        let syntax = parse_style_list("md:hover:bg-accent p-4").expect("must succeed");
        let implicit = lower_style(&syntax).expect("must succeed");
        let explicit = lower_style_with_theme(seed_theme(), &syntax).expect("must succeed");

        assert_eq!(implicit, explicit);
        assert_eq!(
            emit_css(&implicit),
            emit_css_with_theme(seed_theme(), &explicit)
        );
        assert_eq!(emit_seed_theme(), emit_theme(seed_theme()));
    }

    #[test]
    fn theme_overrides_drive_token_values_and_breakpoints() {
        let theme = customized_seed("oklch(70% .2 40)", "52rem");
        let syntax = parse_style_list("md:bg-accent p-4").expect("must succeed");
        let style = lower_style_with_theme(&theme, &syntax).expect("must succeed");
        let css = emit_css_with_theme(&theme, &style).expect("must succeed");

        assert!(css.contains("@media (min-width:52rem){"));
        assert!(css.contains("background-color:var(--color-accent);"));
        assert!(emit_theme(&theme).contains("--color-accent:oklch(70% .2 40);"));
    }

    #[test]
    fn used_theme_emits_only_variable_backed_retained_tokens() {
        let theme = customized_seed("oklch(70% .2 40)", "52rem");
        let syntax = parse_style_list("bg-accent p-4").expect("must succeed");
        let style = lower_style_with_theme(&theme, &syntax).expect("must succeed");
        let css = emit_used_theme(&theme, [&style]);

        assert!(css.contains("--color-accent:oklch(70% .2 40);"));
        assert!(!css.contains("--color-surface:"));
        assert!(!css.contains("--font-"));
        assert!(!css.contains("--spacing-"));
    }

    #[test]
    fn used_theme_preserves_an_empty_root_when_no_variable_is_required() {
        let style = lower("flex p-4").expect("must succeed");
        assert_eq!(emit_used_theme(seed_theme(), [&style]), ":root{}");
    }

    #[test]
    fn numeric_border_widths_do_not_alias_the_spacing_scale() {
        let seed = ThemeRegistry::seed();
        let mut tokens = seed.tokens().to_vec();
        let spacing = tokens
            .iter_mut()
            .find(|token| token.kind == TokenKind::Spacing && token.name == "1")
            .expect("must succeed");
        *spacing =
            pliego_css_theme::TokenDefinition::with_id(TokenKind::Spacing, spacing.id, "1", "2rem");
        let theme = ThemeRegistry::from_definitions(tokens, seed.breakpoints().to_vec())
            .expect("must succeed");
        let syntax = parse_style_list("border p-1 ring-2").expect("must succeed");
        let style = lower_style_with_theme(&theme, &syntax).expect("must succeed");
        let css = emit_css_with_theme(&theme, &style).expect("must succeed");

        assert!(css.contains("border-width:1px;"));
        assert!(css.contains("padding:2rem;"));
        assert!(css.contains("--pc-ring-width:2px;"));
    }

    #[test]
    fn registry_additions_extend_the_semantic_catalog() {
        let seed = ThemeRegistry::seed();
        let mut tokens = seed.tokens().to_vec();
        tokens.push(pliego_css_theme::TokenDefinition::new(
            TokenKind::Spacing,
            "gutter",
            "1.375rem",
        ));
        let theme = ThemeRegistry::from_definitions(tokens, seed.breakpoints().to_vec())
            .expect("must succeed");
        let syntax = parse_style_list("p-gutter").expect("must succeed");
        let style = lower_style_with_theme(&theme, &syntax).expect("must succeed");

        assert!(
            emit_css_with_theme(&theme, &style)
                .expect("must succeed")
                .contains("padding:1.375rem;")
        );
    }

    #[test]
    fn theme_identity_partitions_style_and_composition_ids() {
        let custom = customized_seed("rebeccapurple", "52rem");
        let syntax = parse_style_list("p-4 bg-accent").expect("must succeed");
        let seed_style = lower_style(&syntax).expect("must succeed");
        let custom_style = lower_style_with_theme(&custom, &syntax).expect("must succeed");
        assert_ne!(seed_style.id, custom_style.id);
        assert!(matches!(
            emit_css_with_theme(&custom, &seed_style),
            Err(EmitError::StyleIdentityMismatch { .. })
        ));

        let base_syntax = parse_style_list("p-4").expect("must succeed");
        let branch_syntax = parse_style_list("px-2").expect("must succeed");
        let custom_composed = compose_style_override_with_theme(
            &custom,
            lower_style_with_theme(&custom, &base_syntax).expect("must succeed"),
            lower_style_with_theme(&custom, &branch_syntax).expect("must succeed"),
        );
        let seed_composed = compose_style_override(
            lower_style(&base_syntax).expect("must succeed"),
            lower_style(&branch_syntax).expect("must succeed"),
        );
        assert_ne!(custom_composed.id, seed_composed.id);
    }

    #[test]
    fn fallible_composition_rejects_invalid_branch_ir_without_panicking() {
        let base = lower("block").expect("must succeed");
        let mut branch = lower("hover:opacity-50").expect("must succeed");
        branch.conditions[1].selectors = vec![SelectorId::new(u32::MAX)];

        let error = try_compose_style_override_with_theme(seed_theme(), base, branch)
            .expect_err("invalid semantic IR must be rejected");

        assert_eq!(
            error,
            pliego_css_ir::InvariantError {
                violation: pliego_css_ir::InvariantViolation::SelectorOutOfBounds,
                location: pliego_css_ir::InvariantLocation::Condition(1),
            }
        );
    }

    #[test]
    fn fallible_composition_rejects_invalid_base_ir_without_panicking() {
        let mut base = lower("block").expect("must succeed");
        base.conditions.clear();
        let branch = lower("opacity-50").expect("must succeed");

        let error = try_compose_style_override(base, branch)
            .expect_err("invalid semantic IR must be rejected");

        assert_eq!(
            error,
            pliego_css_ir::InvariantError {
                violation: pliego_css_ir::InvariantViolation::MissingBaseCondition,
                location: pliego_css_ir::InvariantLocation::Style,
            }
        );
    }

    #[test]
    fn composition_overrides_only_overlapping_base_slots() {
        let composed = compose_style_override(
            lower("p-4 bg-surface").expect("must succeed"),
            lower("px-2 bg-accent").expect("must succeed"),
        );
        let css = emit_css(&composed).expect("must succeed");
        assert!(css.contains("padding-top:1rem;"));
        assert!(css.contains("padding-bottom:1rem;"));
        assert!(css.contains("padding-right:.5rem;"));
        assert!(css.contains("padding-left:.5rem;"));
        assert!(css.contains("background-color:var(--color-accent);"));
        assert!(!css.contains("background-color:var(--color-surface);"));
    }

    #[test]
    fn composition_overrides_only_the_same_arbitrary_property() {
        let composed = compose_style_override(
            lower("[mask-type:alpha] [text-wrap:balance]").expect("must succeed"),
            lower("[mask-type:luminance]").expect("must succeed"),
        );
        let css = emit_css(&composed).expect("must succeed");

        assert!(css.contains("mask-type:luminance;"));
        assert!(css.contains("text-wrap:balance;"));
        assert!(!css.contains("mask-type:alpha;"));
    }

    #[test]
    fn composition_identity_is_deterministic_across_authoring_order() {
        let left = compose_style_override(
            lower("p-4 bg-surface").expect("must succeed"),
            lower("px-2 bg-accent").expect("must succeed"),
        );
        let right = compose_style_override(
            lower("bg-surface p-4").expect("must succeed"),
            lower("bg-accent px-2").expect("must succeed"),
        );
        assert_eq!(left.id, right.id);
        assert_eq!(emit_css(&left), emit_css(&right));
    }

    #[test]
    fn cross_clause_analysis_reports_deterministic_condition_slot_overlap() {
        let clauses = vec![
            vec![
                lower("opacity-50").expect("must succeed"),
                lower("opacity-100").expect("must succeed"),
            ],
            vec![lower("opacity-100").expect("must succeed")],
        ];

        assert_eq!(
            analyze_cross_clause_conflicts(&clauses),
            Err(CrossClauseConflict {
                left_clause: 0,
                left_branch: 0,
                right_clause: 1,
                right_branch: 0,
                slots: SlotSet::from_slot(Slot::Opacity),
            })
        );
    }

    #[test]
    fn cross_clause_analysis_compares_resolved_conditions_not_local_ids() {
        let different_selectors = vec![
            vec![lower("[&>.first]:opacity-50").expect("must succeed")],
            vec![lower("[&>.second]:opacity-100").expect("must succeed")],
        ];
        analyze_cross_clause_conflicts(&different_selectors).expect("must succeed");

        let same_selector = vec![
            vec![lower("[&>.same]:opacity-50").expect("must succeed")],
            vec![lower("[&>.same]:opacity-100").expect("must succeed")],
        ];
        assert!(analyze_cross_clause_conflicts(&same_selector).is_err());

        let different_states = vec![
            vec![lower("hover:opacity-50").expect("must succeed")],
            vec![lower("focus:opacity-100").expect("must succeed")],
        ];
        analyze_cross_clause_conflicts(&different_states).expect("must succeed");
    }

    #[test]
    fn cross_clause_analysis_resolves_arbitrary_property_names() {
        let distinct = vec![
            vec![lower("[mask-type:luminance]").expect("must succeed")],
            vec![lower("[text-wrap:balance]").expect("must succeed")],
        ];
        analyze_cross_clause_conflicts(&distinct).expect("must succeed");

        let same = vec![
            vec![lower("[mask-type:alpha]").expect("must succeed")],
            vec![lower("[mask-type:luminance]").expect("must succeed")],
        ];
        assert!(analyze_cross_clause_conflicts(&same).is_err());
    }

    #[test]
    fn rejects_repeated_condition_dimensions() {
        let error = lower("md:lg:flex").expect_err("must reject");
        assert_eq!(error.code, DiagnosticCode::InvalidVariantChain);
    }

    #[test]
    fn decoded_selector_validator_rejects_unbalanced_and_unsafe_syntax() {
        for selector in ["&>p", "&[data-kind='a,b']", "&:not([hidden])"] {
            validate_selector_transform_text(selector).expect("must succeed");
        }
        for selector in [
            "p",
            "&&",
            ":not(&)",
            ":is(&,body)",
            "&([)]",
            "&)",
            "&[data-x",
            "&,p",
            "&/*x*/p",
            "&*/p",
            "&@media",
            "&[data-x='bad\nstring']",
        ] {
            assert!(
                validate_selector_transform_text(selector).is_err(),
                "unsafe decoded selector must fail: {selector:?}"
            );
        }
        assert!(lower("[&&]:block").is_err());
        assert!(parse_style_list("[:is(&,body)]:block").is_err());
    }

    #[test]
    fn seed_catalog_token_ids_do_not_collide_within_a_namespace() {
        let theme = ThemeRegistry::seed();
        let mut ids = std::collections::BTreeMap::new();
        for token in theme.tokens() {
            let previous = ids.insert((token.kind, token.id), token.name.as_str());
            assert!(
                previous.is_none(),
                "{:?} token `{}` collides with `{previous:?}`",
                token.kind,
                token.name
            );
        }
    }
}
