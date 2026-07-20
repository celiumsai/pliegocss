use std::fs;
use std::path::{Component, Path};

use pliego_css_io::read_bounded_regular_file;
use pliego_css_source::{
    MigrationProject, MigrationSourceKind, ReversibleAdditionReceipt,
    ReversibleReplacementGroupReceipt, ReversibleReplacementReceipt, apply_reversible_addition,
    apply_reversible_replacement, apply_reversible_replacement_group,
    build_reversible_migration_plan, inventory_migration_file, rollback_reversible_addition,
    rollback_reversible_replacement, rollback_reversible_replacement_group,
};
use serde::Deserialize;

#[cfg(windows)]
use std::os::windows::fs::MetadataExt as _;

#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

fn metadata_is_link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    false
}

use super::CliFailure;

const MAX_GROUP_FILE_BYTES: usize = 16 * 1024 * 1024;

fn read_group_file(path: &Path, role: &str) -> Result<Vec<u8>, CliFailure> {
    read_bounded_regular_file(path, MAX_GROUP_FILE_BYTES, role).map_err(CliFailure::invalid)
}

pub(crate) fn run_migration_inventory(
    kind: MigrationSourceKind,
    input: &Path,
) -> Result<(), CliFailure> {
    inventory_migration_file(kind, input)
        .map(|inventory| print!("{inventory}"))
        .map_err(|error| CliFailure::tool(error.to_string()))
}

pub(crate) fn run_migration_project(input: &Path) -> Result<(), CliFailure> {
    MigrationProject::from_input(input)
        .and_then(MigrationProject::collect)
        .map(|inventory| print!("{inventory}"))
        .map_err(|error| CliFailure::tool(error.to_string()))
}

pub(crate) fn run_reversible_migration_project_plan(input: &Path) -> Result<(), CliFailure> {
    let project =
        MigrationProject::from_input(input).map_err(|error| CliFailure::tool(error.to_string()))?;
    let inventory = project
        .clone()
        .collect()
        .map_err(|error| CliFailure::tool(error.to_string()))?;
    build_reversible_migration_plan(&project, inventory.sources())
        .map(|plan| print!("{}", String::from_utf8_lossy(plan.as_bytes())))
        .map_err(|error| CliFailure::tool(error.to_string()))
}

const MIGRATION_SIDECAR: &[u8] =
    b"/* Generated PliegoCSS migration sidecar. Safe to remove after rollback. */\n";

pub(crate) fn run_reversible_sidecar_apply(
    output: &Path,
    receipt: &Path,
) -> Result<(), CliFailure> {
    if receipt.exists() {
        return Err(CliFailure::invalid(
            "migration sidecar receipt already exists",
        ));
    }
    let identity = apply_reversible_addition(output, MIGRATION_SIDECAR)
        .map_err(|error| CliFailure::tool(error.to_string()))?;
    let bytes = identity
        .to_json()
        .map_err(|error| CliFailure::tool(error.to_string()))?;
    if let Err(error) = fs::write(receipt, bytes) {
        let _ = rollback_reversible_addition(output, &identity);
        return Err(CliFailure::tool(format!(
            "cannot publish migration sidecar receipt: {error}"
        )));
    }
    Ok(())
}

pub(crate) fn run_reversible_sidecar_rollback(
    output: &Path,
    receipt: &Path,
) -> Result<(), CliFailure> {
    let bytes = fs::read(receipt).map_err(|error| {
        CliFailure::invalid(format!("cannot read migration sidecar receipt: {error}"))
    })?;
    let identity = ReversibleAdditionReceipt::from_json(&bytes)
        .map_err(|error| CliFailure::invalid(error.to_string()))?;
    rollback_reversible_addition(output, &identity)
        .map_err(|error| CliFailure::tool(error.to_string()))?;
    fs::remove_file(receipt)
        .map_err(|error| CliFailure::tool(format!("cannot remove migration receipt: {error}")))
}

pub(crate) fn run_reversible_replace_apply(
    file: &Path,
    before: &Path,
    after: &Path,
    receipt: &Path,
) -> Result<(), CliFailure> {
    if receipt.exists() {
        return Err(CliFailure::invalid(
            "migration replacement receipt already exists",
        ));
    }
    let before_bytes = fs::read(before)
        .map_err(|error| CliFailure::invalid(format!("cannot read before bytes: {error}")))?;
    let after_bytes = fs::read(after)
        .map_err(|error| CliFailure::invalid(format!("cannot read after bytes: {error}")))?;
    let identity = apply_reversible_replacement(file, &before_bytes, &after_bytes)
        .map_err(|error| CliFailure::tool(error.to_string()))?;
    let bytes = identity
        .to_json()
        .map_err(|error| CliFailure::tool(error.to_string()))?;
    if let Err(error) = fs::write(receipt, bytes) {
        let _ = rollback_reversible_replacement(file, &identity);
        return Err(CliFailure::tool(format!(
            "cannot publish replacement receipt: {error}"
        )));
    }
    Ok(())
}

pub(crate) fn run_reversible_replace_rollback(
    file: &Path,
    receipt: &Path,
) -> Result<(), CliFailure> {
    let bytes = fs::read(receipt).map_err(|error| {
        CliFailure::invalid(format!("cannot read replacement receipt: {error}"))
    })?;
    let identity = ReversibleReplacementReceipt::from_json(&bytes)
        .map_err(|error| CliFailure::invalid(error.to_string()))?;
    rollback_reversible_replacement(file, &identity)
        .map_err(|error| CliFailure::tool(error.to_string()))?;
    fs::remove_file(receipt)
        .map_err(|error| CliFailure::tool(format!("cannot remove replacement receipt: {error}")))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct GroupManifest {
    schema_version: u8,
    entries: Vec<GroupManifestEntry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupManifestEntry {
    file: std::path::PathBuf,
    after: std::path::PathBuf,
}

fn validate_group_relative_path(path: &Path, label: &str) -> Result<(), CliFailure> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(CliFailure::invalid(format!(
            "{label} must be workspace-relative without parent traversal"
        )));
    }
    let mut current = std::path::PathBuf::new();
    for component in path.components() {
        current.push(component);
        if current.exists()
            && metadata_is_link_like(
                &fs::symlink_metadata(&current).map_err(|error| {
                    CliFailure::invalid(format!("cannot inspect {label}: {error}"))
                })?,
            )
        {
            return Err(CliFailure::invalid(format!(
                "{label} must not traverse a symbolic link"
            )));
        }
    }
    Ok(())
}

pub(crate) fn run_reversible_group_apply(
    manifest: &Path,
    receipt: &Path,
) -> Result<(), CliFailure> {
    validate_group_relative_path(manifest, "group manifest")?;
    validate_group_relative_path(receipt, "group receipt")?;
    if receipt.exists() {
        return Err(CliFailure::invalid("group receipt already exists"));
    }
    let bytes = read_group_file(manifest, "group manifest")?;
    let manifest: GroupManifest = serde_json::from_slice(&bytes)
        .map_err(|error| CliFailure::invalid(format!("cannot parse group manifest: {error}")))?;
    if manifest.schema_version != 1 {
        return Err(CliFailure::invalid(
            "unsupported group manifest schemaVersion",
        ));
    }
    let mut owned = Vec::with_capacity(manifest.entries.len());
    for entry in manifest.entries {
        validate_group_relative_path(&entry.file, "group destination")?;
        validate_group_relative_path(&entry.after, "group after file")?;
        let before = read_group_file(&entry.file, "group destination")?;
        let after = read_group_file(&entry.after, "group after file")?;
        owned.push((entry.file, before, after));
    }
    let borrowed = owned
        .iter()
        .map(|(path, before, after)| (path.as_path(), before.as_slice(), after.as_slice()))
        .collect::<Vec<_>>();
    let entries = apply_reversible_replacement_group(&borrowed)
        .map_err(|error| CliFailure::tool(error.to_string()))?;
    let group = ReversibleReplacementGroupReceipt::new(entries)
        .map_err(|error| CliFailure::tool(error.to_string()))?;
    let bytes = group
        .to_json()
        .map_err(|error| CliFailure::tool(error.to_string()))?;
    if let Err(error) = fs::write(receipt, bytes) {
        let _ = rollback_reversible_replacement_group(group.entries());
        return Err(CliFailure::tool(format!(
            "cannot publish group receipt: {error}"
        )));
    }
    Ok(())
}

pub(crate) fn run_reversible_group_rollback(receipt: &Path) -> Result<(), CliFailure> {
    validate_group_relative_path(receipt, "group receipt")?;
    let bytes = read_group_file(receipt, "group receipt")?;
    let group = ReversibleReplacementGroupReceipt::from_json(&bytes)
        .map_err(|error| CliFailure::invalid(error.to_string()))?;
    for (path, _) in group.entries() {
        validate_group_relative_path(path, "group receipt destination")?;
    }
    rollback_reversible_replacement_group(group.entries())
        .map_err(|error| CliFailure::tool(error.to_string()))?;
    fs::remove_file(receipt)
        .map_err(|error| CliFailure::tool(format!("cannot remove group receipt: {error}")))
}
