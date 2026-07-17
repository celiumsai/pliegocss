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
const MAX_PROJECT_SOURCES: usize = 4_096;
const MAX_PROJECT_DOCUMENT_BYTES: usize = 1024 * 1024;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationProjectSummary {
    sources: usize,
    consumers: usize,
    auxiliaries: usize,
    sass_sources: usize,
    tailwind_sources: usize,
    css_modules_sources: usize,
    constructs: usize,
    dynamic: usize,
    unsupported: usize,
    dependencies: usize,
    resolved_dependencies: usize,
    external_dependencies: usize,
    unresolved_dependencies: usize,
    dynamic_dependencies: usize,
    consumer_imports: usize,
    static_consumer_usages: usize,
    dynamic_consumer_usages: usize,
    tailwind_configs: usize,
    tailwind_plugins: usize,
    tailwind_templates: usize,
    static_template_candidates: usize,
    dynamic_template_candidates: usize,
    tailwind_config_keys: usize,
    tailwind_plugin_apis: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationProjectDocument<'a> {
    schema_version: u8,
    summary: MigrationProjectSummary,
    sources: Vec<MigrationInventoryDocument<'a>>,
    dependencies: &'a [MigrationDependency],
    consumers: &'a [MigrationConsumerInventory],
    auxiliaries: &'a [MigrationAuxiliaryInventory],
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

/// Explicit source declaration for a migration project snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationProjectSource {
    source_kind: MigrationSourceKind,
    file: String,
}

/// Explicit consumer family inspected by a migration project snapshot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MigrationConsumerKind {
    /// JavaScript/TypeScript module consuming CSS Modules exports.
    CssModules,
}

/// Explicit consumer declaration for a migration project snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationProjectConsumer {
    consumer_kind: MigrationConsumerKind,
    file: String,
}

/// Explicit Tailwind-owned auxiliary family inspected by a migration snapshot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MigrationAuxiliaryKind {
    /// JavaScript/TypeScript Tailwind configuration module.
    TailwindConfig,
    /// JavaScript/TypeScript Tailwind plugin module.
    TailwindPlugin,
    /// Exact template or component file scanned by Tailwind.
    TailwindTemplate,
}

impl MigrationAuxiliaryKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::TailwindConfig => "tailwind-config",
            Self::TailwindPlugin => "tailwind-plugin",
            Self::TailwindTemplate => "tailwind-template",
        }
    }
}

/// Explicit auxiliary declaration for a migration project snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationProjectAuxiliary {
    auxiliary_kind: MigrationAuxiliaryKind,
    file: String,
}

/// Canonical content identity for one declared Tailwind auxiliary file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MigrationAuxiliaryInventory {
    auxiliary_kind: MigrationAuxiliaryKind,
    file: String,
    source_bytes: usize,
    source_sha256: String,
    observations: Vec<MigrationAuxiliaryObservation>,
}

/// Kind of conservative observation retained from a Tailwind auxiliary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MigrationAuxiliaryObservationKind {
    /// One class candidate or dynamic class-bearing attribute from a template tag.
    ClassCandidate,
    /// Recognized top-level-like Tailwind configuration key seam.
    ConfigKey,
    /// Recognized Tailwind plugin registration API call seam.
    PluginApi,
}

/// One exact observation derived from a declared Tailwind auxiliary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MigrationAuxiliaryObservation {
    kind: MigrationAuxiliaryObservationKind,
    byte_start: usize,
    byte_end: usize,
    disposition: MigrationDisposition,
    value: Option<String>,
}

/// Kind of exact observation retained from a migration consumer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MigrationConsumerObservationKind {
    /// Static default/namespace import of a CSS Modules source.
    Import,
    /// Property or bracket access through the imported binding.
    ClassUsage,
}

/// One exact import or class usage observed in a migration consumer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MigrationConsumerObservation {
    kind: MigrationConsumerObservationKind,
    byte_start: usize,
    byte_end: usize,
    disposition: MigrationDisposition,
    binding: Option<String>,
    specifier: Option<String>,
    target: Option<String>,
    class_name: Option<String>,
}

/// Canonical read-only inventory for one declared migration consumer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MigrationConsumerInventory {
    consumer_kind: MigrationConsumerKind,
    file: String,
    source_bytes: usize,
    source_sha256: String,
    observations: Vec<MigrationConsumerObservation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct MigrationProjectDeclaration {
    schema_version: u8,
    sources: Vec<MigrationProjectDeclarationSource>,
    #[serde(default)]
    consumers: Vec<MigrationProjectDeclarationConsumer>,
    #[serde(default)]
    auxiliaries: Vec<MigrationProjectDeclarationAuxiliary>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct MigrationProjectDeclarationSource {
    source_kind: MigrationSourceKind,
    file: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct MigrationProjectDeclarationConsumer {
    consumer_kind: MigrationConsumerKind,
    file: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct MigrationProjectDeclarationAuxiliary {
    auxiliary_kind: MigrationAuxiliaryKind,
    file: String,
}

/// Dependency seam observed in a migration source.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MigrationDependencyKind {
    /// Sass `@use`.
    SassUse,
    /// Sass `@forward`.
    SassForward,
    /// Legacy Sass `@import`.
    SassImport,
    /// Tailwind/CSS `@import`.
    TailwindImport,
    /// Tailwind `@reference`.
    TailwindReference,
    /// Tailwind `@config` seam.
    TailwindConfig,
    /// Tailwind `@plugin` seam.
    TailwindPlugin,
    /// Tailwind `@source` template/discovery seam.
    TailwindSource,
    /// CSS Modules `composes: ... from ...`.
    CssModulesComposes,
    /// ICSS `:import(...)`.
    CssModulesImport,
    /// ICSS `@value ... from ...`.
    CssModulesValue,
}

/// Conservative resolution result for one dependency seam.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MigrationDependencyResolution {
    /// Exact portable relative target exists in the declared snapshot with the expected kind.
    Resolved,
    /// The construct refers only to the current source.
    Local,
    /// Package, built-in, URL, absolute browser path, or otherwise external reference.
    External,
    /// Static syntax is visible but requires source-toolchain-specific resolution.
    Unresolved,
    /// Dynamic syntax prevents a static specifier.
    Dynamic,
}

/// One canonical dependency observation derived from an exact source construct.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MigrationDependency {
    from: String,
    kind: MigrationDependencyKind,
    byte_start: usize,
    byte_end: usize,
    specifier: Option<String>,
    resolution: MigrationDependencyResolution,
    target: Option<String>,
}

impl MigrationDependency {
    /// Returns the source containing this dependency.
    #[must_use]
    pub fn from(&self) -> &str {
        &self.from
    }

    /// Returns the dependency family.
    #[must_use]
    pub const fn kind(&self) -> MigrationDependencyKind {
        self.kind
    }

    /// Returns the conservative resolution result.
    #[must_use]
    pub const fn resolution(&self) -> MigrationDependencyResolution {
        self.resolution
    }

    /// Returns the zero-based inclusive byte start in the containing source.
    #[must_use]
    pub const fn byte_start(&self) -> usize {
        self.byte_start
    }

    /// Returns the zero-based exclusive byte end in the containing source.
    #[must_use]
    pub const fn byte_end(&self) -> usize {
        self.byte_end
    }

    /// Returns the exact observed specifier when statically available.
    #[must_use]
    pub fn specifier(&self) -> Option<&str> {
        self.specifier.as_deref()
    }

    /// Returns the normalized declared target for resolved or local edges.
    #[must_use]
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }
}

impl MigrationProjectSource {
    /// Declares one source kind and portable project-relative path.
    #[must_use]
    pub fn new(source_kind: MigrationSourceKind, file: impl Into<String>) -> Self {
        Self {
            source_kind,
            file: file.into(),
        }
    }
}

impl MigrationProjectConsumer {
    /// Declares one consumer kind and portable project-relative path.
    #[must_use]
    pub fn new(consumer_kind: MigrationConsumerKind, file: impl Into<String>) -> Self {
        Self {
            consumer_kind,
            file: file.into(),
        }
    }
}

impl MigrationProjectAuxiliary {
    /// Declares one auxiliary kind and portable project-relative path.
    #[must_use]
    pub fn new(auxiliary_kind: MigrationAuxiliaryKind, file: impl Into<String>) -> Self {
        Self {
            auxiliary_kind,
            file: file.into(),
        }
    }
}

impl MigrationAuxiliaryInventory {
    /// Returns the auxiliary family.
    #[must_use]
    pub const fn auxiliary_kind(&self) -> MigrationAuxiliaryKind {
        self.auxiliary_kind
    }

    /// Returns the portable logical path.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the exact UTF-8 input byte count.
    #[must_use]
    pub const fn source_bytes(&self) -> usize {
        self.source_bytes
    }

    /// Returns the exact input SHA-256.
    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    /// Returns exact observations in source byte order.
    #[must_use]
    pub fn observations(&self) -> &[MigrationAuxiliaryObservation] {
        &self.observations
    }
}

impl MigrationAuxiliaryObservation {
    /// Returns the observation kind.
    #[must_use]
    pub const fn kind(&self) -> MigrationAuxiliaryObservationKind {
        self.kind
    }

    /// Returns the conservative static classification.
    #[must_use]
    pub const fn disposition(&self) -> MigrationDisposition {
        self.disposition
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

    /// Returns the exact static class candidate, or `None` for a dynamic attribute.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }
}

impl MigrationConsumerObservation {
    /// Returns the observation kind.
    #[must_use]
    pub const fn kind(&self) -> MigrationConsumerObservationKind {
        self.kind
    }

    /// Returns the conservative static classification.
    #[must_use]
    pub const fn disposition(&self) -> MigrationDisposition {
        self.disposition
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

    /// Returns the imported local binding when available.
    #[must_use]
    pub fn binding(&self) -> Option<&str> {
        self.binding.as_deref()
    }

    /// Returns the exact import specifier when available.
    #[must_use]
    pub fn specifier(&self) -> Option<&str> {
        self.specifier.as_deref()
    }

    /// Returns the normalized declared CSS Modules target when available.
    #[must_use]
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    /// Returns the statically visible class export when available.
    #[must_use]
    pub fn class_name(&self) -> Option<&str> {
        self.class_name.as_deref()
    }
}

impl MigrationConsumerInventory {
    /// Returns the consumer family.
    #[must_use]
    pub const fn consumer_kind(&self) -> MigrationConsumerKind {
        self.consumer_kind
    }

    /// Returns the portable logical path.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the exact input SHA-256.
    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    /// Returns observations in exact byte order.
    #[must_use]
    pub fn observations(&self) -> &[MigrationConsumerObservation] {
        &self.observations
    }
}

/// Closed, explicit input set for one confirmed migration project snapshot.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MigrationProject {
    sources: Vec<MigrationProjectSource>,
    consumers: Vec<MigrationProjectConsumer>,
    auxiliaries: Vec<MigrationProjectAuxiliary>,
}

#[derive(Eq, PartialEq)]
struct MigrationProjectPass {
    sources: Vec<MigrationInventory>,
    consumers: Vec<MigrationConsumerInventory>,
    auxiliaries: Vec<MigrationAuxiliaryInventory>,
}

impl MigrationProject {
    /// Creates an empty project declaration.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sources: Vec::new(),
            consumers: Vec::new(),
            auxiliaries: Vec::new(),
        }
    }

    /// Parses a closed schema-1 project declaration without reading any source file.
    ///
    /// Declaration order is not significant; [`Self::collect`] canonicalizes it before reading.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationInventoryError`] for input above 1 MiB, malformed or non-schema-1 JSON,
    /// unknown fields, more than 4,096 combined entries, or unsafe/kind-incompatible paths.
    pub fn from_json(bytes: &[u8]) -> Result<Self, MigrationInventoryError> {
        if bytes.len() > MAX_PROJECT_DOCUMENT_BYTES {
            return Err(MigrationInventoryError::new(
                "migration project declaration exceeds 1 MiB",
            ));
        }
        let declaration: MigrationProjectDeclaration =
            serde_json::from_slice(bytes).map_err(|error| {
                MigrationInventoryError::new(format!(
                    "cannot parse migration project declaration: {error}"
                ))
            })?;
        if declaration.schema_version != MIGRATION_INVENTORY_SCHEMA_VERSION {
            return Err(MigrationInventoryError::new(
                "migration project declaration schemaVersion must be 1",
            ));
        }
        if declaration.sources.len() + declaration.consumers.len() + declaration.auxiliaries.len()
            > MAX_PROJECT_SOURCES
        {
            return Err(MigrationInventoryError::new(
                "migration project exceeds 4,096 declared files",
            ));
        }
        let mut project = Self::new();
        for source in declaration.sources {
            validate_input(source.source_kind, source.file.as_str(), "")?;
            project
                .sources
                .push(MigrationProjectSource::new(source.source_kind, source.file));
        }
        for consumer in declaration.consumers {
            validate_consumer_path(consumer.consumer_kind, consumer.file.as_str())?;
            project.consumers.push(MigrationProjectConsumer::new(
                consumer.consumer_kind,
                consumer.file,
            ));
        }
        for auxiliary in declaration.auxiliaries {
            validate_auxiliary_path(auxiliary.auxiliary_kind, auxiliary.file.as_str())?;
            project.auxiliaries.push(MigrationProjectAuxiliary::new(
                auxiliary.auxiliary_kind,
                auxiliary.file,
            ));
        }
        Ok(project)
    }

    /// Reads a bounded regular project declaration without following link-like path components.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationInventoryError`] when the declaration path is unsafe, a component is a
    /// symbolic link or reparse point, the file is not regular or exceeds 1 MiB, I/O fails, or the
    /// closed schema-1 declaration is invalid.
    pub fn from_file(file: &Path) -> Result<Self, MigrationInventoryError> {
        let (_, bytes) = read_regular_project_file(
            file,
            MAX_PROJECT_DOCUMENT_BYTES,
            "migration project declaration",
        )?;
        Self::from_json(&bytes)
    }

    /// Adds one explicit source. Collection sorts declarations canonically.
    #[must_use]
    pub fn source(mut self, source: MigrationProjectSource) -> Self {
        self.sources.push(source);
        self
    }

    /// Adds one explicit consumer. Collection sorts declarations canonically.
    #[must_use]
    pub fn consumer(mut self, consumer: MigrationProjectConsumer) -> Self {
        self.consumers.push(consumer);
        self
    }

    /// Adds one explicit Tailwind auxiliary. Collection sorts declarations canonically.
    #[must_use]
    pub fn auxiliary(mut self, auxiliary: MigrationProjectAuxiliary) -> Self {
        self.auxiliaries.push(auxiliary);
        self
    }

    /// Reads every source and consumer twice and emits only when both complete inventories agree.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationInventoryError`] for empty, duplicate, unsafe, unstable, malformed, or
    /// over-limit project declarations and for any source inventory failure.
    pub fn collect(mut self) -> Result<MigrationProjectInventory, MigrationInventoryError> {
        self.canonicalize()?;
        let first = self.inventory_once()?;
        let second = self.inventory_once()?;
        if first != second {
            return Err(MigrationInventoryError::new(
                "migration project changed while its snapshot was being collected",
            ));
        }
        MigrationProjectInventory::from_parts(first.sources, first.consumers, first.auxiliaries)
    }

    fn canonicalize(&mut self) -> Result<(), MigrationInventoryError> {
        if self.sources.is_empty() {
            return Err(MigrationInventoryError::new(
                "migration project requires at least one source",
            ));
        }
        if self.sources.len() + self.consumers.len() + self.auxiliaries.len() > MAX_PROJECT_SOURCES
        {
            return Err(MigrationInventoryError::new(
                "migration project exceeds 4,096 declared files",
            ));
        }
        self.sources.sort_by(|left, right| {
            (left.file.as_str(), left.source_kind.as_str())
                .cmp(&(right.file.as_str(), right.source_kind.as_str()))
        });
        for pair in self.sources.windows(2) {
            if pair[0].file == pair[1].file {
                return Err(MigrationInventoryError::new(format!(
                    "migration project source `{}` is declared more than once",
                    pair[0].file
                )));
            }
        }
        self.consumers.sort_by(|left, right| {
            (left.file.as_str(), left.consumer_kind as u8)
                .cmp(&(right.file.as_str(), right.consumer_kind as u8))
        });
        for pair in self.consumers.windows(2) {
            if pair[0].file == pair[1].file {
                return Err(MigrationInventoryError::new(format!(
                    "migration project consumer `{}` is declared more than once",
                    pair[0].file
                )));
            }
        }
        self.auxiliaries.sort_by(|left, right| {
            (left.file.as_str(), left.auxiliary_kind)
                .cmp(&(right.file.as_str(), right.auxiliary_kind))
        });
        for pair in self.auxiliaries.windows(2) {
            if pair[0].file == pair[1].file {
                return Err(MigrationInventoryError::new(format!(
                    "migration project auxiliary `{}` is declared more than once",
                    pair[0].file
                )));
            }
        }
        let mut roles = self
            .sources
            .iter()
            .map(|item| item.file.as_str())
            .collect::<Vec<_>>();
        roles.extend(self.consumers.iter().map(|item| item.file.as_str()));
        roles.extend(self.auxiliaries.iter().map(|item| item.file.as_str()));
        roles.sort_unstable();
        if roles.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(MigrationInventoryError::new(
                "migration project path cannot have more than one declared role",
            ));
        }
        Ok(())
    }

    fn inventory_once(&self) -> Result<MigrationProjectPass, MigrationInventoryError> {
        let sources = self
            .sources
            .iter()
            .map(|source| {
                inventory_migration_file(source.source_kind, Path::new(source.file.as_str()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let consumers = self
            .consumers
            .iter()
            .map(|consumer| {
                inventory_migration_consumer_file(
                    consumer.consumer_kind,
                    Path::new(consumer.file.as_str()),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let auxiliaries = self
            .auxiliaries
            .iter()
            .map(|auxiliary| {
                inventory_migration_auxiliary_file(
                    auxiliary.auxiliary_kind,
                    Path::new(auxiliary.file.as_str()),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MigrationProjectPass {
            sources,
            consumers,
            auxiliaries,
        })
    }
}

/// Canonical, immutable snapshot of every explicitly declared migration source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationProjectInventory {
    summary: MigrationProjectSummary,
    sources: Vec<MigrationInventory>,
    dependencies: Vec<MigrationDependency>,
    consumers: Vec<MigrationConsumerInventory>,
    auxiliaries: Vec<MigrationAuxiliaryInventory>,
    bytes: Vec<u8>,
}

impl MigrationProjectInventory {
    fn from_parts(
        sources: Vec<MigrationInventory>,
        mut consumers: Vec<MigrationConsumerInventory>,
        auxiliaries: Vec<MigrationAuxiliaryInventory>,
    ) -> Result<Self, MigrationInventoryError> {
        let dependencies = derive_project_dependencies(&sources, &auxiliaries)?;
        resolve_consumer_targets(&sources, &mut consumers)?;
        let summary = migration_project_summary(&sources, &dependencies, &consumers, &auxiliaries);
        let mut inventory = Self {
            summary,
            sources,
            dependencies,
            consumers,
            auxiliaries,
            bytes: Vec::new(),
        };
        let document = MigrationProjectDocument {
            schema_version: MIGRATION_INVENTORY_SCHEMA_VERSION,
            summary,
            sources: inventory.sources.iter().map(inventory_document).collect(),
            dependencies: &inventory.dependencies,
            consumers: &inventory.consumers,
            auxiliaries: &inventory.auxiliaries,
        };
        inventory.bytes = serde_json::to_vec_pretty(&document).map_err(|error| {
            MigrationInventoryError::new(format!(
                "cannot serialize migration project inventory: {error}"
            ))
        })?;
        inventory.bytes.push(b'\n');
        Ok(inventory)
    }

    /// Returns the canonically ordered per-source inventories.
    #[must_use]
    pub fn sources(&self) -> &[MigrationInventory] {
        &self.sources
    }

    /// Returns dependency observations in canonical source and byte order.
    #[must_use]
    pub fn dependencies(&self) -> &[MigrationDependency] {
        &self.dependencies
    }

    /// Returns declared consumer inventories in canonical path order.
    #[must_use]
    pub fn consumers(&self) -> &[MigrationConsumerInventory] {
        &self.consumers
    }

    /// Returns declared Tailwind auxiliary inventories in canonical path order.
    #[must_use]
    pub fn auxiliaries(&self) -> &[MigrationAuxiliaryInventory] {
        &self.auxiliaries
    }

    /// Returns the total number of recorded constructs.
    #[must_use]
    pub const fn construct_count(&self) -> usize {
        self.summary.constructs
    }

    /// Returns canonical two-space JSON with one trailing line feed.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

fn migration_project_summary(
    sources: &[MigrationInventory],
    dependencies: &[MigrationDependency],
    consumers: &[MigrationConsumerInventory],
    auxiliaries: &[MigrationAuxiliaryInventory],
) -> MigrationProjectSummary {
    MigrationProjectSummary {
        sources: sources.len(),
        consumers: consumers.len(),
        auxiliaries: auxiliaries.len(),
        sass_sources: sources
            .iter()
            .filter(|source| source.source_kind == MigrationSourceKind::Sass)
            .count(),
        tailwind_sources: sources
            .iter()
            .filter(|source| source.source_kind == MigrationSourceKind::Tailwind)
            .count(),
        css_modules_sources: sources
            .iter()
            .filter(|source| source.source_kind == MigrationSourceKind::CssModules)
            .count(),
        constructs: sources.iter().map(|source| source.summary.constructs).sum(),
        dynamic: sources.iter().map(|source| source.summary.dynamic).sum(),
        unsupported: sources
            .iter()
            .map(|source| source.summary.unsupported)
            .sum(),
        dependencies: dependencies.len(),
        resolved_dependencies: dependencies
            .iter()
            .filter(|dependency| {
                matches!(
                    dependency.resolution,
                    MigrationDependencyResolution::Resolved | MigrationDependencyResolution::Local
                )
            })
            .count(),
        external_dependencies: dependencies
            .iter()
            .filter(|dependency| dependency.resolution == MigrationDependencyResolution::External)
            .count(),
        unresolved_dependencies: dependencies
            .iter()
            .filter(|dependency| dependency.resolution == MigrationDependencyResolution::Unresolved)
            .count(),
        dynamic_dependencies: dependencies
            .iter()
            .filter(|dependency| dependency.resolution == MigrationDependencyResolution::Dynamic)
            .count(),
        consumer_imports: consumers
            .iter()
            .flat_map(|consumer| &consumer.observations)
            .filter(|observation| observation.kind == MigrationConsumerObservationKind::Import)
            .count(),
        static_consumer_usages: consumers
            .iter()
            .flat_map(|consumer| &consumer.observations)
            .filter(|observation| {
                observation.kind == MigrationConsumerObservationKind::ClassUsage
                    && observation.disposition == MigrationDisposition::Static
            })
            .count(),
        dynamic_consumer_usages: consumers
            .iter()
            .flat_map(|consumer| &consumer.observations)
            .filter(|observation| {
                observation.kind == MigrationConsumerObservationKind::ClassUsage
                    && observation.disposition == MigrationDisposition::Dynamic
            })
            .count(),
        tailwind_configs: auxiliaries
            .iter()
            .filter(|item| item.auxiliary_kind == MigrationAuxiliaryKind::TailwindConfig)
            .count(),
        tailwind_plugins: auxiliaries
            .iter()
            .filter(|item| item.auxiliary_kind == MigrationAuxiliaryKind::TailwindPlugin)
            .count(),
        tailwind_templates: auxiliaries
            .iter()
            .filter(|item| item.auxiliary_kind == MigrationAuxiliaryKind::TailwindTemplate)
            .count(),
        static_template_candidates: auxiliaries
            .iter()
            .flat_map(|item| &item.observations)
            .filter(|item| item.disposition == MigrationDisposition::Static)
            .count(),
        dynamic_template_candidates: auxiliaries
            .iter()
            .flat_map(|item| &item.observations)
            .filter(|item| item.disposition == MigrationDisposition::Dynamic)
            .count(),
        tailwind_config_keys: auxiliaries
            .iter()
            .flat_map(|item| &item.observations)
            .filter(|item| item.kind == MigrationAuxiliaryObservationKind::ConfigKey)
            .count(),
        tailwind_plugin_apis: auxiliaries
            .iter()
            .flat_map(|item| &item.observations)
            .filter(|item| item.kind == MigrationAuxiliaryObservationKind::PluginApi)
            .count(),
    }
}

impl fmt::Display for MigrationProjectInventory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            std::str::from_utf8(&self.bytes)
                .expect("migration project inventory JSON is valid UTF-8"),
        )
    }
}

fn derive_project_dependencies(
    sources: &[MigrationInventory],
    auxiliaries: &[MigrationAuxiliaryInventory],
) -> Result<Vec<MigrationDependency>, MigrationInventoryError> {
    let declared = sources
        .iter()
        .map(|source| (source.file.as_str(), source.source_kind))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut dependencies = Vec::new();
    let declared_auxiliaries = auxiliaries
        .iter()
        .map(|item| (item.file.as_str(), item.auxiliary_kind))
        .collect::<std::collections::BTreeMap<_, _>>();
    for source in sources {
        for construct in &source.constructs {
            let Some(kind) = dependency_kind(construct.kind.as_str()) else {
                continue;
            };
            dependencies.extend(observe_dependencies(
                source,
                construct,
                kind,
                &declared,
                &declared_auxiliaries,
            )?);
        }
    }
    Ok(dependencies)
}

fn resolve_consumer_targets(
    sources: &[MigrationInventory],
    consumers: &mut [MigrationConsumerInventory],
) -> Result<(), MigrationInventoryError> {
    let declared = sources
        .iter()
        .map(|source| (source.file.as_str(), source.source_kind))
        .collect::<std::collections::BTreeMap<_, _>>();
    for consumer in consumers {
        for observation in &consumer.observations {
            let Some(target) = observation.target.as_deref() else {
                continue;
            };
            let Some(kind) = declared.get(target).copied() else {
                return Err(MigrationInventoryError::new(format!(
                    "CSS Modules consumer `{}` references undeclared local source `{target}`",
                    consumer.file
                )));
            };
            if kind != MigrationSourceKind::CssModules {
                return Err(MigrationInventoryError::new(format!(
                    "CSS Modules consumer `{}` requires css-modules target `{target}`, but it is declared as {}",
                    consumer.file,
                    kind.as_str()
                )));
            }
        }
    }
    Ok(())
}

fn dependency_kind(kind: &str) -> Option<MigrationDependencyKind> {
    match kind {
        "sass-use" => Some(MigrationDependencyKind::SassUse),
        "sass-forward" => Some(MigrationDependencyKind::SassForward),
        "sass-import" => Some(MigrationDependencyKind::SassImport),
        "tailwind-import" => Some(MigrationDependencyKind::TailwindImport),
        "tailwind-reference" => Some(MigrationDependencyKind::TailwindReference),
        "tailwind-config" => Some(MigrationDependencyKind::TailwindConfig),
        "tailwind-plugin" => Some(MigrationDependencyKind::TailwindPlugin),
        "tailwind-source" => Some(MigrationDependencyKind::TailwindSource),
        "css-modules-composes" => Some(MigrationDependencyKind::CssModulesComposes),
        "css-modules-import" => Some(MigrationDependencyKind::CssModulesImport),
        "css-modules-value" => Some(MigrationDependencyKind::CssModulesValue),
        _ => None,
    }
}

fn observe_dependencies(
    source: &MigrationInventory,
    construct: &MigrationConstruct,
    kind: MigrationDependencyKind,
    declared: &std::collections::BTreeMap<&str, MigrationSourceKind>,
    declared_auxiliaries: &std::collections::BTreeMap<&str, MigrationAuxiliaryKind>,
) -> Result<Vec<MigrationDependency>, MigrationInventoryError> {
    let dependency = MigrationDependency {
        from: source.file.clone(),
        kind,
        byte_start: construct.byte_start,
        byte_end: construct.byte_end,
        specifier: None,
        resolution: MigrationDependencyResolution::Unresolved,
        target: None,
    };
    if construct.disposition == MigrationDisposition::Dynamic || construct.syntax.contains("#{") {
        return Ok(vec![MigrationDependency {
            resolution: MigrationDependencyResolution::Dynamic,
            ..dependency
        }]);
    }
    if kind == MigrationDependencyKind::CssModulesComposes {
        let Some(from) = keyword_suffix(&construct.syntax, "from") else {
            return Ok(vec![MigrationDependency {
                resolution: MigrationDependencyResolution::Local,
                target: Some(source.file.clone()),
                ..dependency
            }]);
        };
        if from.trim_start().starts_with("global") {
            return Ok(vec![MigrationDependency {
                specifier: Some("global".into()),
                resolution: MigrationDependencyResolution::External,
                ..dependency
            }]);
        }
    }
    let specifiers = if kind == MigrationDependencyKind::SassImport {
        quoted_dependency_specifiers(&construct.syntax)
    } else {
        quoted_dependency_specifier(kind, &construct.syntax).map(|specifier| vec![specifier])
    };
    let Some(specifiers) = specifiers else {
        if construct.syntax.contains('\\') {
            return Ok(vec![MigrationDependency {
                resolution: MigrationDependencyResolution::Dynamic,
                ..dependency
            }]);
        }
        return Ok(vec![dependency]);
    };
    specifiers
        .into_iter()
        .map(|specifier| {
            let (resolution, target) = classify_dependency(
                source.file.as_str(),
                source.source_kind,
                kind,
                specifier.as_str(),
                declared,
                declared_auxiliaries,
            )?;
            Ok(MigrationDependency {
                specifier: Some(specifier),
                resolution,
                target,
                ..dependency.clone()
            })
        })
        .collect()
}

fn keyword_suffix<'a>(syntax: &'a str, keyword: &str) -> Option<&'a str> {
    syntax.match_indices(keyword).find_map(|(index, _)| {
        let before = index
            .checked_sub(1)
            .and_then(|value| syntax.as_bytes().get(value));
        let after = syntax.as_bytes().get(index + keyword.len()).copied();
        if !is_identifier(before.copied()) && !is_identifier(after) {
            Some(&syntax[index + keyword.len()..])
        } else {
            None
        }
    })
}

fn quoted_dependency_specifier(kind: MigrationDependencyKind, syntax: &str) -> Option<String> {
    let scope = match kind {
        MigrationDependencyKind::CssModulesComposes | MigrationDependencyKind::CssModulesValue => {
            keyword_suffix(syntax, "from")?
        }
        _ => syntax,
    };
    let bytes = scope.as_bytes();
    let start = bytes.iter().position(|byte| matches!(byte, b'\'' | b'"'))?;
    let quote = bytes[start];
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            return None;
        }
        if bytes[index] == quote {
            return scope.get(start + 1..index).map(str::to_owned);
        }
        index += 1;
    }
    None
}

fn quoted_dependency_specifiers(syntax: &str) -> Option<Vec<String>> {
    let bytes = syntax.as_bytes();
    let mut specifiers = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if !matches!(bytes[index], b'\'' | b'"') {
            index += 1;
            continue;
        }
        let quote = bytes[index];
        let start = index + 1;
        index = start;
        while index < bytes.len() && bytes[index] != quote {
            if bytes[index] == b'\\' {
                return None;
            }
            index += 1;
        }
        if index == bytes.len() {
            return None;
        }
        specifiers.push(syntax.get(start..index)?.to_owned());
        index += 1;
    }
    (!specifiers.is_empty()).then_some(specifiers)
}

fn classify_dependency(
    from: &str,
    source_kind: MigrationSourceKind,
    kind: MigrationDependencyKind,
    specifier: &str,
    declared: &std::collections::BTreeMap<&str, MigrationSourceKind>,
    declared_auxiliaries: &std::collections::BTreeMap<&str, MigrationAuxiliaryKind>,
) -> Result<(MigrationDependencyResolution, Option<String>), MigrationInventoryError> {
    if !specifier.starts_with("./") && !specifier.starts_with("../") {
        return Ok((MigrationDependencyResolution::External, None));
    }
    if specifier.contains('\\') || specifier.contains(['?', '#']) {
        return Ok((MigrationDependencyResolution::Unresolved, None));
    }
    if matches!(
        kind,
        MigrationDependencyKind::TailwindConfig
            | MigrationDependencyKind::TailwindPlugin
            | MigrationDependencyKind::TailwindSource
    ) {
        if kind == MigrationDependencyKind::TailwindSource
            && (specifier.contains('*') || specifier.ends_with('/'))
        {
            return Ok((MigrationDependencyResolution::Unresolved, None));
        }
        let expected = match kind {
            MigrationDependencyKind::TailwindConfig => MigrationAuxiliaryKind::TailwindConfig,
            MigrationDependencyKind::TailwindPlugin => MigrationAuxiliaryKind::TailwindPlugin,
            MigrationDependencyKind::TailwindSource => MigrationAuxiliaryKind::TailwindTemplate,
            _ => unreachable!("matched Tailwind auxiliary dependency"),
        };
        let target = normalize_relative_target(from, specifier)?;
        let Some(actual) = declared_auxiliaries.get(target.as_str()).copied() else {
            return Ok((MigrationDependencyResolution::Unresolved, None));
        };
        if actual != expected {
            return Err(MigrationInventoryError::new(format!(
                "migration dependency `{specifier}` from `{from}` requires {} auxiliary `{target}`, but it is declared as {}",
                expected.as_str(),
                actual.as_str()
            )));
        }
        return Ok((MigrationDependencyResolution::Resolved, Some(target)));
    }
    let extension = specifier.rsplit('/').next().and_then(|name| {
        name.rsplit_once('.')
            .filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
            .map(|(_, extension)| extension)
    });
    let expected = match kind {
        MigrationDependencyKind::SassUse
        | MigrationDependencyKind::SassForward
        | MigrationDependencyKind::SassImport => match extension {
            Some(extension)
                if extension.eq_ignore_ascii_case("scss")
                    || extension.eq_ignore_ascii_case("sass") =>
            {
                MigrationSourceKind::Sass
            }
            Some(extension) if extension.eq_ignore_ascii_case("css") => {
                return Ok((MigrationDependencyResolution::External, None));
            }
            _ => return Ok((MigrationDependencyResolution::Unresolved, None)),
        },
        MigrationDependencyKind::TailwindImport | MigrationDependencyKind::TailwindReference => {
            match extension {
                Some(extension) if extension.eq_ignore_ascii_case("css") => {
                    MigrationSourceKind::Tailwind
                }
                _ => return Ok((MigrationDependencyResolution::Unresolved, None)),
            }
        }
        MigrationDependencyKind::TailwindConfig
        | MigrationDependencyKind::TailwindPlugin
        | MigrationDependencyKind::TailwindSource => unreachable!("classified above"),
        MigrationDependencyKind::CssModulesComposes
        | MigrationDependencyKind::CssModulesImport
        | MigrationDependencyKind::CssModulesValue => {
            if !specifier.to_ascii_lowercase().ends_with(".module.css") {
                return Ok((MigrationDependencyResolution::Unresolved, None));
            }
            MigrationSourceKind::CssModules
        }
    };
    debug_assert_eq!(source_kind, expected);
    let target = normalize_relative_target(from, specifier)?;
    let Some(actual) = declared.get(target.as_str()).copied() else {
        return Err(MigrationInventoryError::new(format!(
            "migration dependency `{specifier}` from `{from}` resolves to undeclared local source `{target}`"
        )));
    };
    if actual != expected {
        return Err(MigrationInventoryError::new(format!(
            "migration dependency `{specifier}` from `{from}` requires {} target `{target}`, but it is declared as {}",
            expected.as_str(),
            actual.as_str()
        )));
    }
    Ok((MigrationDependencyResolution::Resolved, Some(target)))
}

fn normalize_relative_target(
    from: &str,
    specifier: &str,
) -> Result<String, MigrationInventoryError> {
    let mut components = from.split('/').collect::<Vec<_>>();
    components.pop();
    for component in specifier.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(MigrationInventoryError::new(format!(
                        "migration dependency `{specifier}` from `{from}` escapes the project root"
                    )));
                }
            }
            value => components.push(value),
        }
    }
    let target = components.join("/");
    if !is_portable_source_path(&target) {
        return Err(MigrationInventoryError::new(format!(
            "migration dependency `{specifier}` from `{from}` is not portable"
        )));
    }
    Ok(target)
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
    let (logical, bytes) = read_regular_project_file(file, MAX_SOURCE_BYTES, "migration source")?;
    validate_input(source_kind, &logical, "")?;
    let source = std::str::from_utf8(&bytes).map_err(|error| {
        MigrationInventoryError::new(format!("migration source is not valid UTF-8: {error}"))
    })?;
    inventory_migration_source(source_kind, &logical, source)
}

/// Inventories one exact JavaScript/TypeScript migration consumer without executing it.
///
/// # Errors
///
/// Returns [`MigrationInventoryError`] for unsafe/kind-incompatible paths, oversized or malformed
/// input, unterminated lexical state, defensive-limit overflow, or unsafe relative module targets.
pub fn inventory_migration_consumer_source(
    consumer_kind: MigrationConsumerKind,
    file: &str,
    source: &str,
) -> Result<MigrationConsumerInventory, MigrationInventoryError> {
    validate_consumer_path(consumer_kind, file)?;
    if source.len() > MAX_SOURCE_BYTES || source.contains('\0') {
        return Err(MigrationInventoryError::new(
            "migration consumer must be NUL-free and at most 16 MiB",
        ));
    }
    let masks = lexical_masks(source)?;
    let mut observations = scan_css_modules_consumer(file, source, &masks)?;
    observations.sort_by(|left, right| {
        (left.byte_start, left.byte_end, left.kind as u8).cmp(&(
            right.byte_start,
            right.byte_end,
            right.kind as u8,
        ))
    });
    observations.dedup();
    if observations.len() > MAX_CONSTRUCTS {
        return Err(MigrationInventoryError::new(
            "migration consumer exceeds 65,535 observations",
        ));
    }
    Ok(MigrationConsumerInventory {
        consumer_kind,
        file: file.into(),
        source_bytes: source.len(),
        source_sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
        observations,
    })
}

/// Reads and inventories one bounded regular migration consumer without following links.
///
/// # Errors
///
/// Returns [`MigrationInventoryError`] when the path/file boundary or consumer source is invalid.
pub fn inventory_migration_consumer_file(
    consumer_kind: MigrationConsumerKind,
    file: &Path,
) -> Result<MigrationConsumerInventory, MigrationInventoryError> {
    let (logical, bytes) = read_regular_project_file(file, MAX_SOURCE_BYTES, "migration consumer")?;
    let source = std::str::from_utf8(&bytes).map_err(|error| {
        MigrationInventoryError::new(format!("migration consumer is not valid UTF-8: {error}"))
    })?;
    inventory_migration_consumer_source(consumer_kind, &logical, source)
}

/// Inventories the exact content identity of one Tailwind-owned auxiliary without executing it.
///
/// Configuration and plugin modules must also pass the bounded JavaScript lexical check. Template
/// content is retained by exact bytes/hash because arbitrary template-language semantics remain
/// outside this execution-free bridge.
///
/// # Errors
///
/// Returns [`MigrationInventoryError`] for unsafe/kind-incompatible paths, oversized or NUL input,
/// or an unterminated JavaScript lexical state in configuration and plugin modules.
pub fn inventory_migration_auxiliary_source(
    auxiliary_kind: MigrationAuxiliaryKind,
    file: &str,
    source: &str,
) -> Result<MigrationAuxiliaryInventory, MigrationInventoryError> {
    validate_auxiliary_path(auxiliary_kind, file)?;
    if source.len() > MAX_SOURCE_BYTES || source.contains('\0') {
        return Err(MigrationInventoryError::new(
            "migration auxiliary must be NUL-free and at most 16 MiB",
        ));
    }
    let mut observations = match auxiliary_kind {
        MigrationAuxiliaryKind::TailwindTemplate => scan_tailwind_template(source)?,
        MigrationAuxiliaryKind::TailwindConfig | MigrationAuxiliaryKind::TailwindPlugin => {
            let masks = lexical_masks(source)?;
            scan_tailwind_script(auxiliary_kind, source, &masks.code)
        }
    };
    observations.sort_by_key(|item| (item.byte_start, item.byte_end));
    observations.dedup();
    if observations.len() > MAX_CONSTRUCTS {
        return Err(MigrationInventoryError::new(
            "migration auxiliary exceeds 65,535 observations",
        ));
    }
    Ok(MigrationAuxiliaryInventory {
        auxiliary_kind,
        file: file.into(),
        source_bytes: source.len(),
        source_sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
        observations,
    })
}

/// Reads one bounded regular Tailwind auxiliary without following links.
///
/// # Errors
///
/// Returns [`MigrationInventoryError`] when the path/file boundary or auxiliary content is invalid.
pub fn inventory_migration_auxiliary_file(
    auxiliary_kind: MigrationAuxiliaryKind,
    file: &Path,
) -> Result<MigrationAuxiliaryInventory, MigrationInventoryError> {
    let (logical, bytes) =
        read_regular_project_file(file, MAX_SOURCE_BYTES, "migration auxiliary")?;
    let source = std::str::from_utf8(&bytes).map_err(|error| {
        MigrationInventoryError::new(format!("migration auxiliary is not valid UTF-8: {error}"))
    })?;
    inventory_migration_auxiliary_source(auxiliary_kind, &logical, source)
}

fn scan_tailwind_script(
    auxiliary_kind: MigrationAuxiliaryKind,
    source: &str,
    code: &[bool],
) -> Vec<MigrationAuxiliaryObservation> {
    let (kind, delimiter, markers): (MigrationAuxiliaryObservationKind, u8, &[&str]) =
        match auxiliary_kind {
            MigrationAuxiliaryKind::TailwindConfig => (
                MigrationAuxiliaryObservationKind::ConfigKey,
                b':',
                &[
                    "content",
                    "theme",
                    "plugins",
                    "presets",
                    "safelist",
                    "corePlugins",
                ],
            ),
            MigrationAuxiliaryKind::TailwindPlugin => (
                MigrationAuxiliaryObservationKind::PluginApi,
                b'(',
                &[
                    "addUtilities",
                    "matchUtilities",
                    "addComponents",
                    "addVariant",
                    "matchVariant",
                ],
            ),
            MigrationAuxiliaryKind::TailwindTemplate => return Vec::new(),
        };
    let bytes = source.as_bytes();
    let mut observations = Vec::new();
    for marker in markers {
        for (start, _) in source.match_indices(marker) {
            let end = start + marker.len();
            if !code[start]
                || is_identifier(
                    start
                        .checked_sub(1)
                        .and_then(|index| bytes.get(index))
                        .copied(),
                )
                || is_identifier(bytes.get(end).copied())
            {
                continue;
            }
            let mut next = end;
            while bytes.get(next).is_some_and(u8::is_ascii_whitespace) {
                next += 1;
            }
            if bytes.get(next) == Some(&delimiter) {
                observations.push(MigrationAuxiliaryObservation {
                    kind,
                    byte_start: start,
                    byte_end: end,
                    disposition: MigrationDisposition::Unsupported,
                    value: Some((*marker).into()),
                });
            }
        }
    }
    observations
}

fn scan_tailwind_template(
    source: &str,
) -> Result<Vec<MigrationAuxiliaryObservation>, MigrationInventoryError> {
    let bytes = source.as_bytes();
    let mut observations = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = bytes[cursor..].iter().position(|byte| *byte == b'<') {
        let start = cursor + offset;
        if bytes[start..].starts_with(b"<!--") {
            let Some(end) = source[start + 4..].find("-->") else {
                return Err(MigrationInventoryError::new(
                    "migration template contains an unterminated comment",
                ));
            };
            cursor = start + 4 + end + 3;
            continue;
        }
        let end = template_tag_end(bytes, start + 1)?;
        scan_template_tag(source, start + 1, end, &mut observations)?;
        cursor = end + 1;
    }
    Ok(observations)
}

fn template_tag_end(bytes: &[u8], mut cursor: usize) -> Result<usize, MigrationInventoryError> {
    let mut quote = None;
    let mut escaped = false;
    let mut braces = 0_usize;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active {
                quote = None;
            }
        } else {
            match byte {
                b'\'' | b'"' | b'`' => quote = Some(byte),
                b'{' => braces = braces.saturating_add(1),
                b'}' => braces = braces.saturating_sub(1),
                b'>' if braces == 0 => return Ok(cursor),
                _ => {}
            }
        }
        cursor += 1;
    }
    Err(MigrationInventoryError::new(
        "migration template contains an unterminated tag",
    ))
}

fn scan_template_tag(
    source: &str,
    start: usize,
    end: usize,
    observations: &mut Vec<MigrationAuxiliaryObservation>,
) -> Result<(), MigrationInventoryError> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    while cursor < end {
        let Some((name_start, name_end)) = template_class_attribute(bytes, cursor, end) else {
            break;
        };
        cursor = name_end;
        while cursor < end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            continue;
        }
        cursor += 1;
        while cursor < end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let (value_start, value_end, next) = template_attribute_value(bytes, cursor, end)?;
        push_template_candidates(source, value_start, value_end, observations);
        cursor = next.max(name_start + 1);
    }
    Ok(())
}

fn template_class_attribute(bytes: &[u8], mut cursor: usize, end: usize) -> Option<(usize, usize)> {
    let mut quote = None;
    let mut escaped = false;
    let mut braces = 0_usize;
    while cursor < end {
        let byte = bytes[cursor];
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active {
                quote = None;
            }
        } else {
            match byte {
                b'\'' | b'"' | b'`' => quote = Some(byte),
                b'{' => braces = braces.saturating_add(1),
                b'}' => braces = braces.saturating_sub(1),
                _ if braces == 0 => {
                    for name in [b"className".as_slice(), b"class".as_slice()] {
                        if bytes[cursor..end].starts_with(name)
                            && (cursor == 0 || !is_template_attribute_byte(bytes[cursor - 1]))
                            && (cursor + name.len() == end
                                || !is_template_attribute_byte(bytes[cursor + name.len()]))
                        {
                            return Some((cursor, cursor + name.len()));
                        }
                    }
                }
                _ => {}
            }
        }
        cursor += 1;
    }
    None
}

const fn is_template_attribute_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':')
}

fn template_attribute_value(
    bytes: &[u8],
    start: usize,
    end: usize,
) -> Result<(usize, usize, usize), MigrationInventoryError> {
    let Some(first) = bytes.get(start).copied() else {
        return Ok((start, start, start));
    };
    if matches!(first, b'\'' | b'"') {
        let mut cursor = start + 1;
        while cursor < end && bytes[cursor] != first {
            if bytes[cursor] == b'\\' {
                cursor += 1;
            }
            cursor += 1;
        }
        if cursor == end {
            return Err(MigrationInventoryError::new(
                "migration template contains an unterminated class attribute",
            ));
        }
        return Ok((start + 1, cursor, cursor + 1));
    }
    if first == b'{' {
        let mut cursor = start + 1;
        let mut depth = 1_usize;
        while cursor < end && depth > 0 {
            depth += usize::from(bytes[cursor] == b'{');
            depth = depth.saturating_sub(usize::from(bytes[cursor] == b'}'));
            cursor += 1;
        }
        return Ok((start, cursor, cursor));
    }
    let value_end = bytes[start..end]
        .iter()
        .position(u8::is_ascii_whitespace)
        .map_or(end, |offset| start + offset);
    Ok((start, value_end, value_end))
}

fn push_template_candidates(
    source: &str,
    start: usize,
    end: usize,
    observations: &mut Vec<MigrationAuxiliaryObservation>,
) {
    let value = &source[start..end];
    if value.contains(['{', '}', '$', '`']) {
        observations.push(MigrationAuxiliaryObservation {
            kind: MigrationAuxiliaryObservationKind::ClassCandidate,
            byte_start: start,
            byte_end: end,
            disposition: MigrationDisposition::Dynamic,
            value: None,
        });
        return;
    }
    for candidate in value.split_whitespace() {
        let offset = candidate.as_ptr() as usize - value.as_ptr() as usize;
        observations.push(MigrationAuxiliaryObservation {
            kind: MigrationAuxiliaryObservationKind::ClassCandidate,
            byte_start: start + offset,
            byte_end: start + offset + candidate.len(),
            disposition: MigrationDisposition::Static,
            value: Some(candidate.into()),
        });
    }
}

fn validate_auxiliary_path(
    auxiliary_kind: MigrationAuxiliaryKind,
    file: &str,
) -> Result<(), MigrationInventoryError> {
    if !is_portable_source_path(file) {
        return Err(MigrationInventoryError::new(
            "migration auxiliary requires a portable project-relative file",
        ));
    }
    let extension = Path::new(file)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let script = matches!(
        extension.as_str(),
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs"
    );
    let accepted = match auxiliary_kind {
        MigrationAuxiliaryKind::TailwindConfig | MigrationAuxiliaryKind::TailwindPlugin => script,
        MigrationAuxiliaryKind::TailwindTemplate => matches!(
            extension.as_str(),
            "html"
                | "htm"
                | "js"
                | "jsx"
                | "ts"
                | "tsx"
                | "vue"
                | "svelte"
                | "astro"
                | "md"
                | "mdx"
                | "php"
        ),
    };
    if !accepted {
        return Err(MigrationInventoryError::new(format!(
            "{} auxiliary inventory does not accept `{file}`",
            auxiliary_kind.as_str()
        )));
    }
    Ok(())
}

fn validate_consumer_path(
    consumer_kind: MigrationConsumerKind,
    file: &str,
) -> Result<(), MigrationInventoryError> {
    if !is_portable_source_path(file) {
        return Err(MigrationInventoryError::new(
            "migration consumer requires a portable project-relative file",
        ));
    }
    let extension = Path::new(file)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default();
    let accepted = match consumer_kind {
        MigrationConsumerKind::CssModules => matches!(
            extension.to_ascii_lowercase().as_str(),
            "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs"
        ),
    };
    if !accepted {
        return Err(MigrationInventoryError::new(format!(
            "css-modules consumer inventory does not accept `{file}`"
        )));
    }
    Ok(())
}

fn scan_css_modules_consumer(
    file: &str,
    source: &str,
    masks: &LexicalMasks,
) -> Result<Vec<MigrationConsumerObservation>, MigrationInventoryError> {
    let bytes = source.as_bytes();
    let mut observations = Vec::new();
    let mut bindings = Vec::new();
    for start in 0..bytes.len() {
        if !masks.code[start]
            || !bytes[start..].starts_with(b"import")
            || is_identifier(
                start
                    .checked_sub(1)
                    .and_then(|index| bytes.get(index))
                    .copied(),
            )
            || is_identifier(bytes.get(start + 6).copied())
        {
            continue;
        }
        let end = consumer_import_end(source, &masks.code, start);
        let syntax = &source[start..end];
        let Some(specifier) =
            quoted_dependency_specifier(MigrationDependencyKind::CssModulesImport, syntax)
        else {
            continue;
        };
        if !specifier.to_ascii_lowercase().ends_with(".module.css") {
            continue;
        }
        let target = if specifier.starts_with("./") || specifier.starts_with("../") {
            Some(normalize_relative_target(file, specifier.as_str())?)
        } else {
            None
        };
        let prelude = syntax.split(['\'', '"']).next().unwrap_or_default().trim();
        let dynamic_import = prelude
            .strip_prefix("import")
            .is_some_and(|suffix| suffix.trim_start().starts_with('('));
        let binding = (!dynamic_import).then(|| import_binding(prelude)).flatten();
        let disposition = if dynamic_import || (prelude.contains('{') && binding.is_none()) {
            MigrationDisposition::Dynamic
        } else {
            MigrationDisposition::Static
        };
        observations.push(MigrationConsumerObservation {
            kind: MigrationConsumerObservationKind::Import,
            byte_start: start,
            byte_end: end,
            disposition,
            binding: binding.clone(),
            specifier: Some(specifier),
            target: target.clone(),
            class_name: None,
        });
        if let Some(binding) = binding {
            bindings.push((binding, target, start, end));
        }
    }
    for (binding, target, import_start, import_end) in bindings {
        scan_consumer_binding_usages(
            source,
            &masks.code,
            &masks.template,
            binding.as_str(),
            target.as_deref(),
            import_start..import_end,
            &mut observations,
        );
    }
    Ok(observations)
}

fn import_binding(prelude: &str) -> Option<String> {
    let body = prelude.strip_prefix("import")?.trim();
    let declaration = body.strip_suffix("from")?.trim();
    let candidate = if let Some(namespace) = declaration.strip_prefix('*') {
        namespace.trim().strip_prefix("as")?.trim()
    } else {
        declaration.split(',').next()?.trim()
    };
    (!candidate.is_empty()
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')))
    .then(|| candidate.to_owned())
}

fn consumer_import_end(source: &str, code: &[bool], start: usize) -> usize {
    let bytes = source.as_bytes();
    for index in start + 6..bytes.len() {
        if code[index] && bytes[index] == b';' {
            return index + 1;
        }
        if code[index] && bytes[index] == b'\n' {
            return index;
        }
    }
    bytes.len()
}

fn scan_consumer_binding_usages(
    source: &str,
    code: &[bool],
    template: &[bool],
    binding: &str,
    target: Option<&str>,
    import_range: std::ops::Range<usize>,
    output: &mut Vec<MigrationConsumerObservation>,
) {
    let bytes = source.as_bytes();
    let binding_bytes = binding.as_bytes();
    for start in 0..bytes.len() {
        if import_range.contains(&start)
            || (!code[start] && !template[start])
            || !bytes[start..].starts_with(binding_bytes)
            || is_identifier(
                start
                    .checked_sub(1)
                    .and_then(|index| bytes.get(index))
                    .copied(),
            )
            || is_identifier(bytes.get(start + binding_bytes.len()).copied())
        {
            continue;
        }
        if template[start] {
            output.push(MigrationConsumerObservation {
                kind: MigrationConsumerObservationKind::ClassUsage,
                byte_start: start,
                byte_end: start + binding_bytes.len(),
                disposition: MigrationDisposition::Dynamic,
                binding: Some(binding.into()),
                specifier: None,
                target: target.map(str::to_owned),
                class_name: None,
            });
            continue;
        }
        let mut cursor = start + binding_bytes.len();
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let (end, disposition, class_name) = match bytes.get(cursor) {
            Some(b'.') => {
                cursor += 1;
                while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
                    cursor += 1;
                }
                let class_start = cursor;
                while bytes
                    .get(cursor)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
                {
                    cursor += 1;
                }
                if cursor == class_start {
                    (
                        start + binding_bytes.len(),
                        MigrationDisposition::Dynamic,
                        None,
                    )
                } else {
                    (
                        cursor,
                        MigrationDisposition::Static,
                        source.get(class_start..cursor).map(str::to_owned),
                    )
                }
            }
            Some(b'[') => scan_consumer_bracket_usage(source, cursor),
            _ => (
                start + binding_bytes.len(),
                MigrationDisposition::Dynamic,
                None,
            ),
        };
        output.push(MigrationConsumerObservation {
            kind: MigrationConsumerObservationKind::ClassUsage,
            byte_start: start,
            byte_end: end,
            disposition,
            binding: Some(binding.into()),
            specifier: None,
            target: target.map(str::to_owned),
            class_name,
        });
    }
}

fn scan_consumer_bracket_usage(
    source: &str,
    bracket: usize,
) -> (usize, MigrationDisposition, Option<String>) {
    let bytes = source.as_bytes();
    let mut cursor = bracket + 1;
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    if matches!(bytes.get(cursor), Some(b'\'' | b'"')) {
        let quote = bytes[cursor];
        let class_start = cursor + 1;
        cursor = class_start;
        while let Some(byte) = bytes.get(cursor) {
            if *byte == b'\\' {
                return (cursor + 1, MigrationDisposition::Dynamic, None);
            }
            if *byte == quote {
                let class_name = source.get(class_start..cursor).map(str::to_owned);
                cursor += 1;
                while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
                    cursor += 1;
                }
                if bytes.get(cursor) == Some(&b']')
                    && class_name.as_deref().is_some_and(|v| !v.is_empty())
                {
                    return (cursor + 1, MigrationDisposition::Static, class_name);
                }
                return (cursor, MigrationDisposition::Dynamic, None);
            }
            cursor += 1;
        }
    }
    let limit = (bracket + MAX_SYNTAX_BYTES).min(bytes.len());
    while cursor < limit && bytes[cursor] != b']' {
        cursor += 1;
    }
    if cursor < bytes.len() && bytes[cursor] == b']' {
        cursor += 1;
    }
    (cursor, MigrationDisposition::Dynamic, None)
}

fn read_regular_project_file(
    file: &Path,
    max_bytes: usize,
    role: &str,
) -> Result<(String, Vec<u8>), MigrationInventoryError> {
    let mut current = PathBuf::new();
    let mut logical = Vec::new();
    for component in file.components() {
        let Component::Normal(value) = component else {
            return Err(MigrationInventoryError::new(format!(
                "{role} requires a portable project-relative file"
            )));
        };
        current.push(value);
        logical.push(value.to_str().ok_or_else(|| {
            MigrationInventoryError::new("migration inventory path is not valid UTF-8")
        })?);
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            MigrationInventoryError::new(format!(
                "cannot inspect {role} `{}`: {error}",
                current.display()
            ))
        })?;
        if is_link_like(&metadata) {
            return Err(MigrationInventoryError::new(format!(
                "{role} `{}` is a symbolic link or reparse point",
                current.display()
            )));
        }
    }
    let logical = logical.join("/");
    if !is_portable_source_path(&logical) {
        return Err(MigrationInventoryError::new(format!(
            "{role} requires a portable project-relative file"
        )));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    #[cfg(windows)]
    options.custom_flags(0x0020_0000);
    let opened = options.open(file).map_err(|error| {
        MigrationInventoryError::new(format!("cannot open {role} `{}`: {error}", file.display()))
    })?;
    let metadata = opened.metadata().map_err(|error| {
        MigrationInventoryError::new(format!(
            "cannot inspect opened {role} `{}`: {error}",
            file.display()
        ))
    })?;
    if is_link_like(&metadata) || !metadata.is_file() || metadata.len() > max_bytes as u64 {
        return Err(MigrationInventoryError::new(format!(
            "{role} must be a regular file within its byte limit"
        )));
    }
    let mut bytes = Vec::new();
    opened
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| MigrationInventoryError::new(format!("cannot read {role}: {error}")))?;
    if bytes.len() > max_bytes {
        return Err(MigrationInventoryError::new(format!(
            "{role} exceeds its byte limit"
        )));
    }
    Ok((logical, bytes))
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
    let document = inventory_document(inventory);
    let mut bytes = serde_json::to_vec_pretty(&document).map_err(|error| {
        MigrationInventoryError::new(format!("cannot serialize migration inventory: {error}"))
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn inventory_document(inventory: &MigrationInventory) -> MigrationInventoryDocument<'_> {
    MigrationInventoryDocument {
        schema_version: MIGRATION_INVENTORY_SCHEMA_VERSION,
        source_kind: inventory.source_kind,
        file: &inventory.file,
        source_bytes: inventory.source_bytes,
        source_sha256: &inventory.source_sha256,
        preflight_reliance: inventory.preflight_reliance,
        summary: inventory.summary,
        constructs: &inventory.constructs,
    }
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
    template: Vec<bool>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum LexicalState {
    Code,
    SingleQuote,
    DoubleQuote,
    Backtick,
    LineComment,
    BlockComment,
}

fn lexical_masks(source: &str) -> Result<LexicalMasks, MigrationInventoryError> {
    let bytes = source.as_bytes();
    let mut code = vec![true; bytes.len()];
    let mut uncommented = vec![true; bytes.len()];
    let mut template = vec![false; bytes.len()];
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
                (b'`', _) => {
                    code[index] = false;
                    template[index] = true;
                    state = LexicalState::Backtick;
                }
                _ => {}
            },
            LexicalState::SingleQuote | LexicalState::DoubleQuote | LexicalState::Backtick => {
                code[index] = false;
                if state == LexicalState::Backtick {
                    template[index] = true;
                }
                if escaped {
                    escaped = false;
                } else if bytes[index] == b'\\' {
                    escaped = true;
                } else if (state == LexicalState::SingleQuote && bytes[index] == b'\'')
                    || (state == LexicalState::DoubleQuote && bytes[index] == b'"')
                    || (state == LexicalState::Backtick && bytes[index] == b'`')
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
        LexicalState::Code | LexicalState::LineComment => Ok(LexicalMasks {
            code,
            uncommented,
            template,
        }),
        LexicalState::SingleQuote | LexicalState::DoubleQuote | LexicalState::Backtick => Err(
            MigrationInventoryError::new("migration source contains an unterminated string"),
        ),
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
    scan_marker(
        source,
        &masks.code,
        "@import",
        "tailwind-import",
        MigrationDisposition::Static,
        &mut output,
    )?;
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

    #[test]
    fn project_inventory_is_canonical_complete_and_duplicate_closed() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = PathBuf::from(format!(
            ".migration-project-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let sass = directory.join("legacy.scss");
        let tailwind = directory.join("app.css");
        let module = directory.join("card.module.css");
        fs::write(&sass, "$space: 1rem;\n").unwrap();
        fs::write(&tailwind, "@import \"tailwindcss\";\n").unwrap();
        fs::write(&module, ".card { composes: base; }\n").unwrap();

        let source = |kind, path: &Path| {
            MigrationProjectSource::new(kind, path.to_string_lossy().into_owned())
        };
        let first = MigrationProject::new()
            .source(source(MigrationSourceKind::Sass, &sass))
            .source(source(MigrationSourceKind::Tailwind, &tailwind))
            .source(source(MigrationSourceKind::CssModules, &module))
            .collect()
            .unwrap();
        let second = MigrationProject::new()
            .source(source(MigrationSourceKind::CssModules, &module))
            .source(source(MigrationSourceKind::Sass, &sass))
            .source(source(MigrationSourceKind::Tailwind, &tailwind))
            .collect()
            .unwrap();
        assert_eq!(first.as_bytes(), second.as_bytes());
        assert_eq!(first.sources().len(), 3);
        assert_eq!(first.construct_count(), 3);
        let document: serde_json::Value = serde_json::from_slice(first.as_bytes()).unwrap();
        assert_eq!(document["schemaVersion"], 1);
        assert_eq!(document["summary"]["sources"], 3);
        assert_eq!(document["summary"]["sassSources"], 1);
        assert_eq!(document["summary"]["tailwindSources"], 1);
        assert_eq!(document["summary"]["cssModulesSources"], 1);
        assert_eq!(document["summary"]["dependencies"], 2);
        assert_eq!(document["summary"]["resolvedDependencies"], 1);
        assert_eq!(document["summary"]["externalDependencies"], 1);
        assert_eq!(first.dependencies().len(), 2);
        assert_eq!(
            first.dependencies()[0].resolution(),
            MigrationDependencyResolution::External
        );
        assert_eq!(
            first.dependencies()[1].resolution(),
            MigrationDependencyResolution::Local
        );
        assert!(
            first.dependencies()[1]
                .target()
                .is_some_and(|target| target.ends_with("card.module.css"))
        );

        let duplicate = MigrationProject::new()
            .source(source(MigrationSourceKind::Sass, &sass))
            .source(source(MigrationSourceKind::Sass, &sass))
            .collect();
        assert!(duplicate.is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn project_declaration_json_is_closed_bounded_and_order_independent() {
        let project = MigrationProject::from_json(
            br#"{
  "schemaVersion": 1,
  "sources": [
    {"sourceKind": "tailwind", "file": "src/app.css"},
    {"sourceKind": "sass", "file": "src/legacy.scss"}
  ]
}"#,
        )
        .unwrap();
        assert_eq!(project.sources.len(), 2);
        assert_eq!(project.sources[0].file, "src/app.css");
        assert_eq!(project.sources[1].source_kind, MigrationSourceKind::Sass);
        assert!(MigrationProject::from_json(br#"{"schemaVersion":2,"sources":[]}"#).is_err());
        assert!(
            MigrationProject::from_json(br#"{"schemaVersion":1,"sources":[],"unexpected":true}"#)
                .is_err()
        );
        assert!(
            MigrationProject::from_json(
                br#"{"schemaVersion":1,"sources":[{"sourceKind":"sass","file":"../app.scss"}]}"#
            )
            .is_err()
        );
        assert!(MigrationProject::from_json(&vec![b' '; MAX_PROJECT_DOCUMENT_BYTES + 1]).is_err());
    }

    #[test]
    fn project_inventory_resolves_exact_declared_dependency_edges() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = PathBuf::from(format!(
            ".migration-dependency-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(directory.join("styles")).unwrap();
        let app_sass = directory.join("styles/app.scss");
        let tokens_sass = directory.join("styles/tokens.scss");
        let app_css = directory.join("styles/app.css");
        let theme_css = directory.join("styles/theme.css");
        let card_module = directory.join("styles/card.module.css");
        let base_module = directory.join("styles/base.module.css");
        fs::write(&app_sass, "@use \"./tokens.scss\";\n").unwrap();
        fs::write(&tokens_sass, "$brand: red;\n").unwrap();
        fs::write(&app_css, "@reference \"./theme.css\";\n").unwrap();
        fs::write(&theme_css, "@theme { --color-brand: red; }\n").unwrap();
        fs::write(
            &card_module,
            ".card { composes: base from \"./base.module.css\"; }\n",
        )
        .unwrap();
        fs::write(&base_module, ".base { display: block; }\n").unwrap();
        let source = |kind, path: &Path| {
            MigrationProjectSource::new(kind, path.to_string_lossy().into_owned())
        };
        let inventory = MigrationProject::new()
            .source(source(MigrationSourceKind::Sass, &app_sass))
            .source(source(MigrationSourceKind::Sass, &tokens_sass))
            .source(source(MigrationSourceKind::Tailwind, &app_css))
            .source(source(MigrationSourceKind::Tailwind, &theme_css))
            .source(source(MigrationSourceKind::CssModules, &card_module))
            .source(source(MigrationSourceKind::CssModules, &base_module))
            .collect()
            .unwrap();
        assert_eq!(inventory.dependencies().len(), 3);
        assert!(inventory.dependencies().iter().all(|dependency| {
            dependency.resolution() == MigrationDependencyResolution::Resolved
                && dependency.specifier().is_some()
                && dependency.target().is_some()
                && dependency.byte_start() < dependency.byte_end()
        }));
        let targets = inventory
            .dependencies()
            .iter()
            .map(|dependency| dependency.target().unwrap())
            .collect::<Vec<_>>();
        assert!(
            targets
                .iter()
                .any(|target| target.ends_with("styles/tokens.scss"))
        );
        assert!(
            targets
                .iter()
                .any(|target| target.ends_with("styles/theme.css"))
        );
        assert!(
            targets
                .iter()
                .any(|target| target.ends_with("styles/base.module.css"))
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn project_inventory_fails_closed_for_missing_or_mistyped_local_targets() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = PathBuf::from(format!(
            ".migration-dependency-failure-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let sass = directory.join("app.scss");
        fs::write(&sass, "@use \"./missing.scss\";\n").unwrap();
        let source = |kind, path: &Path| {
            MigrationProjectSource::new(kind, path.to_string_lossy().into_owned())
        };
        let missing = MigrationProject::new()
            .source(source(MigrationSourceKind::Sass, &sass))
            .collect()
            .unwrap_err();
        assert!(missing.to_string().contains("undeclared local source"));

        let card = directory.join("card.module.css");
        let base = directory.join("base.module.css");
        fs::write(
            &card,
            ".card { composes: base from \"./base.module.css\"; }\n",
        )
        .unwrap();
        fs::write(&base, ".base { display: block; }\n").unwrap();
        let mistyped = MigrationProject::new()
            .source(source(MigrationSourceKind::CssModules, &card))
            .source(source(MigrationSourceKind::Tailwind, &base))
            .collect()
            .unwrap_err();
        assert!(mistyped.to_string().contains("declared as tailwind"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn project_inventory_exposes_unresolved_and_external_references_without_guessing() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = PathBuf::from(format!(
            ".migration-dependency-classification-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let sass = directory.join("app.scss");
        fs::write(
            &sass,
            "@use \"sass:math\";\n@use \"some-package\";\n@use \"./tokens\";\n@import \"first-package\", \"second-package\";\n",
        )
        .unwrap();
        let inventory = MigrationProject::new()
            .source(MigrationProjectSource::new(
                MigrationSourceKind::Sass,
                sass.to_string_lossy().into_owned(),
            ))
            .collect()
            .unwrap();
        assert_eq!(inventory.dependencies().len(), 5);
        assert_eq!(
            inventory.dependencies()[0].resolution(),
            MigrationDependencyResolution::External
        );
        assert_eq!(
            inventory.dependencies()[1].resolution(),
            MigrationDependencyResolution::External
        );
        assert_eq!(
            inventory.dependencies()[2].resolution(),
            MigrationDependencyResolution::Unresolved
        );
        assert_eq!(
            inventory.dependencies()[3].specifier(),
            Some("first-package")
        );
        assert_eq!(
            inventory.dependencies()[4].specifier(),
            Some("second-package")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn project_inventory_exposes_tailwind_config_plugin_and_template_seams() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = PathBuf::from(format!(
            ".migration-tailwind-seams-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let css = directory.join("app.css");
        let config = directory.join("tailwind.config.js");
        let plugin = directory.join("plugin.ts");
        let template = directory.join("index.html");
        fs::write(
            &css,
            "@config \"./tailwind.config.js\";\n@plugin \"./plugin.ts\";\n@plugin \"@acme/plugin\";\n@source \"./index.html\";\n@source inline(\"grid flex\");\n",
        )
        .unwrap();
        fs::write(&config, "export default { theme: {} };\n").unwrap();
        fs::write(&plugin, "export default function plugin() {}\n").unwrap();
        fs::write(
            &template,
            "<main title=\"class='ignored'\" class=\"grid gap-4\" className={active ? 'x' : 'y'}></main>\n<!-- <div class=\"ignored\"> -->\n",
        )
        .unwrap();
        let inventory = MigrationProject::new()
            .source(MigrationProjectSource::new(
                MigrationSourceKind::Tailwind,
                css.to_string_lossy().into_owned(),
            ))
            .auxiliary(MigrationProjectAuxiliary::new(
                MigrationAuxiliaryKind::TailwindConfig,
                config.to_string_lossy().into_owned(),
            ))
            .auxiliary(MigrationProjectAuxiliary::new(
                MigrationAuxiliaryKind::TailwindPlugin,
                plugin.to_string_lossy().into_owned(),
            ))
            .auxiliary(MigrationProjectAuxiliary::new(
                MigrationAuxiliaryKind::TailwindTemplate,
                template.to_string_lossy().into_owned(),
            ))
            .collect()
            .unwrap();
        let resolutions = inventory
            .dependencies()
            .iter()
            .map(MigrationDependency::resolution)
            .collect::<Vec<_>>();
        assert_eq!(
            resolutions,
            [
                MigrationDependencyResolution::Resolved,
                MigrationDependencyResolution::Resolved,
                MigrationDependencyResolution::External,
                MigrationDependencyResolution::Resolved,
                MigrationDependencyResolution::Dynamic,
            ]
        );
        assert_eq!(
            inventory.dependencies()[0].kind(),
            MigrationDependencyKind::TailwindConfig
        );
        assert_eq!(
            inventory.dependencies()[2].kind(),
            MigrationDependencyKind::TailwindPlugin
        );
        assert_eq!(
            inventory.dependencies()[3].kind(),
            MigrationDependencyKind::TailwindSource
        );
        assert_eq!(inventory.auxiliaries().len(), 3);
        assert!(
            inventory
                .auxiliaries()
                .iter()
                .all(|item| { item.source_bytes() > 0 && item.source_sha256().len() == 64 })
        );
        let template_inventory = inventory
            .auxiliaries()
            .iter()
            .find(|item| item.auxiliary_kind() == MigrationAuxiliaryKind::TailwindTemplate)
            .unwrap();
        assert_eq!(template_inventory.observations().len(), 3);
        assert_eq!(template_inventory.observations()[0].value(), Some("grid"));
        assert_eq!(template_inventory.observations()[1].value(), Some("gap-4"));
        assert_eq!(template_inventory.observations()[2].value(), None);
        let document: serde_json::Value = serde_json::from_slice(inventory.as_bytes()).unwrap();
        assert_eq!(document["summary"]["auxiliaries"], 3);
        assert_eq!(document["summary"]["tailwindConfigs"], 1);
        assert_eq!(document["summary"]["tailwindPlugins"], 1);
        assert_eq!(document["summary"]["tailwindTemplates"], 1);
        assert_eq!(document["summary"]["staticTemplateCandidates"], 2);
        assert_eq!(document["summary"]["dynamicTemplateCandidates"], 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn inventories_closed_tailwind_config_and_plugin_seams() {
        let config = inventory_migration_auxiliary_source(
            MigrationAuxiliaryKind::TailwindConfig,
            "tailwind.config.js",
            "export default { theme: {}, content: [] }; const ignored = 'safelist:';",
        )
        .unwrap();
        assert_eq!(config.observations().len(), 2);
        assert_eq!(config.observations()[0].value(), Some("theme"));
        assert_eq!(config.observations()[1].value(), Some("content"));
        let plugin = inventory_migration_auxiliary_source(
            MigrationAuxiliaryKind::TailwindPlugin,
            "plugin.ts",
            "function plugin({ addUtilities }) { addUtilities({}); } // matchVariant()\nconst ignored = 'addVariant(';",
        )
        .unwrap();
        assert_eq!(plugin.observations().len(), 1);
        assert_eq!(plugin.observations()[0].value(), Some("addUtilities"));
        assert_eq!(
            plugin.observations()[0].disposition(),
            MigrationDisposition::Unsupported
        );
    }

    #[test]
    fn tailwind_auxiliary_resolution_rejects_a_declared_wrong_kind() {
        let sources = std::collections::BTreeMap::new();
        let auxiliaries = std::collections::BTreeMap::from([(
            "src/tailwind.config.js",
            MigrationAuxiliaryKind::TailwindPlugin,
        )]);
        let error = classify_dependency(
            "src/app.css",
            MigrationSourceKind::Tailwind,
            MigrationDependencyKind::TailwindConfig,
            "./tailwind.config.js",
            &sources,
            &auxiliaries,
        )
        .unwrap_err();
        assert!(error.to_string().contains("requires tailwind-config"));
    }

    #[test]
    fn inventories_css_modules_consumer_imports_static_and_dynamic_usages() {
        let source = r#"import styles from "./card.module.css";
import { cardClass } from "./named.module.css";
const lazy = import("./lazy.module.css");
const card = styles.card;
const title = styles["title-name"];
const selected = styles[key];
consume(styles);
const template = `${styles.card}`;
const text = "styles.ignored";
// styles.comment
"#;
        let inventory = inventory_migration_consumer_source(
            MigrationConsumerKind::CssModules,
            "src/Card.tsx",
            source,
        )
        .unwrap();
        assert_eq!(inventory.observations().len(), 8);
        assert_eq!(
            inventory.observations()[0].kind(),
            MigrationConsumerObservationKind::Import
        );
        assert_eq!(inventory.observations()[0].binding(), Some("styles"));
        assert_eq!(
            inventory.observations()[0].target(),
            Some("src/card.module.css")
        );
        let static_classes = inventory
            .observations()
            .iter()
            .filter_map(MigrationConsumerObservation::class_name)
            .collect::<Vec<_>>();
        assert_eq!(static_classes, ["card", "title-name"]);
        assert_eq!(
            inventory
                .observations()
                .iter()
                .filter(|observation| {
                    observation.kind() == MigrationConsumerObservationKind::ClassUsage
                        && observation.disposition() == MigrationDisposition::Dynamic
                })
                .count(),
            3
        );
        assert_eq!(
            inventory
                .observations()
                .iter()
                .filter(|observation| {
                    observation.kind() == MigrationConsumerObservationKind::Import
                        && observation.disposition() == MigrationDisposition::Dynamic
                })
                .count(),
            2
        );
        assert!(inventory.observations().iter().all(|observation| {
            observation.byte_start() < observation.byte_end()
                && source
                    .get(observation.byte_start()..observation.byte_end())
                    .is_some()
        }));
    }

    #[test]
    fn project_inventory_links_declared_css_modules_consumers_fail_closed() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = PathBuf::from(format!(
            ".migration-consumer-project-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let module = directory.join("card.module.css");
        let consumer = directory.join("Card.tsx");
        fs::write(&module, ".card { display: block; }\n").unwrap();
        fs::write(
            &consumer,
            "import styles from \"./card.module.css\";\nexport const card = styles.card;\n",
        )
        .unwrap();
        let project = MigrationProject::new()
            .source(MigrationProjectSource::new(
                MigrationSourceKind::CssModules,
                module.to_string_lossy().into_owned(),
            ))
            .consumer(MigrationProjectConsumer::new(
                MigrationConsumerKind::CssModules,
                consumer.to_string_lossy().into_owned(),
            ));
        let inventory = project.clone().collect().unwrap();
        assert_eq!(inventory.consumers().len(), 1);
        assert_eq!(inventory.consumers()[0].observations().len(), 2);
        let document: serde_json::Value = serde_json::from_slice(inventory.as_bytes()).unwrap();
        assert_eq!(document["summary"]["consumers"], 1);
        assert_eq!(document["summary"]["consumerImports"], 1);
        assert_eq!(document["summary"]["staticConsumerUsages"], 1);
        assert_eq!(document["summary"]["dynamicConsumerUsages"], 0);

        fs::write(
            &consumer,
            "import styles from \"./missing.module.css\";\nexport const card = styles.card;\n",
        )
        .unwrap();
        let missing = project.collect().unwrap_err();
        assert!(missing.to_string().contains("undeclared local source"));
        fs::remove_dir_all(directory).unwrap();
    }
}
