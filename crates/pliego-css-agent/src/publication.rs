use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

use super::{
    RepairChangeReceipt, RepairChangeState, RepairCliFormat, RepairContractError,
    RepairPlanDocument, parse_repair_change_receipt, prepare_repair_application,
};

/// In-memory source transition and receipt prepared for one atomic publisher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedRepairApplication {
    pub(crate) receipt: RepairChangeReceipt,
    pub(crate) sources: BTreeMap<String, Vec<u8>>,
}

impl PreparedRepairApplication {
    /// Returns the receipt that must be published atomically with changed source files.
    #[must_use]
    pub const fn receipt(&self) -> &RepairChangeReceipt {
        &self.receipt
    }

    /// Returns exact after bytes keyed by canonical logical source path.
    #[must_use]
    pub const fn sources(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.sources
    }
}

/// Receipt and changed-destination count returned by atomic repair publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedRepairApplication {
    receipt: RepairChangeReceipt,
    receipt_bytes: Vec<u8>,
    changed_destinations: usize,
}

impl PublishedRepairApplication {
    /// Returns the exact published or preserved receipt.
    #[must_use]
    pub const fn receipt(&self) -> &RepairChangeReceipt {
        &self.receipt
    }

    /// Returns its exact canonical JSON bytes.
    #[must_use]
    pub fn receipt_bytes(&self) -> &[u8] {
        &self.receipt_bytes
    }

    /// Returns the number of source/receipt destinations whose bytes changed.
    #[must_use]
    pub const fn changed_destinations(&self) -> usize {
        self.changed_destinations
    }

    /// Renders canonical JSON or the concise human change report.
    #[must_use]
    pub fn render(&self, format: RepairCliFormat) -> String {
        match format {
            RepairCliFormat::Json => String::from_utf8_lossy(&self.receipt_bytes).into_owned(),
            RepairCliFormat::Text => format!(
                "{}Published destinations: {}\n",
                self.receipt.to_human(),
                self.changed_destinations
            ),
        }
    }
}

pub(crate) static REPAIR_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Resolves and validates a project-relative Change Receipt destination.
///
/// Parent components must already exist and may not traverse symbolic links, junctions, or reparse
/// points. The destination may be missing or a regular non-link file and may not alias any protected
/// input or repair source under portable ASCII case folding.
///
/// # Errors
///
/// Returns [`RepairContractError`] for unsafe components, missing/link-like parents, invalid final
/// file types, reserved lock names, or physical aliases.
pub fn resolve_repair_receipt_path<'a>(
    receipt: &Path,
    protected_inputs: impl IntoIterator<Item = &'a Path>,
    source_paths: &BTreeMap<String, PathBuf>,
) -> Result<PathBuf, RepairContractError> {
    if receipt.as_os_str().is_empty()
        || receipt.is_absolute()
        || receipt
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(RepairContractError::new(
            "change receipt must be a project-relative path without `.` or `..` components",
        ));
    }
    if receipt
        .file_name()
        .is_some_and(|name| name == ".pliegocss-repair.lock")
    {
        return Err(RepairContractError::new(
            "change receipt may not use `.pliegocss-repair.lock`",
        ));
    }
    let cwd = env::current_dir().map_err(|error| {
        RepairContractError::new(format!("cannot resolve change receipt: {error}"))
    })?;
    let mut current = cwd.clone();
    let components = receipt.components().collect::<Vec<_>>();
    for component in &components[..components.len() - 1] {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            RepairContractError::new(format!(
                "cannot inspect change receipt parent `{}`: {error}",
                current.display()
            ))
        })?;
        if is_repair_link_like(&metadata) || !metadata.is_dir() {
            return Err(RepairContractError::new(format!(
                "change receipt parent `{}` is not a regular non-link directory",
                current.display()
            )));
        }
    }
    let resolved = cwd.join(receipt);
    match fs::symlink_metadata(&resolved) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(RepairContractError::new(format!(
                "cannot inspect change receipt `{}`: {error}",
                resolved.display()
            )));
        }
        Ok(metadata) if is_repair_link_like(&metadata) || !metadata.is_file() => {
            return Err(RepairContractError::new(format!(
                "change receipt `{}` is not a regular non-link file",
                resolved.display()
            )));
        }
        Ok(_) => {}
    }
    let receipt_key = repair_path_key(&resolved)?;
    for input in protected_inputs {
        if receipt_key == repair_path_key(input)? {
            return Err(RepairContractError::new(format!(
                "change receipt `{}` aliases protected input `{}`",
                receipt.display(),
                input.display()
            )));
        }
    }
    for source in source_paths.values() {
        if receipt_key == repair_path_key(source)? {
            return Err(RepairContractError::new(format!(
                "change receipt `{}` aliases repair source `{}`",
                receipt.display(),
                source.display()
            )));
        }
    }
    Ok(resolved)
}

/// Resolves a project-relative receipt and atomically publishes a prepared repair.
///
/// # Errors
///
/// Returns [`RepairContractError`] for receipt-path or publication failure.
pub fn publish_repair_application_checked<'a>(
    prepared: &PreparedRepairApplication,
    source_root: &Path,
    source_paths: &BTreeMap<String, PathBuf>,
    verified_sources: &BTreeMap<String, Vec<u8>>,
    receipt: &Path,
    protected_inputs: impl IntoIterator<Item = &'a Path>,
) -> Result<PublishedRepairApplication, RepairContractError> {
    let receipt = resolve_repair_receipt_path(receipt, protected_inputs, source_paths)?;
    publish_repair_application(
        prepared,
        source_root,
        source_paths,
        verified_sources,
        &receipt,
    )
}

/// Verifies, prepares, resolves, and atomically publishes one explicitly authorized repair.
///
/// # Errors
///
/// Returns [`RepairContractError`] for any authorization, artifact, source, path, or publication
/// failure.
#[allow(clippy::too_many_arguments)]
pub fn apply_repair_plan_checked<'a>(
    plan: &RepairPlanDocument,
    finding_file: &str,
    finding_bytes: &[u8],
    verified_sources: &BTreeMap<String, Vec<u8>>,
    authorization: &str,
    source_root: &Path,
    source_paths: &BTreeMap<String, PathBuf>,
    receipt: &Path,
    protected_inputs: impl IntoIterator<Item = &'a Path>,
) -> Result<PublishedRepairApplication, RepairContractError> {
    let prepared = prepare_repair_application(
        plan,
        finding_file,
        finding_bytes,
        verified_sources,
        authorization,
    )?;
    publish_repair_application_checked(
        &prepared,
        source_root,
        source_paths,
        verified_sources,
        receipt,
        protected_inputs,
    )
}

fn repair_path_key(path: &Path) -> Result<String, RepairContractError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| RepairContractError::new(format!("cannot resolve path: {error}")))?
            .join(path)
    };
    let name = absolute
        .file_name()
        .ok_or_else(|| RepairContractError::new(format!("invalid path `{}`", path.display())))?;
    let parent = absolute
        .parent()
        .ok_or_else(|| RepairContractError::new(format!("invalid path `{}`", path.display())))?;
    let parent = fs::canonicalize(parent).map_err(|error| {
        RepairContractError::new(format!(
            "cannot canonicalize path parent `{}`: {error}",
            parent.display()
        ))
    })?;
    Ok(parent.join(name).to_string_lossy().to_ascii_lowercase())
}

pub(crate) fn is_repair_link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Atomically publishes prepared source bytes and their Change Receipt with rollback.
///
/// `source_paths` must map every logical prepared source to its already validated physical file;
/// `verified_sources` must contain the exact bytes used by [`prepare_repair_application`]. A
/// persistent `.pliegocss-repair.lock` under `source_root` coordinates cooperating writers. Source
/// permissions are preserved. An existing receipt for the same plan is preserved only when all
/// sources are already applied; any other receipt collision fails closed.
///
/// # Errors
///
/// Returns [`RepairContractError`] for path/set mismatch, file-type or byte drift, lock failure,
/// receipt collision, staging failure, or a publication/rollback failure.
pub fn publish_repair_application(
    prepared: &PreparedRepairApplication,
    source_root: &Path,
    source_paths: &BTreeMap<String, PathBuf>,
    verified_sources: &BTreeMap<String, Vec<u8>>,
    receipt_path: &Path,
) -> Result<PublishedRepairApplication, RepairContractError> {
    publish_repair_application_with(
        prepared,
        source_root,
        source_paths,
        verified_sources,
        receipt_path,
        |_, temporary, destination| fs::rename(temporary, destination),
    )
}

pub(crate) fn publish_repair_application_with(
    prepared: &PreparedRepairApplication,
    source_root: &Path,
    source_paths: &BTreeMap<String, PathBuf>,
    verified_sources: &BTreeMap<String, Vec<u8>>,
    receipt_path: &Path,
    publish: impl FnMut(usize, &Path, &Path) -> std::io::Result<()>,
) -> Result<PublishedRepairApplication, RepairContractError> {
    validate_publication_paths(
        prepared,
        source_root,
        source_paths,
        verified_sources,
        receipt_path,
    )?;
    let _lock = acquire_repair_lock(source_root)?;
    for (file, path) in source_paths {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            RepairContractError::new(format!(
                "cannot re-inspect repair source `{}` under lock: {error}",
                path.display()
            ))
        })?;
        if is_repair_link_like(&metadata) || !metadata.is_file() {
            return Err(RepairContractError::new(format!(
                "repair source `{}` changed file type under lock",
                path.display()
            )));
        }
        let actual = fs::read(path).map_err(|error| {
            RepairContractError::new(format!(
                "cannot re-read repair source `{}` under lock: {error}",
                path.display()
            ))
        })?;
        if verified_sources.get(file) != Some(&actual) {
            return Err(RepairContractError::new(format!(
                "repair source `{}` changed after plan verification",
                path.display()
            )));
        }
    }

    let existing_receipt = read_existing_receipt(receipt_path)?;
    let (receipt, receipt_bytes) = if let Some((receipt, bytes)) = existing_receipt {
        if receipt.plan_sha256 != prepared.receipt.plan_sha256 {
            return Err(RepairContractError::new(
                "change receipt destination already contains a different plan",
            ));
        }
        if prepared.receipt.state != RepairChangeState::AlreadyApplied {
            return Err(RepairContractError::new(
                "change receipt records this plan but sources returned to the before state",
            ));
        }
        (receipt, bytes)
    } else {
        let bytes = prepared.receipt.to_json_pretty()?.into_bytes();
        (prepared.receipt.clone(), bytes)
    };

    let mut writes = Vec::new();
    for (file, path) in source_paths {
        let after = prepared.sources.get(file).ok_or_else(|| {
            RepairContractError::new(format!("missing after bytes for repair source `{file}`"))
        })?;
        if verified_sources.get(file) != Some(after) {
            writes.push(prepare_source_write(path, after)?);
        }
    }
    if !receipt_path.exists() {
        writes.push(prepare_write(receipt_path, &receipt_bytes)?);
    }
    let changed_destinations = writes.len();
    commit_writes(writes, publish)?;
    Ok(PublishedRepairApplication {
        receipt,
        receipt_bytes,
        changed_destinations,
    })
}

fn validate_publication_paths(
    prepared: &PreparedRepairApplication,
    source_root: &Path,
    source_paths: &BTreeMap<String, PathBuf>,
    verified_sources: &BTreeMap<String, Vec<u8>>,
    receipt_path: &Path,
) -> Result<(), RepairContractError> {
    if source_paths.len() != verified_sources.len()
        || verified_sources.len() != prepared.sources.len()
        || source_paths.keys().ne(verified_sources.keys())
        || source_paths.keys().ne(prepared.sources.keys())
    {
        return Err(RepairContractError::new(
            "repair publication source sets do not match",
        ));
    }
    let root = fs::canonicalize(source_root).map_err(|error| {
        RepairContractError::new(format!(
            "cannot canonicalize repair source root `{}`: {error}",
            source_root.display()
        ))
    })?;
    let mut destinations = BTreeSet::new();
    for path in source_paths
        .values()
        .map(PathBuf::as_path)
        .chain(std::iter::once(receipt_path))
    {
        if path
            .file_name()
            .is_some_and(|name| name == ".pliegocss-repair.lock")
        {
            return Err(RepairContractError::new(
                "repair destinations may not use `.pliegocss-repair.lock`",
            ));
        }
        let parent = path.parent().ok_or_else(|| {
            RepairContractError::new(format!("invalid repair destination `{}`", path.display()))
        })?;
        let name = path.file_name().ok_or_else(|| {
            RepairContractError::new(format!("invalid repair destination `{}`", path.display()))
        })?;
        let parent = fs::canonicalize(parent).map_err(|error| {
            RepairContractError::new(format!(
                "cannot canonicalize repair destination parent `{}`: {error}",
                parent.display()
            ))
        })?;
        let key = parent.join(name).to_string_lossy().to_ascii_lowercase();
        if !destinations.insert(key) {
            return Err(RepairContractError::new(format!(
                "duplicate repair destination `{}`",
                path.display()
            )));
        }
    }
    for path in source_paths.values() {
        let canonical = fs::canonicalize(path).map_err(|error| {
            RepairContractError::new(format!(
                "cannot canonicalize repair source `{}`: {error}",
                path.display()
            ))
        })?;
        if !canonical.starts_with(&root) {
            return Err(RepairContractError::new(format!(
                "repair source `{}` escapes source root `{}`",
                path.display(),
                root.display()
            )));
        }
    }
    Ok(())
}

fn read_existing_receipt(
    path: &Path,
) -> Result<Option<(RepairChangeReceipt, Vec<u8>)>, RepairContractError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(RepairContractError::new(format!(
            "cannot inspect change receipt `{}`: {error}",
            path.display()
        ))),
        Ok(metadata) if is_repair_link_like(&metadata) || !metadata.is_file() => {
            Err(RepairContractError::new(format!(
                "change receipt `{}` is not a regular non-link file",
                path.display()
            )))
        }
        Ok(_) => {
            let bytes = fs::read(path).map_err(|error| {
                RepairContractError::new(format!(
                    "cannot read change receipt `{}`: {error}",
                    path.display()
                ))
            })?;
            let receipt = parse_repair_change_receipt(&bytes).map_err(|error| {
                RepairContractError::new(format!("invalid existing change receipt: {error}"))
            })?;
            Ok(Some((receipt, bytes)))
        }
    }
}

pub(crate) struct RepairLock {
    _inner: pliego_css_publication::PublicationLock,
}

pub(crate) fn acquire_repair_lock(root: &Path) -> Result<RepairLock, RepairContractError> {
    let path = root.join(".pliegocss-repair.lock");
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(RepairContractError::new(format!(
                "cannot inspect repair lock `{}`: {error}",
                path.display()
            )));
        }
        Ok(metadata) if is_repair_link_like(&metadata) || !metadata.is_file() => {
            return Err(RepairContractError::new(format!(
                "repair lock `{}` is not a regular non-link file",
                path.display()
            )));
        }
        Ok(_) => {}
    }
    let file = pliego_css_publication::PublicationLock::acquire(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            RepairContractError::new(format!(
                "cannot acquire repair lock `{}`; another repair may be active: {error}",
                path.display()
            ))
        } else {
            RepairContractError::new(format!(
                "cannot open repair lock `{}`: {error}",
                path.display()
            ))
        }
    })?;
    Ok(RepairLock { _inner: file })
}

pub(crate) struct PreparedRepairWrite {
    pub(crate) destination: PathBuf,
    pub(crate) temporary: Option<PathBuf>,
}

impl Drop for PreparedRepairWrite {
    fn drop(&mut self) {
        if let Some(temporary) = &self.temporary {
            let _ = fs::remove_file(temporary);
        }
    }
}

pub(crate) fn prepare_write(
    destination: &Path,
    bytes: &[u8],
) -> Result<PreparedRepairWrite, RepairContractError> {
    if destination
        .file_name()
        .and_then(|name| name.to_str())
        .is_none()
    {
        return Err(RepairContractError::new(format!(
            "invalid repair destination `{}`",
            destination.display()
        )));
    }
    {
        let mut reservation = pliego_css_publication::ReservedSibling::create(
            destination,
            "pliego-repair",
            "tmp",
            32,
        )
        .map_err(|error| {
            RepairContractError::new(format!(
                "cannot prepare `{}` through a reserved sibling: {error}",
                destination.display()
            ))
        })?;
        let temporary = reservation.path().to_path_buf();
        reservation.write_bytes(bytes, true).map_err(|error| {
            RepairContractError::new(format!(
                "cannot prepare `{}` through `{}`: {error}",
                destination.display(),
                temporary.display()
            ))
        })?;
        Ok(PreparedRepairWrite {
            destination: destination.to_path_buf(),
            temporary: Some(reservation.into_path()),
        })
    }
}

fn prepare_source_write(
    destination: &Path,
    bytes: &[u8],
) -> Result<PreparedRepairWrite, RepairContractError> {
    let permissions = fs::metadata(destination)
        .map_err(|error| {
            RepairContractError::new(format!(
                "cannot read repair source permissions `{}`: {error}",
                destination.display()
            ))
        })?
        .permissions();
    let write = prepare_write(destination, bytes)?;
    let temporary = write
        .temporary
        .as_deref()
        .ok_or_else(|| RepairContractError::new("prepared repair source has no temporary file"))?;
    fs::set_permissions(temporary, permissions).map_err(|error| {
        RepairContractError::new(format!(
            "cannot preserve repair source permissions `{}`: {error}",
            destination.display()
        ))
    })?;
    Ok(write)
}

fn commit_writes(
    mut writes: Vec<PreparedRepairWrite>,
    mut publish: impl FnMut(usize, &Path, &Path) -> std::io::Result<()>,
) -> Result<(), RepairContractError> {
    let mut backups = vec![None; writes.len()];
    for (index, write) in writes.iter().enumerate() {
        match move_to_backup(&write.destination) {
            Ok(backup) => backups[index] = backup,
            Err(error) => {
                return Err(rollback_writes(
                    &writes,
                    &mut backups,
                    0,
                    &error.to_string(),
                ));
            }
        }
    }
    for index in 0..writes.len() {
        let Some(temporary) = writes[index].temporary.take() else {
            return Err(rollback_writes(
                &writes,
                &mut backups,
                index,
                "prepared repair write lost its temporary path",
            ));
        };
        if let Err(error) = publish(index, &temporary, &writes[index].destination) {
            let _ = fs::remove_file(&temporary);
            let primary = format!(
                "cannot publish `{}`: {error}",
                writes[index].destination.display()
            );
            return Err(rollback_writes(&writes, &mut backups, index, &primary));
        }
    }
    for backup in backups.into_iter().flatten() {
        if let Err(error) = fs::remove_file(&backup) {
            eprintln!(
                "warning: repair published but backup `{}` could not be removed: {error}",
                backup.display()
            );
        }
    }
    Ok(())
}

fn move_to_backup(destination: &Path) -> Result<Option<PathBuf>, RepairContractError> {
    match fs::symlink_metadata(destination) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(RepairContractError::new(format!(
                "cannot inspect repair destination `{}`: {error}",
                destination.display()
            )));
        }
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(RepairContractError::new(format!(
                "repair destination `{}` is not a regular non-link file",
                destination.display()
            )));
        }
        Ok(_) => {}
    }
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            RepairContractError::new(format!(
                "invalid repair destination `{}`",
                destination.display()
            ))
        })?;
    for _ in 0..32 {
        let sequence = REPAIR_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let backup = destination.with_file_name(format!(
            ".{name}.pliego-repair-{}-{sequence}.bak",
            std::process::id()
        ));
        if backup.exists() {
            continue;
        }
        fs::rename(destination, &backup).map_err(|error| {
            RepairContractError::new(format!(
                "cannot stage `{}` through `{}`: {error}",
                destination.display(),
                backup.display()
            ))
        })?;
        return Ok(Some(backup));
    }
    Err(RepairContractError::new(format!(
        "cannot allocate a backup beside `{}`",
        destination.display()
    )))
}

fn rollback_writes(
    writes: &[PreparedRepairWrite],
    backups: &mut [Option<PathBuf>],
    published: usize,
    primary: &str,
) -> RepairContractError {
    let mut errors = Vec::new();
    for index in (0..writes.len()).rev() {
        let destination = &writes[index].destination;
        if index < published {
            if let Err(error) = fs::remove_file(destination) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    errors.push(format!(
                        "cannot remove `{}`: {error}",
                        destination.display()
                    ));
                }
            }
        }
        if let Some(backup) = backups[index].take() {
            if let Err(error) = fs::rename(&backup, destination) {
                errors.push(format!(
                    "cannot restore `{}` from `{}`: {error}",
                    destination.display(),
                    backup.display()
                ));
            }
        }
    }
    if errors.is_empty() {
        RepairContractError::new(format!(
            "{primary}; all previous source bytes were restored"
        ))
    } else {
        RepairContractError::new(format!(
            "{primary}; rollback also failed: {}",
            errors.join("; ")
        ))
    }
}
