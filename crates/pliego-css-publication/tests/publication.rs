//! Publication transaction integration contracts.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use pliego_css_publication as publication;

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn create(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "pliego-css-publication-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary test directory");
        Self(path)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn coordination_entries(root: &Path) -> Vec<String> {
    let mut entries = fs::read_dir(root)
        .expect("read publication directory")
        .map(|entry| {
            entry
                .expect("publication entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.contains(".pliego-test-") || name.ends_with(".pliego.lock"))
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

#[cfg(unix)]
#[test]
fn lock_rejects_a_leaf_symlink() {
    use std::os::unix::fs::symlink;

    let root = TempDirectory::create("publication-lock-symlink");
    let target = root.0.join("target");
    let lock = root.0.join("output.pliego.lock");
    fs::write(&target, b"do not lock through this").unwrap();
    symlink(&target, &lock).unwrap();

    let error = publication::PublicationLock::acquire(&lock).expect_err("symlink lock must fail");
    assert!(matches!(
        error.kind(),
        io::ErrorKind::InvalidInput | io::ErrorKind::Other
    ));
    assert_eq!(fs::read(target).unwrap(), b"do not lock through this");
}

#[test]
fn sibling_reservations_skip_collisions_and_retain_the_created_identity() {
    let root = TempDirectory::create("publication-reservation");
    let destination = root.0.join("output.css");
    let first = publication::ReservedSibling::create(&destination, "pliego-test", "tmp", 8)
        .expect("first reservation");
    let first_path = first.path().to_path_buf();
    let second = publication::ReservedSibling::create(&destination, "pliego-test", "tmp", 8)
        .expect("second reservation");

    assert_ne!(first.path(), second.path());
    assert!(first.has_reserved_identity().expect("first identity"));
    assert!(second.has_reserved_identity().expect("second identity"));
    drop(first);
    drop(second);
    assert!(!first_path.exists());
    assert!(coordination_entries(&root.0).is_empty());
}

#[test]
fn transaction_rolls_back_when_the_second_publish_fails_and_cleans_coordination_files() {
    let root = TempDirectory::create("publication-rollback");
    let first = root.0.join("first.css");
    let second = root.0.join("second.json");
    fs::write(&first, b"old-css").unwrap();
    fs::write(&second, b"old-json").unwrap();

    let writes = vec![
        publication::WriteRequest::new(&first, b"new-css"),
        publication::WriteRequest::new(&second, b"new-json"),
    ];
    let error = publication::publish_with(
        &writes,
        publication::PublicationOptions::new("pliego-test").durable(false),
        |index, temporary, destination| {
            if index == 1 {
                Err(io::Error::other("injected second publish failure"))
            } else {
                fs::rename(temporary, destination)
            }
        },
    )
    .expect_err("second publish must fail");

    assert!(
        error
            .to_string()
            .contains("injected second publish failure")
    );
    assert_eq!(fs::read(first).unwrap(), b"old-css");
    assert_eq!(fs::read(second).unwrap(), b"old-json");
    assert!(coordination_entries(&root.0).is_empty());
}

#[test]
fn dropped_preparation_cleans_temporary_files() {
    let root = TempDirectory::create("publication-cleanup");
    let destination = root.0.join("output.css");
    let prepared = publication::PreparedWrite::new(&destination, b"bytes", "pliego-test", false)
        .expect("prepare write");
    assert!(prepared.temporary_path().exists());
    drop(prepared);
    assert!(coordination_entries(&root.0).is_empty());
}

#[cfg(unix)]
#[test]
fn durable_publication_syncs_the_parent_directory_where_supported() {
    let root = TempDirectory::create("publication-directory-sync");
    let destination = root.0.join("output.css");
    publication::publish(
        &[publication::WriteRequest::new(&destination, b"bytes")],
        publication::PublicationOptions::new("pliego-test").durable(true),
    )
    .expect("durable publication");
    assert_eq!(fs::read(destination).unwrap(), b"bytes");
    assert!(coordination_entries(&root.0).is_empty());
}
