//! Parser for the `PliegoCSS` utility grammar.
//!
//! This is an exact-version implementation crate. Partial parser stages are experimental tooling
//! APIs; applications should use the compile-time macros from `pliego-css`.

#![forbid(unsafe_code)]

use pliego_css_ir::{
    Candidate, CandidateKind, Diagnostic, DiagnosticCode, SourceSpan, StyleItem, StyleList,
    Variant, VariantKind,
};

/// Syntax retained for a named candidate before catalog longest-match resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedCandidateSyntax {
    /// Candidate body before an optional `/` modifier. The compiler resolves the utility key.
    pub body: String,
    /// Exact span of `body`.
    pub body_span: SourceSpan,
    /// Optional opacity, line-height, or family-specific modifier.
    pub modifier: Option<OperandSyntax>,
}

/// A syntax-level operand. Catalog passes decide whether it is valid for a utility family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperandSyntax {
    /// Parsed operand representation.
    pub kind: OperandKind,
    /// Exact span covering the operand, including delimiters when present.
    pub span: SourceSpan,
}

/// Operand categories which are unambiguous without consulting the utility catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperandKind {
    /// A token name or numeric value such as `4` or `red-500`.
    Bare(String),
    /// A CSS value between square brackets.
    ArbitraryValue(String),
    /// A CSS custom property, optionally disambiguated by a type hint.
    CustomVariable {
        /// Optional family hint such as `color` in `(color:--label)`.
        type_hint: Option<String>,
        /// Custom property name including its `--` prefix.
        name: String,
    },
}

/// Parses a complete named candidate while deliberately leaving utility-key resolution to the
/// compiler's catalog longest-match pass.
///
/// # Errors
///
/// Returns a stable syntax diagnostic for malformed modifiers, arbitrary values, or variables.
pub fn parse_named_candidate(source: &str) -> Result<NamedCandidateSyntax, Diagnostic> {
    parse_named_candidate_at(source, 0)
}

/// Parses a named candidate whose first byte is at `base_offset` in the macro literal.
///
/// # Errors
///
/// Returns a stable syntax diagnostic for malformed modifiers, arbitrary values, or variables.
pub fn parse_named_candidate_at(
    source: &str,
    base_offset: usize,
) -> Result<NamedCandidateSyntax, Diagnostic> {
    ensure_balanced(source, base_offset)?;
    if source.is_empty() {
        return Err(diagnostic(
            DiagnosticCode::MalformedItem,
            "missing utility candidate",
            SourceSpan::new(base_offset, base_offset),
            None,
        ));
    }

    let pieces = split_top_level_ranges(source, '/', base_offset)?;
    if pieces.len() > 2 {
        let slash = pieces[1].end;
        return Err(diagnostic(
            DiagnosticCode::MalformedItem,
            "a candidate accepts at most one top-level `/` modifier",
            SourceSpan::new(base_offset + slash, base_offset + slash + 1),
            None,
        ));
    }
    let body_range = pieces[0];
    let body = &source[body_range.start..body_range.end];
    if body.is_empty() {
        return Err(diagnostic(
            DiagnosticCode::MalformedItem,
            "missing utility before modifier",
            SourceSpan::new(base_offset, base_offset + 1.min(source.len())),
            None,
        ));
    }

    validate_candidate_body(body, base_offset)?;
    let modifier = if pieces.len() == 2 {
        let range = pieces[1];
        if range.is_empty() {
            return Err(diagnostic(
                DiagnosticCode::MalformedItem,
                "modifier cannot be empty",
                SourceSpan::new(base_offset + source.len() - 1, base_offset + source.len()),
                None,
            ));
        }
        Some(parse_operand_at(
            &source[range.start..range.end],
            base_offset + range.start,
        )?)
    } else {
        None
    };

    Ok(NamedCandidateSyntax {
        body: body.to_owned(),
        body_span: SourceSpan::new(base_offset, base_offset + body.len()),
        modifier,
    })
}

/// Parses one operand after the compiler has resolved a utility key by longest match.
///
/// # Errors
///
/// Returns `PCS008` for malformed arbitrary values or custom variables.
pub fn parse_operand(source: &str) -> Result<OperandSyntax, Diagnostic> {
    parse_operand_at(source, 0)
}

/// Parses one operand with an absolute macro-literal offset.
///
/// # Errors
///
/// Returns `PCS008` for malformed arbitrary values or custom variables.
pub fn parse_operand_at(source: &str, base_offset: usize) -> Result<OperandSyntax, Diagnostic> {
    ensure_balanced(source, base_offset)?;
    let span = SourceSpan::new(base_offset, base_offset + source.len());
    if source.is_empty() {
        return Err(diagnostic(
            DiagnosticCode::InvalidArbitraryValue,
            "operand cannot be empty",
            span,
            None,
        ));
    }

    let kind = if source.starts_with('[') {
        if !source.ends_with(']') {
            return Err(diagnostic(
                DiagnosticCode::InvalidArbitraryValue,
                "arbitrary value must end with `]`",
                span,
                None,
            ));
        }
        let value = &source[1..source.len() - 1];
        validate_arbitrary_value(value, base_offset + 1)?;
        OperandKind::ArbitraryValue(value.to_owned())
    } else if source.starts_with('(') {
        parse_custom_variable(source, base_offset)?
    } else {
        OperandKind::Bare(source.to_owned())
    };
    Ok(OperandSyntax { kind, span })
}

/// Parses the syntax-level utility list used by `pc!`.
///
/// Catalog lookup, token domains, and semantic conflict analysis happen in later compiler passes.
///
/// # Errors
///
/// Returns a stable `PCS` diagnostic for malformed or unbalanced syntax.
pub fn parse_style_list(source: &str) -> Result<StyleList, Diagnostic> {
    let ranges = split_items(source)?;
    if ranges.is_empty() {
        return Err(diagnostic(
            DiagnosticCode::MalformedItem,
            "the style list is empty; use `Style::EMPTY`",
            SourceSpan::new(0, source.len()),
            Some("Style::EMPTY".into()),
        ));
    }

    let mut items = Vec::with_capacity(ranges.len());
    for span in ranges {
        items.push(parse_item(source, span)?);
    }
    Ok(StyleList { items })
}

/// Formats a parsed utility list with one ASCII space between items.
///
/// Candidate payloads and ordered variant chains are preserved exactly; only
/// top-level whitespace is canonicalized.
#[must_use]
pub fn format_style_list(style: &StyleList) -> String {
    style
        .items
        .iter()
        .map(|item| {
            let mut output = String::new();
            for variant in &item.variants {
                match &variant.kind {
                    VariantKind::Named(name) => output.push_str(name),
                    VariantKind::ArbitrarySelector(selector) => {
                        output.push('[');
                        output.push_str(selector);
                        output.push(']');
                    }
                }
                output.push(':');
            }
            if item.negative {
                output.push('-');
            }
            match &item.candidate.kind {
                CandidateKind::Named(candidate) => output.push_str(candidate),
                CandidateKind::ArbitraryProperty { property, value } => {
                    output.push('[');
                    output.push_str(property);
                    output.push(':');
                    output.push_str(value);
                    output.push(']');
                }
            }
            if item.important {
                output.push('!');
            }
            output
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn split_items(source: &str) -> Result<Vec<SourceSpan>, Diagnostic> {
    let mut ranges = Vec::new();
    let mut start = None;
    let mut brackets = Vec::new();
    let mut quote = None;
    let mut escaped = false;

    for (offset, character) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some((open_quote, _)) = quote {
            if character == open_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some((character, offset));
            start.get_or_insert(offset);
            continue;
        }

        match character {
            '[' | '(' => {
                brackets.push((character, offset));
                start.get_or_insert(offset);
            }
            ']' | ')' => {
                let expected = if character == ']' { '[' } else { '(' };
                match brackets.pop() {
                    Some((actual, _)) if actual == expected => {}
                    _ => {
                        return Err(diagnostic(
                            DiagnosticCode::UnbalancedDelimiter,
                            format!("unmatched `{character}`"),
                            SourceSpan::new(offset, offset + character.len_utf8()),
                            None,
                        ));
                    }
                }
            }
            character if character.is_whitespace() && brackets.is_empty() => {
                if let Some(item_start) = start.take() {
                    ranges.push(SourceSpan::new(item_start, offset));
                }
            }
            _ => {
                start.get_or_insert(offset);
            }
        }
    }

    if escaped {
        let start = source.len().saturating_sub(1);
        return Err(diagnostic(
            DiagnosticCode::UnbalancedDelimiter,
            "unterminated escape",
            SourceSpan::new(start, source.len()),
            None,
        ));
    }
    if let Some((open_quote, offset)) = quote {
        return Err(diagnostic(
            DiagnosticCode::UnbalancedDelimiter,
            format!("unclosed `{open_quote}`"),
            SourceSpan::new(offset, offset + open_quote.len_utf8()),
            None,
        ));
    }
    if let Some((delimiter, offset)) = brackets.last().copied() {
        return Err(diagnostic(
            DiagnosticCode::UnbalancedDelimiter,
            format!("unclosed `{delimiter}`"),
            SourceSpan::new(offset, offset + delimiter.len_utf8()),
            None,
        ));
    }
    if let Some(item_start) = start {
        ranges.push(SourceSpan::new(item_start, source.len()));
    }
    Ok(ranges)
}

fn parse_item(source: &str, span: SourceSpan) -> Result<StyleItem, Diagnostic> {
    let text = &source[span.start..span.end];
    let segments = split_top_level(text, ':', span.start)?;
    let Some(candidate_segment) = segments.last().copied() else {
        return Err(diagnostic(
            DiagnosticCode::MalformedItem,
            "missing utility candidate",
            span,
            None,
        ));
    };
    if candidate_segment.is_empty() {
        return Err(diagnostic(
            DiagnosticCode::MalformedItem,
            "variant chain must end with a utility",
            SourceSpan::new(span.end.saturating_sub(1), span.end),
            None,
        ));
    }

    let mut variants = Vec::with_capacity(segments.len().saturating_sub(1));
    let mut consumed = 0;
    for segment in &segments[..segments.len() - 1] {
        let segment_start = span.start + consumed;
        let segment_span = SourceSpan::new(segment_start, segment_start + segment.len());
        if segment.is_empty() {
            return Err(diagnostic(
                DiagnosticCode::MalformedItem,
                "variant names cannot be empty",
                segment_span,
                None,
            ));
        }
        let kind = if segment.starts_with('[') && segment.ends_with(']') {
            let selector = &segment[1..segment.len() - 1];
            if selector.is_empty() || !selector.starts_with('&') {
                return Err(diagnostic(
                    DiagnosticCode::MalformedItem,
                    "arbitrary selector variants must use the `[&...]` form",
                    segment_span,
                    None,
                ));
            }
            VariantKind::ArbitrarySelector(selector.to_owned())
        } else if valid_identifier(segment) || configurable_attribute_variant(segment) {
            VariantKind::Named((*segment).to_owned())
        } else {
            return Err(diagnostic(
                DiagnosticCode::MalformedItem,
                format!("invalid variant `{segment}`"),
                segment_span,
                None,
            ));
        };
        variants.push(Variant {
            kind,
            span: segment_span,
        });
        consumed += segment.len() + 1;
    }

    let candidate_offset = span.end - candidate_segment.len();
    let (mut candidate_text, important) = parse_important(candidate_segment, candidate_offset)?;

    let negative = candidate_text.starts_with('-');
    if negative {
        candidate_text = &candidate_text[1..];
        if candidate_text.starts_with('-') {
            return Err(diagnostic(
                DiagnosticCode::InvalidNegative,
                "negative utilities use exactly one leading `-` marker",
                SourceSpan::new(candidate_offset, candidate_offset + 2),
                Some(candidate_text.to_owned()),
            ));
        }
    }
    if candidate_text.is_empty() {
        return Err(diagnostic(
            DiagnosticCode::MalformedItem,
            "missing utility candidate",
            span,
            None,
        ));
    }

    let marker_bytes = usize::from(negative);
    let candidate_span = SourceSpan::new(
        candidate_offset + marker_bytes,
        candidate_offset + marker_bytes + candidate_text.len(),
    );
    let kind = parse_candidate(candidate_text, candidate_span)?;

    Ok(StyleItem {
        variants,
        negative,
        candidate: Candidate {
            kind,
            span: candidate_span,
        },
        important,
        span,
    })
}

fn configurable_attribute_variant(value: &str) -> bool {
    (value.starts_with("aria-[") || value.starts_with("data-[")) && value.ends_with(']')
}

fn parse_important(source: &str, base_offset: usize) -> Result<(&str, bool), Diagnostic> {
    let bang_parts = split_top_level_ranges(source, '!', base_offset)?;
    let bang_offsets: Vec<_> = bang_parts
        .iter()
        .take(bang_parts.len().saturating_sub(1))
        .map(|part| part.end)
        .collect();
    let important = bang_offsets.as_slice() == [source.len().saturating_sub(1)];
    if !bang_offsets.is_empty() && !important {
        let bang = bang_offsets[0];
        let clean = source.replace('!', "");
        return Err(diagnostic(
            DiagnosticCode::InvalidImportant,
            "`!` is only valid as one trailing important marker",
            SourceSpan::new(base_offset + bang, base_offset + bang + 1),
            Some(format!("{clean}!")),
        ));
    }
    Ok((
        if important {
            &source[..source.len() - 1]
        } else {
            source
        },
        important,
    ))
}

fn parse_candidate(text: &str, span: SourceSpan) -> Result<CandidateKind, Diagnostic> {
    if text.starts_with('[') && text.ends_with(']') {
        let content = &text[1..text.len() - 1];
        let parts = split_top_level(content, ':', span.start + 1)?;
        if parts.len() < 2 || !valid_property_name(parts[0]) {
            return Err(diagnostic(
                DiagnosticCode::InvalidArbitraryProperty,
                "arbitrary properties require `[property:value]`",
                span,
                None,
            ));
        }
        let property = parts[0];
        let value = &content[property.len() + 1..];
        if value.trim().is_empty()
            || has_unsafe_css_control_or_comment(value)
            || value.contains(['{', '}'])
            || contains_top_level(value, ';')?
        {
            return Err(diagnostic(
                DiagnosticCode::InvalidArbitraryProperty,
                "arbitrary properties contain exactly one declaration and no block",
                span,
                None,
            ));
        }
        if value.to_ascii_lowercase().contains("!important") || contains_top_level(value, '!')? {
            return Err(diagnostic(
                DiagnosticCode::InvalidImportant,
                "write the PliegoCSS `!` suffix instead of a top-level `!` in an arbitrary property",
                span,
                None,
            ));
        }
        return Ok(CandidateKind::ArbitraryProperty {
            property: property.to_owned(),
            value: value.to_owned(),
        });
    }
    parse_named_candidate_at(text, span.start)?;
    Ok(CandidateKind::Named(text.to_owned()))
}

fn validate_candidate_body(source: &str, base_offset: usize) -> Result<(), Diagnostic> {
    // Distinctive operands can be validated without deciding where a bare utility key ends.
    if let Some((offset, _)) = source
        .match_indices("-[")
        .chain(source.match_indices("-("))
        .filter(|(offset, _)| is_top_level(source, *offset))
        .max_by_key(|(offset, _)| *offset)
    {
        let operand = &source[offset + 1..];
        let parsed = parse_operand_at(operand, base_offset + offset + 1)?;
        if let OperandKind::ArbitraryValue(value) = &parsed.kind {
            let trimmed = value.trim_start();
            if is_simple_negative(trimmed) {
                let positive = trimmed.trim_start_matches('-');
                let prefix = &source[..offset + 2];
                return Err(diagnostic(
                    DiagnosticCode::InvalidNegative,
                    "put the negative marker before the utility, not inside its arbitrary value",
                    parsed.span,
                    Some(format!("-{}-[{positive}]", &prefix[..prefix.len() - 2])),
                ));
            }
        }
    }

    if let Some(offset) = find_top_level_double_hyphen(source) {
        let fixed = source.replacen("--", "-", 1);
        return Err(diagnostic(
            DiagnosticCode::InvalidNegative,
            "negative utilities use one leading `-` marker",
            SourceSpan::new(base_offset + offset, base_offset + offset + 2),
            Some(format!("-{fixed}")),
        ));
    }
    Ok(())
}

fn validate_arbitrary_value(source: &str, base_offset: usize) -> Result<(), Diagnostic> {
    if source.trim().is_empty() {
        return Err(diagnostic(
            DiagnosticCode::InvalidArbitraryValue,
            "arbitrary value cannot be empty",
            SourceSpan::new(base_offset, base_offset + source.len()),
            None,
        ));
    }
    if source.to_ascii_lowercase().contains("!important") {
        return Err(diagnostic(
            DiagnosticCode::InvalidImportant,
            "write the PliegoCSS `!` suffix instead of embedded `!important`",
            SourceSpan::new(base_offset, base_offset + source.len()),
            None,
        ));
    }
    if has_unsafe_css_control_or_comment(source) {
        return Err(diagnostic(
            DiagnosticCode::InvalidArbitraryValue,
            "arbitrary values cannot contain CSS comments or raw control line breaks",
            SourceSpan::new(base_offset, base_offset + source.len()),
            None,
        ));
    }
    if contains_top_level(source, '!')? {
        return Err(diagnostic(
            DiagnosticCode::InvalidImportant,
            "write the PliegoCSS `!` suffix instead of a top-level `!` in an arbitrary value",
            SourceSpan::new(base_offset, base_offset + source.len()),
            None,
        ));
    }
    if source.contains(['{', '}']) {
        return Err(diagnostic(
            DiagnosticCode::InvalidArbitraryValue,
            "braces are not allowed in an arbitrary value",
            SourceSpan::new(base_offset, base_offset + source.len()),
            None,
        ));
    }
    for forbidden in [';'] {
        if contains_top_level(source, forbidden)? {
            return Err(diagnostic(
                DiagnosticCode::InvalidArbitraryValue,
                format!("top-level `{forbidden}` is not allowed in an arbitrary value"),
                SourceSpan::new(base_offset, base_offset + source.len()),
                None,
            ));
        }
    }
    Ok(())
}

fn has_unsafe_css_control_or_comment(source: &str) -> bool {
    source.contains("/*")
        || source.contains("*/")
        || source
            .chars()
            .any(|character| matches!(character, '\0' | '\n' | '\r' | '\u{000c}'))
}

/// Validates one decoded arbitrary-value payload using the authoring parser's safety rules.
///
/// This entry point is intended for versioned artifact decoders that receive the value without its
/// surrounding utility syntax.
///
/// # Errors
///
/// Returns a structured diagnostic when the value is empty, contains unsafe controls, comments,
/// braces, or a top-level bang/declaration delimiter, or has unbalanced nested syntax.
pub fn validate_arbitrary_value_text(source: &str) -> Result<(), Diagnostic> {
    validate_arbitrary_value(source, 0)
}

/// Returns whether a decoded custom-property name matches the syntax accepted by the parser.
#[must_use]
pub fn is_valid_custom_property_name(value: &str) -> bool {
    valid_custom_property_name(value)
}

/// Returns whether a decoded arbitrary CSS property name matches the syntax accepted by the parser.
#[must_use]
pub fn is_valid_arbitrary_property_name(value: &str) -> bool {
    valid_property_name(value)
}

fn parse_custom_variable(source: &str, base_offset: usize) -> Result<OperandKind, Diagnostic> {
    let span = SourceSpan::new(base_offset, base_offset + source.len());
    if !source.ends_with(')') {
        return Err(diagnostic(
            DiagnosticCode::InvalidArbitraryValue,
            "custom variable must end with `)`",
            span,
            None,
        ));
    }
    let inner = &source[1..source.len() - 1];
    let parts = split_top_level_ranges(inner, ':', base_offset + 1)?;
    if parts.len() > 2 {
        return Err(diagnostic(
            DiagnosticCode::InvalidArbitraryValue,
            "custom variable accepts at most one type hint",
            span,
            None,
        ));
    }
    let (type_hint, name_range) = if parts.len() == 2 {
        let hint = &inner[parts[0].start..parts[0].end];
        if !valid_identifier(hint) {
            return Err(diagnostic(
                DiagnosticCode::InvalidArbitraryValue,
                "custom-variable type hint must be a lowercase identifier",
                SourceSpan::new(base_offset + 1, base_offset + 1 + hint.len()),
                None,
            ));
        }
        (Some(hint.to_owned()), parts[1])
    } else {
        (None, parts[0])
    };
    let name = &inner[name_range.start..name_range.end];
    if !valid_custom_property_name(name) {
        return Err(diagnostic(
            DiagnosticCode::InvalidArbitraryValue,
            "custom variable must be a valid name beginning with `--`",
            SourceSpan::new(
                base_offset + 1 + name_range.start,
                base_offset + 1 + name_range.end,
            ),
            None,
        ));
    }
    Ok(OperandKind::CustomVariable {
        type_hint,
        name: name.to_owned(),
    })
}

fn ensure_balanced(source: &str, base_offset: usize) -> Result<(), Diagnostic> {
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some((open_quote, _)) = quote {
            if character == open_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some((character, offset));
            continue;
        }
        match character {
            '[' | '(' => stack.push((character, offset)),
            ']' | ')' => {
                let expected = if character == ']' { '[' } else { '(' };
                match stack.pop() {
                    Some((actual, _)) if actual == expected => {}
                    _ => {
                        return Err(diagnostic(
                            DiagnosticCode::UnbalancedDelimiter,
                            format!("unmatched `{character}`"),
                            SourceSpan::new(
                                base_offset + offset,
                                base_offset + offset + character.len_utf8(),
                            ),
                            None,
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    if escaped {
        return Err(diagnostic(
            DiagnosticCode::UnbalancedDelimiter,
            "unterminated escape",
            SourceSpan::new(
                base_offset + source.len().saturating_sub(1),
                base_offset + source.len(),
            ),
            None,
        ));
    }
    if let Some((delimiter, offset)) = quote.or_else(|| stack.last().copied()) {
        return Err(diagnostic(
            DiagnosticCode::UnbalancedDelimiter,
            format!("unclosed `{delimiter}`"),
            SourceSpan::new(
                base_offset + offset,
                base_offset + offset + delimiter.len_utf8(),
            ),
            None,
        ));
    }
    Ok(())
}

fn split_top_level_ranges(
    source: &str,
    separator: char,
    base_offset: usize,
) -> Result<Vec<SourceSpan>, Diagnostic> {
    ensure_balanced(source, base_offset)?;
    let mut parts = Vec::new();
    let mut start = 0;
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(open_quote) = quote {
            if character == open_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
            continue;
        }
        match character {
            '[' | '(' => stack.push(character),
            ']' | ')' => {
                stack.pop();
            }
            _ if character == separator && stack.is_empty() => {
                parts.push(SourceSpan::new(start, offset));
                start = offset + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(SourceSpan::new(start, source.len()));
    Ok(parts)
}

fn is_top_level(source: &str, target: usize) -> bool {
    let mut depth = 0usize;
    for (offset, character) in source.char_indices() {
        if offset >= target {
            break;
        }
        match character {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth == 0
}

fn find_top_level_double_hyphen(source: &str) -> Option<usize> {
    source
        .match_indices("--")
        .map(|(offset, _)| offset)
        .find(|offset| is_top_level(source, *offset))
}

fn is_simple_negative(source: &str) -> bool {
    let rest = source.strip_prefix('-').unwrap_or_default();
    matches!(rest.chars().next(), Some('0'..='9' | '.'))
}

fn valid_custom_property_name(value: &str) -> bool {
    let Some(name) = value.strip_prefix("--") else {
        return false;
    };
    let mut characters = name.chars();
    matches!(characters.next(), Some('a'..='z' | 'A'..='Z' | '_'))
        && characters
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn split_top_level(
    source: &str,
    separator: char,
    base_offset: usize,
) -> Result<Vec<&str>, Diagnostic> {
    ensure_balanced(source, base_offset)?;
    let mut parts = Vec::new();
    let mut start = 0;
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;

    for (offset, character) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(open_quote) = quote {
            if character == open_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
            continue;
        }
        match character {
            '[' | '(' => stack.push(character),
            ']' | ')' => {
                stack.pop();
            }
            _ if character == separator && stack.is_empty() => {
                parts.push(&source[start..offset]);
                start = offset + character.len_utf8();
            }
            _ => {}
        }
    }
    if escaped || quote.is_some() || !stack.is_empty() {
        return Err(diagnostic(
            DiagnosticCode::UnbalancedDelimiter,
            "unbalanced nested syntax",
            SourceSpan::new(base_offset, base_offset + source.len()),
            None,
        ));
    }
    parts.push(&source[start..]);
    Ok(parts)
}

fn contains_top_level(source: &str, needle: char) -> Result<bool, Diagnostic> {
    Ok(split_top_level(source, needle, 0)?.len() > 1)
}

fn valid_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some('a'..='z'))
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn valid_property_name(value: &str) -> bool {
    if let Some(custom) = value.strip_prefix("--") {
        return !custom.is_empty()
            && custom
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-');
    }
    valid_identifier(value) && !value.ends_with('-')
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_utility_list() {
        let parsed = parse_style_list("flex items-center gap-4").expect("valid utility list");
        assert_eq!(parsed.items.len(), 3);
        assert_eq!(
            parsed.items[0].candidate.kind,
            CandidateKind::Named("flex".into())
        );
    }

    #[test]
    fn keeps_spaces_inside_arbitrary_values() {
        let parsed = parse_style_list("grid-cols-[1fr 300px] bg-[oklch(62% 0.2 25)]")
            .expect("balanced arbitrary values");
        assert_eq!(parsed.items.len(), 2);
    }

    #[test]
    fn tracks_exact_item_spans_across_whitespace() {
        let parsed = parse_style_list("  flex\tgap-4\nhover:bg-accent  ")
            .expect("whitespace-delimited utilities");
        let spans: Vec<_> = parsed.items.iter().map(|item| item.span).collect();
        assert_eq!(
            spans,
            vec![
                SourceSpan::new(2, 6),
                SourceSpan::new(7, 12),
                SourceSpan::new(13, 28),
            ]
        );
    }

    #[test]
    fn quoted_delimiters_do_not_close_arbitrary_value() {
        let parsed = parse_style_list(r#"before:content-["a ] : b"] block"#)
            .expect("quoted delimiters remain literal");
        assert_eq!(parsed.items.len(), 2);
    }

    #[test]
    fn underscores_and_spaces_in_urls_remain_literal() {
        let parsed = parse_style_list("bg-[url('/img/a_b c.svg')] text-ink")
            .expect("URL content remains one utility");
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(
            parsed.items[0].candidate.kind,
            CandidateKind::Named("bg-[url('/img/a_b c.svg')]".into())
        );
    }

    #[test]
    fn parses_variants_negative_and_important() {
        let parsed = parse_style_list("dark:md:hover:-mt-4!").expect("valid modifiers");
        let item = &parsed.items[0];
        assert_eq!(item.variants.len(), 3);
        assert!(item.negative);
        assert!(item.important);
        assert_eq!(item.candidate.kind, CandidateKind::Named("mt-4".into()));
    }

    #[test]
    fn parses_arbitrary_property_with_colon_in_value() {
        let parsed = parse_style_list("[background:url(https://example.com/a)]")
            .expect("colon inside nested value");
        assert_eq!(
            parsed.items[0].candidate.kind,
            CandidateKind::ArbitraryProperty {
                property: "background".into(),
                value: "url(https://example.com/a)".into(),
            }
        );
    }

    #[test]
    fn rejects_empty_style_list() {
        let error = parse_style_list("  \n").expect_err("empty style must fail");
        assert_eq!(error.code, DiagnosticCode::MalformedItem);
    }

    #[test]
    fn rejects_incomplete_variant() {
        let error = parse_style_list("hover:").expect_err("missing candidate must fail");
        assert_eq!(error.code, DiagnosticCode::MalformedItem);
    }

    #[test]
    fn rejects_unbalanced_bracket() {
        let error =
            parse_style_list("grid-cols-[1fr 2fr").expect_err("unbalanced bracket must fail");
        assert_eq!(error.code, DiagnosticCode::UnbalancedDelimiter);
    }

    #[test]
    fn rejects_multiple_arbitrary_declarations() {
        let error = parse_style_list("[display:flex;color:red]")
            .expect_err("multiple declarations must fail");
        assert_eq!(error.code, DiagnosticCode::InvalidArbitraryProperty);
    }

    #[test]
    fn rejects_at_rule_as_arbitrary_property() {
        let error = parse_style_list("[@media(min-width:1px):block]")
            .expect_err("at-rules are not arbitrary properties");
        assert_eq!(error.code, DiagnosticCode::InvalidArbitraryProperty);
    }

    #[test]
    fn rejects_prefix_important() {
        let error = parse_style_list("!flex").expect_err("prefix important must fail");
        assert_eq!(error.code, DiagnosticCode::InvalidImportant);
        assert_eq!(error.suggestion.as_deref(), Some("flex!"));
        assert_eq!(error.span, SourceSpan::new(0, 1));
    }

    #[test]
    fn parses_structured_modifier_without_splitting_utility_key() {
        let parsed = parse_named_candidate_at("text-red-500/75", 10).expect("valid modifier");
        assert_eq!(parsed.body, "text-red-500");
        assert_eq!(parsed.body_span, SourceSpan::new(10, 22));
        assert_eq!(
            parsed.modifier,
            Some(OperandSyntax {
                kind: OperandKind::Bare("75".into()),
                span: SourceSpan::new(23, 25),
            })
        );
    }

    #[test]
    fn parses_arbitrary_and_custom_variable_operands() {
        assert_eq!(
            parse_operand_at("[oklch(62% 0.2 25)]", 4)
                .expect("arbitrary value")
                .kind,
            OperandKind::ArbitraryValue("oklch(62% 0.2 25)".into())
        );
        assert_eq!(
            parse_operand("(color:--label)")
                .expect("hinted custom variable")
                .kind,
            OperandKind::CustomVariable {
                type_hint: Some("color".into()),
                name: "--label".into(),
            }
        );
        assert_eq!(
            parse_operand("(--brand)")
                .expect("unhinted custom variable remains syntactically valid")
                .kind,
            OperandKind::CustomVariable {
                type_hint: None,
                name: "--brand".into(),
            }
        );
    }

    #[test]
    fn slash_inside_nested_values_is_not_a_modifier() {
        let parsed = parse_named_candidate("bg-[url('/img/a/b.svg')]").expect("nested slash");
        assert_eq!(parsed.body, "bg-[url('/img/a/b.svg')]");
        assert!(parsed.modifier.is_none());
    }

    #[test]
    fn rejects_empty_or_repeated_modifier() {
        let empty = parse_style_list("text-lg/").expect_err("empty modifier");
        assert_eq!(empty.code, DiagnosticCode::MalformedItem);
        assert_eq!(empty.span, SourceSpan::new(7, 8));

        let repeated = parse_style_list("text-lg/6/7").expect_err("repeated modifier");
        assert_eq!(repeated.code, DiagnosticCode::MalformedItem);
        assert_eq!(repeated.span, SourceSpan::new(9, 10));
    }

    #[test]
    fn rejects_noncanonical_negative_arbitrary_number() {
        let error = parse_style_list("mt-[-1rem]").expect_err("inner negative is noncanonical");
        assert_eq!(error.code, DiagnosticCode::InvalidNegative);
        assert_eq!(error.span, SourceSpan::new(3, 10));
        assert_eq!(error.suggestion.as_deref(), Some("-mt-[1rem]"));
    }

    #[test]
    fn allows_minus_inside_arbitrary_expression() {
        parse_style_list("mt-[calc(1rem-2px)]").expect("expression minus stays in value");
    }

    #[test]
    fn rejects_double_negative_marker() {
        let error = parse_style_list("--mt-4").expect_err("double negative");
        assert_eq!(error.code, DiagnosticCode::InvalidNegative);
        assert_eq!(error.span, SourceSpan::new(0, 2));
    }

    #[test]
    fn rejects_empty_and_block_arbitrary_values() {
        let empty = parse_style_list("bg-[]").expect_err("empty arbitrary value");
        assert_eq!(empty.code, DiagnosticCode::InvalidArbitraryValue);

        let block = parse_style_list("bg-[red{color:blue}]").expect_err("CSS block");
        assert_eq!(block.code, DiagnosticCode::InvalidArbitraryValue);
    }

    #[test]
    fn rejects_invalid_custom_variables() {
        for source in ["fill-()", "fill-(color:brand)", "fill-(Color:--brand)"] {
            let error = parse_style_list(source).expect_err("invalid custom variable");
            assert_eq!(
                error.code,
                DiagnosticCode::InvalidArbitraryValue,
                "{source}"
            );
        }
    }

    #[test]
    fn preserves_literal_bang_but_rejects_embedded_important() {
        parse_style_list("content-['!']").expect("quoted bang is CSS content");
        let error = parse_style_list("bg-[red!important]").expect_err("embedded important");
        assert_eq!(error.code, DiagnosticCode::InvalidImportant);
    }

    #[test]
    fn rejects_duplicate_important_marker() {
        let error = parse_style_list("flex!!").expect_err("duplicate important");
        assert_eq!(error.code, DiagnosticCode::InvalidImportant);
        assert_eq!(error.span, SourceSpan::new(4, 5));
        assert_eq!(error.suggestion.as_deref(), Some("flex!"));
    }

    #[test]
    fn points_to_opening_unclosed_delimiter_or_quote() {
        let bracket = parse_style_list("flex bg-[red").expect_err("unclosed bracket");
        assert_eq!(bracket.code, DiagnosticCode::UnbalancedDelimiter);
        assert_eq!(bracket.span, SourceSpan::new(8, 9));

        let quote = parse_style_list("content-['x]").expect_err("unclosed quote");
        assert_eq!(quote.code, DiagnosticCode::UnbalancedDelimiter);
        assert_eq!(quote.span, SourceSpan::new(9, 10));
    }

    #[test]
    fn arbitrary_selector_requires_ampersand() {
        parse_style_list("[&>p]:block").expect("reserved selector syntax");
        let error = parse_style_list("[p]:block").expect_err("selector has no anchor");
        assert_eq!(error.code, DiagnosticCode::MalformedItem);
    }

    #[test]
    fn arbitrary_property_rejects_whitespace_value_and_embedded_important() {
        let empty = parse_style_list("[color:   ]").expect_err("whitespace value");
        assert_eq!(empty.code, DiagnosticCode::InvalidArbitraryProperty);
        let important = parse_style_list("[color:red!important]").expect_err("embedded important");
        assert_eq!(important.code, DiagnosticCode::InvalidImportant);
    }

    #[test]
    fn decoded_text_validators_match_authoring_safety_rules() {
        for value in [
            "clamp(1rem,2vw,2rem)",
            "url(data:image/svg+xml,<svg></svg>)",
        ] {
            validate_arbitrary_value_text(value).expect("valid decoded arbitrary value");
        }
        for value in [
            "",
            "red!important",
            "!\\69mportant",
            "red;display:block",
            "calc(1rem",
            "([)]",
            "foo({)",
            "red/*",
            "'bad\nstring'",
        ] {
            assert!(
                validate_arbitrary_value_text(value).is_err(),
                "unsafe decoded value must fail: {value:?}"
            );
        }

        assert!(is_valid_custom_property_name("--card-gap"));
        assert!(!is_valid_custom_property_name("--bad value"));
        assert!(is_valid_arbitrary_property_name("background-color"));
        assert!(is_valid_arbitrary_property_name("--brand-2"));
        assert!(!is_valid_arbitrary_property_name("BackgroundColor"));
        assert!(!is_valid_arbitrary_property_name("color:"));

        assert!(parse_style_list("[color:red/*]").is_err());
        assert!(parse_style_list("[color:!\\69mportant]").is_err());
        assert!(parse_style_list("[color:foo({)]").is_err());
        assert!(parse_style_list("bg-['bad\nstring']").is_err());
    }

    #[test]
    fn formatter_normalizes_only_top_level_whitespace() {
        let source =
            "\n dark:md:hover:-mt-[2rem]!   [&>p]:[color:oklch(62% 0.2 25)]\tbg-accent/50 ";
        let formatted = format_style_list(&parse_style_list(source).expect("parse source"));
        assert_eq!(
            formatted,
            "dark:md:hover:-mt-[2rem]! [&>p]:[color:oklch(62% 0.2 25)] bg-accent/50"
        );
        assert_eq!(
            format_style_list(&parse_style_list(&formatted).expect("reparse formatted source")),
            formatted
        );
    }

    #[test]
    fn deterministic_generated_inputs_never_panic_or_drift() {
        const ALPHABET: &[char] = &[
            'a', 'Z', '0', '-', '_', ':', '/', '!', '[', ']', '(', ')', '{', '}', '\'', '"', ' ',
            '\t', '\n', '%', '#', '.', ',', '&', ';', '\\', 'é', '界', '🦀',
        ];
        let mut state = 0x70_6c_69_65_67_6f_c5_55_u64;
        for case in 0..5_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let length = usize::from(state.to_le_bytes()[0]) % 80;
            let mut source = String::new();
            for _ in 0..length {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                source.push(ALPHABET[usize::from(state.to_le_bytes()[0]) % ALPHABET.len()]);
            }

            let first = parse_style_list(&source);
            let second = parse_style_list(&source);
            assert_eq!(first, second, "generated case {case}: {source:?}");
            if let Ok(style) = first {
                for item in style.items {
                    assert!(item.span.start <= item.span.end, "case {case}");
                    assert!(item.span.end <= source.len(), "case {case}");
                    assert!(source.is_char_boundary(item.span.start), "case {case}");
                    assert!(source.is_char_boundary(item.span.end), "case {case}");
                }
            }
        }
    }

    #[test]
    fn every_prefix_of_complex_syntax_is_handled() {
        for source in [
            "dark:md:hover:bg-[oklch(62% 0.2 25/var(--alpha))]!",
            "[background:url(https://example.com/a_b.svg)]",
            "[&>p:not([hidden])]:content-['🦀: ]']",
        ] {
            for end in source
                .char_indices()
                .map(|(index, _)| index)
                .chain([source.len()])
            {
                let prefix = &source[..end];
                assert_eq!(parse_style_list(prefix), parse_style_list(prefix));
            }
        }
    }
}
