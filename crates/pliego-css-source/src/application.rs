//! Framework-owned application topology collection.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{ScanDiagnostic, SourceParseError, scan_source_named};

const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_SOURCE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TOTAL_SOURCE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ITEMS: usize = 65_535;
const MAX_PATH_SEGMENTS: usize = 256;
const MAX_ID_BYTES: usize = 256;
const MAX_PATH_OR_NAME_BYTES: usize = 4 * 1024;

/// One framework component and the `PliegoCSS` source sites it owns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationComponent {
    id: String,
    ownership: Vec<SourceOwnership>,
}

/// One application route and its complete component set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationRoute {
    id: String,
    path: String,
    components: Vec<String>,
}

/// One resumable island and its complete component set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationIsland {
    id: String,
    name: String,
    components: Vec<String>,
}

/// Framework-attested topology and bounded Rust source inventory.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ApplicationTopology {
    source_roots: Vec<String>,
    components: Vec<ApplicationComponent>,
    routes: Vec<ApplicationRoute>,
    islands: Vec<ApplicationIsland>,
}

/// Canonical reachability sidecar generated from one immutable application snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectedReachability {
    bytes: Vec<u8>,
    source_files: usize,
    invocations: usize,
}

/// Failure while validating topology, inventorying Rust, or collecting exact sites.
#[non_exhaustive]
#[derive(Debug)]
pub enum CollectError {
    /// The adapter supplied an incomplete, duplicate, unsafe, or stale topology.
    Invalid(String),
    /// A source root or Rust source could not be inspected.
    Io {
        /// Path being inspected.
        path: PathBuf,
        /// Underlying filesystem failure.
        source: std::io::Error,
    },
    /// One Rust source unit did not parse.
    Parse(SourceParseError),
    /// One visible `pc!` or `pcx!` invocation was malformed.
    Diagnostic {
        /// Stable scanner diagnostic code.
        code: &'static str,
        /// Portable logical source path.
        source: String,
        /// One-based line.
        line: usize,
        /// One-based column.
        column: usize,
        /// Scanner explanation.
        message: String,
    },
    /// Canonical JSON serialization failed.
    Serialize(serde_json::Error),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SourceOwnership {
    File(String),
    Site {
        file: String,
        byte_start: usize,
        byte_end: usize,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReachabilityDocument {
    schema: u8,
    application_coverage: &'static str,
    components: Vec<ReachabilityComponent>,
    routes: Vec<ReachabilityRoute>,
    islands: Vec<ReachabilityIsland>,
}

#[derive(Serialize)]
struct ReachabilityComponent {
    id: String,
    sites: Vec<ReachabilitySite>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReachabilitySite {
    file: String,
    byte_start: usize,
    byte_end: usize,
}

#[derive(Serialize)]
struct ReachabilityRoute {
    id: String,
    path: String,
    components: Vec<String>,
}

#[derive(Serialize)]
struct ReachabilityIsland {
    id: String,
    name: String,
    components: Vec<String>,
}

type SiteKey = (String, usize, usize);

impl ApplicationComponent {
    /// Creates a component with no `PliegoCSS` ownership yet.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ownership: Vec::new(),
        }
    }

    /// Attests that every `PliegoCSS` invocation in one Rust unit belongs to this component.
    #[must_use]
    pub fn source_unit(mut self, file: impl Into<String>) -> Self {
        self.ownership.push(SourceOwnership::File(file.into()));
        self
    }

    /// Attests ownership of one exact half-open macro invocation range.
    #[must_use]
    pub fn site(mut self, file: impl Into<String>, byte_start: usize, byte_end: usize) -> Self {
        self.ownership.push(SourceOwnership::Site {
            file: file.into(),
            byte_start,
            byte_end,
        });
        self
    }
}

impl ApplicationRoute {
    /// Creates one route with a stable adapter identity and application path.
    pub fn new(id: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            components: Vec::new(),
        }
    }

    /// Adds one component used by this route.
    #[must_use]
    pub fn component(mut self, id: impl Into<String>) -> Self {
        self.components.push(id.into());
        self
    }
}

impl ApplicationIsland {
    /// Creates one resumable island with stable identity and rendered name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            components: Vec::new(),
        }
    }

    /// Adds one component used by this island.
    #[must_use]
    pub fn component(mut self, id: impl Into<String>) -> Self {
        self.components.push(id.into());
        self
    }
}

impl ApplicationTopology {
    /// Creates an empty topology. At least one source root is required before collection.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            source_roots: Vec::new(),
            components: Vec::new(),
            routes: Vec::new(),
            islands: Vec::new(),
        }
    }

    /// Adds a portable project-relative Rust file or directory to the complete scan inventory.
    #[must_use]
    pub fn source_root(mut self, path: impl Into<String>) -> Self {
        self.source_roots.push(path.into());
        self
    }

    /// Adds one framework component.
    #[must_use]
    pub fn component(mut self, component: ApplicationComponent) -> Self {
        self.components.push(component);
        self
    }

    /// Adds one framework route.
    #[must_use]
    pub fn route(mut self, route: ApplicationRoute) -> Self {
        self.routes.push(route);
        self
    }

    /// Adds one resumable island.
    #[must_use]
    pub fn island(mut self, island: ApplicationIsland) -> Self {
        self.islands.push(island);
        self
    }

    /// Scans the declared immutable source inventory and emits canonical reachability schema 1.
    ///
    /// Every discovered valid `pc!`/`pcx!` invocation must have at least one explicit owner.
    /// Exact-site declarations must match a discovered invocation. File declarations deliberately
    /// own every invocation in that source unit; no route or component identity is inferred from
    /// paths.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or dangling topology, unsafe paths, symlinks, oversized
    /// inputs, unreadable or invalid Rust, scanner diagnostics, unowned invocations, stale exact
    /// sites, or defensive limit violations.
    pub fn collect(
        &self,
        project_root: impl AsRef<Path>,
    ) -> Result<CollectedReachability, CollectError> {
        collect(self, project_root.as_ref())
    }
}

impl CollectedReachability {
    /// Returns canonical UTF-8 JSON ending in one LF.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes the result and returns canonical UTF-8 JSON.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Returns the number of Rust files in the attested scan inventory.
    #[must_use]
    pub const fn source_file_count(&self) -> usize {
        self.source_files
    }

    /// Returns the number of discovered valid macro invocations before shared ownership expands.
    #[must_use]
    pub const fn invocation_count(&self) -> usize {
        self.invocations
    }
}

impl fmt::Display for CollectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Io { path, source } => {
                write!(formatter, "cannot inspect {}: {source}", path.display())
            }
            Self::Parse(source) => source.fmt(formatter),
            Self::Diagnostic {
                code,
                source,
                line,
                column,
                message,
            } => write!(formatter, "{code}: {message} at {source}:{line}:{column}"),
            Self::Serialize(source) => write!(formatter, "cannot serialize reachability: {source}"),
        }
    }
}

impl std::error::Error for CollectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse(source) => Some(source),
            Self::Serialize(source) => Some(source),
            Self::Invalid(_) | Self::Diagnostic { .. } => None,
        }
    }
}

fn collect(
    topology: &ApplicationTopology,
    project_root: &Path,
) -> Result<CollectedReachability, CollectError> {
    let files = source_inventory(project_root, &topology.source_roots)?;
    let components = canonical_components(&topology.components)?;
    let component_ids = components
        .iter()
        .map(|component| component.id.clone())
        .collect::<BTreeSet<_>>();
    let routes = canonical_routes(&topology.routes, &component_ids)?;
    let islands = canonical_islands(&topology.islands, &component_ids)?;
    bounded_topology(&components, &routes, &islands)?;

    let ownership = ownership_index(&components, &files)?;
    let (sites_by_component, invocation_count) = collect_sites(&files, &ownership)?;
    let bytes = render_document(components, routes, islands, sites_by_component)?;
    Ok(CollectedReachability {
        bytes,
        source_files: files.len(),
        invocations: invocation_count,
    })
}

struct OwnershipIndex {
    file: BTreeMap<String, BTreeSet<String>>,
    site: BTreeMap<SiteKey, BTreeSet<String>>,
    declared_sites: BTreeSet<SiteKey>,
}

fn source_inventory(
    project_root: &Path,
    declared_roots: &[String],
) -> Result<BTreeMap<String, PathBuf>, CollectError> {
    let root = fs::canonicalize(project_root).map_err(|source| CollectError::Io {
        path: project_root.to_owned(),
        source,
    })?;
    let metadata = fs::metadata(&root).map_err(|source| CollectError::Io {
        path: root.clone(),
        source,
    })?;
    let root_link_metadata = fs::symlink_metadata(&root).map_err(|source| CollectError::Io {
        path: root.clone(),
        source,
    })?;
    valid(
        metadata.is_dir() && !link_like(&root_link_metadata),
        "project root is not an unlinked directory",
    )?;
    let roots = canonical_source_roots(declared_roots)?;
    let mut files = BTreeMap::new();
    for source_root in &roots {
        discover(&root, source_root, &mut files)?;
    }
    valid(!files.is_empty(), "source inventory contains no Rust files")?;
    valid(
        files.len() <= MAX_ITEMS,
        "source inventory exceeds 65,535 Rust files",
    )?;
    Ok(files)
}

fn ownership_index(
    components: &[ApplicationComponent],
    files: &BTreeMap<String, PathBuf>,
) -> Result<OwnershipIndex, CollectError> {
    let mut index = OwnershipIndex {
        file: BTreeMap::new(),
        site: BTreeMap::new(),
        declared_sites: BTreeSet::new(),
    };
    for component in components {
        for ownership in &component.ownership {
            let file = match ownership {
                SourceOwnership::File(file) | SourceOwnership::Site { file, .. } => file,
            };
            valid(
                files.contains_key(file),
                format!(
                    "component `{}` references source outside the inventory: {file}",
                    component.id
                ),
            )?;
            match ownership {
                SourceOwnership::File(file) => {
                    index
                        .file
                        .entry(file.clone())
                        .or_default()
                        .insert(component.id.clone());
                }
                SourceOwnership::Site {
                    file,
                    byte_start,
                    byte_end,
                } => {
                    let key = (file.clone(), *byte_start, *byte_end);
                    index
                        .site
                        .entry(key.clone())
                        .or_default()
                        .insert(component.id.clone());
                    index.declared_sites.insert(key);
                }
            }
        }
    }
    Ok(index)
}

fn collect_sites(
    files: &BTreeMap<String, PathBuf>,
    ownership: &OwnershipIndex,
) -> Result<(BTreeMap<String, Vec<ReachabilitySite>>, usize), CollectError> {
    let mut sites = BTreeMap::<String, Vec<ReachabilitySite>>::new();
    let mut matched_sites = BTreeSet::new();
    let mut invocation_count = 0_usize;
    let mut expanded_site_count = 0_usize;
    let mut total_source_bytes = 0_u64;
    for (logical, absolute) in files {
        total_source_bytes = total_source_bytes
            .checked_add(read_source_length(absolute, logical)?)
            .ok_or_else(|| invalid("source inventory byte count overflow"))?;
        valid(
            total_source_bytes <= MAX_TOTAL_SOURCE_BYTES,
            "source inventory exceeds 256 MiB",
        )?;
        let source = fs::read_to_string(absolute).map_err(|source| CollectError::Io {
            path: absolute.clone(),
            source,
        })?;
        let report = scan_source_named(logical, &source).map_err(CollectError::Parse)?;
        if let Some(diagnostic) = report.diagnostics.into_iter().next() {
            return Err(diagnostic_error(diagnostic));
        }
        for invocation in report.invocations {
            invocation_count = invocation_count
                .checked_add(1)
                .ok_or_else(|| invalid("invocation count overflow"))?;
            valid(
                invocation_count <= MAX_ITEMS,
                "source inventory exceeds 65,535 PliegoCSS invocations",
            )?;
            let key = (
                logical.clone(),
                invocation.range.start.byte,
                invocation.range.end.byte,
            );
            let mut owners = ownership.file.get(logical).cloned().unwrap_or_default();
            if let Some(exact) = ownership.site.get(&key) {
                owners.extend(exact.iter().cloned());
                matched_sites.insert(key.clone());
            }
            valid(
                !owners.is_empty(),
                format!(
                    "unowned PliegoCSS invocation at {}:{}:{}",
                    logical,
                    invocation.range.start.line,
                    invocation.range.start.column + 1
                ),
            )?;
            expanded_site_count = expanded_site_count
                .checked_add(owners.len())
                .ok_or_else(|| invalid("collected site ownership count overflow"))?;
            valid(
                expanded_site_count <= MAX_ITEMS,
                "collected site ownership exceeds 65,535 items",
            )?;
            for owner in owners {
                sites.entry(owner).or_default().push(ReachabilitySite {
                    file: logical.clone(),
                    byte_start: key.1,
                    byte_end: key.2,
                });
            }
        }
    }
    if let Some((file, byte_start, byte_end)) =
        ownership.declared_sites.difference(&matched_sites).next()
    {
        return Err(invalid(format!(
            "stale exact PliegoCSS site {file}:{byte_start}..{byte_end}"
        )));
    }
    Ok((sites, invocation_count))
}

fn read_source_length(path: &Path, logical: &str) -> Result<u64, CollectError> {
    let metadata = fs::metadata(path).map_err(|source| CollectError::Io {
        path: path.to_owned(),
        source,
    })?;
    valid(
        metadata.len() <= MAX_SOURCE_BYTES,
        format!("Rust source exceeds 16 MiB: {logical}"),
    )?;
    Ok(metadata.len())
}

fn render_document(
    mut components: Vec<ApplicationComponent>,
    routes: Vec<ApplicationRoute>,
    islands: Vec<ApplicationIsland>,
    mut sites: BTreeMap<String, Vec<ReachabilitySite>>,
) -> Result<Vec<u8>, CollectError> {
    let document = ReachabilityDocument {
        schema: 1,
        application_coverage: "complete",
        components: components
            .drain(..)
            .map(|component| ReachabilityComponent {
                sites: sites.remove(&component.id).unwrap_or_default(),
                id: component.id,
            })
            .collect(),
        routes: routes
            .into_iter()
            .map(|route| ReachabilityRoute {
                id: route.id,
                path: route.path,
                components: route.components,
            })
            .collect(),
        islands: islands
            .into_iter()
            .map(|island| ReachabilityIsland {
                id: island.id,
                name: island.name,
                components: island.components,
            })
            .collect(),
    };
    let mut bytes = serde_json::to_vec_pretty(&document).map_err(CollectError::Serialize)?;
    bytes.push(b'\n');
    valid(
        bytes.len() <= MAX_DOCUMENT_BYTES,
        "reachability document exceeds 16 MiB",
    )?;
    Ok(bytes)
}

fn canonical_source_roots(source_roots: &[String]) -> Result<Vec<String>, CollectError> {
    valid(
        !source_roots.is_empty(),
        "application topology requires at least one source root",
    )?;
    let mut roots = source_roots.to_vec();
    for root in &roots {
        validate_portable_path(root)?;
    }
    roots.sort();
    valid(
        !roots.windows(2).any(|pair| pair[0] == pair[1]),
        "duplicate source root",
    )?;
    Ok(roots)
}

fn canonical_components(
    input: &[ApplicationComponent],
) -> Result<Vec<ApplicationComponent>, CollectError> {
    let mut components = input.to_vec();
    for component in &mut components {
        validate_id(&component.id)?;
        for ownership in &component.ownership {
            let (file, range) = match ownership {
                SourceOwnership::File(file) => (file, None),
                SourceOwnership::Site {
                    file,
                    byte_start,
                    byte_end,
                } => (file, Some((*byte_start, *byte_end))),
            };
            validate_portable_path(file)?;
            if let Some((start, end)) = range {
                valid(start < end, "exact source site start must precede end")?;
            }
        }
        component.ownership.sort();
        valid(
            !component
                .ownership
                .windows(2)
                .any(|pair| pair[0] == pair[1]),
            format!("component `{}` repeats source ownership", component.id),
        )?;
    }
    components.sort_by(|left, right| left.id.cmp(&right.id));
    valid(
        !components.windows(2).any(|pair| pair[0].id == pair[1].id),
        "duplicate component id",
    )?;
    Ok(components)
}

fn canonical_routes(
    input: &[ApplicationRoute],
    component_ids: &BTreeSet<String>,
) -> Result<Vec<ApplicationRoute>, CollectError> {
    let mut routes = input.to_vec();
    let mut paths = BTreeSet::new();
    for route in &mut routes {
        validate_id(&route.id)?;
        validate_text(&route.path, MAX_PATH_OR_NAME_BYTES, "route path")?;
        valid(paths.insert(route.path.clone()), "duplicate route path")?;
        canonical_references(&mut route.components, component_ids)?;
    }
    routes.sort_by(|left, right| left.id.cmp(&right.id));
    valid(
        !routes.windows(2).any(|pair| pair[0].id == pair[1].id),
        "duplicate route id",
    )?;
    Ok(routes)
}

fn canonical_islands(
    input: &[ApplicationIsland],
    component_ids: &BTreeSet<String>,
) -> Result<Vec<ApplicationIsland>, CollectError> {
    let mut islands = input.to_vec();
    let mut names = BTreeSet::new();
    for island in &mut islands {
        validate_id(&island.id)?;
        validate_text(&island.name, MAX_PATH_OR_NAME_BYTES, "island name")?;
        valid(names.insert(island.name.clone()), "duplicate island name")?;
        canonical_references(&mut island.components, component_ids)?;
    }
    islands.sort_by(|left, right| left.id.cmp(&right.id));
    valid(
        !islands.windows(2).any(|pair| pair[0].id == pair[1].id),
        "duplicate island id",
    )?;
    Ok(islands)
}

fn canonical_references(
    references: &mut Vec<String>,
    component_ids: &BTreeSet<String>,
) -> Result<(), CollectError> {
    for reference in &*references {
        validate_id(reference)?;
        valid(
            component_ids.contains(reference),
            format!("unknown component reference `{reference}`"),
        )?;
    }
    references.sort();
    valid(
        !references.windows(2).any(|pair| pair[0] == pair[1]),
        "duplicate component reference",
    )
}

fn bounded_topology(
    components: &[ApplicationComponent],
    routes: &[ApplicationRoute],
    islands: &[ApplicationIsland],
) -> Result<(), CollectError> {
    let counts = components
        .len()
        .checked_add(routes.len())
        .and_then(|count| count.checked_add(islands.len()))
        .and_then(|count| {
            components
                .iter()
                .try_fold(count, |sum, item| sum.checked_add(item.ownership.len()))
        })
        .and_then(|count| {
            routes
                .iter()
                .try_fold(count, |sum, item| sum.checked_add(item.components.len()))
        })
        .and_then(|count| {
            islands
                .iter()
                .try_fold(count, |sum, item| sum.checked_add(item.components.len()))
        });
    valid(
        counts.is_some_and(|count| count <= MAX_ITEMS),
        "application topology exceeds 65,535 items",
    )
}

fn discover(
    project_root: &Path,
    logical: &str,
    files: &mut BTreeMap<String, PathBuf>,
) -> Result<(), CollectError> {
    let path = join_portable(project_root, logical);
    let metadata = fs::symlink_metadata(&path).map_err(|source| CollectError::Io {
        path: path.clone(),
        source,
    })?;
    valid(
        !link_like(&metadata),
        format!("source inventory rejects symlink or junction: {logical}"),
    )?;
    if metadata.is_file() {
        if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.insert(logical.to_owned(), path);
        }
        return Ok(());
    }
    valid(
        metadata.is_dir(),
        format!("source root is not a file or directory: {logical}"),
    )?;
    let entries = fs::read_dir(&path).map_err(|source| CollectError::Io {
        path: path.clone(),
        source,
    })?;
    let mut names = entries
        .map(|entry| {
            entry
                .map_err(|source| CollectError::Io {
                    path: path.clone(),
                    source,
                })?
                .file_name()
                .into_string()
                .map_err(|_| {
                    invalid(format!(
                        "source inventory contains a non-UTF-8 name below {logical}"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    names.sort();
    for name in names {
        let child = format!("{logical}/{name}");
        validate_portable_path(&child)?;
        discover(project_root, &child, files)?;
    }
    Ok(())
}

fn join_portable(root: &Path, logical: &str) -> PathBuf {
    logical
        .split('/')
        .fold(root.to_owned(), |path, segment| path.join(segment))
}

fn diagnostic_error(diagnostic: ScanDiagnostic) -> CollectError {
    CollectError::Diagnostic {
        code: diagnostic.code,
        source: diagnostic.source,
        line: diagnostic.range.start.line,
        column: diagnostic.range.start.column + 1,
        message: diagnostic.message,
    }
}

fn validate_id(value: &str) -> Result<(), CollectError> {
    validate_text(value, MAX_ID_BYTES, "application id")
}

fn validate_text(value: &str, maximum: usize, role: &str) -> Result<(), CollectError> {
    valid(
        !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control),
        format!("invalid {role}"),
    )
}

fn validate_portable_path(path: &str) -> Result<(), CollectError> {
    valid(
        is_portable_source_path(path),
        format!("invalid portable source path `{path}`"),
    )
}

pub(crate) fn is_portable_source_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_PATH_OR_NAME_BYTES
        && !path.chars().any(char::is_control)
        && !path.starts_with('/')
        && !path.contains('\\')
        && path.split('/').count() <= MAX_PATH_SEGMENTS
        && path.split('/').all(portable_segment)
}

fn link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    false
}

fn portable_segment(segment: &str) -> bool {
    if segment.is_empty()
        || matches!(segment, "." | "..")
        || segment.ends_with(['.', ' '])
        || segment.bytes().any(|byte| b"<>:\"|?*".contains(&byte))
    {
        return false;
    }
    let stem = segment.split('.').next().unwrap_or(segment);
    if ["con", "prn", "aux", "nul"]
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return false;
    }
    let prefix = stem.get(..3).is_some_and(|value| {
        value.eq_ignore_ascii_case("com") || value.eq_ignore_ascii_case("lpt")
    });
    let port = stem.get(3..).is_some_and(|value| {
        (value.len() == 1 && matches!(value.as_bytes()[0], b'1'..=b'9'))
            || matches!(value, "¹" | "²" | "³")
    });
    !(prefix && port)
}

fn invalid(message: impl Into<String>) -> CollectError {
    CollectError::Invalid(message.into())
}

fn valid(condition: bool, message: impl Into<String>) -> Result<(), CollectError> {
    condition.then_some(()).ok_or_else(|| invalid(message))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "pliego-css-collector-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(path.join("src")).expect("create collector fixture");
            Self(path)
        }

        fn write(&self, logical: &str, source: &str) {
            let path = join_portable(&self.0, logical);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create source parent");
            }
            fs::write(path, source).expect("write source fixture");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn topology() -> ApplicationTopology {
        ApplicationTopology::new()
            .source_root("src")
            .component(ApplicationComponent::new("app::dead").source_unit("src/dead.rs"))
            .component(ApplicationComponent::new("app::home").source_unit("src/home.rs"))
            .route(ApplicationRoute::new("home", "/").component("app::home"))
    }

    #[test]
    fn collects_canonical_file_owned_reachability() {
        let fixture = Fixture::new();
        fixture.write(
            "src/home.rs",
            "fn home() { let _ = pc!(\"grid gap-4\"); }\n",
        );
        fixture.write("src/dead.rs", "fn dead() { let _ = pc!(\"hidden\"); }\n");
        fixture.write("src/no_styles.rs", "fn helper() {}\n");

        let collected = topology().collect(&fixture.0).expect("collect topology");
        let document: serde_json::Value =
            serde_json::from_slice(collected.as_bytes()).expect("valid JSON");
        assert_eq!(collected.source_file_count(), 3);
        assert_eq!(collected.invocation_count(), 2);
        assert_eq!(document["schema"], 1);
        assert_eq!(document["applicationCoverage"], "complete");
        assert_eq!(document["components"][0]["id"], "app::dead");
        assert_eq!(document["components"][1]["id"], "app::home");
        assert_eq!(document["routes"][0]["components"][0], "app::home");
        assert!(collected.as_bytes().ends_with(b"\n"));
    }

    #[test]
    fn registration_order_does_not_change_bytes() {
        let fixture = Fixture::new();
        fixture.write("src/home.rs", "fn home() { let _ = pc!(\"grid\"); }\n");
        fixture.write("src/dead.rs", "fn dead() { let _ = pc!(\"hidden\"); }\n");
        let first = topology().collect(&fixture.0).expect("first collection");
        let second = ApplicationTopology::new()
            .component(ApplicationComponent::new("app::home").source_unit("src/home.rs"))
            .route(ApplicationRoute::new("home", "/").component("app::home"))
            .component(ApplicationComponent::new("app::dead").source_unit("src/dead.rs"))
            .source_root("src")
            .collect(&fixture.0)
            .expect("second collection");
        assert_eq!(first.as_bytes(), second.as_bytes());
    }

    #[test]
    fn rejects_unowned_and_stale_invocations() {
        let fixture = Fixture::new();
        fixture.write("src/home.rs", "fn home() { let _ = pc!(\"grid\"); }\n");
        let unowned = ApplicationTopology::new()
            .source_root("src")
            .component(ApplicationComponent::new("app::home"))
            .route(ApplicationRoute::new("home", "/").component("app::home"))
            .collect(&fixture.0)
            .expect_err("unowned invocation must fail");
        assert!(unowned.to_string().contains("unowned PliegoCSS invocation"));

        let stale = ApplicationTopology::new()
            .source_root("src")
            .component(
                ApplicationComponent::new("app::home")
                    .source_unit("src/home.rs")
                    .site("src/home.rs", 1, 2),
            )
            .route(ApplicationRoute::new("home", "/").component("app::home"))
            .collect(&fixture.0)
            .expect_err("stale site must fail");
        assert!(stale.to_string().contains("stale exact PliegoCSS site"));
    }

    #[test]
    fn exact_site_can_share_one_invocation_between_components() {
        let fixture = Fixture::new();
        let source = "fn style() { let _ = pc!(\"grid\"); }\n";
        fixture.write("src/shared.rs", source);
        let report = scan_source_named("src/shared.rs", source).expect("scan source");
        let range = report.invocations[0].range;
        let topology = ApplicationTopology::new()
            .source_root("src")
            .component(ApplicationComponent::new("app::a").site(
                "src/shared.rs",
                range.start.byte,
                range.end.byte,
            ))
            .component(ApplicationComponent::new("app::b").site(
                "src/shared.rs",
                range.start.byte,
                range.end.byte,
            ))
            .route(
                ApplicationRoute::new("home", "/")
                    .component("app::b")
                    .component("app::a"),
            );
        let collected = topology.collect(&fixture.0).expect("shared collection");
        let document: serde_json::Value =
            serde_json::from_slice(collected.as_bytes()).expect("valid JSON");
        assert_eq!(
            document["components"][0]["sites"][0],
            document["components"][1]["sites"][0]
        );
        assert_eq!(document["routes"][0]["components"][0], "app::a");
    }

    #[test]
    fn rejects_unknown_components_and_scanner_diagnostics() {
        let fixture = Fixture::new();
        fixture.write("src/home.rs", "fn home() { let _ = pc!(dynamic); }\n");
        let dangling = ApplicationTopology::new()
            .source_root("src")
            .component(ApplicationComponent::new("app::home").source_unit("src/home.rs"))
            .route(ApplicationRoute::new("home", "/").component("app::missing"))
            .collect(&fixture.0)
            .expect_err("unknown component must fail");
        assert!(dangling.to_string().contains("unknown component reference"));

        let diagnostic = ApplicationTopology::new()
            .source_root("src")
            .component(ApplicationComponent::new("app::home").source_unit("src/home.rs"))
            .route(ApplicationRoute::new("home", "/").component("app::home"))
            .collect(&fixture.0)
            .expect_err("invalid macro must fail");
        assert!(diagnostic.to_string().contains("PSC001"));
    }

    #[test]
    fn rejects_expanded_shared_site_ownership_above_the_schema_limit() {
        let fixture = Fixture::new();
        fixture.write(
            "src/shared.rs",
            "fn style() { let _ = pc!(\"grid\"); let _ = pc!(\"flex\"); \
             let _ = pc!(\"block\"); let _ = pc!(\"hidden\"); }\n",
        );
        let mut topology = ApplicationTopology::new().source_root("src");
        for index in 0..17_000 {
            topology = topology.component(
                ApplicationComponent::new(format!("component-{index}"))
                    .source_unit("src/shared.rs"),
            );
        }
        let error = topology
            .collect(&fixture.0)
            .expect_err("expanded ownership must be bounded");
        assert!(
            error
                .to_string()
                .contains("collected site ownership exceeds")
        );
    }

    #[test]
    fn rejects_unsafe_inventory_paths_and_duplicate_topology() {
        let fixture = Fixture::new();
        fixture.write("src/home.rs", "fn home() {}\n");
        let unsafe_path = ApplicationTopology::new()
            .source_root("../src")
            .collect(&fixture.0)
            .expect_err("traversal must fail");
        assert!(
            unsafe_path
                .to_string()
                .contains("invalid portable source path")
        );

        let duplicate = ApplicationTopology::new()
            .source_root("src")
            .component(ApplicationComponent::new("same").source_unit("src/home.rs"))
            .component(ApplicationComponent::new("same"))
            .collect(&fixture.0)
            .expect_err("duplicate component must fail");
        assert!(duplicate.to_string().contains("duplicate component id"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_inside_the_declared_inventory() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        fixture.write("outside.rs", "fn style() { let _ = pc!(\"grid\"); }\n");
        symlink(
            fixture.0.join("outside.rs"),
            fixture.0.join("src/linked.rs"),
        )
        .expect("create source symlink");
        let error = ApplicationTopology::new()
            .source_root("src")
            .component(ApplicationComponent::new("app::linked").source_unit("src/linked.rs"))
            .collect(&fixture.0)
            .expect_err("source symlink must fail");
        assert!(error.to_string().contains("rejects symlink or junction"));
    }

    #[cfg(windows)]
    #[test]
    fn rejects_symlinks_inside_the_declared_inventory() {
        use std::os::windows::fs::symlink_file;

        let fixture = Fixture::new();
        fixture.write("outside.rs", "fn style() { let _ = pc!(\"grid\"); }\n");
        if symlink_file(
            fixture.0.join("outside.rs"),
            fixture.0.join("src/linked.rs"),
        )
        .is_err()
        {
            return;
        }
        let error = ApplicationTopology::new()
            .source_root("src")
            .component(ApplicationComponent::new("app::linked").source_unit("src/linked.rs"))
            .collect(&fixture.0)
            .expect_err("source symlink must fail");
        assert!(error.to_string().contains("rejects symlink or junction"));
    }
}
