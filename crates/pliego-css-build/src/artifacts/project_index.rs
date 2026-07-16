use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{AssetPlanBundle, AssetRuleSelection, build_asset_plan, sha256_hex};

const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_ITEMS: usize = 65_535;
const MAX_PATH_BYTES: usize = 4 * 1024;

/// One exact UTF-8 source document represented by a project index.
#[derive(Clone, Copy, Debug)]
pub struct ProjectIndexDocument<'a> {
    path: &'a str,
    bytes: &'a [u8],
}

impl<'a> ProjectIndexDocument<'a> {
    /// Creates one project-index document from a portable logical path and exact bytes.
    #[must_use]
    pub const fn new(path: &'a str, bytes: &'a [u8]) -> Self {
        Self { path, bytes }
    }
}

/// Builds a deterministic schema-1 or schema-2 project index over schema-5 bundle manifests.
///
/// The rule-selection mode determines the compatible index and Asset Plan schema. The index binds
/// exact source documents, CSS, manifests, and the derived asset plan. Every source
/// site maps to semantic declarations, tokens, adapter-attested components, and bundle-qualified
/// physical declarations. Consumers can therefore navigate the complete source-to-output chain
/// without inspecting repository layout or reconstructing compiler ownership heuristically.
///
/// # Errors
///
/// Returns an error for incompatible or malformed manifests, unsafe logical paths, incomplete
/// source coverage, invalid UTF-8 ranges, integrity drift, or defensive item-limit violations.
#[allow(clippy::too_many_lines)]
pub fn build_project_index(
    bundles: &[AssetPlanBundle<'_>],
    documents: &[ProjectIndexDocument<'_>],
    rule_selection: AssetRuleSelection,
) -> Result<Vec<u8>, String> {
    require(
        !bundles.is_empty(),
        "project index requires at least one bundle",
    )?;
    require(
        bundles.len() <= MAX_ITEMS,
        "project index bundle limit exceeded",
    )?;
    require(
        documents.len() <= MAX_ITEMS,
        "project index document limit exceeded",
    )?;

    let asset_plan = build_asset_plan(bundles, rule_selection)?;
    let mut prepared_documents = prepare_documents(documents)?;
    let mut ordered_bundles = bundles.iter().collect::<Vec<_>>();
    ordered_bundles.sort_by(|left, right| {
        right
            .emits_theme
            .cmp(&left.emits_theme)
            .then_with(|| left.id.cmp(right.id))
    });

    let mut common_identity = None;
    let mut sites = BTreeMap::<SiteKey, SiteAccumulator>::new();
    let mut prepared_bundles = Vec::with_capacity(ordered_bundles.len());
    for bundle in ordered_bundles {
        let manifest: ProjectManifest = serde_json::from_slice(bundle.manifest)
            .map_err(|error| format!("bundle `{}` has an invalid manifest: {error}", bundle.id))?;
        validate_project_manifest(&manifest)
            .map_err(|error| format!("bundle `{}`: {error}", bundle.id))?;
        let identity = BuildIdentity::from_manifest(&manifest)?;
        if let Some(common) = &common_identity {
            require(
                common == &identity,
                format!(
                    "bundle `{}` has an incompatible project identity",
                    bundle.id
                ),
            )?;
        } else {
            common_identity = Some(identity);
        }

        let links = GraphLinks::from_graph(&manifest.graph)?;
        let mut bundle_site_keys = BTreeSet::new();
        for style in &manifest.styles {
            let declarations = links
                .declarations_by_style
                .get(&style.style_id)
                .ok_or_else(|| {
                    format!(
                        "bundle `{}` style {} has no semantic declarations",
                        bundle.id, style.style_id
                    )
                })?;
            for origin in &style.origins {
                let file = origin
                    .file
                    .as_deref()
                    .ok_or_else(|| "project-index origin lacks a source file".to_owned())?;
                let byte_start = origin
                    .byte_start
                    .ok_or_else(|| "project-index origin lacks a start offset".to_owned())?;
                let byte_end = origin
                    .byte_end
                    .ok_or_else(|| "project-index origin lacks an end offset".to_owned())?;
                let document = prepared_documents.get(file).ok_or_else(|| {
                    format!("project-index origin references unknown document `{file}`")
                })?;
                require(
                    byte_start < byte_end && byte_end <= document.bytes,
                    format!(
                        "project-index origin `{file}` range {byte_start}..{byte_end} is outside {} bytes",
                        document.bytes
                    ),
                )?;

                let key = SiteKey {
                    document_id: document.id.clone(),
                    path: file.to_owned(),
                    byte_start,
                    byte_end,
                    macro_kind: origin.macro_kind.clone(),
                    reason: origin.reason.clone(),
                    source: origin.source.clone(),
                    style_id: style.style_id.clone(),
                    class_name: style.class_name.clone(),
                };
                let accumulator = sites.entry(key.clone()).or_default();
                accumulator.bundle_ids.insert(bundle.id.to_owned());
                accumulator
                    .declaration_ids
                    .extend(declarations.iter().cloned());
                for declaration in declarations {
                    accumulator.token_ids.extend(
                        links
                            .tokens_by_declaration
                            .get(declaration)
                            .into_iter()
                            .flatten()
                            .cloned(),
                    );
                    accumulator.component_ids.extend(
                        links
                            .components_by_declaration
                            .get(declaration)
                            .into_iter()
                            .flatten()
                            .cloned(),
                    );
                    let physical = links.physical_by_declaration.get(declaration).ok_or_else(
                        || {
                            format!(
                                "bundle `{}` declaration `{declaration}` lacks a physical mapping",
                                bundle.id
                            )
                        },
                    )?;
                    accumulator
                        .physical_declarations
                        .extend(physical.iter().map(|id| PhysicalReference {
                            bundle_id: bundle.id.to_owned(),
                            id: id.clone(),
                        }));
                }
                require(
                    !accumulator.component_ids.is_empty(),
                    format!("project-index origin `{file}` has no owning component"),
                )?;
                bundle_site_keys.insert(key);
            }
        }
        prepared_bundles.push(PreparedBundle {
            id: bundle.id.to_owned(),
            css_file: format!("{}.css", bundle.id),
            manifest_file: format!("{}.manifest.json", bundle.id),
            emits_theme: bundle.emits_theme,
            css_bytes: bundle.css.len(),
            css_sha256: sha256_hex(bundle.css),
            manifest_bytes: bundle.manifest.len(),
            manifest_sha256: sha256_hex(bundle.manifest),
            site_keys: bundle_site_keys,
        });
    }

    let identity = common_identity
        .ok_or_else(|| "project index failed to establish a common identity".to_owned())?;
    let projected_sites = sites
        .into_iter()
        .map(|(key, accumulator)| ProjectSite::from_parts(key, accumulator))
        .collect::<Vec<_>>();
    let site_ids = projected_sites
        .iter()
        .map(|site| (site.key.clone(), site.id.clone()))
        .collect::<BTreeMap<_, _>>();
    for site in &projected_sites {
        let document = prepared_documents
            .get_mut(&site.path)
            .ok_or_else(|| "validated project site lost its source document".to_owned())?;
        document.site_ids.push(site.id.clone());
    }
    let output_bundles = prepared_bundles
        .into_iter()
        .map(|bundle| -> Result<ProjectBundle, String> {
            let bundle_site_ids = bundle
                .site_keys
                .iter()
                .map(|key| {
                    site_ids
                        .get(key)
                        .cloned()
                        .ok_or_else(|| "validated bundle site was not projected".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ProjectBundle {
                id: bundle.id,
                css_file: bundle.css_file,
                manifest_file: bundle.manifest_file,
                emits_theme: bundle.emits_theme,
                css_bytes: bundle.css_bytes,
                css_sha256: bundle.css_sha256,
                manifest_bytes: bundle.manifest_bytes,
                manifest_sha256: bundle.manifest_sha256,
                site_ids: bundle_site_ids,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let output_documents = prepared_documents
        .into_values()
        .map(|mut document| {
            document.site_ids.sort();
            ProjectDocument {
                id: document.id,
                path: document.path,
                bytes: document.bytes,
                sha256: document.sha256,
                site_ids: document.site_ids,
            }
        })
        .collect::<Vec<_>>();

    let item_count = output_bundles
        .len()
        .checked_add(output_documents.len())
        .and_then(|count| count.checked_add(projected_sites.len()))
        .ok_or_else(|| "project index item limit exceeded".to_owned())?;
    require(item_count <= MAX_ITEMS, "project index item limit exceeded")?;

    let index = ProjectIndex {
        schema_version: rule_selection.artifact_schema_version(),
        source_site_id_format_version: 1,
        manifest_schema_version: identity.manifest_schema_version,
        graph_schema_version: identity.graph_schema_version,
        declaration_id_format_version: identity.declaration_id_format_version,
        physical_rule_id_format_version: identity.physical_rule_id_format_version,
        physical_declaration_id_format_version: identity.physical_declaration_id_format_version,
        origin_coverage: identity.origin_coverage,
        application_coverage: identity.application_coverage,
        physical_coverage: identity.physical_coverage,
        style_id_format_version: identity.style_id_format_version,
        class_name_format_version: identity.class_name_format_version,
        theme_id_format_version: identity.theme_id_format_version,
        theme_id: identity.theme_id,
        targets: identity.targets,
        format: identity.format,
        rule_selection,
        asset_plan_file: "pliego.assets.json",
        asset_plan_bytes: asset_plan.len(),
        asset_plan_sha256: sha256_hex(&asset_plan),
        documents: output_documents,
        bundles: output_bundles,
        sites: projected_sites,
    };
    let mut output = serde_json::to_vec_pretty(&index)
        .map_err(|error| format!("cannot serialize project index: {error}"))?;
    output.push(b'\n');
    Ok(output)
}

fn prepare_documents(
    documents: &[ProjectIndexDocument<'_>],
) -> Result<BTreeMap<String, PreparedDocument>, String> {
    let mut prepared = BTreeMap::new();
    let mut portable_keys = BTreeSet::new();
    for document in documents {
        validate_portable_path(document.path)?;
        require(
            document.bytes.len() <= MAX_SOURCE_BYTES,
            format!("project document `{}` exceeds 16 MiB", document.path),
        )?;
        std::str::from_utf8(document.bytes).map_err(|error| {
            format!("project document `{}` is not UTF-8: {error}", document.path)
        })?;
        require(
            portable_keys.insert(document.path.to_lowercase()),
            format!(
                "project document path `{}` collides case-insensitively with another document",
                document.path
            ),
        )?;
        let id = document_id(document.path);
        let item = PreparedDocument {
            id,
            path: document.path.to_owned(),
            bytes: document.bytes.len(),
            sha256: sha256_hex(document.bytes),
            site_ids: Vec::new(),
        };
        require(
            prepared.insert(document.path.to_owned(), item).is_none(),
            format!("duplicate project document `{}`", document.path),
        )?;
    }
    Ok(prepared)
}

fn validate_portable_path(path: &str) -> Result<(), String> {
    let valid = !path.is_empty()
        && path.len() <= MAX_PATH_BYTES
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..");
    require(
        valid,
        format!("unsafe project document path `{path}`; use a normalized relative UTF-8 path"),
    )
}

fn document_id(path: &str) -> String {
    let mut identity = b"pliego-project-document-v1\0".to_vec();
    push_identity_part(&mut identity, path.as_bytes());
    format!("document:{}", sha256_hex(&identity))
}

fn site_id(key: &SiteKey) -> String {
    let mut identity = b"pliego-project-site-v1\0".to_vec();
    for part in [
        key.path.as_bytes(),
        key.macro_kind.as_bytes(),
        key.reason.as_bytes(),
        key.source.as_bytes(),
        key.style_id.as_bytes(),
    ] {
        push_identity_part(&mut identity, part);
    }
    identity.extend_from_slice(
        &u64::try_from(key.byte_start)
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    identity.extend_from_slice(
        &u64::try_from(key.byte_end)
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    format!("site:{}", sha256_hex(&identity))
}

fn push_identity_part(buffer: &mut Vec<u8>, part: &[u8]) {
    buffer.extend_from_slice(&u64::try_from(part.len()).unwrap_or(u64::MAX).to_be_bytes());
    buffer.extend_from_slice(part);
}

fn validate_project_manifest(manifest: &ProjectManifest) -> Result<(), String> {
    require(
        manifest.schema_version == 5 && manifest.graph.schema_version == 2,
        "project index requires manifest schema 5 and graph schema 2",
    )?;
    require(
        manifest.graph.declaration_id_format_version == 1
            && manifest.graph.physical_rule_id_format_version == Some(1)
            && manifest.graph.physical_declaration_id_format_version == Some(1),
        "project index requires declaration and physical ID format version 1",
    )?;
    require(
        manifest.graph.origin_coverage == "compiler-verified-complete"
            && manifest.graph.application_coverage == "adapter-attested-complete"
            && manifest.graph.physical_coverage.as_deref() == Some("compiler-verified-complete"),
        "project index requires complete source, application, and physical coverage",
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BuildIdentity {
    manifest_schema_version: u8,
    graph_schema_version: u8,
    declaration_id_format_version: u8,
    physical_rule_id_format_version: u8,
    physical_declaration_id_format_version: u8,
    origin_coverage: String,
    application_coverage: String,
    physical_coverage: String,
    style_id_format_version: u16,
    class_name_format_version: u16,
    theme_id_format_version: u16,
    theme_id: String,
    targets: String,
    format: String,
}

impl BuildIdentity {
    fn from_manifest(manifest: &ProjectManifest) -> Result<Self, String> {
        Ok(Self {
            manifest_schema_version: manifest.schema_version,
            graph_schema_version: manifest.graph.schema_version,
            declaration_id_format_version: manifest.graph.declaration_id_format_version,
            physical_rule_id_format_version: manifest
                .graph
                .physical_rule_id_format_version
                .ok_or_else(|| "project manifest lacks a physical rule ID version".to_owned())?,
            physical_declaration_id_format_version: manifest
                .graph
                .physical_declaration_id_format_version
                .ok_or_else(|| {
                    "project manifest lacks a physical declaration ID version".to_owned()
                })?,
            origin_coverage: manifest.graph.origin_coverage.clone(),
            application_coverage: manifest.graph.application_coverage.clone(),
            physical_coverage: manifest
                .graph
                .physical_coverage
                .clone()
                .ok_or_else(|| "project manifest lacks physical coverage".to_owned())?,
            style_id_format_version: manifest.style_id_format_version,
            class_name_format_version: manifest.class_name_format_version,
            theme_id_format_version: manifest.theme_id_format_version,
            theme_id: manifest.theme_id.clone(),
            targets: manifest.targets.clone(),
            format: manifest.format.clone(),
        })
    }
}

#[derive(Default)]
struct GraphLinks {
    declarations_by_style: BTreeMap<String, BTreeSet<String>>,
    tokens_by_declaration: BTreeMap<String, BTreeSet<String>>,
    components_by_declaration: BTreeMap<String, BTreeSet<String>>,
    physical_by_declaration: BTreeMap<String, BTreeSet<String>>,
}

impl GraphLinks {
    fn from_graph(graph: &ProjectGraph) -> Result<Self, String> {
        let mut links = Self::default();
        for declaration in &graph.declarations {
            require(
                links
                    .declarations_by_style
                    .entry(declaration.style_id.clone())
                    .or_default()
                    .insert(declaration.id.clone()),
                format!("duplicate project declaration `{}`", declaration.id),
            )?;
        }
        for edge in &graph.edges {
            match edge.kind.as_str() {
                "declarationUsesToken" => {
                    links
                        .tokens_by_declaration
                        .entry(edge.from.clone())
                        .or_default()
                        .insert(edge.to.clone());
                }
                "componentUsesDeclaration" => {
                    links
                        .components_by_declaration
                        .entry(edge.to.clone())
                        .or_default()
                        .insert(edge.from.clone());
                }
                "declarationContributesToPhysicalDeclaration" => {
                    links
                        .physical_by_declaration
                        .entry(edge.from.clone())
                        .or_default()
                        .insert(edge.to.clone());
                }
                _ => {}
            }
        }
        Ok(links)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectManifest {
    schema_version: u8,
    style_id_format_version: u16,
    class_name_format_version: u16,
    theme_id_format_version: u16,
    theme_id: String,
    targets: String,
    format: String,
    styles: Vec<ProjectManifestStyle>,
    graph: ProjectGraph,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectManifestStyle {
    style_id: String,
    class_name: String,
    origins: Vec<ProjectOrigin>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectOrigin {
    source: String,
    file: Option<String>,
    byte_start: Option<usize>,
    byte_end: Option<usize>,
    macro_kind: String,
    reason: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectGraph {
    schema_version: u8,
    declaration_id_format_version: u8,
    physical_rule_id_format_version: Option<u8>,
    physical_declaration_id_format_version: Option<u8>,
    origin_coverage: String,
    application_coverage: String,
    physical_coverage: Option<String>,
    declarations: Vec<ProjectDeclaration>,
    edges: Vec<ProjectEdge>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectDeclaration {
    id: String,
    style_id: String,
}

#[derive(Deserialize)]
struct ProjectEdge {
    kind: String,
    from: String,
    to: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SiteKey {
    document_id: String,
    path: String,
    byte_start: usize,
    byte_end: usize,
    macro_kind: String,
    reason: String,
    source: String,
    style_id: String,
    class_name: String,
}

#[derive(Default)]
struct SiteAccumulator {
    bundle_ids: BTreeSet<String>,
    declaration_ids: BTreeSet<String>,
    token_ids: BTreeSet<String>,
    component_ids: BTreeSet<String>,
    physical_declarations: BTreeSet<PhysicalReference>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalReference {
    bundle_id: String,
    id: String,
}

struct PreparedDocument {
    id: String,
    path: String,
    bytes: usize,
    sha256: String,
    site_ids: Vec<String>,
}

struct PreparedBundle {
    id: String,
    css_file: String,
    manifest_file: String,
    emits_theme: bool,
    css_bytes: usize,
    css_sha256: String,
    manifest_bytes: usize,
    manifest_sha256: String,
    site_keys: BTreeSet<SiteKey>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectIndex {
    schema_version: u8,
    source_site_id_format_version: u8,
    manifest_schema_version: u8,
    graph_schema_version: u8,
    declaration_id_format_version: u8,
    physical_rule_id_format_version: u8,
    physical_declaration_id_format_version: u8,
    origin_coverage: String,
    application_coverage: String,
    physical_coverage: String,
    style_id_format_version: u16,
    class_name_format_version: u16,
    theme_id_format_version: u16,
    theme_id: String,
    targets: String,
    format: String,
    rule_selection: AssetRuleSelection,
    asset_plan_file: &'static str,
    asset_plan_bytes: usize,
    asset_plan_sha256: String,
    documents: Vec<ProjectDocument>,
    bundles: Vec<ProjectBundle>,
    sites: Vec<ProjectSite>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectDocument {
    id: String,
    path: String,
    bytes: usize,
    sha256: String,
    site_ids: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectBundle {
    id: String,
    css_file: String,
    manifest_file: String,
    emits_theme: bool,
    css_bytes: usize,
    css_sha256: String,
    manifest_bytes: usize,
    manifest_sha256: String,
    site_ids: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSite {
    #[serde(skip)]
    key: SiteKey,
    id: String,
    document_id: String,
    path: String,
    byte_start: usize,
    byte_end: usize,
    macro_kind: String,
    reason: String,
    source: String,
    style_id: String,
    class_name: String,
    bundle_ids: Vec<String>,
    declaration_ids: Vec<String>,
    token_ids: Vec<String>,
    component_ids: Vec<String>,
    physical_declarations: Vec<PhysicalReference>,
}

impl ProjectSite {
    fn from_parts(key: SiteKey, accumulator: SiteAccumulator) -> Self {
        Self {
            id: site_id(&key),
            document_id: key.document_id.clone(),
            path: key.path.clone(),
            byte_start: key.byte_start,
            byte_end: key.byte_end,
            macro_kind: key.macro_kind.clone(),
            reason: key.reason.clone(),
            source: key.source.clone(),
            style_id: key.style_id.clone(),
            class_name: key.class_name.clone(),
            key,
            bundle_ids: accumulator.bundle_ids.into_iter().collect(),
            declaration_ids: accumulator.declaration_ids.into_iter().collect(),
            token_ids: accumulator.token_ids.into_iter().collect(),
            component_ids: accumulator.component_ids.into_iter().collect(),
            physical_declarations: accumulator.physical_declarations.into_iter().collect(),
        }
    }
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
