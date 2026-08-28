//! Deterministic design-token and breakpoint registries for `PliegoCSS`.
//!
//! Theme identities and binary bytes are versioned interoperability contracts. The Rust registry
//! construction API remains an exact-version advanced tooling surface for the candidate.

#![forbid(unsafe_code)]

use core::fmt;
use std::collections::BTreeMap;

use pliego_css_ir::{
    BreakpointId, CASCADE_LAYER_VARIANTS, CLASSIFIED_SELECTOR_VARIANTS, TokenId, TokenKind,
};
use sha2::{Digest, Sha256};

mod binary;

pub use binary::{
    MAX_DEFINITION_COUNT, MAX_TEXT_BYTES, MAX_THEME_BINARY_BYTES, THEME_BINARY_FORMAT_VERSION,
    THEME_BINARY_MAGIC, ThemeBinaryError,
};

/// Version of the canonical byte stream used to derive [`ThemeId`].
pub const THEME_ID_FORMAT_VERSION: u16 = 3;

const THEME_ID_STREAM_DOMAIN: &[u8; 16] = b"pliego-theme-id\0";
const THEME_ID_TOKENS: u8 = 0x01;
const THEME_ID_TOKEN: u8 = 0x02;
const THEME_ID_BREAKPOINTS: u8 = 0x03;
const THEME_ID_BREAKPOINT: u8 = 0x04;

/// Stable identity of a canonical theme registry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ThemeId(u128);

impl ThemeId {
    /// Creates an identity from its compact representation.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the compact representation.
    #[must_use]
    pub const fn get(self) -> u128 {
        self.0
    }
}

impl fmt::Display for ThemeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:032x}", self.0)
    }
}

/// One named value in a typed design-token namespace.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TokenDefinition {
    /// Token namespace.
    pub kind: TokenKind,
    /// Stable name-derived identifier inside the namespace.
    pub id: TokenId,
    /// Human-readable token name.
    pub name: String,
    /// Canonical CSS value or design-system expression.
    pub value: String,
}

impl TokenDefinition {
    /// Defines a token and derives its stable 32-bit lookup ID from `name`.
    pub fn new(kind: TokenKind, name: impl Into<String>, value: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            kind,
            id: token_id(&name),
            name,
            value: value.into(),
        }
    }

    /// Defines a token with an explicit ID.
    ///
    /// This supports persisted registries and makes ID collisions detectable at
    /// the registry boundary instead of silently selecting one definition.
    pub fn with_id(
        kind: TokenKind,
        id: TokenId,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            id,
            name: name.into(),
            value: value.into(),
        }
    }
}

/// One named responsive breakpoint.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BreakpointDefinition {
    /// Stable breakpoint identifier.
    pub id: BreakpointId,
    /// Total cascade order, from narrowest to widest.
    ///
    /// This is intentionally independent from [`Self::id`]: stable lookup
    /// identity must not determine responsive precedence.
    pub cascade_rank: u16,
    /// Human-readable variant name.
    pub name: String,
    /// Canonical CSS minimum width.
    pub min_width: String,
}

impl BreakpointDefinition {
    /// Defines a responsive breakpoint.
    pub fn new(
        id: BreakpointId,
        cascade_rank: u16,
        name: impl Into<String>,
        min_width: impl Into<String>,
    ) -> Self {
        Self {
            id,
            cascade_rank,
            name: name.into(),
            min_width: min_width.into(),
        }
    }
}

/// Fatal validation error while constructing a theme registry.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThemeError {
    /// A token repeats a `(kind, name)` key.
    DuplicateToken {
        /// Repeated token namespace.
        kind: TokenKind,
        /// Repeated token name.
        name: String,
    },
    /// Two distinct names map to one token ID in a namespace.
    TokenIdCollision {
        /// Colliding token namespace.
        kind: TokenKind,
        /// Colliding token ID.
        id: TokenId,
        /// Lexicographically first colliding name.
        first: String,
        /// Lexicographically second colliding name.
        second: String,
    },
    /// A breakpoint name appears more than once.
    DuplicateBreakpointName {
        /// Repeated breakpoint name.
        name: String,
    },
    /// A breakpoint ID is assigned to two names.
    DuplicateBreakpointId {
        /// Repeated breakpoint ID.
        id: BreakpointId,
        /// Lexicographically first name assigned to the ID.
        first: String,
        /// Lexicographically second name assigned to the ID.
        second: String,
    },
    /// A cascade rank is assigned to two breakpoints.
    DuplicateBreakpointCascadeRank {
        /// Repeated cascade rank.
        rank: u16,
        /// Lexicographically first name assigned to the rank.
        first: String,
        /// Lexicographically second name assigned to the rank.
        second: String,
    },
    /// A breakpoint attempts to shadow a built-in non-responsive condition.
    ReservedBreakpointName {
        /// Reserved condition name.
        name: String,
    },
    /// A definition contains an empty field or a NUL byte.
    InvalidDefinition {
        /// Stable description of the invalid field.
        field: &'static str,
        /// Name of the affected definition, when available.
        name: String,
    },
}

impl fmt::Display for ThemeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateToken { kind, name } => {
                write!(formatter, "duplicate {kind:?} token `{name}`")
            }
            Self::TokenIdCollision {
                kind,
                id,
                first,
                second,
            } => write!(
                formatter,
                "fatal {kind:?} token ID collision {} between `{first}` and `{second}`",
                id.get()
            ),
            Self::DuplicateBreakpointName { name } => {
                write!(formatter, "duplicate breakpoint name `{name}`")
            }
            Self::DuplicateBreakpointId { id, first, second } => write!(
                formatter,
                "duplicate breakpoint ID {} assigned to `{first}` and `{second}`",
                id.get()
            ),
            Self::DuplicateBreakpointCascadeRank {
                rank,
                first,
                second,
            } => write!(
                formatter,
                "duplicate breakpoint cascade rank {rank} assigned to `{first}` and `{second}`"
            ),
            Self::ReservedBreakpointName { name } => write!(
                formatter,
                "breakpoint name `{name}` is reserved by a built-in condition variant"
            ),
            Self::InvalidDefinition { field, name } => {
                write!(formatter, "invalid {field} in theme definition `{name}`")
            }
        }
    }
}

impl std::error::Error for ThemeError {}

/// Canonical, collision-checked token and breakpoint registry.
#[derive(Clone, Debug)]
pub struct ThemeRegistry {
    id: ThemeId,
    tokens: Vec<TokenDefinition>,
    breakpoints: Vec<BreakpointDefinition>,
    token_names: BTreeMap<TokenKind, BTreeMap<String, usize>>,
    token_ids: BTreeMap<TokenKind, BTreeMap<TokenId, usize>>,
    breakpoint_names: BTreeMap<String, usize>,
    breakpoint_ids: BTreeMap<BreakpointId, usize>,
}

impl ThemeRegistry {
    /// Constructs, validates, and canonicalizes a registry.
    ///
    /// Input order does not affect storage order or [`ThemeId`].
    ///
    /// # Errors
    ///
    /// Returns a fatal error for malformed fields, duplicate keys, or ID
    /// collisions. Token IDs are scoped by [`TokenKind`].
    pub fn from_definitions(
        tokens: impl IntoIterator<Item = TokenDefinition>,
        breakpoints: impl IntoIterator<Item = BreakpointDefinition>,
    ) -> Result<Self, ThemeError> {
        let mut tokens = tokens.into_iter().collect::<Vec<_>>();
        let mut breakpoints = breakpoints.into_iter().collect::<Vec<_>>();
        tokens.sort();
        breakpoints.sort();

        validate_fields(&tokens, &breakpoints)?;

        let mut token_names: BTreeMap<TokenKind, BTreeMap<String, usize>> = BTreeMap::new();
        let mut token_ids: BTreeMap<TokenKind, BTreeMap<TokenId, usize>> = BTreeMap::new();
        for (index, token) in tokens.iter().enumerate() {
            if token_names
                .entry(token.kind)
                .or_default()
                .insert(token.name.clone(), index)
                .is_some()
            {
                return Err(ThemeError::DuplicateToken {
                    kind: token.kind,
                    name: token.name.clone(),
                });
            }
            if let Some(previous) = token_ids
                .entry(token.kind)
                .or_default()
                .insert(token.id, index)
            {
                return Err(ThemeError::TokenIdCollision {
                    kind: token.kind,
                    id: token.id,
                    first: tokens[previous].name.clone(),
                    second: token.name.clone(),
                });
            }
        }

        let mut breakpoint_names = BTreeMap::new();
        let mut breakpoint_ids = BTreeMap::new();
        let mut breakpoint_ranks = BTreeMap::new();
        for (index, breakpoint) in breakpoints.iter().enumerate() {
            if is_reserved_condition_variant(&breakpoint.name) {
                return Err(ThemeError::ReservedBreakpointName {
                    name: breakpoint.name.clone(),
                });
            }
            if breakpoint_names
                .insert(breakpoint.name.clone(), index)
                .is_some()
            {
                return Err(ThemeError::DuplicateBreakpointName {
                    name: breakpoint.name.clone(),
                });
            }
            if let Some(previous) = breakpoint_ids.insert(breakpoint.id, index) {
                return Err(ThemeError::DuplicateBreakpointId {
                    id: breakpoint.id,
                    first: breakpoints[previous].name.clone(),
                    second: breakpoint.name.clone(),
                });
            }
            if let Some(previous) = breakpoint_ranks.insert(breakpoint.cascade_rank, index) {
                return Err(ThemeError::DuplicateBreakpointCascadeRank {
                    rank: breakpoint.cascade_rank,
                    first: breakpoints[previous].name.clone(),
                    second: breakpoint.name.clone(),
                });
            }
        }

        let id = derive_theme_id(&tokens, &breakpoints);
        Ok(Self {
            id,
            tokens,
            breakpoints,
            token_names,
            token_ids,
            breakpoint_names,
            breakpoint_ids,
        })
    }

    /// Returns the built-in Gate-A registry.
    ///
    /// # Panics
    ///
    /// Panics only if the compile-time-owned seed definitions violate registry
    /// invariants, which indicates a bug in this crate.
    #[must_use]
    pub fn seed() -> Self {
        Self::from_definitions(seed_tokens(), seed_breakpoints())
            .expect("built-in theme definitions must be valid")
    }

    /// Returns the versioned identity of this registry.
    #[must_use]
    pub const fn id(&self) -> ThemeId {
        self.id
    }

    /// Returns canonical token definitions.
    #[must_use]
    pub fn tokens(&self) -> &[TokenDefinition] {
        &self.tokens
    }

    /// Returns canonical breakpoint definitions.
    #[must_use]
    pub fn breakpoints(&self) -> &[BreakpointDefinition] {
        &self.breakpoints
    }

    /// Looks up a token by namespace and name.
    #[must_use]
    pub fn token_by_name(&self, kind: TokenKind, name: &str) -> Option<&TokenDefinition> {
        let index = self.token_names.get(&kind)?.get(name)?;
        self.tokens.get(*index)
    }

    /// Looks up a token by namespace and stable ID.
    #[must_use]
    pub fn token_by_id(&self, kind: TokenKind, id: TokenId) -> Option<&TokenDefinition> {
        let index = self.token_ids.get(&kind)?.get(&id)?;
        self.tokens.get(*index)
    }

    /// Looks up a breakpoint by name.
    #[must_use]
    pub fn breakpoint_by_name(&self, name: &str) -> Option<&BreakpointDefinition> {
        let index = self.breakpoint_names.get(name)?;
        self.breakpoints.get(*index)
    }

    /// Looks up a breakpoint by stable ID.
    #[must_use]
    pub fn breakpoint_by_id(&self, id: BreakpointId) -> Option<&BreakpointDefinition> {
        let index = self.breakpoint_ids.get(&id)?;
        self.breakpoints.get(*index)
    }

    /// Encodes the complete registry into the canonical, versioned binary format.
    ///
    /// # Errors
    ///
    /// Returns [`ThemeBinaryError::LimitExceeded`] if a registry exceeds the
    /// format's defensive size limits.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ThemeBinaryError> {
        binary::encode(self)
    }

    /// Decodes and validates a complete registry from canonical binary data.
    ///
    /// Decoding rejects unknown versions, malformed or trailing data, excessive
    /// lengths, invalid UTF-8, invalid registry definitions, and identity
    /// mismatches.
    ///
    /// # Errors
    ///
    /// Returns a precise [`ThemeBinaryError`] for the first invalid field.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ThemeBinaryError> {
        binary::decode(bytes)
    }
}

fn is_reserved_condition_variant(name: &str) -> bool {
    name.starts_with("cq-")
        || CASCADE_LAYER_VARIANTS
            .iter()
            .any(|(variant, _)| name == *variant)
        || CLASSIFIED_SELECTOR_VARIANTS
            .iter()
            .any(|(variant, _)| name == *variant)
        || matches!(
            name,
            "dark"
                | "light"
                | "motion-safe"
                | "motion-reduce"
                | "contrast-more"
                | "contrast-less"
                | "hover"
                | "focus"
                | "focus-visible"
                | "active"
                | "disabled"
                | "placeholder"
        )
}

/// Derives the same FNV-1a 32-bit token ID currently carried by semantic IR.
#[must_use]
pub const fn token_id(name: &str) -> TokenId {
    let bytes = name.as_bytes();
    let mut hash = 2_166_136_261_u32;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u32;
        hash = hash.wrapping_mul(16_777_619);
        index += 1;
    }
    TokenId::new(hash)
}

fn validate_fields(
    tokens: &[TokenDefinition],
    breakpoints: &[BreakpointDefinition],
) -> Result<(), ThemeError> {
    for token in tokens {
        validate_name("token name", &token.name)?;
        validate_css_fragment("token value", &token.value, &token.name)?;
        if !valid_token_value(token.kind, &token.name, &token.value) {
            return Err(ThemeError::InvalidDefinition {
                field: "typed token value",
                name: token.name.clone(),
            });
        }
    }
    for breakpoint in breakpoints {
        validate_name("breakpoint name", &breakpoint.name)?;
        validate_breakpoint_value(
            "breakpoint minimum width",
            &breakpoint.min_width,
            &breakpoint.name,
        )?;
    }
    Ok(())
}

fn validate_name(field: &'static str, name: &str) -> Result<(), ThemeError> {
    let mut previous_hyphen = false;
    let mut valid = !name.is_empty() && !name.starts_with('-') && !name.ends_with('-');
    for byte in name.bytes() {
        if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            || (byte == b'-' && previous_hyphen)
        {
            valid = false;
            break;
        }
        previous_hyphen = byte == b'-';
    }
    if valid {
        Ok(())
    } else {
        Err(ThemeError::InvalidDefinition {
            field,
            name: name.to_owned(),
        })
    }
}

fn validate_css_fragment(field: &'static str, value: &str, name: &str) -> Result<(), ThemeError> {
    let valid = !value.is_empty()
        && !value.chars().any(char::is_control)
        && !value.contains([';', '{', '}'])
        && !value.contains("/*")
        && !value.contains("*/")
        && !value.to_ascii_lowercase().contains("!important");
    if valid {
        Ok(())
    } else {
        Err(ThemeError::InvalidDefinition {
            field,
            name: name.to_owned(),
        })
    }
}

fn validate_breakpoint_value(
    field: &'static str,
    value: &str,
    name: &str,
) -> Result<(), ThemeError> {
    validate_css_fragment(field, value, name)?;
    let split = value
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    if valid_positive_decimal(number) && matches!(unit, "px" | "rem" | "em" | "ch" | "vw") {
        Ok(())
    } else {
        Err(ThemeError::InvalidDefinition {
            field,
            name: name.to_owned(),
        })
    }
}

fn valid_positive_decimal(value: &str) -> bool {
    if value.is_empty() || value.ends_with('.') {
        return false;
    }
    let mut dots = 0_u8;
    let mut digits = 0_usize;
    for byte in value.bytes() {
        if byte == b'.' {
            dots += 1;
            if dots > 1 {
                return false;
            }
        } else if !byte.is_ascii_digit() {
            return false;
        } else {
            digits += 1;
        }
    }
    digits > 0
        && value
            .parse::<f64>()
            .is_ok_and(|number| number.is_finite() && number > 0.0)
}

fn valid_token_value(kind: TokenKind, name: &str, value: &str) -> bool {
    if kind == TokenKind::Color {
        match name {
            "transparent" => return value == "transparent",
            "current" => return value == "currentColor",
            _ => {}
        }
    }
    match kind {
        TokenKind::Spacing | TokenKind::FontSize | TokenKind::Radius => valid_length(value, false),
        TokenKind::Color => valid_color(value),
        TokenKind::FontFamily => value.chars().any(char::is_alphabetic),
        TokenKind::FontWeight => {
            matches!(value, "normal" | "bold" | "bolder" | "lighter")
                || value
                    .parse::<u16>()
                    .is_ok_and(|weight| (1..=1000).contains(&weight))
        }
        TokenKind::LineHeight => {
            value == "normal" || valid_positive_decimal(value) || valid_length(value, false)
        }
        TokenKind::LetterSpacing => value == "normal" || valid_length(value, true),
        TokenKind::Shadow => {
            let mut parts = value.split_ascii_whitespace();
            let first = parts.next();
            value == "none"
                || first.is_some_and(|part| {
                    valid_length(part, true)
                        || (part == "inset"
                            && parts.next().is_some_and(|next| valid_length(next, true)))
                })
        }
        TokenKind::ZIndex => value == "auto" || value.parse::<i32>().is_ok(),
    }
}

fn valid_color(value: &str) -> bool {
    if let Some(hex) = value.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8)
            && hex.bytes().all(|byte| byte.is_ascii_hexdigit());
    }
    if value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return true;
    }
    valid_css_function(
        value,
        &[
            "rgb",
            "rgba",
            "hsl",
            "hsla",
            "hwb",
            "lab",
            "lch",
            "oklab",
            "oklch",
            "color",
            "color-mix",
            "light-dark",
            "var",
        ],
    )
}

fn valid_length(value: &str, allow_negative: bool) -> bool {
    if valid_css_function(value, &["calc", "clamp", "min", "max", "var"]) {
        return true;
    }
    let split = value
        .char_indices()
        .find(|(index, character)| {
            !character.is_ascii_digit()
                && *character != '.'
                && !(*index == 0 && allow_negative && matches!(*character, '-' | '+'))
        })
        .map_or(value.len(), |(index, _)| index);
    let (number, unit) = value.split_at(split);
    if !valid_decimal(number, allow_negative) {
        return false;
    }
    if unit.is_empty() {
        return number.parse::<f64>() == Ok(0.0);
    }
    matches!(
        unit,
        "%" | "px"
            | "rem"
            | "em"
            | "ch"
            | "vw"
            | "vh"
            | "vmin"
            | "vmax"
            | "svh"
            | "lvh"
            | "dvh"
            | "cm"
            | "mm"
            | "in"
            | "pt"
            | "pc"
            | "ex"
            | "cap"
            | "ic"
            | "lh"
            | "rlh"
    )
}

fn valid_decimal(value: &str, allow_negative: bool) -> bool {
    let unsigned = if allow_negative {
        value
            .strip_prefix('-')
            .or_else(|| value.strip_prefix('+'))
            .unwrap_or(value)
    } else {
        value
    };
    if unsigned.is_empty() || unsigned.ends_with('.') {
        return false;
    }
    let mut dots = 0_u8;
    let mut digits = 0_usize;
    for byte in unsigned.bytes() {
        if byte == b'.' {
            dots += 1;
            if dots > 1 {
                return false;
            }
        } else if byte.is_ascii_digit() {
            digits += 1;
        } else {
            return false;
        }
    }
    digits > 0 && value.parse::<f64>().is_ok_and(f64::is_finite)
}

fn valid_css_function(value: &str, names: &[&str]) -> bool {
    let Some((name, body)) = value.split_once('(') else {
        return false;
    };
    if !names.contains(&name) || !value.ends_with(')') || body.is_empty() {
        return false;
    }
    let mut depth = 1_i32;
    for character in body[..body.len() - 1].chars() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 1
}

fn derive_theme_id(tokens: &[TokenDefinition], breakpoints: &[BreakpointDefinition]) -> ThemeId {
    let digest = Sha256::digest(encode_theme_identity(tokens, breakpoints));
    let mut truncated = [0_u8; 16];
    truncated.copy_from_slice(&digest[..16]);
    let value = u128::from_be_bytes(truncated);
    ThemeId::new(if value == 0 { 1 } else { value })
}

fn encode_theme_identity(
    tokens: &[TokenDefinition],
    breakpoints: &[BreakpointDefinition],
) -> Vec<u8> {
    let mut stream = Vec::new();
    stream.extend_from_slice(THEME_ID_STREAM_DOMAIN);
    stream.extend_from_slice(&THEME_ID_FORMAT_VERSION.to_be_bytes());
    stream.push(THEME_ID_TOKENS);
    push_identity_count(&mut stream, tokens.len());
    for token in tokens {
        stream.push(THEME_ID_TOKEN);
        stream.push(token_kind_tag(token.kind));
        stream.extend_from_slice(&token.id.get().to_be_bytes());
        push_identity_text(&mut stream, &token.name);
        push_identity_text(&mut stream, &token.value);
    }
    stream.push(THEME_ID_BREAKPOINTS);
    push_identity_count(&mut stream, breakpoints.len());
    for breakpoint in breakpoints {
        stream.push(THEME_ID_BREAKPOINT);
        stream.extend_from_slice(&breakpoint.id.get().to_be_bytes());
        stream.extend_from_slice(&breakpoint.cascade_rank.to_be_bytes());
        push_identity_text(&mut stream, &breakpoint.name);
        push_identity_text(&mut stream, &breakpoint.min_width);
    }
    stream
}

fn push_identity_count(stream: &mut Vec<u8>, count: usize) {
    let count = u32::try_from(count).expect("validated theme definition count must fit in u32");
    stream.extend_from_slice(&count.to_be_bytes());
}

fn push_identity_text(stream: &mut Vec<u8>, text: &str) {
    let length = u32::try_from(text.len()).expect("validated theme text length must fit in u32");
    stream.extend_from_slice(&length.to_be_bytes());
    stream.extend_from_slice(text.as_bytes());
}

pub(crate) const fn token_kind_tag(kind: TokenKind) -> u8 {
    match kind {
        TokenKind::Spacing => 0,
        TokenKind::Color => 1,
        TokenKind::FontFamily => 2,
        TokenKind::FontSize => 3,
        TokenKind::FontWeight => 4,
        TokenKind::LineHeight => 5,
        TokenKind::LetterSpacing => 6,
        TokenKind::Radius => 7,
        TokenKind::Shadow => 8,
        TokenKind::ZIndex => 9,
    }
}

// Keeping the seed values together makes drift against the compiler catalog visible.
#[allow(clippy::too_many_lines)]
fn seed_tokens() -> Vec<TokenDefinition> {
    let mut definitions = Vec::new();
    extend_tokens(
        &mut definitions,
        TokenKind::Spacing,
        &[
            ("0", "0"),
            ("1", ".25rem"),
            ("2", ".5rem"),
            ("3", ".75rem"),
            ("4", "1rem"),
            ("5", "1.25rem"),
            ("6", "1.5rem"),
            ("8", "2rem"),
            ("12", "3rem"),
            ("28", "7rem"),
            ("40", "10rem"),
            ("full", "100%"),
            ("screen", "100vw"),
            ("prose", "65ch"),
            ("6xl", "72rem"),
        ],
    );
    extend_tokens(
        &mut definitions,
        TokenKind::Color,
        &[
            ("transparent", "transparent"),
            ("current", "currentColor"),
            ("white", "#fff"),
            ("canvas", "oklch(98.5% .004 80)"),
            ("surface", "#fff"),
            ("surface-raised", "oklch(96% .006 80)"),
            ("ink", "oklch(19% .012 70)"),
            ("muted", "oklch(50% .014 70)"),
            ("accent", "oklch(58% .19 32)"),
            ("accent-strong", "oklch(50% .18 30)"),
            ("line", "oklch(88% .008 75)"),
        ],
    );
    extend_tokens(
        &mut definitions,
        TokenKind::FontFamily,
        &[
            ("sans", "ui-sans-serif,system-ui,sans-serif"),
            ("mono", "ui-monospace,SFMono-Regular,monospace"),
        ],
    );
    extend_tokens(
        &mut definitions,
        TokenKind::FontSize,
        &[
            ("xs", ".75rem"),
            ("sm", ".875rem"),
            ("base", "1rem"),
            ("lg", "1.125rem"),
            ("xl", "1.25rem"),
            ("2xl", "1.5rem"),
            ("3xl", "1.875rem"),
        ],
    );
    extend_tokens(
        &mut definitions,
        TokenKind::FontWeight,
        &[
            ("normal", "400"),
            ("medium", "500"),
            ("semibold", "600"),
            ("bold", "700"),
        ],
    );
    extend_tokens(
        &mut definitions,
        TokenKind::LineHeight,
        &[("tight", "1.25"), ("normal", "1.5"), ("6", "1.5rem")],
    );
    extend_tokens(
        &mut definitions,
        TokenKind::LetterSpacing,
        &[("tight", "-.025em")],
    );
    extend_tokens(
        &mut definitions,
        TokenKind::Radius,
        &[
            ("sm", ".25rem"),
            ("md", ".375rem"),
            ("lg", ".5rem"),
            ("xl", ".75rem"),
            ("2xl", "1rem"),
            ("full", "9999px"),
        ],
    );
    extend_tokens(
        &mut definitions,
        TokenKind::Shadow,
        &[
            ("sm", "0 1px 2px #0000000d"),
            ("md", "0 4px 6px -1px #0000001a"),
        ],
    );
    definitions
}

fn extend_tokens(output: &mut Vec<TokenDefinition>, kind: TokenKind, definitions: &[(&str, &str)]) {
    output.extend(
        definitions
            .iter()
            .map(|(name, value)| TokenDefinition::new(kind, *name, *value)),
    );
}

fn seed_breakpoints() -> Vec<BreakpointDefinition> {
    vec![
        BreakpointDefinition::new(BreakpointId::new(0), 0, "sm", "40rem"),
        BreakpointDefinition::new(BreakpointId::new(1), 1, "md", "48rem"),
        BreakpointDefinition::new(BreakpointId::new(2), 2, "lg", "64rem"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    const V1_THEME_FIXTURE_HEX: &str = "504c474354484d000001bdcf7d279f16eef34e3be1db98ab3894000000020022aecc820000000667757474657200000006312e3572656d019b21fbf6000000056272616e640000000423333666000000010007000000067461626c657400000005353272656d";

    fn definitions() -> (Vec<TokenDefinition>, Vec<BreakpointDefinition>) {
        (
            vec![
                TokenDefinition::new(TokenKind::Color, "accent", "red"),
                TokenDefinition::new(TokenKind::Spacing, "4", "1rem"),
            ],
            vec![BreakpointDefinition::new(
                BreakpointId::new(0),
                0,
                "sm",
                "40rem",
            )],
        )
    }

    fn decode_hex(source: &str) -> Vec<u8> {
        source
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII hex"), 16)
                    .expect("valid hex byte")
            })
            .collect()
    }

    #[test]
    fn input_reordering_preserves_theme_id_and_canonical_order() {
        let (tokens, breakpoints) = definitions();
        let first = ThemeRegistry::from_definitions(tokens.clone(), breakpoints.clone()).unwrap();
        let second = ThemeRegistry::from_definitions(
            tokens.into_iter().rev(),
            breakpoints.into_iter().rev(),
        )
        .unwrap();

        assert_eq!(first.id(), second.id());
        assert_eq!(first.tokens(), second.tokens());
        assert_eq!(first.breakpoints(), second.breakpoints());
    }

    #[test]
    fn value_change_changes_theme_id() {
        let (tokens, breakpoints) = definitions();
        let first = ThemeRegistry::from_definitions(tokens.clone(), breakpoints.clone()).unwrap();
        let mut changed = tokens;
        changed[0].value = "blue".into();
        let second = ThemeRegistry::from_definitions(changed, breakpoints).unwrap();

        assert_ne!(first.id(), second.id());
    }

    #[test]
    fn rejects_duplicate_token_keys() {
        let token = TokenDefinition::new(TokenKind::Color, "accent", "red");
        let error = ThemeRegistry::from_definitions([token.clone(), token], []).unwrap_err();

        assert_eq!(
            error,
            ThemeError::DuplicateToken {
                kind: TokenKind::Color,
                name: "accent".into(),
            }
        );
    }

    #[test]
    fn rejects_fatal_token_id_collisions() {
        let id = TokenId::new(7);
        let error = ThemeRegistry::from_definitions(
            [
                TokenDefinition::with_id(TokenKind::Color, id, "alpha", "red"),
                TokenDefinition::with_id(TokenKind::Color, id, "beta", "blue"),
            ],
            [],
        )
        .unwrap_err();

        assert_eq!(
            error,
            ThemeError::TokenIdCollision {
                kind: TokenKind::Color,
                id,
                first: "alpha".into(),
                second: "beta".into(),
            }
        );
    }

    #[test]
    fn rejects_duplicate_breakpoint_names_and_ids() {
        let duplicate_name = ThemeRegistry::from_definitions(
            [],
            [
                BreakpointDefinition::new(BreakpointId::new(0), 0, "sm", "40rem"),
                BreakpointDefinition::new(BreakpointId::new(1), 1, "sm", "41rem"),
            ],
        )
        .unwrap_err();
        assert_eq!(
            duplicate_name,
            ThemeError::DuplicateBreakpointName { name: "sm".into() }
        );

        let duplicate_id = ThemeRegistry::from_definitions(
            [],
            [
                BreakpointDefinition::new(BreakpointId::new(0), 0, "small", "40rem"),
                BreakpointDefinition::new(BreakpointId::new(0), 1, "wide", "80rem"),
            ],
        )
        .unwrap_err();
        assert_eq!(
            duplicate_id,
            ThemeError::DuplicateBreakpointId {
                id: BreakpointId::new(0),
                first: "small".into(),
                second: "wide".into(),
            }
        );

        let duplicate_rank = ThemeRegistry::from_definitions(
            [],
            [
                BreakpointDefinition::new(BreakpointId::new(0), 0, "small", "40rem"),
                BreakpointDefinition::new(BreakpointId::new(1), 0, "wide", "80rem"),
            ],
        )
        .unwrap_err();
        assert_eq!(
            duplicate_rank,
            ThemeError::DuplicateBreakpointCascadeRank {
                rank: 0,
                first: "small".into(),
                second: "wide".into(),
            }
        );
    }

    #[test]
    fn rejects_breakpoints_that_shadow_builtin_conditions() {
        for name in ["hover", "aria-expanded", "rtl", "cq-card"] {
            let error = ThemeRegistry::from_definitions(
                [],
                [BreakpointDefinition::new(
                    BreakpointId::new(7),
                    0,
                    name,
                    "50rem",
                )],
            )
            .expect_err("reserved condition name must fail");

            assert_eq!(
                error,
                ThemeError::ReservedBreakpointName { name: name.into() }
            );
        }
    }

    #[test]
    fn rejects_definitions_that_can_escape_generated_css() {
        let unsafe_name = ThemeRegistry::from_definitions(
            [TokenDefinition::new(TokenKind::Color, "brand;color", "red")],
            [],
        )
        .expect_err("unsafe token name must fail");
        assert!(matches!(unsafe_name, ThemeError::InvalidDefinition { .. }));

        let unsafe_value = ThemeRegistry::from_definitions(
            [TokenDefinition::new(
                TokenKind::Color,
                "brand",
                "red;display:block",
            )],
            [],
        )
        .expect_err("unsafe token value must fail");
        assert!(matches!(unsafe_value, ThemeError::InvalidDefinition { .. }));

        for (kind, value) in [
            (TokenKind::Color, "1rem"),
            (TokenKind::Spacing, "red"),
            (TokenKind::FontWeight, "heavy-ish"),
        ] {
            let wrong_domain =
                ThemeRegistry::from_definitions([TokenDefinition::new(kind, "invalid", value)], [])
                    .expect_err("typed token domain must fail");
            assert!(matches!(wrong_domain, ThemeError::InvalidDefinition { .. }));
        }

        let reserved_override = ThemeRegistry::from_definitions(
            [TokenDefinition::new(TokenKind::Color, "transparent", "red")],
            [],
        )
        .expect_err("semantic color keyword cannot be overridden");
        assert!(matches!(
            reserved_override,
            ThemeError::InvalidDefinition { .. }
        ));

        let unsafe_breakpoint = ThemeRegistry::from_definitions(
            [],
            [BreakpointDefinition::new(
                BreakpointId::new(7),
                0,
                "print",
                "40rem), print",
            )],
        )
        .expect_err("media expression must fail");
        assert!(matches!(
            unsafe_breakpoint,
            ThemeError::InvalidDefinition { .. }
        ));
    }

    #[test]
    fn accepts_canonical_names_fragments_and_fractional_breakpoints() {
        let registry = ThemeRegistry::from_definitions(
            [
                TokenDefinition::new(TokenKind::Color, "brand-2", "oklch(56% .18 255)"),
                TokenDefinition::new(TokenKind::Spacing, "gutter", "clamp(1rem, 2vw, 2rem)"),
                TokenDefinition::new(TokenKind::Shadow, "inner", "inset 0 1px 2px #0002"),
            ],
            [BreakpointDefinition::new(
                BreakpointId::new(7),
                0,
                "compact",
                ".5rem",
            )],
        )
        .expect("canonical definitions");

        assert!(
            registry
                .token_by_name(TokenKind::Color, "brand-2")
                .is_some()
        );
        assert!(registry.breakpoint_by_name("compact").is_some());
    }

    #[test]
    fn seed_supports_name_id_and_breakpoint_lookups() {
        let seed = ThemeRegistry::seed();
        let accent = seed
            .token_by_name(TokenKind::Color, "accent")
            .expect("accent token");

        assert_eq!(accent.value, "oklch(58% .19 32)");
        assert_eq!(seed.token_by_id(TokenKind::Color, accent.id), Some(accent));
        assert_eq!(
            seed.breakpoint_by_name("md").map(|item| item.id),
            Some(BreakpointId::new(1))
        );
        assert_eq!(
            seed.breakpoint_by_id(BreakpointId::new(2))
                .map(|item| item.min_width.as_str()),
            Some("64rem")
        );
        assert_eq!(seed.breakpoints().len(), 3);
        assert_eq!(seed.tokens().len(), 51);
    }

    #[test]
    fn v3_theme_identity_stream_digest_and_id_are_frozen() {
        let registry = ThemeRegistry::from_definitions(
            [
                TokenDefinition::new(TokenKind::Color, "brand", "#36f"),
                TokenDefinition::new(TokenKind::Spacing, "gutter", "1.5rem"),
            ],
            [BreakpointDefinition::new(
                BreakpointId::new(7),
                0,
                "tablet",
                "52rem",
            )],
        )
        .expect("build compatibility registry");
        let stream = encode_theme_identity(registry.tokens(), registry.breakpoints());
        let digest = Sha256::digest(&stream);
        let bytes = registry.to_bytes().expect("encode compatibility registry");

        assert_eq!(THEME_ID_FORMAT_VERSION, 3);
        assert_eq!(THEME_BINARY_FORMAT_VERSION, 3);
        assert_eq!(THEME_BINARY_MAGIC, *b"PLGCTHM\0");
        assert_eq!(
            stream,
            decode_hex(
                "706c6965676f2d7468656d652d69640000030100000002020022aecc820000000667757474657200000006312e3572656d02019b21fbf6000000056272616e64000000042333366603000000010400070000000000067461626c657400000005353272656d"
            )
        );
        assert_eq!(
            format!("{digest:x}"),
            "eda25b5ed8fa8ce973662d4f18a46bdce3850281287e24d934a701503b526b9c"
        );
        assert_eq!(
            format!("{:032x}", registry.id().get()),
            "eda25b5ed8fa8ce973662d4f18a46bdc"
        );
        assert_eq!(
            registry.id().to_string(),
            "eda25b5ed8fa8ce973662d4f18a46bdc"
        );
        assert_eq!(
            bytes.len(),
            104,
            "theme binary length must be updated intentionally"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            "d2ce5bd919ba720e3b108fe1479e52c70b0441af708ddd5c7025e16fb2a0f4e0",
            "theme binary hash must be updated intentionally"
        );
        let decoded = ThemeRegistry::from_bytes(&bytes).expect("decode frozen v3 fixture");
        assert_eq!(decoded.id(), registry.id());
        assert_eq!(decoded.tokens(), registry.tokens());
        assert_eq!(decoded.breakpoints(), registry.breakpoints());
    }

    #[test]
    fn v1_theme_binary_is_rejected_explicitly_after_identity_migration() {
        let fixture = decode_hex(V1_THEME_FIXTURE_HEX);
        assert!(matches!(
            ThemeRegistry::from_bytes(&fixture),
            Err(ThemeBinaryError::UnsupportedVersion {
                found: 1,
                expected: 3,
            })
        ));
    }

    #[test]
    fn seed_has_no_token_id_collisions_within_namespaces() {
        let seed = ThemeRegistry::seed();
        let mut registered = BTreeMap::new();
        for token in seed.tokens() {
            assert!(
                registered
                    .insert((token.kind, token.id), token.name.as_str())
                    .is_none(),
                "collision for {:?} {}",
                token.kind,
                token.id.get()
            );
        }
    }
}
