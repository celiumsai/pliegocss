//! Closed Asset Plan ownership and route-composition contracts.
//!
//! [`parse_asset_plan`] validates the plan's closed schema, identifiers, ordering inputs, and
//! references. It does **not** establish provenance: callers must regenerate the plan from its exact
//! adjacent CSS and manifest bytes and compare byte-for-byte before trusting it. [`parse_ownership`]
//! then binds a closed schema-1 ownership sidecar to those exact plan bytes.
//! Adapters can use [`build_ownership_document`] to emit the same contract canonically.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_ITEMS: usize = 65_535;
const MAX_BUNDLE_ID_BYTES: usize = 64;
const MAX_ID_BYTES: usize = 256;
const MAX_PATH_OR_NAME_BYTES: usize = 4 * 1024;
const MAX_PACKAGE_ID_BYTES: usize = 128;

/// An error produced while parsing or validating an Asset Plan or ownership sidecar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnershipError {
    message: String,
}

impl OwnershipError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the stable human-readable reason for the validation failure.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for OwnershipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for OwnershipError {}

/// Rule-selection mode carried by a supported Asset Plan.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum AssetRuleSelection {
    /// Every compiled style remains in its source bundle.
    AllCompiled,
    /// Only complete `StyleId` sets reachable from the application graph remain.
    ReachableStyleIds,
    /// Complete `StyleId` sets remain when reachable or explicitly retained by policy.
    ReachableOrRetainedStyleIds,
}

/// One validated bundle from an Asset Plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetBundle {
    id: String,
    css_file: String,
    manifest_file: String,
    emits_theme: bool,
}

impl AssetBundle {
    /// Returns the portable bundle identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the canonical adjacent CSS file name.
    #[must_use]
    pub fn css_file(&self) -> &str {
        &self.css_file
    }

    /// Returns the canonical adjacent manifest file name.
    #[must_use]
    pub fn manifest_file(&self) -> &str {
        &self.manifest_file
    }

    /// Reports whether this bundle emits the shared theme layer.
    #[must_use]
    pub const fn emits_theme(&self) -> bool {
        self.emits_theme
    }
}

/// One validated route and its directly selected bundles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetRoute {
    id: String,
    path: String,
    bundle_ids: Vec<String>,
}

impl AssetRoute {
    /// Returns the namespaced route identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the application route path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns direct bundle identifiers in canonical plan order.
    #[must_use]
    pub fn bundle_ids(&self) -> &[String] {
        &self.bundle_ids
    }
}

/// One validated island and its selected bundles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetIsland {
    id: String,
    name: String,
    bundle_ids: Vec<String>,
}

impl AssetIsland {
    /// Returns the namespaced island identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the application island name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns selected bundle identifiers in canonical plan order.
    #[must_use]
    pub fn bundle_ids(&self) -> &[String] {
        &self.bundle_ids
    }
}

/// A structurally validated schema-1 or schema-2 Asset Plan.
///
/// This type records the exact input byte length and SHA-256 digest for ownership binding. Its
/// existence does not prove that the plan was regenerated from trusted adjacent artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetPlan {
    exact_bytes: usize,
    sha256: String,
    rule_selection: AssetRuleSelection,
    bundles: Vec<AssetBundle>,
    routes: Vec<AssetRoute>,
    islands: Vec<AssetIsland>,
}

impl AssetPlan {
    /// Returns the exact number of bytes parsed for this plan.
    #[must_use]
    pub const fn exact_bytes(&self) -> usize {
        self.exact_bytes
    }

    /// Returns the lowercase SHA-256 digest of the exact bytes parsed for this plan.
    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// Returns the plan's rule-selection contract.
    #[must_use]
    pub const fn rule_selection(&self) -> AssetRuleSelection {
        self.rule_selection
    }

    /// Returns bundles in canonical plan order: theme emitter first, then bundle ID.
    #[must_use]
    pub fn bundles(&self) -> &[AssetBundle] {
        &self.bundles
    }

    /// Returns routes ordered by route ID.
    #[must_use]
    pub fn routes(&self) -> &[AssetRoute] {
        &self.routes
    }

    /// Returns islands ordered by island ID.
    #[must_use]
    pub fn islands(&self) -> &[AssetIsland] {
        &self.islands
    }
}

/// All bundles owned by one package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageOwnership {
    package_id: String,
    bundle_ids: Vec<String>,
}

impl PackageOwnership {
    /// Returns the adapter-provided package identifier.
    #[must_use]
    pub fn package_id(&self) -> &str {
        &self.package_id
    }

    /// Returns this package's bundle identifiers in canonical plan order.
    #[must_use]
    pub fn bundle_ids(&self) -> &[String] {
        &self.bundle_ids
    }
}

/// One route view composed from its direct bundles and declared islands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposedRoute {
    route_id: String,
    path: String,
    island_ids: Vec<String>,
    bundle_ids: Vec<String>,
}

impl ComposedRoute {
    /// Returns the route identifier.
    #[must_use]
    pub fn route_id(&self) -> &str {
        &self.route_id
    }

    /// Returns the route path from the Asset Plan.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns composed island identifiers in canonical plan order.
    #[must_use]
    pub fn island_ids(&self) -> &[String] {
        &self.island_ids
    }

    /// Returns the deduplicated union of route and island bundles in canonical plan order.
    #[must_use]
    pub fn bundle_ids(&self) -> &[String] {
        &self.bundle_ids
    }
}

/// A validated schema-1 ownership sidecar resolved against an exact Asset Plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ownership {
    packages: Vec<PackageOwnership>,
    routes: Vec<ComposedRoute>,
}

/// One adapter-provided bundle-to-package ownership mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BundlePackageInput<'a> {
    bundle_id: &'a str,
    package_id: &'a str,
}

impl<'a> BundlePackageInput<'a> {
    /// Creates one bundle-to-package ownership mapping.
    #[must_use]
    pub const fn new(bundle_id: &'a str, package_id: &'a str) -> Self {
        Self {
            bundle_id,
            package_id,
        }
    }

    /// Returns the Asset Plan bundle identifier being assigned.
    #[must_use]
    pub const fn bundle_id(&self) -> &'a str {
        self.bundle_id
    }

    /// Returns the adapter package identifier that owns the bundle.
    #[must_use]
    pub const fn package_id(&self) -> &'a str {
        self.package_id
    }
}

/// One adapter-provided route composition and its active islands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteCompositionInput<'a> {
    route_id: &'a str,
    island_ids: &'a [&'a str],
}

impl<'a> RouteCompositionInput<'a> {
    /// Creates one route composition from a route ID and its active island IDs.
    #[must_use]
    pub const fn new(route_id: &'a str, island_ids: &'a [&'a str]) -> Self {
        Self {
            route_id,
            island_ids,
        }
    }

    /// Returns the Asset Plan route identifier being composed.
    #[must_use]
    pub const fn route_id(&self) -> &'a str {
        self.route_id
    }

    /// Returns the active island identifiers supplied by the adapter.
    #[must_use]
    pub const fn island_ids(&self) -> &'a [&'a str] {
        self.island_ids
    }
}

impl Ownership {
    /// Returns package ownership records ordered by package ID.
    #[must_use]
    pub fn packages(&self) -> &[PackageOwnership] {
        &self.packages
    }

    /// Returns composed route views in canonical Asset Plan route order.
    #[must_use]
    pub fn composed_routes(&self) -> &[ComposedRoute] {
        &self.routes
    }
}

/// Parses and structurally validates a supported closed schema-1 or schema-2 Asset Plan.
///
/// Callers must separately regenerate the plan from exact adjacent CSS and manifest bytes and
/// compare the regenerated bytes before treating this parsed value as trusted.
///
/// # Errors
///
/// Returns an error for malformed or non-canonical contracts, unknown or missing fields, duplicate
/// or dangling identifiers, invalid references, and defensive limit violations.
pub fn parse_asset_plan(source: &[u8]) -> Result<AssetPlan, OwnershipError> {
    require_document_size(source, "asset plan")?;
    let document: AssetPlanDocument = serde_json::from_slice(source)
        .map_err(|error| fail(format!("invalid asset plan JSON: {error}")))?;
    validate_asset_plan(document, source)
}

/// Parses a closed schema-1 ownership sidecar and resolves it against an exact Asset Plan.
///
/// Every plan bundle must have exactly one package owner and every plan route must have exactly one
/// composition. Route views may overlap and are never interpreted as attribution partitions.
///
/// # Errors
///
/// Returns an error for malformed or open contracts, byte/hash drift, incomplete or ambiguous
/// ownership, duplicate or dangling route/island references, invalid package IDs, and limits.
pub fn parse_ownership(source: &[u8], plan: &AssetPlan) -> Result<Ownership, OwnershipError> {
    require_document_size(source, "ownership sidecar")?;
    let document: OwnershipDocument = serde_json::from_slice(source)
        .map_err(|error| fail(format!("invalid ownership JSON: {error}")))?;
    validate_ownership(document, plan)
}

/// Builds a canonical schema-1 ownership sidecar from adapter-provided topology.
///
/// Bundle mappings are ordered by bundle ID; route compositions and their islands are ordered by
/// ID. The result uses two-space pretty JSON, ends in exactly one LF, and is parsed again through
/// [`parse_ownership`] before it is returned so producer and consumer share one validation contract.
///
/// # Errors
///
/// Returns an error when mappings or compositions violate the closed ownership contract, when the
/// canonical document exceeds defensive limits, or if serialization fails.
pub fn build_ownership_document(
    plan: &AssetPlan,
    mappings: &[BundlePackageInput<'_>],
    compositions: &[RouteCompositionInput<'_>],
) -> Result<Vec<u8>, OwnershipError> {
    let mut bundle_packages = mappings
        .iter()
        .map(|mapping| BundlePackageDocument {
            bundle_id: mapping.bundle_id.to_owned(),
            package_id: mapping.package_id.to_owned(),
        })
        .collect::<Vec<_>>();
    bundle_packages.sort_by(|left, right| left.bundle_id.cmp(&right.bundle_id));

    let mut route_compositions = compositions
        .iter()
        .map(|composition| {
            let mut island_ids = composition
                .island_ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            island_ids.sort();
            RouteCompositionDocument {
                route_id: composition.route_id.to_owned(),
                island_ids,
            }
        })
        .collect::<Vec<_>>();
    route_compositions.sort_by(|left, right| left.route_id.cmp(&right.route_id));

    let document = OwnershipDocument {
        schema_version: 1,
        ownership_coverage: "adapter-attested-complete".to_owned(),
        asset_plan_bytes: plan.exact_bytes as u64,
        asset_plan_sha256: plan.sha256.clone(),
        bundle_packages,
        route_compositions,
    };
    let mut output = serde_json::to_vec_pretty(&document)
        .map_err(|error| fail(format!("cannot serialize ownership document: {error}")))?;
    output.push(b'\n');
    parse_ownership(&output, plan)?;
    Ok(output)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AssetPlanDocument {
    schema_version: u8,
    manifest_schema_version: u8,
    graph_schema_version: u8,
    rule_selection: AssetRuleSelection,
    origin_coverage: String,
    application_coverage: String,
    style_id_format_version: u16,
    class_name_format_version: u16,
    theme_id_format_version: u16,
    theme_id: String,
    targets: String,
    format: String,
    bundles: Vec<AssetBundleDocument>,
    routes: Vec<AssetRouteDocument>,
    islands: Vec<AssetIslandDocument>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AssetBundleDocument {
    id: String,
    css_file: String,
    manifest_file: String,
    emits_theme: bool,
    css_bytes: u64,
    css_sha256: String,
    manifest_bytes: u64,
    manifest_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetRouteDocument {
    id: String,
    path: String,
    bundles: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetIslandDocument {
    id: String,
    name: String,
    bundles: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct OwnershipDocument {
    schema_version: u8,
    ownership_coverage: String,
    asset_plan_bytes: u64,
    asset_plan_sha256: String,
    bundle_packages: Vec<BundlePackageDocument>,
    route_compositions: Vec<RouteCompositionDocument>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BundlePackageDocument {
    bundle_id: String,
    package_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RouteCompositionDocument {
    route_id: String,
    island_ids: Vec<String>,
}

#[allow(clippy::too_many_lines)]
fn validate_asset_plan(
    document: AssetPlanDocument,
    source: &[u8],
) -> Result<AssetPlan, OwnershipError> {
    require(
        matches!(
            (document.schema_version, document.rule_selection),
            (
                1,
                AssetRuleSelection::AllCompiled | AssetRuleSelection::ReachableStyleIds
            ) | (2, AssetRuleSelection::ReachableOrRetainedStyleIds)
        ),
        "asset plan schemaVersion and ruleSelection are incompatible",
    )?;
    let expected_graph = match document.manifest_schema_version {
        4 => 1,
        5 => 2,
        _ => return Err(fail("asset plan manifestSchemaVersion must be 4 or 5")),
    };
    require(
        document.graph_schema_version == expected_graph,
        "asset plan manifest and graph schema versions are incompatible",
    )?;
    require(
        document.origin_coverage == "compiler-verified-complete",
        "asset plan originCoverage must be compiler-verified-complete",
    )?;
    require(
        document.application_coverage == "adapter-attested-complete",
        "asset plan applicationCoverage must be adapter-attested-complete",
    )?;
    require(
        document.style_id_format_version > 0
            && document.class_name_format_version > 0
            && document.theme_id_format_version > 0,
        "asset plan identity format versions must be non-zero",
    )?;
    validate_lower_hex(&document.theme_id, 32, "asset plan themeId")?;
    require(
        matches!(
            document.targets.as_str(),
            "baseline-widely" | "modern" | "none"
        ),
        "asset plan targets contract is unsupported",
    )?;
    require(
        matches!(document.format.as_str(), "minified" | "pretty"),
        "asset plan format contract is unsupported",
    )?;
    require(
        !document.bundles.is_empty(),
        "asset plan requires at least one bundle",
    )?;

    let mut item_count = checked_add(document.bundles.len(), document.routes.len())?;
    item_count = checked_add(item_count, document.islands.len())?;
    require(item_count <= MAX_ITEMS, "asset plan item limit exceeded")?;

    let mut bundle_ids = BTreeSet::new();
    let mut css_files = BTreeSet::new();
    let mut manifest_files = BTreeSet::new();
    let mut theme_count = 0_usize;
    let mut bundles = Vec::with_capacity(document.bundles.len());
    for bundle in document.bundles {
        validate_bundle_id(&bundle.id)?;
        require(
            bundle_ids.insert(bundle.id.clone()),
            format!("duplicate asset plan bundle `{}`", bundle.id),
        )?;
        require(
            bundle.css_file == format!("{}.css", bundle.id),
            format!(
                "asset plan bundle `{}` has a noncanonical cssFile",
                bundle.id
            ),
        )?;
        require(
            bundle.manifest_file == format!("{}.manifest.json", bundle.id),
            format!(
                "asset plan bundle `{}` has a noncanonical manifestFile",
                bundle.id
            ),
        )?;
        require(
            css_files.insert(bundle.css_file.clone()),
            "duplicate asset plan cssFile",
        )?;
        require(
            manifest_files.insert(bundle.manifest_file.clone()),
            "duplicate asset plan manifestFile",
        )?;
        require(
            bundle.css_bytes <= MAX_DOCUMENT_BYTES as u64,
            format!("asset plan bundle `{}` CSS exceeds 16 MiB", bundle.id),
        )?;
        require(
            bundle.manifest_bytes <= MAX_DOCUMENT_BYTES as u64,
            format!("asset plan bundle `{}` manifest exceeds 16 MiB", bundle.id),
        )?;
        validate_lower_hex(&bundle.css_sha256, 64, "asset plan CSS SHA-256")?;
        validate_lower_hex(&bundle.manifest_sha256, 64, "asset plan manifest SHA-256")?;
        theme_count = checked_add(theme_count, usize::from(bundle.emits_theme))?;
        bundles.push(AssetBundle {
            id: bundle.id,
            css_file: bundle.css_file,
            manifest_file: bundle.manifest_file,
            emits_theme: bundle.emits_theme,
        });
    }
    require(
        theme_count <= 1,
        "asset plan accepts at most one theme-emitting bundle",
    )?;
    bundles.sort_by(|left, right| {
        right
            .emits_theme
            .cmp(&left.emits_theme)
            .then_with(|| left.id.cmp(&right.id))
    });
    let bundle_indices = bundles
        .iter()
        .enumerate()
        .map(|(index, bundle)| (bundle.id.as_str(), index))
        .collect::<BTreeMap<_, _>>();

    let mut route_ids = BTreeSet::new();
    let mut route_paths = BTreeSet::new();
    let mut routes = Vec::with_capacity(document.routes.len());
    for route in document.routes {
        validate_namespaced_id(&route.id, "route:", "route ID")?;
        validate_text(&route.path, MAX_PATH_OR_NAME_BYTES, "route path")?;
        require(
            route_ids.insert(route.id.clone()),
            "duplicate asset plan route ID",
        )?;
        require(
            route_paths.insert(route.path.clone()),
            "duplicate asset plan route path",
        )?;
        item_count = checked_add(item_count, route.bundles.len())?;
        require(item_count <= MAX_ITEMS, "asset plan item limit exceeded")?;
        let bundle_ids = canonical_bundle_references(
            route.bundles,
            &bundle_indices,
            &bundles,
            "asset plan route",
        )?;
        routes.push(AssetRoute {
            id: route.id,
            path: route.path,
            bundle_ids,
        });
    }
    routes.sort_by(|left, right| left.id.cmp(&right.id));

    let mut island_ids = BTreeSet::new();
    let mut island_names = BTreeSet::new();
    let mut islands = Vec::with_capacity(document.islands.len());
    for island in document.islands {
        validate_namespaced_id(&island.id, "island:", "island ID")?;
        validate_text(&island.name, MAX_PATH_OR_NAME_BYTES, "island name")?;
        require(
            island_ids.insert(island.id.clone()),
            "duplicate asset plan island ID",
        )?;
        require(
            island_names.insert(island.name.clone()),
            "duplicate asset plan island name",
        )?;
        item_count = checked_add(item_count, island.bundles.len())?;
        require(item_count <= MAX_ITEMS, "asset plan item limit exceeded")?;
        let bundle_ids = canonical_bundle_references(
            island.bundles,
            &bundle_indices,
            &bundles,
            "asset plan island",
        )?;
        islands.push(AssetIsland {
            id: island.id,
            name: island.name,
            bundle_ids,
        });
    }
    islands.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(AssetPlan {
        exact_bytes: source.len(),
        sha256: sha256_hex(source),
        rule_selection: document.rule_selection,
        bundles,
        routes,
        islands,
    })
}

#[allow(clippy::too_many_lines)]
fn validate_ownership(
    document: OwnershipDocument,
    plan: &AssetPlan,
) -> Result<Ownership, OwnershipError> {
    require(
        document.schema_version == 1,
        "ownership schemaVersion must be 1",
    )?;
    require(
        document.ownership_coverage == "adapter-attested-complete",
        "ownershipCoverage must be adapter-attested-complete",
    )?;
    require(
        document.asset_plan_bytes == plan.exact_bytes as u64,
        "ownership assetPlanBytes does not match exact Asset Plan bytes",
    )?;
    validate_lower_hex(&document.asset_plan_sha256, 64, "ownership assetPlanSha256")?;
    require(
        document.asset_plan_sha256 == plan.sha256,
        "ownership assetPlanSha256 does not match exact Asset Plan bytes",
    )?;

    let mut item_count = checked_add(
        document.bundle_packages.len(),
        document.route_compositions.len(),
    )?;
    require(item_count <= MAX_ITEMS, "ownership item limit exceeded")?;
    require(
        document.bundle_packages.len() == plan.bundles.len(),
        "ownership must map every Asset Plan bundle exactly once",
    )?;
    require(
        document.route_compositions.len() == plan.routes.len(),
        "ownership must compose every Asset Plan route exactly once",
    )?;

    let bundle_indices = plan
        .bundles
        .iter()
        .enumerate()
        .map(|(index, bundle)| (bundle.id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut mapped_bundles = BTreeSet::new();
    let mut package_bundles = BTreeMap::<String, BTreeSet<usize>>::new();
    for mapping in document.bundle_packages {
        validate_package_id(&mapping.package_id)?;
        let bundle_index = bundle_indices
            .get(mapping.bundle_id.as_str())
            .copied()
            .ok_or_else(|| fail(format!("unknown ownership bundle `{}`", mapping.bundle_id)))?;
        require(
            mapped_bundles.insert(bundle_index),
            format!(
                "duplicate ownership mapping for bundle `{}`",
                mapping.bundle_id
            ),
        )?;
        package_bundles
            .entry(mapping.package_id)
            .or_default()
            .insert(bundle_index);
    }
    require(
        mapped_bundles.len() == plan.bundles.len(),
        "ownership bundle mapping is incomplete",
    )?;
    let packages = package_bundles
        .into_iter()
        .map(|(package_id, indices)| PackageOwnership {
            package_id,
            bundle_ids: indices
                .into_iter()
                .map(|index| plan.bundles[index].id.clone())
                .collect(),
        })
        .collect();

    let route_indices = plan
        .routes
        .iter()
        .enumerate()
        .map(|(index, route)| (route.id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let island_indices = plan
        .islands
        .iter()
        .enumerate()
        .map(|(index, island)| (island.id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut route_islands = vec![None; plan.routes.len()];
    for composition in document.route_compositions {
        let route_index = route_indices
            .get(composition.route_id.as_str())
            .copied()
            .ok_or_else(|| {
                fail(format!(
                    "unknown ownership route `{}`",
                    composition.route_id
                ))
            })?;
        require(
            route_islands[route_index].is_none(),
            format!(
                "duplicate ownership composition for route `{}`",
                composition.route_id
            ),
        )?;
        item_count = checked_add(item_count, composition.island_ids.len())?;
        require(item_count <= MAX_ITEMS, "ownership item limit exceeded")?;
        let mut selected = BTreeSet::new();
        for island_id in composition.island_ids {
            let island_index = island_indices
                .get(island_id.as_str())
                .copied()
                .ok_or_else(|| fail(format!("unknown ownership island `{island_id}`")))?;
            require(
                selected.insert(island_index),
                format!("duplicate ownership island `{island_id}`"),
            )?;
        }
        route_islands[route_index] = Some(selected);
    }
    require(
        route_islands.iter().all(Option::is_some),
        "ownership route composition is incomplete",
    )?;

    let mut composed_memberships = 0_usize;
    let mut routes = Vec::with_capacity(plan.routes.len());
    for (route_index, route) in plan.routes.iter().enumerate() {
        let selected_islands = route_islands[route_index]
            .take()
            .expect("complete route compositions validated");
        let island_ids = selected_islands
            .iter()
            .map(|index| plan.islands[*index].id.clone())
            .collect::<Vec<_>>();
        let mut selected_bundles = route
            .bundle_ids
            .iter()
            .filter_map(|id| bundle_indices.get(id.as_str()).copied())
            .collect::<BTreeSet<_>>();
        for island_index in selected_islands {
            for bundle_id in &plan.islands[island_index].bundle_ids {
                let bundle_index = bundle_indices
                    .get(bundle_id.as_str())
                    .copied()
                    .expect("validated plan bundle reference");
                selected_bundles.insert(bundle_index);
            }
        }
        composed_memberships = checked_add(composed_memberships, selected_bundles.len())?;
        require(
            composed_memberships <= MAX_ITEMS,
            "ownership composed bundle membership limit exceeded",
        )?;
        routes.push(ComposedRoute {
            route_id: route.id.clone(),
            path: route.path.clone(),
            island_ids,
            bundle_ids: selected_bundles
                .into_iter()
                .map(|index| plan.bundles[index].id.clone())
                .collect(),
        });
    }

    Ok(Ownership { packages, routes })
}

fn canonical_bundle_references(
    references: Vec<String>,
    indices: &BTreeMap<&str, usize>,
    bundles: &[AssetBundle],
    role: &str,
) -> Result<Vec<String>, OwnershipError> {
    let mut selected = BTreeSet::new();
    for bundle_id in references {
        let index = indices
            .get(bundle_id.as_str())
            .copied()
            .ok_or_else(|| fail(format!("{role} references unknown bundle `{bundle_id}`")))?;
        require(
            selected.insert(index),
            format!("{role} contains duplicate bundle `{bundle_id}`"),
        )?;
    }
    Ok(selected
        .into_iter()
        .map(|index| bundles[index].id.clone())
        .collect())
}

fn validate_bundle_id(id: &str) -> Result<(), OwnershipError> {
    let bytes = id.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= MAX_BUNDLE_ID_BYTES
        && bytes.first().is_some_and(u8::is_ascii_lowercase)
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
        valid && !reserved,
        format!(
            "unsafe bundle name `{id}`; use 1-64 lowercase kebab-case ASCII characters and avoid reserved device names"
        ),
    )
}

fn validate_package_id(id: &str) -> Result<(), OwnershipError> {
    require(
        !id.is_empty()
            && id.len() <= MAX_PACKAGE_ID_BYTES
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')),
        format!(
            "invalid packageId `{id}`; use 1-128 ASCII alphanumeric, underscore, or hyphen characters"
        ),
    )
}

fn validate_namespaced_id(id: &str, prefix: &str, role: &str) -> Result<(), OwnershipError> {
    let value = id
        .strip_prefix(prefix)
        .ok_or_else(|| fail(format!("invalid {role}")))?;
    validate_text(value, MAX_ID_BYTES, role)
}

fn validate_text(value: &str, maximum: usize, role: &str) -> Result<(), OwnershipError> {
    require(
        !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control),
        format!("invalid {role}"),
    )
}

fn validate_lower_hex(value: &str, length: usize, role: &str) -> Result<(), OwnershipError> {
    require(
        value.len() == length
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')),
        format!("invalid {role}"),
    )
}

fn checked_add(left: usize, right: usize) -> Result<usize, OwnershipError> {
    left.checked_add(right)
        .ok_or_else(|| fail("item limit exceeded"))
}

fn require_document_size(source: &[u8], role: &str) -> Result<(), OwnershipError> {
    require(
        source.len() <= MAX_DOCUMENT_BYTES,
        format!("{role} exceeds 16 MiB"),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), OwnershipError> {
    condition.then_some(()).ok_or_else(|| fail(message))
}

fn fail(message: impl Into<String>) -> OwnershipError {
    OwnershipError::new(message)
}
