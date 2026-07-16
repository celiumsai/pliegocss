use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_ASSET_BYTES: usize = 16 * 1024 * 1024;
const MAX_ITEMS: usize = 65_535;
const MAX_BUNDLE_ID_BYTES: usize = 64;
const MAX_ID_BYTES: usize = 256;
const MAX_PATH_OR_NAME_BYTES: usize = 4 * 1024;

/// Rule-selection mode represented by an asset-plan artifact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetRuleSelection {
    /// Every compiled style remains in its source bundle.
    AllCompiled,
    /// Only complete `StyleId` sets reachable from the application graph remain.
    ReachableStyleIds,
    /// Complete `StyleId` sets remain when reachable or explicitly retained by policy.
    ReachableOrRetainedStyleIds,
}

impl AssetRuleSelection {
    pub(super) const fn artifact_schema_version(self) -> u8 {
        match self {
            Self::AllCompiled | Self::ReachableStyleIds => 1,
            Self::ReachableOrRetainedStyleIds => 2,
        }
    }
}

/// One compiled bundle used to derive a neutral asset plan.
#[derive(Clone, Copy, Debug)]
pub struct AssetPlanBundle<'a> {
    pub(super) id: &'a str,
    pub(super) emits_theme: bool,
    pub(super) css: &'a [u8],
    pub(super) manifest: &'a [u8],
}

impl<'a> AssetPlanBundle<'a> {
    /// Creates one asset-plan input from exact adjacent CSS and manifest bytes.
    #[must_use]
    pub const fn new(id: &'a str, emits_theme: bool, css: &'a [u8], manifest: &'a [u8]) -> Self {
        Self {
            id,
            emits_theme,
            css,
            manifest,
        }
    }
}

/// Returns the lowercase hexadecimal SHA-256 digest of exact bytes.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Validates a portable bundle identifier shared by plans and generated artifacts.
///
/// # Errors
///
/// Returns an error unless `id` contains 1-64 lowercase kebab-case ASCII characters,
/// starts with a letter, has no repeated or trailing hyphen, and is not a reserved
/// Windows device name.
pub fn validate_asset_bundle_id(id: &str) -> Result<(), String> {
    let bytes = id.as_bytes();
    let valid_length = !bytes.is_empty() && bytes.len() <= MAX_BUNDLE_ID_BYTES;
    let valid_characters = bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes.last() != Some(&b'-')
        && !id.contains("--")
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-');
    let reserved = matches!(
        id,
        "con"
            | "prn"
            | "aux"
            | "nul"
            | "com1"
            | "com2"
            | "com3"
            | "com4"
            | "com5"
            | "com6"
            | "com7"
            | "com8"
            | "com9"
            | "lpt1"
            | "lpt2"
            | "lpt3"
            | "lpt4"
            | "lpt5"
            | "lpt6"
            | "lpt7"
            | "lpt8"
            | "lpt9"
    );
    require(
        valid_length && valid_characters && !reserved,
        format!(
            "unsafe bundle name `{id}`; use 1-64 lowercase kebab-case ASCII characters and avoid reserved device names"
        ),
    )
}

/// Builds a deterministic schema-1 or schema-2 route/island-to-bundle asset plan.
///
/// Manifest schema 4/graph schema 1 and manifest schema 5/graph schema 2 are
/// accepted. The rule-selection mode determines the compatible artifact schema. Every input is
/// validated and integrity-bound to its exact CSS bytes before any topology is used.
///
/// # Errors
///
/// Returns an error for malformed or incompatible manifests, integrity drift,
/// unsafe or duplicate bundle IDs, invalid graph endpoints, ambiguous theme
/// ownership, or defensive limit violations.
#[allow(clippy::too_many_lines)]
pub fn build_asset_plan(
    bundles: &[AssetPlanBundle<'_>],
    rule_selection: AssetRuleSelection,
) -> Result<Vec<u8>, String> {
    require(
        !bundles.is_empty(),
        "asset plan requires at least one bundle",
    )?;
    require(
        bundles.len() <= MAX_ITEMS,
        "asset plan bundle limit exceeded",
    )?;

    let mut ids = BTreeSet::new();
    let mut theme_count = 0_usize;
    for bundle in bundles {
        validate_asset_bundle_id(bundle.id)?;
        require(
            ids.insert(bundle.id),
            format!("duplicate asset bundle `{}`", bundle.id),
        )?;
        theme_count = theme_count
            .checked_add(usize::from(bundle.emits_theme))
            .ok_or_else(|| "asset plan bundle limit exceeded".to_owned())?;
    }
    require(
        theme_count <= 1,
        "asset plan accepts at most one theme-emitting bundle",
    )?;

    let mut ordered = bundles.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        right
            .emits_theme
            .cmp(&left.emits_theme)
            .then_with(|| left.id.cmp(right.id))
    });

    let mut prepared = Vec::with_capacity(ordered.len());
    let mut component_bundles = BTreeMap::<String, Vec<usize>>::new();
    let mut common_identity = None;
    let mut common_topology = None;
    for bundle in ordered {
        require(
            bundle.css.len() <= MAX_ASSET_BYTES,
            format!("bundle `{}` CSS exceeds 16 MiB", bundle.id),
        )?;
        require(
            bundle.manifest.len() <= MAX_ASSET_BYTES,
            format!("bundle `{}` manifest exceeds 16 MiB", bundle.id),
        )?;
        let document: ManifestDocument = serde_json::from_slice(bundle.manifest)
            .map_err(|error| format!("bundle `{}` has an invalid manifest: {error}", bundle.id))?;
        let validated = validate_manifest(document, bundle.css, bundle.emits_theme)
            .map_err(|error| format!("bundle `{}`: {error}", bundle.id))?;

        if let Some(identity) = &common_identity {
            require(
                identity == &validated.identity,
                format!("bundle `{}` has an incompatible build identity", bundle.id),
            )?;
        } else {
            common_identity = Some(validated.identity.clone());
        }
        if let Some(topology) = &common_topology {
            require(
                topology == &validated.topology,
                format!(
                    "bundle `{}` has incompatible application topology",
                    bundle.id
                ),
            )?;
        } else {
            common_topology = Some(validated.topology.clone());
        }

        let bundle_index = prepared.len();
        for component in validated.active_components {
            component_bundles
                .entry(component)
                .or_default()
                .push(bundle_index);
        }
        prepared.push(PreparedBundle {
            id: bundle.id.to_owned(),
            emits_theme: bundle.emits_theme,
            css_bytes: bundle.css.len(),
            css_sha256: sha256_hex(bundle.css),
            manifest_bytes: bundle.manifest.len(),
            manifest_sha256: sha256_hex(bundle.manifest),
        });
    }

    let identity = common_identity
        .ok_or_else(|| "asset plan failed to establish a common identity".to_owned())?;
    let topology = common_topology
        .ok_or_else(|| "asset plan failed to establish common topology".to_owned())?;
    let mut plan_item_count = prepared.len();
    plan_item_count = checked_add(plan_item_count, topology.routes.len())?;
    plan_item_count = checked_add(plan_item_count, topology.islands.len())?;
    require(
        plan_item_count <= MAX_ITEMS,
        "asset plan item limit exceeded",
    )?;

    let mut routes = Vec::with_capacity(topology.routes.len());
    for (id, path) in &topology.routes {
        let components = topology
            .route_components
            .get(id)
            .ok_or_else(|| "asset plan route topology is incomplete".to_owned())?;
        let selected = selected_bundles(
            &prepared,
            &component_bundles,
            components,
            MAX_ITEMS - plan_item_count,
        )?;
        plan_item_count = checked_add(plan_item_count, selected.len())?;
        routes.push(AssetRoute {
            id: id.clone(),
            path: path.clone(),
            bundles: selected,
        });
    }

    let mut islands = Vec::with_capacity(topology.islands.len());
    for (id, name) in &topology.islands {
        let components = topology
            .island_components
            .get(id)
            .ok_or_else(|| "asset plan island topology is incomplete".to_owned())?;
        let selected = selected_bundles(
            &prepared,
            &component_bundles,
            components,
            MAX_ITEMS - plan_item_count,
        )?;
        plan_item_count = checked_add(plan_item_count, selected.len())?;
        islands.push(AssetIsland {
            id: id.clone(),
            name: name.clone(),
            bundles: selected,
        });
    }
    require(
        plan_item_count <= MAX_ITEMS,
        "asset plan item limit exceeded",
    )?;

    let output_bundles = prepared
        .into_iter()
        .map(|bundle| AssetBundle {
            css_file: format!("{}.css", bundle.id),
            manifest_file: format!("{}.manifest.json", bundle.id),
            id: bundle.id,
            emits_theme: bundle.emits_theme,
            css_bytes: bundle.css_bytes,
            css_sha256: bundle.css_sha256,
            manifest_bytes: bundle.manifest_bytes,
            manifest_sha256: bundle.manifest_sha256,
        })
        .collect();
    let plan = AssetPlan {
        schema_version: rule_selection.artifact_schema_version(),
        manifest_schema_version: identity.manifest_schema_version,
        graph_schema_version: identity.graph_schema_version,
        rule_selection,
        origin_coverage: "compiler-verified-complete",
        application_coverage: "adapter-attested-complete",
        style_id_format_version: identity.style_id_format_version,
        class_name_format_version: identity.class_name_format_version,
        theme_id_format_version: identity.theme_id_format_version,
        theme_id: identity.theme_id,
        targets: identity.targets,
        format: identity.format,
        bundles: output_bundles,
        routes,
        islands,
    };
    let mut bytes = serde_json::to_vec_pretty(&plan)
        .map_err(|error| format!("cannot serialize asset plan: {error}"))?;
    validate_plan_length(bytes.len())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn selected_bundles(
    bundles: &[PreparedBundle],
    component_bundles: &BTreeMap<String, Vec<usize>>,
    components: &BTreeSet<String>,
    maximum: usize,
) -> Result<Vec<String>, String> {
    let mut selected = BTreeSet::new();
    if let Some(index) = bundles.iter().position(|bundle| bundle.emits_theme) {
        selected.insert(index);
    }
    for component in components {
        if let Some(indices) = component_bundles.get(component) {
            for index in indices {
                selected.insert(*index);
                require(selected.len() <= maximum, "asset plan item limit exceeded")?;
            }
        }
    }
    require(selected.len() <= maximum, "asset plan item limit exceeded")?;
    Ok(selected
        .into_iter()
        .map(|index| bundles[index].id.clone())
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetPlan {
    schema_version: u8,
    manifest_schema_version: u8,
    graph_schema_version: u8,
    rule_selection: AssetRuleSelection,
    origin_coverage: &'static str,
    application_coverage: &'static str,
    style_id_format_version: u16,
    class_name_format_version: u16,
    theme_id_format_version: u16,
    theme_id: String,
    targets: String,
    format: String,
    bundles: Vec<AssetBundle>,
    routes: Vec<AssetRoute>,
    islands: Vec<AssetIsland>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetBundle {
    id: String,
    css_file: String,
    manifest_file: String,
    emits_theme: bool,
    css_bytes: usize,
    css_sha256: String,
    manifest_bytes: usize,
    manifest_sha256: String,
}

#[derive(Serialize)]
struct AssetRoute {
    id: String,
    path: String,
    bundles: Vec<String>,
}

#[derive(Serialize)]
struct AssetIsland {
    id: String,
    name: String,
    bundles: Vec<String>,
}

struct PreparedBundle {
    id: String,
    emits_theme: bool,
    css_bytes: usize,
    css_sha256: String,
    manifest_bytes: usize,
    manifest_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BuildIdentity {
    manifest_schema_version: u8,
    graph_schema_version: u8,
    style_id_format_version: u16,
    class_name_format_version: u16,
    theme_id_format_version: u16,
    theme_id: String,
    targets: String,
    format: String,
}

struct ValidatedManifest {
    identity: BuildIdentity,
    topology: ApplicationTopology,
    active_components: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ApplicationTopology {
    components: BTreeSet<String>,
    routes: BTreeMap<String, String>,
    islands: BTreeMap<String, String>,
    route_components: BTreeMap<String, BTreeSet<String>>,
    island_components: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ManifestDocument {
    schema_version: u8,
    style_id_format_version: u16,
    class_name_format_version: u16,
    theme_id_format_version: u16,
    theme_id: String,
    targets: String,
    format: String,
    css_sha256: String,
    css_bytes: usize,
    styles: Vec<ManifestStyleDocument>,
    graph: GraphDocument,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ManifestStyleDocument {
    style_id: String,
    class_name: String,
    origins: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct GraphDocument {
    schema_version: u8,
    declaration_id_format_version: u8,
    physical_rule_id_format_version: Option<u8>,
    physical_declaration_id_format_version: Option<u8>,
    origin_coverage: String,
    application_coverage: String,
    physical_coverage: Option<String>,
    declarations: Vec<DeclarationDocument>,
    tokens: Vec<TokenDocument>,
    components: Vec<ComponentDocument>,
    routes: Vec<RouteDocument>,
    islands: Vec<IslandDocument>,
    synthetic_producers: Option<Vec<PhysicalProducerDocument>>,
    physical_rules: Option<Vec<PhysicalRuleDocument>>,
    physical_declarations: Option<Vec<PhysicalDeclarationDocument>>,
    edges: Vec<EdgeDocument>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DeclarationDocument {
    id: String,
    style_id: String,
    ordinal: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TokenDocument {
    id: String,
    kind: String,
    token_id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentDocument {
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteDocument {
    id: String,
    path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IslandDocument {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct PhysicalProducerDocument {
    id: String,
    kind: String,
}

#[derive(Deserialize)]
struct PhysicalRuleDocument {
    id: String,
    ordinal: u32,
    kind: String,
}

#[derive(Deserialize)]
struct PhysicalDeclarationDocument {
    id: String,
    ordinal: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
struct EdgeDocument {
    kind: String,
    from: String,
    to: String,
}

fn validate_manifest(
    document: ManifestDocument,
    css: &[u8],
    emits_theme: bool,
) -> Result<ValidatedManifest, String> {
    let expected_graph = match document.schema_version {
        4 => 1,
        5 => 2,
        _ => return Err("unsupported asset manifest schema; expected 4 or 5".into()),
    };
    require(
        document.graph.schema_version == expected_graph,
        "manifest and graph schema versions are incompatible",
    )?;
    require(
        document.style_id_format_version > 0
            && document.class_name_format_version > 0
            && document.theme_id_format_version > 0,
        "manifest identity format version must be non-zero",
    )?;
    validate_lower_hex(&document.theme_id, 32, "theme ID")?;
    require(
        matches!(
            document.targets.as_str(),
            "baseline-widely" | "modern" | "none"
        ),
        "unsupported CSS target contract",
    )?;
    require(
        matches!(document.format.as_str(), "minified" | "pretty"),
        "unsupported CSS format contract",
    )?;
    validate_lower_hex(&document.css_sha256, 64, "CSS SHA-256")?;
    require(
        document.css_bytes == css.len(),
        "CSS byte count does not match",
    )?;
    require(
        document.css_sha256 == sha256_hex(css),
        "CSS SHA-256 does not match",
    )?;

    let style_ids = validate_styles(&document.styles)?;
    let (topology, active_components, has_theme_producer) =
        validate_graph(&document.graph, &style_ids)?;
    if expected_graph == 2 {
        require(
            has_theme_producer == emits_theme,
            "theme-emission flag does not match graph physical producer",
        )?;
    }

    Ok(ValidatedManifest {
        identity: BuildIdentity {
            manifest_schema_version: document.schema_version,
            graph_schema_version: document.graph.schema_version,
            style_id_format_version: document.style_id_format_version,
            class_name_format_version: document.class_name_format_version,
            theme_id_format_version: document.theme_id_format_version,
            theme_id: document.theme_id,
            targets: document.targets,
            format: document.format,
        },
        topology,
        active_components,
    })
}

fn validate_styles(styles: &[ManifestStyleDocument]) -> Result<BTreeSet<String>, String> {
    let mut ids = BTreeSet::new();
    let mut classes = BTreeSet::new();
    for style in styles {
        validate_lower_hex(&style.style_id, 32, "style ID")?;
        validate_text(&style.class_name, MAX_ID_BYTES, "class name")?;
        require(ids.insert(style.style_id.clone()), "duplicate style ID")?;
        require(
            classes.insert(style.class_name.clone()),
            "duplicate style class name",
        )?;
        require(
            style.origins.len() <= MAX_ITEMS,
            "manifest style origin limit exceeded",
        )?;
    }
    require(styles.len() <= MAX_ITEMS, "manifest style limit exceeded")?;
    Ok(ids)
}

#[allow(clippy::too_many_lines)]
fn validate_graph(
    graph: &GraphDocument,
    style_ids: &BTreeSet<String>,
) -> Result<(ApplicationTopology, BTreeSet<String>, bool), String> {
    require(
        graph.declaration_id_format_version == 1,
        "unsupported declaration ID format",
    )?;
    require(
        graph.origin_coverage == "compiler-verified-complete",
        "manifest origin coverage is incomplete",
    )?;
    require(
        graph.application_coverage == "adapter-attested-complete",
        "manifest application coverage is incomplete",
    )?;

    let (producers, physical_rules, physical_declarations) = match graph.schema_version {
        1 => {
            require(
                graph.physical_rule_id_format_version.is_none()
                    && graph.physical_declaration_id_format_version.is_none()
                    && graph.physical_coverage.is_none()
                    && graph.synthetic_producers.is_none()
                    && graph.physical_rules.is_none()
                    && graph.physical_declarations.is_none(),
                "graph schema 1 contains physical-trace fields",
            )?;
            (&[][..], &[][..], &[][..])
        }
        2 => {
            require(
                graph.physical_rule_id_format_version == Some(1)
                    && graph.physical_declaration_id_format_version == Some(1)
                    && graph.physical_coverage.as_deref() == Some("compiler-verified-complete"),
                "graph schema 2 has an invalid physical contract",
            )?;
            (
                graph
                    .synthetic_producers
                    .as_deref()
                    .ok_or_else(|| "graph schema 2 lacks synthetic producers".to_owned())?,
                graph
                    .physical_rules
                    .as_deref()
                    .ok_or_else(|| "graph schema 2 lacks physical rules".to_owned())?,
                graph
                    .physical_declarations
                    .as_deref()
                    .ok_or_else(|| "graph schema 2 lacks physical declarations".to_owned())?,
            )
        }
        _ => return Err("unsupported manifest graph schema".into()),
    };

    let node_count = checked_sum([
        style_ids.len(),
        graph.declarations.len(),
        graph.tokens.len(),
        graph.components.len(),
        graph.routes.len(),
        graph.islands.len(),
        producers.len(),
        physical_rules.len(),
        physical_declarations.len(),
    ])?;
    require(
        node_count <= MAX_ITEMS,
        "manifest graph node limit exceeded",
    )?;
    require(
        graph.edges.len() <= MAX_ITEMS,
        "manifest graph edge limit exceeded",
    )?;

    let style_nodes = style_ids
        .iter()
        .map(|id| format!("style:{id}"))
        .collect::<BTreeSet<_>>();
    let declarations = validate_declarations(&graph.declarations, style_ids)?;
    let tokens = validate_tokens(&graph.tokens)?;
    let components = validate_components(&graph.components)?;
    let routes = validate_routes(&graph.routes)?;
    let islands = validate_islands(&graph.islands)?;
    let producer_nodes = validate_producers(producers)?;
    let rule_nodes = validate_physical_rules(physical_rules)?;
    let physical_declaration_nodes =
        validate_physical_declarations(physical_declarations, &rule_nodes)?;

    let mut route_components = routes
        .keys()
        .map(|id| (id.clone(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut island_components = islands
        .keys()
        .map(|id| (id.clone(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut unique_edges = BTreeSet::new();
    let mut style_links = BTreeMap::<String, BTreeSet<String>>::new();
    let mut owned_declarations = BTreeSet::new();
    let mut used_tokens = BTreeSet::new();
    let mut active_components = BTreeSet::new();
    let mut semantic_contributions = BTreeSet::new();
    let mut synthetic_contributions = BTreeSet::new();
    let mut physical_produced = BTreeSet::new();
    let mut physical_owners = BTreeMap::<String, usize>::new();
    let mut rule_parents = BTreeMap::<String, String>::new();

    for edge in &graph.edges {
        require(
            unique_edges.insert(edge.clone()),
            "duplicate manifest graph edge",
        )?;
        match edge.kind.as_str() {
            "styleHasDeclaration" => {
                require(
                    style_nodes.contains(&edge.from) && declarations.contains_key(&edge.to),
                    "styleHasDeclaration has a dangling endpoint",
                )?;
                let expected = format!("style:{}", declarations[&edge.to]);
                require(
                    edge.from == expected,
                    "declaration is linked to the wrong style",
                )?;
                style_links
                    .entry(edge.to.clone())
                    .or_default()
                    .insert(edge.from.clone());
            }
            "declarationUsesToken" => {
                require(
                    declarations.contains_key(&edge.from) && tokens.contains(&edge.to),
                    "declarationUsesToken has a dangling endpoint",
                )?;
                used_tokens.insert(edge.to.clone());
            }
            "componentUsesDeclaration" => {
                require(
                    components.contains(&edge.from) && declarations.contains_key(&edge.to),
                    "componentUsesDeclaration has a dangling endpoint",
                )?;
                active_components.insert(edge.from.clone());
                owned_declarations.insert(edge.to.clone());
            }
            "routeUsesComponent" => {
                require(
                    routes.contains_key(&edge.from) && components.contains(&edge.to),
                    "routeUsesComponent has a dangling endpoint",
                )?;
                route_components
                    .get_mut(&edge.from)
                    .expect("validated route initializes topology")
                    .insert(edge.to.clone());
            }
            "islandUsesComponent" => {
                require(
                    islands.contains_key(&edge.from) && components.contains(&edge.to),
                    "islandUsesComponent has a dangling endpoint",
                )?;
                island_components
                    .get_mut(&edge.from)
                    .expect("validated island initializes topology")
                    .insert(edge.to.clone());
            }
            "declarationContributesToPhysicalDeclaration" if graph.schema_version == 2 => {
                require(
                    declarations.contains_key(&edge.from)
                        && physical_declaration_nodes.contains(&edge.to),
                    "semantic physical contribution has a dangling endpoint",
                )?;
                semantic_contributions.insert(edge.from.clone());
                physical_produced.insert(edge.to.clone());
            }
            "syntheticProducerProducesPhysicalDeclaration" if graph.schema_version == 2 => {
                require(
                    producer_nodes.contains(&edge.from)
                        && physical_declaration_nodes.contains(&edge.to),
                    "synthetic physical contribution has a dangling endpoint",
                )?;
                synthetic_contributions.insert(edge.from.clone());
                physical_produced.insert(edge.to.clone());
            }
            "physicalDeclarationBelongsToRule" if graph.schema_version == 2 => {
                require(
                    physical_declaration_nodes.contains(&edge.from)
                        && rule_nodes.get(&edge.to) == Some(&PhysicalRuleRole::Qualified),
                    "physical declaration ownership has a dangling endpoint",
                )?;
                require(
                    edge.to == physical_declaration_rule_id(&edge.from),
                    "physical declaration is linked to the wrong rule",
                )?;
                *physical_owners.entry(edge.from.clone()).or_default() += 1;
            }
            "ruleNestedInRule" if graph.schema_version == 2 => {
                require(
                    rule_nodes.contains_key(&edge.from)
                        && rule_nodes.get(&edge.to) == Some(&PhysicalRuleRole::Group)
                        && edge.from != edge.to,
                    "physical rule nesting has an invalid endpoint or non-group parent",
                )?;
                require(
                    rule_parents
                        .insert(edge.from.clone(), edge.to.clone())
                        .is_none(),
                    "physical rule has multiple parents",
                )?;
            }
            _ => return Err(format!("unknown graph edge kind `{}`", edge.kind)),
        }
    }

    let linked_declarations = style_links.keys().cloned().collect::<BTreeSet<_>>();
    let declaration_ids = declarations.keys().cloned().collect::<BTreeSet<_>>();
    require(
        linked_declarations == declaration_ids
            && style_links.values().all(|owners| owners.len() == 1),
        "semantic declaration style ownership is incomplete",
    )?;
    require(
        owned_declarations == declaration_ids,
        "semantic declaration component ownership is incomplete",
    )?;
    let linked_styles = style_links
        .values()
        .flatten()
        .cloned()
        .collect::<BTreeSet<_>>();
    require(
        linked_styles == style_nodes,
        "manifest style declaration coverage is incomplete",
    )?;
    require(
        used_tokens == tokens,
        "manifest token coverage is incomplete",
    )?;

    if graph.schema_version == 2 {
        require(
            semantic_contributions == declaration_ids,
            "semantic physical coverage is incomplete",
        )?;
        require(
            physical_declaration_nodes
                .iter()
                .all(|id| physical_owners.get(id) == Some(&1)),
            "physical declaration rule ownership is incomplete",
        )?;
        require(
            physical_produced == physical_declaration_nodes,
            "physical declaration producer coverage is incomplete",
        )?;
        require(
            synthetic_contributions == producer_nodes,
            "synthetic producer coverage is incomplete",
        )?;
        validate_rule_parent_acyclic(&rule_parents)?;
    }

    Ok((
        ApplicationTopology {
            components,
            routes,
            islands,
            route_components,
            island_components,
        },
        active_components,
        producer_nodes.contains("producer:theme"),
    ))
}

fn validate_declarations(
    declarations: &[DeclarationDocument],
    style_ids: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>, String> {
    let mut output = BTreeMap::new();
    for declaration in declarations {
        validate_lower_hex(&declaration.style_id, 32, "declaration style ID")?;
        require(
            style_ids.contains(&declaration.style_id),
            "declaration references an unknown style",
        )?;
        let expected = format!("decl:{}:{:08x}", declaration.style_id, declaration.ordinal);
        require(declaration.id == expected, "invalid declaration ID")?;
        require(
            output
                .insert(declaration.id.clone(), declaration.style_id.clone())
                .is_none(),
            "duplicate declaration ID",
        )?;
    }
    Ok(output)
}

fn validate_tokens(tokens: &[TokenDocument]) -> Result<BTreeSet<String>, String> {
    let mut output = BTreeSet::new();
    for token in tokens {
        require(
            matches!(
                token.kind.as_str(),
                "spacing"
                    | "color"
                    | "font-family"
                    | "font-size"
                    | "font-weight"
                    | "line-height"
                    | "letter-spacing"
                    | "radius"
                    | "shadow"
                    | "z-index"
            ),
            "invalid token kind",
        )?;
        validate_lower_hex(&token.token_id, 8, "token ID")?;
        require(
            token.id == format!("token:{}:{}", token.kind, token.token_id),
            "invalid token node ID",
        )?;
        validate_text(&token.name, MAX_PATH_OR_NAME_BYTES, "token name")?;
        require(output.insert(token.id.clone()), "duplicate token node ID")?;
    }
    Ok(output)
}

fn validate_components(components: &[ComponentDocument]) -> Result<BTreeSet<String>, String> {
    let mut output = BTreeSet::new();
    for component in components {
        validate_namespaced_id(&component.id, "component:")?;
        require(
            output.insert(component.id.clone()),
            "duplicate component node ID",
        )?;
    }
    Ok(output)
}

fn validate_routes(routes: &[RouteDocument]) -> Result<BTreeMap<String, String>, String> {
    let mut output = BTreeMap::new();
    let mut paths = BTreeSet::new();
    for route in routes {
        validate_namespaced_id(&route.id, "route:")?;
        validate_text(&route.path, MAX_PATH_OR_NAME_BYTES, "route path")?;
        require(paths.insert(route.path.clone()), "duplicate route path")?;
        require(
            output
                .insert(route.id.clone(), route.path.clone())
                .is_none(),
            "duplicate route node ID",
        )?;
    }
    Ok(output)
}

fn validate_islands(islands: &[IslandDocument]) -> Result<BTreeMap<String, String>, String> {
    let mut output = BTreeMap::new();
    let mut names = BTreeSet::new();
    for island in islands {
        validate_namespaced_id(&island.id, "island:")?;
        validate_text(&island.name, MAX_PATH_OR_NAME_BYTES, "island name")?;
        require(names.insert(island.name.clone()), "duplicate island name")?;
        require(
            output
                .insert(island.id.clone(), island.name.clone())
                .is_none(),
            "duplicate island node ID",
        )?;
    }
    Ok(output)
}

fn validate_producers(producers: &[PhysicalProducerDocument]) -> Result<BTreeSet<String>, String> {
    let mut output = BTreeSet::new();
    for producer in producers {
        require(
            producer.id == "producer:theme" && producer.kind == "theme",
            "unsupported physical producer",
        )?;
        require(
            output.insert(producer.id.clone()),
            "duplicate physical producer ID",
        )?;
    }
    Ok(output)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PhysicalRuleRole {
    Qualified,
    Group,
    Statement,
}

fn validate_physical_rules(
    rules: &[PhysicalRuleDocument],
) -> Result<BTreeMap<String, PhysicalRuleRole>, String> {
    let mut output = BTreeMap::new();
    for rule in rules {
        let suffix = rule
            .id
            .strip_prefix("css-rule:")
            .ok_or_else(|| "invalid physical rule ID".to_owned())?;
        validate_lower_hex(suffix, 8, "physical rule ID")?;
        require(
            u32::from_str_radix(suffix, 16).ok() == Some(rule.ordinal),
            "physical rule ID ordinal mismatch",
        )?;
        let role = match rule.kind.as_str() {
            "qualified" => PhysicalRuleRole::Qualified,
            "media" | "container" | "layer" => PhysicalRuleRole::Group,
            "layer-order" => PhysicalRuleRole::Statement,
            _ => return Err("unsupported physical rule kind".into()),
        };
        require(
            output.insert(rule.id.clone(), role).is_none(),
            "duplicate physical rule ID",
        )?;
    }
    Ok(output)
}

fn validate_physical_declarations(
    declarations: &[PhysicalDeclarationDocument],
    rules: &BTreeMap<String, PhysicalRuleRole>,
) -> Result<BTreeSet<String>, String> {
    let mut output = BTreeSet::new();
    for declaration in declarations {
        let suffix = declaration
            .id
            .strip_prefix("css-decl:")
            .ok_or_else(|| "invalid physical declaration ID".to_owned())?;
        let mut parts = suffix.split(':');
        let rule = parts.next().unwrap_or_default();
        let ordinal = parts.next().unwrap_or_default();
        require(parts.next().is_none(), "invalid physical declaration ID")?;
        validate_lower_hex(rule, 8, "physical declaration rule ID")?;
        validate_lower_hex(ordinal, 8, "physical declaration ordinal")?;
        require(
            u32::from_str_radix(ordinal, 16).ok() == Some(declaration.ordinal),
            "physical declaration ID ordinal mismatch",
        )?;
        require(
            rules.get(&format!("css-rule:{rule}")) == Some(&PhysicalRuleRole::Qualified),
            "physical declaration references an unknown or non-qualified rule",
        )?;
        require(
            output.insert(declaration.id.clone()),
            "duplicate physical declaration ID",
        )?;
    }
    Ok(output)
}

fn validate_rule_parent_acyclic(parents: &BTreeMap<String, String>) -> Result<(), String> {
    let mut complete = BTreeSet::<&str>::new();
    for start in parents.keys().map(String::as_str) {
        if complete.contains(start) {
            continue;
        }
        let mut path = BTreeSet::new();
        let mut visited = Vec::new();
        let mut current = start;
        loop {
            if complete.contains(current) {
                break;
            }
            require(
                path.insert(current),
                "physical rule nesting contains a cycle",
            )?;
            visited.push(current);
            let Some(parent) = parents.get(current) else {
                break;
            };
            current = parent;
        }
        for node in visited {
            complete.insert(node);
        }
    }
    Ok(())
}

fn physical_declaration_rule_id(id: &str) -> String {
    let rule = id
        .strip_prefix("css-decl:")
        .and_then(|suffix| suffix.split(':').next())
        .expect("validated physical declaration ID");
    format!("css-rule:{rule}")
}

fn validate_namespaced_id(id: &str, prefix: &str) -> Result<(), String> {
    let raw = id
        .strip_prefix(prefix)
        .ok_or_else(|| format!("invalid `{prefix}` node ID"))?;
    validate_text(raw, MAX_ID_BYTES, "application node ID")
}

fn validate_lower_hex(value: &str, length: usize, role: &str) -> Result<(), String> {
    require(
        value.len() == length
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')),
        format!("invalid {role}"),
    )
}

fn validate_text(value: &str, maximum: usize, role: &str) -> Result<(), String> {
    require(
        !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control),
        format!("invalid {role}"),
    )
}

fn checked_sum<const N: usize>(counts: [usize; N]) -> Result<usize, String> {
    counts
        .into_iter()
        .try_fold(0_usize, usize::checked_add)
        .ok_or_else(|| "manifest graph item limit exceeded".to_owned())
}

fn checked_add(left: usize, right: usize) -> Result<usize, String> {
    left.checked_add(right)
        .ok_or_else(|| "asset plan item limit exceeded".to_owned())
}

fn validate_plan_length(serialized_length: usize) -> Result<(), String> {
    require(
        serialized_length
            .checked_add(1)
            .is_some_and(|length| length <= MAX_ASSET_BYTES),
        "asset plan exceeds 16 MiB",
    )
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.into())
}

#[cfg(all(test, not(feature = "package-verify")))]
#[path = "../../tests/internal/asset_plan.rs"]
mod tests;
