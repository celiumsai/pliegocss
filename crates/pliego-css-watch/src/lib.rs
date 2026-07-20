//! Minimal native filesystem-event wakeups for `PliegoCSS` watch mode.

#![allow(unsafe_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

/// One immutable file read used by a watch snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSnapshot {
    /// Input path captured by this snapshot.
    pub path: PathBuf,
    /// Exact bytes or the stable read failure observed for this capture.
    pub contents: Result<Vec<u8>, String>,
}

impl FileSnapshot {
    /// Returns the captured bytes or read failure.
    ///
    /// # Errors
    ///
    /// Returns the read error captured with the snapshot.
    pub fn bytes(&self) -> Result<&[u8], String> {
        self.contents.as_deref().map_err(Clone::clone)
    }

    /// Decodes captured bytes as UTF-8 under the supplied input role.
    ///
    /// # Errors
    ///
    /// Returns the captured read error or a UTF-8 decoding failure.
    pub fn utf8(&self, role: &str) -> Result<&str, String> {
        std::str::from_utf8(self.bytes()?).map_err(|error| {
            format!(
                "{role} `{}` is not valid UTF-8: {error}",
                self.path.display()
            )
        })
    }
}

/// Captures one file as exact bytes without retrying or consulting metadata.
#[must_use]
pub fn snapshot_file(path: &Path, role: &str) -> FileSnapshot {
    FileSnapshot {
        path: path.to_path_buf(),
        contents: pliego_css_io::read_bounded_regular_file(path, 16 * 1024 * 1024, role),
    }
}

/// Keeps native filesystem watches alive and coalesces their events into wakeups.
///
/// Events are only scheduling hints. Callers must recapture and compare their complete input
/// snapshot after every wakeup and timeout.
pub struct FilesystemWake {
    events: Receiver<()>,
}

/// Chooses native wakeups when available and bounded polling otherwise.
pub struct WatchScheduler {
    native: Option<FilesystemWake>,
    poll: Duration,
}

impl WatchScheduler {
    /// Builds a scheduler and returns its human-readable active mode.
    pub fn new(
        paths: impl IntoIterator<Item = PathBuf>,
        poll: Duration,
        fallback: Duration,
    ) -> (Self, String) {
        match FilesystemWake::new(paths) {
            Ok(native) => (
                Self {
                    native: Some(native),
                    poll,
                },
                format!(
                    "filesystem events; snapshot fallback every {} ms",
                    fallback.as_millis()
                ),
            ),
            Err(error) => (
                Self { native: None, poll },
                format!("polling every {} ms; {error}", poll.as_millis()),
            ),
        }
    }

    /// Waits for a native event or the requested timeout, bounded by the polling interval when the
    /// native backend is unavailable.
    pub fn wait(&self, timeout: Duration) {
        if let Some(native) = &self.native {
            let _ = native.wait(timeout);
        } else {
            std::thread::sleep(timeout.min(self.poll));
        }
    }
}

/// Requires a changed value to appear twice before accepting it.
pub fn confirmed<T: Clone + PartialEq>(
    previous: Option<&T>,
    pending: &mut Option<T>,
    current: &T,
) -> bool {
    if previous == Some(current) {
        *pending = None;
        true
    } else if pending.as_ref() == Some(current) {
        true
    } else {
        *pending = Some(current.clone());
        false
    }
}

impl FilesystemWake {
    /// Watches existing directories recursively and file parents non-recursively.
    ///
    /// File-parent watches preserve notifications when an editor replaces a file atomically.
    ///
    /// # Errors
    ///
    /// Returns an error when inputs cannot be resolved or no certified native backend exists.
    pub fn new(paths: impl IntoIterator<Item = PathBuf>) -> Result<Self, String> {
        let roots = watch_roots(paths)?;
        if roots.is_empty() {
            return Err("filesystem events require at least one existing input root".into());
        }
        let (sender, events) = mpsc::channel();
        platform::start(roots, sender)?;
        Ok(Self { events })
    }

    /// Waits for one or more native events, or returns after the fallback timeout.
    #[must_use]
    pub fn wait(&self, timeout: Duration) -> bool {
        match self.events.recv_timeout(timeout) {
            Ok(()) => {
                while self.events.try_recv().is_ok() {}
                true
            }
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => false,
        }
    }
}

fn watch_roots(
    paths: impl IntoIterator<Item = PathBuf>,
) -> Result<BTreeMap<PathBuf, bool>, String> {
    let mut roots = BTreeMap::new();
    for path in paths {
        let metadata = path
            .metadata()
            .map_err(|error| format!("cannot inspect watch input `{}`: {error}", path.display()))?;
        let (root, recursive) = if metadata.is_dir() {
            (path, true)
        } else {
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            (parent.to_path_buf(), false)
        };
        let root = root
            .canonicalize()
            .map_err(|error| format!("cannot resolve watch root `{}`: {error}", root.display()))?;
        roots
            .entry(root)
            .and_modify(|current| *current |= recursive)
            .or_insert(recursive);
    }
    Ok(roots)
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{BTreeMap, Path, PathBuf};
    use inotify::{Inotify, WatchMask};
    use std::fs;
    use std::sync::mpsc::Sender;

    const MASK: WatchMask = WatchMask::ACCESS
        .union(WatchMask::ATTRIB)
        .union(WatchMask::CLOSE_WRITE)
        .union(WatchMask::CREATE)
        .union(WatchMask::DELETE)
        .union(WatchMask::DELETE_SELF)
        .union(WatchMask::MODIFY)
        .union(WatchMask::MOVE_SELF)
        .union(WatchMask::MOVED_FROM)
        .union(WatchMask::MOVED_TO);

    pub(super) fn start(roots: BTreeMap<PathBuf, bool>, sender: Sender<()>) -> Result<(), String> {
        let mut watcher = Inotify::init()
            .map_err(|error| format!("cannot initialize Linux filesystem events: {error}"))?;
        register(&watcher, &roots)?;
        std::thread::Builder::new()
            .name("pliego-css-watch".into())
            .spawn(move || {
                let mut buffer = [0; 4096];
                while watcher.read_events_blocking(&mut buffer).is_ok() {
                    let _ = sender.send(());
                    let _ = register(&watcher, &roots);
                }
            })
            .map(|_| ())
            .map_err(|error| format!("cannot start Linux filesystem-event thread: {error}"))
    }

    fn register(watcher: &Inotify, roots: &BTreeMap<PathBuf, bool>) -> Result<(), String> {
        for (root, recursive) in roots {
            add(watcher, root)?;
            if *recursive {
                let mut pending = vec![root.clone()];
                while let Some(directory) = pending.pop() {
                    for entry in fs::read_dir(&directory).map_err(|error| {
                        format!(
                            "cannot inventory watch root `{}`: {error}",
                            directory.display()
                        )
                    })? {
                        let path = entry
                            .map_err(|error| format!("cannot inventory watch root: {error}"))?
                            .path();
                        if path.is_dir() {
                            add(watcher, &path)?;
                            pending.push(path);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn add(watcher: &Inotify, path: &Path) -> Result<(), String> {
        watcher
            .watches()
            .add(path, MASK)
            .map(|_| ())
            .map_err(|error| format!("cannot watch `{}`: {error}", path.display()))
    }
}

#[cfg(windows)]
mod platform {
    use super::{BTreeMap, PathBuf};
    use std::os::windows::ffi::OsStrExt;
    use std::sync::mpsc::Sender;
    use windows_sys::Win32::Foundation::{INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME,
        FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SIZE, FindCloseChangeNotification,
        FindFirstChangeNotificationW, FindNextChangeNotification,
    };
    use windows_sys::Win32::System::Threading::{INFINITE, WaitForSingleObject};

    pub(super) fn start(roots: BTreeMap<PathBuf, bool>, sender: Sender<()>) -> Result<(), String> {
        for (root, recursive) in roots {
            let wide = root
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>();
            // SAFETY: `wide` is NUL-terminated and remains alive for the duration of the call.
            let handle = unsafe {
                FindFirstChangeNotificationW(
                    wide.as_ptr(),
                    recursive.into(),
                    FILE_NOTIFY_CHANGE_FILE_NAME
                        | FILE_NOTIFY_CHANGE_DIR_NAME
                        | FILE_NOTIFY_CHANGE_ATTRIBUTES
                        | FILE_NOTIFY_CHANGE_SIZE
                        | FILE_NOTIFY_CHANGE_LAST_WRITE,
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                return Err(format!("cannot watch Windows root `{}`", root.display()));
            }
            let raw = handle as usize;
            let sender = sender.clone();
            std::thread::Builder::new()
                .name("pliego-css-watch".into())
                .spawn(move || {
                    // SAFETY: the valid notification handle is owned by this thread and closed
                    // exactly once after the wait/rearm loop ends.
                    unsafe {
                        let handle = raw as *mut core::ffi::c_void;
                        while WaitForSingleObject(handle, INFINITE) == WAIT_OBJECT_0 {
                            let _ = sender.send(());
                            if FindNextChangeNotification(handle) == 0 {
                                break;
                            }
                        }
                        FindCloseChangeNotification(handle);
                    }
                })
                .map_err(|error| {
                    format!("cannot start Windows filesystem-event thread: {error}")
                })?;
        }
        drop(sender);
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod platform {
    use super::{BTreeMap, PathBuf};
    use std::sync::mpsc::Sender;

    pub(super) fn start(
        _roots: BTreeMap<PathBuf, bool>,
        _sender: Sender<()>,
    ) -> Result<(), String> {
        Err("native filesystem events are not yet certified on this platform".into())
    }
}

#[cfg(test)]
mod tests {
    use super::{snapshot_file, watch_roots};
    use std::fs;
    #[cfg(any(target_os = "linux", windows))]
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn roots_keep_recursive_directories_and_stable_file_parents() {
        let root = std::env::temp_dir().join(format!(
            "pliego-watch-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let directory = root.join("src");
        fs::create_dir_all(&directory).expect("create source directory");
        let file = root.join("theme.toml");
        fs::write(&file, "[theme]").expect("write file");
        let roots = watch_roots([directory.clone(), file]).expect("watch roots");
        assert_eq!(roots.get(&directory.canonicalize().unwrap()), Some(&true));
        assert_eq!(roots.get(&root.canonicalize().unwrap()), Some(&false));
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn native_backend_wakes_before_timeout() {
        let root = std::env::temp_dir().join(format!(
            "pliego-watch-event-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir(&root).expect("create watch root");
        let wake = super::FilesystemWake::new([root.clone()]).expect("native backend");
        fs::write(root.join("changed.rs"), "fn changed() {}").expect("write event");
        assert!(wake.wait(Duration::from_secs(2)));
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn snapshots_reject_oversized_and_non_regular_inputs() {
        let root = std::env::temp_dir().join(format!("pliego-watch-safe-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let large = root.join("large.rs");
        fs::write(&large, vec![b'x'; 16 * 1024 * 1024 + 1]).unwrap();
        assert!(
            snapshot_file(&large, "Rust source")
                .bytes()
                .unwrap_err()
                .contains("bounded regular file")
        );
        assert!(
            snapshot_file(&root, "Rust source")
                .bytes()
                .unwrap_err()
                .contains("bounded regular file")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn snapshots_reject_links() {
        let root = std::env::temp_dir().join(format!("pliego-watch-link-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("target.rs");
        let link = root.join("link.rs");
        fs::write(&target, "fn main() {}").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        #[cfg(windows)]
        if std::os::windows::fs::symlink_file(&target, &link).is_err() {
            fs::remove_dir_all(root).unwrap();
            return;
        }
        assert!(
            snapshot_file(&link, "Rust source")
                .bytes()
                .unwrap_err()
                .contains("bounded regular file")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
