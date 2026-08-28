//! Canonical persisted representation of a validated theme registry.

use core::fmt;

use pliego_css_ir::{BreakpointId, TokenId, TokenKind};

use super::{
    BreakpointDefinition, ThemeError, ThemeId, ThemeRegistry, TokenDefinition, token_kind_tag,
};

/// Magic prefix of a canonical theme registry artifact.
pub const THEME_BINARY_MAGIC: [u8; 8] = *b"PLGCTHM\0";

/// Current canonical theme artifact format version.
pub const THEME_BINARY_FORMAT_VERSION: u16 = 3;

/// Maximum accepted size of one encoded theme artifact (16 MiB).
pub const MAX_THEME_BINARY_BYTES: usize = 16 * 1024 * 1024;

/// Maximum accepted token or breakpoint count.
pub const MAX_DEFINITION_COUNT: usize = 65_536;

/// Maximum accepted UTF-8 byte length of one name or value (1 MiB).
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

/// Error returned while encoding or decoding a canonical theme artifact.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThemeBinaryError {
    /// The artifact exceeds [`MAX_THEME_BINARY_BYTES`].
    ArtifactTooLarge {
        /// Actual artifact length in bytes.
        actual: usize,
        /// Maximum supported artifact length in bytes.
        limit: usize,
    },
    /// A count or text field exceeds its defensive format limit.
    LimitExceeded {
        /// Stable field description.
        field: &'static str,
        /// Actual count or byte length.
        actual: usize,
        /// Maximum supported count or byte length.
        limit: usize,
    },
    /// The artifact ended before a complete field could be read.
    Truncated {
        /// Byte offset at which the field starts.
        offset: usize,
        /// Bytes required for the complete field.
        needed: usize,
        /// Bytes still available at `offset`.
        remaining: usize,
    },
    /// The file does not begin with [`THEME_BINARY_MAGIC`].
    InvalidMagic,
    /// The file uses an unsupported artifact version.
    UnsupportedVersion {
        /// Version found in the file.
        found: u16,
        /// Version accepted by this crate.
        expected: u16,
    },
    /// A token uses an unknown namespace tag.
    UnknownTokenKind {
        /// Unknown byte tag.
        tag: u8,
        /// Byte offset containing the tag.
        offset: usize,
    },
    /// A name or value is not valid UTF-8.
    InvalidUtf8 {
        /// Stable field description.
        field: &'static str,
        /// Byte offset containing the text payload.
        offset: usize,
    },
    /// Valid records were followed by unrecognized bytes.
    TrailingData {
        /// Offset where trailing data begins.
        offset: usize,
        /// Number of trailing bytes.
        remaining: usize,
    },
    /// Records are valid but are not stored in canonical registry order.
    NonCanonical,
    /// Decoded definitions violate registry invariants.
    Registry(ThemeError),
    /// The persisted identity does not match the reconstructed registry.
    ThemeIdMismatch {
        /// Identity stored in the artifact header.
        stored: ThemeId,
        /// Identity recomputed from decoded definitions.
        computed: ThemeId,
    },
}

impl fmt::Display for ThemeBinaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArtifactTooLarge { actual, limit } => write!(
                formatter,
                "theme artifact is {actual} bytes; maximum supported size is {limit} bytes"
            ),
            Self::LimitExceeded {
                field,
                actual,
                limit,
            } => write!(
                formatter,
                "theme artifact {field} is {actual}; maximum supported value is {limit}"
            ),
            Self::Truncated {
                offset,
                needed,
                remaining,
            } => write!(
                formatter,
                "truncated theme artifact at byte {offset}: need {needed} bytes, have {remaining}"
            ),
            Self::InvalidMagic => formatter.write_str("invalid theme artifact magic header"),
            Self::UnsupportedVersion { found, expected } => write!(
                formatter,
                "unsupported theme artifact version {found}; expected {expected}"
            ),
            Self::UnknownTokenKind { tag, offset } => {
                write!(formatter, "unknown token kind tag {tag} at byte {offset}")
            }
            Self::InvalidUtf8 { field, offset } => {
                write!(formatter, "invalid UTF-8 in {field} at byte {offset}")
            }
            Self::TrailingData { offset, remaining } => write!(
                formatter,
                "theme artifact has {remaining} trailing bytes at byte {offset}"
            ),
            Self::NonCanonical => {
                formatter.write_str("theme artifact records are not canonically ordered")
            }
            Self::Registry(source) => write!(formatter, "invalid decoded theme registry: {source}"),
            Self::ThemeIdMismatch { stored, computed } => write!(
                formatter,
                "theme artifact identity mismatch: stored {:032x}, computed {:032x}",
                stored.get(),
                computed.get()
            ),
        }
    }
}

impl std::error::Error for ThemeBinaryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Registry(source) => Some(source),
            Self::ArtifactTooLarge { .. }
            | Self::LimitExceeded { .. }
            | Self::Truncated { .. }
            | Self::InvalidMagic
            | Self::UnsupportedVersion { .. }
            | Self::UnknownTokenKind { .. }
            | Self::InvalidUtf8 { .. }
            | Self::TrailingData { .. }
            | Self::NonCanonical
            | Self::ThemeIdMismatch { .. } => None,
        }
    }
}

impl From<ThemeError> for ThemeBinaryError {
    fn from(error: ThemeError) -> Self {
        Self::Registry(error)
    }
}

pub(super) fn encode(registry: &ThemeRegistry) -> Result<Vec<u8>, ThemeBinaryError> {
    ensure_count("token count", registry.tokens().len())?;
    ensure_count("breakpoint count", registry.breakpoints().len())?;

    let encoded_size = encoded_size(registry)?;
    if encoded_size > MAX_THEME_BINARY_BYTES {
        return Err(ThemeBinaryError::ArtifactTooLarge {
            actual: encoded_size,
            limit: MAX_THEME_BINARY_BYTES,
        });
    }

    let mut output = Vec::with_capacity(encoded_size);
    output.extend_from_slice(&THEME_BINARY_MAGIC);
    output.extend_from_slice(&THEME_BINARY_FORMAT_VERSION.to_be_bytes());
    output.extend_from_slice(&registry.id().get().to_be_bytes());
    push_count(&mut output, registry.tokens().len());
    for token in registry.tokens() {
        output.push(token_kind_tag(token.kind));
        output.extend_from_slice(&token.id.get().to_be_bytes());
        push_text(&mut output, "token name length", &token.name)?;
        push_text(&mut output, "token value length", &token.value)?;
    }
    push_count(&mut output, registry.breakpoints().len());
    for breakpoint in registry.breakpoints() {
        output.extend_from_slice(&breakpoint.id.get().to_be_bytes());
        output.extend_from_slice(&breakpoint.cascade_rank.to_be_bytes());
        push_text(&mut output, "breakpoint name length", &breakpoint.name)?;
        push_text(
            &mut output,
            "breakpoint minimum width length",
            &breakpoint.min_width,
        )?;
    }

    debug_assert_eq!(output.len(), encoded_size);
    Ok(output)
}

pub(super) fn decode(bytes: &[u8]) -> Result<ThemeRegistry, ThemeBinaryError> {
    if bytes.len() > MAX_THEME_BINARY_BYTES {
        return Err(ThemeBinaryError::ArtifactTooLarge {
            actual: bytes.len(),
            limit: MAX_THEME_BINARY_BYTES,
        });
    }

    let mut reader = Reader::new(bytes);
    if reader.read_array::<8>()? != THEME_BINARY_MAGIC {
        return Err(ThemeBinaryError::InvalidMagic);
    }
    let version = u16::from_be_bytes(reader.read_array()?);
    if version != THEME_BINARY_FORMAT_VERSION {
        return Err(ThemeBinaryError::UnsupportedVersion {
            found: version,
            expected: THEME_BINARY_FORMAT_VERSION,
        });
    }
    let stored_id = ThemeId::new(u128::from_be_bytes(reader.read_array()?));

    let token_count = reader.read_count("token count")?;
    let mut tokens = Vec::with_capacity(token_count);
    for _ in 0..token_count {
        let kind_offset = reader.offset();
        let kind_tag = reader.read_u8()?;
        let kind = token_kind_from_tag(kind_tag).ok_or(ThemeBinaryError::UnknownTokenKind {
            tag: kind_tag,
            offset: kind_offset,
        })?;
        let id = TokenId::new(u32::from_be_bytes(reader.read_array()?));
        let name = reader.read_text("token name")?;
        let value = reader.read_text("token value")?;
        tokens.push(TokenDefinition::with_id(kind, id, name, value));
    }

    let breakpoint_count = reader.read_count("breakpoint count")?;
    let mut breakpoints = Vec::with_capacity(breakpoint_count);
    for _ in 0..breakpoint_count {
        let id = BreakpointId::new(u16::from_be_bytes(reader.read_array()?));
        let cascade_rank = u16::from_be_bytes(reader.read_array()?);
        let name = reader.read_text("breakpoint name")?;
        let min_width = reader.read_text("breakpoint minimum width")?;
        breakpoints.push(BreakpointDefinition::new(id, cascade_rank, name, min_width));
    }

    if reader.remaining() != 0 {
        return Err(ThemeBinaryError::TrailingData {
            offset: reader.offset(),
            remaining: reader.remaining(),
        });
    }

    let registry = ThemeRegistry::from_definitions(tokens, breakpoints)?;
    if registry.id() != stored_id {
        return Err(ThemeBinaryError::ThemeIdMismatch {
            stored: stored_id,
            computed: registry.id(),
        });
    }
    if encode(&registry)?.as_slice() != bytes {
        return Err(ThemeBinaryError::NonCanonical);
    }
    Ok(registry)
}

fn encoded_size(registry: &ThemeRegistry) -> Result<usize, ThemeBinaryError> {
    let mut size = THEME_BINARY_MAGIC.len() + 2 + 16 + 4 + 4;
    for token in registry.tokens() {
        ensure_text_length("token name length", token.name.len())?;
        ensure_text_length("token value length", token.value.len())?;
        size = checked_size_add(size, 1 + 4 + 4 + 4)?;
        size = checked_size_add(size, token.name.len())?;
        size = checked_size_add(size, token.value.len())?;
    }
    for breakpoint in registry.breakpoints() {
        ensure_text_length("breakpoint name length", breakpoint.name.len())?;
        ensure_text_length(
            "breakpoint minimum width length",
            breakpoint.min_width.len(),
        )?;
        size = checked_size_add(size, 2 + 2 + 4 + 4)?;
        size = checked_size_add(size, breakpoint.name.len())?;
        size = checked_size_add(size, breakpoint.min_width.len())?;
    }
    Ok(size)
}

fn checked_size_add(current: usize, additional: usize) -> Result<usize, ThemeBinaryError> {
    current
        .checked_add(additional)
        .ok_or(ThemeBinaryError::ArtifactTooLarge {
            actual: usize::MAX,
            limit: MAX_THEME_BINARY_BYTES,
        })
}

fn push_count(output: &mut Vec<u8>, count: usize) {
    let count = u32::try_from(count).expect("validated definition count must fit in u32");
    output.extend_from_slice(&count.to_be_bytes());
}

fn push_text(
    output: &mut Vec<u8>,
    field: &'static str,
    value: &str,
) -> Result<(), ThemeBinaryError> {
    ensure_text_length(field, value.len())?;
    let length = u32::try_from(value.len()).expect("validated text length must fit in u32");
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn ensure_count(field: &'static str, count: usize) -> Result<(), ThemeBinaryError> {
    if count > MAX_DEFINITION_COUNT {
        Err(ThemeBinaryError::LimitExceeded {
            field,
            actual: count,
            limit: MAX_DEFINITION_COUNT,
        })
    } else {
        Ok(())
    }
}

fn ensure_text_length(field: &'static str, length: usize) -> Result<(), ThemeBinaryError> {
    if length > MAX_TEXT_BYTES {
        Err(ThemeBinaryError::LimitExceeded {
            field,
            actual: length,
            limit: MAX_TEXT_BYTES,
        })
    } else {
        Ok(())
    }
}

const fn token_kind_from_tag(tag: u8) -> Option<TokenKind> {
    match tag {
        0 => Some(TokenKind::Spacing),
        1 => Some(TokenKind::Color),
        2 => Some(TokenKind::FontFamily),
        3 => Some(TokenKind::FontSize),
        4 => Some(TokenKind::FontWeight),
        5 => Some(TokenKind::LineHeight),
        6 => Some(TokenKind::LetterSpacing),
        7 => Some(TokenKind::Radius),
        8 => Some(TokenKind::Shadow),
        9 => Some(TokenKind::ZIndex),
        _ => None,
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    const fn offset(&self) -> usize {
        self.offset
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn read_u8(&mut self) -> Result<u8, ThemeBinaryError> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_array<const LENGTH: usize>(&mut self) -> Result<[u8; LENGTH], ThemeBinaryError> {
        let bytes = self.read_exact(LENGTH)?;
        let mut output = [0; LENGTH];
        output.copy_from_slice(bytes);
        Ok(output)
    }

    fn read_count(&mut self, field: &'static str) -> Result<usize, ThemeBinaryError> {
        let count = usize::try_from(u32::from_be_bytes(self.read_array()?))
            .expect("u32 must fit usize on supported targets");
        ensure_count(field, count)?;
        Ok(count)
    }

    fn read_text(&mut self, field: &'static str) -> Result<String, ThemeBinaryError> {
        let length = usize::try_from(u32::from_be_bytes(self.read_array()?))
            .expect("u32 must fit usize on supported targets");
        ensure_text_length(field, length)?;
        let offset = self.offset;
        let bytes = self.read_exact(length)?;
        let value = std::str::from_utf8(bytes)
            .map_err(|_| ThemeBinaryError::InvalidUtf8 { field, offset })?;
        Ok(value.to_owned())
    }

    fn read_exact(&mut self, length: usize) -> Result<&'a [u8], ThemeBinaryError> {
        let remaining = self.remaining();
        if remaining < length {
            return Err(ThemeBinaryError::Truncated {
                offset: self.offset,
                needed: length,
                remaining,
            });
        }
        let start = self.offset;
        self.offset += length;
        Ok(&self.bytes[start..self.offset])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom_registry(reverse: bool) -> ThemeRegistry {
        let mut tokens = vec![
            TokenDefinition::new(TokenKind::Color, "brand", "#36f"),
            TokenDefinition::new(TokenKind::Spacing, "gutter", "1.5rem"),
        ];
        let mut breakpoints = vec![
            BreakpointDefinition::new(BreakpointId::new(4), 4, "wide", "80rem"),
            BreakpointDefinition::new(BreakpointId::new(3), 3, "tablet", "52rem"),
        ];
        if reverse {
            tokens.reverse();
            breakpoints.reverse();
        }
        ThemeRegistry::from_definitions(tokens, breakpoints).expect("custom registry")
    }

    fn assert_same_registry(first: &ThemeRegistry, second: &ThemeRegistry) {
        assert_eq!(first.id(), second.id());
        assert_eq!(first.tokens(), second.tokens());
        assert_eq!(first.breakpoints(), second.breakpoints());
    }

    #[test]
    fn seed_and_custom_registries_round_trip() {
        for registry in [ThemeRegistry::seed(), custom_registry(false)] {
            let decoded = decode(&encode(&registry).expect("encode")).expect("decode");
            assert_same_registry(&registry, &decoded);
        }
    }

    #[test]
    fn canonical_order_produces_identical_bytes() {
        let first = custom_registry(false);
        let reordered = custom_registry(true);

        assert_eq!(encode(&first).unwrap(), encode(&reordered).unwrap());
    }

    #[test]
    fn decoder_rejects_valid_records_in_noncanonical_order() {
        let registry = custom_registry(false);
        let canonical = encode(&registry).unwrap();
        let first_start = THEME_BINARY_MAGIC.len() + 2 + 16 + 4;
        let first_end = token_record_end(&canonical, first_start);
        let second_end = token_record_end(&canonical, first_end);
        let mut reordered = Vec::with_capacity(canonical.len());
        reordered.extend_from_slice(&canonical[..first_start]);
        reordered.extend_from_slice(&canonical[first_end..second_end]);
        reordered.extend_from_slice(&canonical[first_start..first_end]);
        reordered.extend_from_slice(&canonical[second_end..]);

        assert!(matches!(
            decode(&reordered),
            Err(ThemeBinaryError::NonCanonical)
        ));
    }

    fn token_record_end(bytes: &[u8], start: usize) -> usize {
        let name_length_offset = start + 1 + 4;
        let name_length = usize::try_from(u32::from_be_bytes(
            bytes[name_length_offset..name_length_offset + 4]
                .try_into()
                .unwrap(),
        ))
        .unwrap();
        let value_length_offset = name_length_offset + 4 + name_length;
        let value_length = usize::try_from(u32::from_be_bytes(
            bytes[value_length_offset..value_length_offset + 4]
                .try_into()
                .unwrap(),
        ))
        .unwrap();
        value_length_offset + 4 + value_length
    }

    #[test]
    fn corruption_is_detected_by_reconstructed_identity() {
        let registry = custom_registry(false);
        let mut bytes = encode(&registry).unwrap();
        let value_offset = bytes
            .windows(4)
            .position(|window| window == b"#36f")
            .expect("custom value offset");
        bytes[value_offset + 1] = b'2';

        assert!(matches!(
            decode(&bytes),
            Err(ThemeBinaryError::ThemeIdMismatch { .. })
        ));
    }

    #[test]
    fn rejects_trailing_truncated_unknown_and_malformed_data() {
        let registry = custom_registry(false);
        let canonical = encode(&registry).unwrap();

        let mut trailing = canonical.clone();
        trailing.push(0);
        assert!(matches!(
            decode(&trailing),
            Err(ThemeBinaryError::TrailingData { remaining: 1, .. })
        ));

        assert!(matches!(
            decode(&canonical[..canonical.len() - 1]),
            Err(ThemeBinaryError::Truncated { .. })
        ));

        let mut unknown_kind = canonical.clone();
        let first_token_tag = THEME_BINARY_MAGIC.len() + 2 + 16 + 4;
        unknown_kind[first_token_tag] = u8::MAX;
        assert!(matches!(
            decode(&unknown_kind),
            Err(ThemeBinaryError::UnknownTokenKind { tag: u8::MAX, .. })
        ));

        let mut malformed_length = canonical;
        let first_name_length = first_token_tag + 1 + 4;
        malformed_length[first_name_length..first_name_length + 4]
            .copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(matches!(
            decode(&malformed_length),
            Err(ThemeBinaryError::LimitExceeded {
                field: "token name",
                ..
            })
        ));
    }

    #[test]
    fn rejects_bad_magic_and_version() {
        let registry = custom_registry(false);
        let mut bad_magic = encode(&registry).unwrap();
        bad_magic[0] ^= 1;
        assert!(matches!(
            decode(&bad_magic),
            Err(ThemeBinaryError::InvalidMagic)
        ));

        let mut bad_version = encode(&registry).unwrap();
        let version_offset = THEME_BINARY_MAGIC.len();
        bad_version[version_offset..version_offset + 2].copy_from_slice(&1_u16.to_be_bytes());
        assert!(matches!(
            decode(&bad_version),
            Err(ThemeBinaryError::UnsupportedVersion {
                found: 1,
                expected: THEME_BINARY_FORMAT_VERSION
            })
        ));
    }
}
