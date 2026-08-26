//! Emits reachability and bundle partitions from the PliegoRS product registry.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use pliego_css_source::ProductBundle;
use pliegocss_pliegors_smoke::css_adapter::generate_css_inputs_from_inventory;
use pliegocss_pliegors_smoke::product::application_registry;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CollectorSummary<'a> {
    schema: &'static str,
    cargo_targets: usize,
    cargo_rust_files: usize,
    collected_rust_files: usize,
    style_sites: usize,
    reachability_bytes: usize,
    bundle_plan_bytes: usize,
    bundles: &'a [ProductBundle],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CargoSourceInventory {
    schema_version: u8,
    coverage: String,
    targets: Vec<CargoTargetInventory>,
    source_units: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CargoTargetInventory {
    id: String,
    source_units: Vec<String>,
}

fn required_path(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
    role: &str,
) -> PathBuf {
    arguments
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("missing {role} path"))
}

fn read_inventory(path: &Path) -> Result<CargoSourceInventory, Box<dyn std::error::Error>> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 16 * 1024 * 1024
    {
        return Err("invalid or oversized Cargo source inventory".into());
    }
    let inventory: CargoSourceInventory = serde_json::from_slice(&std::fs::read(path)?)?;
    let expected_targets = ["browser-client", "site-lib", "site-ssg"];
    if inventory.schema_version != 1
        || inventory.coverage != "rustc-dep-info"
        || inventory.targets.len() != expected_targets.len()
        || inventory.source_units.is_empty()
        || inventory.source_units.len() > 65_535
        || !strictly_sorted(&inventory.source_units)
    {
        return Err("invalid Cargo source inventory contract".into());
    }
    let mut union = BTreeSet::new();
    for (target, expected_id) in inventory.targets.iter().zip(expected_targets) {
        if target.id != expected_id
            || target.source_units.is_empty()
            || !strictly_sorted(&target.source_units)
        {
            return Err("invalid Cargo target source inventory".into());
        }
        union.extend(target.source_units.iter().cloned());
    }
    if union.into_iter().collect::<Vec<_>>() != inventory.source_units {
        return Err("Cargo target inventories do not equal the source-unit union".into());
    }
    Ok(inventory)
}

fn strictly_sorted(values: &[String]) -> bool {
    values
        .windows(2)
        .all(|pair| pair[0] < pair[1])
        && values.iter().all(|value| !value.is_empty())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let project_root = required_path(&mut arguments, "project root");
    let cargo_inventory_path = required_path(&mut arguments, "Cargo source inventory");
    let output = required_path(&mut arguments, "reachability output");
    let bundle_plan_output = required_path(&mut arguments, "bundle plan output");
    if arguments.next().is_some() {
        return Err("unexpected collector argument".into());
    }
    let cargo_inventory = read_inventory(&cargo_inventory_path)?;
    let generated = generate_css_inputs_from_inventory(
        &application_registry(),
        &project_root,
        &cargo_inventory.source_units,
    )?;
    std::fs::write(&output, generated.reachability.as_bytes())?;
    std::fs::write(&bundle_plan_output, &generated.bundle_plan)?;
    println!(
        "{}",
        serde_json::to_string(&CollectorSummary {
            schema: "pliegocss/pliegors-collector/2",
            cargo_targets: cargo_inventory.targets.len(),
            cargo_rust_files: cargo_inventory.source_units.len(),
            collected_rust_files: generated.reachability.source_file_count(),
            style_sites: generated.reachability.invocation_count(),
            reachability_bytes: generated.reachability.as_bytes().len(),
            bundle_plan_bytes: generated.bundle_plan.len(),
            bundles: &generated.bundles,
        })?
    );
    Ok(())
}
