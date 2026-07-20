//! Shared filesystem publication primitives.
//!
//! Transactions are rollback-capable and, when requested, durable across file and directory
//! metadata updates. They are deliberately **not crash-atomic**: interruption can expose an
//! intermediate rename state, while failures observed by the live process trigger rollback.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fs2::FileExt;

static RESERVATION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// One destination and its exact replacement bytes.
#[derive(Clone, Copy, Debug)]
pub struct WriteRequest<'a> {
    /// Destination replaced by this transaction.
    pub destination: &'a Path,
    /// Exact bytes to publish.
    pub bytes: &'a [u8],
}

impl<'a> WriteRequest<'a> {
    /// Creates a publication request.
    #[must_use]
    pub const fn new(destination: &'a Path, bytes: &'a [u8]) -> Self {
        Self { destination, bytes }
    }
}

/// Coordination namespace and durability policy for one transaction.
#[derive(Clone, Copy, Debug)]
pub struct PublicationOptions<'a> {
    namespace: &'a str,
    durable: bool,
}

impl<'a> PublicationOptions<'a> {
    /// Uses `namespace` in temporary and backup siblings.
    #[must_use]
    pub const fn new(namespace: &'a str) -> Self {
        Self {
            namespace,
            durable: true,
        }
    }

    /// Enables or disables file and parent-directory synchronization.
    #[must_use]
    pub const fn durable(mut self, durable: bool) -> Self {
        self.durable = durable;
        self
    }
}

/// An advisory lock held by an opened, validated regular non-link file.
#[derive(Debug)]
pub struct PublicationLock {
    file: File,
}

impl PublicationLock {
    /// Opens and exclusively locks `path`, rejecting a link-like leaf.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the path cannot be opened safely, is not a regular file, or the
    /// exclusive advisory lock cannot be acquired.
    pub fn acquire(path: &Path) -> io::Result<Self> {
        reject_existing_non_regular_or_link(path)?;
        let file = open_lock_no_follow(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata_is_link_like(&metadata) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "publication lock is not a regular non-link file",
            ));
        }
        reject_existing_non_regular_or_link(path)?;
        file.try_lock_exclusive().map_err(|error| {
            if error.kind() == io::ErrorKind::WouldBlock {
                error
            } else {
                io::Error::new(io::ErrorKind::WouldBlock, error)
            }
        })?;
        Ok(Self { file })
    }
}

impl Drop for PublicationLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

/// A collision-safe sibling reservation whose identity remains bound to an open file.
#[derive(Debug)]
pub struct ReservedSibling {
    path: PathBuf,
    file: File,
    remove_on_drop: bool,
}

impl ReservedSibling {
    /// Reserves a unique sibling with `create_new`.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when no collision-free sibling can be created within `attempts`.
    pub fn create(
        destination: &Path,
        namespace: &str,
        suffix: &str,
        attempts: usize,
    ) -> io::Result<Self> {
        let name = destination.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "destination has no file name")
        })?;
        for _ in 0..attempts {
            let sequence = RESERVATION_COUNTER.fetch_add(1, Ordering::Relaxed);
            let mut coordination_name = std::ffi::OsString::from(".");
            coordination_name.push(name);
            coordination_name.push(format!(
                ".{namespace}-{}-{sequence}.{suffix}",
                std::process::id()
            ));
            let path = destination.with_file_name(coordination_name);
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file,
                        remove_on_drop: true,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "coordination sibling collision limit reached",
        ))
    }

    /// Reserved path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Checks that the path still names the file created by this reservation.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when either identity handle cannot be inspected.
    pub fn has_reserved_identity(&self) -> io::Result<bool> {
        let reserved = same_file::Handle::from_file(self.file.try_clone()?)?;
        let named = same_file::Handle::from_path(&self.path)?;
        Ok(reserved == named)
    }

    /// Writes bytes through the identity-bound descriptor and optionally synchronizes them.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when writing or synchronizing the reserved file fails.
    pub fn write_bytes(&mut self, bytes: &[u8], durable: bool) -> io::Result<()> {
        self.file.write_all(bytes)?;
        if durable {
            self.file.sync_all()?;
        }
        Ok(())
    }

    /// Keeps the reserved sibling and returns its path.
    #[must_use]
    pub fn into_path(mut self) -> PathBuf {
        self.remove_on_drop = false;
        self.path.clone()
    }

    fn persist(self) -> PathBuf {
        self.into_path()
    }
}

impl Drop for ReservedSibling {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// A fully written temporary replacement removed unless committed.
#[derive(Debug)]
pub struct PreparedWrite {
    destination: PathBuf,
    temporary: Option<ReservedSibling>,
}

impl PreparedWrite {
    /// Writes and optionally synchronizes replacement bytes into a reserved sibling.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when reservation, writing, or synchronization fails.
    pub fn new(
        destination: &Path,
        bytes: &[u8],
        namespace: &str,
        durable: bool,
    ) -> io::Result<Self> {
        let mut temporary = ReservedSibling::create(destination, namespace, "tmp", 32)?;
        temporary.file.write_all(bytes)?;
        if durable {
            temporary.file.sync_all()?;
        }
        Ok(Self {
            destination: destination.to_path_buf(),
            temporary: Some(temporary),
        })
    }

    /// Path used for the prepared bytes.
    ///
    /// # Panics
    ///
    /// Panics only if the internal ownership invariant is violated after construction.
    #[must_use]
    pub fn temporary_path(&self) -> &Path {
        self.temporary
            .as_ref()
            .expect("prepared write owns temporary")
            .path()
    }
}

/// Publishes all writes with filesystem rename.
///
/// # Errors
///
/// Returns the first preparation, rename, synchronization, cleanup, or rollback I/O error.
pub fn publish(writes: &[WriteRequest<'_>], options: PublicationOptions<'_>) -> io::Result<()> {
    publish_with(writes, options, |_, temporary, destination| {
        fs::rename(temporary, destination)
    })
}

/// Publishes all writes with an injectable rename seam, rolling back live-process failures.
///
/// # Errors
///
/// Returns the first preparation, callback, synchronization, cleanup, or rollback I/O error.
///
/// # Panics
///
/// Panics only if the internal prepared-write ownership invariant is violated.
pub fn publish_with(
    writes: &[WriteRequest<'_>],
    options: PublicationOptions<'_>,
    mut publish_one: impl FnMut(usize, &Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let mut prepared = writes
        .iter()
        .map(|write| {
            PreparedWrite::new(
                write.destination,
                write.bytes,
                options.namespace,
                options.durable,
            )
        })
        .collect::<io::Result<Vec<_>>>()?;
    let mut backups = vec![None; prepared.len()];

    for (index, write) in prepared.iter().enumerate() {
        match move_to_reserved_backup(&write.destination, options.namespace, options.durable) {
            Ok(backup) => backups[index] = backup,
            Err(error) => {
                rollback(&prepared, &mut backups, 0, options.durable)?;
                return Err(error);
            }
        }
    }

    for index in 0..prepared.len() {
        let temporary = prepared[index]
            .temporary
            .take()
            .expect("prepared temporary");
        if !temporary.has_reserved_identity()? {
            rollback(&prepared, &mut backups, index, options.durable)?;
            return Err(io::Error::other("prepared temporary identity changed"));
        }
        let temporary_path = temporary.persist();
        if let Err(error) = publish_one(index, &temporary_path, &prepared[index].destination) {
            let _ = fs::remove_file(&temporary_path);
            rollback(&prepared, &mut backups, index, options.durable)?;
            return Err(error);
        }
        sync_parent_if_requested(&prepared[index].destination, options.durable)?;
    }

    for backup in backups.into_iter().flatten() {
        fs::remove_file(&backup)?;
        sync_parent_if_requested(&backup, options.durable)?;
    }
    Ok(())
}

fn move_to_reserved_backup(
    destination: &Path,
    namespace: &str,
    durable: bool,
) -> io::Result<Option<PathBuf>> {
    match fs::symlink_metadata(destination) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
        Ok(metadata) if metadata_is_link_like(&metadata) || !metadata.is_file() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "destination is not a regular non-link file",
            ));
        }
        Ok(_) => {}
    }
    let reservation = ReservedSibling::create(destination, namespace, "bak", 32)?;
    if !reservation.has_reserved_identity()? {
        return Err(io::Error::other("backup reservation identity changed"));
    }
    // Windows cannot replace the open reservation. Remove only the file whose identity we retained,
    // then rename immediately while the transaction's publication lock is held by the caller.
    fs::remove_file(reservation.path())?;
    let backup = reservation.persist();
    fs::rename(destination, &backup)?;
    sync_parent_if_requested(destination, durable)?;
    Ok(Some(backup))
}

fn rollback(
    writes: &[PreparedWrite],
    backups: &mut [Option<PathBuf>],
    published: usize,
    durable: bool,
) -> io::Result<()> {
    let mut first_error = None;
    for index in (0..writes.len()).rev() {
        let destination = &writes[index].destination;
        if index < published {
            if let Err(error) = fs::remove_file(destination) {
                if error.kind() != io::ErrorKind::NotFound && first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Some(backup) = backups[index].take() {
            if let Err(error) = fs::rename(&backup, destination) {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Err(error) = sync_parent_if_requested(destination, durable) {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}

fn reject_existing_non_regular_or_link(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(metadata) if metadata_is_link_like(&metadata) || !metadata.is_file() => {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "publication lock is not a regular non-link file",
            ))
        }
        Ok(_) => Ok(()),
    }
}

#[cfg(unix)]
fn open_lock_no_follow(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(windows)]
fn open_lock_no_follow(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_lock_no_follow(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

#[cfg(unix)]
fn metadata_is_link_like(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn metadata_is_link_like(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(any(unix, windows)))]
fn metadata_is_link_like(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(unix)]
fn sync_parent_if_requested(path: &Path, durable: bool) -> io::Result<()> {
    if durable {
        File::open(path.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()?;
    }
    Ok(())
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn sync_parent_if_requested(_path: &Path, _durable: bool) -> io::Result<()> {
    Ok(())
}
