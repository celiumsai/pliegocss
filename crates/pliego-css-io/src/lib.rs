//! Minimal bounded filesystem reads for `PliegoCSS` tooling.

#![forbid(unsafe_code)]

use std::fs::{self, OpenOptions};
use std::io::{Read, Write as _};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;
#[cfg(windows)]
use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Component, Path, PathBuf};

#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

/// Reads at most `limit` bytes from a regular file without following link-like components.
///
/// # Errors
/// Returns a stable error when a path component is link-like, the opened object is not a regular
/// file, the content exceeds `limit`, or an I/O operation fails.
pub fn read_bounded_regular_file(path: &Path, limit: usize, role: &str) -> Result<Vec<u8>, String> {
    reject_link_components(path, role)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    #[cfg(windows)]
    options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    let mut file = options.open(path).map_err(|error| {
        format!(
            "{role} `{}` is not a bounded regular file: cannot open: {error}",
            path.display()
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        format!(
            "{role} `{}` is not a bounded regular file: cannot inspect opened file: {error}",
            path.display()
        )
    })?;
    if link_like(&metadata) || !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(format!(
            "{role} `{}` is not a bounded regular file",
            path.display()
        ));
    }
    let capacity = usize::try_from(metadata.len()).unwrap_or(limit).min(limit);
    let mut bytes = Vec::with_capacity(capacity);
    Read::by_ref(&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {role} `{}`: {error}", path.display()))?;
    if bytes.len() > limit {
        return Err(format!(
            "{role} `{}` is not a bounded regular file",
            path.display()
        ));
    }
    Ok(bytes)
}

/// Creates a new regular file, writes all bytes, and synchronizes it.
///
/// # Errors
/// Returns an error if the path exists, traverses a link-like parent, cannot be written, or cannot
/// be synchronized.
pub fn write_new_synced_regular_file(path: &Path, bytes: &[u8], role: &str) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        reject_link_components(parent, role)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    #[cfg(windows)]
    options.custom_flags(0x0020_0000);
    let mut file = options
        .open(path)
        .map_err(|error| format!("cannot create {role} `{}` safely: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("cannot write {role} `{}`: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("cannot sync {role} `{}`: {error}", path.display()))
}

/// Renames a prepared file over a destination and syncs its directory where supported.
///
/// # Errors
/// Returns an error when rename fails or Unix directory synchronization fails. Windows treats
/// directory synchronization as unsupported and succeeds after the rename.
pub fn rename_prepared_synced(source: &Path, destination: &Path, role: &str) -> Result<(), String> {
    fs::rename(source, destination)
        .map_err(|error| format!("cannot publish {role} `{}`: {error}", destination.display()))?;
    #[cfg(unix)]
    {
        let parent = match destination.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                format!(
                    "cannot sync {role} directory `{}`: {error}",
                    parent.display()
                )
            })?;
    }
    Ok(())
}

fn reject_link_components(path: &Path, role: &str) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => current.push(component),
            Component::Normal(value) => {
                current.push(value);
                let metadata = fs::symlink_metadata(&current).map_err(|error| {
                    format!(
                        "cannot inspect {role} path component `{}`: {error}",
                        current.display()
                    )
                })?;
                if link_like(&metadata) {
                    return Err(format!(
                        "{role} `{}` is not a bounded regular file: unsafe path component",
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return true;
    }
    false
}
