//! Canonical persisted representation of validated semantic style IR.
//!
//! Successful decoding enforces the compiler's external-stylesheet CSS
//! boundary. It does not HTML-escape text for direct concatenation into a
//! `<style>` element.

use core::{cmp::Ordering, fmt};
use std::collections::{BTreeMap, BTreeSet};

use pliego_css_ir::{
    ArbitraryProperty, ArbitraryPropertyId, ArbitraryValueId, Assignment, BreakpointId,
    CascadeLayer, ColorValue, Condition, ConditionId, ContrastPreference, CssNumber,
    CustomPropertyId, CustomPropertyRef, InvariantError, Keyword, Length, LengthUnit,
    MotionPreference, Percentage, PseudoState, SelectorId, SemanticStyle, SemanticValue, Slot,
    SlotSet, Span, StateSet, StyleId, ThemeMode, TokenId, TokenKind, TokenRef, Utility, ValueKind,
};
use pliego_css_parser::{
    is_valid_arbitrary_property_name, is_valid_custom_property_name, validate_arbitrary_value_text,
};
use pliego_css_theme::{THEME_ID_FORMAT_VERSION, ThemeRegistry};
use sha2::{Digest, Sha256};

use super::identity::{self, IdentityError, STYLE_ID_FORMAT_VERSION};
use super::{
    canonicalize_assignments, canonicalize_conditions, seed_theme, try_derive_style_id_with_theme,
    validate_selector_transform_text,
};

/// Magic prefix of a canonical semantic-IR artifact.
pub const IR_BINARY_MAGIC: [u8; 8] = *b"PLGCIR\0\0";

/// Current canonical semantic-IR artifact format version.
pub const IR_BINARY_FORMAT_VERSION: u16 = 2;

/// Maximum accepted size of one encoded semantic-IR artifact (16 MiB).
pub const MAX_IR_BINARY_BYTES: usize = 16 * 1024 * 1024;

/// Maximum accepted assignment count in one semantic-IR artifact.
pub const MAX_IR_RECORDS: usize = 65_535;

/// Maximum accepted selector-transform count in one assignment condition.
pub const MAX_IR_SELECTORS_PER_CONDITION: usize = 65_535;

/// Maximum accepted selector-transform occurrences across one artifact.
pub const MAX_IR_TOTAL_SELECTORS: usize = 65_535;

/// Maximum accepted UTF-8 byte length of one resolved text field (1 MiB).
pub const MAX_IR_TEXT_BYTES: usize = 1024 * 1024;

const PAYLOAD_OFFSET: usize = IR_BINARY_MAGIC.len() + 2 + 2 + 2 + 16 + 16 + 32;
const HEADER_BYTES: usize = PAYLOAD_OFFSET + 4;

/// Error returned while encoding or decoding canonical semantic IR.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IrBinaryError {
    /// The complete artifact exceeds [`MAX_IR_BINARY_BYTES`].
    ArtifactTooLarge {
        /// Actual artifact length in bytes.
        actual: usize,
        /// Maximum accepted artifact length in bytes.
        limit: usize,
    },
    /// A count or text payload exceeds its defensive limit.
    LimitExceeded {
        /// Stable field description.
        field: &'static str,
        /// Actual count or byte length.
        actual: usize,
        /// Maximum accepted count or byte length.
        limit: usize,
    },
    /// The artifact ended before a complete field could be read.
    Truncated {
        /// Absolute byte offset at which the field starts.
        offset: usize,
        /// Bytes required for the complete field.
        needed: usize,
        /// Bytes still available at `offset`.
        remaining: usize,
    },
    /// The artifact does not begin with [`IR_BINARY_MAGIC`].
    InvalidMagic,
    /// The artifact uses an unsupported semantic-IR format version.
    UnsupportedVersion {
        /// Version found in the artifact.
        found: u16,
        /// Version accepted by this crate.
        expected: u16,
    },
    /// The artifact names identity formats not understood by this compiler.
    IdentityFormatMismatch {
        /// Style identity format found in the artifact.
        style_found: u16,
        /// Style identity format required by this compiler.
        style_expected: u16,
        /// Theme identity format found in the artifact.
        theme_found: u16,
        /// Theme identity format required by this compiler.
        theme_expected: u16,
    },
    /// The artifact was produced for a different active theme.
    ThemeIdMismatch {
        /// Theme identity stored in the artifact.
        stored: u128,
        /// Theme identity supplied to the decoder.
        active: u128,
    },
    /// A persisted style identity does not match the decoded semantic payload.
    StyleIdMismatch {
        /// Style identity stored in the artifact.
        stored: StyleId,
        /// Identity recomputed from the decoded payload and active theme.
        computed: StyleId,
    },
    /// The record payload does not match its stored SHA-256 digest.
    PayloadHashMismatch {
        /// Digest stored in the artifact header.
        stored: [u8; 32],
        /// Digest recomputed from the complete record payload.
        computed: [u8; 32],
    },
    /// A tagged semantic domain contains an unknown tag.
    UnknownTag {
        /// Stable tagged-domain description.
        field: &'static str,
        /// Unknown byte tag.
        tag: u8,
        /// Absolute byte offset containing the tag.
        offset: usize,
    },
    /// A field has an invalid or non-canonical representation.
    InvalidField {
        /// Stable field description.
        field: &'static str,
        /// Absolute byte offset at which the field starts.
        offset: usize,
    },
    /// A resolved text field is not valid UTF-8.
    InvalidUtf8 {
        /// Stable text-field description.
        field: &'static str,
        /// Absolute byte offset containing the text payload.
        offset: usize,
    },
    /// A resolved CSS fragment fails the parser/compiler safety boundary.
    UnsafeText {
        /// Stable semantic field description.
        field: &'static str,
        /// Zero-based assignment record containing the text.
        record: usize,
    },
    /// Recognized records are followed by trailing bytes.
    TrailingData {
        /// Offset at which trailing data begins.
        offset: usize,
        /// Number of unrecognized bytes.
        remaining: usize,
    },
    /// Valid fields are not encoded in the one canonical representation.
    NonCanonical,
    /// Decoded semantic IR violates a structural invariant.
    InvalidIr(InvariantError),
    /// Semantic identity encoding failed.
    Identity(IdentityError),
    /// A decoded token is absent from the active theme.
    UnknownToken {
        /// Referenced token namespace.
        kind: TokenKind,
        /// Referenced token identifier.
        id: u32,
    },
    /// A decoded breakpoint is absent from the active theme.
    UnknownBreakpoint {
        /// Referenced breakpoint identifier.
        id: u16,
    },
}

impl fmt::Display for IrBinaryError {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArtifactTooLarge { actual, limit } => write!(
                formatter,
                "semantic-IR artifact is {actual} bytes; maximum supported size is {limit} bytes"
            ),
            Self::LimitExceeded {
                field,
                actual,
                limit,
            } => write!(
                formatter,
                "semantic-IR artifact {field} is {actual}; maximum supported value is {limit}"
            ),
            Self::Truncated {
                offset,
                needed,
                remaining,
            } => write!(
                formatter,
                "truncated semantic-IR artifact at byte {offset}: need {needed} bytes, have {remaining}"
            ),
            Self::InvalidMagic => formatter.write_str("invalid semantic-IR artifact magic header"),
            Self::UnsupportedVersion { found, expected } => write!(
                formatter,
                "unsupported semantic-IR artifact version {found}; expected {expected}"
            ),
            Self::IdentityFormatMismatch {
                style_found,
                style_expected,
                theme_found,
                theme_expected,
            } => write!(
                formatter,
                "semantic-IR identity formats are style {style_found}/theme {theme_found}; expected style {style_expected}/theme {theme_expected}"
            ),
            Self::ThemeIdMismatch { stored, active } => write!(
                formatter,
                "semantic-IR artifact theme {stored:032x} does not match active theme {active:032x}"
            ),
            Self::StyleIdMismatch { stored, computed } => write!(
                formatter,
                "semantic-IR artifact style {:032x} does not match computed style {:032x}",
                stored.get(),
                computed.get()
            ),
            Self::PayloadHashMismatch { .. } => {
                formatter.write_str("semantic-IR artifact payload SHA-256 mismatch")
            }
            Self::UnknownTag { field, tag, offset } => {
                write!(formatter, "unknown {field} tag {tag} at byte {offset}")
            }
            Self::InvalidField { field, offset } => {
                write!(formatter, "invalid semantic-IR {field} at byte {offset}")
            }
            Self::InvalidUtf8 { field, offset } => {
                write!(formatter, "invalid UTF-8 in {field} at byte {offset}")
            }
            Self::UnsafeText { field, record } => write!(
                formatter,
                "unsafe semantic-IR {field} in assignment record {record}"
            ),
            Self::TrailingData { offset, remaining } => write!(
                formatter,
                "semantic-IR artifact has {remaining} trailing bytes at byte {offset}"
            ),
            Self::NonCanonical => {
                formatter.write_str("semantic-IR artifact is not canonically encoded")
            }
            Self::InvalidIr(source) => write!(formatter, "invalid decoded semantic IR: {source}"),
            Self::Identity(source) => write!(formatter, "cannot encode semantic IR: {source}"),
            Self::UnknownToken { kind, id } => {
                write!(formatter, "unknown decoded {kind:?} token ID {id}")
            }
            Self::UnknownBreakpoint { id } => {
                write!(formatter, "unknown decoded breakpoint ID {id}")
            }
        }
    }
}

impl std::error::Error for IrBinaryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidIr(source) => Some(source),
            Self::Identity(source) => Some(source),
            _ => None,
        }
    }
}

impl From<InvariantError> for IrBinaryError {
    fn from(error: InvariantError) -> Self {
        Self::InvalidIr(error)
    }
}

impl From<IdentityError> for IrBinaryError {
    fn from(error: IdentityError) -> Self {
        Self::Identity(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WireRecord {
    semantic: Vec<u8>,
    source: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WireUtility {
    Catalog(Utility),
    ArbitraryProperty(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WireValue {
    Direct(SemanticValue),
    Arbitrary(String),
    CustomProperty { name: String, hint: ValueKind },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WireCondition {
    breakpoint: Option<BreakpointId>,
    container_breakpoint: Option<BreakpointId>,
    layer: CascadeLayer,
    theme: ThemeMode,
    motion: MotionPreference,
    contrast: ContrastPreference,
    states: StateSet,
    selectors: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedAssignment {
    utility: WireUtility,
    slots: SlotSet,
    value: WireValue,
    negative: bool,
    important: bool,
    condition: WireCondition,
    source: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedRecord {
    wire: WireRecord,
    assignment: DecodedAssignment,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CanonicalConditionKey<'a> {
    breakpoint: Option<BreakpointId>,
    container_breakpoint: Option<BreakpointId>,
    layer: CascadeLayer,
    theme: ThemeMode,
    motion: MotionPreference,
    contrast: ContrastPreference,
    states: StateSet,
    selectors: Vec<&'a str>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CanonicalUtilityKey<'a> {
    Catalog(Utility),
    ArbitraryProperty(&'a str),
}

/// Encodes semantic IR under the built-in seed theme.
///
/// # Errors
///
/// Returns [`IrBinaryError`] when the style is invalid, unsafe, non-canonical,
/// or exceeds a defensive format limit.
pub fn encode_style_ir(style: &SemanticStyle) -> Result<Vec<u8>, IrBinaryError> {
    encode_style_ir_with_theme(seed_theme(), style)
}

/// Encodes semantic IR under an explicit active theme.
///
/// The wire records resolve interned text and conditions by content instead of
/// persisting implementation table indices. The input must use the compiler's
/// canonical cascade order; structural validity alone is insufficient.
///
/// # Errors
///
/// Returns [`IrBinaryError`] when the style is invalid, belongs to another
/// theme, contains an unsafe CSS fragment, or exceeds a format limit.
pub fn encode_style_ir_with_theme(
    theme: &ThemeRegistry,
    style: &SemanticStyle,
) -> Result<Vec<u8>, IrBinaryError> {
    validate_style_shape_limits(style)?;
    style.validate()?;
    if !has_normalized_assignment_semantics(style) {
        return Err(IrBinaryError::NonCanonical);
    }
    validate_theme_references(theme, style)?;

    let mut records = Vec::with_capacity(style.assignments.len());
    let mut projected_size = HEADER_BYTES;
    let mut total_selectors: usize = 0;
    for (index, assignment) in style.assignments.iter().enumerate() {
        let selector_count = style.conditions[assignment.condition.index()]
            .selectors
            .len();
        total_selectors =
            total_selectors
                .checked_add(selector_count)
                .ok_or(IrBinaryError::LimitExceeded {
                    field: "total selector count",
                    actual: usize::MAX,
                    limit: MAX_IR_TOTAL_SELECTORS,
                })?;
        ensure_limit(
            "total selector count",
            total_selectors,
            MAX_IR_TOTAL_SELECTORS,
        )?;
        validate_resolved_text(style, assignment, index)?;
        let semantic_size = preflight_semantic_size(style, assignment)?;
        projected_size = checked_size_add(projected_size, 4 + semantic_size + 8)?;
        let semantic = identity::encode_assignment(style, assignment)?;
        debug_assert_eq!(semantic.len(), semantic_size);
        records.push(WireRecord {
            semantic,
            source: assignment.source,
        });
    }

    if !has_canonical_cascade_order(style) {
        return Err(IrBinaryError::NonCanonical);
    }

    // Derivation performs its own identity encoding. Run it only after the
    // bounded preflight above so an attacker-controlled in-memory IR cannot
    // force allocation beyond the artifact ceiling before limits are known.
    let computed = try_derive_style_id_with_theme(theme, style)?;
    if computed != style.id {
        return Err(IrBinaryError::StyleIdMismatch {
            stored: style.id,
            computed,
        });
    }

    records.sort_unstable_by(compare_wire_records);
    if records.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(IrBinaryError::NonCanonical);
    }

    let encoded_size = encoded_size(&records)?;
    debug_assert_eq!(encoded_size, projected_size);
    let mut payload = Vec::with_capacity(encoded_size - PAYLOAD_OFFSET);
    push_u32(&mut payload, records.len());
    for record in records {
        push_u32(&mut payload, record.semantic.len());
        payload.extend_from_slice(&record.semantic);
        payload.extend_from_slice(&record.source.start.to_be_bytes());
        payload.extend_from_slice(&record.source.end.to_be_bytes());
    }
    let payload_sha256: [u8; 32] = Sha256::digest(&payload).into();
    let mut output = Vec::with_capacity(encoded_size);
    output.extend_from_slice(&IR_BINARY_MAGIC);
    output.extend_from_slice(&IR_BINARY_FORMAT_VERSION.to_be_bytes());
    output.extend_from_slice(&STYLE_ID_FORMAT_VERSION.to_be_bytes());
    output.extend_from_slice(&THEME_ID_FORMAT_VERSION.to_be_bytes());
    output.extend_from_slice(&theme.id().get().to_be_bytes());
    output.extend_from_slice(&style.id.get().to_be_bytes());
    output.extend_from_slice(&payload_sha256);
    output.extend_from_slice(&payload);
    debug_assert_eq!(output.len(), encoded_size);
    Ok(output)
}

/// Decodes semantic IR under the built-in seed theme.
///
/// # Errors
///
/// Returns [`IrBinaryError`] for malformed, non-canonical, unsafe, or
/// incompatible bytes.
pub fn decode_style_ir(bytes: &[u8]) -> Result<SemanticStyle, IrBinaryError> {
    decode_style_ir_with_theme(seed_theme(), bytes)
}

/// Decodes semantic IR under an explicit active theme.
///
/// # Errors
///
/// Returns [`IrBinaryError`] for malformed, non-canonical, unsafe, or
/// incompatible bytes.
pub fn decode_style_ir_with_theme(
    theme: &ThemeRegistry,
    bytes: &[u8],
) -> Result<SemanticStyle, IrBinaryError> {
    decode_impl(theme, bytes)
}

// Filled by the record-decoder section below. Keeping the public entrypoint
// separate makes the trust boundary explicit in rustdoc.
#[allow(clippy::too_many_lines)]
fn decode_impl(theme: &ThemeRegistry, bytes: &[u8]) -> Result<SemanticStyle, IrBinaryError> {
    if bytes.len() > MAX_IR_BINARY_BYTES {
        return Err(IrBinaryError::ArtifactTooLarge {
            actual: bytes.len(),
            limit: MAX_IR_BINARY_BYTES,
        });
    }
    let mut reader = Reader::new(bytes);
    if reader.read_array::<8>()? != IR_BINARY_MAGIC {
        return Err(IrBinaryError::InvalidMagic);
    }
    let version = reader.read_u16()?;
    if version != IR_BINARY_FORMAT_VERSION {
        return Err(IrBinaryError::UnsupportedVersion {
            found: version,
            expected: IR_BINARY_FORMAT_VERSION,
        });
    }
    let style_format = reader.read_u16()?;
    let theme_format = reader.read_u16()?;
    if style_format != STYLE_ID_FORMAT_VERSION || theme_format != THEME_ID_FORMAT_VERSION {
        return Err(IrBinaryError::IdentityFormatMismatch {
            style_found: style_format,
            style_expected: STYLE_ID_FORMAT_VERSION,
            theme_found: theme_format,
            theme_expected: THEME_ID_FORMAT_VERSION,
        });
    }
    let stored_theme = reader.read_u128()?;
    if stored_theme != theme.id().get() {
        return Err(IrBinaryError::ThemeIdMismatch {
            stored: stored_theme,
            active: theme.id().get(),
        });
    }
    let stored_style = StyleId::new(reader.read_u128()?);
    let stored_payload_sha256 = reader.read_array::<32>()?;
    debug_assert_eq!(reader.absolute_offset(), PAYLOAD_OFFSET);
    let computed_payload_sha256: [u8; 32] = Sha256::digest(&bytes[PAYLOAD_OFFSET..]).into();
    if stored_payload_sha256 != computed_payload_sha256 {
        return Err(IrBinaryError::PayloadHashMismatch {
            stored: stored_payload_sha256,
            computed: computed_payload_sha256,
        });
    }

    let record_count = usize::try_from(reader.read_u32()?).expect("u32 fits supported usize");
    ensure_limit("assignment count", record_count, MAX_IR_RECORDS)?;
    let minimum_framing = record_count
        .checked_mul(12)
        .ok_or(IrBinaryError::Truncated {
            offset: reader.absolute_offset(),
            needed: usize::MAX,
            remaining: reader.remaining(),
        })?;
    if minimum_framing > reader.remaining() {
        return Err(IrBinaryError::Truncated {
            offset: reader.absolute_offset(),
            needed: minimum_framing,
            remaining: reader.remaining(),
        });
    }
    let mut records: Vec<ParsedRecord> = Vec::with_capacity(record_count);
    let mut total_selectors: usize = 0;
    for index in 0..record_count {
        let semantic_length =
            usize::try_from(reader.read_u32()?).expect("u32 fits supported usize");
        ensure_limit(
            "semantic record length",
            semantic_length,
            MAX_IR_BINARY_BYTES,
        )?;
        let semantic_offset = reader.absolute_offset();
        let framed_length = semantic_length
            .checked_add(8)
            .ok_or(IrBinaryError::Truncated {
                offset: semantic_offset,
                needed: usize::MAX,
                remaining: reader.remaining(),
            })?;
        if framed_length > reader.remaining() {
            return Err(IrBinaryError::Truncated {
                offset: semantic_offset,
                needed: framed_length,
                remaining: reader.remaining(),
            });
        }
        let semantic = reader.read_slice(semantic_length)?;
        let source_offset = reader.absolute_offset();
        let source = Span::new(reader.read_u32()?, reader.read_u32()?);
        if source.is_empty() {
            return Err(IrBinaryError::InvalidField {
                field: "source span",
                offset: source_offset,
            });
        }
        let assignment =
            decode_assignment_record(semantic, semantic_offset, index, source, total_selectors)?;
        total_selectors += assignment.condition.selectors.len();
        let parsed = ParsedRecord {
            wire: WireRecord {
                semantic: semantic.to_vec(),
                source,
            },
            assignment,
        };
        if let Some(previous) = records.last() {
            if compare_wire_records(&previous.wire, &parsed.wire) != Ordering::Less {
                return Err(IrBinaryError::NonCanonical);
            }
        }
        records.push(parsed);
    }
    if reader.remaining() != 0 {
        return Err(IrBinaryError::TrailingData {
            offset: reader.absolute_offset(),
            remaining: reader.remaining(),
        });
    }

    let mut style = SemanticStyle::new(stored_style);
    let mut selector_ids = BTreeMap::new();
    let mut condition_ids = BTreeMap::from([(Condition::BASE, SemanticStyle::base_condition())]);
    let mut arbitrary_property_ids = BTreeMap::new();
    let mut arbitrary_value_ids = BTreeMap::new();
    let mut custom_property_ids = BTreeMap::new();
    for parsed in records {
        let condition = parsed.assignment.condition;
        let selectors = condition
            .selectors
            .into_iter()
            .map(|selector| {
                intern_ordered(&mut selector_ids, &mut style.selectors, selector, |index| {
                    SelectorId::new(
                        u32::try_from(index).expect("artifact limit keeps selector IDs within u32"),
                    )
                })
            })
            .collect();
        let condition = intern_ordered(
            &mut condition_ids,
            &mut style.conditions,
            Condition {
                breakpoint: condition.breakpoint,
                container_breakpoint: condition.container_breakpoint,
                layer: condition.layer,
                theme: condition.theme,
                motion: condition.motion,
                contrast: condition.contrast,
                states: condition.states,
                selectors,
            },
            |index| {
                ConditionId::new(
                    u16::try_from(index).expect("assignment limit keeps condition IDs within u16"),
                )
            },
        );
        let utility = match parsed.assignment.utility {
            WireUtility::Catalog(utility) => utility,
            WireUtility::ArbitraryProperty(name) => Utility::ArbitraryProperty(intern_ordered(
                &mut arbitrary_property_ids,
                &mut style.arbitrary_properties,
                ArbitraryProperty { name },
                |index| {
                    ArbitraryPropertyId::new(
                        u32::try_from(index)
                            .expect("artifact limit keeps arbitrary-property IDs within u32"),
                    )
                },
            )),
        };
        let value = match parsed.assignment.value {
            WireValue::Direct(value) => value,
            WireValue::Arbitrary(value) => SemanticValue::Arbitrary(intern_ordered(
                &mut arbitrary_value_ids,
                &mut style.arbitrary_values,
                value,
                |index| {
                    ArbitraryValueId::new(
                        u32::try_from(index)
                            .expect("artifact limit keeps arbitrary-value IDs within u32"),
                    )
                },
            )),
            WireValue::CustomProperty { name, hint } => {
                SemanticValue::CustomProperty(CustomPropertyRef {
                    id: intern_ordered(
                        &mut custom_property_ids,
                        &mut style.custom_properties,
                        name,
                        |index| {
                            CustomPropertyId::new(
                                u32::try_from(index)
                                    .expect("artifact limit keeps custom-property IDs within u32"),
                            )
                        },
                    ),
                    hint,
                })
            }
        };
        style.assignments.push(Assignment {
            utility,
            slots: parsed.assignment.slots,
            value,
            negative: parsed.assignment.negative,
            condition,
            important: parsed.assignment.important,
            source: parsed.assignment.source,
        });
    }

    // Wire order is an identity order, not the emitter's cascade order. Restore
    // the compiler canonical order before any downstream consumer sees the IR.
    canonicalize_conditions(&mut style);
    canonicalize_assignments(&mut style);
    style.validate()?;
    if !has_normalized_assignment_semantics(&style) {
        return Err(IrBinaryError::NonCanonical);
    }
    validate_theme_references(theme, &style)?;
    for (index, assignment) in style.assignments.iter().enumerate() {
        validate_resolved_text(&style, assignment, index)?;
    }
    let computed = try_derive_style_id_with_theme(theme, &style)?;
    if computed != stored_style {
        return Err(IrBinaryError::StyleIdMismatch {
            stored: stored_style,
            computed,
        });
    }
    if encode_style_ir_with_theme(theme, &style)?.as_slice() != bytes {
        return Err(IrBinaryError::NonCanonical);
    }
    Ok(style)
}

fn intern_ordered<T, I>(
    ids: &mut BTreeMap<T, I>,
    values: &mut Vec<T>,
    value: T,
    id_from_index: impl FnOnce(usize) -> I,
) -> I
where
    T: Clone + Ord,
    I: Copy,
{
    if let Some(id) = ids.get(&value) {
        return *id;
    }
    let id = id_from_index(values.len());
    values.push(value.clone());
    ids.insert(value, id);
    id
}

#[allow(clippy::too_many_lines)]
fn decode_assignment_record(
    bytes: &[u8],
    base: usize,
    record: usize,
    source: Span,
    selectors_seen: usize,
) -> Result<DecodedAssignment, IrBinaryError> {
    let mut reader = Reader::with_base(bytes, base);
    expect_marker(&mut reader, identity::RECORD_UTILITY, "utility marker")?;
    let utility_offset = reader.absolute_offset();
    let utility_tag = reader.read_u8()?;
    let utility = match utility_tag {
        1..=59 | 61..=62 => {
            WireUtility::Catalog(utility_from_tag(utility_tag).expect("known utility tag"))
        }
        60 => {
            let name = reader.read_text("arbitrary property")?;
            if !is_valid_arbitrary_property_name(&name) {
                return Err(IrBinaryError::UnsafeText {
                    field: "arbitrary property",
                    record,
                });
            }
            WireUtility::ArbitraryProperty(name)
        }
        tag => {
            return Err(IrBinaryError::UnknownTag {
                field: "utility",
                tag,
                offset: utility_offset,
            });
        }
    };

    expect_marker(&mut reader, identity::RECORD_SLOTS, "slots marker")?;
    let slot_count = usize::from(reader.read_u8()?);
    if slot_count == 0 || slot_count > Slot::COUNT {
        return Err(IrBinaryError::InvalidField {
            field: "slot count",
            offset: reader.absolute_offset() - 1,
        });
    }
    let mut slots = SlotSet::EMPTY;
    let mut previous_slot_tag = 0;
    for _ in 0..slot_count {
        let offset = reader.absolute_offset();
        let tag = reader.read_u8()?;
        let slot = slot_from_tag(tag).ok_or(IrBinaryError::UnknownTag {
            field: "slot",
            tag,
            offset,
        })?;
        if tag <= previous_slot_tag {
            return Err(IrBinaryError::NonCanonical);
        }
        previous_slot_tag = tag;
        slots = slots.with(slot);
    }

    expect_marker(&mut reader, identity::RECORD_VALUE, "value marker")?;
    let value = decode_value(&mut reader, record)?;

    expect_marker(&mut reader, identity::RECORD_FLAGS, "flags marker")?;
    let negative = reader.read_bool("negative flag")?;
    let important = reader.read_bool("important flag")?;

    expect_marker(&mut reader, identity::RECORD_CONDITION, "condition marker")?;
    let condition = decode_condition(&mut reader, record, selectors_seen)?;
    if reader.remaining() != 0 {
        return Err(IrBinaryError::TrailingData {
            offset: reader.absolute_offset(),
            remaining: reader.remaining(),
        });
    }
    Ok(DecodedAssignment {
        utility,
        slots,
        value,
        negative,
        important,
        condition,
        source,
    })
}

fn expect_marker(
    reader: &mut Reader<'_>,
    expected: u8,
    field: &'static str,
) -> Result<(), IrBinaryError> {
    let offset = reader.absolute_offset();
    if reader.read_u8()? == expected {
        Ok(())
    } else {
        Err(IrBinaryError::InvalidField { field, offset })
    }
}

fn decode_value(reader: &mut Reader<'_>, record: usize) -> Result<WireValue, IrBinaryError> {
    let offset = reader.absolute_offset();
    let tag = reader.read_u8()?;
    let direct = match tag {
        1 => {
            let tag_offset = reader.absolute_offset();
            let keyword_tag = reader.read_u8()?;
            let keyword = keyword_from_tag(keyword_tag).ok_or(IrBinaryError::UnknownTag {
                field: "keyword",
                tag: keyword_tag,
                offset: tag_offset,
            })?;
            SemanticValue::Keyword(keyword)
        }
        2 => SemanticValue::Token(read_token(reader)?),
        3 => SemanticValue::Integer(reader.read_i32()?),
        4 => SemanticValue::Number(read_css_number(reader)?),
        5 => {
            let number = read_css_number(reader)?;
            let tag_offset = reader.absolute_offset();
            let unit_tag = reader.read_u8()?;
            let unit = length_unit_from_tag(unit_tag).ok_or(IrBinaryError::UnknownTag {
                field: "length unit",
                tag: unit_tag,
                offset: tag_offset,
            })?;
            SemanticValue::Length(Length { number, unit })
        }
        6 => {
            let percentage_offset = reader.absolute_offset();
            let basis_points = reader.read_u16()?;
            let percentage =
                Percentage::from_basis_points(basis_points).ok_or(IrBinaryError::InvalidField {
                    field: "percentage",
                    offset: percentage_offset,
                })?;
            SemanticValue::Percentage(percentage)
        }
        7 => SemanticValue::Color(decode_color(reader)?),
        8 => SemanticValue::Fraction {
            numerator: reader.read_u16()?,
            denominator: reader.read_u16()?,
        },
        9 => {
            let value = reader.read_text("arbitrary value")?;
            if validate_arbitrary_value_text(&value).is_err() {
                return Err(IrBinaryError::UnsafeText {
                    field: "arbitrary value",
                    record,
                });
            }
            return Ok(WireValue::Arbitrary(value));
        }
        10 => {
            let name = reader.read_text("custom property")?;
            if !is_valid_custom_property_name(&name) {
                return Err(IrBinaryError::UnsafeText {
                    field: "custom property",
                    record,
                });
            }
            let tag_offset = reader.absolute_offset();
            let hint_tag = reader.read_u8()?;
            let hint = value_kind_from_tag(hint_tag).ok_or(IrBinaryError::UnknownTag {
                field: "value kind",
                tag: hint_tag,
                offset: tag_offset,
            })?;
            return Ok(WireValue::CustomProperty { name, hint });
        }
        tag => {
            return Err(IrBinaryError::UnknownTag {
                field: "semantic value",
                tag,
                offset,
            });
        }
    };
    Ok(WireValue::Direct(direct))
}

fn read_css_number(reader: &mut Reader<'_>) -> Result<CssNumber, IrBinaryError> {
    let offset = reader.absolute_offset();
    let coefficient = reader.read_i64()?;
    let scale = reader.read_u8()?;
    let number = CssNumber::new(coefficient, scale).ok_or(IrBinaryError::InvalidField {
        field: "CSS number",
        offset,
    })?;
    if number.coefficient() != coefficient || number.scale() != scale {
        return Err(IrBinaryError::InvalidField {
            field: "noncanonical CSS number",
            offset,
        });
    }
    Ok(number)
}

fn read_token(reader: &mut Reader<'_>) -> Result<TokenRef, IrBinaryError> {
    let offset = reader.absolute_offset();
    let tag = reader.read_u8()?;
    let kind = token_kind_from_tag(tag).ok_or(IrBinaryError::UnknownTag {
        field: "token kind",
        tag,
        offset,
    })?;
    Ok(TokenRef {
        kind,
        id: TokenId::new(reader.read_u32()?),
    })
}

fn decode_color(reader: &mut Reader<'_>) -> Result<ColorValue, IrBinaryError> {
    let offset = reader.absolute_offset();
    let tag = reader.read_u8()?;
    match tag {
        1 => Ok(ColorValue::Transparent),
        2 => Ok(ColorValue::CurrentColor),
        3 => {
            let id = TokenId::new(reader.read_u32()?);
            let alpha = if reader.read_bool("color alpha presence")? {
                let alpha_offset = reader.absolute_offset();
                Some(Percentage::from_basis_points(reader.read_u16()?).ok_or(
                    IrBinaryError::InvalidField {
                        field: "color alpha",
                        offset: alpha_offset,
                    },
                )?)
            } else {
                None
            };
            Ok(ColorValue::Token { id, alpha })
        }
        4 => Ok(ColorValue::Srgba {
            red: reader.read_u8()?,
            green: reader.read_u8()?,
            blue: reader.read_u8()?,
            alpha: reader.read_u8()?,
        }),
        tag => Err(IrBinaryError::UnknownTag {
            field: "color",
            tag,
            offset,
        }),
    }
}

fn decode_condition(
    reader: &mut Reader<'_>,
    record: usize,
    selectors_seen: usize,
) -> Result<WireCondition, IrBinaryError> {
    let breakpoint = if reader.read_bool("breakpoint presence")? {
        Some(BreakpointId::new(reader.read_u16()?))
    } else {
        None
    };
    let container_breakpoint = if reader.peek_u8() == Some(identity::RECORD_CONTAINER_BREAKPOINT) {
        reader.read_u8()?;
        Some(BreakpointId::new(reader.read_u16()?))
    } else {
        None
    };
    let layer = if reader.peek_u8() == Some(identity::RECORD_CASCADE_LAYER) {
        reader.read_u8()?;
        read_enum_tag(reader, "cascade layer", cascade_layer_from_tag)?
    } else {
        CascadeLayer::Unlayered
    };
    let theme = read_enum_tag(reader, "theme mode", theme_mode_from_tag)?;
    let motion = read_enum_tag(reader, "motion preference", motion_from_tag)?;
    let contrast = read_enum_tag(reader, "contrast preference", contrast_from_tag)?;

    let state_count = usize::from(reader.read_u8()?);
    let mut states = StateSet::EMPTY;
    let mut previous_state_tag = 0;
    for _ in 0..state_count {
        let offset = reader.absolute_offset();
        let tag = reader.read_u8()?;
        let state = pseudo_state_from_tag(tag).ok_or(IrBinaryError::UnknownTag {
            field: "pseudo state",
            tag,
            offset,
        })?;
        if tag <= previous_state_tag {
            return Err(IrBinaryError::NonCanonical);
        }
        previous_state_tag = tag;
        states = states.with(state);
    }

    let selector_count = usize::try_from(reader.read_u32()?).expect("u32 fits supported usize");
    ensure_limit(
        "condition selector count",
        selector_count,
        MAX_IR_SELECTORS_PER_CONDITION,
    )?;
    let total_selectors =
        selectors_seen
            .checked_add(selector_count)
            .ok_or(IrBinaryError::LimitExceeded {
                field: "total selector count",
                actual: usize::MAX,
                limit: MAX_IR_TOTAL_SELECTORS,
            })?;
    ensure_limit(
        "total selector count",
        total_selectors,
        MAX_IR_TOTAL_SELECTORS,
    )?;
    if selector_count > reader.remaining() / 4 {
        return Err(IrBinaryError::Truncated {
            offset: reader.absolute_offset(),
            needed: selector_count.saturating_mul(4),
            remaining: reader.remaining(),
        });
    }
    let mut selectors = Vec::with_capacity(selector_count);
    for _ in 0..selector_count {
        let selector = reader.read_text("selector")?;
        if validate_selector_transform_text(&selector).is_err() {
            return Err(IrBinaryError::UnsafeText {
                field: "selector",
                record,
            });
        }
        selectors.push(selector);
    }
    Ok(WireCondition {
        breakpoint,
        container_breakpoint,
        layer,
        theme,
        motion,
        contrast,
        states,
        selectors,
    })
}

fn read_enum_tag<T>(
    reader: &mut Reader<'_>,
    field: &'static str,
    decode: impl FnOnce(u8) -> Option<T>,
) -> Result<T, IrBinaryError> {
    let offset = reader.absolute_offset();
    let tag = reader.read_u8()?;
    decode(tag).ok_or(IrBinaryError::UnknownTag { field, tag, offset })
}

fn compare_wire_records(left: &WireRecord, right: &WireRecord) -> Ordering {
    left.semantic
        .cmp(&right.semantic)
        .then_with(|| left.source.cmp(&right.source))
}

fn has_canonical_cascade_order(style: &SemanticStyle) -> bool {
    let referenced_conditions = style
        .assignments
        .iter()
        .map(|assignment| assignment.condition)
        .collect::<BTreeSet<_>>();
    let mut conditions = referenced_conditions
        .into_iter()
        .map(|id| {
            let condition = &style.conditions[id.index()];
            (
                id,
                CanonicalConditionKey {
                    breakpoint: condition.breakpoint,
                    container_breakpoint: condition.container_breakpoint,
                    layer: condition.layer,
                    theme: condition.theme,
                    motion: condition.motion,
                    contrast: condition.contrast,
                    states: condition.states,
                    selectors: condition
                        .selectors
                        .iter()
                        .map(|selector| style.selectors[selector.index()].as_str())
                        .collect(),
                },
            )
        })
        .collect::<Vec<_>>();
    conditions.sort_by(|left, right| left.1.cmp(&right.1));
    let condition_ranks = conditions
        .into_iter()
        .enumerate()
        .map(|(rank, (id, _))| (id, rank))
        .collect::<BTreeMap<_, _>>();

    let utility_key = |assignment: &Assignment| match assignment.utility {
        Utility::ArbitraryProperty(id) => CanonicalUtilityKey::ArbitraryProperty(
            style.arbitrary_properties[id.index()].name.as_str(),
        ),
        utility => CanonicalUtilityKey::Catalog(utility),
    };
    let mut order = (0..style.assignments.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        let left = &style.assignments[*left];
        let right = &style.assignments[*right];
        condition_ranks[&left.condition]
            .cmp(&condition_ranks[&right.condition])
            .then_with(|| {
                right
                    .slots
                    .bits()
                    .count_ones()
                    .cmp(&left.slots.bits().count_ones())
            })
            .then_with(|| left.slots.cmp(&right.slots))
            .then_with(|| utility_key(left).cmp(&utility_key(right)))
            .then_with(|| left.value.cmp(&right.value))
            .then_with(|| left.negative.cmp(&right.negative))
            .then_with(|| left.important.cmp(&right.important))
    });
    order
        .into_iter()
        .enumerate()
        .all(|(expected, actual)| expected == actual)
}

fn has_normalized_assignment_semantics(style: &SemanticStyle) -> bool {
    let mut arbitrary_properties = BTreeSet::new();
    let mut slot_assignments: BTreeMap<(ConditionId, Slot), Vec<usize>> = BTreeMap::new();

    for (index, assignment) in style.assignments.iter().enumerate() {
        if let Utility::ArbitraryProperty(id) = assignment.utility {
            if !arbitrary_properties.insert((assignment.condition, id)) {
                return false;
            }
            continue;
        }

        let mut candidates = BTreeSet::new();
        for slot in assignment.slots.iter() {
            if let Some(indices) = slot_assignments.get(&(assignment.condition, slot)) {
                candidates.extend(indices.iter().copied());
            }
        }
        for candidate in candidates {
            let existing = &style.assignments[candidate];
            if same_semantic_assignment(existing, assignment) {
                return false;
            }
            if assignment.slots.is_proper_subset(existing.slots)
                || existing.slots.is_proper_subset(assignment.slots)
            {
                let (broader, narrower) = if assignment.slots.is_proper_subset(existing.slots) {
                    (existing, assignment)
                } else {
                    (assignment, existing)
                };
                if broader.important && !narrower.important {
                    return false;
                }
                continue;
            }
            return false;
        }
        for slot in assignment.slots.iter() {
            slot_assignments
                .entry((assignment.condition, slot))
                .or_default()
                .push(index);
        }
    }
    true
}

fn same_semantic_assignment(left: &Assignment, right: &Assignment) -> bool {
    left.utility == right.utility
        && left.slots == right.slots
        && left.value == right.value
        && left.negative == right.negative
        && left.condition == right.condition
        && left.important == right.important
}

fn encoded_size(records: &[WireRecord]) -> Result<usize, IrBinaryError> {
    let mut size = HEADER_BYTES;
    for record in records {
        size = checked_size_add(size, 4 + record.semantic.len() + 8)?;
    }
    Ok(size)
}

fn checked_size_add(current: usize, additional: usize) -> Result<usize, IrBinaryError> {
    let size = current
        .checked_add(additional)
        .ok_or(IrBinaryError::ArtifactTooLarge {
            actual: usize::MAX,
            limit: MAX_IR_BINARY_BYTES,
        })?;
    if size > MAX_IR_BINARY_BYTES {
        Err(IrBinaryError::ArtifactTooLarge {
            actual: size,
            limit: MAX_IR_BINARY_BYTES,
        })
    } else {
        Ok(size)
    }
}

fn preflight_semantic_size(
    style: &SemanticStyle,
    assignment: &Assignment,
) -> Result<usize, IrBinaryError> {
    let mut size = 0;
    size = checked_size_add(size, 2)?; // utility marker + tag
    if let Utility::ArbitraryProperty(id) = assignment.utility {
        size = checked_size_add(size, 4 + style.arbitrary_properties[id.index()].name.len())?;
    }

    size = checked_size_add(size, 2 + assignment.slots.iter().count())?;
    size = checked_size_add(size, 1 + semantic_value_size(style, assignment.value))?;
    size = checked_size_add(size, 3)?; // flags marker + two booleans
    size = checked_size_add(
        size,
        1 + condition_size(style, &style.conditions[assignment.condition.index()])?,
    )?;
    Ok(size)
}

fn semantic_value_size(style: &SemanticStyle, value: SemanticValue) -> usize {
    match value {
        SemanticValue::Keyword(_)
        | SemanticValue::Color(ColorValue::Transparent | ColorValue::CurrentColor) => 2,
        SemanticValue::Token(_) | SemanticValue::Color(ColorValue::Srgba { .. }) => 6,
        SemanticValue::Integer(_) | SemanticValue::Fraction { .. } => 5,
        SemanticValue::Number(_) => 10,
        SemanticValue::Length(_) => 11,
        SemanticValue::Percentage(_) => 3,
        SemanticValue::Color(ColorValue::Token { alpha, .. }) => {
            if alpha.is_some() {
                9
            } else {
                7
            }
        }
        SemanticValue::Arbitrary(id) => 5 + style.arbitrary_values[id.index()].len(),
        SemanticValue::CustomProperty(reference) => {
            6 + style.custom_properties[reference.id.index()].len()
        }
    }
}

fn condition_size(style: &SemanticStyle, condition: &Condition) -> Result<usize, IrBinaryError> {
    ensure_limit(
        "condition selector count",
        condition.selectors.len(),
        MAX_IR_SELECTORS_PER_CONDITION,
    )?;
    let mut size = 1 + usize::from(condition.breakpoint.is_some()) * 2 + 3;
    size += usize::from(condition.container_breakpoint.is_some()) * 3;
    size += usize::from(condition.layer != CascadeLayer::Unlayered) * 2;
    size = checked_size_add(
        size,
        1 + usize::try_from(condition.states.bits().count_ones())
            .expect("state bit count fits usize"),
    )?;
    size = checked_size_add(size, 4)?;
    for selector in &condition.selectors {
        size = checked_size_add(size, 4 + style.selectors[selector.index()].len())?;
    }
    Ok(size)
}

fn push_u32(output: &mut Vec<u8>, value: usize) {
    let value = u32::try_from(value).expect("validated wire length must fit in u32");
    output.extend_from_slice(&value.to_be_bytes());
}

fn ensure_limit(field: &'static str, actual: usize, limit: usize) -> Result<(), IrBinaryError> {
    if actual > limit {
        Err(IrBinaryError::LimitExceeded {
            field,
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}

fn validate_style_shape_limits(style: &SemanticStyle) -> Result<(), IrBinaryError> {
    ensure_limit("assignment count", style.assignments.len(), MAX_IR_RECORDS)?;
    ensure_limit(
        "condition table entry count",
        style.conditions.len(),
        MAX_IR_RECORDS + 1,
    )?;
    for (field, count) in [
        ("selector table entry count", style.selectors.len()),
        (
            "arbitrary-value table entry count",
            style.arbitrary_values.len(),
        ),
        (
            "custom-property table entry count",
            style.custom_properties.len(),
        ),
        (
            "arbitrary-property table entry count",
            style.arbitrary_properties.len(),
        ),
    ] {
        ensure_limit(field, count, MAX_IR_RECORDS)?;
    }
    let selector_count = style
        .conditions
        .iter()
        .try_fold(0_usize, |total, condition| {
            total.checked_add(condition.selectors.len())
        })
        .ok_or(IrBinaryError::LimitExceeded {
            field: "condition-table selector count",
            actual: usize::MAX,
            limit: MAX_IR_TOTAL_SELECTORS,
        })?;
    ensure_limit(
        "condition-table selector count",
        selector_count,
        MAX_IR_TOTAL_SELECTORS,
    )
}

fn validate_theme_references(
    theme: &ThemeRegistry,
    style: &SemanticStyle,
) -> Result<(), IrBinaryError> {
    for condition in &style.conditions {
        for id in [condition.breakpoint, condition.container_breakpoint]
            .into_iter()
            .flatten()
        {
            if theme.breakpoint_by_id(id).is_none() {
                return Err(IrBinaryError::UnknownBreakpoint { id: id.get() });
            }
        }
    }
    for assignment in &style.assignments {
        let token = match assignment.value {
            SemanticValue::Token(reference) => Some((reference.kind, reference.id)),
            SemanticValue::Color(ColorValue::Token { id, .. }) => Some((TokenKind::Color, id)),
            _ => None,
        };
        if let Some((kind, id)) = token {
            if theme.token_by_id(kind, id).is_none() {
                return Err(IrBinaryError::UnknownToken { kind, id: id.get() });
            }
        }
    }
    Ok(())
}

fn validate_resolved_text(
    style: &SemanticStyle,
    assignment: &Assignment,
    record: usize,
) -> Result<(), IrBinaryError> {
    if let Utility::ArbitraryProperty(id) = assignment.utility {
        let name = &style.arbitrary_properties[id.index()].name;
        ensure_text("arbitrary property", name)?;
        if !is_valid_arbitrary_property_name(name) {
            return Err(IrBinaryError::UnsafeText {
                field: "arbitrary property",
                record,
            });
        }
    }
    match assignment.value {
        SemanticValue::Arbitrary(id) => {
            let value = &style.arbitrary_values[id.index()];
            ensure_text("arbitrary value", value)?;
            if validate_arbitrary_value_text(value).is_err() {
                return Err(IrBinaryError::UnsafeText {
                    field: "arbitrary value",
                    record,
                });
            }
        }
        SemanticValue::CustomProperty(reference) => {
            let name = &style.custom_properties[reference.id.index()];
            ensure_text("custom property", name)?;
            if !is_valid_custom_property_name(name) {
                return Err(IrBinaryError::UnsafeText {
                    field: "custom property",
                    record,
                });
            }
        }
        _ => {}
    }
    let condition = &style.conditions[assignment.condition.index()];
    for selector in &condition.selectors {
        let selector = &style.selectors[selector.index()];
        ensure_text("selector", selector)?;
        if validate_selector_transform_text(selector).is_err() {
            return Err(IrBinaryError::UnsafeText {
                field: "selector",
                record,
            });
        }
    }
    Ok(())
}

fn ensure_text(field: &'static str, value: &str) -> Result<(), IrBinaryError> {
    ensure_limit(field, value.len(), MAX_IR_TEXT_BYTES)
}

const UTILITY_TAGS: [Utility; 59] = [
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
];

const KEYWORD_TAGS: [Keyword; 63] = [
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

fn indexed_tag<T: Copy>(tag: u8, values: &[T]) -> Option<T> {
    let index = usize::from(tag.checked_sub(1)?);
    values.get(index).copied()
}

fn utility_from_tag(tag: u8) -> Option<Utility> {
    match tag {
        61 => Some(Utility::ContainerType),
        62 => Some(Utility::WritingMode),
        _ => indexed_tag(tag, &UTILITY_TAGS),
    }
}

fn slot_from_tag(tag: u8) -> Option<Slot> {
    indexed_tag(tag, &Slot::ALL)
}

fn keyword_from_tag(tag: u8) -> Option<Keyword> {
    indexed_tag(tag, &KEYWORD_TAGS)
}

fn token_kind_from_tag(tag: u8) -> Option<TokenKind> {
    indexed_tag(
        tag,
        &[
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
        ],
    )
}

fn length_unit_from_tag(tag: u8) -> Option<LengthUnit> {
    indexed_tag(
        tag,
        &[
            LengthUnit::Px,
            LengthUnit::Rem,
            LengthUnit::Em,
            LengthUnit::Percent,
            LengthUnit::Ch,
            LengthUnit::Vw,
            LengthUnit::Vh,
            LengthUnit::Dvw,
            LengthUnit::Dvh,
        ],
    )
}

fn value_kind_from_tag(tag: u8) -> Option<ValueKind> {
    indexed_tag(
        tag,
        &[
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
        ],
    )
}

fn theme_mode_from_tag(tag: u8) -> Option<ThemeMode> {
    indexed_tag(tag, &[ThemeMode::Any, ThemeMode::Light, ThemeMode::Dark])
}

fn cascade_layer_from_tag(tag: u8) -> Option<CascadeLayer> {
    match tag {
        1 => Some(CascadeLayer::Base),
        2 => Some(CascadeLayer::Components),
        3 => Some(CascadeLayer::Utilities),
        4 => Some(CascadeLayer::Overrides),
        _ => None,
    }
}

fn motion_from_tag(tag: u8) -> Option<MotionPreference> {
    indexed_tag(
        tag,
        &[
            MotionPreference::Any,
            MotionPreference::Safe,
            MotionPreference::Reduce,
        ],
    )
}

fn contrast_from_tag(tag: u8) -> Option<ContrastPreference> {
    indexed_tag(
        tag,
        &[
            ContrastPreference::Any,
            ContrastPreference::More,
            ContrastPreference::Less,
        ],
    )
}

fn pseudo_state_from_tag(tag: u8) -> Option<PseudoState> {
    indexed_tag(
        tag,
        &[
            PseudoState::Hover,
            PseudoState::Focus,
            PseudoState::FocusVisible,
            PseudoState::Active,
            PseudoState::Disabled,
        ],
    )
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    base: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            base: 0,
        }
    }

    const fn with_base(bytes: &'a [u8], base: usize) -> Self {
        Self {
            bytes,
            offset: 0,
            base,
        }
    }

    const fn absolute_offset(&self) -> usize {
        self.base + self.offset
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn peek_u8(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], IrBinaryError> {
        let slice = self.read_slice(N)?;
        let mut output = [0; N];
        output.copy_from_slice(slice);
        Ok(output)
    }

    fn read_slice(&mut self, length: usize) -> Result<&'a [u8], IrBinaryError> {
        let offset = self.absolute_offset();
        let remaining = self.remaining();
        if length > remaining {
            return Err(IrBinaryError::Truncated {
                offset,
                needed: length,
                remaining,
            });
        }
        let start = self.offset;
        self.offset += length;
        Ok(&self.bytes[start..self.offset])
    }

    fn read_u8(&mut self) -> Result<u8, IrBinaryError> {
        Ok(self.read_array::<1>()?[0])
    }

    fn read_u16(&mut self) -> Result<u16, IrBinaryError> {
        Ok(u16::from_be_bytes(self.read_array()?))
    }

    fn read_u32(&mut self) -> Result<u32, IrBinaryError> {
        Ok(u32::from_be_bytes(self.read_array()?))
    }

    fn read_i32(&mut self) -> Result<i32, IrBinaryError> {
        Ok(i32::from_be_bytes(self.read_array()?))
    }

    fn read_i64(&mut self) -> Result<i64, IrBinaryError> {
        Ok(i64::from_be_bytes(self.read_array()?))
    }

    fn read_u128(&mut self) -> Result<u128, IrBinaryError> {
        Ok(u128::from_be_bytes(self.read_array()?))
    }

    fn read_bool(&mut self, field: &'static str) -> Result<bool, IrBinaryError> {
        let offset = self.absolute_offset();
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(IrBinaryError::InvalidField { field, offset }),
        }
    }

    fn read_text(&mut self, field: &'static str) -> Result<String, IrBinaryError> {
        let length = usize::try_from(self.read_u32()?).expect("u32 must fit supported usize");
        ensure_limit(field, length, MAX_IR_TEXT_BYTES)?;
        let offset = self.absolute_offset();
        let payload = self.read_slice(length)?;
        let value = core::str::from_utf8(payload)
            .map_err(|_| IrBinaryError::InvalidUtf8 { field, offset })?;
        Ok(value.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use core::fmt::Write as _;

    use pliego_css_parser::parse_style_list;

    use super::*;
    use crate::{emit_css, lower_style, try_derive_style_id, utility_catalog};

    const GOLDEN_SOURCE: &str = "block md:dark:hover:[&>svg]:bg-accent/20 -mt-[2rem]! [mask-type:luminance] text-(color:--label)";

    fn lower(source: &str) -> SemanticStyle {
        let syntax = parse_style_list(source).expect("must succeed");
        lower_style(&syntax).expect("must succeed")
    }

    fn hex(bytes: &[u8]) -> String {
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(&mut output, "{byte:02x}").expect("must succeed");
        }
        output
    }

    fn refresh_payload_hash(bytes: &mut [u8]) {
        let digest: [u8; 32] = Sha256::digest(&bytes[PAYLOAD_OFFSET..]).into();
        bytes[PAYLOAD_OFFSET - 32..PAYLOAD_OFFSET].copy_from_slice(&digest);
    }

    fn record_frames(bytes: &[u8]) -> Vec<(usize, usize, usize)> {
        let count = usize::try_from(u32::from_be_bytes(
            bytes[PAYLOAD_OFFSET..HEADER_BYTES]
                .try_into()
                .expect("must succeed"),
        ))
        .expect("must succeed");
        let mut cursor = HEADER_BYTES;
        let mut frames = Vec::with_capacity(count);
        for _ in 0..count {
            let start = cursor;
            let semantic_length = usize::try_from(u32::from_be_bytes(
                bytes[start..start + 4].try_into().expect("must succeed"),
            ))
            .expect("must succeed");
            let semantic_start = start + 4;
            cursor = semantic_start + semantic_length + 8;
            frames.push((start, semantic_start, cursor));
        }
        assert_eq!(
            cursor,
            bytes.len(),
            "test artifact must have complete frames"
        );
        frames
    }

    #[test]
    fn golden_vector_is_frozen() {
        let style = lower(GOLDEN_SOURCE);
        let bytes = encode_style_ir(&style).expect("must succeed");
        let digest: [u8; 32] = Sha256::digest(&bytes).into();

        assert_eq!(bytes.len(), 309);
        assert_eq!(
            hex(&digest),
            "337865f8e7b5b32fe58f67442537ad1d2926d94164d1692863a2616dddc85023"
        );
        assert_eq!(
            hex(&bytes),
            concat!(
                "504c474349520000000200020002b3d5ad77175995c2b8f51ef7c0d419919dda0d2bd9a9bc5c77077006110d08717f17",
                "43fbde6e0fc37de303b085e83ff58ce3791ebfa0a06c443a936792d0350d000000050000001511011201011301041400",
                "001500010101000000000000000000000000050000001c110e1201121309000000043272656d14010115000101010000",
                "000000000000290000003400000028112012012b13070313e3e9b70107d0140000150100010301010101000000010000",
                "0005263e737667000000060000002800000020112212012d130a000000072d2d6c6162656c0814000015000101010000",
                "0000000000004b0000005f0000002e113c000000096d61736b2d747970651201531309000000096c756d696e616e6365",
                "14000015000101010000000000000000350000004a",
            )
        );

        let decoded = decode_style_ir(&bytes).expect("must succeed");
        assert_eq!(decoded.id, style.id);
        assert_eq!(encode_style_ir(&decoded).expect("must succeed"), bytes);
        assert_eq!(
            emit_css(&decoded).expect("must succeed"),
            emit_css(&style).expect("must succeed")
        );
    }

    #[test]
    fn every_catalog_example_round_trips_through_format_two() {
        for descriptor in utility_catalog() {
            let style = lower(descriptor.example());
            let original_css = emit_css(&style).unwrap_or_else(|error| {
                panic!(
                    "catalog example `{}` must emit: {error}",
                    descriptor.example()
                )
            });
            let bytes = encode_style_ir(&style).unwrap_or_else(|error| {
                panic!(
                    "catalog example `{}` must encode: {error}",
                    descriptor.example()
                )
            });
            let decoded = decode_style_ir(&bytes).unwrap_or_else(|error| {
                panic!(
                    "catalog example `{}` must decode: {error}",
                    descriptor.example()
                )
            });
            assert_eq!(
                emit_css(&decoded).expect("must succeed"),
                original_css,
                "catalog example `{}` changed CSS",
                descriptor.example()
            );
            assert_eq!(
                encode_style_ir(&decoded).expect("must succeed"),
                bytes,
                "catalog example `{}` changed bytes",
                descriptor.example()
            );
        }
    }

    #[test]
    fn all_condition_dimensions_and_manual_value_variants_round_trip() {
        let conditioned = lower("light:motion-safe:contrast-less:focus-visible:active:[&>p]:block");
        let conditioned_bytes = encode_style_ir(&conditioned).expect("must succeed");
        assert_eq!(
            encode_style_ir(&decode_style_ir(&conditioned_bytes).expect("must succeed"))
                .expect("must succeed"),
            conditioned_bytes
        );

        let mut number = lower("flex-1");
        number
            .assignments
            .iter_mut()
            .find(|assignment| assignment.utility == Utility::FlexGrow)
            .expect("must succeed")
            .value = SemanticValue::Number(CssNumber::new(125, 2).expect("must succeed"));
        number.id = try_derive_style_id(&number).expect("must succeed");

        let mut srgba = lower("bg-transparent");
        srgba.assignments[0].value = SemanticValue::Color(ColorValue::Srgba {
            red: 12,
            green: 34,
            blue: 56,
            alpha: 78,
        });
        srgba.id = try_derive_style_id(&srgba).expect("must succeed");

        for style in [number, srgba] {
            let bytes = encode_style_ir(&style).expect("must succeed");
            let decoded = decode_style_ir(&bytes).expect("must succeed");
            assert_eq!(decoded.assignments, style.assignments);
            assert_eq!(encode_style_ir(&decoded).expect("must succeed"), bytes);
        }
    }

    #[test]
    fn round_trip_restores_cascade_order_for_shorthand_refinements() {
        let style = lower("p-4 px-2");
        let original_css = emit_css(&style).expect("must succeed");
        let bytes = encode_style_ir(&style).expect("must succeed");
        let decoded = decode_style_ir(&bytes).expect("must succeed");

        assert_eq!(emit_css(&decoded).expect("must succeed"), original_css);
        assert_eq!(decoded.assignments, style.assignments);
    }

    #[test]
    fn round_trip_ignores_selector_table_insertion_ids_without_changing_css() {
        let mut style = lower("[&.a]:bg-[blue] [&.z]:bg-[red]");
        style.selectors.swap(0, 1);
        for condition in &mut style.conditions {
            for selector in &mut condition.selectors {
                *selector = SelectorId::new(1 - selector.get());
            }
        }
        style.validate().expect("must succeed");
        let original_css = emit_css(&style).expect("must succeed");
        let bytes = encode_style_ir(&style).expect("must succeed");
        let decoded = decode_style_ir(&bytes).expect("must succeed");

        assert_eq!(emit_css(&decoded).expect("must succeed"), original_css);
        assert_eq!(encode_style_ir(&decoded).expect("must succeed"), bytes);
    }

    #[test]
    fn encoder_rejects_noncanonical_cascade_order() {
        let mut style = lower("p-4 px-2");
        style.assignments.reverse();
        style.validate().expect("must succeed");
        assert_eq!(encode_style_ir(&style), Err(IrBinaryError::NonCanonical));
    }

    #[test]
    fn encoder_rejects_compiler_unreachable_conflicts_and_semantic_duplicates() {
        let mut conflicting = lower("bg-[z]");
        let value = conflicting.intern_arbitrary_value("a".into());
        let mut second = conflicting.assignments[0];
        second.value = SemanticValue::Arbitrary(value);
        second.source = Span::new(100, 106);
        conflicting.assignments.push(second);
        conflicting.validate().expect("must succeed");
        assert_eq!(
            encode_style_ir(&conflicting),
            Err(IrBinaryError::NonCanonical)
        );

        let mut duplicate = lower("block");
        let mut second = duplicate.assignments[0];
        second.source = Span::new(100, 105);
        duplicate.assignments.push(second);
        duplicate.validate().expect("must succeed");
        assert_eq!(
            encode_style_ir(&duplicate),
            Err(IrBinaryError::NonCanonical)
        );

        let distinct = lower("[mask-type:luminance] [text-wrap:balance]");
        let bytes = encode_style_ir(&distinct).expect("must succeed");
        assert_eq!(
            encode_style_ir(&decode_style_ir(&bytes).expect("must succeed")).expect("must succeed"),
            bytes
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn inverse_tag_tables_match_the_frozen_identity_encoder() {
        for (index, utility) in UTILITY_TAGS.into_iter().enumerate() {
            let tag = u8::try_from(index + 1).expect("must succeed");
            assert_eq!(identity::utility_tag(utility), tag);
            assert_eq!(utility_from_tag(tag), Some(utility));
        }
        assert_eq!(identity::utility_tag(Utility::ContainerType), 61);
        assert_eq!(utility_from_tag(61), Some(Utility::ContainerType));
        assert_eq!(identity::utility_tag(Utility::WritingMode), 62);
        assert_eq!(utility_from_tag(62), Some(Utility::WritingMode));
        for slot in Slot::ALL {
            let tag = identity::slot_tag(slot);
            assert_eq!(slot_from_tag(tag), Some(slot));
        }
        for (index, keyword) in KEYWORD_TAGS.into_iter().enumerate() {
            let tag = u8::try_from(index + 1).expect("must succeed");
            assert_eq!(identity::keyword_tag(keyword), tag);
            assert_eq!(keyword_from_tag(tag), Some(keyword));
        }
        for kind in [
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
        ] {
            assert_eq!(
                token_kind_from_tag(identity::token_kind_tag(kind)),
                Some(kind)
            );
        }
        for unit in [
            LengthUnit::Px,
            LengthUnit::Rem,
            LengthUnit::Em,
            LengthUnit::Percent,
            LengthUnit::Ch,
            LengthUnit::Vw,
            LengthUnit::Vh,
            LengthUnit::Dvw,
            LengthUnit::Dvh,
        ] {
            assert_eq!(
                length_unit_from_tag(identity::length_unit_tag(unit)),
                Some(unit)
            );
        }
        for kind in [
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
        ] {
            assert_eq!(
                value_kind_from_tag(identity::value_kind_tag(kind)),
                Some(kind)
            );
        }
        for mode in [ThemeMode::Any, ThemeMode::Light, ThemeMode::Dark] {
            assert_eq!(
                theme_mode_from_tag(identity::theme_mode_tag(mode)),
                Some(mode)
            );
        }
        for motion in [
            MotionPreference::Any,
            MotionPreference::Safe,
            MotionPreference::Reduce,
        ] {
            assert_eq!(
                motion_from_tag(identity::motion_preference_tag(motion)),
                Some(motion)
            );
        }
        for contrast in [
            ContrastPreference::Any,
            ContrastPreference::More,
            ContrastPreference::Less,
        ] {
            assert_eq!(
                contrast_from_tag(identity::contrast_preference_tag(contrast)),
                Some(contrast)
            );
        }
        for state in [
            PseudoState::Hover,
            PseudoState::Focus,
            PseudoState::FocusVisible,
            PseudoState::Active,
            PseudoState::Disabled,
        ] {
            assert_eq!(
                pseudo_state_from_tag(identity::pseudo_state_tag(state)),
                Some(state)
            );
        }
    }

    #[test]
    fn header_and_payload_corruption_fail_closed() {
        let style = lower("block opacity-50");
        let bytes = encode_style_ir(&style).expect("must succeed");

        let mut invalid_magic = bytes.clone();
        invalid_magic[0] ^= 1;
        assert_eq!(
            decode_style_ir(&invalid_magic),
            Err(IrBinaryError::InvalidMagic)
        );

        let mut unsupported = bytes.clone();
        unsupported[8..10].copy_from_slice(&3_u16.to_be_bytes());
        assert!(matches!(
            decode_style_ir(&unsupported),
            Err(IrBinaryError::UnsupportedVersion {
                found: 3,
                expected: 2
            })
        ));

        let mut identity_mismatch = bytes.clone();
        identity_mismatch[10..12].copy_from_slice(&999_u16.to_be_bytes());
        assert!(matches!(
            decode_style_ir(&identity_mismatch),
            Err(IrBinaryError::IdentityFormatMismatch {
                style_found: 999,
                ..
            })
        ));

        let mut wrong_style = bytes.clone();
        wrong_style[30] ^= 1;
        assert!(matches!(
            decode_style_ir(&wrong_style),
            Err(IrBinaryError::StyleIdMismatch { .. })
        ));

        let mut corrupt_payload = bytes;
        let last = corrupt_payload.len() - 1;
        corrupt_payload[last] ^= 1;
        assert!(matches!(
            decode_style_ir(&corrupt_payload),
            Err(IrBinaryError::PayloadHashMismatch { .. })
        ));
    }

    #[test]
    fn structural_corruption_with_a_valid_digest_is_rejected() {
        let style = lower("block opacity-50");
        let bytes = encode_style_ir(&style).expect("must succeed");
        let frames = record_frames(&bytes);

        let mut invalid_boolean = bytes.clone();
        let (_, semantic_start, frame_end) = frames[0];
        let semantic_end = frame_end - 8;
        let flag_marker = invalid_boolean[semantic_start..semantic_end]
            .windows(4)
            .position(|window| window == [identity::RECORD_FLAGS, 0, 0, identity::RECORD_CONDITION])
            .expect("must succeed")
            + semantic_start;
        invalid_boolean[flag_marker + 1] = 2;
        refresh_payload_hash(&mut invalid_boolean);
        assert!(matches!(
            decode_style_ir(&invalid_boolean),
            Err(IrBinaryError::InvalidField {
                field: "negative flag",
                ..
            })
        ));

        let mut unknown_utility = bytes.clone();
        unknown_utility[semantic_start + 1] = 0xff;
        refresh_payload_hash(&mut unknown_utility);
        assert!(matches!(
            decode_style_ir(&unknown_utility),
            Err(IrBinaryError::UnknownTag {
                field: "utility",
                tag: 0xff,
                ..
            })
        ));

        let mut empty_span = bytes.clone();
        let source_start = frame_end - 8;
        let start = empty_span[source_start..source_start + 4].to_vec();
        empty_span[source_start + 4..source_start + 8].copy_from_slice(&start);
        refresh_payload_hash(&mut empty_span);
        assert!(matches!(
            decode_style_ir(&empty_span),
            Err(IrBinaryError::InvalidField {
                field: "source span",
                ..
            })
        ));

        let mut trailing = bytes;
        trailing.push(0);
        refresh_payload_hash(&mut trailing);
        assert!(matches!(
            decode_style_ir(&trailing),
            Err(IrBinaryError::TrailingData { remaining: 1, .. })
        ));
    }

    #[test]
    fn framing_is_preflighted_before_record_allocation_or_semantic_copy() {
        let bytes = encode_style_ir(&lower("block")).expect("must succeed");

        let mut impossible_count = bytes.clone();
        impossible_count[PAYLOAD_OFFSET..HEADER_BYTES]
            .copy_from_slice(&u32::try_from(MAX_IR_RECORDS).unwrap().to_be_bytes());
        refresh_payload_hash(&mut impossible_count);
        assert!(matches!(
            decode_style_ir(&impossible_count),
            Err(IrBinaryError::Truncated { .. })
        ));

        let mut missing_span_byte = bytes;
        let remaining_after_length = missing_span_byte.len() - (HEADER_BYTES + 4);
        let semantic_length = remaining_after_length - 7;
        missing_span_byte[HEADER_BYTES..HEADER_BYTES + 4]
            .copy_from_slice(&u32::try_from(semantic_length).unwrap().to_be_bytes());
        refresh_payload_hash(&mut missing_span_byte);
        assert!(matches!(
            decode_style_ir(&missing_span_byte),
            Err(IrBinaryError::Truncated { .. })
        ));
    }

    #[test]
    fn duplicate_and_reordered_records_are_noncanonical() {
        let style = lower("block opacity-50");
        let bytes = encode_style_ir(&style).expect("must succeed");
        let frames = record_frames(&bytes);

        let mut reordered = bytes[..HEADER_BYTES].to_vec();
        reordered.extend_from_slice(&bytes[frames[1].0..frames[1].2]);
        reordered.extend_from_slice(&bytes[frames[0].0..frames[0].2]);
        refresh_payload_hash(&mut reordered);
        assert_eq!(
            decode_style_ir(&reordered),
            Err(IrBinaryError::NonCanonical)
        );

        let single = encode_style_ir(&lower("block")).expect("must succeed");
        let frame = record_frames(&single)[0];
        let mut duplicate = single;
        duplicate[PAYLOAD_OFFSET..HEADER_BYTES].copy_from_slice(&2_u32.to_be_bytes());
        let record = duplicate[frame.0..frame.2].to_vec();
        duplicate.extend_from_slice(&record);
        refresh_payload_hash(&mut duplicate);
        assert_eq!(
            decode_style_ir(&duplicate),
            Err(IrBinaryError::NonCanonical)
        );
    }

    #[test]
    fn decoded_text_reuses_authoring_safety_boundaries() {
        let style = lower("[mask-type:luminance]");
        let bytes = encode_style_ir(&style).expect("must succeed");
        let value_offset = bytes
            .windows(b"luminance".len())
            .position(|window| window == b"luminance")
            .expect("must succeed");

        let mut invalid_utf8 = bytes.clone();
        invalid_utf8[value_offset] = 0xff;
        refresh_payload_hash(&mut invalid_utf8);
        assert!(matches!(
            decode_style_ir(&invalid_utf8),
            Err(IrBinaryError::InvalidUtf8 {
                field: "arbitrary value",
                ..
            })
        ));

        let mut escaped_important = bytes.clone();
        escaped_important[value_offset..value_offset + 9].copy_from_slice(b"!\\69mport");
        refresh_payload_hash(&mut escaped_important);
        assert_eq!(
            decode_style_ir(&escaped_important),
            Err(IrBinaryError::UnsafeText {
                field: "arbitrary value",
                record: 0,
            })
        );

        let mut unsafe_value = bytes;
        unsafe_value[value_offset..value_offset + 9].copy_from_slice(b"red/*xxxx");
        refresh_payload_hash(&mut unsafe_value);
        assert_eq!(
            decode_style_ir(&unsafe_value),
            Err(IrBinaryError::UnsafeText {
                field: "arbitrary value",
                record: 0,
            })
        );

        let mut unsafe_brace =
            encode_style_ir(&lower("[mask-type:luminance]")).expect("must succeed");
        let brace_offset = unsafe_brace
            .windows(9)
            .position(|window| window == b"luminance")
            .expect("must succeed");
        unsafe_brace[brace_offset..brace_offset + 9].copy_from_slice(b"foo({)xxx");
        refresh_payload_hash(&mut unsafe_brace);
        assert_eq!(
            decode_style_ir(&unsafe_brace),
            Err(IrBinaryError::UnsafeText {
                field: "arbitrary value",
                record: 0,
            })
        );

        let mut repeated_anchor = encode_style_ir(&lower("[&>p]:block")).expect("must succeed");
        let selector_offset = repeated_anchor
            .windows(3)
            .position(|window| window == b"&>p")
            .expect("must succeed");
        repeated_anchor[selector_offset..selector_offset + 3].copy_from_slice(b"&&p");
        refresh_payload_hash(&mut repeated_anchor);
        assert_eq!(
            decode_style_ir(&repeated_anchor),
            Err(IrBinaryError::UnsafeText {
                field: "selector",
                record: 0,
            })
        );
    }

    #[test]
    fn theme_and_catalog_references_are_bound_to_the_active_registry() {
        let style = lower("md:text-ink");
        let bytes = encode_style_ir(&style).expect("must succeed");
        let empty_theme = ThemeRegistry::from_definitions([], []).expect("must succeed");
        assert!(matches!(
            decode_style_ir_with_theme(&empty_theme, &bytes),
            Err(IrBinaryError::ThemeIdMismatch { .. })
        ));

        let mut unknown_token = encode_style_ir(&lower("text-ink")).expect("must succeed");
        let marker = unknown_token
            .windows(3)
            .position(|window| window == [identity::RECORD_VALUE, 7, 3])
            .expect("must succeed");
        unknown_token[marker + 3..marker + 7].copy_from_slice(&u32::MAX.to_be_bytes());
        refresh_payload_hash(&mut unknown_token);
        assert!(matches!(
            decode_style_ir(&unknown_token),
            Err(IrBinaryError::UnknownToken { .. })
        ));
    }

    #[test]
    fn provenance_changes_artifact_but_not_style_identity() {
        let style = lower("block");
        let mut moved = style.clone();
        moved.assignments[0].source = Span::new(100, 105);
        assert_eq!(style.id, moved.id);
        assert_ne!(
            encode_style_ir(&style).expect("must succeed"),
            encode_style_ir(&moved).expect("must succeed")
        );
    }

    #[test]
    fn artifact_and_text_limits_apply_before_identity_reencoding() {
        let oversized = vec![0; MAX_IR_BINARY_BYTES + 1];
        assert_eq!(
            decode_style_ir(&oversized),
            Err(IrBinaryError::ArtifactTooLarge {
                actual: MAX_IR_BINARY_BYTES + 1,
                limit: MAX_IR_BINARY_BYTES,
            })
        );

        let mut style = lower("[mask-type:x]");
        style.arbitrary_values[0] = "x".repeat(MAX_IR_TEXT_BYTES + 1);
        assert!(matches!(
            encode_style_ir(&style),
            Err(IrBinaryError::LimitExceeded {
                field: "arbitrary value",
                ..
            })
        ));

        let mut too_many_selectors = lower("[&>p]:block");
        too_many_selectors.conditions[1].selectors =
            vec![SelectorId::new(0); MAX_IR_SELECTORS_PER_CONDITION + 1];
        assert!(matches!(
            encode_style_ir(&too_many_selectors),
            Err(IrBinaryError::LimitExceeded {
                field: "condition-table selector count",
                ..
            })
        ));

        let mut total_selector_overflow = lower("[&>p]:block [&>q]:hidden");
        total_selector_overflow.conditions[1].selectors =
            vec![SelectorId::new(0); (MAX_IR_TOTAL_SELECTORS / 2) + 1];
        total_selector_overflow.conditions[2].selectors =
            vec![SelectorId::new(1); (MAX_IR_TOTAL_SELECTORS / 2) + 1];
        assert!(matches!(
            encode_style_ir(&total_selector_overflow),
            Err(IrBinaryError::LimitExceeded {
                field: "condition-table selector count",
                ..
            })
        ));

        let mut too_many_records = encode_style_ir(&lower("block")).expect("must succeed");
        too_many_records[PAYLOAD_OFFSET..HEADER_BYTES]
            .copy_from_slice(&u32::try_from(MAX_IR_RECORDS + 1).unwrap().to_be_bytes());
        refresh_payload_hash(&mut too_many_records);
        assert!(matches!(
            decode_style_ir(&too_many_records),
            Err(IrBinaryError::LimitExceeded {
                field: "assignment count",
                ..
            })
        ));
    }

    #[test]
    fn decoder_caps_total_selector_occurrences_before_allocating_the_next_chain() {
        let mut style = lower("[&>p]:block");
        style.conditions[1].selectors = vec![SelectorId::new(0); (MAX_IR_TOTAL_SELECTORS / 2) + 1];
        style.id = try_derive_style_id(&style).expect("must succeed");
        let mut bytes = encode_style_ir(&style).expect("must succeed");
        let frame = record_frames(&bytes)[0];
        let duplicate = bytes[frame.0..frame.2].to_vec();
        bytes[PAYLOAD_OFFSET..HEADER_BYTES].copy_from_slice(&2_u32.to_be_bytes());
        bytes.extend_from_slice(&duplicate);
        let second_source = bytes.len() - 8;
        bytes[second_source..second_source + 4].copy_from_slice(&100_u32.to_be_bytes());
        bytes[second_source + 4..second_source + 8].copy_from_slice(&101_u32.to_be_bytes());
        refresh_payload_hash(&mut bytes);

        assert!(matches!(
            decode_style_ir(&bytes),
            Err(IrBinaryError::LimitExceeded {
                field: "total selector count",
                ..
            })
        ));
    }
}
