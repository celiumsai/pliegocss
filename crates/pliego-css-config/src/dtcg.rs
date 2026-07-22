//! DTCG 2025.10 exchange bridge.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Read};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;
#[cfg(windows)]
use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pliego_css_ir::{BreakpointId, TokenKind};
use pliego_css_theme::{
    BreakpointDefinition, ThemeError, ThemeRegistry, TokenDefinition, token_id,
};
use serde_json::{Map, Number, Value, json};
use sha2::{Digest, Sha256};

use super::token_graph::{
    TokenExpression, TokenGraph, TokenGraphAdapter, TokenGraphError, TokenGraphProjection,
    TokenGraphReference, TokenGraphSource, TokenGraphTheme, TokenGraphToken, TokenReferenceSyntax,
    graph_tokens, token_kind_name,
};
use super::{normalize_name, normalize_value};

/// Stable DTCG format and resolver version implemented by this bridge.
pub const DTCG_FORMAT_VERSION: &str = "2025.10";
/// Reverse-domain key used for lossless `PliegoCSS` exchange metadata.
pub const DTCG_EXTENSION_KEY: &str = "io.celiums.pliego-css";

const PROFILE_VERSION: u64 = 1;
pub(crate) const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_DEPTH: usize = 64;
const MAX_TOKEN_COUNT: usize = 65_536;
const MAX_ALIAS_DEPTH: usize = 256;
const MAX_RESOLUTION_WORK: usize = 100_000;
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

/// Summary of the standard and Pliego-specific portions of one DTCG bridge operation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DtcgReport {
    imported_tokens: usize,
    aliases: usize,
    derived_values: usize,
    deprecated_tokens: Vec<String>,
    preserved_tokens: Vec<String>,
    unmapped_tokens: Vec<String>,
}

impl DtcgReport {
    /// Number of standard DTCG tokens imported into the typed registry.
    #[must_use]
    pub const fn imported_tokens(&self) -> usize {
        self.imported_tokens
    }

    /// Number of imported tokens whose value contains a DTCG reference.
    #[must_use]
    pub const fn aliases(&self) -> usize {
        self.aliases
    }

    /// Number of imported composite/property values that depend on references.
    #[must_use]
    pub const fn derived_values(&self) -> usize {
        self.derived_values
    }

    /// Paths of imported tokens marked deprecated by a token or parent group.
    #[must_use]
    pub fn deprecated_tokens(&self) -> &[String] {
        &self.deprecated_tokens
    }

    /// DTCG token paths preserved in the document but outside `PliegoCSS` namespaces.
    #[must_use]
    pub fn preserved_tokens(&self) -> &[String] {
        &self.preserved_tokens
    }

    /// `PliegoCSS` definitions carried losslessly in the vendor extension.
    #[must_use]
    pub fn unmapped_tokens(&self) -> &[String] {
        &self.unmapped_tokens
    }

    /// Returns true when every `PliegoCSS` definition has a standard DTCG projection.
    #[must_use]
    pub fn is_fully_interoperable(&self) -> bool {
        self.unmapped_tokens.is_empty()
    }
}

/// A preserved DTCG document and the exact typed registry derived from it.
#[derive(Clone, Debug)]
pub struct DtcgTheme {
    registry: ThemeRegistry,
    graph: Arc<TokenGraph>,
    selections: BTreeMap<String, String>,
    document: Value,
    report: DtcgReport,
}

impl DtcgTheme {
    /// Returns the validated `PliegoCSS` registry.
    #[must_use]
    pub const fn registry(&self) -> &ThemeRegistry {
        &self.registry
    }

    /// Consumes the bridge result and returns its registry.
    #[must_use]
    pub fn into_registry(self) -> ThemeRegistry {
        self.registry
    }

    /// Returns the canonical internal graph retained before registry flattening.
    #[must_use]
    pub fn graph(&self) -> &TokenGraph {
        &self.graph
    }

    /// Returns the canonical resolver selections that produced this theme.
    ///
    /// Direct DTCG format documents have no modifier selections and return an empty map.
    #[must_use]
    pub const fn selections(&self) -> &BTreeMap<String, String> {
        &self.selections
    }

    pub(crate) fn with_resolution(
        mut self,
        graph: Arc<TokenGraph>,
        selections: BTreeMap<String, String>,
    ) -> Self {
        self.graph = graph;
        self.selections = selections;
        self
    }

    /// Returns the original semantic JSON document, including unknown extension metadata.
    #[must_use]
    pub const fn document(&self) -> &Value {
        &self.document
    }

    /// Returns the import/export inventory.
    #[must_use]
    pub const fn report(&self) -> &DtcgReport {
        &self.report
    }

    /// Serializes the preserved document deterministically with one trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`DtcgError::Serialize`] if JSON serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, DtcgError> {
        let mut output =
            serde_json::to_string_pretty(&self.document).map_err(DtcgError::Serialize)?;
        output.push('\n');
        Ok(output)
    }
}

/// Errors returned by the DTCG 2025.10 bridge.
#[non_exhaustive]
#[derive(Debug)]
pub enum DtcgError {
    /// A DTCG file could not be read.
    Read {
        /// Path that failed to load.
        path: PathBuf,
        /// Underlying I/O error.
        source: io::Error,
    },
    /// The input is not valid JSON.
    Parse(serde_json::Error),
    /// The preserved document could not be serialized.
    Serialize(serde_json::Error),
    /// A defensive document, nesting, or token-count limit was exceeded.
    Limit(&'static str),
    /// A DTCG structure, reference, type, or `PliegoCSS` projection is invalid.
    Invalid {
        /// Dotted token/group path, or `$` for the document root.
        path: String,
        /// Stable human-readable reason.
        reason: String,
    },
    /// Projected definitions violate the canonical registry contract.
    Registry(ThemeError),
    /// The retained internal token graph violates its canonical contract.
    Graph(TokenGraphError),
}

impl fmt::Display for DtcgError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    formatter,
                    "failed to read DTCG file `{}`: {source}",
                    path.display()
                )
            }
            Self::Parse(source) => write!(formatter, "invalid DTCG JSON: {source}"),
            Self::Serialize(source) => write!(formatter, "cannot serialize DTCG JSON: {source}"),
            Self::Limit(reason) => write!(formatter, "DTCG document limit exceeded: {reason}"),
            Self::Invalid { path, reason } => {
                write!(formatter, "invalid DTCG entry `{path}`: {reason}")
            }
            Self::Registry(source) => write!(formatter, "invalid DTCG theme registry: {source}"),
            Self::Graph(source) => write!(formatter, "invalid DTCG token graph: {source}"),
        }
    }
}

impl std::error::Error for DtcgError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse(source) | Self::Serialize(source) => Some(source),
            Self::Registry(source) => Some(source),
            Self::Graph(source) => Some(source),
            Self::Limit(_) | Self::Invalid { .. } => None,
        }
    }
}

/// Parses a DTCG 2025.10 format document and validates every value projected into `PliegoCSS`.
///
/// Standard tokens outside a recognized `PliegoCSS` namespace remain preserved in [`DtcgTheme`]
/// and are listed in [`DtcgReport::preserved_tokens`]. The projected registry always extends the
/// seed registry, matching theme-schema 1.
///
/// # Errors
///
/// Returns [`DtcgError`] for malformed JSON, invalid DTCG references or values, unsafe namespace
/// projections, and invalid resulting registry definitions.
pub fn parse_dtcg_str(source: &str) -> Result<DtcgTheme, DtcgError> {
    if source.len() > MAX_DOCUMENT_BYTES {
        return Err(DtcgError::Limit("document exceeds 16 MiB"));
    }
    let document = serde_json::from_str(source).map_err(DtcgError::Parse)?;
    build_theme(document)
}

/// Reads and parses a DTCG 2025.10 format document.
///
/// # Errors
///
/// The path must name a regular, non-link UTF-8 file no larger than 16 MiB. Returns
/// [`DtcgError`] when the file cannot be read or does not satisfy the bridge contract.
pub fn parse_dtcg_path(path: impl AsRef<Path>) -> Result<DtcgTheme, DtcgError> {
    let path = path.as_ref();
    let source = read_bounded_utf8_document(path, "document exceeds 16 MiB")?;
    parse_dtcg_str(&source)
}

pub(crate) fn read_bounded_utf8_document(
    path: &Path,
    limit_reason: &'static str,
) -> Result<String, DtcgError> {
    let read_error = |source| DtcgError::Read {
        path: path.to_path_buf(),
        source,
    };
    let path_metadata = fs::symlink_metadata(path).map_err(read_error)?;
    if is_link_like(&path_metadata) || !path_metadata.is_file() {
        return Err(read_error(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DTCG input must be a regular file, not a symbolic link or reparse point",
        )));
    }
    if path_metadata.len() > MAX_DOCUMENT_BYTES as u64 {
        return Err(DtcgError::Limit(limit_reason));
    }

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    #[cfg(windows)]
    options.custom_flags(0x0020_0000);
    let file = options.open(path).map_err(read_error)?;
    let opened_metadata = file.metadata().map_err(read_error)?;
    if is_link_like(&opened_metadata) || !opened_metadata.is_file() {
        return Err(read_error(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DTCG input changed before it could be opened as a regular file",
        )));
    }
    if opened_metadata.len() > MAX_DOCUMENT_BYTES as u64 {
        return Err(DtcgError::Limit(limit_reason));
    }

    let mut bytes = Vec::new();
    file.take((MAX_DOCUMENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(DtcgError::Limit(limit_reason));
    }
    String::from_utf8(bytes).map_err(|error| {
        read_error(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("DTCG input is not valid UTF-8: {}", error.utf8_error()),
        ))
    })
}

fn is_link_like(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || {
        #[cfg(windows)]
        {
            metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

/// Exports a registry through the lossless `PliegoCSS` profile of DTCG 2025.10.
///
/// Definitions representable by the stable DTCG types are emitted as standard tokens. Remaining
/// CSS-native values are carried under [`DTCG_EXTENSION_KEY`] and inventoried rather than projected
/// to a misleading standard value.
///
/// # Errors
///
/// Returns [`DtcgError`] only if the generated profile fails its own import contract.
pub fn export_dtcg(registry: &ThemeRegistry) -> Result<DtcgTheme, DtcgError> {
    let mut root = Map::new();
    let mut groups: BTreeMap<&'static str, Map<String, Value>> = BTreeMap::new();
    let mut unmapped = Vec::new();

    for token in registry.tokens() {
        let kind = kind_name(token.kind);
        if let Some((dtcg_type, value)) = project_token(token.kind, &token.value) {
            let group = groups.entry(kind).or_default();
            group.insert(
                token.name.clone(),
                projected_token(token.kind, &token.name, &token.value, dtcg_type, &value),
            );
        } else {
            unmapped.push(json!({
                "kind": kind,
                "name": token.name,
                "cssValue": token.value,
            }));
        }
    }

    for breakpoint in registry.breakpoints() {
        if let Some(value) = project_dimension(&breakpoint.min_width) {
            let group = groups.entry("breakpoints").or_default();
            group.insert(
                breakpoint.name.clone(),
                projected_entry(
                    "breakpoints",
                    &breakpoint.name,
                    &breakpoint.min_width,
                    "dimension",
                    &value,
                ),
            );
        } else {
            unmapped.push(json!({
                "kind": "breakpoints",
                "name": breakpoint.name,
                "cssValue": breakpoint.min_width,
            }));
        }
    }

    for (kind, mut group) in groups {
        group.insert(
            "$extensions".into(),
            json!({DTCG_EXTENSION_KEY: {"kind": kind}}),
        );
        root.insert(kind.into(), Value::Object(group));
    }

    root.insert(
        "$extensions".into(),
        json!({
            DTCG_EXTENSION_KEY: {
                "profile": PROFILE_VERSION,
                "format": DTCG_FORMAT_VERSION,
                "extends": "seed",
                "themeId": format!("{:032x}", registry.id().get()),
                "unmappedTokens": unmapped,
            }
        }),
    );
    build_theme(Value::Object(root))
}

#[derive(Clone, Debug)]
struct TokenRecord {
    path: Vec<String>,
    pointer: String,
    object: Map<String, Value>,
    inherited_type: Option<String>,
    kind_hint: Option<String>,
    deprecated: bool,
    alias: bool,
}

#[derive(Clone, Debug)]
struct ResolvedToken {
    value: Value,
    type_name: String,
}

#[derive(Clone, Debug)]
struct ResolvedValue {
    value: Value,
    token_type: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolveState {
    Unvisited,
    Visiting,
    Resolved,
}

struct Resolver<'a> {
    records: &'a [TokenRecord],
    document: &'a Value,
    paths: HashMap<Vec<String>, usize>,
    pointers: HashMap<String, usize>,
    states: Vec<ResolveState>,
    cache: Vec<Option<ResolvedToken>>,
    active: Vec<usize>,
    work: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ProjectionKind {
    Token(TokenKind),
    Breakpoint,
}

fn build_theme(document: Value) -> Result<DtcgTheme, DtcgError> {
    let root = document
        .as_object()
        .ok_or_else(|| invalid("$", "root must be a JSON object"))?;
    let profile = profile(root)?;
    let mut records = Vec::new();
    collect_records(root, &[], None, None, false, 0, &mut records)?;
    if records.len() > MAX_TOKEN_COUNT {
        return Err(DtcgError::Limit(
            "document contains more than 65,536 tokens",
        ));
    }

    let mut resolver = Resolver::new(&records, &document);
    for index in 0..records.len() {
        resolver.resolve_record(index)?;
    }
    let resolved_tokens = resolver.cache;

    let seed = ThemeRegistry::seed();
    let mut tokens = seed.tokens().to_vec();
    let mut breakpoints = seed.breakpoints().to_vec();
    let mut report = DtcgReport::default();
    let mut input_names = BTreeSet::new();
    let (mut graph_sources, dtcg_inventory) = project_records(
        &records,
        &resolved_tokens,
        profile,
        &mut tokens,
        &mut breakpoints,
        &mut report,
        &mut input_names,
    )?;

    import_unmapped(
        root,
        profile,
        &mut tokens,
        &mut breakpoints,
        &mut input_names,
        &mut report,
    )?;
    super::rank_breakpoints(&mut breakpoints)
        .map_err(|error| invalid(&error.name, error.reason))?;
    let registry =
        ThemeRegistry::from_definitions(tokens, breakpoints).map_err(DtcgError::Registry)?;
    verify_profile_theme_id(root, profile, &registry)?;
    graph_sources.sort_by(|left, right| left.path.cmp(&right.path));
    let graph_tokens = graph_tokens_with_sources(&registry, &graph_sources);
    let graph = TokenGraph {
        schema_version: 1,
        graph_version: super::TOKEN_GRAPH_VERSION.into(),
        adapter: Some(TokenGraphAdapter {
            name: "dtcg".into(),
            version: DTCG_FORMAT_VERSION.into(),
            source_hash: Some(semantic_document_hash(&document)?),
        }),
        sources: graph_sources,
        themes: vec![TokenGraphTheme {
            name: "default".into(),
            selections: BTreeMap::new(),
            theme_id: format!("{:032x}", registry.id().get()),
            dtcg_inventory: dtcg_inventory.clone(),
            tokens: graph_tokens,
        }],
        dtcg_inventory,
    };
    graph.to_canonical_json().map_err(DtcgError::Graph)?;
    Ok(DtcgTheme {
        registry,
        graph: Arc::new(graph),
        selections: BTreeMap::new(),
        document,
        report,
    })
}

pub(crate) fn semantic_document_hash(document: &Value) -> Result<String, DtcgError> {
    let bytes = serde_json::to_vec(document).map_err(DtcgError::Serialize)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

#[allow(clippy::too_many_arguments)]
fn project_records(
    records: &[TokenRecord],
    resolved: &[Option<ResolvedToken>],
    profile: bool,
    tokens: &mut Vec<TokenDefinition>,
    breakpoints: &mut Vec<BreakpointDefinition>,
    report: &mut DtcgReport,
    input_names: &mut BTreeSet<(ProjectionKind, String)>,
) -> Result<(Vec<TokenGraphSource>, BTreeMap<String, u64>), DtcgError> {
    let mut graph_sources = Vec::with_capacity(records.len());
    let mut dtcg_inventory = BTreeMap::new();
    for (record, resolved) in records.iter().zip(resolved) {
        let path = display_path(&record.path);
        let resolved = resolved.as_ref().expect("all records resolved");
        let kind = projection_kind(record)?;
        let projected_name = kind.map(|_| projected_name(record)).transpose()?;
        let (expression, references) = graph_expression(record, records)?;
        *dtcg_inventory
            .entry(resolved.type_name.clone())
            .or_insert(0_u64) += 1;
        graph_sources.push(graph_source(
            record,
            resolved,
            kind,
            projected_name.as_deref(),
            expression,
            references,
        ));
        let Some(kind) = kind else {
            report.preserved_tokens.push(path);
            continue;
        };
        let name = projected_name.expect("projected records have a name");
        let value = project_to_css(kind, &resolved.type_name, &resolved.value, &path)?;
        let value = profile_css_value(profile, record, &value)?;
        if !input_names.insert((kind, name.clone())) {
            return Err(invalid(
                &path,
                format!("duplicate canonical PliegoCSS name `{name}`"),
            ));
        }
        match kind {
            ProjectionKind::Token(kind) => merge_token(tokens, kind, name, value),
            ProjectionKind::Breakpoint => merge_breakpoint(breakpoints, name, value)?,
        }
        report.imported_tokens += 1;
        report.aliases += usize::from(record.alias);
        report.derived_values += usize::from(expression == TokenExpression::Derived);
        if record.deprecated {
            report.deprecated_tokens.push(path);
        }
    }
    Ok((graph_sources, dtcg_inventory))
}

fn graph_source(
    record: &TokenRecord,
    resolved: &ResolvedToken,
    kind: Option<ProjectionKind>,
    projected_name: Option<&str>,
    expression: TokenExpression,
    references: Vec<TokenGraphReference>,
) -> TokenGraphSource {
    TokenGraphSource {
        path: display_path(&record.path),
        pointer: record.pointer.clone(),
        token_type: resolved.type_name.clone(),
        expression,
        references,
        deprecated: record.deprecated,
        projection: match kind {
            Some(ProjectionKind::Token(kind)) => {
                let name = projected_name.expect("projected token records have a canonical name");
                Some(TokenGraphProjection {
                    kind: token_kind_name(kind).into(),
                    token_id: format!("{:08x}", token_id(name).get()),
                    name: name.into(),
                })
            }
            Some(ProjectionKind::Breakpoint) | None => None,
        },
        resolved_value: resolved.value.clone(),
    }
}

fn graph_tokens_with_sources(
    registry: &ThemeRegistry,
    sources: &[TokenGraphSource],
) -> Vec<TokenGraphToken> {
    let mut tokens = graph_tokens(registry);
    for token in &mut tokens {
        token.source = sources
            .iter()
            .find(|source| {
                source.projection.as_ref().is_some_and(|projection| {
                    projection.kind == token.kind
                        && projection.token_id == token.token_id
                        && projection.name == token.name
                })
            })
            .map(|source| source.path.clone());
    }
    tokens
}

#[allow(clippy::too_many_arguments)]
fn collect_records(
    object: &Map<String, Value>,
    path: &[String],
    inherited_type: Option<&str>,
    inherited_kind: Option<&str>,
    inherited_deprecated: bool,
    depth: usize,
    output: &mut Vec<TokenRecord>,
) -> Result<(), DtcgError> {
    if depth > MAX_DEPTH {
        return Err(DtcgError::Limit("group nesting exceeds 64 levels"));
    }
    if object.contains_key("$extends") {
        return Err(invalid(
            display_path(path),
            "group `$extends` is not part of the PliegoCSS 0.1 exchange profile",
        ));
    }
    validate_extensions(object, &display_path(path))?;
    let local_type = optional_string(object, "$type", &display_path(path))?;
    let effective_type = local_type.as_deref().or(inherited_type);
    let local_kind = extension_string(object, "kind")?;
    let effective_kind = local_kind.as_deref().or(inherited_kind);
    let deprecated = deprecated_value(object, inherited_deprecated, &display_path(path))?;

    for (name, value) in object {
        if name == "$root" {
            let child = value
                .as_object()
                .ok_or_else(|| invalid(display_path(path), "`$root` token must be an object"))?;
            if !child.contains_key("$value") && !child.contains_key("$ref") {
                return Err(invalid(
                    display_path(path),
                    "`$root` must contain a token value or reference",
                ));
            }
            let mut child_path = path.to_vec();
            child_path.push(name.clone());
            collect_token(
                child,
                child_path,
                effective_type,
                effective_kind,
                deprecated,
                output,
            )?;
            continue;
        }
        if name.starts_with('$') {
            continue;
        }
        validate_dtcg_name(name, path)?;
        let mut child_path = path.to_vec();
        child_path.push(name.clone());
        let child = value.as_object().ok_or_else(|| {
            invalid(
                display_path(&child_path),
                "token or group must be a JSON object",
            )
        })?;
        if child.contains_key("$value") || child.contains_key("$ref") {
            collect_token(
                child,
                child_path,
                effective_type,
                effective_kind,
                deprecated,
                output,
            )?;
        } else {
            collect_records(
                child,
                &child_path,
                effective_type,
                effective_kind,
                deprecated,
                depth + 1,
                output,
            )?;
        }
    }
    Ok(())
}

fn collect_token(
    object: &Map<String, Value>,
    path: Vec<String>,
    inherited_type: Option<&str>,
    inherited_kind: Option<&str>,
    inherited_deprecated: bool,
    output: &mut Vec<TokenRecord>,
) -> Result<(), DtcgError> {
    let shown = display_path(&path);
    validate_extensions(object, &shown)?;
    if object.contains_key("$value") && object.contains_key("$ref") {
        return Err(invalid(
            &shown,
            "token cannot contain both `$value` and `$ref`",
        ));
    }
    if object.keys().any(|key| !key.starts_with('$')) {
        return Err(invalid(
            &shown,
            "token cannot also contain child tokens or groups",
        ));
    }
    if let Some(reference) = object.get("$ref") {
        if !reference.is_string() {
            return Err(invalid(&shown, "token `$ref` must be a string"));
        }
    }
    let local_type = optional_string(object, "$type", &shown)?;
    let kind_hint = extension_string(object, "kind")?.or_else(|| inherited_kind.map(str::to_owned));
    let deprecated = deprecated_value(object, inherited_deprecated, &shown)?;
    let alias =
        object.get("$ref").is_some() || object.get("$value").is_some_and(contains_reference);
    output.push(TokenRecord {
        pointer: pointer_for_path(&path),
        path,
        object: object.clone(),
        inherited_type: local_type.or_else(|| inherited_type.map(str::to_owned)),
        kind_hint,
        deprecated,
        alias,
    });
    Ok(())
}

impl<'a> Resolver<'a> {
    fn new(records: &'a [TokenRecord], document: &'a Value) -> Self {
        let paths = records
            .iter()
            .enumerate()
            .map(|(index, record)| (record.path.clone(), index))
            .collect();
        let mut pointers = HashMap::with_capacity(records.len() * 2);
        for (index, record) in records.iter().enumerate() {
            pointers.insert(record.pointer.clone(), index);
            pointers.insert(format!("{}/$value", record.pointer), index);
        }
        Self {
            records,
            document,
            paths,
            pointers,
            states: vec![ResolveState::Unvisited; records.len()],
            cache: vec![None; records.len()],
            active: Vec::new(),
            work: 0,
        }
    }

    fn charge(&mut self) -> Result<(), DtcgError> {
        self.work += 1;
        if self.work > MAX_RESOLUTION_WORK {
            return Err(DtcgError::Limit(
                "alias resolution exceeds 100,000 work units",
            ));
        }
        Ok(())
    }

    fn resolve_record(&mut self, root: usize) -> Result<ResolvedToken, DtcgError> {
        let mut stack = vec![(root, false)];
        while let Some((index, expanded)) = stack.pop() {
            if self.states[index] == ResolveState::Resolved {
                continue;
            }
            if expanded {
                let record = &self.records[index];
                let source = record
                    .object
                    .get("$value")
                    .cloned()
                    .or_else(|| {
                        record
                            .object
                            .get("$ref")
                            .cloned()
                            .map(|value| json!({"$ref": value}))
                    })
                    .ok_or_else(|| invalid(display_path(&record.path), "token has no value"))?;
                let resolved = self.resolve_value(source)?;
                let type_name = match (&record.inherited_type, resolved.token_type) {
                    (Some(declared), Some(target)) if declared != &target => {
                        return Err(invalid(
                            display_path(&record.path),
                            format!(
                                "declared type `{declared}` does not match referenced type `{target}`"
                            ),
                        ));
                    }
                    (Some(declared), _) => declared.clone(),
                    (None, Some(target)) => target,
                    (None, None) => {
                        return Err(invalid(
                            display_path(&record.path),
                            "token type is missing and cannot be inherited from its reference",
                        ));
                    }
                };
                self.cache[index] = Some(ResolvedToken {
                    value: resolved.value,
                    type_name,
                });
                self.states[index] = ResolveState::Resolved;
                let popped = self.active.pop();
                debug_assert_eq!(popped, Some(index));
                continue;
            }

            if self.states[index] == ResolveState::Visiting {
                return Err(self.circular_reference(index));
            }
            if self.active.len() >= MAX_ALIAS_DEPTH {
                return Err(DtcgError::Limit("alias reference depth exceeds 256 tokens"));
            }
            self.states[index] = ResolveState::Visiting;
            self.active.push(index);
            stack.push((index, true));

            let source = self.records[index]
                .object
                .get("$value")
                .cloned()
                .or_else(|| {
                    self.records[index]
                        .object
                        .get("$ref")
                        .cloned()
                        .map(|value| json!({"$ref": value}))
                })
                .ok_or_else(|| {
                    invalid(
                        display_path(&self.records[index].path),
                        "token has no value",
                    )
                })?;
            let dependencies = self.dependencies(&source)?;
            for dependency in dependencies.into_iter().rev() {
                let state = self
                    .states
                    .get(dependency)
                    .copied()
                    .ok_or(DtcgError::Limit(
                        "resolved dependency index exceeds token graph",
                    ))?;
                match state {
                    ResolveState::Unvisited => stack.push((dependency, false)),
                    ResolveState::Visiting => return Err(self.circular_reference(dependency)),
                    ResolveState::Resolved => {}
                }
            }
        }
        Ok(self.cache[root]
            .as_ref()
            .expect("root record resolved")
            .clone())
    }

    fn circular_reference(&self, index: usize) -> DtcgError {
        let mut chain = self
            .active
            .iter()
            .map(|item| display_path(&self.records[*item].path))
            .collect::<Vec<_>>();
        chain.push(display_path(&self.records[index].path));
        invalid(
            display_path(&self.records[index].path),
            format!("circular reference: {}", chain.join(" -> ")),
        )
    }

    fn dependencies(&mut self, source: &Value) -> Result<Vec<usize>, DtcgError> {
        let mut dependencies = Vec::new();
        let mut values = vec![source];
        while let Some(value) = values.pop() {
            match value {
                Value::String(text) => {
                    if let Some(path) = curly_reference(text) {
                        self.charge()?;
                        dependencies.push(*self.paths.get(&path).ok_or_else(|| {
                            invalid(text, "curly reference does not target a token")
                        })?);
                    }
                }
                Value::Array(array) => values.extend(array.iter().rev()),
                Value::Object(object) if object.contains_key("$ref") => {
                    self.charge()?;
                    if object.len() != 1 {
                        return Err(invalid("$ref", "reference object must contain only `$ref`"));
                    }
                    let reference = object
                        .get("$ref")
                        .and_then(Value::as_str)
                        .ok_or_else(|| invalid("$ref", "reference must be a string"))?;
                    if let Some(index) = self.pointer_record(reference)? {
                        dependencies.push(index);
                    } else {
                        let fragment = pointer_fragment(reference)?;
                        let target = self.document.pointer(fragment).ok_or_else(|| {
                            invalid(reference, "JSON Pointer target does not exist")
                        })?;
                        values.push(target);
                    }
                }
                Value::Object(object) => values.extend(object.values().rev()),
                _ => {}
            }
        }
        Ok(dependencies)
    }

    fn pointer_record(&self, reference: &str) -> Result<Option<usize>, DtcgError> {
        let fragment = pointer_fragment(reference)?;
        if let Some(index) = self.pointers.get(fragment) {
            return Ok(Some(*index));
        }
        let mut prefix = fragment;
        while let Some((parent, _)) = prefix.rsplit_once('/') {
            if let Some(index) = self.pointers.get(parent) {
                return Ok(Some(*index));
            }
            prefix = parent;
        }
        Ok(None)
    }

    fn resolve_value(&mut self, value: Value) -> Result<ResolvedValue, DtcgError> {
        match value {
            Value::String(text) if curly_reference(&text).is_some() => {
                self.charge()?;
                let path = curly_reference(&text).expect("checked reference");
                let index = *self
                    .paths
                    .get(&path)
                    .ok_or_else(|| invalid(&text, "curly reference does not target a token"))?;
                let target = self.cache[index]
                    .as_ref()
                    .expect("dependencies resolved before values");
                Ok(ResolvedValue {
                    value: target.value.clone(),
                    token_type: Some(target.type_name.clone()),
                })
            }
            Value::Object(mut object) if object.contains_key("$ref") => {
                if object.len() != 1 {
                    return Err(invalid("$ref", "reference object must contain only `$ref`"));
                }
                let reference = object
                    .remove("$ref")
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .ok_or_else(|| invalid("$ref", "reference must be a string"))?;
                self.resolve_pointer(&reference)
            }
            Value::Array(values) => {
                let mut output = Vec::with_capacity(values.len());
                for value in values {
                    output.push(self.resolve_value(value)?.value);
                }
                Ok(ResolvedValue {
                    value: Value::Array(output),
                    token_type: None,
                })
            }
            Value::Object(object) => {
                let mut output = Map::new();
                for (key, value) in object {
                    output.insert(key, self.resolve_value(value)?.value);
                }
                Ok(ResolvedValue {
                    value: Value::Object(output),
                    token_type: None,
                })
            }
            value => Ok(ResolvedValue {
                value,
                token_type: None,
            }),
        }
    }

    fn resolve_pointer(&mut self, reference: &str) -> Result<ResolvedValue, DtcgError> {
        let fragment = pointer_fragment(reference)?;
        if let Some(index) = self.pointers.get(fragment).copied() {
            let target = self.cache[index]
                .as_ref()
                .expect("pointer dependency resolved before value");
            return Ok(ResolvedValue {
                value: target.value.clone(),
                token_type: Some(target.type_name.clone()),
            });
        }
        let mut prefix = fragment;
        while let Some((parent, _)) = prefix.rsplit_once('/') {
            if let Some(index) = self.pointers.get(parent).copied() {
                let target = self.cache[index]
                    .as_ref()
                    .expect("pointer dependency resolved before value");
                let remainder = fragment
                    .strip_prefix(parent)
                    .and_then(|value| value.strip_prefix('/'))
                    .expect("parent is a pointer prefix");
                let nested = target
                    .value
                    .pointer(&format!("/{remainder}"))
                    .cloned()
                    .ok_or_else(|| invalid(reference, "JSON Pointer target does not exist"))?;
                return self.resolve_value(nested);
            }
            prefix = parent;
        }
        let target = self
            .document
            .pointer(fragment)
            .cloned()
            .ok_or_else(|| invalid(reference, "JSON Pointer target does not exist"))?;
        self.resolve_value(target)
    }
}

fn pointer_fragment(reference: &str) -> Result<&str, DtcgError> {
    let fragment = reference.strip_prefix('#').ok_or_else(|| {
        invalid(
            reference,
            "external JSON Pointer references are not supported",
        )
    })?;
    if !fragment.is_empty() && !fragment.starts_with('/') {
        return Err(invalid(
            reference,
            "JSON Pointer fragment must begin with `#/`",
        ));
    }
    Ok(fragment)
}

fn projection_kind(record: &TokenRecord) -> Result<Option<ProjectionKind>, DtcgError> {
    if let Some(kind) = &record.kind_hint {
        return parse_projection_kind(kind).map(Some).ok_or_else(|| {
            invalid(
                display_path(&record.path),
                format!("unknown PliegoCSS kind `{kind}`"),
            )
        });
    }
    Ok(record
        .path
        .first()
        .and_then(|segment| parse_projection_kind(segment)))
}

fn parse_projection_kind(kind: &str) -> Option<ProjectionKind> {
    let token = match kind {
        "color" => TokenKind::Color,
        "spacing" => TokenKind::Spacing,
        "font-family" => TokenKind::FontFamily,
        "font-size" => TokenKind::FontSize,
        "font-weight" => TokenKind::FontWeight,
        "line-height" => TokenKind::LineHeight,
        "letter-spacing" => TokenKind::LetterSpacing,
        "radius" => TokenKind::Radius,
        "shadow" => TokenKind::Shadow,
        "z-index" => TokenKind::ZIndex,
        "breakpoints" => return Some(ProjectionKind::Breakpoint),
        _ => return None,
    };
    Some(ProjectionKind::Token(token))
}

fn projected_name(record: &TokenRecord) -> Result<String, DtcgError> {
    if let Some(name) = extension_string(&record.object, "name")? {
        return normalize_name("dtcg", &name)
            .map_err(|error| invalid(display_path(&record.path), error.to_string()));
    }
    let mut segments = record
        .path
        .iter()
        .skip(1)
        .map(String::as_str)
        .collect::<Vec<_>>();
    if segments.last() == Some(&"$root") {
        segments.pop();
    }
    if segments.is_empty() {
        return Err(invalid(
            display_path(&record.path),
            "namespace root token requires extension field `name`",
        ));
    }
    normalize_name("dtcg", &segments.join("-"))
        .map_err(|error| invalid(display_path(&record.path), error.to_string()))
}

fn project_to_css(
    kind: ProjectionKind,
    type_name: &str,
    value: &Value,
    path: &str,
) -> Result<String, DtcgError> {
    match kind {
        ProjectionKind::Token(TokenKind::Color) => {
            require_type(type_name, &["color"], path)?;
            color_to_css(value, path)
        }
        ProjectionKind::Token(
            TokenKind::Spacing | TokenKind::FontSize | TokenKind::LetterSpacing | TokenKind::Radius,
        )
        | ProjectionKind::Breakpoint => {
            require_type(type_name, &["dimension"], path)?;
            dimension_to_css(value, path)
        }
        ProjectionKind::Token(TokenKind::FontFamily) => {
            require_type(type_name, &["fontFamily"], path)?;
            font_family_to_css(value, path)
        }
        ProjectionKind::Token(TokenKind::FontWeight) => {
            require_type(type_name, &["fontWeight"], path)?;
            font_weight_to_css(value, path)
        }
        ProjectionKind::Token(TokenKind::LineHeight) => {
            require_type(type_name, &["number", "dimension"], path)?;
            if type_name == "dimension" {
                dimension_to_css(value, path)
            } else {
                number_to_css(value, path)
            }
        }
        ProjectionKind::Token(TokenKind::Shadow) => {
            require_type(type_name, &["shadow"], path)?;
            shadow_to_css(value, path)
        }
        ProjectionKind::Token(TokenKind::ZIndex) => {
            require_type(type_name, &["number"], path)?;
            integer_to_css(value, path)
        }
    }
}

fn require_type(type_name: &str, accepted: &[&str], path: &str) -> Result<(), DtcgError> {
    if accepted.contains(&type_name) {
        Ok(())
    } else {
        Err(invalid(
            path,
            format!("type `{type_name}` cannot project to this PliegoCSS namespace"),
        ))
    }
}

fn dimension_to_css(value: &Value, path: &str) -> Result<String, DtcgError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(path, "dimension value must be an object"))?;
    if object.len() != 2 {
        return Err(invalid(
            path,
            "dimension requires exactly `value` and `unit`",
        ));
    }
    let number = finite_number(object.get("value"), path)?;
    let unit = object
        .get("unit")
        .and_then(Value::as_str)
        .filter(|unit| matches!(*unit, "px" | "rem"))
        .ok_or_else(|| invalid(path, "dimension unit must be `px` or `rem`"))?;
    Ok(format_number(number) + unit)
}

fn number_to_css(value: &Value, path: &str) -> Result<String, DtcgError> {
    finite_number(Some(value), path).map(format_number)
}

fn integer_to_css(value: &Value, path: &str) -> Result<String, DtcgError> {
    let number = finite_number(Some(value), path)?;
    if number.fract() != 0.0 || number < f64::from(i32::MIN) || number > f64::from(i32::MAX) {
        return Err(invalid(path, "PliegoCSS z-index requires a 32-bit integer"));
    }
    Ok(format_number(number))
}

fn font_weight_to_css(value: &Value, path: &str) -> Result<String, DtcgError> {
    if let Some(number) = value.as_f64().filter(|number| number.is_finite()) {
        if number.fract() != 0.0 || !(1.0..=1000.0).contains(&number) {
            return Err(invalid(
                path,
                "fontWeight number must be an integer from 1 through 1000",
            ));
        }
        return Ok(format_number(number));
    }
    let alias = value
        .as_str()
        .ok_or_else(|| invalid(path, "fontWeight must be a number or standard string alias"))?;
    let weight = match alias {
        "thin" | "hairline" => 100,
        "extra-light" | "ultra-light" => 200,
        "light" => 300,
        "normal" | "regular" | "book" => 400,
        "medium" => 500,
        "semi-bold" | "demi-bold" => 600,
        "bold" => 700,
        "extra-bold" | "ultra-bold" => 800,
        "black" | "heavy" => 900,
        "extra-black" | "ultra-black" => 950,
        _ => return Err(invalid(path, format!("unknown fontWeight alias `{alias}`"))),
    };
    Ok(weight.to_string())
}

fn font_family_to_css(value: &Value, path: &str) -> Result<String, DtcgError> {
    if let Some(family) = value.as_str() {
        return Ok(quote_font_family(family));
    }
    let families = value
        .as_array()
        .ok_or_else(|| invalid(path, "fontFamily value must be a string or string array"))?;
    if families.is_empty() {
        return Err(invalid(path, "fontFamily array must not be empty"));
    }
    families
        .iter()
        .map(|family| {
            family
                .as_str()
                .map(quote_font_family)
                .ok_or_else(|| invalid(path, "fontFamily array entries must be strings"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|families| families.join(","))
}

fn color_to_css(value: &Value, path: &str) -> Result<String, DtcgError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(path, "color value must be an object"))?;
    let space = object
        .get("colorSpace")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(path, "colorSpace must be a string"))?;
    let components = object
        .get("components")
        .and_then(Value::as_array)
        .filter(|components| components.len() == 3)
        .ok_or_else(|| invalid(path, "color components must contain exactly three entries"))?;
    let parsed_components = components
        .iter()
        .map(|component| color_component(component, path))
        .collect::<Result<Vec<_>, _>>()?;
    validate_color_components(space, &parsed_components, path)?;
    let components = parsed_components
        .iter()
        .map(|component| component.map_or_else(|| "none".into(), format_number))
        .collect::<Vec<_>>();
    let alpha = object
        .get("alpha")
        .map(|value| finite_number(Some(value), path))
        .transpose()?
        .unwrap_or(1.0);
    if !(0.0..=1.0).contains(&alpha) {
        return Err(invalid(path, "color alpha must be between 0 and 1"));
    }
    if let Some(hex) = object.get("hex") {
        let valid = hex.as_str().is_some_and(|value| {
            value.len() == 7
                && value.starts_with('#')
                && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
        });
        if !valid {
            return Err(invalid(
                path,
                "color `hex` fallback must use six-digit CSS hex",
            ));
        }
    }
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "colorSpace" | "components" | "alpha" | "hex"))
    {
        return Err(invalid(path, "color value contains an unknown property"));
    }
    let suffix = if alpha >= 1.0 {
        String::new()
    } else {
        format!(" / {}", format_number(alpha))
    };
    let joined = components.join(" ");
    match space {
        "hsl" | "hwb" => Ok(format!(
            "{space}({} {} {}{suffix})",
            components[0],
            percentage_component(&components[1]),
            percentage_component(&components[2])
        )),
        "lab" | "lch" | "oklab" | "oklch" => Ok(format!("{space}({joined}{suffix})")),
        "srgb" | "srgb-linear" | "display-p3" | "a98-rgb" | "prophoto-rgb" | "rec2020"
        | "xyz-d65" => Ok(format!("color({space} {joined}{suffix})")),
        _ => Err(invalid(
            path,
            format!("unsupported DTCG color space `{space}`"),
        )),
    }
}

fn color_component(value: &Value, path: &str) -> Result<Option<f64>, DtcgError> {
    if value.as_str() == Some("none") {
        return Ok(None);
    }
    finite_number(Some(value), path).map(Some)
}

fn validate_color_components(
    space: &str,
    values: &[Option<f64>],
    path: &str,
) -> Result<(), DtcgError> {
    let in_range = |index: usize, minimum: f64, maximum: f64, exclusive_maximum: bool| {
        values[index].is_none_or(|value| {
            value >= minimum
                && if exclusive_maximum {
                    value < maximum
                } else {
                    value <= maximum
                }
        })
    };
    let valid = match space {
        "srgb" | "srgb-linear" | "display-p3" | "a98-rgb" | "prophoto-rgb" | "rec2020"
        | "xyz-d65" => (0..3).all(|index| in_range(index, 0.0, 1.0, false)),
        "hsl" | "hwb" => {
            in_range(0, 0.0, 360.0, true)
                && in_range(1, 0.0, 100.0, false)
                && in_range(2, 0.0, 100.0, false)
        }
        "lab" => in_range(0, 0.0, 100.0, false),
        "lch" => {
            in_range(0, 0.0, 100.0, false)
                && values[1].is_none_or(|value| value >= 0.0)
                && in_range(2, 0.0, 360.0, true)
        }
        "oklab" => in_range(0, 0.0, 1.0, false),
        "oklch" => {
            in_range(0, 0.0, 1.0, false)
                && values[1].is_none_or(|value| value >= 0.0)
                && in_range(2, 0.0, 360.0, true)
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid(
            path,
            format!("color components are outside the `{space}` ranges"),
        ))
    }
}

fn shadow_to_css(value: &Value, path: &str) -> Result<String, DtcgError> {
    let layers = value
        .as_array()
        .map_or_else(|| vec![value], |values| values.iter().collect());
    if layers.is_empty() {
        return Err(invalid(path, "shadow array must not be empty"));
    }
    layers
        .into_iter()
        .map(|layer| {
            let object = layer
                .as_object()
                .ok_or_else(|| invalid(path, "shadow layer must be an object"))?;
            let mut output = String::new();
            if let Some(inset) = object.get("inset") {
                let inset = inset
                    .as_bool()
                    .ok_or_else(|| invalid(path, "shadow `inset` must be a boolean"))?;
                if inset {
                    output.push_str("inset ");
                }
            }
            for field in ["offsetX", "offsetY", "blur", "spread"] {
                if !output.is_empty() && !output.ends_with(' ') {
                    output.push(' ');
                }
                output.push_str(&dimension_to_css(
                    object
                        .get(field)
                        .ok_or_else(|| invalid(path, format!("shadow misses `{field}`")))?,
                    path,
                )?);
            }
            output.push(' ');
            output.push_str(&color_to_css(
                object
                    .get("color")
                    .ok_or_else(|| invalid(path, "shadow misses `color`"))?,
                path,
            )?);
            Ok(output)
        })
        .collect::<Result<Vec<_>, DtcgError>>()
        .map(|layers| layers.join(","))
}

fn merge_token(tokens: &mut Vec<TokenDefinition>, kind: TokenKind, name: String, value: String) {
    if let Some(existing) = tokens
        .iter_mut()
        .find(|definition| definition.kind == kind && definition.name == name)
    {
        *existing = TokenDefinition::with_id(kind, existing.id, name, value);
    } else {
        tokens.push(TokenDefinition::new(kind, name, value));
    }
}

fn merge_breakpoint(
    breakpoints: &mut Vec<BreakpointDefinition>,
    name: String,
    value: String,
) -> Result<(), DtcgError> {
    if let Some(existing) = breakpoints
        .iter_mut()
        .find(|definition| definition.name == name)
    {
        *existing = BreakpointDefinition::new(existing.id, existing.cascade_rank, name, value);
        return Ok(());
    }
    let next = breakpoints
        .iter()
        .map(|definition| definition.id.get())
        .max()
        .unwrap_or_default()
        .checked_add(1)
        .ok_or_else(|| invalid("breakpoints", "breakpoint ID space is exhausted"))?;
    breakpoints.push(BreakpointDefinition::new(
        BreakpointId::new(next),
        0,
        name,
        value,
    ));
    Ok(())
}

fn profile(root: &Map<String, Value>) -> Result<bool, DtcgError> {
    let Some(extension) = extension(root) else {
        return Ok(false);
    };
    let Some(profile) = extension.get("profile") else {
        return Ok(false);
    };
    if profile.as_u64() != Some(PROFILE_VERSION) {
        return Err(invalid("$extensions", "unsupported PliegoCSS DTCG profile"));
    }
    if extension.get("format").and_then(Value::as_str) != Some(DTCG_FORMAT_VERSION) {
        return Err(invalid("$extensions", "profile format must be `2025.10`"));
    }
    if extension.get("extends").and_then(Value::as_str) != Some("seed") {
        return Err(invalid("$extensions", "profile must extend `seed`"));
    }
    Ok(true)
}

fn profile_css_value(
    profile: bool,
    record: &TokenRecord,
    projected: &str,
) -> Result<String, DtcgError> {
    if !profile {
        return Ok(projected.to_owned());
    }
    let Some(value) = extension(&record.object).and_then(|extension| extension.get("cssValue"))
    else {
        return Ok(projected.to_owned());
    };
    let value = value.as_str().ok_or_else(|| {
        invalid(
            display_path(&record.path),
            "extension `cssValue` must be a string",
        )
    })?;
    normalize_value("dtcg", &projected_name(record)?, value)
        .map_err(|error| invalid(display_path(&record.path), error.to_string()))
}

fn import_unmapped(
    root: &Map<String, Value>,
    profile: bool,
    tokens: &mut Vec<TokenDefinition>,
    breakpoints: &mut Vec<BreakpointDefinition>,
    input_names: &mut BTreeSet<(ProjectionKind, String)>,
    report: &mut DtcgReport,
) -> Result<(), DtcgError> {
    if !profile {
        return Ok(());
    }
    let Some(values) = extension(root)
        .and_then(|extension| extension.get("unmappedTokens"))
        .and_then(Value::as_array)
    else {
        return Err(invalid(
            "$extensions",
            "profile requires `unmappedTokens` array",
        ));
    };
    for value in values {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("$extensions.unmappedTokens", "entry must be an object"))?;
        let kind_name = object
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("$extensions.unmappedTokens", "entry requires string `kind`"))?;
        let kind = parse_projection_kind(kind_name).ok_or_else(|| {
            invalid(
                "$extensions.unmappedTokens",
                format!("unknown kind `{kind_name}`"),
            )
        })?;
        let source_name = object
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("$extensions.unmappedTokens", "entry requires string `name`"))?;
        let name = normalize_name("dtcg", source_name)
            .map_err(|error| invalid("$extensions.unmappedTokens", error.to_string()))?;
        let source_value = object
            .get("cssValue")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                invalid(
                    "$extensions.unmappedTokens",
                    "entry requires string `cssValue`",
                )
            })?;
        let css_value = normalize_value("dtcg", &name, source_value)
            .map_err(|error| invalid("$extensions.unmappedTokens", error.to_string()))?;
        if !input_names.insert((kind, name.clone())) {
            return Err(invalid(
                "$extensions.unmappedTokens",
                format!("duplicate canonical PliegoCSS name `{name}`"),
            ));
        }
        match kind {
            ProjectionKind::Token(kind) => merge_token(tokens, kind, name.clone(), css_value),
            ProjectionKind::Breakpoint => {
                merge_breakpoint(breakpoints, name.clone(), css_value)?;
            }
        }
        report.unmapped_tokens.push(format!("{kind_name}.{name}"));
    }
    Ok(())
}

fn verify_profile_theme_id(
    root: &Map<String, Value>,
    profile: bool,
    registry: &ThemeRegistry,
) -> Result<(), DtcgError> {
    if !profile {
        return Ok(());
    }
    let expected = extension(root)
        .and_then(|extension| extension.get("themeId"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("$extensions", "profile requires string `themeId`"))?;
    let actual = format!("{:032x}", registry.id().get());
    if expected != actual {
        return Err(invalid(
            "$extensions.themeId",
            format!("stored ThemeId `{expected}` does not match derived `{actual}`"),
        ));
    }
    Ok(())
}

fn project_token(kind: TokenKind, value: &str) -> Option<(&'static str, Value)> {
    match kind {
        TokenKind::Color => project_hex_color(value).map(|value| ("color", value)),
        TokenKind::Spacing | TokenKind::FontSize | TokenKind::LetterSpacing | TokenKind::Radius => {
            project_dimension(value).map(|value| ("dimension", value))
        }
        TokenKind::FontFamily => project_font_family(value).map(|value| ("fontFamily", value)),
        TokenKind::FontWeight => project_number(value).map(|value| ("fontWeight", value)),
        TokenKind::LineHeight => project_number(value)
            .map(|value| ("number", value))
            .or_else(|| project_dimension(value).map(|value| ("dimension", value))),
        TokenKind::Shadow => None,
        TokenKind::ZIndex => project_number(value).map(|value| ("number", value)),
    }
}

fn project_dimension(value: &str) -> Option<Value> {
    if value == "0" || value == "+0" || value == "-0" {
        return Some(json!({"value": 0, "unit": "px"}));
    }
    for unit in ["rem", "px"] {
        if let Some(number) = value.strip_suffix(unit).and_then(parse_finite) {
            return Some(json!({"value": number, "unit": unit}));
        }
    }
    None
}

fn project_number(value: &str) -> Option<Value> {
    parse_finite(value)
        .and_then(Number::from_f64)
        .map(Value::Number)
}

fn project_font_family(value: &str) -> Option<Value> {
    let families = value
        .split(',')
        .map(str::trim)
        .map(|family| family.trim_matches(['\'', '"']))
        .collect::<Vec<_>>();
    if families.is_empty() || families.iter().any(|family| family.is_empty()) {
        return None;
    }
    Some(if families.len() == 1 {
        Value::String(families[0].to_owned())
    } else {
        Value::Array(
            families
                .into_iter()
                .map(|family| Value::String(family.to_owned()))
                .collect(),
        )
    })
}

fn project_hex_color(value: &str) -> Option<Value> {
    let hex = value.strip_prefix('#')?;
    let expanded = match hex.len() {
        3 | 4 => hex
            .chars()
            .flat_map(|character| [character, character])
            .collect::<String>(),
        6 | 8 => hex.to_owned(),
        _ => return None,
    };
    let red = u8::from_str_radix(&expanded[0..2], 16).ok()?;
    let green = u8::from_str_radix(&expanded[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&expanded[4..6], 16).ok()?;
    let alpha = if expanded.len() == 8 {
        f64::from(u8::from_str_radix(&expanded[6..8], 16).ok()?) / 255.0
    } else {
        1.0
    };
    Some(json!({
        "colorSpace": "srgb",
        "components": [f64::from(red) / 255.0, f64::from(green) / 255.0, f64::from(blue) / 255.0],
        "alpha": alpha,
        "hex": format!("#{:02x}{:02x}{:02x}", red, green, blue),
    }))
}

fn projected_token(
    kind: TokenKind,
    name: &str,
    css_value: &str,
    dtcg_type: &str,
    value: &Value,
) -> Value {
    projected_entry(kind_name(kind), name, css_value, dtcg_type, value)
}

fn projected_entry(
    kind: &str,
    name: &str,
    css_value: &str,
    dtcg_type: &str,
    value: &Value,
) -> Value {
    json!({
        "$type": dtcg_type,
        "$value": value,
        "$extensions": {
            DTCG_EXTENSION_KEY: {
                "kind": kind,
                "name": name,
                "cssValue": css_value,
            }
        }
    })
}

const fn kind_name(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::Spacing => "spacing",
        TokenKind::Color => "color",
        TokenKind::FontFamily => "font-family",
        TokenKind::FontSize => "font-size",
        TokenKind::FontWeight => "font-weight",
        TokenKind::LineHeight => "line-height",
        TokenKind::LetterSpacing => "letter-spacing",
        TokenKind::Radius => "radius",
        TokenKind::Shadow => "shadow",
        TokenKind::ZIndex => "z-index",
    }
}

fn validate_dtcg_name(name: &str, parent: &[String]) -> Result<(), DtcgError> {
    if name.is_empty() || name.starts_with('$') || name.contains(['{', '}', '.']) {
        let mut path = parent.to_vec();
        path.push(name.to_owned());
        return Err(invalid(
            display_path(&path),
            "names must be non-empty, not start with `$`, and contain no `{`, `}`, or `.`",
        ));
    }
    Ok(())
}

fn deprecated_value(
    object: &Map<String, Value>,
    inherited: bool,
    path: &str,
) -> Result<bool, DtcgError> {
    match object.get("$deprecated") {
        None => Ok(inherited),
        Some(Value::Bool(value)) => Ok(*value),
        Some(Value::String(_)) => Ok(true),
        Some(_) => Err(invalid(path, "`$deprecated` must be a boolean or string")),
    }
}

fn optional_string(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<String>, DtcgError> {
    object
        .get(key)
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid(path, format!("`{key}` must be a string")))
        })
        .transpose()
}

fn extension(object: &Map<String, Value>) -> Option<&Map<String, Value>> {
    object
        .get("$extensions")?
        .as_object()?
        .get(DTCG_EXTENSION_KEY)?
        .as_object()
}

fn extension_string(object: &Map<String, Value>, key: &str) -> Result<Option<String>, DtcgError> {
    extension(object)
        .and_then(|value| value.get(key))
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid("$extensions", format!("`{key}` must be a string")))
        })
        .transpose()
}

fn validate_extensions(object: &Map<String, Value>, path: &str) -> Result<(), DtcgError> {
    let Some(extensions) = object.get("$extensions") else {
        return Ok(());
    };
    let extensions = extensions
        .as_object()
        .ok_or_else(|| invalid(path, "`$extensions` must be an object"))?;
    if let Some(pliego) = extensions.get(DTCG_EXTENSION_KEY) {
        if !pliego.is_object() {
            return Err(invalid(
                path,
                format!("`$extensions.{DTCG_EXTENSION_KEY}` must be an object"),
            ));
        }
    }
    Ok(())
}

fn graph_expression(
    record: &TokenRecord,
    records: &[TokenRecord],
) -> Result<(TokenExpression, Vec<TokenGraphReference>), DtcgError> {
    let source = record
        .object
        .get("$value")
        .cloned()
        .or_else(|| {
            record
                .object
                .get("$ref")
                .cloned()
                .map(|reference| json!({"$ref": reference}))
        })
        .expect("validated token record has one source value");
    let mut references = BTreeSet::new();
    collect_graph_references(&source, records, &mut references)?;
    let expression = if references.is_empty() {
        TokenExpression::Literal
    } else if is_complete_reference(&source) {
        TokenExpression::Alias
    } else {
        TokenExpression::Derived
    };
    Ok((expression, references.into_iter().collect()))
}

fn collect_graph_references(
    value: &Value,
    records: &[TokenRecord],
    output: &mut BTreeSet<TokenGraphReference>,
) -> Result<(), DtcgError> {
    match value {
        Value::String(reference) => {
            if let Some(path) = curly_reference(reference) {
                let target = records
                    .iter()
                    .find(|record| record.path == path)
                    .ok_or_else(|| invalid(reference, "curly reference does not target a token"))?;
                output.insert(TokenGraphReference {
                    syntax: TokenReferenceSyntax::Curly,
                    reference: reference.clone(),
                    target: Some(display_path(&target.path)),
                    property: None,
                });
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_graph_references(value, records, output)?;
            }
        }
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref") {
                let reference = reference
                    .as_str()
                    .ok_or_else(|| invalid("$ref", "reference must be a string"))?;
                output.insert(graph_pointer_edge(reference, records)?);
            } else {
                for value in object.values() {
                    collect_graph_references(value, records, output)?;
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn graph_pointer_edge(
    reference: &str,
    records: &[TokenRecord],
) -> Result<TokenGraphReference, DtcgError> {
    let fragment = reference.strip_prefix('#').ok_or_else(|| {
        invalid(
            reference,
            "external JSON Pointer references are not supported",
        )
    })?;
    for record in records {
        let value_pointer = format!("{}/$value", record.pointer);
        if fragment == record.pointer || fragment == value_pointer {
            return Ok(TokenGraphReference {
                syntax: TokenReferenceSyntax::JsonPointer,
                reference: reference.into(),
                target: Some(display_path(&record.path)),
                property: None,
            });
        }
        if let Some(property) = fragment.strip_prefix(&(value_pointer + "/")) {
            return Ok(TokenGraphReference {
                syntax: TokenReferenceSyntax::JsonPointer,
                reference: reference.into(),
                target: Some(display_path(&record.path)),
                property: Some(format!("/{property}")),
            });
        }
    }
    Ok(TokenGraphReference {
        syntax: TokenReferenceSyntax::JsonPointer,
        reference: reference.into(),
        target: None,
        property: None,
    })
}

fn is_complete_reference(value: &Value) -> bool {
    match value {
        Value::String(value) => curly_reference(value).is_some(),
        Value::Object(object) => object.len() == 1 && object.contains_key("$ref"),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) => false,
    }
}

fn curly_reference(value: &str) -> Option<Vec<String>> {
    let inner = value.strip_prefix('{')?.strip_suffix('}')?;
    if inner.is_empty() || inner.contains(['{', '}']) {
        return None;
    }
    Some(inner.split('.').map(str::to_owned).collect())
}

fn contains_reference(value: &Value) -> bool {
    match value {
        Value::String(value) => curly_reference(value).is_some(),
        Value::Array(values) => values.iter().any(contains_reference),
        Value::Object(object) => {
            object.contains_key("$ref") || object.values().any(contains_reference)
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn pointer_for_path(path: &[String]) -> String {
    let mut output = String::new();
    for segment in path {
        output.push('/');
        output.push_str(&segment.replace('~', "~0").replace('/', "~1"));
    }
    output
}

fn display_path(path: &[String]) -> String {
    if path.is_empty() {
        "$".into()
    } else {
        path.join(".")
    }
}

fn finite_number(value: Option<&Value>, path: &str) -> Result<f64, DtcgError> {
    value
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or_else(|| invalid(path, "value must be a finite JSON number"))
}

fn parse_finite(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    value.to_string()
}

fn percentage_component(value: &str) -> String {
    if value == "none" {
        value.to_owned()
    } else {
        format!("{value}%")
    }
}

fn quote_font_family(value: &str) -> String {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        value.to_owned()
    } else {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
}

fn invalid(path: impl Into<String>, reason: impl Into<String>) -> DtcgError {
    DtcgError::Invalid {
        path: path.into(),
        reason: reason.into(),
    }
}
