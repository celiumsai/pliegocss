//! Conservative source inventory for migration bridges.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;
#[cfg(windows)]
use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::application::is_portable_source_path;

/// Schema version emitted by the migration inventory producer.
pub const MIGRATION_INVENTORY_SCHEMA_VERSION: u8 = 1;
const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_CONSTRUCTS: usize = 65_535;
const MAX_SYNTAX_BYTES: usize = 4_096;
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

/// Source family inspected by the migration inventory.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MigrationSourceKind {
    /// Sass/SCSS source.
    Sass,
    /// Tailwind CSS v4 entry CSS.
    Tailwind,
    /// CSS Modules source.
    CssModules,
}

impl MigrationSourceKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Sass => "sass",
            Self::Tailwind => "tailwind",
            Self::CssModules => "css-modules",
        }
    }
}

impl std::str::FromStr for MigrationSourceKind {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "sass" => Ok(Self::Sass),
            "tailwind" => Ok(Self::Tailwind),
            "css-modules" => Ok(Self::CssModules),
            _ => Err("migration source kind must be `sass`, `tailwind`, or `css-modules`"),
        }
    }
}

/// Static classification of one observed migration construct.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MigrationDisposition {
    /// The syntax and its bounded prelude were statically inventoried.
    Static,
    /// The construct depends on control flow, interpolation, or inline generation.
    Dynamic,
    /// The bridge records the construct but has no safe semantic normalization for it.
    Unsupported,
}

/// Tailwind Preflight reliance inferred from exact import syntax.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MigrationPreflightReliance {
    /// Preflight does not apply to this source family.
    NotApplicable,
    /// No Tailwind import that enables Preflight was observed.
    NotObserved,
    /// The full Tailwind import implicitly enables Preflight.
    Implicit,
    /// The dedicated Preflight import was observed.
    Explicit,
    /// Both full and dedicated Preflight imports were observed.
    Mixed,
}

/// One exact source construct retained by the migration inventory.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MigrationConstruct {
    kind: String,
    byte_start: usize,
    byte_end: usize,
    disposition: MigrationDisposition,
    syntax: String,
}

impl MigrationConstruct {
    /// Returns the stable construct kind.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Returns the zero-based inclusive byte start.
    #[must_use]
    pub const fn byte_start(&self) -> usize {
        self.byte_start
    }

    /// Returns the zero-based exclusive byte end.
    #[must_use]
    pub const fn byte_end(&self) -> usize {
        self.byte_end
    }

    /// Returns the conservative static classification.
    #[must_use]
    pub const fn disposition(&self) -> MigrationDisposition {
        self.disposition
    }

    /// Returns the exact bounded syntax covered by the byte range.
    #[must_use]
    pub fn syntax(&self) -> &str {
        &self.syntax
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationSummary {
    constructs: usize,
    dynamic: usize,
    unsupported: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationInventoryDocument<'a> {
    schema_version: u8,
    source_kind: MigrationSourceKind,
    file: &'a str,
    source_bytes: usize,
    source_sha256: &'a str,
    preflight_reliance: MigrationPreflightReliance,
    summary: MigrationSummary,
    constructs: &'a [MigrationConstruct],
}

/// Canonical read-only inventory for one migration source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationInventory {
    source_kind: MigrationSourceKind,
    file: String,
    source_bytes: usize,
    source_sha256: String,
    preflight_reliance: MigrationPreflightReliance,
    summary: MigrationSummary,
    constructs: Vec<MigrationConstruct>,
    bytes: Vec<u8>,
}

impl MigrationInventory {
    /// Returns the source family.
    #[must_use]
    pub const fn source_kind(&self) -> MigrationSourceKind {
        self.source_kind
    }

    /// Returns the portable logical source path.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the exact input SHA-256.
    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    /// Returns the exact UTF-8 input byte count.
    #[must_use]
    pub const fn source_bytes(&self) -> usize {
        self.source_bytes
    }

    /// Returns the Tailwind Preflight classification.
    #[must_use]
    pub const fn preflight_reliance(&self) -> MigrationPreflightReliance {
        self.preflight_reliance
    }

    /// Returns constructs in exact byte order.
    #[must_use]
    pub fn constructs(&self) -> &[MigrationConstruct] {
        &self.constructs
    }

    /// Returns the number of dynamic constructs.
    #[must_use]
    pub const fn dynamic_count(&self) -> usize {
        self.summary.dynamic
    }

    /// Returns the number of unsupported constructs.
    #[must_use]
    pub const fn unsupported_count(&self) -> usize {
        self.summary.unsupported
    }

    /// Returns canonical two-space JSON with one trailing line feed.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Display for MigrationInventory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            std::str::from_utf8(&self.bytes).expect("migration inventory JSON is valid UTF-8"),
        )
    }
}

/// Failure while conservatively inventorying a migration source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationInventoryError {
    message: String,
}

impl MigrationInventoryError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for MigrationInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for MigrationInventoryError {}

/// Inventories one exact Sass, Tailwind, or CSS Modules source without executing project code.
///
/// This is a lexical, fail-closed bridge. `Static` means that the source prelude was bounded and
/// inventoried; it does not mean that `PliegoCSS` can transform the construct. Dynamic and unsupported
/// syntax remains visible rather than being silently dropped.
///
/// # Errors
///
/// Returns [`MigrationInventoryError`] for unsafe paths, kind/extension mismatch, oversized or NUL
/// input, unterminated comments/strings, defensive-limit violations, or serialization failure.
pub fn inventory_migration_source(
    source_kind: MigrationSourceKind,
    file: &str,
    source: &str,
) -> Result<MigrationInventory, MigrationInventoryError> {
    validate_input(source_kind, file, source)?;
    let masks = lexical_masks(source)?;
    let mut constructs = match source_kind {
        MigrationSourceKind::Sass => inventory_sass(source, &masks)?,
        MigrationSourceKind::Tailwind => inventory_tailwind(source, &masks)?,
        MigrationSourceKind::CssModules => inventory_css_modules(source, &masks)?,
    };
    constructs.sort_by(|left, right| {
        (left.byte_start, left.byte_end, left.kind.as_str()).cmp(&(
            right.byte_start,
            right.byte_end,
            right.kind.as_str(),
        ))
    });
    constructs.dedup();
    if constructs.len() > MAX_CONSTRUCTS {
        return Err(MigrationInventoryError::new(
            "migration inventory exceeds 65,535 constructs",
        ));
    }
    let summary = MigrationSummary {
        constructs: constructs.len(),
        dynamic: constructs
            .iter()
            .filter(|item| item.disposition == MigrationDisposition::Dynamic)
            .count(),
        unsupported: constructs
            .iter()
            .filter(|item| item.disposition == MigrationDisposition::Unsupported)
            .count(),
    };
    let preflight_reliance = derive_preflight(source_kind, &constructs);
    let source_sha256 = format!("{:x}", Sha256::digest(source.as_bytes()));
    let mut inventory = MigrationInventory {
        source_kind,
        file: file.into(),
        source_bytes: source.len(),
        source_sha256,
        preflight_reliance,
        summary,
        constructs,
        bytes: Vec::new(),
    };
    inventory.bytes = render_inventory(&inventory)?;
    Ok(inventory)
}

/// Reads and inventories one project-relative migration source without following its final link.
///
/// # Errors
///
/// Returns [`MigrationInventoryError`] when the path is not portable, any path component is a
/// symbolic link or reparse point, the file changes away from a regular bounded file, I/O fails, or
/// the source cannot be inventoried.
pub fn inventory_migration_file(
    source_kind: MigrationSourceKind,
    file: &Path,
) -> Result<MigrationInventory, MigrationInventoryError> {
    let mut current = PathBuf::new();
    let mut logical = Vec::new();
    for component in file.components() {
        let Component::Normal(value) = component else {
            return Err(MigrationInventoryError::new(
                "migration inventory requires a portable project-relative file",
            ));
        };
        current.push(value);
        logical.push(value.to_str().ok_or_else(|| {
            MigrationInventoryError::new("migration inventory path is not valid UTF-8")
        })?);
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            MigrationInventoryError::new(format!(
                "cannot inspect migration source `{}`: {error}",
                current.display()
            ))
        })?;
        if is_link_like(&metadata) {
            return Err(MigrationInventoryError::new(format!(
                "migration source `{}` is a symbolic link or reparse point",
                current.display()
            )));
        }
    }
    let logical = logical.join("/");
    validate_input(source_kind, &logical, "")?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    #[cfg(windows)]
    options.custom_flags(0x0020_0000);
    let opened = options.open(file).map_err(|error| {
        MigrationInventoryError::new(format!(
            "cannot open migration source `{}`: {error}",
            file.display()
        ))
    })?;
    let metadata = opened.metadata().map_err(|error| {
        MigrationInventoryError::new(format!(
            "cannot inspect opened migration source `{}`: {error}",
            file.display()
        ))
    })?;
    if is_link_like(&metadata) || !metadata.is_file() || metadata.len() > MAX_SOURCE_BYTES as u64 {
        return Err(MigrationInventoryError::new(
            "migration source must be a regular file of at most 16 MiB",
        ));
    }
    let mut bytes = Vec::new();
    opened
        .take((MAX_SOURCE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            MigrationInventoryError::new(format!("cannot read migration source: {error}"))
        })?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(MigrationInventoryError::new(
            "migration source exceeds 16 MiB",
        ));
    }
    let source = std::str::from_utf8(&bytes).map_err(|error| {
        MigrationInventoryError::new(format!("migration source is not valid UTF-8: {error}"))
    })?;
    inventory_migration_source(source_kind, &logical, source)
}

fn is_link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return true;
    }
    false
}

fn render_inventory(inventory: &MigrationInventory) -> Result<Vec<u8>, MigrationInventoryError> {
    let document = MigrationInventoryDocument {
        schema_version: MIGRATION_INVENTORY_SCHEMA_VERSION,
        source_kind: inventory.source_kind,
        file: &inventory.file,
        source_bytes: inventory.source_bytes,
        source_sha256: &inventory.source_sha256,
        preflight_reliance: inventory.preflight_reliance,
        summary: inventory.summary,
        constructs: &inventory.constructs,
    };
    let mut bytes = serde_json::to_vec_pretty(&document).map_err(|error| {
        MigrationInventoryError::new(format!("cannot serialize migration inventory: {error}"))
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_input(
    source_kind: MigrationSourceKind,
    file: &str,
    source: &str,
) -> Result<(), MigrationInventoryError> {
    if !is_portable_source_path(file) {
        return Err(MigrationInventoryError::new(
            "migration inventory requires a portable project-relative file",
        ));
    }
    let path = std::path::Path::new(file);
    let extension = path.extension().and_then(std::ffi::OsStr::to_str);
    let stem_suffix = path
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(|stem| stem.rsplit('.').next());
    let extension_matches = match source_kind {
        MigrationSourceKind::Sass => extension.is_some_and(|value| {
            value.eq_ignore_ascii_case("scss") || value.eq_ignore_ascii_case("sass")
        }),
        MigrationSourceKind::Tailwind => {
            extension.is_some_and(|value| value.eq_ignore_ascii_case("css"))
        }
        MigrationSourceKind::CssModules => {
            extension.is_some_and(|value| value.eq_ignore_ascii_case("css"))
                && stem_suffix.is_some_and(|value| value.eq_ignore_ascii_case("module"))
        }
    };
    if !extension_matches {
        return Err(MigrationInventoryError::new(format!(
            "{} inventory does not accept `{file}`",
            source_kind.as_str()
        )));
    }
    if source.len() > MAX_SOURCE_BYTES {
        return Err(MigrationInventoryError::new(
            "migration source exceeds 16 MiB",
        ));
    }
    if source.contains('\0') {
        return Err(MigrationInventoryError::new(
            "migration source contains a NUL byte",
        ));
    }
    Ok(())
}

struct LexicalMasks {
    code: Vec<bool>,
    uncommented: Vec<bool>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum LexicalState {
    Code,
    SingleQuote,
    DoubleQuote,
    LineComment,
    BlockComment,
}

fn lexical_masks(source: &str) -> Result<LexicalMasks, MigrationInventoryError> {
    let bytes = source.as_bytes();
    let mut code = vec![true; bytes.len()];
    let mut uncommented = vec![true; bytes.len()];
    let mut state = LexicalState::Code;
    let mut escaped = false;
    let mut index = 0;
    while index < bytes.len() {
        match state {
            LexicalState::Code => match (bytes[index], bytes.get(index + 1).copied()) {
                (b'/', Some(b'/')) => {
                    code[index] = false;
                    code[index + 1] = false;
                    uncommented[index] = false;
                    uncommented[index + 1] = false;
                    state = LexicalState::LineComment;
                    index += 2;
                    continue;
                }
                (b'/', Some(b'*')) => {
                    code[index] = false;
                    code[index + 1] = false;
                    uncommented[index] = false;
                    uncommented[index + 1] = false;
                    state = LexicalState::BlockComment;
                    index += 2;
                    continue;
                }
                (b'\'', _) => {
                    code[index] = false;
                    state = LexicalState::SingleQuote;
                }
                (b'"', _) => {
                    code[index] = false;
                    state = LexicalState::DoubleQuote;
                }
                _ => {}
            },
            LexicalState::SingleQuote | LexicalState::DoubleQuote => {
                code[index] = false;
                if escaped {
                    escaped = false;
                } else if bytes[index] == b'\\' {
                    escaped = true;
                } else if (state == LexicalState::SingleQuote && bytes[index] == b'\'')
                    || (state == LexicalState::DoubleQuote && bytes[index] == b'"')
                {
                    state = LexicalState::Code;
                }
            }
            LexicalState::LineComment => {
                code[index] = false;
                uncommented[index] = false;
                if bytes[index] == b'\n' {
                    code[index] = true;
                    uncommented[index] = true;
                    state = LexicalState::Code;
                }
            }
            LexicalState::BlockComment => {
                code[index] = false;
                uncommented[index] = false;
                if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    code[index + 1] = false;
                    uncommented[index + 1] = false;
                    state = LexicalState::Code;
                    index += 2;
                    continue;
                }
            }
        }
        index += 1;
    }
    match state {
        LexicalState::Code | LexicalState::LineComment => Ok(LexicalMasks { code, uncommented }),
        LexicalState::SingleQuote | LexicalState::DoubleQuote => Err(MigrationInventoryError::new(
            "migration source contains an unterminated string",
        )),
        LexicalState::BlockComment => Err(MigrationInventoryError::new(
            "migration source contains an unterminated block comment",
        )),
    }
}

fn inventory_tailwind(
    source: &str,
    masks: &LexicalMasks,
) -> Result<Vec<MigrationConstruct>, MigrationInventoryError> {
    let mut output = Vec::new();
    for (marker, kind, disposition) in [
        (
            "@custom-variant",
            "tailwind-custom-variant",
            MigrationDisposition::Static,
        ),
        (
            "@reference",
            "tailwind-reference",
            MigrationDisposition::Static,
        ),
        ("@utility", "tailwind-utility", MigrationDisposition::Static),
        ("@variant", "tailwind-variant", MigrationDisposition::Static),
        ("@theme", "tailwind-theme", MigrationDisposition::Static),
        (
            "@plugin",
            "tailwind-plugin",
            MigrationDisposition::Unsupported,
        ),
        (
            "@config",
            "tailwind-config",
            MigrationDisposition::Unsupported,
        ),
        (
            "@apply",
            "tailwind-apply",
            MigrationDisposition::Unsupported,
        ),
        ("@source", "tailwind-source", MigrationDisposition::Static),
    ] {
        scan_marker(source, &masks.code, marker, kind, disposition, &mut output)?;
    }
    let mut imports = Vec::new();
    scan_marker(
        source,
        &masks.code,
        "@import",
        "tailwind-import",
        MigrationDisposition::Static,
        &mut imports,
    )?;
    output.extend(
        imports
            .into_iter()
            .filter(|item| item.syntax.contains("tailwindcss")),
    );
    for item in &mut output {
        if item.kind == "tailwind-source" && item.syntax.contains("inline(") {
            item.disposition = MigrationDisposition::Dynamic;
        }
    }
    Ok(output)
}

fn inventory_sass(
    source: &str,
    masks: &LexicalMasks,
) -> Result<Vec<MigrationConstruct>, MigrationInventoryError> {
    let mut output = Vec::new();
    for (marker, kind, disposition) in [
        ("@forward", "sass-forward", MigrationDisposition::Static),
        ("@function", "sass-function", MigrationDisposition::Dynamic),
        ("@include", "sass-include", MigrationDisposition::Dynamic),
        ("@mixin", "sass-mixin", MigrationDisposition::Dynamic),
        ("@extend", "sass-extend", MigrationDisposition::Dynamic),
        ("@import", "sass-import", MigrationDisposition::Unsupported),
        ("@while", "sass-control", MigrationDisposition::Dynamic),
        ("@each", "sass-control", MigrationDisposition::Dynamic),
        ("@else", "sass-control", MigrationDisposition::Dynamic),
        ("@for", "sass-control", MigrationDisposition::Dynamic),
        ("@if", "sass-control", MigrationDisposition::Dynamic),
        ("@use", "sass-use", MigrationDisposition::Static),
    ] {
        scan_marker(source, &masks.code, marker, kind, disposition, &mut output)?;
    }
    scan_sass_variables(source, &masks.code, &mut output)?;
    scan_interpolation(source, &masks.uncommented, &mut output)?;
    for item in &mut output {
        if matches!(item.kind.as_str(), "sass-use" | "sass-forward") && item.syntax.contains("#{") {
            item.disposition = MigrationDisposition::Dynamic;
        }
    }
    Ok(output)
}

fn inventory_css_modules(
    source: &str,
    masks: &LexicalMasks,
) -> Result<Vec<MigrationConstruct>, MigrationInventoryError> {
    let mut output = Vec::new();
    for (marker, kind, disposition) in [
        (
            ":global",
            "css-modules-global",
            MigrationDisposition::Static,
        ),
        (":local", "css-modules-local", MigrationDisposition::Static),
        (
            ":export",
            "css-modules-export",
            MigrationDisposition::Static,
        ),
        (
            ":import",
            "css-modules-import",
            MigrationDisposition::Unsupported,
        ),
        (
            "@value",
            "css-modules-value",
            MigrationDisposition::Unsupported,
        ),
    ] {
        scan_marker(source, &masks.code, marker, kind, disposition, &mut output)?;
    }
    scan_composes(source, &masks.code, &mut output)?;
    Ok(output)
}

fn scan_marker(
    source: &str,
    code: &[bool],
    marker: &str,
    kind: &str,
    disposition: MigrationDisposition,
    output: &mut Vec<MigrationConstruct>,
) -> Result<(), MigrationInventoryError> {
    let bytes = source.as_bytes();
    let marker_bytes = marker.as_bytes();
    for start in 0..bytes.len() {
        if code[start]
            && bytes[start..].starts_with(marker_bytes)
            && keyword_boundary(bytes.get(start + marker_bytes.len()).copied())
        {
            let end = construct_end(source, code, start, marker_bytes.len());
            push_construct(source, kind, start, end, disposition, output)?;
        }
    }
    Ok(())
}

fn scan_sass_variables(
    source: &str,
    code: &[bool],
    output: &mut Vec<MigrationConstruct>,
) -> Result<(), MigrationInventoryError> {
    let bytes = source.as_bytes();
    for start in 0..bytes.len() {
        if !code[start] || bytes[start] != b'$' || !is_identifier(bytes.get(start + 1).copied()) {
            continue;
        }
        let mut end = start + 2;
        while is_identifier(bytes.get(end).copied()) {
            end += 1;
        }
        push_construct(
            source,
            "sass-variable",
            start,
            end,
            MigrationDisposition::Static,
            output,
        )?;
    }
    Ok(())
}

fn scan_interpolation(
    source: &str,
    uncommented: &[bool],
    output: &mut Vec<MigrationConstruct>,
) -> Result<(), MigrationInventoryError> {
    let bytes = source.as_bytes();
    for start in 0..bytes.len().saturating_sub(1) {
        if uncommented[start] && uncommented[start + 1] && bytes[start..].starts_with(b"#{") {
            push_construct(
                source,
                "sass-interpolation",
                start,
                start + 2,
                MigrationDisposition::Dynamic,
                output,
            )?;
        }
    }
    Ok(())
}

fn scan_composes(
    source: &str,
    code: &[bool],
    output: &mut Vec<MigrationConstruct>,
) -> Result<(), MigrationInventoryError> {
    let bytes = source.as_bytes();
    let marker = b"composes";
    for start in 0..bytes.len() {
        if !code[start]
            || !bytes[start..].starts_with(marker)
            || !keyword_boundary(bytes.get(start + marker.len()).copied())
        {
            continue;
        }
        let mut delimiter = start + marker.len();
        while bytes.get(delimiter).is_some_and(u8::is_ascii_whitespace) {
            delimiter += 1;
        }
        if bytes.get(delimiter) != Some(&b':') {
            continue;
        }
        let end = construct_end(source, code, start, marker.len());
        push_construct(
            source,
            "css-modules-composes",
            start,
            end,
            MigrationDisposition::Static,
            output,
        )?;
    }
    Ok(())
}

fn push_construct(
    source: &str,
    kind: &str,
    byte_start: usize,
    byte_end: usize,
    disposition: MigrationDisposition,
    output: &mut Vec<MigrationConstruct>,
) -> Result<(), MigrationInventoryError> {
    let syntax = source
        .get(byte_start..byte_end)
        .ok_or_else(|| MigrationInventoryError::new("construct range is not valid UTF-8"))?;
    if syntax.len() > MAX_SYNTAX_BYTES {
        return Err(MigrationInventoryError::new(format!(
            "migration construct `{kind}` prelude exceeds 4 KiB"
        )));
    }
    output.push(MigrationConstruct {
        kind: kind.into(),
        byte_start,
        byte_end,
        disposition,
        syntax: syntax.into(),
    });
    Ok(())
}

fn construct_end(source: &str, code: &[bool], start: usize, marker_len: usize) -> usize {
    let bytes = source.as_bytes();
    for index in start + marker_len..bytes.len() {
        if code[index] && matches!(bytes[index], b';' | b'{') {
            return index + 1;
        }
        if code[index] && bytes[index] == b'\n' {
            return index;
        }
    }
    bytes.len()
}

fn keyword_boundary(byte: Option<u8>) -> bool {
    !is_identifier(byte)
}

fn is_identifier(byte: Option<u8>) -> bool {
    byte.is_some_and(|value| value.is_ascii_alphanumeric() || matches!(value, b'_' | b'-'))
}

fn derive_preflight(
    source_kind: MigrationSourceKind,
    constructs: &[MigrationConstruct],
) -> MigrationPreflightReliance {
    if source_kind != MigrationSourceKind::Tailwind {
        return MigrationPreflightReliance::NotApplicable;
    }
    let implicit = constructs.iter().any(|item| {
        item.kind == "tailwind-import"
            && (item.syntax.contains("\"tailwindcss\"") || item.syntax.contains("'tailwindcss'"))
    });
    let explicit = constructs.iter().any(|item| {
        item.kind == "tailwind-import"
            && (item.syntax.contains("\"tailwindcss/preflight\"")
                || item.syntax.contains("'tailwindcss/preflight'"))
    });
    match (implicit, explicit) {
        (false, false) => MigrationPreflightReliance::NotObserved,
        (true, false) => MigrationPreflightReliance::Implicit,
        (false, true) => MigrationPreflightReliance::Explicit,
        (true, true) => MigrationPreflightReliance::Mixed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn inventories_tailwind_preflight_dynamic_sources_and_unsupported_hooks() {
        let source = r#"@import "tailwindcss";
@source inline("{hover:,}bg-red-{50,{100..900..100},950}");
@theme { --color-brand: red; }
@plugin "./plugin.js";
.button { @apply px-4; }
"#;
        let first =
            inventory_migration_source(MigrationSourceKind::Tailwind, "src/tailwind.css", source)
                .unwrap();
        let second =
            inventory_migration_source(MigrationSourceKind::Tailwind, "src/tailwind.css", source)
                .unwrap();
        assert_eq!(first.as_bytes(), second.as_bytes());
        assert_eq!(
            first.preflight_reliance(),
            MigrationPreflightReliance::Implicit
        );
        assert_eq!(first.constructs().len(), 5);
        assert_eq!(first.summary.dynamic, 1);
        assert_eq!(first.summary.unsupported, 2);
        assert!(
            first
                .constructs()
                .iter()
                .all(|item| { source[item.byte_start()..item.byte_end()] == *item.syntax() })
        );
    }

    #[test]
    fn inventories_sass_without_treating_comments_as_code() {
        let source = r#"// @import "ignored";
$color: red;
@mixin button($size) { color: $color; }
.button-#{"large"} { @include button(2rem); }
"#;
        let inventory =
            inventory_migration_source(MigrationSourceKind::Sass, "src/app.scss", source).unwrap();
        assert_eq!(
            inventory.preflight_reliance(),
            MigrationPreflightReliance::NotApplicable
        );
        assert!(
            inventory
                .constructs()
                .iter()
                .all(|item| item.kind() != "sass-import")
        );
        assert!(
            inventory
                .constructs()
                .iter()
                .any(|item| item.kind() == "sass-interpolation")
        );
        assert!(inventory.summary.dynamic >= 3);
    }

    #[test]
    fn inventories_css_modules_scopes_composition_and_icss_extensions() {
        let source = r":local(.button) {
  composes: reset from global;
}
:global(.legacy) {}
@value brand: red;
:export { brand: red; }
";
        let inventory = inventory_migration_source(
            MigrationSourceKind::CssModules,
            "src/button.module.css",
            source,
        )
        .unwrap();
        assert_eq!(inventory.constructs().len(), 5);
        assert_eq!(inventory.summary.unsupported, 1);
        let document: serde_json::Value = serde_json::from_slice(inventory.as_bytes()).unwrap();
        assert_eq!(document["schemaVersion"], 1);
        assert_eq!(document["sourceKind"], "css-modules");
        assert_eq!(document["summary"]["constructs"], 5);
    }

    #[test]
    fn rejects_unsafe_paths_kind_mismatches_and_unterminated_lexemes() {
        assert!(inventory_migration_source(MigrationSourceKind::Sass, "../app.scss", "").is_err());
        assert!(
            inventory_migration_source(MigrationSourceKind::Tailwind, "C:/app.css", "").is_err()
        );
        assert!(
            inventory_migration_source(MigrationSourceKind::Tailwind, "src/aux.css", "").is_err()
        );
        assert!(
            inventory_migration_source(MigrationSourceKind::CssModules, "src/app.css", "").is_err()
        );
        assert!(
            inventory_migration_source(
                MigrationSourceKind::Tailwind,
                "src/app.css",
                "/* unterminated",
            )
            .is_err()
        );
    }

    #[test]
    fn file_inventory_reads_regular_sources_and_rejects_links() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = PathBuf::from(format!(
            ".migration-file-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let source = directory.join("app.css");
        fs::write(&source, "@import \"tailwindcss\";\n").unwrap();
        let inventory = inventory_migration_file(MigrationSourceKind::Tailwind, &source).unwrap();
        assert_eq!(
            inventory.file(),
            source.to_string_lossy().replace('\\', "/")
        );
        #[cfg(unix)]
        {
            let link = directory.join("linked.css");
            std::os::unix::fs::symlink("app.css", &link).unwrap();
            assert!(inventory_migration_file(MigrationSourceKind::Tailwind, &link).is_err());
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
