use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

/// Maximum accepted size of one reachability document (16 MiB).
pub const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_ITEMS: usize = 65_535;
const MAX_ID_BYTES: usize = 256;
const MAX_PATH_OR_NAME_BYTES: usize = 4 * 1024;
const INVALID: &str = "invalid reachability document";

/// Canonical, validated schema-1 application reachability document.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReachabilityDocument {
    pub(super) schema: u8,
    pub(super) application_coverage: String,
    pub(super) components: Vec<ReachabilityComponent>,
    pub(super) routes: Vec<ReachabilityRoute>,
    pub(super) islands: Vec<ReachabilityIsland>,
}

/// Exact source-site lookup used to decide whether an emitted style is reachable.
pub struct ReachabilityIndex {
    sites: BTreeMap<SiteKey, Vec<String>>,
    routes_by_component: BTreeMap<String, Vec<String>>,
    islands_by_component: BTreeMap<String, Vec<String>>,
}

/// Adapter-attested application owners for one exact compiler origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReachabilityOriginEvidence {
    reachable: bool,
    component_ids: Vec<String>,
    route_ids: Vec<String>,
    island_ids: Vec<String>,
}

type SiteKey = (String, usize, usize);

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ReachabilityComponent {
    pub(super) id: String,
    pub(super) sites: Vec<ReachabilitySite>,
}

#[derive(Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ReachabilitySite {
    pub(super) file: String,
    pub(super) byte_start: usize,
    pub(super) byte_end: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ReachabilityRoute {
    pub(super) id: String,
    pub(super) path: String,
    pub(super) components: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ReachabilityIsland {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) components: Vec<String>,
}

/// Parses, validates, and canonicalizes a schema-1 reachability document.
///
/// # Errors
///
/// Returns an error for malformed JSON, unknown fields, invalid portable paths,
/// duplicate identities, dangling component references, or defensive limit violations.
pub fn parse_reachability_document(source: &[u8]) -> Result<ReachabilityDocument, String> {
    valid(source.len() <= MAX_DOCUMENT_BYTES)?;
    let mut document: ReachabilityDocument =
        serde_json::from_slice(source).map_err(|error| error.to_string())?;
    document.validate_and_canonicalize()?;
    Ok(document)
}

impl ReachabilityDocument {
    /// Builds a deterministic lookup over every adapter-attested source site.
    #[must_use]
    pub fn source_index(&self) -> ReachabilityIndex {
        let mut routes_by_component = BTreeMap::<String, Vec<String>>::new();
        for route in &self.routes {
            for component_id in &route.components {
                routes_by_component
                    .entry(component_id.clone())
                    .or_default()
                    .push(route.id.clone());
            }
        }
        let mut islands_by_component = BTreeMap::<String, Vec<String>>::new();
        for island in &self.islands {
            for component_id in &island.components {
                islands_by_component
                    .entry(component_id.clone())
                    .or_default()
                    .push(island.id.clone());
            }
        }
        let mut sites = BTreeMap::<SiteKey, Vec<String>>::new();
        for component in &self.components {
            for site in &component.sites {
                sites
                    .entry((site.file.clone(), site.byte_start, site.byte_end))
                    .or_default()
                    .push(component.id.clone());
            }
        }
        ReachabilityIndex {
            sites,
            routes_by_component,
            islands_by_component,
        }
    }

    fn validate_and_canonicalize(&mut self) -> Result<(), String> {
        valid(self.schema == 1 && self.application_coverage == "complete")?;
        bounded_count(
            [self.components.len(), self.routes.len(), self.islands.len()]
                .into_iter()
                .chain(self.components.iter().map(|item| item.sites.len())),
        )?;
        bounded_count(
            self.routes
                .iter()
                .map(|item| item.components.len())
                .chain(self.islands.iter().map(|item| item.components.len())),
        )?;

        let mut component_ids = BTreeSet::new();
        for component in &mut self.components {
            validate_id(&component.id)?;
            unique(&mut component_ids, &component.id)?;
            component.sites.iter().try_for_each(validate_site)?;
            component.sites.sort();
            valid(!component.sites.windows(2).any(|pair| pair[0] == pair[1]))?;
        }

        let (mut route_ids, mut route_paths) = (BTreeSet::new(), BTreeSet::new());
        for route in &mut self.routes {
            validate_id(&route.id)?;
            validate_text(&route.path, MAX_PATH_OR_NAME_BYTES)?;
            unique(&mut route_ids, &route.id)?;
            unique(&mut route_paths, &route.path)?;
            validate_references(&mut route.components, &component_ids)?;
        }

        let (mut island_ids, mut island_names) = (BTreeSet::new(), BTreeSet::new());
        for island in &mut self.islands {
            validate_id(&island.id)?;
            validate_text(&island.name, MAX_PATH_OR_NAME_BYTES)?;
            unique(&mut island_ids, &island.id)?;
            unique(&mut island_names, &island.name)?;
            validate_references(&mut island.components, &component_ids)?;
        }

        self.components.sort_by(|a, b| a.id.cmp(&b.id));
        self.routes.sort_by(|a, b| a.id.cmp(&b.id));
        self.islands.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(())
    }
}

impl ReachabilityIndex {
    /// Returns exact component, route, island, and reachability evidence for one origin.
    ///
    /// # Errors
    ///
    /// Returns an error when the origin path is not portable or the adapter did
    /// not attest the exact source range as belonging to a component.
    pub fn origin_evidence(
        &self,
        file: &str,
        byte_start: usize,
        byte_end: usize,
    ) -> Result<ReachabilityOriginEvidence, String> {
        let component_ids = self.origin_components(file, byte_start, byte_end)?;
        let route_ids = component_ids
            .iter()
            .filter_map(|id| self.routes_by_component.get(id))
            .flatten()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let island_ids = component_ids
            .iter()
            .filter_map(|id| self.islands_by_component.get(id))
            .flatten()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        Ok(ReachabilityOriginEvidence {
            reachable: !route_ids.is_empty() || !island_ids.is_empty(),
            component_ids: component_ids.to_vec(),
            route_ids,
            island_ids,
        })
    }

    /// Classifies one exact compiler origin against the complete application graph.
    ///
    /// # Errors
    ///
    /// Returns an error when the origin path is not portable or the adapter did
    /// not attest the exact source range as belonging to a component.
    pub fn origin_is_reachable(
        &self,
        file: &str,
        byte_start: usize,
        byte_end: usize,
    ) -> Result<bool, String> {
        let component_ids = self.origin_components(file, byte_start, byte_end)?;
        Ok(component_ids.iter().any(|id| {
            self.routes_by_component
                .get(id)
                .is_some_and(|routes| !routes.is_empty())
                || self
                    .islands_by_component
                    .get(id)
                    .is_some_and(|islands| !islands.is_empty())
        }))
    }

    fn origin_components(
        &self,
        file: &str,
        byte_start: usize,
        byte_end: usize,
    ) -> Result<&[String], String> {
        let key = (origin_file_key(file)?, byte_start, byte_end);
        self.sites
            .get(&key)
            .map(Vec::as_slice)
            .ok_or_else(|| "origin has no component".to_owned())
    }
}

impl ReachabilityOriginEvidence {
    /// Returns whether at least one owning component belongs to a route or island root.
    #[must_use]
    pub const fn is_reachable(&self) -> bool {
        self.reachable
    }

    /// Returns sorted adapter-attested component owners.
    #[must_use]
    pub fn component_ids(&self) -> &[String] {
        &self.component_ids
    }

    /// Returns sorted routes that reference the owning components.
    #[must_use]
    pub fn route_ids(&self) -> &[String] {
        &self.route_ids
    }

    /// Returns sorted islands that reference the owning components.
    #[must_use]
    pub fn island_ids(&self) -> &[String] {
        &self.island_ids
    }
}

fn bounded_count(mut counts: impl Iterator<Item = usize>) -> Result<(), String> {
    let count = counts
        .try_fold(0_usize, usize::checked_add)
        .ok_or_else(|| INVALID.to_owned())?;
    valid(count <= MAX_ITEMS)
}

fn validate_id(id: &str) -> Result<(), String> {
    validate_text(id, MAX_ID_BYTES)
}

fn validate_text(value: &str, maximum: usize) -> Result<(), String> {
    valid(!value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control))
}

fn unique(set: &mut BTreeSet<String>, value: &str) -> Result<(), String> {
    valid(set.insert(value.into()))
}

fn validate_site(site: &ReachabilitySite) -> Result<(), String> {
    validate_path(&site.file)?;
    valid(site.byte_start < site.byte_end)
}

fn validate_path(path: &str) -> Result<(), String> {
    validate_text(path, MAX_PATH_OR_NAME_BYTES)?;
    valid(
        !(path.starts_with('/')
            || path.contains('\\')
            || path.split('/').any(|part| !portable_segment(part))),
    )
}

pub(super) fn origin_file_key(file: &str) -> Result<String, String> {
    #[cfg(windows)]
    let normalized = file.replace(char::from(92), "/");
    #[cfg(not(windows))]
    let normalized = file.to_owned();
    #[cfg(not(windows))]
    if file.contains(char::from(92)) {
        return Err("origin path is not portable".into());
    }
    if normalized.is_empty() {
        Err("origin path is not portable".into())
    } else {
        Ok(normalized)
    }
}

fn portable_segment(segment: &str) -> bool {
    if segment.is_empty()
        || matches!(segment, "." | "..")
        || segment.ends_with('.')
        || segment.ends_with(' ')
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
    let prefix = stem.get(..3).is_some_and(|prefix| {
        prefix.eq_ignore_ascii_case("com") || prefix.eq_ignore_ascii_case("lpt")
    });
    let port = stem.get(3..).is_some_and(|suffix| {
        (suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
            || matches!(suffix, "¹" | "²" | "³")
    });
    !(prefix && port)
}

fn validate_references(
    references: &mut Vec<String>,
    component_ids: &BTreeSet<String>,
) -> Result<(), String> {
    for id in &*references {
        validate_id(id)?;
    }
    references.sort();
    valid(!references.windows(2).any(|pair| pair[0] == pair[1]))?;
    if references.iter().any(|id| !component_ids.contains(id)) {
        return Err("unknown component reference".into());
    }
    Ok(())
}

fn valid(condition: bool) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| INVALID.to_owned())
}

#[cfg(all(test, not(feature = "package-verify")))]
#[path = "../../tests/internal/reachability.rs"]
mod tests;
