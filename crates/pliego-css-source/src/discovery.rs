//! Bounded project discovery for conservative migration inventories.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::migration::normalize_relative_target;
use crate::{
    MigrationAuxiliaryKind, MigrationConsumerKind, MigrationInventory, MigrationInventoryError,
    MigrationPreflightReliance, MigrationProject, MigrationProjectAuxiliary,
    MigrationProjectConsumer, MigrationProjectSource, MigrationSourceKind,
    inventory_migration_auxiliary_file, inventory_migration_consumer_file,
    inventory_migration_file,
};

const MAX_DISCOVERY_DEPTH: usize = 32;
const MAX_DISCOVERY_ENTRIES: usize = 65_536;
const MAX_DISCOVERY_CANDIDATE_BYTES: u64 = 256 * 1024 * 1024;
const IGNORED_DIRECTORIES: [&str; 3] = [".git", "node_modules", "target"];

/// Discovers a conservative migration declaration below one project-relative directory.
///
/// Discovery recognizes Sass by extension, CSS Modules by the `.module.css` suffix, Tailwind CSS
/// only from exact Tailwind constructs, CSS Modules consumers only when an import observation is
/// present, and Tailwind templates only when the project contains a Tailwind entry. Standard
/// `tailwind.config.*` files and exact relative `@config`, `@plugin`, and `@source` targets are
/// retained as typed auxiliaries. `.git`, `node_modules`, and `target` directories are ignored.
///
/// The returned declaration remains uncollected. Call [`MigrationProject::collect`] to perform the
/// normal canonical two-pass inventory and dependency validation.
///
/// # Errors
///
/// Returns [`MigrationInventoryError`] when the root is not a portable relative directory, a
/// traversed path is link-like or non-regular, traversal exceeds 32 levels, 65,536 entries, or 256
/// MiB of candidate files, a candidate cannot be safely inventoried, or no source can be confirmed.
pub fn discover_migration_project(
    root: &Path,
) -> Result<MigrationProject, MigrationInventoryError> {
    validate_discovery_root(root)?;
    let files = walk_discovery_files(root)?;
    let mut sources = Vec::new();
    let mut consumers = Vec::new();
    let mut templates = Vec::new();
    let mut standard_configs = Vec::new();
    let mut linked_auxiliaries = Vec::new();
    let mut has_tailwind = false;

    for file in &files {
        let logical = portable_discovery_path(file)?;
        if is_css_module(file) {
            sources.push(MigrationProjectSource::new(
                MigrationSourceKind::CssModules,
                logical,
            ));
        } else if is_sass(file) {
            sources.push(MigrationProjectSource::new(
                MigrationSourceKind::Sass,
                logical,
            ));
        } else if is_css(file) {
            let inventory = inventory_migration_file(MigrationSourceKind::Tailwind, file)?;
            if is_tailwind_entry(&inventory) {
                has_tailwind = true;
                linked_auxiliaries.extend(linked_tailwind_auxiliaries(&inventory)?);
                sources.push(MigrationProjectSource::new(
                    MigrationSourceKind::Tailwind,
                    logical,
                ));
            }
        } else if is_script(file) {
            discover_script_roles(
                file,
                logical,
                &mut consumers,
                &mut templates,
                &mut standard_configs,
            )?;
        } else if is_template(file) {
            templates.push(logical);
        }
    }

    if sources.is_empty() {
        return Err(MigrationInventoryError::new(
            "migration discovery found no confirmed source",
        ));
    }
    let mut project = MigrationProject::new();
    for source in sources {
        project = project.source(source);
    }
    for consumer in consumers {
        project = project.consumer(consumer);
    }
    if has_tailwind {
        project = add_tailwind_auxiliaries(
            project,
            &files,
            templates,
            standard_configs,
            linked_auxiliaries,
        )?;
    }
    Ok(project)
}

fn discover_script_roles(
    file: &Path,
    logical: String,
    consumers: &mut Vec<MigrationProjectConsumer>,
    templates: &mut Vec<String>,
    configs: &mut Vec<String>,
) -> Result<(), MigrationInventoryError> {
    let consumer = inventory_migration_consumer_file(MigrationConsumerKind::CssModules, file)?;
    if consumer
        .observations()
        .iter()
        .any(|item| item.specifier().is_some())
    {
        consumers.push(MigrationProjectConsumer::new(
            MigrationConsumerKind::CssModules,
            logical.clone(),
        ));
    }
    if is_standard_tailwind_config(file) {
        configs.push(logical.clone());
    }
    templates.push(logical);
    Ok(())
}

fn add_tailwind_auxiliaries(
    mut project: MigrationProject,
    files: &[PathBuf],
    templates: Vec<String>,
    configs: Vec<String>,
    linked: Vec<(MigrationAuxiliaryKind, String)>,
) -> Result<MigrationProject, MigrationInventoryError> {
    let available = files
        .iter()
        .map(|file| portable_discovery_path(file))
        .collect::<Result<Vec<_>, _>>()?;
    let mut auxiliaries = configs
        .into_iter()
        .map(|file| (MigrationAuxiliaryKind::TailwindConfig, file))
        .collect::<Vec<_>>();
    auxiliaries.extend(
        linked
            .into_iter()
            .filter(|(_, file)| available.contains(file)),
    );
    for file in templates {
        let inventory = inventory_migration_auxiliary_file(
            MigrationAuxiliaryKind::TailwindTemplate,
            Path::new(&file),
        )?;
        if !inventory.observations().is_empty()
            && !auxiliaries.iter().any(|(_, existing)| existing == &file)
        {
            auxiliaries.push((MigrationAuxiliaryKind::TailwindTemplate, file));
        }
    }
    auxiliaries.sort();
    auxiliaries.dedup();
    for (kind, file) in auxiliaries {
        project = project.auxiliary(MigrationProjectAuxiliary::new(kind, file));
    }
    Ok(project)
}

fn linked_tailwind_auxiliaries(
    inventory: &MigrationInventory,
) -> Result<Vec<(MigrationAuxiliaryKind, String)>, MigrationInventoryError> {
    let mut auxiliaries = Vec::new();
    for construct in inventory.constructs() {
        let kind = match construct.kind() {
            "tailwind-config" => MigrationAuxiliaryKind::TailwindConfig,
            "tailwind-plugin" => MigrationAuxiliaryKind::TailwindPlugin,
            "tailwind-source" => MigrationAuxiliaryKind::TailwindTemplate,
            _ => continue,
        };
        let Some(specifier) = single_quoted_value(construct.syntax()) else {
            continue;
        };
        if (!specifier.starts_with("./") && !specifier.starts_with("../"))
            || specifier.contains(['*', '?', '[', ']'])
        {
            continue;
        }
        let target = normalize_relative_target(inventory.file(), specifier)?;
        auxiliaries.push((kind, target));
    }
    Ok(auxiliaries)
}

fn single_quoted_value(syntax: &str) -> Option<&str> {
    let quote_start = syntax.find(['\'', '"'])?;
    let quote = syntax.as_bytes()[quote_start];
    let value_start = quote_start + 1;
    let relative_end = syntax[value_start..]
        .bytes()
        .position(|byte| byte == quote)?;
    let value = &syntax[value_start..value_start + relative_end];
    (!value.contains('\\')).then_some(value)
}

fn is_tailwind_entry(inventory: &MigrationInventory) -> bool {
    inventory.preflight_reliance() != MigrationPreflightReliance::NotObserved
        || inventory.constructs().iter().any(|item| {
            matches!(
                item.kind(),
                "tailwind-apply"
                    | "tailwind-config"
                    | "tailwind-custom-variant"
                    | "tailwind-plugin"
                    | "tailwind-reference"
                    | "tailwind-source"
                    | "tailwind-theme"
                    | "tailwind-utility"
                    | "tailwind-variant"
            )
        })
}

fn walk_discovery_files(root: &Path) -> Result<Vec<PathBuf>, MigrationInventoryError> {
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut files = Vec::new();
    let mut entries_seen = 0_usize;
    let mut candidate_bytes = 0_u64;
    while let Some((directory, depth)) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| {
                MigrationInventoryError::new(format!(
                    "cannot read migration discovery directory `{}`: {error}",
                    directory.display()
                ))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                MigrationInventoryError::new(format!(
                    "cannot read migration discovery entry: {error}"
                ))
            })?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            if ignored_directory_name(entry.file_name().to_str()) {
                continue;
            }
            entries_seen += 1;
            if entries_seen > MAX_DISCOVERY_ENTRIES {
                return Err(MigrationInventoryError::new(
                    "migration discovery exceeds 65,536 entries",
                ));
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                MigrationInventoryError::new(format!(
                    "cannot inspect migration discovery path `{}`: {error}",
                    path.display()
                ))
            })?;
            if link_like(&metadata) {
                return Err(MigrationInventoryError::new(format!(
                    "migration discovery path `{}` is link-like",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                if depth >= MAX_DISCOVERY_DEPTH {
                    return Err(MigrationInventoryError::new(
                        "migration discovery exceeds 32 directory levels",
                    ));
                }
                pending.push((path, depth + 1));
            } else if metadata.is_file() {
                if is_discovery_candidate(&path) {
                    candidate_bytes =
                        candidate_bytes.checked_add(metadata.len()).ok_or_else(|| {
                            MigrationInventoryError::new(
                                "migration discovery candidate byte count overflowed",
                            )
                        })?;
                    if candidate_bytes > MAX_DISCOVERY_CANDIDATE_BYTES {
                        return Err(MigrationInventoryError::new(
                            "migration discovery exceeds 256 MiB of candidate files",
                        ));
                    }
                    files.push(path);
                }
            } else {
                return Err(MigrationInventoryError::new(format!(
                    "migration discovery path `{}` is not regular",
                    path.display()
                )));
            }
        }
    }
    files.sort();
    Ok(files)
}

fn validate_discovery_root(root: &Path) -> Result<(), MigrationInventoryError> {
    if root.as_os_str().is_empty() || root.is_absolute() {
        return Err(MigrationInventoryError::new(
            "migration discovery requires a project-relative directory",
        ));
    }
    let mut current = PathBuf::new();
    for component in root.components() {
        match component {
            Component::CurDir => current.push("."),
            Component::Normal(value) => current.push(value),
            _ => {
                return Err(MigrationInventoryError::new(
                    "migration discovery requires a project-relative directory",
                ));
            }
        }
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            MigrationInventoryError::new(format!(
                "cannot inspect migration discovery root component `{}`: {error}",
                current.display()
            ))
        })?;
        if link_like(&metadata) {
            return Err(MigrationInventoryError::new(format!(
                "migration discovery root component `{}` is link-like",
                current.display()
            )));
        }
    }
    let metadata = fs::symlink_metadata(root).map_err(|error| {
        MigrationInventoryError::new(format!(
            "cannot inspect migration discovery root `{}`: {error}",
            root.display()
        ))
    })?;
    if link_like(&metadata) || !metadata.is_dir() {
        return Err(MigrationInventoryError::new(
            "migration discovery root must be a non-link directory",
        ));
    }
    Ok(())
}

fn portable_discovery_path(path: &Path) -> Result<String, MigrationInventoryError> {
    let mut values = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => values.push(value.to_str().ok_or_else(|| {
                MigrationInventoryError::new("migration discovery path is not UTF-8")
            })?),
            _ => {
                return Err(MigrationInventoryError::new(
                    "migration discovery path is not portable",
                ));
            }
        }
    }
    Ok(values.join("/"))
}

fn ignored_directory_name(name: Option<&str>) -> bool {
    name.is_some_and(|value| IGNORED_DIRECTORIES.contains(&value))
}

fn link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

fn extension(file: &Path) -> &str {
    file.extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
}

fn is_sass(file: &Path) -> bool {
    matches!(
        extension(file).to_ascii_lowercase().as_str(),
        "sass" | "scss"
    )
}

fn is_css(file: &Path) -> bool {
    extension(file).eq_ignore_ascii_case("css")
}

fn is_css_module(file: &Path) -> bool {
    file.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(".module.css"))
}

fn is_script(file: &Path) -> bool {
    matches!(
        extension(file).to_ascii_lowercase().as_str(),
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs"
    )
}

fn is_template(file: &Path) -> bool {
    matches!(
        extension(file).to_ascii_lowercase().as_str(),
        "html" | "htm" | "vue" | "svelte" | "astro" | "md" | "mdx" | "php"
    )
}

fn is_standard_tailwind_config(file: &Path) -> bool {
    file.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().starts_with("tailwind.config."))
}

fn is_discovery_candidate(file: &Path) -> bool {
    is_sass(file) || is_css(file) || is_script(file) || is_template(file)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn discovers_bounded_multi_role_migration_project() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from(format!(
            ".migration-discovery-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("styles")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("node_modules/ignored")).unwrap();
        fs::write(
            root.join("styles/app.css"),
            "@import \"tailwindcss\";\n@config \"../tailwind.config.js\";\n",
        )
        .unwrap();
        fs::write(root.join("styles/legacy.scss"), "$brand: red;\n").unwrap();
        fs::write(
            root.join("styles/card.module.css"),
            ".card { display: block; }\n",
        )
        .unwrap();
        fs::write(root.join("tailwind.config.js"), "export default {};\n").unwrap();
        fs::write(
            root.join("src/Card.tsx"),
            "import styles from \"../styles/card.module.css\";\nexport const Card = () => <div className=\"p-4\" data-class={styles.card} />;\n",
        )
        .unwrap();
        fs::write(
            root.join("node_modules/ignored/bad.scss"),
            "@unterminated\n",
        )
        .unwrap();

        let first = discover_migration_project(&root)
            .unwrap()
            .collect()
            .unwrap();
        let second = discover_migration_project(&root)
            .unwrap()
            .collect()
            .unwrap();
        assert_eq!(first.as_bytes(), second.as_bytes());
        assert_eq!(first.sources().len(), 3);
        assert_eq!(first.consumers().len(), 1);
        assert_eq!(first.auxiliaries().len(), 2);
        assert_eq!(
            first.auxiliaries()[0].file(),
            root.join("src/Card.tsx")
                .to_string_lossy()
                .replace('\\', "/")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unsafe_or_missing_discovery_roots() {
        assert!(discover_migration_project(Path::new("../outside")).is_err());
        assert!(discover_migration_project(Path::new("missing-migration-root")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_link_like_discovery_root_components() {
        use std::os::unix::fs::symlink;

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from(format!(
            ".migration-discovery-link-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("actual")).unwrap();
        symlink("actual", root.join("linked")).unwrap();
        assert!(discover_migration_project(&root.join("linked")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
