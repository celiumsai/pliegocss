//! Semantic intermediate representation for `PliegoCSS`.
//!
//! This is an exact-version implementation crate. Its mutable AST and semantic model are not part
//! of the minimal application API; applications should use the `pliego-css` facade.

#![forbid(unsafe_code)]

use core::fmt;
use std::borrow::Cow;
use std::collections::BTreeSet;

/// A half-open byte range inside the string literal passed to a style macro.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct SourceSpan {
    /// First byte included in the span.
    pub start: usize,
    /// First byte after the span.
    pub end: usize,
}

impl SourceSpan {
    /// Creates a new half-open byte range.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns the number of bytes covered by the span.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Returns `true` when the span contains no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

/// A portable half-open source range stored in semantic IR.
///
/// Syntax parsing uses [`SourceSpan`] because it indexes an in-memory Rust
/// string. Semantic IR uses this fixed-width representation so serialized
/// manifests have the same layout on every target.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Span {
    /// First byte included in the span.
    pub start: u32,
    /// First byte after the span.
    pub end: u32,
}

impl Span {
    /// Creates a new half-open byte range.
    #[must_use]
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Returns the number of bytes covered by the span.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Returns `true` when the span contains no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

/// Error returned when a syntax span cannot fit in portable semantic IR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpanOverflow;

impl fmt::Display for SpanOverflow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("source span exceeds the 32-bit semantic IR limit")
    }
}

impl std::error::Error for SpanOverflow {}

impl TryFrom<SourceSpan> for Span {
    type Error = SpanOverflow;

    fn try_from(span: SourceSpan) -> Result<Self, Self::Error> {
        Ok(Self {
            start: u32::try_from(span.start).map_err(|_| SpanOverflow)?,
            end: u32::try_from(span.end).map_err(|_| SpanOverflow)?,
        })
    }
}

impl From<Span> for SourceSpan {
    fn from(span: Span) -> Self {
        Self {
            start: usize::try_from(span.start).expect("u32 must fit in usize on supported targets"),
            end: usize::try_from(span.end).expect("u32 must fit in usize on supported targets"),
        }
    }
}

/// The syntax-level result of parsing one `pc!` literal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleList {
    /// Utilities in authoring order.
    pub items: Vec<StyleItem>,
}

/// One utility and its condition prefixes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleItem {
    /// Condition or selector prefixes applied to the utility.
    pub variants: Vec<Variant>,
    /// Whether the utility uses its canonical negative form.
    pub negative: bool,
    /// The utility candidate.
    pub candidate: Candidate,
    /// Whether the declaration must interoperate using `!important`.
    pub important: bool,
    /// Span covering the complete item.
    pub span: SourceSpan,
}

/// A condition or ordered selector transformation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variant {
    /// Parsed variant representation.
    pub kind: VariantKind,
    /// Span covering the variant without its trailing colon.
    pub span: SourceSpan,
}

/// Syntax-level variant categories.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantKind {
    /// A catalog name such as `hover` or `md`.
    Named(String),
    /// A reserved selector transformation such as `[&>p]`.
    ArbitrarySelector(String),
}

/// A utility candidate before catalog resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    /// Parsed candidate representation.
    pub kind: CandidateKind,
    /// Span covering the candidate without the negative or important markers.
    pub span: SourceSpan,
}

/// Syntax-level candidate categories.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CandidateKind {
    /// A catalog candidate such as `flex` or `bg-[oklch(...)]`.
    Named(String),
    /// A single arbitrary CSS declaration.
    ArbitraryProperty {
        /// CSS property name.
        property: String,
        /// CSS value text.
        value: String,
    },
}

/// Stable diagnostic codes emitted by the syntax parser.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DiagnosticCode {
    /// Unknown utility.
    UnknownUtility,
    /// Malformed item or empty variant.
    MalformedItem,
    /// Unknown variant or token.
    UnknownName,
    /// Value outside a utility domain.
    InvalidDomain,
    /// Semantic conflict.
    Conflict,
    /// Unsupported or non-canonical negative form.
    InvalidNegative,
    /// Unbalanced delimiter, quote, or escape.
    UnbalancedDelimiter,
    /// Invalid arbitrary CSS value.
    InvalidArbitraryValue,
    /// Misplaced or duplicated important marker.
    InvalidImportant,
    /// Ambiguous namespace.
    AmbiguousNamespace,
    /// Arbitrary property contains more than one declaration or a block.
    InvalidArbitraryProperty,
    /// Redundant or impossible variant chain.
    InvalidVariantChain,
}

impl DiagnosticCode {
    /// Returns the documented external code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownUtility => "PCS001",
            Self::MalformedItem => "PCS002",
            Self::UnknownName => "PCS003",
            Self::InvalidDomain => "PCS004",
            Self::Conflict => "PCS005",
            Self::InvalidNegative => "PCS006",
            Self::UnbalancedDelimiter => "PCS007",
            Self::InvalidArbitraryValue => "PCS008",
            Self::InvalidImportant => "PCS009",
            Self::AmbiguousNamespace => "PCS010",
            Self::InvalidArbitraryProperty => "PCS011",
            Self::InvalidVariantChain => "PCS012",
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A parser or semantic diagnostic with a source span and optional fix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    /// Stable machine-readable code.
    pub code: DiagnosticCode,
    /// Human-readable explanation.
    pub message: String,
    /// Most relevant source range.
    pub span: SourceSpan,
    /// Optional replacement or guidance.
    pub suggestion: Option<String>,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Diagnostic {}

macro_rules! index_id {
    ($name:ident, $storage:ty, $documentation:literal) => {
        #[doc = $documentation]
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name($storage);

        impl $name {
            /// Creates an ID from its compact representation.
            #[must_use]
            pub const fn new(value: $storage) -> Self {
                Self(value)
            }

            /// Returns the compact representation.
            #[must_use]
            pub const fn get(self) -> $storage {
                self.0
            }

            /// Creates an ID usable for the given table index.
            #[must_use]
            pub fn from_index(index: usize) -> Option<Self> {
                <$storage>::try_from(index).ok().map(Self)
            }

            /// Returns the table index represented by this ID.
            #[must_use]
            pub fn index(self) -> usize {
                usize::try_from(self.0).expect("compact ID must fit in usize on supported targets")
            }
        }
    };
}

index_id!(
    ConditionId,
    u16,
    "An index into a semantic style's canonical condition table."
);
index_id!(
    BreakpointId,
    u16,
    "A stable identifier supplied by the active breakpoint catalog."
);
index_id!(
    TokenId,
    u32,
    "A stable identifier supplied by the active design-token catalog."
);
index_id!(
    ArbitraryValueId,
    u32,
    "An index into a semantic style's arbitrary-value table."
);
index_id!(
    CustomPropertyId,
    u32,
    "An index into a semantic style's custom-property table."
);
index_id!(
    SelectorId,
    u32,
    "An index into a semantic style's ordered selector table."
);
index_id!(
    ArbitraryPropertyId,
    u32,
    "An index into a semantic style's arbitrary-property table."
);

/// Version of the lowercase base-36 CSS class encoding for [`StyleId`].
pub const CLASS_NAME_FORMAT_VERSION: u16 = 1;

/// A stable identifier derived from normalized style semantics.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct StyleId(u128);

impl StyleId {
    /// The reserved ID used while a style is being normalized.
    pub const UNRESOLVED: Self = Self(0);

    /// Creates an ID from its deterministic 128-bit representation.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the deterministic 128-bit representation.
    #[must_use]
    pub const fn get(self) -> u128 {
        self.0
    }

    /// Returns whether this ID still needs to be derived by normalization.
    #[must_use]
    pub const fn is_unresolved(self) -> bool {
        self.0 == 0
    }

    /// Encodes all 128 identity bits as a lowercase CSS-safe class name.
    #[must_use]
    pub fn to_class_name(self) -> String {
        let mut value = self.get();
        if value == 0 {
            return "pc_0".into();
        }
        let mut encoded = [0_u8; 25];
        let mut cursor = encoded.len();
        while value > 0 {
            cursor -= 1;
            let digit = (value % 36) as u8;
            encoded[cursor] = if digit < 10 {
                b'0' + digit
            } else {
                b'a' + digit - 10
            };
            value /= 36;
        }
        let body: String = encoded[cursor..].iter().copied().map(char::from).collect();
        format!("pc_{body}")
    }
}

/// Theme dimension of a canonical condition.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ThemeMode {
    /// Applies in every theme.
    #[default]
    Any,
    /// Applies only in a light theme.
    Light,
    /// Applies only in a dark theme.
    Dark,
}

/// Reduced-motion dimension of a canonical condition.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MotionPreference {
    /// Applies regardless of the user preference.
    #[default]
    Any,
    /// Applies when motion is permitted.
    Safe,
    /// Applies when reduced motion is requested.
    Reduce,
}

/// Contrast dimension of a canonical condition.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContrastPreference {
    /// Applies regardless of the user preference.
    #[default]
    Any,
    /// Applies when more contrast is requested.
    More,
    /// Applies when less contrast is requested.
    Less,
}

/// A pseudo-state represented in a canonical condition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum PseudoState {
    /// Pointer hover state.
    Hover,
    /// Focus state.
    Focus,
    /// Keyboard-oriented visible focus state.
    FocusVisible,
    /// Active or pressed state.
    Active,
    /// Disabled state.
    Disabled,
}

/// A compact set of pseudo-states.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct StateSet(u16);

impl StateSet {
    /// The empty state set.
    pub const EMPTY: Self = Self(0);

    const KNOWN_BITS: u16 = (1 << 5) - 1;

    /// Creates a set containing one state.
    #[must_use]
    pub const fn from_state(state: PseudoState) -> Self {
        Self(1 << state as u8)
    }

    /// Creates a set when every bit represents a known state.
    #[must_use]
    pub const fn from_bits(bits: u16) -> Option<Self> {
        if bits & !Self::KNOWN_BITS == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

    /// Returns the compact bit representation.
    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Returns a copy with `state` inserted.
    #[must_use]
    pub const fn with(self, state: PseudoState) -> Self {
        Self(self.0 | Self::from_state(state).0)
    }

    /// Returns whether the set contains `state`.
    #[must_use]
    pub const fn contains(self, state: PseudoState) -> bool {
        self.0 & Self::from_state(state).0 != 0
    }

    /// Returns whether this set contains every state in `other`.
    #[must_use]
    pub const fn is_superset(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns `true` when the set is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// A canonical, order-independent condition.
///
/// Structured dimensions commute. Ordered selector transformations are kept
/// as IDs in source order because selector composition does not commute.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Condition {
    /// Optional minimum-width breakpoint.
    pub breakpoint: Option<BreakpointId>,
    /// Optional minimum-width container breakpoint.
    pub container_breakpoint: Option<BreakpointId>,
    /// Explicit cascade layer; unlayered preserves the historical host-owned boundary.
    pub layer: CascadeLayer,
    /// Theme dimension.
    pub theme: ThemeMode,
    /// Reduced-motion dimension.
    pub motion: MotionPreference,
    /// Contrast dimension.
    pub contrast: ContrastPreference,
    /// Pseudo-states, represented independently of authoring order.
    pub states: StateSet,
    /// Ordered selector transformations.
    pub selectors: Vec<SelectorId>,
}

/// Compiler-owned cascade layer with a fixed low-to-high precedence order.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CascadeLayer {
    /// Native unlayered author CSS, which outranks normal layered author CSS.
    #[default]
    Unlayered,
    /// Low-precedence application foundations; `PliegoCSS` emits no implicit reset here.
    Base,
    /// Reusable component defaults.
    Components,
    /// Utility-level declarations.
    Utilities,
    /// Explicit high-precedence layered overrides.
    Overrides,
}

impl CascadeLayer {
    /// Returns the native namespaced CSS layer name, if this is a layered value.
    #[must_use]
    pub const fn css_name(self) -> Option<&'static str> {
        match self {
            Self::Unlayered => None,
            Self::Base => Some("pliego.base"),
            Self::Components => Some("pliego.components"),
            Self::Utilities => Some("pliego.utilities"),
            Self::Overrides => Some("pliego.overrides"),
        }
    }
}

/// Canonical low-to-high CSS order statement for every compiler-owned layer.
pub const CASCADE_LAYER_ORDER_CSS: &str =
    "@layer pliego.base,pliego.components,pliego.utilities,pliego.overrides;";

/// Named authoring variants for compiler-owned cascade layers.
pub const CASCADE_LAYER_VARIANTS: &[(&str, CascadeLayer)] = &[
    ("layer-base", CascadeLayer::Base),
    ("layer-components", CascadeLayer::Components),
    ("layer-overrides", CascadeLayer::Overrides),
    ("layer-utilities", CascadeLayer::Utilities),
];

/// Resolves a compiler-owned layer variant.
#[must_use]
pub fn cascade_layer_for_variant(name: &str) -> Option<CascadeLayer> {
    CASCADE_LAYER_VARIANTS
        .iter()
        .find_map(|(candidate, layer)| (*candidate == name).then_some(*layer))
}

/// Compiler-owned named variants and their canonical native selector transforms.
///
/// This table is semantic IR vocabulary shared by lowering and strict compatibility checks. Its
/// order is canonical and additions require compatibility-policy review.
pub const CLASSIFIED_SELECTOR_VARIANTS: &[(&str, &str)] = &[
    ("aria-busy", "&[aria-busy=true]"),
    ("aria-checked", "&[aria-checked=true]"),
    ("aria-disabled", "&[aria-disabled=true]"),
    ("aria-expanded", "&[aria-expanded=true]"),
    ("aria-hidden", "&[aria-hidden=true]"),
    ("aria-invalid", "&[aria-invalid=true]"),
    ("aria-pressed", "&[aria-pressed=true]"),
    ("aria-readonly", "&[aria-readonly=true]"),
    ("aria-required", "&[aria-required=true]"),
    ("aria-selected", "&[aria-selected=true]"),
    ("data-active", "&[data-active]"),
    ("data-checked", "&[data-checked]"),
    ("data-disabled", "&[data-disabled]"),
    ("data-loading", "&[data-loading]"),
    ("data-selected", "&[data-selected]"),
    ("data-state-active", "&[data-state=active]"),
    ("data-state-checked", "&[data-state=checked]"),
    ("data-state-closed", "&[data-state=closed]"),
    ("data-state-inactive", "&[data-state=inactive]"),
    ("data-state-open", "&[data-state=open]"),
    ("data-state-unchecked", "&[data-state=unchecked]"),
    ("ltr", "&:dir(ltr)"),
    ("rtl", "&:dir(rtl)"),
];

/// Resolves a compiler-owned named variant to its canonical native selector transform.
///
/// In addition to the frozen short names, this accepts canonical `aria-[name=value]` and
/// `data-[name]` / `data-[name=value]` spellings. Attribute fragments are lowercase ASCII,
/// hyphen-separated, at most 64 bytes, and values use the same bounded identifier grammar.
#[must_use]
pub fn classified_selector_for_variant(name: &str) -> Option<Cow<'static, str>> {
    if let Some(selector) = CLASSIFIED_SELECTOR_VARIANTS
        .iter()
        .find_map(|(candidate, selector)| (*candidate == name).then_some(*selector))
    {
        return Some(Cow::Borrowed(selector));
    }
    let (namespace, body) = if let Some(body) = name
        .strip_prefix("aria-[")
        .and_then(|body| body.strip_suffix(']'))
    {
        ("aria", body)
    } else if let Some(body) = name
        .strip_prefix("data-[")
        .and_then(|body| body.strip_suffix(']'))
    {
        ("data", body)
    } else {
        return None;
    };
    let (attribute, value) = match body.split_once('=') {
        Some((attribute, value)) => {
            if value.contains('=') || !is_attribute_fragment(value) {
                return None;
            }
            (attribute, Some(value))
        }
        None if namespace == "data" => (body, None),
        None => return None,
    };
    if !is_attribute_fragment(attribute) {
        return None;
    }
    Some(Cow::Owned(value.map_or_else(
        || format!("&[{namespace}-{attribute}]"),
        |value| format!("&[{namespace}-{attribute}={value}]"),
    )))
}

/// Returns whether a name is attempting the typed configurable attribute syntax.
#[must_use]
pub fn is_typed_attribute_variant(name: &str) -> bool {
    name.starts_with("aria-[") || name.starts_with("data-[")
}

fn is_attribute_fragment(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_lowercase()
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && !bytes.windows(2).any(|pair| pair == b"--")
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

/// Returns the canonical ARIA/data attribute name owned by a classified selector transform.
#[must_use]
pub fn classified_attribute_name(selector: &str) -> Option<&str> {
    let body = selector.strip_prefix("&[")?.strip_suffix(']')?;
    let name = body.split_once('=').map_or(body, |(name, _)| name);
    let (namespace, fragment) = name.split_once('-')?;
    if !matches!(namespace, "aria" | "data") || !is_attribute_fragment(fragment) {
        return None;
    }
    let value = body.split_once('=').map(|(_, value)| value);
    if namespace == "aria" && value.is_none() {
        return None;
    }
    if value.is_some_and(|value| !is_attribute_fragment(value)) {
        return None;
    }
    Some(name)
}

/// Returns whether a selector transform has a compiler-owned semantic classification.
#[must_use]
pub fn is_classified_selector_transform(selector: &str) -> bool {
    selector == "&::placeholder"
        || classified_attribute_name(selector).is_some()
        || CLASSIFIED_SELECTOR_VARIANTS
            .iter()
            .any(|(_, classified)| selector == *classified)
}

impl Condition {
    /// The unconditional base context.
    pub const BASE: Self = Self {
        breakpoint: None,
        container_breakpoint: None,
        layer: CascadeLayer::Unlayered,
        theme: ThemeMode::Any,
        motion: MotionPreference::Any,
        contrast: ContrastPreference::Any,
        states: StateSet::EMPTY,
        selectors: Vec::new(),
    };

    /// Returns whether this is the unconditional base context.
    #[must_use]
    pub fn is_base(&self) -> bool {
        self == &Self::BASE
    }

    /// Returns whether this condition is at least as constrained as `other`.
    ///
    /// Distinct viewport or container breakpoint IDs are deliberately incomparable here because
    /// their numeric widths live in the theme catalog, not in semantic IR.
    #[must_use]
    pub fn refines(&self, other: &Self) -> bool {
        dimension_refines(self.breakpoint, other.breakpoint)
            && dimension_refines(self.container_breakpoint, other.container_breakpoint)
            && self.layer == other.layer
            && enum_dimension_refines(self.theme, other.theme, ThemeMode::Any)
            && enum_dimension_refines(self.motion, other.motion, MotionPreference::Any)
            && enum_dimension_refines(self.contrast, other.contrast, ContrastPreference::Any)
            && self.states.is_superset(other.states)
            && selector_chain_refines(&self.selectors, &other.selectors)
    }
}

fn dimension_refines<T: Eq>(candidate: Option<T>, base: Option<T>) -> bool {
    match (candidate, base) {
        (_, None) => true,
        (Some(candidate), Some(base)) => candidate == base,
        (None, Some(_)) => false,
    }
}

fn enum_dimension_refines<T: Copy + Eq>(candidate: T, base: T, any: T) -> bool {
    base == any || candidate == base
}

fn selector_chain_refines(candidate: &[SelectorId], base: &[SelectorId]) -> bool {
    candidate.starts_with(base)
}

/// A semantic leaf destination used for conflict analysis.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Slot {
    /// `display` mode.
    DisplayMode,
    /// `visibility`.
    Visibility,
    /// `position` mode.
    PositionMode,
    /// Top inset.
    InsetTop,
    /// Right inset.
    InsetRight,
    /// Bottom inset.
    InsetBottom,
    /// Left inset.
    InsetLeft,
    /// Stacking order.
    ZIndex,
    /// Horizontal overflow.
    OverflowX,
    /// Vertical overflow.
    OverflowY,
    /// Width.
    Width,
    /// Minimum width.
    MinWidth,
    /// Maximum width.
    MaxWidth,
    /// Height.
    Height,
    /// Minimum height.
    MinHeight,
    /// Maximum height.
    MaxHeight,
    /// Preferred aspect ratio.
    AspectRatio,
    /// Top margin.
    MarginTop,
    /// Right margin.
    MarginRight,
    /// Bottom margin.
    MarginBottom,
    /// Left margin.
    MarginLeft,
    /// Top padding.
    PaddingTop,
    /// Right padding.
    PaddingRight,
    /// Bottom padding.
    PaddingBottom,
    /// Left padding.
    PaddingLeft,
    /// Row gap.
    GapRow,
    /// Column gap.
    GapColumn,
    /// Flex direction.
    FlexDirection,
    /// Flex wrapping.
    FlexWrap,
    /// Flex grow factor.
    FlexGrow,
    /// Flex shrink factor.
    FlexShrink,
    /// Flex basis.
    FlexBasis,
    /// Cross-axis item alignment.
    AlignItems,
    /// Cross-axis content alignment.
    AlignContent,
    /// Per-item cross-axis alignment.
    AlignSelf,
    /// Main-axis content alignment.
    JustifyContent,
    /// Inline-axis item alignment.
    JustifyItems,
    /// Per-item inline-axis alignment.
    JustifySelf,
    /// Grid column template.
    GridTemplateColumns,
    /// Grid row template.
    GridTemplateRows,
    /// Grid column placement.
    GridColumn,
    /// Grid row placement.
    GridRow,
    /// Background color.
    BackgroundColor,
    /// Background image.
    BackgroundImage,
    /// Foreground text color.
    TextColor,
    /// Font family.
    FontFamily,
    /// Font size.
    FontSize,
    /// Font weight.
    FontWeight,
    /// Line height.
    LineHeight,
    /// Letter spacing.
    LetterSpacing,
    /// Platform font smoothing mode.
    FontSmoothing,
    /// Text alignment.
    TextAlign,
    /// Text-decoration line.
    TextDecorationLine,
    /// Top border width.
    BorderTopWidth,
    /// Right border width.
    BorderRightWidth,
    /// Bottom border width.
    BorderBottomWidth,
    /// Left border width.
    BorderLeftWidth,
    /// Top border color.
    BorderTopColor,
    /// Right border color.
    BorderRightColor,
    /// Bottom border color.
    BorderBottomColor,
    /// Left border color.
    BorderLeftColor,
    /// Top border style.
    BorderTopStyle,
    /// Right border style.
    BorderRightStyle,
    /// Bottom border style.
    BorderBottomStyle,
    /// Left border style.
    BorderLeftStyle,
    /// Top-left corner radius.
    BorderTopLeftRadius,
    /// Top-right corner radius.
    BorderTopRightRadius,
    /// Bottom-right corner radius.
    BorderBottomRightRadius,
    /// Bottom-left corner radius.
    BorderBottomLeftRadius,
    /// Outline width.
    OutlineWidth,
    /// Outline color.
    OutlineColor,
    /// Outline offset.
    OutlineOffset,
    /// Outline style.
    OutlineStyle,
    /// Element opacity.
    Opacity,
    /// Box-shadow layers.
    BoxShadow,
    /// Focus-ring width.
    RingWidth,
    /// Focus-ring color.
    RingColor,
    /// Transform list.
    Transform,
    /// Transitioned property set.
    TransitionProperty,
    /// User resize behavior.
    Resize,
    /// Cursor.
    Cursor,
    /// Pointer-event behavior.
    PointerEvents,
    /// A property outside the typed MVP catalog.
    ArbitraryProperty,
    /// Size-query containment established by `container-type`.
    ContainerType,
    /// Block-flow and inline-axis orientation established by `writing-mode`.
    WritingMode,
}

impl Slot {
    /// Every currently defined slot in stable ordinal order.
    pub const ALL: [Self; 85] = [
        Self::DisplayMode,
        Self::Visibility,
        Self::PositionMode,
        Self::InsetTop,
        Self::InsetRight,
        Self::InsetBottom,
        Self::InsetLeft,
        Self::ZIndex,
        Self::OverflowX,
        Self::OverflowY,
        Self::Width,
        Self::MinWidth,
        Self::MaxWidth,
        Self::Height,
        Self::MinHeight,
        Self::MaxHeight,
        Self::AspectRatio,
        Self::MarginTop,
        Self::MarginRight,
        Self::MarginBottom,
        Self::MarginLeft,
        Self::PaddingTop,
        Self::PaddingRight,
        Self::PaddingBottom,
        Self::PaddingLeft,
        Self::GapRow,
        Self::GapColumn,
        Self::FlexDirection,
        Self::FlexWrap,
        Self::FlexGrow,
        Self::FlexShrink,
        Self::FlexBasis,
        Self::AlignItems,
        Self::AlignContent,
        Self::AlignSelf,
        Self::JustifyContent,
        Self::JustifyItems,
        Self::JustifySelf,
        Self::GridTemplateColumns,
        Self::GridTemplateRows,
        Self::GridColumn,
        Self::GridRow,
        Self::BackgroundColor,
        Self::BackgroundImage,
        Self::TextColor,
        Self::FontFamily,
        Self::FontSize,
        Self::FontWeight,
        Self::LineHeight,
        Self::LetterSpacing,
        Self::FontSmoothing,
        Self::TextAlign,
        Self::TextDecorationLine,
        Self::BorderTopWidth,
        Self::BorderRightWidth,
        Self::BorderBottomWidth,
        Self::BorderLeftWidth,
        Self::BorderTopColor,
        Self::BorderRightColor,
        Self::BorderBottomColor,
        Self::BorderLeftColor,
        Self::BorderTopStyle,
        Self::BorderRightStyle,
        Self::BorderBottomStyle,
        Self::BorderLeftStyle,
        Self::BorderTopLeftRadius,
        Self::BorderTopRightRadius,
        Self::BorderBottomRightRadius,
        Self::BorderBottomLeftRadius,
        Self::OutlineWidth,
        Self::OutlineColor,
        Self::OutlineOffset,
        Self::OutlineStyle,
        Self::Opacity,
        Self::BoxShadow,
        Self::RingWidth,
        Self::RingColor,
        Self::Transform,
        Self::TransitionProperty,
        Self::Resize,
        Self::Cursor,
        Self::PointerEvents,
        Self::ArbitraryProperty,
        Self::ContainerType,
        Self::WritingMode,
    ];

    /// Number of representable leaf slots.
    pub const COUNT: usize = Self::ALL.len();
}

/// A compact set of semantic leaf slots.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct SlotSet(u128);

impl SlotSet {
    /// The empty footprint.
    pub const EMPTY: Self = Self(0);
    /// Every known footprint bit.
    pub const ALL: Self = Self((1_u128 << Slot::COUNT) - 1);
    /// The four physical margin slots.
    pub const MARGIN: Self = Self::of(&[
        Slot::MarginTop,
        Slot::MarginRight,
        Slot::MarginBottom,
        Slot::MarginLeft,
    ]);
    /// The four physical padding slots.
    pub const PADDING: Self = Self::of(&[
        Slot::PaddingTop,
        Slot::PaddingRight,
        Slot::PaddingBottom,
        Slot::PaddingLeft,
    ]);
    /// Both gap axes.
    pub const GAP: Self = Self::of(&[Slot::GapRow, Slot::GapColumn]);
    /// Every physical inset slot.
    pub const INSET: Self = Self::of(&[
        Slot::InsetTop,
        Slot::InsetRight,
        Slot::InsetBottom,
        Slot::InsetLeft,
    ]);
    /// Width and height constraints.
    pub const SIZE: Self = Self::of(&[
        Slot::Width,
        Slot::MinWidth,
        Slot::MaxWidth,
        Slot::Height,
        Slot::MinHeight,
        Slot::MaxHeight,
    ]);
    /// Every physical border-width slot.
    pub const BORDER_WIDTH: Self = Self::of(&[
        Slot::BorderTopWidth,
        Slot::BorderRightWidth,
        Slot::BorderBottomWidth,
        Slot::BorderLeftWidth,
    ]);
    /// Every physical border-color slot.
    pub const BORDER_COLOR: Self = Self::of(&[
        Slot::BorderTopColor,
        Slot::BorderRightColor,
        Slot::BorderBottomColor,
        Slot::BorderLeftColor,
    ]);
    /// Every physical border-style slot.
    pub const BORDER_STYLE: Self = Self::of(&[
        Slot::BorderTopStyle,
        Slot::BorderRightStyle,
        Slot::BorderBottomStyle,
        Slot::BorderLeftStyle,
    ]);
    /// Every physical corner-radius slot.
    pub const BORDER_RADIUS: Self = Self::of(&[
        Slot::BorderTopLeftRadius,
        Slot::BorderTopRightRadius,
        Slot::BorderBottomRightRadius,
        Slot::BorderBottomLeftRadius,
    ]);

    /// Creates a footprint containing one slot.
    #[must_use]
    pub const fn from_slot(slot: Slot) -> Self {
        Self(1_u128 << slot as u8)
    }

    /// Creates a footprint from a slice of slots.
    #[must_use]
    pub const fn of(slots: &[Slot]) -> Self {
        let mut bits = 0_u128;
        let mut index = 0;
        while index < slots.len() {
            bits |= Self::from_slot(slots[index]).0;
            index += 1;
        }
        Self(bits)
    }

    /// Creates a footprint if all bits refer to known slots.
    #[must_use]
    pub const fn from_bits(bits: u128) -> Option<Self> {
        if bits & !Self::ALL.0 == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

    /// Returns the compact bit representation.
    #[must_use]
    pub const fn bits(self) -> u128 {
        self.0
    }

    /// Returns a copy with `slot` inserted.
    #[must_use]
    pub const fn with(self, slot: Slot) -> Self {
        Self(self.0 | Self::from_slot(slot).0)
    }

    /// Returns the union of two footprints.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns the intersection of two footprints.
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Returns whether this footprint contains `slot`.
    #[must_use]
    pub const fn contains(self, slot: Slot) -> bool {
        self.0 & Self::from_slot(slot).0 != 0
    }

    /// Returns whether the footprints overlap.
    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Returns whether every slot in this footprint exists in `other`.
    #[must_use]
    pub const fn is_subset(self, other: Self) -> bool {
        self.0 & other.0 == self.0
    }

    /// Returns whether this is a strict subset of `other`.
    #[must_use]
    pub const fn is_proper_subset(self, other: Self) -> bool {
        self.0 != other.0 && self.is_subset(other)
    }

    /// Returns `true` when the footprint contains no slots.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Iterates over slots in stable ordinal order.
    pub fn iter(self) -> impl Iterator<Item = Slot> {
        Slot::ALL
            .into_iter()
            .filter(move |slot| self.contains(*slot))
    }
}

/// Namespace of a design token.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TokenKind {
    /// Spacing or sizing token.
    Spacing,
    /// Color token.
    Color,
    /// Font-family token.
    FontFamily,
    /// Font-size token.
    FontSize,
    /// Font-weight token.
    FontWeight,
    /// Line-height token.
    LineHeight,
    /// Letter-spacing token.
    LetterSpacing,
    /// Border-radius token.
    Radius,
    /// Shadow token.
    Shadow,
    /// Stacking-order token.
    ZIndex,
}

/// A typed reference into the design-token catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TokenRef {
    /// Token namespace.
    pub kind: TokenKind,
    /// Stable token identifier inside the namespace.
    pub id: TokenId,
}

/// A canonical base-10 CSS number.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CssNumber {
    coefficient: i64,
    scale: u8,
}

impl CssNumber {
    /// Maximum supported decimal scale.
    pub const MAX_SCALE: u8 = 18;

    /// Creates and canonicalizes a decimal number.
    ///
    /// Returns `None` when `scale` cannot be represented without exceeding
    /// the exact signed 64-bit decimal range.
    #[must_use]
    pub fn new(mut coefficient: i64, mut scale: u8) -> Option<Self> {
        if scale > Self::MAX_SCALE {
            return None;
        }
        while scale > 0 && coefficient % 10 == 0 {
            coefficient /= 10;
            scale -= 1;
        }
        Some(Self { coefficient, scale })
    }

    /// Creates an integral CSS number.
    #[must_use]
    pub const fn integer(value: i64) -> Self {
        Self {
            coefficient: value,
            scale: 0,
        }
    }

    /// Returns the signed coefficient.
    #[must_use]
    pub const fn coefficient(self) -> i64 {
        self.coefficient
    }

    /// Returns the number of decimal digits after the point.
    #[must_use]
    pub const fn scale(self) -> u8 {
        self.scale
    }
}

/// A CSS length unit represented without source text.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LengthUnit {
    /// CSS pixels.
    Px,
    /// Root-relative `rem` units.
    Rem,
    /// Element-relative `em` units.
    Em,
    /// Percent.
    Percent,
    /// Character width.
    Ch,
    /// Viewport width.
    Vw,
    /// Viewport height.
    Vh,
    /// Dynamic viewport width.
    Dvw,
    /// Dynamic viewport height.
    Dvh,
}

/// A typed CSS length.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Length {
    /// Exact numeric component.
    pub number: CssNumber,
    /// Unit component.
    pub unit: LengthUnit,
}

/// A percentage stored as basis points from `0%` through `100%`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Percentage(u16);

impl Percentage {
    /// Fully transparent or zero percent.
    pub const ZERO: Self = Self(0);
    /// Fully opaque or one hundred percent.
    pub const FULL: Self = Self(10_000);

    /// Creates a percentage from basis points.
    #[must_use]
    pub const fn from_basis_points(value: u16) -> Option<Self> {
        if value <= 10_000 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Returns the basis-point representation.
    #[must_use]
    pub const fn basis_points(self) -> u16 {
        self.0
    }
}

/// A color representable without retaining arbitrary CSS text.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ColorValue {
    /// Fully transparent black.
    Transparent,
    /// The current computed foreground color.
    CurrentColor,
    /// A token color with an optional alpha modifier.
    Token {
        /// Stable color-token identifier.
        id: TokenId,
        /// Optional alpha modifier.
        alpha: Option<Percentage>,
    },
    /// Eight-bit sRGB channels and alpha.
    Srgba {
        /// Red channel.
        red: u8,
        /// Green channel.
        green: u8,
        /// Blue channel.
        blue: u8,
        /// Alpha channel.
        alpha: u8,
    },
}

/// Type hint attached to a custom CSS property reference.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueKind {
    /// No narrower static type is known.
    Any,
    /// CSS keyword.
    Keyword,
    /// Design-token reference.
    Token,
    /// Signed integer.
    Integer,
    /// Unitless number.
    Number,
    /// Length or percentage accepted by a length domain.
    Length,
    /// Percentage constrained to zero through one hundred.
    Percentage,
    /// Color.
    Color,
    /// Rational fraction.
    Fraction,
    /// Shadow list.
    Shadow,
    /// Image value.
    Image,
    /// Grid template.
    GridTemplate,
    /// Transform list.
    Transform,
}

/// A typed custom-property reference.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CustomPropertyRef {
    /// Interned custom-property name.
    pub id: CustomPropertyId,
    /// Optional author-supplied type hint.
    pub hint: ValueKind,
}

/// Canonical CSS keywords used by the MVP catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Keyword {
    /// `none`.
    None,
    /// `auto`.
    Auto,
    /// `normal`.
    Normal,
    /// `block`.
    Block,
    /// `inline`.
    Inline,
    /// `inline-block`.
    InlineBlock,
    /// `flex`.
    Flex,
    /// `inline-flex`.
    InlineFlex,
    /// `grid`.
    Grid,
    /// `inline-grid`.
    InlineGrid,
    /// `visible`.
    Visible,
    /// `hidden`.
    Hidden,
    /// `collapse`.
    Collapse,
    /// `static`.
    Static,
    /// `relative`.
    Relative,
    /// `absolute`.
    Absolute,
    /// `fixed`.
    Fixed,
    /// `sticky`.
    Sticky,
    /// `clip`.
    Clip,
    /// `scroll`.
    Scroll,
    /// `row`.
    Row,
    /// `row-reverse`.
    RowReverse,
    /// `column`.
    Column,
    /// `column-reverse`.
    ColumnReverse,
    /// `wrap`.
    Wrap,
    /// `wrap-reverse`.
    WrapReverse,
    /// `nowrap`.
    NoWrap,
    /// `start`.
    Start,
    /// `end`.
    End,
    /// `center`.
    Center,
    /// `space-between`.
    SpaceBetween,
    /// `space-around`.
    SpaceAround,
    /// `space-evenly`.
    SpaceEvenly,
    /// `stretch`.
    Stretch,
    /// `baseline`.
    Baseline,
    /// `min-content`.
    MinContent,
    /// `max-content`.
    MaxContent,
    /// `fit-content`.
    FitContent,
    /// `content`.
    Content,
    /// `left`.
    Left,
    /// `right`.
    Right,
    /// `justify`.
    Justify,
    /// `bold`.
    Bold,
    /// `underline`.
    Underline,
    /// `overline`.
    Overline,
    /// `line-through`.
    LineThrough,
    /// `solid`.
    Solid,
    /// `dashed`.
    Dashed,
    /// `dotted`.
    Dotted,
    /// `double`.
    Double,
    /// `pointer`.
    Pointer,
    /// `default`.
    Default,
    /// `wait`.
    Wait,
    /// `not-allowed`.
    NotAllowed,
    /// `text`.
    Text,
    /// `move`.
    Move,
    /// Platform antialiasing mode.
    Antialiased,
    /// Transition bundle for color-related properties.
    Colors,
    /// Vertical-only resizing.
    Vertical,
    /// `inline-size`.
    InlineSize,
    /// `horizontal-tb`.
    HorizontalTb,
    /// `vertical-lr`.
    VerticalLr,
    /// `vertical-rl`.
    VerticalRl,
}

/// A typed semantic value assigned by one utility.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticValue {
    /// Canonical keyword.
    Keyword(Keyword),
    /// Typed design-token reference.
    Token(TokenRef),
    /// Signed integer.
    Integer(i32),
    /// Exact unitless number.
    Number(CssNumber),
    /// Exact length.
    Length(Length),
    /// Bounded percentage.
    Percentage(Percentage),
    /// Typed color.
    Color(ColorValue),
    /// Reduced rational fraction.
    Fraction {
        /// Positive numerator.
        numerator: u16,
        /// Positive denominator.
        denominator: u16,
    },
    /// Interned arbitrary CSS value.
    Arbitrary(ArbitraryValueId),
    /// Interned custom-property reference.
    CustomProperty(CustomPropertyRef),
}

impl SemanticValue {
    /// Returns the broad domain used for utility validation.
    #[must_use]
    pub const fn kind(self) -> ValueKind {
        match self {
            Self::Keyword(_) => ValueKind::Keyword,
            Self::Token(_) => ValueKind::Token,
            Self::Integer(_) => ValueKind::Integer,
            Self::Number(_) => ValueKind::Number,
            Self::Length(_) => ValueKind::Length,
            Self::Percentage(_) => ValueKind::Percentage,
            Self::Color(_) => ValueKind::Color,
            Self::Fraction { .. } => ValueKind::Fraction,
            Self::Arbitrary(_) | Self::CustomProperty(_) => ValueKind::Any,
        }
    }
}

/// A utility family after catalog resolution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Utility {
    /// Display mode.
    Display,
    /// Visibility.
    Visibility,
    /// Positioning mode.
    Position,
    /// Physical inset.
    Inset,
    /// Stacking order.
    ZIndex,
    /// Overflow on one or both axes.
    Overflow,
    /// Width.
    Width,
    /// Minimum width.
    MinWidth,
    /// Maximum width.
    MaxWidth,
    /// Height.
    Height,
    /// Minimum height.
    MinHeight,
    /// Maximum height.
    MaxHeight,
    /// Preferred aspect ratio.
    AspectRatio,
    /// Physical margin.
    Margin,
    /// Physical padding.
    Padding,
    /// Row or column gap.
    Gap,
    /// Flex direction.
    FlexDirection,
    /// Flex wrapping.
    FlexWrap,
    /// Flex grow factor.
    FlexGrow,
    /// Flex shrink factor.
    FlexShrink,
    /// Flex basis.
    FlexBasis,
    /// `align-items`.
    AlignItems,
    /// `align-content`.
    AlignContent,
    /// `align-self`.
    AlignSelf,
    /// `justify-content`.
    JustifyContent,
    /// `justify-items`.
    JustifyItems,
    /// `justify-self`.
    JustifySelf,
    /// Grid column template.
    GridTemplateColumns,
    /// Grid row template.
    GridTemplateRows,
    /// Grid column placement.
    GridColumn,
    /// Grid row placement.
    GridRow,
    /// Background color.
    BackgroundColor,
    /// Background image.
    BackgroundImage,
    /// Foreground text color.
    TextColor,
    /// Font family.
    FontFamily,
    /// Font size.
    FontSize,
    /// Font weight.
    FontWeight,
    /// Line height.
    LineHeight,
    /// Letter spacing.
    LetterSpacing,
    /// Platform font smoothing behavior.
    FontSmoothing,
    /// Text alignment.
    TextAlign,
    /// Text-decoration line.
    TextDecorationLine,
    /// Physical border width.
    BorderWidth,
    /// Physical border color.
    BorderColor,
    /// Physical border style.
    BorderStyle,
    /// Physical corner radius.
    BorderRadius,
    /// Outline width.
    OutlineWidth,
    /// Outline color.
    OutlineColor,
    /// Outline offset.
    OutlineOffset,
    /// Outline style.
    OutlineStyle,
    /// Element opacity.
    Opacity,
    /// Box-shadow list.
    BoxShadow,
    /// Focus-ring width.
    RingWidth,
    /// Focus-ring color.
    RingColor,
    /// Transform list.
    Transform,
    /// Transitioned property bundle.
    TransitionProperty,
    /// User resize behavior.
    Resize,
    /// Cursor.
    Cursor,
    /// Pointer-event behavior.
    PointerEvents,
    /// A declaration outside the typed MVP catalog.
    ArbitraryProperty(ArbitraryPropertyId),
    /// Size-query containment mode.
    ContainerType,
    /// Block-flow and inline-axis orientation.
    WritingMode,
}

impl Utility {
    /// Returns every slot this family is permitted to assign.
    #[must_use]
    pub const fn allowed_slots(self) -> SlotSet {
        match self {
            Self::Display => SlotSet::from_slot(Slot::DisplayMode),
            Self::Visibility => SlotSet::from_slot(Slot::Visibility),
            Self::Position => SlotSet::from_slot(Slot::PositionMode),
            Self::Inset => SlotSet::INSET,
            Self::ZIndex => SlotSet::from_slot(Slot::ZIndex),
            Self::Overflow => SlotSet::of(&[Slot::OverflowX, Slot::OverflowY]),
            Self::Width => SlotSet::from_slot(Slot::Width),
            Self::MinWidth => SlotSet::from_slot(Slot::MinWidth),
            Self::MaxWidth => SlotSet::from_slot(Slot::MaxWidth),
            Self::Height => SlotSet::from_slot(Slot::Height),
            Self::MinHeight => SlotSet::from_slot(Slot::MinHeight),
            Self::MaxHeight => SlotSet::from_slot(Slot::MaxHeight),
            Self::AspectRatio => SlotSet::from_slot(Slot::AspectRatio),
            Self::Margin => SlotSet::MARGIN,
            Self::Padding => SlotSet::PADDING,
            Self::Gap => SlotSet::GAP,
            Self::FlexDirection => SlotSet::from_slot(Slot::FlexDirection),
            Self::FlexWrap => SlotSet::from_slot(Slot::FlexWrap),
            Self::FlexGrow => SlotSet::from_slot(Slot::FlexGrow),
            Self::FlexShrink => SlotSet::from_slot(Slot::FlexShrink),
            Self::FlexBasis => SlotSet::from_slot(Slot::FlexBasis),
            Self::AlignItems => SlotSet::from_slot(Slot::AlignItems),
            Self::AlignContent => SlotSet::from_slot(Slot::AlignContent),
            Self::AlignSelf => SlotSet::from_slot(Slot::AlignSelf),
            Self::JustifyContent => SlotSet::from_slot(Slot::JustifyContent),
            Self::JustifyItems => SlotSet::from_slot(Slot::JustifyItems),
            Self::JustifySelf => SlotSet::from_slot(Slot::JustifySelf),
            Self::GridTemplateColumns => SlotSet::from_slot(Slot::GridTemplateColumns),
            Self::GridTemplateRows => SlotSet::from_slot(Slot::GridTemplateRows),
            Self::GridColumn => SlotSet::from_slot(Slot::GridColumn),
            Self::GridRow => SlotSet::from_slot(Slot::GridRow),
            Self::BackgroundColor => SlotSet::from_slot(Slot::BackgroundColor),
            Self::BackgroundImage => SlotSet::from_slot(Slot::BackgroundImage),
            Self::TextColor => SlotSet::from_slot(Slot::TextColor),
            Self::FontFamily => SlotSet::from_slot(Slot::FontFamily),
            Self::FontSize => SlotSet::from_slot(Slot::FontSize),
            Self::FontWeight => SlotSet::from_slot(Slot::FontWeight),
            Self::LineHeight => SlotSet::from_slot(Slot::LineHeight),
            Self::LetterSpacing => SlotSet::from_slot(Slot::LetterSpacing),
            Self::FontSmoothing => SlotSet::from_slot(Slot::FontSmoothing),
            Self::TextAlign => SlotSet::from_slot(Slot::TextAlign),
            Self::TextDecorationLine => SlotSet::from_slot(Slot::TextDecorationLine),
            Self::BorderWidth => SlotSet::BORDER_WIDTH,
            Self::BorderColor => SlotSet::BORDER_COLOR,
            Self::BorderStyle => SlotSet::BORDER_STYLE,
            Self::BorderRadius => SlotSet::BORDER_RADIUS,
            Self::OutlineWidth => SlotSet::from_slot(Slot::OutlineWidth),
            Self::OutlineColor => SlotSet::from_slot(Slot::OutlineColor),
            Self::OutlineOffset => SlotSet::from_slot(Slot::OutlineOffset),
            Self::OutlineStyle => SlotSet::from_slot(Slot::OutlineStyle),
            Self::Opacity => SlotSet::from_slot(Slot::Opacity),
            Self::BoxShadow => SlotSet::from_slot(Slot::BoxShadow),
            Self::RingWidth => SlotSet::from_slot(Slot::RingWidth),
            Self::RingColor => SlotSet::from_slot(Slot::RingColor),
            Self::Transform => SlotSet::from_slot(Slot::Transform),
            Self::TransitionProperty => SlotSet::from_slot(Slot::TransitionProperty),
            Self::Resize => SlotSet::from_slot(Slot::Resize),
            Self::Cursor => SlotSet::from_slot(Slot::Cursor),
            Self::PointerEvents => SlotSet::from_slot(Slot::PointerEvents),
            Self::ArbitraryProperty(_) => SlotSet::from_slot(Slot::ArbitraryProperty),
            Self::ContainerType => SlotSet::from_slot(Slot::ContainerType),
            Self::WritingMode => SlotSet::from_slot(Slot::WritingMode),
        }
    }

    /// Returns whether this family supports the canonical leading minus form.
    #[must_use]
    pub const fn allows_negative(self) -> bool {
        matches!(self, Self::Inset | Self::Margin | Self::LetterSpacing)
    }

    /// Returns whether this family accepts the semantic value.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn accepts(self, value: SemanticValue) -> bool {
        if matches!(value, SemanticValue::Arbitrary(_)) {
            return true;
        }
        if let SemanticValue::CustomProperty(reference) = value {
            return reference.hint == ValueKind::Any || self.accepts_kind(reference.hint);
        }
        if let SemanticValue::Token(reference) = value {
            return self.accepts_token(reference.kind);
        }
        match (self, value) {
            (Self::Display, SemanticValue::Keyword(keyword)) => matches!(
                keyword,
                Keyword::None
                    | Keyword::Block
                    | Keyword::Inline
                    | Keyword::InlineBlock
                    | Keyword::Flex
                    | Keyword::InlineFlex
                    | Keyword::Grid
                    | Keyword::InlineGrid
            ),
            (Self::Visibility, SemanticValue::Keyword(keyword)) => {
                matches!(
                    keyword,
                    Keyword::Visible | Keyword::Hidden | Keyword::Collapse
                )
            }
            (Self::Position, SemanticValue::Keyword(keyword)) => matches!(
                keyword,
                Keyword::Static
                    | Keyword::Relative
                    | Keyword::Absolute
                    | Keyword::Fixed
                    | Keyword::Sticky
            ),
            (Self::Overflow, SemanticValue::Keyword(keyword)) => matches!(
                keyword,
                Keyword::Auto
                    | Keyword::Visible
                    | Keyword::Hidden
                    | Keyword::Clip
                    | Keyword::Scroll
            ),
            (Self::FlexDirection, SemanticValue::Keyword(keyword)) => matches!(
                keyword,
                Keyword::Row | Keyword::RowReverse | Keyword::Column | Keyword::ColumnReverse
            ),
            (Self::FlexWrap, SemanticValue::Keyword(keyword)) => matches!(
                keyword,
                Keyword::Wrap | Keyword::WrapReverse | Keyword::NoWrap
            ),
            (
                Self::AlignItems
                | Self::AlignContent
                | Self::AlignSelf
                | Self::JustifyContent
                | Self::JustifyItems
                | Self::JustifySelf,
                SemanticValue::Keyword(keyword),
            ) => matches!(
                keyword,
                Keyword::Auto
                    | Keyword::Normal
                    | Keyword::Start
                    | Keyword::End
                    | Keyword::Center
                    | Keyword::SpaceBetween
                    | Keyword::SpaceAround
                    | Keyword::SpaceEvenly
                    | Keyword::Stretch
                    | Keyword::Baseline
            ),
            (Self::TextAlign, SemanticValue::Keyword(keyword)) => matches!(
                keyword,
                Keyword::Left
                    | Keyword::Right
                    | Keyword::Start
                    | Keyword::End
                    | Keyword::Center
                    | Keyword::Justify
            ),
            (Self::TextDecorationLine, SemanticValue::Keyword(keyword)) => matches!(
                keyword,
                Keyword::None | Keyword::Underline | Keyword::Overline | Keyword::LineThrough
            ),
            (Self::BorderStyle, SemanticValue::Keyword(keyword)) => matches!(
                keyword,
                Keyword::None
                    | Keyword::Solid
                    | Keyword::Dashed
                    | Keyword::Dotted
                    | Keyword::Double
            ),
            (Self::Cursor, SemanticValue::Keyword(keyword)) => matches!(
                keyword,
                Keyword::Auto
                    | Keyword::Pointer
                    | Keyword::Default
                    | Keyword::Wait
                    | Keyword::NotAllowed
                    | Keyword::Text
                    | Keyword::Move
            ),
            (Self::PointerEvents, SemanticValue::Keyword(keyword)) => {
                matches!(keyword, Keyword::Auto | Keyword::None)
            }
            (Self::FontSmoothing, SemanticValue::Keyword(keyword)) => {
                keyword == Keyword::Antialiased
            }
            (Self::TransitionProperty, SemanticValue::Keyword(keyword)) => {
                keyword == Keyword::Colors
            }
            (Self::Resize, SemanticValue::Keyword(keyword)) => keyword == Keyword::Vertical,
            (Self::ContainerType, SemanticValue::Keyword(keyword)) => {
                matches!(keyword, Keyword::Normal | Keyword::InlineSize)
            }
            (Self::WritingMode, SemanticValue::Keyword(keyword)) => matches!(
                keyword,
                Keyword::HorizontalTb | Keyword::VerticalLr | Keyword::VerticalRl
            ),
            (
                Self::BackgroundImage | Self::BoxShadow | Self::Transform | Self::OutlineStyle,
                SemanticValue::Keyword(keyword),
            ) => keyword == Keyword::None,
            (
                Self::Width
                | Self::MinWidth
                | Self::MaxWidth
                | Self::Height
                | Self::MinHeight
                | Self::MaxHeight
                | Self::FlexBasis,
                SemanticValue::Keyword(keyword),
            ) => matches!(
                keyword,
                Keyword::Auto
                    | Keyword::MinContent
                    | Keyword::MaxContent
                    | Keyword::FitContent
                    | Keyword::Content
            ),
            (Self::Margin, SemanticValue::Keyword(Keyword::Auto)) => true,
            (Self::FontWeight, SemanticValue::Keyword(keyword)) => {
                matches!(keyword, Keyword::Normal | Keyword::Bold)
            }
            (utility, semantic_value) => utility.accepts_kind(semantic_value.kind()),
        }
    }

    fn accepts_kind(self, kind: ValueKind) -> bool {
        match self {
            Self::Inset
            | Self::Width
            | Self::MinWidth
            | Self::MaxWidth
            | Self::Height
            | Self::MinHeight
            | Self::MaxHeight
            | Self::Margin
            | Self::Padding
            | Self::Gap
            | Self::FlexBasis
            | Self::BorderWidth
            | Self::BorderRadius
            | Self::OutlineWidth
            | Self::OutlineOffset
            | Self::RingWidth => matches!(
                kind,
                ValueKind::Length | ValueKind::Percentage | ValueKind::Fraction | ValueKind::Token
            ),
            Self::ZIndex | Self::GridColumn | Self::GridRow => {
                matches!(kind, ValueKind::Integer | ValueKind::Token)
            }
            Self::AspectRatio => kind == ValueKind::Fraction,
            Self::FlexGrow | Self::FlexShrink => {
                matches!(kind, ValueKind::Integer | ValueKind::Number)
            }
            Self::GridTemplateColumns | Self::GridTemplateRows => {
                matches!(kind, ValueKind::GridTemplate | ValueKind::Integer)
            }
            Self::BackgroundColor
            | Self::TextColor
            | Self::BorderColor
            | Self::OutlineColor
            | Self::RingColor => {
                matches!(kind, ValueKind::Color | ValueKind::Token)
            }
            Self::BackgroundImage => kind == ValueKind::Image,
            Self::FontFamily => kind == ValueKind::Token,
            Self::FontSize => matches!(kind, ValueKind::Length | ValueKind::Token),
            Self::FontWeight => matches!(
                kind,
                ValueKind::Integer | ValueKind::Number | ValueKind::Token
            ),
            Self::LineHeight => matches!(
                kind,
                ValueKind::Length | ValueKind::Number | ValueKind::Token
            ),
            Self::LetterSpacing => matches!(kind, ValueKind::Length | ValueKind::Token),
            Self::Opacity => matches!(kind, ValueKind::Number | ValueKind::Percentage),
            Self::BoxShadow => matches!(kind, ValueKind::Shadow | ValueKind::Token),
            Self::Transform => kind == ValueKind::Transform,
            Self::ArbitraryProperty(_) => kind == ValueKind::Any,
            Self::Display
            | Self::Visibility
            | Self::Position
            | Self::Overflow
            | Self::FlexDirection
            | Self::FlexWrap
            | Self::AlignItems
            | Self::AlignContent
            | Self::AlignSelf
            | Self::JustifyContent
            | Self::JustifyItems
            | Self::JustifySelf
            | Self::TextAlign
            | Self::TextDecorationLine
            | Self::BorderStyle
            | Self::FontSmoothing
            | Self::OutlineStyle
            | Self::TransitionProperty
            | Self::Resize
            | Self::Cursor
            | Self::PointerEvents
            | Self::ContainerType
            | Self::WritingMode => kind == ValueKind::Keyword,
        }
    }

    fn accepts_token(self, kind: TokenKind) -> bool {
        match kind {
            TokenKind::Spacing => matches!(
                self,
                Self::Inset
                    | Self::Width
                    | Self::MinWidth
                    | Self::MaxWidth
                    | Self::Height
                    | Self::MinHeight
                    | Self::MaxHeight
                    | Self::Margin
                    | Self::Padding
                    | Self::Gap
                    | Self::FlexBasis
                    | Self::BorderWidth
                    | Self::OutlineWidth
                    | Self::OutlineOffset
                    | Self::RingWidth
            ),
            TokenKind::Color => matches!(
                self,
                Self::BackgroundColor
                    | Self::TextColor
                    | Self::BorderColor
                    | Self::OutlineColor
                    | Self::RingColor
            ),
            TokenKind::FontFamily => self == Self::FontFamily,
            TokenKind::FontSize => self == Self::FontSize,
            TokenKind::FontWeight => self == Self::FontWeight,
            TokenKind::LineHeight => self == Self::LineHeight,
            TokenKind::LetterSpacing => self == Self::LetterSpacing,
            TokenKind::Radius => self == Self::BorderRadius,
            TokenKind::Shadow => self == Self::BoxShadow,
            TokenKind::ZIndex => self == Self::ZIndex,
        }
    }
}

/// One normalized assignment in the semantic IR.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Assignment {
    /// Catalog utility family that produced the assignment.
    pub utility: Utility,
    /// Semantic leaf slots touched by the assignment.
    pub slots: SlotSet,
    /// Typed value assigned to every touched slot.
    pub value: SemanticValue,
    /// Whether the resolved value is negated by the canonical utility form.
    pub negative: bool,
    /// Canonical condition table entry.
    pub condition: ConditionId,
    /// Whether the declaration must interoperate as `!important`.
    pub important: bool,
    /// Portable source range for diagnostics and provenance.
    pub source: Span,
}

/// An interned arbitrary CSS property.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArbitraryProperty {
    /// Canonical CSS property name without a value or colon.
    pub name: String,
}

/// Flat, owned semantic representation of one style literal.
///
/// IDs index adjacent vectors; there are no trait objects, borrowed strings,
/// or target-sized source offsets in this representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticStyle {
    /// Stable ID derived after semantic normalization.
    pub id: StyleId,
    /// Normalized assignments.
    pub assignments: Vec<Assignment>,
    /// Canonical conditions. Entry zero is always [`Condition::BASE`].
    pub conditions: Vec<Condition>,
    /// Interned arbitrary values without surrounding brackets.
    pub arbitrary_values: Vec<String>,
    /// Interned custom-property names including their `--` prefix.
    pub custom_properties: Vec<String>,
    /// Interned ordered selector transformations.
    pub selectors: Vec<String>,
    /// Interned arbitrary CSS property names.
    pub arbitrary_properties: Vec<ArbitraryProperty>,
}

/// Concise alias used by compiler APIs.
pub type StyleIr = SemanticStyle;

impl Default for SemanticStyle {
    fn default() -> Self {
        Self::new(StyleId::UNRESOLVED)
    }
}

impl SemanticStyle {
    /// Creates an empty style with its canonical base condition installed.
    #[must_use]
    pub fn new(id: StyleId) -> Self {
        Self {
            id,
            assignments: Vec::new(),
            conditions: vec![Condition::BASE],
            arbitrary_values: Vec::new(),
            custom_properties: Vec::new(),
            selectors: Vec::new(),
            arbitrary_properties: Vec::new(),
        }
    }

    /// Returns the stable ID of the base condition.
    #[must_use]
    pub const fn base_condition() -> ConditionId {
        ConditionId::new(0)
    }

    /// Interns a canonical condition.
    pub fn intern_condition(&mut self, condition: Condition) -> ConditionId {
        intern(&mut self.conditions, condition)
    }

    /// Interns an arbitrary CSS value.
    pub fn intern_arbitrary_value(&mut self, value: String) -> ArbitraryValueId {
        intern(&mut self.arbitrary_values, value)
    }

    /// Interns a custom-property name.
    pub fn intern_custom_property(&mut self, name: String) -> CustomPropertyId {
        intern(&mut self.custom_properties, name)
    }

    /// Interns an ordered selector transformation.
    pub fn intern_selector(&mut self, selector: String) -> SelectorId {
        intern(&mut self.selectors, selector)
    }

    /// Interns an arbitrary CSS property name.
    pub fn intern_arbitrary_property(
        &mut self,
        property: ArbitraryProperty,
    ) -> ArbitraryPropertyId {
        intern(&mut self.arbitrary_properties, property)
    }

    /// Checks all structural invariants required by later compiler passes.
    ///
    /// Semantic conflicts are intentionally not checked here; they are a
    /// separate analysis over structurally valid assignments.
    ///
    /// # Errors
    ///
    /// Returns the first [`InvariantError`] when an ID is out of bounds, an
    /// intern table is not canonical, or an assignment is structurally invalid.
    pub fn validate(&self) -> Result<(), InvariantError> {
        if self.conditions.first() != Some(&Condition::BASE) {
            return Err(InvariantError::new(
                InvariantViolation::MissingBaseCondition,
            ));
        }
        ensure_unique(&self.conditions, InvariantViolation::DuplicateCondition)?;
        ensure_string_table(
            &self.arbitrary_values,
            InvariantViolation::EmptyArbitraryValue,
            InvariantViolation::DuplicateArbitraryValue,
        )?;
        ensure_string_table(
            &self.selectors,
            InvariantViolation::EmptySelector,
            InvariantViolation::DuplicateSelector,
        )?;
        ensure_string_table(
            &self.custom_properties,
            InvariantViolation::InvalidCustomProperty,
            InvariantViolation::DuplicateCustomProperty,
        )?;
        if self
            .custom_properties
            .iter()
            .any(|name| !valid_custom_property(name))
        {
            return Err(InvariantError::new(
                InvariantViolation::InvalidCustomProperty,
            ));
        }
        ensure_unique(
            &self.arbitrary_properties,
            InvariantViolation::DuplicateArbitraryProperty,
        )?;
        if self
            .arbitrary_properties
            .iter()
            .any(|property| !valid_arbitrary_property(&property.name))
        {
            return Err(InvariantError::new(
                InvariantViolation::InvalidArbitraryProperty,
            ));
        }
        for (index, condition) in self.conditions.iter().enumerate() {
            if condition
                .selectors
                .iter()
                .any(|selector| selector.index() >= self.selectors.len())
            {
                return Err(InvariantError::at_condition(
                    InvariantViolation::SelectorOutOfBounds,
                    index,
                ));
            }
        }
        let mut seen_assignments = BTreeSet::new();
        for (index, assignment) in self.assignments.iter().enumerate() {
            self.validate_assignment(index, assignment)?;
            if !seen_assignments.insert(assignment) {
                return Err(InvariantError::at_assignment(
                    InvariantViolation::DuplicateAssignment,
                    index,
                ));
            }
        }
        Ok(())
    }

    fn validate_assignment(
        &self,
        index: usize,
        assignment: &Assignment,
    ) -> Result<(), InvariantError> {
        let invalid = |violation| InvariantError::at_assignment(violation, index);
        if assignment.source.is_empty() {
            return Err(invalid(InvariantViolation::EmptySourceSpan));
        }
        if assignment.slots.is_empty() {
            return Err(invalid(InvariantViolation::EmptySlotSet));
        }
        if !assignment
            .slots
            .is_subset(assignment.utility.allowed_slots())
        {
            return Err(invalid(InvariantViolation::SlotOutsideUtility));
        }
        if !assignment.utility.accepts(assignment.value) {
            return Err(invalid(InvariantViolation::IncompatibleValue));
        }
        if assignment.negative && !assignment.utility.allows_negative() {
            return Err(invalid(InvariantViolation::NegativeUnsupported));
        }
        if assignment.condition.index() >= self.conditions.len() {
            return Err(invalid(InvariantViolation::ConditionOutOfBounds));
        }
        match assignment.value {
            SemanticValue::Fraction {
                numerator,
                denominator,
            } if numerator == 0
                || denominator == 0
                || greatest_common_divisor(numerator, denominator) != 1 =>
            {
                return Err(invalid(InvariantViolation::InvalidFraction));
            }
            SemanticValue::Arbitrary(id) if id.index() >= self.arbitrary_values.len() => {
                return Err(invalid(InvariantViolation::ArbitraryValueOutOfBounds));
            }
            SemanticValue::CustomProperty(reference)
                if reference.id.index() >= self.custom_properties.len() =>
            {
                return Err(invalid(InvariantViolation::CustomPropertyOutOfBounds));
            }
            _ => {}
        }
        if let Utility::ArbitraryProperty(id) = assignment.utility {
            if id.index() >= self.arbitrary_properties.len() {
                return Err(invalid(InvariantViolation::ArbitraryPropertyOutOfBounds));
            }
            if assignment.slots != SlotSet::from_slot(Slot::ArbitraryProperty) {
                return Err(invalid(InvariantViolation::SlotOutsideUtility));
            }
        }
        Ok(())
    }
}

fn intern<T, I>(table: &mut Vec<T>, value: T) -> I
where
    T: Eq,
    I: CompactIndex,
{
    let index = table
        .iter()
        .position(|entry| entry == &value)
        .unwrap_or_else(|| {
            table.push(value);
            table.len() - 1
        });
    I::from_index(index).expect("semantic IR table exceeded its compact ID capacity")
}

trait CompactIndex: Sized {
    fn from_index(index: usize) -> Option<Self>;
}

macro_rules! compact_index_impl {
    ($($name:ident),+ $(,)?) => {
        $(
            impl CompactIndex for $name {
                fn from_index(index: usize) -> Option<Self> {
                    Self::from_index(index)
                }
            }
        )+
    };
}

compact_index_impl!(
    ConditionId,
    ArbitraryValueId,
    CustomPropertyId,
    SelectorId,
    ArbitraryPropertyId,
);

fn ensure_unique<T: Ord>(table: &[T], violation: InvariantViolation) -> Result<(), InvariantError> {
    let mut seen = BTreeSet::new();
    for (index, entry) in table.iter().enumerate() {
        if !seen.insert(entry) {
            return Err(InvariantError::at_table(violation, index));
        }
    }
    Ok(())
}

fn ensure_string_table(
    table: &[String],
    empty: InvariantViolation,
    duplicate: InvariantViolation,
) -> Result<(), InvariantError> {
    if let Some(index) = table.iter().position(String::is_empty) {
        return Err(InvariantError::at_table(empty, index));
    }
    ensure_unique(table, duplicate)
}

fn valid_custom_property(name: &str) -> bool {
    name.starts_with("--")
        && name.len() > 2
        && !name.chars().any(|character| {
            character.is_whitespace() || matches!(character, ':' | ';' | '{' | '}')
        })
}

fn valid_arbitrary_property(name: &str) -> bool {
    !name.is_empty()
        && !name.chars().any(|character| {
            character.is_whitespace() || matches!(character, ':' | ';' | '{' | '}')
        })
}

const fn greatest_common_divisor(mut left: u16, mut right: u16) -> u16 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// A structural semantic-IR invariant that was violated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvariantViolation {
    /// Condition zero is absent or is not the base condition.
    MissingBaseCondition,
    /// The condition table contains a duplicate canonical key.
    DuplicateCondition,
    /// An arbitrary value is empty.
    EmptyArbitraryValue,
    /// The arbitrary-value table contains a duplicate.
    DuplicateArbitraryValue,
    /// A selector is empty.
    EmptySelector,
    /// The selector table contains a duplicate.
    DuplicateSelector,
    /// A custom-property name is malformed.
    InvalidCustomProperty,
    /// The custom-property table contains a duplicate.
    DuplicateCustomProperty,
    /// An arbitrary CSS property name is malformed.
    InvalidArbitraryProperty,
    /// The arbitrary-property table contains a duplicate.
    DuplicateArbitraryProperty,
    /// A condition references an unknown selector.
    SelectorOutOfBounds,
    /// An assignment has no diagnostic source range.
    EmptySourceSpan,
    /// An assignment has no semantic destination.
    EmptySlotSet,
    /// An assignment touches a slot outside its utility family.
    SlotOutsideUtility,
    /// A value does not belong to its utility domain.
    IncompatibleValue,
    /// The utility family cannot be negated.
    NegativeUnsupported,
    /// An assignment references an unknown condition.
    ConditionOutOfBounds,
    /// A fraction is zero, has a zero denominator, or is not reduced.
    InvalidFraction,
    /// An assignment references an unknown arbitrary value.
    ArbitraryValueOutOfBounds,
    /// An assignment references an unknown custom property.
    CustomPropertyOutOfBounds,
    /// An assignment references an unknown arbitrary CSS property.
    ArbitraryPropertyOutOfBounds,
    /// The assignment table contains an exact duplicate.
    DuplicateAssignment,
}

/// Location category for a structural IR error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvariantLocation {
    /// The error concerns the style as a whole.
    Style,
    /// The error concerns an assignment at this index.
    Assignment(usize),
    /// The error concerns a condition at this index.
    Condition(usize),
    /// The error concerns an intern table at this index.
    TableEntry(usize),
}

/// Error returned when semantic IR violates a structural invariant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvariantError {
    /// Broken invariant.
    pub violation: InvariantViolation,
    /// Most precise structural location available.
    pub location: InvariantLocation,
}

impl InvariantError {
    const fn new(violation: InvariantViolation) -> Self {
        Self {
            violation,
            location: InvariantLocation::Style,
        }
    }

    const fn at_assignment(violation: InvariantViolation, index: usize) -> Self {
        Self {
            violation,
            location: InvariantLocation::Assignment(index),
        }
    }

    const fn at_condition(violation: InvariantViolation, index: usize) -> Self {
        Self {
            violation,
            location: InvariantLocation::Condition(index),
        }
    }

    const fn at_table(violation: InvariantViolation, index: usize) -> Self {
        Self {
            violation,
            location: InvariantLocation::TableEntry(index),
        }
    }
}

impl fmt::Display for InvariantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "semantic IR invariant {:?} failed at {:?}",
            self.violation, self.location
        )
    }
}

impl std::error::Error for InvariantError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn spacing_token(id: u32) -> SemanticValue {
        SemanticValue::Token(TokenRef {
            kind: TokenKind::Spacing,
            id: TokenId::new(id),
        })
    }

    fn assignment(utility: Utility, slots: SlotSet, value: SemanticValue) -> Assignment {
        Assignment {
            utility,
            slots,
            value,
            negative: false,
            condition: SemanticStyle::base_condition(),
            important: false,
            source: Span::new(0, 3),
        }
    }

    #[test]
    fn portable_span_round_trips() {
        let syntax = SourceSpan::new(7, 31);
        let semantic = Span::try_from(syntax).expect("small span must fit");

        assert_eq!(semantic, Span::new(7, 31));
        assert_eq!(SourceSpan::from(semantic), syntax);
    }

    #[test]
    fn decimal_numbers_are_canonicalized() {
        let number = CssNumber::new(12_300, 3).expect("scale is supported");

        assert_eq!(number.coefficient(), 123);
        assert_eq!(number.scale(), 1);
        assert!(CssNumber::new(1, CssNumber::MAX_SCALE + 1).is_none());
    }

    #[test]
    fn footprints_express_axis_refinement() {
        let horizontal = SlotSet::of(&[Slot::PaddingLeft, Slot::PaddingRight]);
        let vertical = SlotSet::of(&[Slot::PaddingTop, Slot::PaddingBottom]);

        assert!(horizontal.is_proper_subset(SlotSet::PADDING));
        assert!(!horizontal.intersects(vertical));
        assert_eq!(horizontal.union(vertical), SlotSet::PADDING);
        assert_eq!(SlotSet::PADDING.iter().count(), 4);
    }

    #[test]
    fn structured_conditions_ignore_commuting_variant_order() {
        let first = Condition {
            breakpoint: Some(BreakpointId::new(2)),
            container_breakpoint: None,
            theme: ThemeMode::Dark,
            states: StateSet::from_state(PseudoState::Hover).with(PseudoState::FocusVisible),
            ..Condition::BASE
        };
        let second = Condition {
            breakpoint: Some(BreakpointId::new(2)),
            container_breakpoint: None,
            theme: ThemeMode::Dark,
            states: StateSet::from_state(PseudoState::FocusVisible).with(PseudoState::Hover),
            ..Condition::BASE
        };

        assert_eq!(first, second);
        assert!(first.refines(&Condition::BASE));
        assert!(!Condition::BASE.refines(&first));
    }

    #[test]
    fn intern_tables_return_stable_ids() {
        let mut style = SemanticStyle::default();
        let first = style.intern_arbitrary_value("calc(100%-1rem)".to_owned());
        let second = style.intern_arbitrary_value("calc(100%-1rem)".to_owned());

        assert_eq!(first, second);
        assert_eq!(style.arbitrary_values.len(), 1);
    }

    #[test]
    fn v1_class_encoding_preserves_all_identity_bits() {
        assert_eq!(CLASS_NAME_FORMAT_VERSION, 1);
        assert_eq!(StyleId::new(0).to_class_name(), "pc_0");
        assert_eq!(
            StyleId::new(u128::MAX).to_class_name(),
            "pc_f5lxx1zz5pnorynqglhzmsp33"
        );
    }

    #[test]
    fn utility_domains_are_typed() {
        let color = SemanticValue::Token(TokenRef {
            kind: TokenKind::Color,
            id: TokenId::new(1),
        });

        assert!(Utility::BackgroundColor.accepts(color));
        assert!(!Utility::Padding.accepts(color));
        assert!(Utility::GridTemplateColumns.accepts(SemanticValue::Integer(3)));
        assert!(!Utility::Display.accepts(SemanticValue::Integer(3)));
    }

    #[test]
    fn a_well_formed_flat_style_validates() {
        let mut style = SemanticStyle::default();
        style.assignments.push(assignment(
            Utility::Padding,
            SlotSet::PADDING,
            spacing_token(4),
        ));

        assert_eq!(style.validate(), Ok(()));
    }

    #[test]
    fn validation_rejects_the_first_non_adjacent_duplicate_assignment() {
        let duplicate = assignment(Utility::Padding, SlotSet::PADDING, spacing_token(4));
        let mut style = SemanticStyle::default();
        style.assignments.push(duplicate);
        style.assignments.push(assignment(
            Utility::Margin,
            SlotSet::MARGIN,
            spacing_token(4),
        ));
        style.assignments.push(duplicate);

        let error = style.validate().expect_err("exact duplicate must fail");
        assert_eq!(error.violation, InvariantViolation::DuplicateAssignment);
        assert_eq!(error.location, InvariantLocation::Assignment(2));
    }

    #[test]
    fn validation_reports_the_first_non_adjacent_duplicate_table_entry() {
        let style = SemanticStyle {
            arbitrary_values: vec![
                "first".to_owned(),
                "second".to_owned(),
                "first".to_owned(),
                "second".to_owned(),
            ],
            ..SemanticStyle::default()
        };

        let error = style
            .validate()
            .expect_err("duplicate table entry must fail");
        assert_eq!(error.violation, InvariantViolation::DuplicateArbitraryValue);
        assert_eq!(error.location, InvariantLocation::TableEntry(2));
    }

    #[test]
    fn validation_scales_to_65k_assignments_and_table_entries() {
        const VALIDATION_SCALE: u32 = u16::MAX as u32;

        let mut style = SemanticStyle::default();
        style.assignments.reserve(VALIDATION_SCALE as usize);
        style.arbitrary_values.reserve(VALIDATION_SCALE as usize);
        for start in 0..VALIDATION_SCALE {
            let mut entry = assignment(Utility::Padding, SlotSet::PADDING, spacing_token(4));
            entry.source = Span::new(start, start + 1);
            style.assignments.push(entry);
            style.arbitrary_values.push(format!("value-{start}"));
        }

        assert_eq!(style.assignments.len(), VALIDATION_SCALE as usize);
        assert_eq!(style.arbitrary_values.len(), VALIDATION_SCALE as usize);
        assert_eq!(style.validate(), Ok(()));
    }

    #[test]
    fn validation_rejects_slots_outside_the_utility() {
        let mut style = SemanticStyle::default();
        style.assignments.push(assignment(
            Utility::Padding,
            SlotSet::from_slot(Slot::DisplayMode),
            spacing_token(4),
        ));

        let error = style.validate().expect_err("wrong slot must fail");
        assert_eq!(error.violation, InvariantViolation::SlotOutsideUtility);
        assert_eq!(error.location, InvariantLocation::Assignment(0));
    }

    #[test]
    fn validation_rejects_dangling_ids() {
        let mut style = SemanticStyle::default();
        style.assignments.push(assignment(
            Utility::Width,
            SlotSet::from_slot(Slot::Width),
            SemanticValue::Arbitrary(ArbitraryValueId::new(7)),
        ));

        let error = style.validate().expect_err("dangling ID must fail");
        assert_eq!(
            error.violation,
            InvariantViolation::ArbitraryValueOutOfBounds
        );
    }

    #[test]
    fn validation_requires_reduced_nonzero_fractions() {
        let mut style = SemanticStyle::default();
        style.assignments.push(assignment(
            Utility::Width,
            SlotSet::from_slot(Slot::Width),
            SemanticValue::Fraction {
                numerator: 2,
                denominator: 4,
            },
        ));

        let error = style.validate().expect_err("fraction must be reduced");
        assert_eq!(error.violation, InvariantViolation::InvalidFraction);
    }

    #[test]
    fn validation_enforces_negative_families() {
        let mut invalid = assignment(Utility::Padding, SlotSet::PADDING, spacing_token(4));
        invalid.negative = true;
        let mut style = SemanticStyle::default();
        style.assignments.push(invalid);

        assert_eq!(
            style
                .validate()
                .expect_err("padding cannot be negative")
                .violation,
            InvariantViolation::NegativeUnsupported
        );

        let mut valid = SemanticStyle::default();
        let mut margin = assignment(Utility::Margin, SlotSet::MARGIN, spacing_token(4));
        margin.negative = true;
        valid.assignments.push(margin);
        assert_eq!(valid.validate(), Ok(()));
    }

    #[test]
    fn arbitrary_properties_use_an_explicit_interned_identity() {
        let mut style = SemanticStyle::default();
        let property = style.intern_arbitrary_property(ArbitraryProperty {
            name: "mask-type".to_owned(),
        });
        let value = style.intern_arbitrary_value("luminance".to_owned());
        style.assignments.push(assignment(
            Utility::ArbitraryProperty(property),
            SlotSet::from_slot(Slot::ArbitraryProperty),
            SemanticValue::Arbitrary(value),
        ));

        assert_eq!(style.validate(), Ok(()));
    }
}
