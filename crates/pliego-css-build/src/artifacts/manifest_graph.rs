use std::collections::{BTreeMap, BTreeSet};

use pliego_css_ir::{ColorValue, SemanticStyle, SemanticValue, TokenId, TokenKind};
use pliego_css_theme::ThemeRegistry;
use serde::Serialize;

use super::physical_trace::{
    PhysicalDeclaration, PhysicalProducer, PhysicalProjection, PhysicalRule,
};
use super::reachability::{ReachabilityDocument, origin_file_key};

const MAX_GRAPH_ITEMS: usize = 65_535;

/// Source range used to attach one semantic style to application ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphOrigin<'a> {
    file: Option<&'a str>,
    byte_start: Option<usize>,
    byte_end: Option<usize>,
}

impl<'a> GraphOrigin<'a> {
    /// Creates a neutral graph origin from an optional exact source range.
    #[must_use]
    pub const fn new(
        file: Option<&'a str>,
        byte_start: Option<usize>,
        byte_end: Option<usize>,
    ) -> Self {
        Self {
            file,
            byte_start,
            byte_end,
        }
    }
}

/// One semantic style and the source origins that produced it.
#[derive(Clone, Copy, Debug)]
pub struct GraphStyle<'a> {
    style: &'a SemanticStyle,
    origins: &'a [GraphOrigin<'a>],
}

impl<'a> GraphStyle<'a> {
    /// Creates a neutral semantic graph input.
    #[must_use]
    pub const fn new(style: &'a SemanticStyle, origins: &'a [GraphOrigin<'a>]) -> Self {
        Self { style, origins }
    }
}

/// Canonical schema-1 semantic and application graph embedded by manifest schema 4.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestGraph {
    schema_version: u8,
    declaration_id_format_version: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    physical_rule_id_format_version: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    physical_declaration_id_format_version: Option<u8>,
    origin_coverage: &'static str,
    application_coverage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    physical_coverage: Option<&'static str>,
    declarations: Vec<Declaration>,
    tokens: Vec<TokenNode>,
    components: Vec<Component>,
    routes: Vec<Route>,
    islands: Vec<Island>,
    #[serde(skip_serializing_if = "Option::is_none")]
    synthetic_producers: Option<Vec<PhysicalProducer>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    physical_rules: Option<Vec<PhysicalRule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    physical_declarations: Option<Vec<PhysicalDeclaration>>,
    edges: Vec<Edge>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Declaration {
    id: String,
    style_id: String,
    ordinal: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TokenNode {
    id: String,
    kind: &'static str,
    token_id: String,
    name: String,
}

#[derive(Serialize)]
struct Component {
    id: String,
}

#[derive(Serialize)]
struct Route {
    id: String,
    path: String,
}

#[derive(Serialize)]
struct Island {
    id: String,
    name: String,
}

#[derive(Eq, Ord, PartialEq, PartialOrd, Serialize)]
struct Edge {
    kind: &'static str,
    from: String,
    to: String,
}

macro_rules! edge {
    ($edges:expr, $kind:literal, $from:expr, $to:expr $(,)?) => {
        insert_edge($edges, $kind, $from, $to)
    };
}

/// Builds the canonical schema-1 manifest graph.
///
/// # Errors
///
/// Returns an error when source ownership is incomplete, a token cannot be
/// resolved, or graph limits would be exceeded.
pub fn build_manifest_graph(
    theme: &ThemeRegistry,
    styles: &[GraphStyle<'_>],
    reachability: &ReachabilityDocument,
) -> Result<ManifestGraph, String> {
    build_manifest_graph_inner(theme, styles, reachability, None)
}

/// Builds the canonical schema-2 graph with a complete physical CSS projection.
///
/// # Errors
///
/// Returns an error when the semantic graph is incomplete, the physical graph
/// has dangling endpoints, any semantic declaration lacks a physical
/// contribution, or the combined graph exceeds its hard budgets.
pub fn build_manifest_graph_with_physical(
    theme: &ThemeRegistry,
    styles: &[GraphStyle<'_>],
    reachability: &ReachabilityDocument,
    physical: PhysicalProjection,
) -> Result<ManifestGraph, String> {
    build_manifest_graph_inner(theme, styles, reachability, Some(physical))
}

fn build_manifest_graph_inner(
    theme: &ThemeRegistry,
    styles: &[GraphStyle<'_>],
    reachability: &ReachabilityDocument,
    physical: Option<PhysicalProjection>,
) -> Result<ManifestGraph, String> {
    let base_nodes = styles
        .len()
        .checked_add(reachability.components.len())
        .and_then(|count| count.checked_add(reachability.routes.len()))
        .and_then(|count| count.checked_add(reachability.islands.len()))
        .ok_or_else(|| "manifest graph node limit exceeded".to_owned())?;
    check_node_limit(base_nodes, 0, 0, 0)?;
    let sites = site_owners(reachability);
    let mut decls = BTreeMap::new();
    let mut tokens = BTreeMap::new();
    let mut edges = BTreeSet::new();

    for input in styles {
        let style = input.style;
        let sid = format!("{:032x}", style.id.get());
        let style_node = node("style", &sid);
        let owners = owners(input.origins, &sites)?;
        for (ordinal, assignment) in style.assignments.iter().enumerate() {
            let ordinal = u32::try_from(ordinal).map_err(|_| "too many declarations")?;
            let id = format!("decl:{sid}:{ordinal:08x}");
            let declaration = Declaration {
                id: id.clone(),
                style_id: sid.clone(),
                ordinal,
            };
            if decls.insert(id.clone(), declaration).is_some() {
                return Err("duplicate declaration".into());
            }

            edge!(
                &mut edges,
                "styleHasDeclaration",
                style_node.clone(),
                id.clone(),
            )?;
            for owner in &owners {
                edge!(
                    &mut edges,
                    "componentUsesDeclaration",
                    node("component", owner),
                    id.clone(),
                )?;
            }
            if let Some((kind, token_id)) = token_ref(assignment.value) {
                let token = resolve(theme, kind, token_id)?;
                edge!(&mut edges, "declarationUsesToken", id, token.id.clone())?;
                tokens.entry(token.id.clone()).or_insert(token);
            }
            check_node_limit(base_nodes, decls.len(), tokens.len(), 0)?;
        }
    }

    let (components, routes, islands) = project(reachability, &mut edges)?;
    let physical_enabled = physical.is_some();
    let (producers, physical_rules, physical_declarations) = if let Some(physical) = physical {
        let physical_nodes = physical
            .producers
            .len()
            .checked_add(physical.rules.len())
            .and_then(|count| count.checked_add(physical.declarations.len()))
            .ok_or_else(|| "manifest graph node limit exceeded".to_owned())?;
        check_node_limit(base_nodes, decls.len(), tokens.len(), physical_nodes)?;
        validate_physical_endpoints(&decls, &physical)?;
        for item in physical.edges {
            insert_edge(&mut edges, item.kind, item.from, item.to)?;
        }
        (
            Some(physical.producers),
            Some(physical.rules),
            Some(physical.declarations),
        )
    } else {
        check_node_limit(base_nodes, decls.len(), tokens.len(), 0)?;
        (None, None, None)
    };
    Ok(ManifestGraph {
        schema_version: if physical_enabled { 2 } else { 1 },
        declaration_id_format_version: 1,
        physical_rule_id_format_version: physical_enabled.then_some(1),
        physical_declaration_id_format_version: physical_enabled.then_some(1),
        origin_coverage: "compiler-verified-complete",
        application_coverage: "adapter-attested-complete",
        physical_coverage: physical_enabled.then_some("compiler-verified-complete"),
        declarations: decls.into_values().collect(),
        tokens: tokens.into_values().collect(),
        components,
        routes,
        islands,
        synthetic_producers: producers,
        physical_rules,
        physical_declarations,
        edges: edges.into_iter().collect(),
    })
}

fn check_node_limit(
    base: usize,
    declarations: usize,
    tokens: usize,
    physical: usize,
) -> Result<(), String> {
    let total = base
        .checked_add(declarations)
        .and_then(|count| count.checked_add(tokens))
        .and_then(|count| count.checked_add(physical));
    if total.is_some_and(|count| count <= MAX_GRAPH_ITEMS) {
        Ok(())
    } else {
        Err("manifest graph node limit exceeded".into())
    }
}

fn validate_physical_endpoints(
    declarations: &BTreeMap<String, Declaration>,
    physical: &PhysicalProjection,
) -> Result<(), String> {
    let producer_nodes = physical
        .producers
        .iter()
        .map(|item| item.id.as_str())
        .collect::<BTreeSet<_>>();
    let rule_nodes = physical
        .rules
        .iter()
        .map(|item| item.id.as_str())
        .collect::<BTreeSet<_>>();
    let physical_declaration_nodes = physical
        .declarations
        .iter()
        .map(|item| item.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut contributed = BTreeSet::new();
    for edge in &physical.edges {
        let valid = match edge.kind {
            "declarationContributesToPhysicalDeclaration" => {
                contributed.insert(edge.from.as_str());
                declarations.contains_key(&edge.from)
                    && physical_declaration_nodes.contains(edge.to.as_str())
            }
            "syntheticProducerProducesPhysicalDeclaration" => {
                producer_nodes.contains(edge.from.as_str())
                    && physical_declaration_nodes.contains(edge.to.as_str())
            }
            "physicalDeclarationBelongsToRule" => {
                physical_declaration_nodes.contains(edge.from.as_str())
                    && rule_nodes.contains(edge.to.as_str())
            }
            "ruleNestedInRule" => {
                rule_nodes.contains(edge.from.as_str()) && rule_nodes.contains(edge.to.as_str())
            }
            _ => false,
        };
        if !valid {
            return Err("manifest graph has an invalid physical endpoint".into());
        }
    }
    if declarations
        .keys()
        .any(|declaration| !contributed.contains(declaration.as_str()))
    {
        return Err("semantic declaration lacks a physical contribution".into());
    }
    Ok(())
}

fn insert_edge(
    edges: &mut BTreeSet<Edge>,
    kind: &'static str,
    from: String,
    to: String,
) -> Result<(), String> {
    edges.insert(Edge { kind, from, to });
    if edges.len() <= MAX_GRAPH_ITEMS {
        Ok(())
    } else {
        Err("manifest graph edge limit exceeded".into())
    }
}

type Projection = (Vec<Component>, Vec<Route>, Vec<Island>);

fn project(
    reachability: &ReachabilityDocument,
    edges: &mut BTreeSet<Edge>,
) -> Result<Projection, String> {
    let components = reachability
        .components
        .iter()
        .map(|item| Component {
            id: node("component", &item.id),
        })
        .collect::<Vec<_>>();
    let routes = reachability
        .routes
        .iter()
        .map(|item| Route {
            id: node("route", &item.id),
            path: item.path.clone(),
        })
        .collect::<Vec<_>>();
    let islands = reachability
        .islands
        .iter()
        .map(|item| Island {
            id: node("island", &item.id),
            name: item.name.clone(),
        })
        .collect::<Vec<_>>();
    for route in &reachability.routes {
        for component in &route.components {
            edge!(
                edges,
                "routeUsesComponent",
                node("route", &route.id),
                node("component", component),
            )?;
        }
    }
    for island in &reachability.islands {
        for component in &island.components {
            edge!(
                edges,
                "islandUsesComponent",
                node("island", &island.id),
                node("component", component),
            )?;
        }
    }

    Ok((components, routes, islands))
}

type SiteKey = (String, usize, usize);

fn site_owners(reachability: &ReachabilityDocument) -> BTreeMap<SiteKey, BTreeSet<String>> {
    let mut owners = BTreeMap::<SiteKey, BTreeSet<String>>::new();
    for component in &reachability.components {
        for site in &component.sites {
            owners
                .entry((site.file.clone(), site.byte_start, site.byte_end))
                .or_default()
                .insert(component.id.clone());
        }
    }
    owners
}

fn owners(
    origins: &[GraphOrigin<'_>],
    site_owners: &BTreeMap<SiteKey, BTreeSet<String>>,
) -> Result<BTreeSet<String>, String> {
    let mut owners = BTreeSet::new();
    for origin in origins {
        let (Some(file), Some(start), Some(end)) =
            (origin.file, origin.byte_start, origin.byte_end)
        else {
            return Err("origin lacks a source range".into());
        };
        let key = (origin_file_key(file)?, start, end);
        let found = site_owners
            .get(&key)
            .ok_or_else(|| "origin has no component".to_owned())?;
        owners.extend(found.iter().cloned());
    }
    Ok(owners)
}

fn token_ref(value: SemanticValue) -> Option<(TokenKind, TokenId)> {
    match value {
        SemanticValue::Token(reference) => Some((reference.kind, reference.id)),
        SemanticValue::Color(ColorValue::Token { id, .. }) => Some((TokenKind::Color, id)),
        _ => None,
    }
}

fn resolve(theme: &ThemeRegistry, kind: TokenKind, id: TokenId) -> Result<TokenNode, String> {
    let kind_name = kind_name(kind);
    let token = theme
        .token_by_id(kind, id)
        .ok_or_else(|| "unknown token".to_owned())?;
    let token_id = format!("{:08x}", id.get());
    Ok(TokenNode {
        id: format!("token:{kind_name}:{token_id}"),
        kind: kind_name,
        token_id,
        name: token.name.clone(),
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

fn node(kind: &str, raw: &str) -> String {
    format!("{kind}:{raw}")
}
