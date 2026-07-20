use std::fs;
use std::path::{Path, PathBuf};

use super::{acquire_publication_locks, commit_prepared_locked_with, publication_destinations};

pub(crate) struct PreparedWrite {
    pub(crate) destination: PathBuf,
    pub(crate) temporary: Option<PathBuf>,
}

impl PreparedWrite {
    pub(crate) fn commit(self) -> Result<(), String> {
        commit_prepared(vec![self])
    }
}

impl Drop for PreparedWrite {
    fn drop(&mut self) {
        if let Some(temporary) = &self.temporary {
            let _ = fs::remove_file(temporary);
        }
    }
}

pub(crate) fn commit_prepared(writes: Vec<PreparedWrite>) -> Result<(), String> {
    commit_prepared_with(writes, |_, temporary, destination| {
        fs::rename(temporary, destination)
    })
}

pub(crate) fn commit_prepared_with(
    writes: Vec<PreparedWrite>,
    publish: impl FnMut(usize, &Path, &Path) -> std::io::Result<()>,
) -> Result<(), String> {
    let destinations =
        publication_destinations(writes.iter().map(|write| write.destination.as_path()))?;
    let _locks = acquire_publication_locks(&destinations)?;
    commit_prepared_locked_with(writes, publish)
}

pub(crate) fn prepare_atomic_write(
    destination: &Path,
    bytes: &[u8],
) -> Result<PreparedWrite, String> {
    if destination
        .file_name()
        .and_then(|name| name.to_str())
        .is_none()
    {
        return Err(format!("invalid output path `{}`", destination.display()));
    }
    {
        let mut reservation =
            pliego_css_publication::ReservedSibling::create(destination, "pliego", "tmp", 32)
                .map_err(|error| {
                    format!(
                        "cannot prepare `{}` through a reserved sibling: {error}",
                        destination.display()
                    )
                })?;
        let temporary = reservation.path().to_path_buf();
        if let Err(error) = reservation.write_bytes(bytes, true) {
            return Err(format!(
                "cannot prepare `{}` through `{}`: {error}",
                destination.display(),
                temporary.display()
            ));
        }
        Ok(PreparedWrite {
            destination: destination.to_path_buf(),
            temporary: Some(reservation.into_path()),
        })
    }
}

pub(crate) fn prepare_atomic_write_if_changed(
    destination: &Path,
    bytes: &[u8],
) -> Result<Option<PreparedWrite>, String> {
    match fs::read(destination) {
        Ok(existing) if existing == bytes => Ok(None),
        Ok(_) => prepare_atomic_write(destination, bytes).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            prepare_atomic_write(destination, bytes).map(Some)
        }
        Err(error) => Err(format!(
            "cannot compare existing output `{}` before publication: {error}",
            destination.display()
        )),
    }
}
