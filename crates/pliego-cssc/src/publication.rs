use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use super::atomic_write::PreparedWrite;
use super::{TEMP_FILE_COUNTER, publication_path_identity};

pub(crate) fn publication_destinations<'a>(
    destinations: impl IntoIterator<Item = &'a Path>,
) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut destinations_by_key = BTreeMap::new();
    for destination in destinations {
        let (key, resolved) = publication_path_identity(destination)?;
        let name = resolved
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("invalid publication destination")?;
        let reserved = name.strip_prefix('.').is_some_and(|body| {
            body.ends_with(".pliego.lock")
                || (body.contains(".pliego-")
                    && (body.as_bytes().ends_with(b".tmp") || body.as_bytes().ends_with(b".bak")))
        });
        if reserved {
            return Err(format!(
                "publication destination `{}` uses the reserved coordination namespace",
                destination.display()
            ));
        }
        if destinations_by_key.insert(key, resolved).is_some() {
            return Err(format!(
                "duplicate publication destination `{}`",
                destination.display()
            ));
        }
    }
    Ok(destinations_by_key)
}

pub(crate) fn commit_prepared_locked_with(
    mut writes: Vec<PreparedWrite>,
    mut publish: impl FnMut(usize, &Path, &Path) -> std::io::Result<()>,
) -> Result<(), String> {
    let mut backups = vec![None; writes.len()];
    for (index, write) in writes.iter().enumerate() {
        match move_destination_to_backup(&write.destination) {
            Ok(backup) => backups[index] = backup,
            Err(error) => {
                let rollback = rollback_prepared(&writes, &mut backups, 0);
                return Err(publication_error(&error, &rollback));
            }
        }
    }

    for (published, index) in (0..writes.len()).enumerate() {
        let temporary = writes[index]
            .temporary
            .take()
            .expect("prepared write retains its temporary path");
        if let Err(error) = publish(index, &temporary, &writes[index].destination) {
            let _ = fs::remove_file(&temporary);
            let message = format!(
                "cannot publish `{}` from `{}`: {error}",
                writes[index].destination.display(),
                temporary.display()
            );
            let rollback = rollback_prepared(&writes, &mut backups, published);
            return Err(publication_error(&message, &rollback));
        }
    }

    for backup in backups.into_iter().flatten() {
        if let Err(error) = fs::remove_file(&backup) {
            eprintln!(
                "warning: published outputs but could not remove backup `{}`: {error}",
                backup.display()
            );
        }
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct PublicationLock {
    _inner: pliego_css_publication::PublicationLock,
}

pub(crate) fn acquire_publication_locks(
    destinations: &BTreeMap<String, PathBuf>,
) -> Result<Vec<PublicationLock>, String> {
    destinations
        .values()
        .map(|destination| {
            let file_name = destination
                .file_name()
                .ok_or("invalid publication destination")?;
            let mut lock_name = OsString::from(".");
            lock_name.push(file_name);
            lock_name.push(".pliego.lock");
            let lock_path = destination.with_file_name(lock_name);
            let inner = pliego_css_publication::PublicationLock::acquire(&lock_path)
                .map_err(|error| {
                    if error.kind() == std::io::ErrorKind::WouldBlock {
                        format!(
                            "cannot acquire publication lock `{}`; another writer may be active: {error}",
                            lock_path.display()
                        )
                    } else {
                        format!(
                            "cannot open publication lock `{}`: {error}",
                            lock_path.display()
                        )
                    }
                })?;
            Ok(PublicationLock { _inner: inner })
        })
        .collect()
}

fn move_destination_to_backup(destination: &Path) -> Result<Option<PathBuf>, String> {
    match fs::symlink_metadata(destination) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "cannot inspect existing output `{}` before publication: {error}",
                destination.display()
            ));
        }
        Ok(metadata) if metadata.file_type().is_dir() => {
            return Err(format!(
                "output destination `{}` is a directory",
                destination.display()
            ));
        }
        Ok(_) => {}
    }

    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid output path `{}`", destination.display()))?;
    for _ in 0..32 {
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let backup = destination.with_file_name(format!(
            ".{file_name}.pliego-{}-{sequence}.bak",
            std::process::id()
        ));
        match fs::symlink_metadata(&backup) {
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "cannot inspect backup path `{}`: {error}",
                    backup.display()
                ));
            }
        }
        fs::rename(destination, &backup).map_err(|error| {
            format!(
                "cannot stage existing output `{}` through `{}`: {error}",
                destination.display(),
                backup.display()
            )
        })?;
        return Ok(Some(backup));
    }
    Err(format!(
        "cannot reserve a backup path for `{}` after 32 attempts",
        destination.display()
    ))
}

fn rollback_prepared(
    writes: &[PreparedWrite],
    backups: &mut [Option<PathBuf>],
    published: usize,
) -> RollbackSummary {
    let mut summary = RollbackSummary::default();
    for index in (0..writes.len()).rev() {
        let destination = &writes[index].destination;
        if index < published {
            match fs::remove_file(destination) {
                Ok(()) => summary.removed_partial += 1,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => summary.errors.push(format!(
                    "cannot remove partially published `{}`: {error}",
                    destination.display()
                )),
            }
        }
        if let Some(backup) = backups[index].take() {
            if let Err(error) = fs::rename(&backup, destination) {
                summary.errors.push(format!(
                    "cannot restore `{}` from `{}`: {error}",
                    destination.display(),
                    backup.display()
                ));
            } else {
                summary.restored += 1;
            }
        }
    }
    summary
}

#[derive(Default)]
struct RollbackSummary {
    restored: usize,
    removed_partial: usize,
    errors: Vec<String>,
}

fn publication_error(primary: &str, rollback: &RollbackSummary) -> String {
    if !rollback.errors.is_empty() {
        format!(
            "{primary}; rollback also failed: {}",
            rollback.errors.join("; ")
        )
    } else if rollback.restored != 0 && rollback.removed_partial != 0 {
        format!(
            "{primary}; rollback restored {} previous output(s) and removed {} partial output(s)",
            rollback.restored, rollback.removed_partial
        )
    } else if rollback.restored != 0 {
        format!(
            "{primary}; rollback restored {} previous output(s)",
            rollback.restored
        )
    } else if rollback.removed_partial != 0 {
        format!(
            "{primary}; rollback removed {} partial output(s); no previous outputs existed",
            rollback.removed_partial
        )
    } else {
        format!("{primary}; no destination changes required rollback")
    }
}
