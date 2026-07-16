//! Canonical internal token-graph contract.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::io::{self, Write};

use pliego_css_ir::TokenKind;
use pliego_css_theme::{ThemeRegistry, token_id};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Stable identity of the first canonical internal graph contract.
pub const TOKEN_GRAPH_VERSION: &str = "pliegocss-token-graph/1";

const MAX_GRAPH_SOURCES: usize = 65_536;
const MAX_GRAPH_THEMES: usize = 256;
const MAX_GRAPH_TOKENS: usize = 65_536;
const MAX_GRAPH_REFERENCE_EDGES: usize = 262_144;
const MAX_GRAPH_TEXT_BYTES: usize = 48 * 1024 * 1024;
const MAX_GRAPH_VALUE_NODES: usize = 1_048_576;
const MAX_GRAPH_VALUE_DEPTH: usize = 64;
const MAX_CANONICAL_GRAPH_BYTES: usize = 64 * 1024 * 1024;

/// Adapter that supplied graph provenance.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TokenGraphAdapter {
    /// Stable adapter name.
    pub name: String,
    /// Exact input-format version.
    pub version: String,
    /// SHA-256 of the canonical semantic adapter input, when one exists.
    pub source_hash: Option<String>,
}

/// How one source token obtains its value before resolution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TokenExpression {
    /// A value with no token reference.
    Literal,
    /// A complete value delegated to one other token.
    Alias,
    /// A composite or property value containing one or more references.
    Derived,
}

/// Reference syntax retained by one source token.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TokenReferenceSyntax {
    /// DTCG curly-brace token reference.
    Curly,
    /// Same-document JSON Pointer reference.
    JsonPointer,
}

/// One normalized reference edge in the source graph.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TokenGraphReference {
    /// Reference syntax used by the source document.
    pub syntax: TokenReferenceSyntax,
    /// Exact normalized reference expression.
    pub reference: String,
    /// Referenced source-token path when the expression targets a token.
    pub target: Option<String>,
    /// Property path below the referenced token value, when present.
    pub property: Option<String>,
}

/// Projection of a source token into the active typed registry.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TokenGraphProjection {
    /// Canonical `PliegoCSS` token namespace.
    pub kind: String,
    /// Stable namespace-scoped token ID as eight lowercase hexadecimal digits.
    pub token_id: String,
    /// Canonical token name.
    pub name: String,
}

/// One declaration in an adapter source document.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TokenGraphSource {
    /// Stable document-local dotted path.
    pub path: String,
    /// Stable JSON Pointer for the token object.
    pub pointer: String,
    /// Effective declared token type.
    pub token_type: String,
    /// Literal, alias, or derived source expression.
    pub expression: TokenExpression,
    /// Sorted unique reference edges.
    pub references: Vec<TokenGraphReference>,
    /// Effective inherited deprecation state.
    pub deprecated: bool,
    /// Optional projection into the active `PliegoCSS` registry.
    pub projection: Option<TokenGraphProjection>,
    /// Fully resolved semantic DTCG value.
    pub resolved_value: Value,
}

/// One resolved typed token in a graph theme.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TokenGraphToken {
    /// Canonical `PliegoCSS` token namespace.
    pub kind: String,
    /// Stable namespace-scoped token ID as eight lowercase hexadecimal digits.
    pub token_id: String,
    /// Canonical token name.
    pub name: String,
    /// Winning source declaration, absent only when an adapter cannot represent its provenance.
    pub source: Option<String>,
    /// Validated CSS value selected for this graph theme.
    pub value: String,
}

/// One fully resolved theme or resolver-context permutation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TokenGraphTheme {
    /// Stable theme name. The non-resolver theme is `default`.
    pub name: String,
    /// Resolver modifier selections that produced this theme.
    pub selections: BTreeMap<String, String>,
    /// Exact resolved `ThemeRegistry` identity.
    pub theme_id: String,
    /// Counts keyed by exact DTCG token type for this resolution.
    pub dtcg_inventory: BTreeMap<String, u64>,
    /// Canonical resolved token set.
    pub tokens: Vec<TokenGraphToken>,
}

/// Closed, deterministic internal graph document.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TokenGraph {
    /// Wire schema version.
    pub schema_version: u8,
    /// Stable graph-contract identity.
    pub graph_version: String,
    /// Optional adapter provenance.
    pub adapter: Option<TokenGraphAdapter>,
    /// Canonical source declarations and reference edges.
    pub sources: Vec<TokenGraphSource>,
    /// Canonical resolved themes/permutations.
    pub themes: Vec<TokenGraphTheme>,
    /// Counts keyed by exact DTCG token type.
    pub dtcg_inventory: BTreeMap<String, u64>,
}

/// Failure to build, encode, or parse a canonical token graph.
#[derive(Debug)]
pub enum TokenGraphError {
    /// JSON serialization or decoding failed.
    Json(serde_json::Error),
    /// The graph violates a closed semantic invariant.
    Invalid(String),
    /// Input bytes describe valid JSON but are not canonical graph bytes.
    NonCanonical,
}

impl fmt::Display for TokenGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(source) => write!(formatter, "invalid token-graph JSON: {source}"),
            Self::Invalid(reason) => write!(formatter, "invalid token graph: {reason}"),
            Self::NonCanonical => formatter.write_str("token-graph bytes are not canonical"),
        }
    }
}

impl std::error::Error for TokenGraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(source) => Some(source),
            Self::Invalid(_) | Self::NonCanonical => None,
        }
    }
}

impl From<serde_json::Error> for TokenGraphError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Default)]
struct BoundedJsonWriter {
    bytes: Vec<u8>,
    limit_exceeded: bool,
}

impl BoundedJsonWriter {
    fn push_newline(&mut self) -> Result<(), TokenGraphError> {
        if self.bytes.len() == MAX_CANONICAL_GRAPH_BYTES {
            return Err(TokenGraphError::Invalid(
                "canonical graph size exceeds 64 MiB".into(),
            ));
        }
        self.bytes.push(b'\n');
        Ok(())
    }
}

impl Write for BoundedJsonWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let Some(next_length) = self.bytes.len().checked_add(buffer.len()) else {
            self.limit_exceeded = true;
            return Err(io::Error::other("canonical graph size overflow"));
        };
        if next_length > MAX_CANONICAL_GRAPH_BYTES {
            self.limit_exceeded = true;
            return Err(io::Error::other("canonical graph size exceeds 64 MiB"));
        }
        self.bytes
            .try_reserve(buffer.len())
            .map_err(|_| io::Error::other("unable to reserve canonical graph storage"))?;
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl TokenGraph {
    /// Builds the canonical one-theme graph for an already resolved flat registry.
    #[must_use]
    pub fn from_registry(registry: &ThemeRegistry) -> Self {
        let tokens = graph_tokens(registry);
        Self {
            schema_version: 1,
            graph_version: TOKEN_GRAPH_VERSION.to_owned(),
            adapter: None,
            sources: tokens
                .iter()
                .map(|token| TokenGraphSource {
                    path: format!("registry.{}.{}", token.kind, token.name),
                    pointer: format!("/registry/{}/{}", token.kind, token.name),
                    token_type: token.kind.clone(),
                    expression: TokenExpression::Literal,
                    references: Vec::new(),
                    deprecated: false,
                    projection: Some(TokenGraphProjection {
                        kind: token.kind.clone(),
                        token_id: token.token_id.clone(),
                        name: token.name.clone(),
                    }),
                    resolved_value: Value::String(token.value.clone()),
                })
                .collect(),
            themes: vec![TokenGraphTheme {
                name: "default".into(),
                selections: BTreeMap::new(),
                theme_id: format!("{:032x}", registry.id().get()),
                dtcg_inventory: BTreeMap::new(),
                tokens,
            }],
            dtcg_inventory: BTreeMap::new(),
        }
    }

    /// Returns canonical compact JSON with one trailing newline.
    ///
    /// # Errors
    ///
    /// Returns an error when the graph is invalid or serialization fails.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, TokenGraphError> {
        validate_graph(self)?;
        let mut writer = BoundedJsonWriter::default();
        if let Err(error) = serde_json::to_writer(&mut writer, self) {
            return if writer.limit_exceeded {
                Err(TokenGraphError::Invalid(
                    "canonical graph size exceeds 64 MiB".into(),
                ))
            } else {
                Err(TokenGraphError::Json(error))
            };
        }
        writer.push_newline()?;
        Ok(writer.bytes)
    }

    /// Counts complete-value aliases retained by the graph.
    #[must_use]
    pub fn aliases(&self) -> u64 {
        u64::try_from(
            self.sources
                .iter()
                .filter(|source| {
                    source.projection.is_some() && source.expression == TokenExpression::Alias
                })
                .count(),
        )
        .unwrap_or(u64::MAX)
    }

    /// Counts composite/property values derived through references.
    #[must_use]
    pub fn derived_values(&self) -> u64 {
        u64::try_from(
            self.sources
                .iter()
                .filter(|source| {
                    source.projection.is_some() && source.expression == TokenExpression::Derived
                })
                .count(),
        )
        .unwrap_or(u64::MAX)
    }

    /// Counts effectively deprecated source declarations.
    #[must_use]
    pub fn deprecations(&self) -> u64 {
        u64::try_from(
            self.sources
                .iter()
                .filter(|source| source.projection.is_some() && source.deprecated)
                .count(),
        )
        .unwrap_or(u64::MAX)
    }

    /// Returns the selected resolved theme by exact modifier selections.
    #[must_use]
    pub fn theme(&self, selections: &BTreeMap<String, String>) -> Option<&TokenGraphTheme> {
        self.themes
            .iter()
            .find(|theme| &theme.selections == selections)
    }
}

/// Parses and validates exact canonical graph bytes.
///
/// # Errors
///
/// Rejects malformed JSON, unknown fields, invalid graph relationships, and noncanonical bytes.
pub fn parse_token_graph(bytes: &[u8]) -> Result<TokenGraph, TokenGraphError> {
    if bytes.len() > MAX_CANONICAL_GRAPH_BYTES {
        return Err(TokenGraphError::Invalid(
            "canonical graph size exceeds 64 MiB".into(),
        ));
    }
    let graph: TokenGraph = serde_json::from_slice(bytes)?;
    validate_graph(&graph)?;
    if graph.to_canonical_json()? != bytes {
        return Err(TokenGraphError::NonCanonical);
    }
    Ok(graph)
}

pub(crate) fn graph_tokens(registry: &ThemeRegistry) -> Vec<TokenGraphToken> {
    let mut tokens = registry
        .tokens()
        .iter()
        .map(|token| TokenGraphToken {
            kind: token_kind_name(token.kind).into(),
            token_id: format!("{:08x}", token.id.get()),
            name: token.name.clone(),
            source: Some(format!(
                "registry.{}.{}",
                token_kind_name(token.kind),
                token.name
            )),
            value: token.value.clone(),
        })
        .collect::<Vec<_>>();
    tokens.sort_by(|left, right| token_key(left).cmp(&token_key(right)));
    tokens
}

pub(crate) const fn token_kind_name(kind: TokenKind) -> &'static str {
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

fn parse_token_kind(value: &str) -> Option<TokenKind> {
    match value {
        "spacing" => Some(TokenKind::Spacing),
        "color" => Some(TokenKind::Color),
        "font-family" => Some(TokenKind::FontFamily),
        "font-size" => Some(TokenKind::FontSize),
        "font-weight" => Some(TokenKind::FontWeight),
        "line-height" => Some(TokenKind::LineHeight),
        "letter-spacing" => Some(TokenKind::LetterSpacing),
        "radius" => Some(TokenKind::Radius),
        "shadow" => Some(TokenKind::Shadow),
        "z-index" => Some(TokenKind::ZIndex),
        _ => None,
    }
}

fn validate_graph(graph: &TokenGraph) -> Result<(), TokenGraphError> {
    if graph.schema_version != 1 || graph.graph_version != TOKEN_GRAPH_VERSION {
        return Err(TokenGraphError::Invalid(format!(
            "expected schema 1 and graph version `{TOKEN_GRAPH_VERSION}`"
        )));
    }
    if let Some(adapter) = &graph.adapter {
        if adapter.name.is_empty() || adapter.version.is_empty() {
            return Err(TokenGraphError::Invalid(
                "adapter name and version must be non-empty".into(),
            ));
        }
        if adapter
            .source_hash
            .as_deref()
            .is_some_and(|hash| !is_sha256(hash))
        {
            return Err(TokenGraphError::Invalid(
                "adapter source hash must use canonical prefixed SHA-256".into(),
            ));
        }
        if matches!(adapter.name.as_str(), "dtcg" | "dtcg-resolver")
            && adapter.source_hash.is_none()
        {
            return Err(TokenGraphError::Invalid(
                "DTCG adapters require a canonical source hash".into(),
            ));
        }
    }
    if graph.themes.is_empty() || graph.themes.len() > MAX_GRAPH_THEMES {
        return Err(TokenGraphError::Invalid(
            "one through 256 resolved themes are required".into(),
        ));
    }
    if graph.sources.len() > MAX_GRAPH_SOURCES {
        return Err(TokenGraphError::Invalid(
            "source count exceeds 65,536".into(),
        ));
    }
    if graph
        .themes
        .iter()
        .any(|theme| theme.tokens.len() > MAX_GRAPH_TOKENS)
    {
        return Err(TokenGraphError::Invalid(
            "resolved token count exceeds 65,536 for one theme".into(),
        ));
    }
    validate_resource_limits(graph)?;
    let sources = graph
        .sources
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    validate_sources(graph, &sources)?;
    validate_themes(graph, &sources)?;
    if graph
        .dtcg_inventory
        .iter()
        .any(|(token_type, count)| token_type.is_empty() || *count == 0)
    {
        return Err(TokenGraphError::Invalid(
            "DTCG inventory keys and counts must be non-empty".into(),
        ));
    }
    let mut expected_inventory = BTreeMap::new();
    for theme in &graph.themes {
        for (token_type, count) in &theme.dtcg_inventory {
            expected_inventory
                .entry(token_type.clone())
                .and_modify(|maximum: &mut u64| *maximum = (*maximum).max(*count))
                .or_insert(*count);
        }
    }
    if graph.dtcg_inventory != expected_inventory {
        return Err(TokenGraphError::Invalid(
            "graph DTCG inventory must be the per-type maximum across themes".into(),
        ));
    }
    Ok(())
}

#[derive(Default)]
struct GraphResourceBudget {
    text_bytes: usize,
    reference_edges: usize,
    value_nodes: usize,
}

impl GraphResourceBudget {
    fn add_text(&mut self, value: &str) -> Result<(), TokenGraphError> {
        add_bounded(
            &mut self.text_bytes,
            value.len(),
            MAX_GRAPH_TEXT_BYTES,
            "graph text exceeds 48 MiB",
        )
    }

    fn add_references(&mut self, count: usize) -> Result<(), TokenGraphError> {
        add_bounded(
            &mut self.reference_edges,
            count,
            MAX_GRAPH_REFERENCE_EDGES,
            "reference edge count exceeds 262,144",
        )
    }

    fn add_value(&mut self, root: &Value) -> Result<(), TokenGraphError> {
        let mut pending = vec![(root, 0_usize)];
        while let Some((value, depth)) = pending.pop() {
            if depth > MAX_GRAPH_VALUE_DEPTH {
                return Err(TokenGraphError::Invalid(
                    "resolved-value nesting exceeds 64 levels".into(),
                ));
            }
            add_bounded(
                &mut self.value_nodes,
                1,
                MAX_GRAPH_VALUE_NODES,
                "resolved-value node count exceeds 1,048,576",
            )?;
            match value {
                Value::String(value) => self.add_text(value)?,
                Value::Array(values) => {
                    self.ensure_pending_values(pending.len(), values.len())?;
                    pending.extend(values.iter().map(|value| (value, depth + 1)));
                }
                Value::Object(values) => {
                    self.ensure_pending_values(pending.len(), values.len())?;
                    for (key, value) in values {
                        self.add_text(key)?;
                        pending.push((value, depth + 1));
                    }
                }
                Value::Null | Value::Bool(_) | Value::Number(_) => {}
            }
        }
        Ok(())
    }

    fn ensure_pending_values(
        &self,
        pending: usize,
        additional: usize,
    ) -> Result<(), TokenGraphError> {
        self.value_nodes
            .checked_add(pending)
            .and_then(|nodes| nodes.checked_add(additional))
            .filter(|nodes| *nodes <= MAX_GRAPH_VALUE_NODES)
            .map(|_| ())
            .ok_or_else(|| {
                TokenGraphError::Invalid("resolved-value node count exceeds 1,048,576".into())
            })
    }

    fn add_source(&mut self, source: &TokenGraphSource) -> Result<(), TokenGraphError> {
        self.add_text(&source.path)?;
        self.add_text(&source.pointer)?;
        self.add_text(&source.token_type)?;
        self.add_references(source.references.len())?;
        for reference in &source.references {
            self.add_text(&reference.reference)?;
            if let Some(target) = &reference.target {
                self.add_text(target)?;
            }
            if let Some(property) = &reference.property {
                self.add_text(property)?;
            }
        }
        if let Some(projection) = &source.projection {
            self.add_text(&projection.kind)?;
            self.add_text(&projection.token_id)?;
            self.add_text(&projection.name)?;
        }
        self.add_value(&source.resolved_value)
    }

    fn add_theme(&mut self, theme: &TokenGraphTheme) -> Result<(), TokenGraphError> {
        self.add_text(&theme.name)?;
        self.add_text(&theme.theme_id)?;
        for token_type in theme.dtcg_inventory.keys() {
            self.add_text(token_type)?;
        }
        for (name, value) in &theme.selections {
            self.add_text(name)?;
            self.add_text(value)?;
        }
        for token in &theme.tokens {
            self.add_text(&token.kind)?;
            self.add_text(&token.token_id)?;
            self.add_text(&token.name)?;
            if let Some(source) = &token.source {
                self.add_text(source)?;
            }
            self.add_text(&token.value)?;
        }
        Ok(())
    }
}

fn add_bounded(
    total: &mut usize,
    additional: usize,
    limit: usize,
    reason: &'static str,
) -> Result<(), TokenGraphError> {
    *total = total
        .checked_add(additional)
        .filter(|total| *total <= limit)
        .ok_or_else(|| TokenGraphError::Invalid(reason.into()))?;
    Ok(())
}

fn validate_resource_limits(graph: &TokenGraph) -> Result<(), TokenGraphError> {
    let mut budget = GraphResourceBudget::default();
    budget.add_text(&graph.graph_version)?;
    if let Some(adapter) = &graph.adapter {
        budget.add_text(&adapter.name)?;
        budget.add_text(&adapter.version)?;
        if let Some(source_hash) = &adapter.source_hash {
            budget.add_text(source_hash)?;
        }
    }
    for source in &graph.sources {
        budget.add_source(source)?;
    }
    for theme in &graph.themes {
        budget.add_theme(theme)?;
    }
    for token_type in graph.dtcg_inventory.keys() {
        budget.add_text(token_type)?;
    }
    Ok(())
}

fn validate_sources(
    graph: &TokenGraph,
    sources: &BTreeMap<&str, &TokenGraphSource>,
) -> Result<(), TokenGraphError> {
    if !graph
        .sources
        .windows(2)
        .all(|window| window[0].path < window[1].path)
    {
        return Err(TokenGraphError::Invalid(
            "source paths must be sorted and unique".into(),
        ));
    }
    let mut source_pointers = BTreeSet::new();
    for source in &graph.sources {
        if source.path.is_empty()
            || source.pointer.is_empty()
            || source.token_type.is_empty()
            || !source.pointer.starts_with('/')
            || !source_pointers.insert(source.pointer.as_str())
        {
            return Err(TokenGraphError::Invalid(format!(
                "source `{}` has an empty, duplicate, or noncanonical identity",
                source.path
            )));
        }
        if !source
            .references
            .windows(2)
            .all(|window| window[0] < window[1])
        {
            return Err(TokenGraphError::Invalid(format!(
                "source `{}` references must be sorted and unique",
                source.path
            )));
        }
        for reference in &source.references {
            if reference.reference.is_empty()
                || reference.property.as_deref().is_some_and(str::is_empty)
                || reference.target.is_none() && reference.property.is_some()
                || reference
                    .target
                    .as_deref()
                    .is_some_and(|target| !sources.contains_key(target))
            {
                return Err(TokenGraphError::Invalid(format!(
                    "source `{}` has an invalid reference edge",
                    source.path
                )));
            }
        }
        if source.expression == TokenExpression::Literal && !source.references.is_empty()
            || source.expression != TokenExpression::Literal && source.references.is_empty()
        {
            return Err(TokenGraphError::Invalid(format!(
                "source `{}` expression disagrees with its references",
                source.path
            )));
        }
        if let Some(projection) = &source.projection {
            validate_projection(projection)?;
        }
    }
    reject_reference_cycles(graph)?;
    Ok(())
}

fn validate_themes(
    graph: &TokenGraph,
    sources: &BTreeMap<&str, &TokenGraphSource>,
) -> Result<(), TokenGraphError> {
    if !graph
        .themes
        .windows(2)
        .all(|window| window[0].name < window[1].name)
    {
        return Err(TokenGraphError::Invalid(
            "theme names must be sorted and unique".into(),
        ));
    }
    let mut selections = BTreeSet::new();
    for theme in &graph.themes {
        if theme.name.is_empty()
            || !is_lower_hex(&theme.theme_id, 32)
            || theme.tokens.is_empty()
            || theme
                .selections
                .iter()
                .any(|(name, value)| name.is_empty() || value.is_empty())
            || !selections.insert(&theme.selections)
            || theme
                .dtcg_inventory
                .iter()
                .any(|(token_type, count)| token_type.is_empty() || *count == 0)
        {
            return Err(TokenGraphError::Invalid(format!(
                "theme `{}` has an invalid identity or duplicate selections",
                theme.name
            )));
        }
        if !theme
            .tokens
            .windows(2)
            .all(|window| token_key(&window[0]) < token_key(&window[1]))
        {
            return Err(TokenGraphError::Invalid(format!(
                "theme `{}` tokens must be sorted and unique",
                theme.name
            )));
        }
        for token in &theme.tokens {
            let kind = parse_token_kind(&token.kind).ok_or_else(|| {
                TokenGraphError::Invalid(format!("unknown token kind `{}`", token.kind))
            })?;
            if token.name.is_empty()
                || token.value.is_empty()
                || !is_lower_hex(&token.token_id, 8)
                || u32::from_str_radix(&token.token_id, 16).ok()
                    != Some(token_id(&token.name).get())
            {
                return Err(TokenGraphError::Invalid(format!(
                    "invalid {:?} token `{}`",
                    kind, token.name
                )));
            }
            if let Some(source) = token.source.as_deref() {
                let source = sources.get(source).ok_or_else(|| {
                    TokenGraphError::Invalid(format!(
                        "token `{}` references an absent winning source",
                        token.name
                    ))
                })?;
                let projection = source.projection.as_ref().ok_or_else(|| {
                    TokenGraphError::Invalid(format!(
                        "token `{}` winning source has no typed projection",
                        token.name
                    ))
                })?;
                if projection.kind != token.kind
                    || projection.token_id != token.token_id
                    || projection.name != token.name
                {
                    return Err(TokenGraphError::Invalid(format!(
                        "token `{}` disagrees with its winning source projection",
                        token.name
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_projection(projection: &TokenGraphProjection) -> Result<(), TokenGraphError> {
    let kind = parse_token_kind(&projection.kind).ok_or_else(|| {
        TokenGraphError::Invalid(format!(
            "unknown projected token kind `{}`",
            projection.kind
        ))
    })?;
    if projection.name.is_empty()
        || !is_lower_hex(&projection.token_id, 8)
        || u32::from_str_radix(&projection.token_id, 16).ok()
            != Some(token_id(&projection.name).get())
    {
        return Err(TokenGraphError::Invalid(format!(
            "invalid {:?} projection `{}`",
            kind, projection.name
        )));
    }
    Ok(())
}

fn reject_reference_cycles(graph: &TokenGraph) -> Result<(), TokenGraphError> {
    let indices = graph
        .sources
        .iter()
        .enumerate()
        .map(|(index, source)| (source.path.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut incoming = vec![0_usize; graph.sources.len()];
    for source in &graph.sources {
        for reference in &source.references {
            if let Some(target) = reference.target.as_deref() {
                let Some(index) = indices.get(target) else {
                    return Err(TokenGraphError::Invalid(format!(
                        "source `{}` has an absent reference target",
                        source.path
                    )));
                };
                incoming[*index] += 1;
            }
        }
    }
    let mut ready = incoming
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect::<VecDeque<_>>();
    let mut visited = 0_usize;
    while let Some(index) = ready.pop_front() {
        visited += 1;
        for reference in &graph.sources[index].references {
            if let Some(target) = reference.target.as_deref() {
                let target_index = indices[target];
                incoming[target_index] -= 1;
                if incoming[target_index] == 0 {
                    ready.push_back(target_index);
                }
            }
        }
    }
    if visited == graph.sources.len() {
        return Ok(());
    }
    let cycle_index = incoming
        .iter()
        .position(|count| *count > 0)
        .unwrap_or_default();
    Err(TokenGraphError::Invalid(format!(
        "reference graph contains a cycle near `{}`",
        graph.sources[cycle_index].path
    )))
}

fn token_key(token: &TokenGraphToken) -> (&str, &str) {
    (&token.kind, &token.name)
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_sha256(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|digest| is_lower_hex(digest, 64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_graph_round_trips_as_canonical_closed_bytes() {
        let graph = TokenGraph::from_registry(&ThemeRegistry::seed());
        let bytes = graph.to_canonical_json().expect("canonical graph");
        let parsed = parse_token_graph(&bytes).expect("parse canonical graph");

        assert_eq!(parsed, graph);
        assert_eq!(parsed.graph_version, TOKEN_GRAPH_VERSION);
        assert_eq!(parsed.themes.len(), 1);
        assert_eq!(parsed.themes[0].tokens.len(), 51);
        assert_eq!(parsed.aliases(), 0);
        assert_eq!(parsed.derived_values(), 0);
        assert!(bytes.ends_with(b"\n"));
    }

    #[test]
    fn parser_rejects_noncanonical_unknown_and_cyclic_graphs() {
        let graph = TokenGraph::from_registry(&ThemeRegistry::seed());
        let mut bytes = graph.to_canonical_json().expect("canonical graph");
        bytes.insert(0, b' ');
        assert!(matches!(
            parse_token_graph(&bytes),
            Err(TokenGraphError::NonCanonical)
        ));

        let mut value = serde_json::to_value(&graph).expect("graph JSON");
        value["unknown"] = Value::Bool(true);
        let bytes = serde_json::to_vec(&value).expect("unknown graph");
        assert!(matches!(
            parse_token_graph(&bytes),
            Err(TokenGraphError::Json(_))
        ));

        let mut cyclic = graph;
        let cyclic_path = cyclic.sources[0].path.clone();
        cyclic.sources[0].expression = TokenExpression::Alias;
        cyclic.sources[0].references.push(TokenGraphReference {
            syntax: TokenReferenceSyntax::Curly,
            reference: format!("{{{cyclic_path}}}"),
            target: Some(cyclic_path),
            property: None,
        });
        assert!(cyclic.to_canonical_json().is_err());
    }

    #[test]
    fn resource_budgets_fail_closed_without_counter_overflow() {
        let mut text = GraphResourceBudget {
            text_bytes: MAX_GRAPH_TEXT_BYTES,
            ..GraphResourceBudget::default()
        };
        assert!(text.add_text("x").is_err());

        let mut edges = GraphResourceBudget {
            reference_edges: MAX_GRAPH_REFERENCE_EDGES,
            ..GraphResourceBudget::default()
        };
        assert!(edges.add_references(1).is_err());

        let mut values = GraphResourceBudget {
            value_nodes: MAX_GRAPH_VALUE_NODES,
            ..GraphResourceBudget::default()
        };
        assert!(values.add_value(&Value::Null).is_err());

        let nested =
            (0..=MAX_GRAPH_VALUE_DEPTH).fold(Value::Null, |value, _| Value::Array(vec![value]));
        assert!(GraphResourceBudget::default().add_value(&nested).is_err());
    }

    #[test]
    fn deep_acyclic_graph_is_checked_without_recursive_traversal() {
        const SOURCE_COUNT: usize = 4_096;

        let mut graph = TokenGraph::from_registry(&ThemeRegistry::seed());
        graph.sources = (0..SOURCE_COUNT)
            .map(|index| {
                let next = (index + 1 < SOURCE_COUNT).then(|| format!("chain.{:05}", index + 1));
                TokenGraphSource {
                    path: format!("chain.{index:05}"),
                    pointer: format!("/chain/{index:05}"),
                    token_type: "color".into(),
                    expression: if next.is_some() {
                        TokenExpression::Alias
                    } else {
                        TokenExpression::Literal
                    },
                    references: next
                        .map(|target| {
                            vec![TokenGraphReference {
                                syntax: TokenReferenceSyntax::Curly,
                                reference: format!("{{{target}}}"),
                                target: Some(target),
                                property: None,
                            }]
                        })
                        .unwrap_or_default(),
                    deprecated: false,
                    projection: None,
                    resolved_value: Value::String("#000000".into()),
                }
            })
            .collect();
        for token in &mut graph.themes[0].tokens {
            token.source = None;
        }

        graph.to_canonical_json().expect("deep acyclic graph");
    }

    #[test]
    fn duplicate_source_pointers_and_empty_selections_are_rejected() {
        let mut graph = TokenGraph::from_registry(&ThemeRegistry::seed());
        let duplicate_pointer = graph.sources[0].pointer.clone();
        graph.sources[1].pointer = duplicate_pointer;
        assert!(graph.to_canonical_json().is_err());

        let mut graph = TokenGraph::from_registry(&ThemeRegistry::seed());
        graph.themes[0]
            .selections
            .insert("mode".into(), String::new());
        assert!(graph.to_canonical_json().is_err());

        let mut graph = TokenGraph::from_registry(&ThemeRegistry::seed());
        graph.themes[0].tokens[0].source = Some(graph.sources[1].path.clone());
        assert!(graph.to_canonical_json().is_err());
    }
}
