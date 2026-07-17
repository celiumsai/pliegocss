//! Deterministic Rust source path expansion.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::is_ignored_source_directory;

/// Expands explicit Rust files or directories without following symbolic links.
///
/// # Errors
///
/// Returns an error for invalid explicit inputs or unreadable source trees.
pub fn expand_source_paths(sources: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    expand(sources, false)
}

/// Expands bundle Rust inputs and additionally rejects Windows reparse points.
///
/// # Errors
///
/// Returns an error for invalid explicit inputs, links, or unreadable source trees.
pub fn expand_bundle_source_paths(sources: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    expand(sources, true)
}

fn expand(sources: &[PathBuf], reject_reparse: bool) -> Result<Vec<PathBuf>, String> {
    let mut files = BTreeMap::new();
    for source in sources {
        collect(source, true, reject_reparse, &mut files)?;
    }
    Ok(files.into_values().collect())
}

fn collect(
    path: &Path,
    explicit: bool,
    reject_reparse: bool,
    files: &mut BTreeMap<PathBuf, PathBuf>,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect source `{}`: {error}", path.display()))?;
    let kind = metadata.file_type();
    if kind.is_symlink() || (reject_reparse && link_like(&metadata)) {
        return if explicit {
            Err(format!(
                "source `{}` is a symbolic link; source traversal does not follow symlinks",
                path.display()
            ))
        } else {
            Ok(())
        };
    }
    if kind.is_file() {
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("rs"))
        {
            return if explicit {
                Err(format!("source `{}` is not a Rust file", path.display()))
            } else {
                Ok(())
            };
        }
        if path.to_str().is_none() {
            return Err(format!(
                "source path `{}` is not valid UTF-8",
                path.display()
            ));
        }
        let canonical = fs::canonicalize(path)
            .map_err(|error| format!("cannot canonicalize source `{}`: {error}", path.display()))?;
        files
            .entry(canonical)
            .and_modify(|current| {
                if path.as_os_str() < current.as_os_str() {
                    *current = path.to_path_buf();
                }
            })
            .or_insert_with(|| path.to_path_buf());
        return Ok(());
    }
    if !kind.is_dir() {
        return if explicit {
            Err(format!(
                "source `{}` is neither a Rust file nor a directory",
                path.display()
            ))
        } else {
            Ok(())
        };
    }
    if is_ignored_source_directory(path) {
        return Ok(());
    }
    let mut entries = fs::read_dir(path)
        .map_err(|error| format!("cannot read source directory `{}`: {error}", path.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot read source directory `{}`: {error}", path.display()))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("cannot inspect `{}`: {error}", entry.path().display()))?;
        let kind = metadata.file_type();
        if kind.is_symlink() || (reject_reparse && link_like(&metadata)) {
            continue;
        }
        if kind.is_dir() && (name == "target" || name == ".git" || name.starts_with('.')) {
            continue;
        }
        collect(&entry.path(), false, reject_reparse, files)?;
    }
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn link_like(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
