//! PliegoCSS adapter generated from the framework-owned PliegoRS product registry.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use pliego_css_source::{
    ApplicationComponent, ApplicationIsland, ApplicationRoute, ApplicationTopology,
    CollectedReachability,
};
use pliego_ssg::ProductRegistry;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// One automatically derived physical CSS partition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedBundle {
    pub id: String,
    pub sources: Vec<String>,
    pub emit_theme: bool,
}

/// Reachability and bundle-plan bytes derived from one immutable product registry.
#[derive(Debug)]
pub struct GeneratedCssInputs {
    pub reachability: CollectedReachability,
    pub bundle_plan: Vec<u8>,
    pub bundles: Vec<GeneratedBundle>,
}

/// Failure to convert or collect one product registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterError {
    message: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Root {
    Route(String),
    Island(String),
}

/// Generates canonical reachability and an automatic source partition from one product registry.
pub fn generate_css_inputs(
    registry: &ProductRegistry,
    project_root: &Path,
) -> Result<GeneratedCssInputs, AdapterError> {
    generate_css_inputs_internal(registry, project_root, None)
}

/// Generates CSS inputs while scanning every Rust source unit attested by Cargo/rustc.
pub fn generate_css_inputs_from_inventory(
    registry: &ProductRegistry,
    project_root: &Path,
    source_units: &[String],
) -> Result<GeneratedCssInputs, AdapterError> {
    if source_units.is_empty() {
        return Err(invalid("Cargo source inventory cannot be empty"));
    }
    generate_css_inputs_internal(registry, project_root, Some(source_units))
}

fn generate_css_inputs_internal(
    registry: &ProductRegistry,
    project_root: &Path,
    source_units: Option<&[String]>,
) -> Result<GeneratedCssInputs, AdapterError> {
    registry.validate().map_err(|error| invalid(error.to_string()))?;
    let topology = topology(registry, source_units);
    let reachability = topology
        .collect(project_root)
        .map_err(|error| invalid(error.to_string()))?;
    let bundles = partition(registry)?;
    let bundle_plan = render_bundle_plan(&bundles)?;
    Ok(GeneratedCssInputs {
        reachability,
        bundle_plan,
        bundles,
    })
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AdapterError {}

fn topology(registry: &ProductRegistry, inventory: Option<&[String]>) -> ApplicationTopology {
    let mut topology = ApplicationTopology::new();
    let mut source_roots = BTreeSet::new();
    for component in registry.components() {
        let mut application_component = ApplicationComponent::new(component.id());
        for source in component.source_units() {
            application_component = application_component.source_unit(source);
            let path = Path::new(source);
            let root = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .and_then(Path::to_str)
                .unwrap_or(source)
                .replace('\\', "/");
            source_roots.insert(root);
        }
        topology = topology.component(application_component);
    }
    if let Some(inventory) = inventory {
        source_roots = inventory.iter().cloned().collect();
    }
    for source_root in source_roots {
        topology = topology.source_root(source_root);
    }
    for route in registry.routes() {
        let mut application_route = ApplicationRoute::new(route.id(), route.path());
        for component in route.components() {
            application_route = application_route.component(component);
        }
        topology = topology.route(application_route);
    }
    for island in registry.islands() {
        let mut application_island = ApplicationIsland::new(island.id(), island.name());
        for component in island.components() {
            application_island = application_island.component(component);
        }
        topology = topology.island(application_island);
    }
    topology
}

fn partition(registry: &ProductRegistry) -> Result<Vec<GeneratedBundle>, AdapterError> {
    let mut memberships = registry
        .components()
        .iter()
        .map(|component| (component.id(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for route in registry.routes() {
        for component in route.components() {
            memberships
                .get_mut(component.as_str())
                .ok_or_else(|| invalid(format!("unknown route component `{component}`")))?
                .insert(Root::Route(route.id().to_owned()));
        }
    }
    for island in registry.islands() {
        for component in island.components() {
            memberships
                .get_mut(component.as_str())
                .ok_or_else(|| invalid(format!("unknown island component `{component}`")))?
                .insert(Root::Island(island.id().to_owned()));
        }
    }

    let mut sources = BTreeMap::<String, BTreeSet<Root>>::new();
    for component in registry.components() {
        let roots = memberships
            .get(component.id())
            .ok_or_else(|| invalid(format!("component `{}` has no membership", component.id())))?;
        for source in component.source_units() {
            sources
                .entry(source.clone())
                .or_default()
                .extend(roots.iter().cloned());
        }
    }

    let mut groups = BTreeMap::<BTreeSet<Root>, BTreeSet<String>>::new();
    for (source, roots) in sources {
        groups.entry(roots).or_default().insert(source);
    }
    let route_ids = registry
        .routes()
        .iter()
        .map(|route| route.id().to_owned())
        .collect::<BTreeSet<_>>();
    let mut bundles = BTreeMap::new();
    let mut universal = Vec::new();
    for (roots, sources) in groups {
        let id = bundle_id(&roots, &route_ids);
        if bundles.contains_key(&id) {
            return Err(invalid(format!("automatic bundle ID collision `{id}`")));
        }
        if route_ids
            .iter()
            .all(|route| roots.contains(&Root::Route(route.clone())))
        {
            universal.push(id.clone());
        }
        bundles.insert(
            id.clone(),
            GeneratedBundle {
                id,
                sources: sources.into_iter().collect(),
                emit_theme: false,
            },
        );
    }
    universal.sort();
    let theme_bundle = universal
        .first()
        .ok_or_else(|| invalid("automatic partition requires one bundle shared by every route"))?;
    bundles
        .get_mut(theme_bundle)
        .expect("selected theme bundle came from the bundle map")
        .emit_theme = true;
    Ok(bundles.into_values().collect())
}

fn bundle_id(roots: &BTreeSet<Root>, route_ids: &BTreeSet<String>) -> String {
    if roots.is_empty() {
        return "unreachable".to_owned();
    }
    let routes = roots
        .iter()
        .filter_map(|root| match root {
            Root::Route(id) => Some(id.as_str()),
            Root::Island(_) => None,
        })
        .collect::<Vec<_>>();
    let islands = roots
        .iter()
        .filter_map(|root| match root {
            Root::Island(id) => Some(id.as_str()),
            Root::Route(_) => None,
        })
        .collect::<Vec<_>>();
    if islands.is_empty()
        && routes.len() == route_ids.len()
        && routes.iter().all(|route| route_ids.contains(*route))
    {
        return "shared".to_owned();
    }
    if routes.len() == 1 && islands.is_empty() {
        return format!("route-{}", routes[0]);
    }
    if routes.is_empty() && islands.len() == 1 {
        return format!("island-{}", islands[0]);
    }
    let mut digest = Sha256::new();
    for root in roots {
        match root {
            Root::Route(id) => digest.update(format!("route\0{id}\0")),
            Root::Island(id) => digest.update(format!("island\0{id}\0")),
        }
    }
    format!("shared-{}", &format!("{:x}", digest.finalize())[..12])
}

fn render_bundle_plan(bundles: &[GeneratedBundle]) -> Result<Vec<u8>, AdapterError> {
    let mut output = String::from(
        "schema = 1\ntargets = \"modern\"\nformat = \"minified\"\n\n[theme]\nkind = \"seed\"\n",
    );
    for bundle in bundles {
        output.push_str("\n[bundles.");
        output.push_str(&bundle.id);
        output.push_str("]\nsources = [");
        for (index, source) in bundle.sources.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            output.push_str(
                &serde_json::to_string(source)
                    .map_err(|error| invalid(format!("cannot encode bundle source: {error}")))?,
            );
        }
        output.push_str("]\nemit-theme = ");
        output.push_str(if bundle.emit_theme { "true" } else { "false" });
        output.push('\n');
    }
    Ok(output.into_bytes())
}

fn invalid(message: impl Into<String>) -> AdapterError {
    AdapterError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product::application_registry;

    #[test]
    fn registry_derives_reachability_and_physical_partitions() {
        let generated = generate_css_inputs(
            &application_registry(),
            Path::new(env!("CARGO_MANIFEST_DIR")),
        )
        .unwrap();
        assert_eq!(generated.reachability.source_file_count(), 6);
        assert_eq!(generated.reachability.invocation_count(), 5);
        assert_eq!(
            generated
                .bundles
                .iter()
                .map(|bundle| bundle.id.as_str())
                .collect::<Vec<_>>(),
            [
                "island-visit-counter",
                "route-home",
                "route-visit",
                "shared",
                "unreachable",
            ]
        );
        assert_eq!(
            generated
                .bundles
                .iter()
                .filter(|bundle| bundle.emit_theme)
                .map(|bundle| bundle.id.as_str())
                .collect::<Vec<_>>(),
            ["shared"]
        );
        let plan = String::from_utf8(generated.bundle_plan).unwrap();
        assert!(plan.contains("[bundles.shared]\n"));
        assert!(plan.contains("sources = [\"src/styles/global.rs\"]\nemit-theme = true"));
        assert!(plan.contains("[bundles.unreachable]\n"));
        assert!(plan.contains("sources = [\"src/styles/dead.rs\"]\nemit-theme = false"));
    }

    #[test]
    fn cargo_inventory_rejects_an_unregistered_compiled_macro_site() {
        let registry = application_registry();
        let mut source_units = registry
            .components()
            .iter()
            .flat_map(|component| component.source_units().iter().cloned())
            .collect::<BTreeSet<_>>();
        source_units.insert("src/main.rs".to_owned());
        let error = generate_css_inputs_from_inventory(
            &registry,
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &source_units.into_iter().collect::<Vec<_>>(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("unowned PliegoCSS invocation"));
    }
}
