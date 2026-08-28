//! PliegoCSS adapter generated from the framework-owned PliegoRS product registry.

use std::fmt;
use std::path::Path;

use pliego_css_source::{CollectedReachability, ProductBundle, ProductTopology};
use pliego_ssg::ProductRegistry;

/// Reachability and bundle-plan bytes derived from one immutable product registry.
#[derive(Debug)]
pub struct GeneratedCssInputs {
    pub reachability: CollectedReachability,
    pub bundle_plan: Vec<u8>,
    pub bundles: Vec<ProductBundle>,
}

/// Failure to convert or collect one product registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterError {
    message: String,
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
    let topology_bytes = registry.to_topology_json().map_err(|error| invalid(error.to_string()))?;
    let topology = ProductTopology::from_json(&topology_bytes)
        .map_err(|error| invalid(error.to_string()))?;
    let generated = topology
        .collect_css_inputs(project_root, source_units)
        .map_err(|error| invalid(error.to_string()))?;
    let (reachability, bundle_plan, bundles) = generated.into_parts();
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

fn invalid(message: impl Into<String>) -> AdapterError {
    AdapterError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

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
                .map(ProductBundle::id)
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
                .filter(|bundle| bundle.emits_theme())
                .map(ProductBundle::id)
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
