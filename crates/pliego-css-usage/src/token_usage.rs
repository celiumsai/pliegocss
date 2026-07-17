use std::collections::{BTreeMap, BTreeSet};

use pliego_css_build::artifacts::{sha256_hex, validate_asset_bundle_id};
use pliego_css_compiler::{referenced_tokens, theme_custom_property_name};
use pliego_css_config::{TokenExpression, TokenGraph, TokenGraphSource, TokenGraphTheme};
use pliego_css_ir::{ColorValue, SemanticStyle, SemanticValue, TokenId, TokenKind, TokenRef};
use pliego_css_theme::ThemeRegistry;
use serde::{Deserialize, Serialize};

use super::{MAX_USAGE_DOCUMENT_BYTES, UsageSelection};

/// Wire version of the token-usage report.
pub const TOKEN_USAGE_SCHEMA_VERSION: u8 = 1;
/// Canonical adjacent filename for generated token-usage reports.
pub const TOKEN_USAGE_FILE: &str = "pliego.token-usage.json";

const REPORT_KIND: &str = "pliegocss-token-usage/1";

/// One exact retained semantic consumer of a direct typed token reference.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TokenUsageConsumerInput {
    bundle_id: String,
    style_id: String,
    reference: TokenRef,
}

impl TokenUsageConsumerInput {
    /// Creates one bundle-qualified direct token consumer.
    #[must_use]
    pub fn new(
        bundle_id: impl Into<String>,
        style_id: impl Into<String>,
        kind: TokenKind,
        token_id: TokenId,
    ) -> Self {
        Self {
            bundle_id: bundle_id.into(),
            style_id: style_id.into(),
            reference: TokenRef { kind, id: token_id },
        }
    }
}

/// Collects canonical direct token consumers from already selected semantic styles.
///
/// # Errors
///
/// Returns an error when the bundle identifier is invalid.
pub fn collect_token_usage_consumers<'a>(
    bundle_id: &str,
    styles: impl IntoIterator<Item = &'a SemanticStyle>,
) -> Result<Vec<TokenUsageConsumerInput>, String> {
    validate_asset_bundle_id(bundle_id)?;
    let mut consumers = BTreeSet::new();
    for style in styles {
        let style_id = format!("{:032x}", style.id.get());
        for assignment in &style.assignments {
            let reference = match assignment.value {
                SemanticValue::Token(reference) => Some(reference),
                SemanticValue::Color(ColorValue::Token { id, .. }) => Some(TokenRef {
                    kind: TokenKind::Color,
                    id,
                }),
                _ => None,
            };
            if let Some(reference) = reference {
                consumers.insert(TokenUsageConsumerInput::new(
                    bundle_id,
                    &style_id,
                    reference.kind,
                    reference.id,
                ));
            }
        }
    }
    Ok(consumers.into_iter().collect())
}

/// Collects direct consumers only from styles admitted by an immutable usage selection.
///
/// # Errors
///
/// Returns an error when the bundle identifier is invalid.
pub fn collect_selected_token_usage_consumers<'a>(
    bundle_id: &str,
    selection: &UsageSelection,
    styles: impl IntoIterator<Item = &'a SemanticStyle>,
) -> Result<Vec<TokenUsageConsumerInput>, String> {
    collect_token_usage_consumers(
        bundle_id,
        styles
            .into_iter()
            .filter(|style| selection.contains(bundle_id, &format!("{:032x}", style.id.get()))),
    )
}

/// Collects distinct typed references from styles admitted by an immutable usage selection.
#[must_use]
pub fn collect_selected_token_references<'a>(
    bundle_id: &str,
    selection: &UsageSelection,
    styles: impl IntoIterator<Item = &'a SemanticStyle>,
) -> BTreeSet<TokenRef> {
    referenced_tokens(
        styles
            .into_iter()
            .filter(|style| selection.contains(bundle_id, &format!("{:032x}", style.id.get()))),
    )
}

/// Collects the application-wide token union from bundle-qualified selected styles.
#[must_use]
pub fn collect_selected_bundle_token_references<'a, Styles>(
    selection: &UsageSelection,
    bundles: impl IntoIterator<Item = (&'a str, Styles)>,
) -> BTreeSet<TokenRef>
where
    Styles: IntoIterator<Item = &'a SemanticStyle>,
{
    bundles
        .into_iter()
        .flat_map(|(bundle, styles)| collect_selected_token_references(bundle, selection, styles))
        .collect()
}

/// Static disposition of one active resolved token.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TokenUsageStatus {
    /// A retained semantic style references this token directly.
    Direct,
    /// A direct token's winning source depends on this token transitively.
    Dependency,
    /// No retained semantic consumer or dependency path reaches this token.
    Unused,
}

/// Closed token-usage artifact projected from one canonical graph resolution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TokenUsageReport {
    schema_version: u8,
    report_kind: String,
    graph_version: String,
    graph_sha256: String,
    theme: ReportTheme,
    summary: ReportSummary,
    tokens: Vec<ReportToken>,
}

impl TokenUsageReport {
    /// Returns the report's active-token count.
    #[must_use]
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }

    /// Serializes canonical pretty JSON with one final LF.
    ///
    /// # Errors
    ///
    /// Returns an error when report invariants or the document-size limit are violated.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("cannot serialize token usage: {error}"))?;
        bytes.push(b'\n');
        require(bytes.len() <= MAX_USAGE_DOCUMENT_BYTES)?;
        Ok(bytes)
    }

    fn validate(&self) -> Result<(), String> {
        require(self.schema_version == TOKEN_USAGE_SCHEMA_VERSION)?;
        require(self.report_kind == REPORT_KIND)?;
        require(!self.graph_version.is_empty() && self.graph_version.len() <= 128)?;
        require_hash(&self.graph_sha256)?;
        require_hex(&self.theme.theme_id, 32)?;
        require(!self.theme.name.is_empty() && self.theme.name.len() <= 256)?;

        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut direct_ids = BTreeSet::new();
        let mut direct = 0usize;
        let mut dependency = 0usize;
        let mut unused = 0usize;
        let mut emitted = 0usize;
        require(self.tokens.windows(2).all(|pair| pair[0].id < pair[1].id))?;
        for token in &self.tokens {
            require(ids.insert(token.id.as_str()))?;
            require(names.insert((token.kind.as_str(), token.name.as_str())))?;
            require(token.id == format!("token:{}:{}", token.kind, token.token_id))?;
            require_hex(&token.token_id, 8)?;
            require(!token.name.is_empty() && token.name.len() <= 256)?;
            if let Some(source) = &token.source {
                require(!source.is_empty() && source.len() <= 4_096)?;
            }
            if let Some(property) = &token.custom_property {
                require(property.starts_with("--") && property.len() <= 512)?;
            }
            require(!token.emitted || token.status == TokenUsageStatus::Direct)?;
            require(!token.emitted || token.custom_property.is_some())?;
            match token.status {
                TokenUsageStatus::Direct => {
                    direct += 1;
                    direct_ids.insert(token.id.as_str());
                    require(!token.consumers.is_empty())?;
                }
                TokenUsageStatus::Dependency => {
                    dependency += 1;
                    require(token.consumers.is_empty() && !token.required_by.is_empty())?;
                }
                TokenUsageStatus::Unused => {
                    unused += 1;
                    require(token.consumers.is_empty() && token.required_by.is_empty())?;
                }
            }
            emitted += usize::from(token.emitted);
            let mut consumers = BTreeSet::new();
            for consumer in &token.consumers {
                validate_asset_bundle_id(&consumer.bundle_id)?;
                require_hex(&consumer.style_id, 32)?;
                require(
                    consumers.insert((consumer.bundle_id.as_str(), consumer.style_id.as_str())),
                )?;
            }
            require(token.consumers.windows(2).all(|pair| pair[0] < pair[1]))?;
            require(token.required_by.windows(2).all(|pair| pair[0] < pair[1]))?;
        }
        for token in &self.tokens {
            require(
                token
                    .required_by
                    .iter()
                    .all(|id| direct_ids.contains(id.as_str())),
            )?;
        }
        require(self.summary.tokens == self.tokens.len())?;
        require(self.summary.direct == direct)?;
        require(self.summary.dependencies == dependency)?;
        require(self.summary.unused == unused)?;
        require(self.summary.emitted_custom_properties == emitted)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ReportTheme {
    name: String,
    selections: BTreeMap<String, String>,
    theme_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ReportSummary {
    tokens: usize,
    direct: usize,
    dependencies: usize,
    unused: usize,
    emitted_custom_properties: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ReportToken {
    id: String,
    kind: String,
    token_id: String,
    name: String,
    source: Option<String>,
    expression: TokenExpression,
    deprecated: bool,
    status: TokenUsageStatus,
    custom_property: Option<String>,
    emitted: bool,
    consumers: Vec<ReportConsumer>,
    required_by: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ReportConsumer {
    bundle_id: String,
    style_id: String,
}

/// Builds one deterministic report for an exact token-graph resolution and retained consumer set.
///
/// # Errors
///
/// Returns an error for graph/registry drift, invalid consumers, dangling dependency sources, or
/// document-limit violations.
pub fn build_token_usage_report(
    graph: &TokenGraph,
    selections: &BTreeMap<String, String>,
    registry: &ThemeRegistry,
    consumers: impl IntoIterator<Item = TokenUsageConsumerInput>,
    theme_emitted: bool,
) -> Result<TokenUsageReport, String> {
    let graph_bytes = graph
        .to_canonical_json()
        .map_err(|error| format!("cannot encode token graph: {error}"))?;
    let resolution = graph
        .theme(selections)
        .ok_or_else(|| "token graph has no theme for the selected resolver inputs".to_owned())?;
    validate_resolution(resolution, registry)?;

    let resolved = resolution
        .tokens
        .iter()
        .map(|token| Ok((reference(&token.kind, &token.token_id)?, token)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    require(resolved.len() == resolution.tokens.len())?;
    let sources = graph
        .sources
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let mut direct = BTreeMap::<TokenRef, BTreeSet<ReportConsumer>>::new();
    for consumer in consumers {
        validate_asset_bundle_id(&consumer.bundle_id)?;
        require_hex(&consumer.style_id, 32)?;
        require(resolved.contains_key(&consumer.reference))?;
        direct
            .entry(consumer.reference)
            .or_default()
            .insert(ReportConsumer {
                bundle_id: consumer.bundle_id,
                style_id: consumer.style_id,
            });
    }

    let mut required_by = BTreeMap::<TokenRef, BTreeSet<TokenRef>>::new();
    for root in direct.keys().copied() {
        let Some(source) = resolved[&root].source.as_deref() else {
            continue;
        };
        collect_dependencies(root, source, &sources, &resolved, &mut required_by)?;
    }

    let tokens = build_report_tokens(&resolved, &sources, &direct, &required_by, theme_emitted);
    let summary = ReportSummary {
        tokens: tokens.len(),
        direct: tokens
            .iter()
            .filter(|token| token.status == TokenUsageStatus::Direct)
            .count(),
        dependencies: tokens
            .iter()
            .filter(|token| token.status == TokenUsageStatus::Dependency)
            .count(),
        unused: tokens
            .iter()
            .filter(|token| token.status == TokenUsageStatus::Unused)
            .count(),
        emitted_custom_properties: tokens.iter().filter(|token| token.emitted).count(),
    };
    let report = TokenUsageReport {
        schema_version: TOKEN_USAGE_SCHEMA_VERSION,
        report_kind: REPORT_KIND.into(),
        graph_version: graph.graph_version.clone(),
        graph_sha256: sha256_hex(&graph_bytes),
        theme: ReportTheme {
            name: resolution.name.clone(),
            selections: selections.clone(),
            theme_id: resolution.theme_id.clone(),
        },
        summary,
        tokens,
    };
    report.validate()?;
    Ok(report)
}

fn build_report_tokens(
    resolved: &BTreeMap<TokenRef, &pliego_css_config::TokenGraphToken>,
    sources: &BTreeMap<&str, &TokenGraphSource>,
    direct: &BTreeMap<TokenRef, BTreeSet<ReportConsumer>>,
    required_by: &BTreeMap<TokenRef, BTreeSet<TokenRef>>,
    theme_emitted: bool,
) -> Vec<ReportToken> {
    let mut tokens = resolved
        .iter()
        .map(|(reference, token)| {
            let source = token
                .source
                .as_deref()
                .and_then(|path| sources.get(path).copied());
            let status = if direct.contains_key(reference) {
                TokenUsageStatus::Direct
            } else if required_by.contains_key(reference) {
                TokenUsageStatus::Dependency
            } else {
                TokenUsageStatus::Unused
            };
            let custom_property = theme_custom_property_name(reference.kind, &token.name);
            ReportToken {
                id: token_node_id(&token.kind, &token.token_id),
                kind: token.kind.clone(),
                token_id: token.token_id.clone(),
                name: token.name.clone(),
                source: token.source.clone(),
                expression: source.map_or(TokenExpression::Literal, |source| source.expression),
                deprecated: source.is_some_and(|source| source.deprecated),
                emitted: theme_emitted
                    && status == TokenUsageStatus::Direct
                    && custom_property.is_some(),
                custom_property,
                status,
                consumers: direct
                    .get(reference)
                    .map_or_else(Vec::new, |items| items.iter().cloned().collect()),
                required_by: required_by.get(reference).map_or_else(Vec::new, |roots| {
                    roots
                        .iter()
                        .map(|root| {
                            let token = resolved[root];
                            token_node_id(&token.kind, &token.token_id)
                        })
                        .collect()
                }),
            }
        })
        .collect::<Vec<_>>();
    tokens.sort_by(|left, right| left.id.cmp(&right.id));
    tokens
}

fn collect_dependencies(
    root: TokenRef,
    initial: &str,
    sources: &BTreeMap<&str, &TokenGraphSource>,
    resolved: &BTreeMap<TokenRef, &pliego_css_config::TokenGraphToken>,
    output: &mut BTreeMap<TokenRef, BTreeSet<TokenRef>>,
) -> Result<(), String> {
    let mut pending = vec![initial];
    let mut visited = BTreeSet::new();
    while let Some(path) = pending.pop() {
        if !visited.insert(path) {
            continue;
        }
        let source = sources
            .get(path)
            .ok_or_else(|| format!("token graph source `{path}` is absent"))?;
        for edge in &source.references {
            let Some(target) = edge.target.as_deref() else {
                continue;
            };
            let target_source = sources
                .get(target)
                .ok_or_else(|| format!("token graph dependency source `{target}` is absent"))?;
            pending.push(target);
            if let Some(projection) = &target_source.projection {
                let reference = reference(&projection.kind, &projection.token_id)?;
                require(resolved.contains_key(&reference))?;
                if reference != root {
                    output.entry(reference).or_default().insert(root);
                }
            }
        }
    }
    Ok(())
}

fn validate_resolution(
    resolution: &TokenGraphTheme,
    registry: &ThemeRegistry,
) -> Result<(), String> {
    require(resolution.theme_id == format!("{:032x}", registry.id().get()))?;
    require(resolution.tokens.len() == registry.tokens().len())?;
    for token in &resolution.tokens {
        let reference = reference(&token.kind, &token.token_id)?;
        let actual = registry
            .token_by_id(reference.kind, reference.id)
            .ok_or_else(|| format!("token graph contains absent token `{}`", token.name))?;
        require(actual.name == token.name && actual.value == token.value)?;
    }
    Ok(())
}

/// Parses exact canonical token-usage report bytes.
///
/// # Errors
///
/// Rejects oversized, malformed, noncanonical, or internally inconsistent documents.
pub fn parse_token_usage_report(source: &[u8]) -> Result<TokenUsageReport, String> {
    require(source.len() <= MAX_USAGE_DOCUMENT_BYTES)?;
    let report: TokenUsageReport = serde_json::from_slice(source)
        .map_err(|error| format!("invalid token usage JSON: {error}"))?;
    report.validate()?;
    require(report.to_canonical_json()? == source)?;
    Ok(report)
}

/// Finds one token by exact `kind.name` or `token:kind:tokenId` identity.
///
/// # Errors
///
/// Returns an error when the query is empty, ambiguous, or absent.
pub fn explain_token_usage(report: &TokenUsageReport, query: &str) -> Result<String, String> {
    serde_json::to_string_pretty(resolve_query(report, query)?)
        .map(|mut json| {
            json.push('\n');
            json
        })
        .map_err(|error| format!("cannot serialize token usage explanation: {error}"))
}

/// Renders one exact token query as concise human-readable text.
///
/// # Errors
///
/// Returns an error when the query is empty, ambiguous, or absent.
pub fn explain_token_usage_text(report: &TokenUsageReport, query: &str) -> Result<String, String> {
    use std::fmt::Write as _;

    let token = resolve_query(report, query)?;
    let status = match token.status {
        TokenUsageStatus::Direct => "direct",
        TokenUsageStatus::Dependency => "dependency",
        TokenUsageStatus::Unused => "unused",
    };
    let mut output = format!(
        "token {}.{} ({})\nstatus: {status}\n",
        token.kind, token.name, token.id
    );
    if token.emitted {
        let property = token
            .custom_property
            .as_deref()
            .ok_or_else(|| "invalid token usage".to_owned())?;
        writeln!(output, "emitted custom property: {property}")
            .expect("writing to a string cannot fail");
    } else {
        output.push_str("emitted custom property: none\n");
    }
    for consumer in &token.consumers {
        writeln!(
            output,
            "consumer: bundle {} style {}",
            consumer.bundle_id, consumer.style_id
        )
        .expect("writing to a string cannot fail");
    }
    for root in &token.required_by {
        writeln!(output, "required by: {root}").expect("writing to a string cannot fail");
    }
    if token.status == TokenUsageStatus::Unused {
        output.push_str("reason: no retained semantic consumer or token dependency\n");
    }
    Ok(output)
}

fn resolve_query<'a>(report: &'a TokenUsageReport, query: &str) -> Result<&'a ReportToken, String> {
    require(!query.is_empty() && query.len() <= 512)?;
    let mut matches = report
        .tokens
        .iter()
        .filter(|token| token.id == query || format!("{}.{}", token.kind, token.name) == query);
    let Some(token) = matches.next() else {
        return Err(format!("token usage query `{query}` matched no token"));
    };
    if matches.next().is_some() {
        return Err(format!("token usage query `{query}` is ambiguous"));
    }
    Ok(token)
}

fn reference(kind: &str, token_id: &str) -> Result<TokenRef, String> {
    let kind = match kind {
        "spacing" => TokenKind::Spacing,
        "color" => TokenKind::Color,
        "font-family" => TokenKind::FontFamily,
        "font-size" => TokenKind::FontSize,
        "font-weight" => TokenKind::FontWeight,
        "line-height" => TokenKind::LineHeight,
        "letter-spacing" => TokenKind::LetterSpacing,
        "radius" => TokenKind::Radius,
        "shadow" => TokenKind::Shadow,
        "z-index" => TokenKind::ZIndex,
        _ => return Err(format!("token graph contains unknown token kind `{kind}`")),
    };
    let id = u32::from_str_radix(token_id, 16)
        .map_err(|_| format!("token graph contains invalid token ID `{token_id}`"))?;
    Ok(TokenRef {
        kind,
        id: TokenId::new(id),
    })
}

fn token_node_id(kind: &str, token_id: &str) -> String {
    format!("token:{kind}:{token_id}")
}

fn require_hash(value: &str) -> Result<(), String> {
    require_hex(value, 64)
}

fn require_hex(value: &str, length: usize) -> Result<(), String> {
    require(
        value.len() == length
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
    )
}

fn require(condition: bool) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err("invalid token usage".into())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use pliego_css_config::{
        TokenExpression, TokenGraph, TokenGraphReference, TokenReferenceSyntax,
    };
    use pliego_css_ir::TokenKind;
    use pliego_css_theme::ThemeRegistry;

    use super::{
        TokenUsageConsumerInput, TokenUsageStatus, build_token_usage_report, explain_token_usage,
        parse_token_usage_report,
    };

    fn direct_accent(graph: &TokenGraph, theme: &ThemeRegistry) -> TokenUsageConsumerInput {
        let accent = graph.themes[0]
            .tokens
            .iter()
            .find(|token| token.kind == "color" && token.name == "accent")
            .expect("seed graph must contain accent");
        let id = u32::from_str_radix(&accent.token_id, 16).expect("token ID must be hexadecimal");
        let actual = theme
            .token_by_id(TokenKind::Color, pliego_css_ir::TokenId::new(id))
            .expect("seed registry must contain accent");
        TokenUsageConsumerInput::new(
            "application",
            "0123456789abcdef0123456789abcdef",
            actual.kind,
            actual.id,
        )
    }

    #[test]
    fn report_separates_direct_dependency_and_unused_tokens() {
        let theme = ThemeRegistry::seed();
        let mut graph = TokenGraph::from_registry(&theme);
        let accent = graph
            .sources
            .iter_mut()
            .find(|source| source.path == "registry.color.accent")
            .expect("seed graph must contain accent source");
        accent.expression = TokenExpression::Alias;
        accent.references.push(TokenGraphReference {
            syntax: TokenReferenceSyntax::Curly,
            reference: "{registry.color.surface}".into(),
            target: Some("registry.color.surface".into()),
            property: None,
        });
        let report = build_token_usage_report(
            &graph,
            &BTreeMap::new(),
            &theme,
            [direct_accent(&graph, &theme)],
            true,
        )
        .expect("report must build");

        let accent = report
            .tokens
            .iter()
            .find(|token| token.name == "accent" && token.kind == "color")
            .expect("accent report node must exist");
        assert_eq!(accent.status, TokenUsageStatus::Direct);
        assert_eq!(accent.custom_property.as_deref(), Some("--color-accent"));
        assert!(accent.emitted);
        let surface = report
            .tokens
            .iter()
            .find(|token| token.name == "surface" && token.kind == "color")
            .expect("surface report node must exist");
        assert_eq!(surface.status, TokenUsageStatus::Dependency);
        assert_eq!(
            surface.required_by.as_slice(),
            std::slice::from_ref(&accent.id)
        );
        assert!(
            report
                .tokens
                .iter()
                .any(|token| token.status == TokenUsageStatus::Unused)
        );
    }

    #[test]
    fn canonical_round_trip_and_query_are_exact() {
        let theme = ThemeRegistry::seed();
        let graph = TokenGraph::from_registry(&theme);
        let report = build_token_usage_report(
            &graph,
            &BTreeMap::new(),
            &theme,
            [direct_accent(&graph, &theme)],
            true,
        )
        .expect("report must build");
        let bytes = report.to_canonical_json().expect("report must encode");
        let parsed = parse_token_usage_report(&bytes).expect("report must parse");
        assert_eq!(parsed, report);
        let explanation = explain_token_usage(&parsed, "color.accent").expect("query must resolve");
        assert!(explanation.contains("\"status\": \"direct\""));
        assert!(explanation.ends_with('\n'));

        let report_without_theme = build_token_usage_report(
            &graph,
            &BTreeMap::new(),
            &theme,
            [direct_accent(&graph, &theme)],
            false,
        )
        .expect("report without theme emission must build");
        let accent = report_without_theme
            .tokens
            .iter()
            .find(|token| token.name == "accent" && token.kind == "color")
            .expect("accent report node must exist");
        assert!(!accent.emitted);
        assert_eq!(accent.custom_property.as_deref(), Some("--color-accent"));

        let mut noncanonical = bytes;
        noncanonical.pop();
        assert!(parse_token_usage_report(&noncanonical).is_err());
        assert!(explain_token_usage(&parsed, "color.absent").is_err());
    }
}
