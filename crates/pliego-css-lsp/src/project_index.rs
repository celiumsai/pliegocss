use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ITEMS: usize = 65_535;

pub(crate) struct NavigationRequest<'a> {
    pub root: &'a Path,
    pub index_path: &'a Path,
    pub document_path: &'a Path,
    pub document_text: &'a str,
    pub literal_start: usize,
    pub literal_end: usize,
    pub origin_range: Value,
}

#[allow(clippy::too_many_lines)]
pub(crate) fn definitions(request: &NavigationRequest<'_>) -> Result<Value, String> {
    let index_path = resolve(request.root, request.index_path);
    let artifact_dir = index_path
        .parent()
        .ok_or("project index has no parent directory")?;
    let index_bytes = read_bounded(&index_path, MAX_INDEX_BYTES, "project index")?;
    let index: ProjectIndex = serde_json::from_slice(&index_bytes)
        .map_err(|error| format!("invalid project index: {error}"))?;
    index.validate()?;
    verified_artifact(
        &artifact_path(artifact_dir, &index.asset_plan_file)?,
        index.asset_plan_bytes,
        &index.asset_plan_sha256,
        "asset plan",
    )?;

    let logical_path = logical_path(request.root, request.document_path)?;
    let document = index
        .documents
        .iter()
        .find(|document| document.path == logical_path)
        .ok_or("open document is absent from the project index")?;
    if document.bytes != request.document_text.len()
        || document.sha256 != sha256(request.document_text.as_bytes())
    {
        return Err("open document does not match the indexed source snapshot".into());
    }

    let site = index
        .sites
        .iter()
        .find(|site| {
            site.document_id == document.id
                && site.path == logical_path
                && site.byte_start == request.literal_start
                && site.byte_end == request.literal_end
        })
        .ok_or("utility literal has no exact project-index site")?;
    if !document.site_ids.iter().any(|id| id == &site.id) {
        return Err("project-index document does not own the selected site".into());
    }

    let bundles = index
        .bundles
        .iter()
        .map(|bundle| (bundle.id.as_str(), bundle))
        .collect::<BTreeMap<_, _>>();
    let mut manifests = BTreeMap::<String, Manifest>::new();
    let mut stylesheets = BTreeMap::<String, Vec<u8>>::new();
    let mut seen = BTreeSet::new();
    let mut links = Vec::new();
    for reference in &site.physical_declarations {
        if !seen.insert((reference.bundle_id.clone(), reference.id.clone())) {
            continue;
        }
        if !site.bundle_ids.iter().any(|id| id == &reference.bundle_id) {
            return Err("physical declaration references an unowned bundle".into());
        }
        let bundle = bundles
            .get(reference.bundle_id.as_str())
            .ok_or("physical declaration references an unknown bundle")?;
        if !bundle.site_ids.iter().any(|id| id == &site.id) {
            return Err("project-index bundle does not own the selected site".into());
        }
        let css = if let Some(css) = stylesheets.get(&bundle.id) {
            css
        } else {
            let path = artifact_path(artifact_dir, &bundle.css_file)?;
            let bytes = verified_artifact(
                &path,
                bundle.css_bytes,
                &bundle.css_sha256,
                "project stylesheet",
            )?;
            stylesheets.insert(bundle.id.clone(), bytes);
            stylesheets.get(&bundle.id).expect("stylesheet inserted")
        };
        let manifest = if let Some(manifest) = manifests.get(&bundle.id) {
            manifest
        } else {
            let path = artifact_path(artifact_dir, &bundle.manifest_file)?;
            let bytes = verified_artifact(
                &path,
                bundle.manifest_bytes,
                &bundle.manifest_sha256,
                "project manifest",
            )?;
            let manifest: Manifest = serde_json::from_slice(&bytes)
                .map_err(|error| format!("invalid project manifest: {error}"))?;
            manifest.validate(bundle)?;
            manifests.insert(bundle.id.clone(), manifest);
            manifests.get(&bundle.id).expect("manifest inserted")
        };
        let declaration = manifest
            .graph
            .physical_declarations
            .iter()
            .find(|declaration| declaration.id == reference.id)
            .ok_or("project manifest lacks a referenced physical declaration")?;
        if declaration.byte_start >= declaration.byte_end || declaration.byte_end > css.len() {
            return Err("physical declaration range is outside its stylesheet".into());
        }
        let target_path = artifact_path(artifact_dir, &bundle.css_file)?;
        let target_range = byte_range(css, declaration.byte_start, declaration.byte_end)?;
        links.push(json!({
            "originSelectionRange": request.origin_range,
            "targetUri": path_to_file_uri(&target_path)?,
            "targetRange": target_range,
            "targetSelectionRange": target_range
        }));
    }
    if links.is_empty() {
        return Err("project-index site has no physical declarations".into());
    }
    Ok(Value::Array(links))
}

fn resolve(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn logical_path(root: &Path, document: &Path) -> Result<String, String> {
    let relative = document
        .strip_prefix(root)
        .map_err(|_| "open document is outside the workspace root")?;
    let mut segments = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(segment) => {
                segments.push(segment.to_str().ok_or("document path is not valid UTF-8")?);
            }
            _ => return Err("document path is not portable relative to the workspace root".into()),
        }
    }
    if segments.is_empty() {
        return Err("document path is empty".into());
    }
    Ok(segments.join("/"))
}

fn artifact_path(root: &Path, file: &str) -> Result<PathBuf, String> {
    if file.is_empty()
        || file.contains(['/', '\\', ':'])
        || file == "."
        || file == ".."
        || file.chars().any(char::is_control)
    {
        return Err("project index contains an unsafe artifact filename".into());
    }
    Ok(root.join(file))
}

fn verified_artifact(
    path: &Path,
    expected_bytes: usize,
    expected_sha256: &str,
    label: &str,
) -> Result<Vec<u8>, String> {
    let bytes = read_bounded(path, MAX_ARTIFACT_BYTES, label)?;
    if bytes.len() != expected_bytes || sha256(&bytes) != expected_sha256 {
        return Err(format!("{label} does not match the project index"));
    }
    Ok(bytes)
}

fn read_bounded(path: &Path, limit: u64, label: &str) -> Result<Vec<u8>, String> {
    let limit = usize::try_from(limit).map_err(|_| format!("{label} limit exceeds this host"))?;
    pliego_css_io::read_bounded_regular_file(path, limit, label)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn byte_range(bytes: &[u8], start: usize, end: usize) -> Result<Value, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "stylesheet is not UTF-8")?;
    if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return Err("physical declaration splits a UTF-8 scalar".into());
    }
    Ok(json!({
        "start": byte_position(text, start),
        "end": byte_position(text, end)
    }))
}

fn byte_position(text: &str, byte: usize) -> Value {
    let prefix = &text[..byte];
    let line = prefix.bytes().filter(|value| *value == b'\n').count();
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    json!({"line":line,"character":prefix[line_start..].encode_utf16().count()})
}

fn path_to_file_uri(path: &Path) -> Result<String, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("cannot resolve artifact path: {error}"))?
            .join(path)
    };
    let portable = absolute
        .to_str()
        .ok_or("artifact path is not valid UTF-8")?
        .replace('\\', "/");
    let portable = if portable.starts_with('/') {
        portable
    } else {
        format!("/{portable}")
    };
    let mut encoded = String::from("file://");
    for byte in portable.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b':' | b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            write!(&mut encoded, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    Ok(encoded)
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

impl ProjectIndex {
    #[allow(clippy::too_many_lines)]
    fn validate(&self) -> Result<(), String> {
        let selection_ok = matches!(
            (self.schema_version, self.rule_selection.as_str()),
            (1, "all-compiled" | "reachable-style-ids") | (2, "reachable-or-retained-style-ids")
        );
        let versions_ok = self.source_site_id_format_version == 1
            && self.manifest_schema_version == 5
            && self.graph_schema_version == 2
            && self.declaration_id_format_version == 1
            && self.physical_rule_id_format_version == 1
            && self.physical_declaration_id_format_version == 1
            && self.style_id_format_version > 0
            && self.class_name_format_version > 0
            && self.theme_id_format_version > 0;
        let coverage_ok = self.origin_coverage == "compiler-verified-complete"
            && self.application_coverage == "adapter-attested-complete"
            && self.physical_coverage == "compiler-verified-complete";
        let identity_ok = !self.theme_id.is_empty()
            && !self.targets.is_empty()
            && !self.format.is_empty()
            && self.asset_plan_file == "pliego.assets.json"
            && self.asset_plan_bytes > 0
            && valid_sha256(&self.asset_plan_sha256);
        if !selection_ok || !versions_ok || !coverage_ok || !identity_ok {
            return Err("unsupported or detached project index".into());
        }
        if self.documents.len() > MAX_ITEMS
            || self.bundles.len() > MAX_ITEMS
            || self.sites.len() > MAX_ITEMS
        {
            return Err("project index item limit exceeded".into());
        }
        if self
            .bundles
            .iter()
            .filter(|bundle| bundle.emits_theme)
            .count()
            > 1
        {
            return Err("project index has multiple theme bundles".into());
        }
        if !unique(self.documents.iter().map(|item| item.id.as_str()))
            || !unique(self.documents.iter().map(|item| item.path.as_str()))
            || !unique(self.bundles.iter().map(|item| item.id.as_str()))
            || !unique(self.sites.iter().map(|item| item.id.as_str()))
        {
            return Err("project index contains duplicate identities".into());
        }
        if self.bundles.iter().any(|bundle| {
            bundle.id.is_empty()
                || bundle.site_ids.len() > MAX_ITEMS
                || !valid_sha256(&bundle.css_sha256)
                || !valid_sha256(&bundle.manifest_sha256)
        }) || self.documents.iter().any(|document| {
            document.id.is_empty()
                || document.bytes > 16 * 1024 * 1024
                || document.site_ids.len() > MAX_ITEMS
                || !valid_logical_path(&document.path)
                || !valid_sha256(&document.sha256)
        }) || self.sites.iter().any(|site| {
            site.id.is_empty()
                || !valid_logical_path(&site.path)
                || site.byte_start >= site.byte_end
                || site.physical_declarations.is_empty()
                || site.physical_declarations.len() > MAX_ITEMS
                || site.bundle_ids.is_empty()
                || site.declaration_ids.is_empty()
                || site.component_ids.is_empty()
                || site.macro_kind.is_empty()
                || site.reason.is_empty()
                || site.source.is_empty()
                || site.style_id.is_empty()
                || site.class_name.is_empty()
                || site.token_ids.len() > MAX_ITEMS
        }) {
            return Err("project index collection contract drifted".into());
        }
        let document_by_id = self
            .documents
            .iter()
            .map(|item| (item.id.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        let site_ids = self
            .sites
            .iter()
            .map(|item| item.id.as_str())
            .collect::<BTreeSet<_>>();
        let bundle_ids = self
            .bundles
            .iter()
            .map(|item| item.id.as_str())
            .collect::<BTreeSet<_>>();
        for document in &self.documents {
            if document
                .site_ids
                .iter()
                .any(|id| !site_ids.contains(id.as_str()))
            {
                return Err("project-index document references an unknown site".into());
            }
        }
        for bundle in &self.bundles {
            if bundle.css_file != format!("{}.css", bundle.id)
                || bundle.manifest_file != format!("{}.manifest.json", bundle.id)
                || bundle
                    .site_ids
                    .iter()
                    .any(|id| !site_ids.contains(id.as_str()))
            {
                return Err("project-index bundle contract drifted".into());
            }
        }
        for site in &self.sites {
            let document = document_by_id
                .get(site.document_id.as_str())
                .ok_or("project-index site references an unknown document")?;
            if document.path != site.path
                || site.byte_end > document.bytes
                || !document.site_ids.iter().any(|id| id == &site.id)
                || site
                    .bundle_ids
                    .iter()
                    .any(|id| !bundle_ids.contains(id.as_str()))
            {
                return Err("project-index site ownership drifted".into());
            }
        }
        Ok(())
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn unique<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    values.into_iter().all(|value| seen.insert(value))
}

fn valid_logical_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && !value.contains(['\\', ':'])
        && !value.chars().any(char::is_control)
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
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

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PhysicalReference {
    bundle_id: String,
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u8,
    css_sha256: String,
    css_bytes: usize,
    graph: ManifestGraph,
    #[serde(flatten)]
    rest: BTreeMap<String, Value>,
}

impl Manifest {
    fn validate(&self, bundle: &ProjectBundle) -> Result<(), String> {
        if self.schema_version != 5
            || self.css_sha256 != bundle.css_sha256
            || self.css_bytes != bundle.css_bytes
            || self.graph.schema_version != 2
            || self.graph.physical_declaration_id_format_version != 1
            || self.graph.physical_coverage != "compiler-verified-complete"
            || self.graph.physical_declarations.len() > MAX_ITEMS
            || self.graph.rest.is_empty()
            || self.rest.is_empty()
        {
            return Err("unsupported project manifest".into());
        }
        for (ordinal, declaration) in self.graph.physical_declarations.iter().enumerate() {
            if declaration.ordinal != ordinal
                || declaration.property.is_empty()
                || declaration.byte_start >= declaration.byte_end
                || declaration.property_byte_start >= declaration.property_byte_end
                || declaration.value_byte_start >= declaration.value_byte_end
                || declaration.property_byte_start < declaration.byte_start
                || declaration.property_byte_end > declaration.byte_end
                || declaration.value_byte_start < declaration.byte_start
                || declaration.value_byte_end > declaration.byte_end
            {
                return Err("project manifest physical declaration contract drifted".into());
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestGraph {
    schema_version: u8,
    physical_declaration_id_format_version: u8,
    physical_coverage: String,
    physical_declarations: Vec<PhysicalDeclaration>,
    #[serde(flatten)]
    rest: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PhysicalDeclaration {
    id: String,
    ordinal: usize,
    property: String,
    #[serde(rename = "important")]
    _important: bool,
    #[serde(rename = "generated")]
    _generated: bool,
    byte_start: usize,
    byte_end: usize,
    property_byte_start: usize,
    property_byte_end: usize,
    value_byte_start: usize,
    value_byte_end: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn logical_paths_and_artifact_names_fail_closed() {
        assert!(valid_logical_path("src/card.rs"));
        assert!(!valid_logical_path("../card.rs"));
        assert!(artifact_path(Path::new("out"), "app.css").is_ok());
        assert!(artifact_path(Path::new("out"), "../app.css").is_err());
    }

    #[test]
    fn file_uri_encodes_spaces() {
        let uri = path_to_file_uri(Path::new("/tmp/Pliego CSS/app.css")).unwrap();
        assert!(uri.ends_with("/tmp/Pliego%20CSS/app.css"));
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn project_index_reader_rejects_file_and_parent_links() {
        let root = std::env::temp_dir().join(format!("pliego-index-links-{}", std::process::id()));
        fs::create_dir_all(root.join("real")).unwrap();
        fs::write(root.join("real/index.json"), b"{}").unwrap();
        let file_link = root.join("index.json");
        let parent_link = root.join("linked");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("real/index.json"), &file_link).unwrap();
            std::os::unix::fs::symlink(root.join("real"), &parent_link).unwrap();
        }
        #[cfg(windows)]
        {
            if std::os::windows::fs::symlink_file(root.join("real/index.json"), &file_link).is_err()
                || std::os::windows::fs::symlink_dir(root.join("real"), &parent_link).is_err()
            {
                fs::remove_dir_all(root).unwrap();
                return;
            }
        }
        assert!(
            read_bounded(&file_link, 1024, "project index")
                .unwrap_err()
                .contains("bounded regular file")
        );
        assert!(
            read_bounded(&parent_link.join("index.json"), 1024, "project index")
                .unwrap_err()
                .contains("unsafe path component")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn definition_follows_verified_index_manifest_and_css() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::current_dir()
            .expect("current directory")
            .join("target/pliego-css-lsp-tests")
            .join(format!("navigation-{}-{nonce}", std::process::id()));
        let source_dir = root.join("src");
        let output_dir = root.join("out");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&output_dir).unwrap();
        let source = "fn view(){let _=pc!(\"flex\");}";
        let literal_start = source.find("\"flex\"").unwrap();
        let literal_end = literal_start + "\"flex\"".len();
        let css = b".pc{display:flex}\n";
        let manifest = serde_json::to_vec(&json!({
            "schemaVersion":5,
            "cssSha256":sha256(css),
            "cssBytes":css.len(),
            "themeId":"theme",
            "graph":{
                "schemaVersion":2,
                "physicalDeclarationIdFormatVersion":1,
                "physicalCoverage":"compiler-verified-complete",
                "declarations":[],
                "physicalDeclarations":[{
                    "id":"css-decl:00000000:00000000","ordinal":0,
                    "property":"display","important":false,"generated":false,
                    "byteStart":4,"byteEnd":16,
                    "propertyByteStart":4,"propertyByteEnd":11,
                    "valueByteStart":12,"valueByteEnd":16
                }]
            }
        }))
        .unwrap();
        let index = serde_json::to_vec(&json!({
            "schemaVersion":1,
            "sourceSiteIdFormatVersion":1,
            "manifestSchemaVersion":5,
            "graphSchemaVersion":2,
            "declarationIdFormatVersion":1,
            "physicalRuleIdFormatVersion":1,
            "physicalDeclarationIdFormatVersion":1,
            "originCoverage":"compiler-verified-complete",
            "applicationCoverage":"adapter-attested-complete",
            "physicalCoverage":"compiler-verified-complete",
            "styleIdFormatVersion":2,
            "classNameFormatVersion":1,
            "themeIdFormatVersion":3,
            "themeId":"theme","targets":"modern","format":"minified",
            "ruleSelection":"all-compiled",
            "assetPlanFile":"pliego.assets.json",
            "assetPlanBytes":1,
            "assetPlanSha256":sha256(b"x"),
            "documents":[{
                "id":"document:one","path":"src/card.rs","bytes":source.len(),
                "sha256":sha256(source.as_bytes()),"siteIds":["site:one"]
            }],
            "bundles":[{
                "id":"app","cssFile":"app.css","manifestFile":"app.manifest.json",
                "emitsTheme":false,"cssBytes":css.len(),"cssSha256":sha256(css),
                "manifestBytes":manifest.len(),"manifestSha256":sha256(&manifest),
                "siteIds":["site:one"]
            }],
            "sites":[{
                "id":"site:one","documentId":"document:one","path":"src/card.rs",
                "byteStart":literal_start,"byteEnd":literal_end,"macroKind":"pc",
                "reason":"visible-literal","source":"flex","styleId":"style",
                "className":"pc_one","bundleIds":["app"],
                "declarationIds":["decl:one"],"tokenIds":[],
                "componentIds":["component:one"],
                "physicalDeclarations":[{
                    "bundleId":"app","id":"css-decl:00000000:00000000"
                }]
            }]
        }))
        .unwrap();
        fs::write(source_dir.join("card.rs"), source).unwrap();
        fs::write(output_dir.join("app.css"), css).unwrap();
        fs::write(output_dir.join("app.manifest.json"), manifest).unwrap();
        fs::write(output_dir.join("pliego.index.json"), index).unwrap();
        fs::write(output_dir.join("pliego.assets.json"), b"x").unwrap();

        let result = definitions(&NavigationRequest {
            root: &root,
            index_path: Path::new("out/pliego.index.json"),
            document_path: &source_dir.join("card.rs"),
            document_text: source,
            literal_start,
            literal_end,
            origin_range: json!({
                "start":{"line":0,"character":literal_start + 1},
                "end":{"line":0,"character":literal_end - 1}
            }),
        })
        .unwrap();
        assert_eq!(
            result[0]["targetRange"]["start"],
            json!({"line":0,"character":4})
        );
        assert_eq!(
            result[0]["targetRange"]["end"],
            json!({"line":0,"character":16})
        );

        let stale = format!("{source} ");
        let failure = definitions(&NavigationRequest {
            root: &root,
            index_path: Path::new("out/pliego.index.json"),
            document_path: &source_dir.join("card.rs"),
            document_text: &stale,
            literal_start,
            literal_end,
            origin_range: Value::Null,
        })
        .unwrap_err();
        assert!(failure.contains("indexed source snapshot"));

        fs::remove_dir_all(root).unwrap();
    }
}
