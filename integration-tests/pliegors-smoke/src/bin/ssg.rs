//! Builds the two-route PliegoRS SSG fixture with compiler-produced CSS assets.

use pliego_resume::{RUNTIME_PATH, runtime_bytes};
use pliego_ssg::{Asset, Head, Page, ProductRegistry, ProductRoute, Site};
use pliegocss_pliegors_smoke::product::{
    HOME_ROUTE_ID, VISIT_ROUTE_ID, application_registry,
};
use pliegocss_pliegors_smoke::site::{home, visit};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

const MAX_ASSET_BYTES: usize = 16 * 1024 * 1024;
const MAX_ITEMS: usize = 65_535;
const CLIENT_BINDGEN_DIR: &str = "target/pliegocss-pliegors-client/pkg";
const CLIENT_BOOTSTRAP_PATH: &str = "assets/pliegocss-pliegors-client-bootstrap.js";
const CLIENT_JS_FILE: &str = "pliegocss_pliegors_client.js";
const CLIENT_JS_PATH: &str = "assets/pliegocss_pliegors_client.js";
const CLIENT_WASM_FILE: &str = "pliegocss_pliegors_client_bg.wasm";
const CLIENT_WASM_PATH: &str = "assets/pliegocss_pliegors_client_bg.wasm";
const CLIENT_BOOTSTRAP: &[u8] =
    b"import init from './pliegocss_pliegors_client.js';\nawait init();\n";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetPlan {
    schema_version: u8,
    manifest_schema_version: u8,
    graph_schema_version: u8,
    rule_selection: String,
    origin_coverage: String,
    application_coverage: String,
    style_id_format_version: u16,
    class_name_format_version: u16,
    theme_id_format_version: u16,
    theme_id: String,
    targets: String,
    format: String,
    bundles: Vec<Bundle>,
    routes: Vec<RouteAssets>,
    islands: Vec<IslandAssets>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Bundle {
    id: String,
    css_file: String,
    manifest_file: String,
    emits_theme: bool,
    css_bytes: usize,
    css_sha256: String,
    manifest_bytes: usize,
    manifest_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteAssets {
    id: String,
    path: String,
    bundles: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IslandAssets {
    id: String,
    name: String,
    bundles: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
    rule_selection: String,
    asset_plan_file: String,
    asset_plan_bytes: usize,
    asset_plan_sha256: String,
    documents: Vec<ProjectDocument>,
    bundles: Vec<ProjectBundle>,
    sites: Vec<ProjectSite>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectDocument {
    id: String,
    path: String,
    bytes: usize,
    sha256: String,
    site_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectSite {
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PhysicalReference {
    bundle_id: String,
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema_version: u8,
    style_id_format_version: u16,
    class_name_format_version: u16,
    theme_id_format_version: u16,
    theme_id: String,
    targets: String,
    format: String,
    css_sha256: String,
    css_bytes: usize,
    styles: Vec<serde_json::Value>,
    graph: ManifestGraph,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestGraph {
    schema_version: u8,
    declaration_id_format_version: u8,
    physical_rule_id_format_version: u8,
    physical_declaration_id_format_version: u8,
    origin_coverage: String,
    application_coverage: String,
    physical_coverage: String,
    declarations: Vec<serde_json::Value>,
    tokens: Vec<serde_json::Value>,
    components: Vec<serde_json::Value>,
    routes: Vec<serde_json::Value>,
    islands: Vec<serde_json::Value>,
    synthetic_producers: Vec<serde_json::Value>,
    physical_rules: Vec<serde_json::Value>,
    physical_declarations: Vec<serde_json::Value>,
    edges: Vec<serde_json::Value>,
}

fn required_path(arguments: &mut impl Iterator<Item = std::ffi::OsString>, role: &str) -> PathBuf {
    arguments
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("missing {role} path"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

fn valid_bundle_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes.last() != Some(&b'-')
        && !id.contains("--")
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !matches!(
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
        )
}

fn single_file_name(file: &str) -> bool {
    let mut components = Path::new(file).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

fn portable_source_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4 * 1024
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn strictly_sorted(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn prefixed_hash(value: &str, prefix: &str) -> bool {
    value
        .strip_prefix(prefix)
        .is_some_and(|digest| lower_hex(digest, 64))
}

fn physical_declaration_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("css-decl:") else {
        return false;
    };
    let mut parts = rest.split(':');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(rule), Some(declaration), None)
            if lower_hex(rule, 8) && lower_hex(declaration, 8)
    )
}

fn push_identity_part(identity: &mut Vec<u8>, value: &str) {
    identity.extend_from_slice(&(value.len() as u64).to_be_bytes());
    identity.extend_from_slice(value.as_bytes());
}

fn project_document_id(path: &str) -> String {
    let mut identity = b"pliego-project-document-v1\0".to_vec();
    push_identity_part(&mut identity, path);
    format!("document:{}", sha256_hex(&identity))
}

fn project_site_id(site: &ProjectSite) -> String {
    let mut identity = b"pliego-project-site-v1\0".to_vec();
    for value in [
        &site.path,
        &site.macro_kind,
        &site.reason,
        &site.source,
        &site.style_id,
    ] {
        push_identity_part(&mut identity, value);
    }
    identity.extend_from_slice(&(site.byte_start as u64).to_be_bytes());
    identity.extend_from_slice(&(site.byte_end as u64).to_be_bytes());
    format!("site:{}", sha256_hex(&identity))
}

fn read_bounded(path: &Path, role: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_ASSET_BYTES as u64
    {
        return Err(format!("invalid or oversized {role}").into());
    }
    Ok(std::fs::read(path)?)
}

fn verify_file(
    base: &Path,
    file: &str,
    expected_bytes: usize,
    expected_sha256: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if expected_bytes > MAX_ASSET_BYTES || !single_file_name(file) {
        return Err(format!("unsafe or oversized asset-plan filename `{file}`").into());
    }
    let bytes = read_bounded(&base.join(file), "asset-plan input")?;
    if bytes.len() != expected_bytes || sha256_hex(&bytes) != expected_sha256 {
        return Err(format!("asset plan integrity mismatch for `{file}`").into());
    }
    Ok(bytes)
}

fn validate_references(
    references: &[String],
    bundle_positions: &BTreeMap<&str, usize>,
    item_count: &mut usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut previous = None;
    for reference in references {
        let position = *bundle_positions
            .get(reference.as_str())
            .ok_or("asset plan root references an unknown bundle")?;
        if previous.is_some_and(|previous| previous >= position) {
            return Err("asset plan has duplicate or non-canonical bundle references".into());
        }
        previous = Some(position);
        *item_count = item_count
            .checked_add(1)
            .filter(|count| *count <= MAX_ITEMS)
            .ok_or("asset plan item limit exceeded")?;
    }
    Ok(())
}

fn validate_plan(plan: &AssetPlan) -> Result<(), Box<dyn std::error::Error>> {
    if plan.schema_version != 1
        || plan.manifest_schema_version != 5
        || plan.graph_schema_version != 2
        || plan.rule_selection != "reachable-style-ids"
        || plan.origin_coverage != "compiler-verified-complete"
        || plan.application_coverage != "adapter-attested-complete"
        || plan.style_id_format_version != 2
        || plan.class_name_format_version != 1
        || plan.theme_id_format_version != 1
        || !lower_hex(&plan.theme_id, 32)
        || plan.targets != "modern"
        || plan.format != "minified"
    {
        return Err("unsupported PliegoCSS asset plan contract".into());
    }
    if plan.bundles.is_empty()
        || plan.bundles.len() > MAX_ITEMS
        || plan.routes.len() > MAX_ITEMS
        || plan.islands.len() > MAX_ITEMS
    {
        return Err("asset plan collection limit exceeded".into());
    }

    let mut bundle_ids = BTreeSet::new();
    let mut theme_count = 0_usize;
    for bundle in &plan.bundles {
        if !valid_bundle_id(&bundle.id)
            || !bundle_ids.insert(bundle.id.as_str())
            || !single_file_name(&bundle.css_file)
            || !single_file_name(&bundle.manifest_file)
            || bundle.css_file != format!("{}.css", bundle.id)
            || bundle.manifest_file != format!("{}.manifest.json", bundle.id)
            || bundle.css_bytes > MAX_ASSET_BYTES
            || bundle.manifest_bytes > MAX_ASSET_BYTES
            || !lower_hex(&bundle.css_sha256, 64)
            || !lower_hex(&bundle.manifest_sha256, 64)
        {
            return Err(
                format!("invalid derived filename or bundle record `{}`", bundle.id).into(),
            );
        }
        theme_count += usize::from(bundle.emits_theme);
    }
    if theme_count > 1 {
        return Err("asset plan has ambiguous theme ownership".into());
    }
    let mut canonical_bundles = plan.bundles.iter().collect::<Vec<_>>();
    canonical_bundles.sort_by(|left, right| {
        right
            .emits_theme
            .cmp(&left.emits_theme)
            .then_with(|| left.id.cmp(&right.id))
    });
    if !canonical_bundles
        .iter()
        .zip(&plan.bundles)
        .all(|(expected, actual)| expected.id == actual.id)
    {
        return Err("asset plan bundle order is non-canonical".into());
    }

    let bundle_positions = plan
        .bundles
        .iter()
        .enumerate()
        .map(|(index, bundle)| (bundle.id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut item_count = plan
        .bundles
        .len()
        .checked_add(plan.routes.len())
        .and_then(|count| count.checked_add(plan.islands.len()))
        .filter(|count| *count <= MAX_ITEMS)
        .ok_or("asset plan item limit exceeded")?;

    let mut route_ids = BTreeSet::new();
    let mut route_paths = BTreeSet::new();
    let mut previous_route = None::<&str>;
    for route in &plan.routes {
        if !route.id.starts_with("route:")
            || !valid_text(&route.id["route:".len()..], 256)
            || !valid_text(&route.path, 4_096)
            || !route_ids.insert(route.id.as_str())
            || !route_paths.insert(route.path.as_str())
            || previous_route.is_some_and(|previous| previous >= route.id.as_str())
        {
            return Err("invalid, duplicate, or non-canonical asset-plan route".into());
        }
        previous_route = Some(&route.id);
        validate_references(&route.bundles, &bundle_positions, &mut item_count)?;
    }

    let mut island_ids = BTreeSet::new();
    let mut island_names = BTreeSet::new();
    let mut previous_island = None::<&str>;
    for island in &plan.islands {
        if !island.id.starts_with("island:")
            || !valid_text(&island.id["island:".len()..], 256)
            || !valid_text(&island.name, 4_096)
            || !island_ids.insert(island.id.as_str())
            || !island_names.insert(island.name.as_str())
            || previous_island.is_some_and(|previous| previous >= island.id.as_str())
        {
            return Err("invalid, duplicate, or non-canonical asset-plan island".into());
        }
        previous_island = Some(&island.id);
        validate_references(&island.bundles, &bundle_positions, &mut item_count)?;
    }
    Ok(())
}

fn validate_project_index(
    index: &ProjectIndex,
    plan: &AssetPlan,
    plan_bytes: &[u8],
    source_root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if index.schema_version != 1
        || index.source_site_id_format_version != 1
        || index.manifest_schema_version != plan.manifest_schema_version
        || index.graph_schema_version != plan.graph_schema_version
        || index.declaration_id_format_version != 1
        || index.physical_rule_id_format_version != 1
        || index.physical_declaration_id_format_version != 1
        || index.origin_coverage != plan.origin_coverage
        || index.application_coverage != plan.application_coverage
        || index.physical_coverage != "compiler-verified-complete"
        || index.style_id_format_version != plan.style_id_format_version
        || index.class_name_format_version != plan.class_name_format_version
        || index.theme_id_format_version != plan.theme_id_format_version
        || index.theme_id != plan.theme_id
        || index.targets != plan.targets
        || index.format != plan.format
        || index.rule_selection != plan.rule_selection
        || index.asset_plan_file != "pliego.assets.json"
        || index.asset_plan_bytes != plan_bytes.len()
        || index.asset_plan_sha256 != sha256_hex(plan_bytes)
    {
        return Err("unsupported or detached PliegoCSS project index".into());
    }

    let item_count = index
        .documents
        .len()
        .checked_add(index.bundles.len())
        .and_then(|count| count.checked_add(index.sites.len()))
        .filter(|count| *count <= MAX_ITEMS)
        .ok_or("project index item limit exceeded")?;
    if item_count == 0
        || index.documents.is_empty()
        || index.bundles.len() != plan.bundles.len()
    {
        return Err("project index collection contract drifted".into());
    }

    let canonical_source_root = std::fs::canonicalize(source_root)?;
    let mut document_ids = BTreeSet::new();
    let mut document_paths = BTreeSet::new();
    let mut source_bytes = BTreeMap::new();
    let mut previous_path = None::<&str>;
    for document in &index.documents {
        if !portable_source_path(&document.path)
            || !prefixed_hash(&document.id, "document:")
            || document.id != project_document_id(&document.path)
            || !lower_hex(&document.sha256, 64)
            || document.bytes > MAX_ASSET_BYTES
            || !strictly_sorted(&document.site_ids)
            || !document_ids.insert(document.id.as_str())
            || !document_paths.insert(document.path.to_lowercase())
            || previous_path.is_some_and(|previous| previous >= document.path.as_str())
        {
            return Err("invalid, duplicate, or non-canonical project document".into());
        }
        previous_path = Some(&document.path);
        let source_path = std::fs::canonicalize(source_root.join(&document.path))?;
        if !source_path.starts_with(&canonical_source_root) {
            return Err("project document escaped the supplied source root".into());
        }
        let bytes = read_bounded(&source_path, "project source document")?;
        if bytes.len() != document.bytes || sha256_hex(&bytes) != document.sha256 {
            return Err(format!("project source snapshot drifted for `{}`", document.path).into());
        }
        std::str::from_utf8(&bytes)?;
        source_bytes.insert(document.id.as_str(), bytes);
    }

    let mut index_bundles = BTreeMap::new();
    for (bundle, expected) in index.bundles.iter().zip(&plan.bundles) {
        if bundle.id != expected.id
            || bundle.css_file != expected.css_file
            || bundle.manifest_file != expected.manifest_file
            || bundle.emits_theme != expected.emits_theme
            || bundle.css_bytes != expected.css_bytes
            || bundle.css_sha256 != expected.css_sha256
            || bundle.manifest_bytes != expected.manifest_bytes
            || bundle.manifest_sha256 != expected.manifest_sha256
            || !strictly_sorted(&bundle.site_ids)
            || index_bundles.insert(bundle.id.as_str(), bundle).is_some()
        {
            return Err(format!("project-index bundle drifted for `{}`", expected.id).into());
        }
    }

    let mut canonical_sites = index.sites.iter().collect::<Vec<_>>();
    canonical_sites.sort_by(|left, right| {
        left.document_id
            .cmp(&right.document_id)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.byte_start.cmp(&right.byte_start))
            .then_with(|| left.byte_end.cmp(&right.byte_end))
            .then_with(|| left.macro_kind.cmp(&right.macro_kind))
            .then_with(|| left.reason.cmp(&right.reason))
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.style_id.cmp(&right.style_id))
            .then_with(|| left.class_name.cmp(&right.class_name))
    });
    if !canonical_sites
        .iter()
        .zip(&index.sites)
        .all(|(expected, actual)| expected.id == actual.id)
    {
        return Err("project-index site order is non-canonical".into());
    }

    let documents_by_id = index
        .documents
        .iter()
        .map(|document| (document.id.as_str(), document))
        .collect::<BTreeMap<_, _>>();
    let mut site_ids = BTreeSet::new();
    let mut document_sites = BTreeMap::<&str, BTreeSet<String>>::new();
    let mut bundle_sites = BTreeMap::<&str, BTreeSet<String>>::new();
    for site in &index.sites {
        let document = documents_by_id
            .get(site.document_id.as_str())
            .ok_or("project site references an unknown document")?;
        let bytes = source_bytes
            .get(site.document_id.as_str())
            .ok_or("verified project source inventory is incomplete")?;
        let source = std::str::from_utf8(bytes)?;
        if !prefixed_hash(&site.id, "site:")
            || site.id != project_site_id(site)
            || !site_ids.insert(site.id.as_str())
            || site.path != document.path
            || site.byte_start >= site.byte_end
            || site.byte_end > bytes.len()
            || !source.is_char_boundary(site.byte_start)
            || !source.is_char_boundary(site.byte_end)
            || !valid_text(&site.macro_kind, 64)
            || !valid_text(&site.reason, 4_096)
            || !valid_text(&site.source, MAX_ASSET_BYTES)
            || !valid_text(&site.style_id, 4_096)
            || !valid_text(&site.class_name, 4_096)
            || site.bundle_ids.is_empty()
            || site.declaration_ids.is_empty()
            || site.component_ids.is_empty()
            || site.physical_declarations.is_empty()
            || !strictly_sorted(&site.bundle_ids)
            || !strictly_sorted(&site.declaration_ids)
            || !strictly_sorted(&site.token_ids)
            || !strictly_sorted(&site.component_ids)
        {
            return Err(format!("invalid project source site `{}`", site.id).into());
        }
        document_sites
            .entry(site.document_id.as_str())
            .or_default()
            .insert(site.id.clone());
        for bundle_id in &site.bundle_ids {
            if !index_bundles.contains_key(bundle_id.as_str()) {
                return Err("project site references an unknown bundle".into());
            }
            bundle_sites
                .entry(bundle_id.as_str())
                .or_default()
                .insert(site.id.clone());
        }
        for pair in site.physical_declarations.windows(2) {
            if (pair[0].bundle_id.as_str(), pair[0].id.as_str())
                >= (pair[1].bundle_id.as_str(), pair[1].id.as_str())
            {
                return Err("project physical declarations are non-canonical".into());
            }
        }
        for physical in &site.physical_declarations {
            if !site.bundle_ids.contains(&physical.bundle_id)
                || !physical_declaration_id(&physical.id)
            {
                return Err("project physical declaration is detached from its bundle".into());
            }
        }
    }

    for document in &index.documents {
        let expected = document_sites
            .remove(document.id.as_str())
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        if document.site_ids != expected {
            return Err(format!("project document backlinks drifted for `{}`", document.path).into());
        }
    }
    for bundle in &index.bundles {
        let expected = bundle_sites
            .remove(bundle.id.as_str())
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        if bundle.site_ids != expected {
            return Err(format!("project bundle backlinks drifted for `{}`", bundle.id).into());
        }
    }
    Ok(())
}

fn verify_bundle_files(
    plan: &AssetPlan,
    base: &Path,
) -> Result<BTreeMap<String, Vec<u8>>, Box<dyn std::error::Error>> {
    let mut verified_css = BTreeMap::new();
    for bundle in &plan.bundles {
        let css = verify_file(base, &bundle.css_file, bundle.css_bytes, &bundle.css_sha256)?;
        let manifest_bytes = verify_file(
            base,
            &bundle.manifest_file,
            bundle.manifest_bytes,
            &bundle.manifest_sha256,
        )?;
        let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
        let graph_nodes = manifest
            .styles
            .len()
            .checked_add(manifest.graph.declarations.len())
            .and_then(|count| count.checked_add(manifest.graph.tokens.len()))
            .and_then(|count| count.checked_add(manifest.graph.components.len()))
            .and_then(|count| count.checked_add(manifest.graph.routes.len()))
            .and_then(|count| count.checked_add(manifest.graph.islands.len()))
            .and_then(|count| count.checked_add(manifest.graph.synthetic_producers.len()))
            .and_then(|count| count.checked_add(manifest.graph.physical_rules.len()))
            .and_then(|count| count.checked_add(manifest.graph.physical_declarations.len()))
            .filter(|count| *count <= MAX_ITEMS)
            .ok_or("manifest graph node limit exceeded")?;
        if graph_nodes > MAX_ITEMS
            || manifest.graph.edges.len() > MAX_ITEMS
            || manifest.schema_version != plan.manifest_schema_version
            || manifest.graph.schema_version != plan.graph_schema_version
            || manifest.style_id_format_version != plan.style_id_format_version
            || manifest.class_name_format_version != plan.class_name_format_version
            || manifest.theme_id_format_version != plan.theme_id_format_version
            || manifest.theme_id != plan.theme_id
            || manifest.targets != plan.targets
            || manifest.format != plan.format
            || manifest.graph.declaration_id_format_version != 1
            || manifest.graph.physical_rule_id_format_version != 1
            || manifest.graph.physical_declaration_id_format_version != 1
            || manifest.graph.origin_coverage != plan.origin_coverage
            || manifest.graph.application_coverage != plan.application_coverage
            || manifest.graph.physical_coverage != "compiler-verified-complete"
            || manifest.css_bytes != css.len()
            || manifest.css_sha256 != sha256_hex(&css)
            || manifest.css_bytes != bundle.css_bytes
            || manifest.css_sha256 != bundle.css_sha256
        {
            return Err(format!("manifest/CSS contract mismatch for `{}`", bundle.id).into());
        }
        verified_css.insert(bundle.id.clone(), css);
    }
    Ok(verified_css)
}

fn selected_bundles<'a>(
    plan: &'a AssetPlan,
    route_path: &str,
    rendered_islands: &[&str],
) -> Result<Vec<&'a Bundle>, Box<dyn std::error::Error>> {
    let route = plan
        .routes
        .iter()
        .find(|route| route.path == route_path)
        .ok_or_else(|| format!("asset plan has no route `{route_path}`"))?;
    let mut selected = route.bundles.iter().collect::<BTreeSet<_>>();
    for name in rendered_islands {
        let island = plan
            .islands
            .iter()
            .find(|island| island.name == *name)
            .ok_or_else(|| format!("asset plan has no island `{name}`"))?;
        selected.extend(&island.bundles);
    }
    let bundles = plan
        .bundles
        .iter()
        .filter(|bundle| selected.contains(&bundle.id))
        .collect::<Vec<_>>();
    if bundles.len() != selected.len() {
        return Err("asset plan root references an unknown bundle".into());
    }
    Ok(bundles)
}

fn required_route<'a>(
    registry: &'a ProductRegistry,
    id: &str,
) -> Result<&'a ProductRoute, Box<dyn std::error::Error>> {
    registry
        .routes()
        .iter()
        .find(|route| route.id() == id)
        .ok_or_else(|| format!("product registry has no route `{id}`").into())
}

fn rendered_island_names<'a>(
    registry: &'a ProductRegistry,
    route: &ProductRoute,
) -> Result<Vec<&'a str>, Box<dyn std::error::Error>> {
    route
        .islands()
        .iter()
        .map(|id| {
            registry
                .islands()
                .iter()
                .find(|island| island.id() == id)
                .map(|island| island.name())
                .ok_or_else(|| format!("product route references unknown island `{id}`").into())
        })
        .collect()
}

fn head(title: &str, bundles: &[&Bundle]) -> Result<Head, Box<dyn std::error::Error>> {
    let preload_candidates = bundles
        .iter()
        .filter(|bundle| bundle.emits_theme)
        .collect::<Vec<_>>();
    let [preload] = preload_candidates.as_slice() else {
        return Err("each route must select exactly one theme-bearing CSS preload".into());
    };
    let head = bundles.iter().fold(Head::new(title), |head, bundle| {
        head.stylesheet(format!("/assets/{}", bundle.css_file))
    });
    Ok(head.preload_stylesheet(format!("/assets/{}", preload.css_file)))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let output = required_path(&mut arguments, "output");
    let supplied = arguments.map(PathBuf::from).collect::<Vec<_>>();
    let (asset_plan_path, project_index_path, source_root) = match supplied.as_slice() {
        [] => {
            let project_root = std::env::current_dir()?;
            let artifact_dir = project_root.join(".pliegocss-gate/bundles");
            (
                artifact_dir.join("pliego.assets.json"),
                artifact_dir.join("pliego.index.json"),
                project_root,
            )
        }
        [asset_plan, project_index, source_root] => (
            asset_plan.clone(),
            project_index.clone(),
            source_root.clone(),
        ),
        _ => return Err("unexpected SSG fixture argument".into()),
    };

    let plan_bytes = read_bounded(&asset_plan_path, "asset plan")?;
    let plan: AssetPlan = serde_json::from_slice(&plan_bytes)?;
    validate_plan(&plan)?;
    if project_index_path.parent() != asset_plan_path.parent() {
        return Err("project index and asset plan must share one artifact directory".into());
    }
    let project_index_bytes = read_bounded(&project_index_path, "project index")?;
    let project_index: ProjectIndex = serde_json::from_slice(&project_index_bytes)?;
    validate_project_index(&project_index, &plan, &plan_bytes, &source_root)?;
    let registry = application_registry();
    registry.validate()?;
    let home_route = required_route(&registry, HOME_ROUTE_ID)?;
    let visit_route = required_route(&registry, VISIT_ROUTE_ID)?;
    let home_islands = rendered_island_names(&registry, home_route)?;
    let visit_islands = rendered_island_names(&registry, visit_route)?;
    let asset_dir = asset_plan_path
        .parent()
        .ok_or("asset plan path has no parent")?;
    let verified_css = verify_bundle_files(&plan, asset_dir)?;
    let client_bindgen = Path::new(CLIENT_BINDGEN_DIR);
    let client_js = read_bounded(&client_bindgen.join(CLIENT_JS_FILE), "WASM client module")?;
    let client_wasm = read_bounded(
        &client_bindgen.join(CLIENT_WASM_FILE),
        "WASM client binary",
    )?;
    let home_bundles = selected_bundles(&plan, home_route.path(), &home_islands)?;
    let visit_bundles = selected_bundles(&plan, visit_route.path(), &visit_islands)?;

    let home_head = head("PliegoCSS home", &home_bundles)?;
    let visit_head = head("PliegoCSS visit", &visit_bundles)?
        .module_script(format!("/{RUNTIME_PATH}"))
        .module_script(format!("/{CLIENT_BOOTSTRAP_PATH}"));
    let deployed = home_bundles
        .iter()
        .chain(&visit_bundles)
        .map(|bundle| bundle.id.as_str())
        .collect::<BTreeSet<_>>();

    let mut site = Site::new()
        .page(Page::new(home_route.path(), home_head, home()))
        .page(Page::new(visit_route.path(), visit_head, visit()?));
    for bundle in plan
        .bundles
        .iter()
        .filter(|bundle| deployed.contains(bundle.id.as_str()))
    {
        let css = verified_css
            .get(&bundle.id)
            .ok_or("verified CSS inventory is incomplete")?
            .clone();
        site = site.asset(Asset::new(format!("assets/{}", bundle.css_file), css));
    }
    let report = site
        .asset(Asset::new(RUNTIME_PATH, runtime_bytes()))
        .asset(Asset::new(CLIENT_BOOTSTRAP_PATH, CLIENT_BOOTSTRAP))
        .asset(Asset::new(CLIENT_JS_PATH, client_js))
        .asset(Asset::new(CLIENT_WASM_PATH, client_wasm))
        .build(&output)?;

    println!("{}", report.receipt.outputs.files.len());
    Ok(())
}
