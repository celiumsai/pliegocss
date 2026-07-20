//! Contract tests for bounded regular-file reads.

use std::fs;

use pliego_css_io::{
    read_bounded_regular_file, rename_prepared_synced, write_new_synced_regular_file,
};

fn fixture(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("pliego-io-{name}-{}", std::process::id()))
}

#[test]
fn reads_regular_files_up_to_the_limit() {
    let root = fixture("regular");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("input.css");
    fs::write(&path, b"abc").unwrap();
    assert_eq!(
        read_bounded_regular_file(&path, 3, "input").unwrap(),
        b"abc"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_oversized_and_non_regular_inputs_with_stable_messages() {
    let root = fixture("bounded");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("input.css");
    fs::write(&path, b"abcd").unwrap();
    assert!(
        read_bounded_regular_file(&path, 3, "input")
            .unwrap_err()
            .contains("input `")
    );
    assert!(
        read_bounded_regular_file(&root, 1024, "input")
            .unwrap_err()
            .contains("bounded regular file")
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(any(unix, windows))]
#[test]
fn rejects_file_and_parent_links() {
    let root = fixture("links");
    fs::create_dir_all(root.join("real")).unwrap();
    fs::write(root.join("real/input.css"), b"abc").unwrap();
    let file_link = root.join("input.css");
    let parent_link = root.join("linked");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("real/input.css"), &file_link).unwrap();
        std::os::unix::fs::symlink(root.join("real"), &parent_link).unwrap();
    }
    #[cfg(windows)]
    {
        if std::os::windows::fs::symlink_file(root.join("real/input.css"), &file_link).is_err()
            || std::os::windows::fs::symlink_dir(root.join("real"), &parent_link).is_err()
        {
            fs::remove_dir_all(root).unwrap();
            return;
        }
    }
    assert!(
        read_bounded_regular_file(&file_link, 1024, "input")
            .unwrap_err()
            .contains("bounded regular file")
    );
    assert!(
        read_bounded_regular_file(&parent_link.join("input.css"), 1024, "input")
            .unwrap_err()
            .contains("unsafe path component")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn creates_once_and_renames_prepared_bytes_over_destination() {
    let root = fixture("publication");
    fs::create_dir_all(&root).unwrap();
    let prepared = root.join("prepared.tmp");
    let destination = root.join("output.css");
    fs::write(&destination, b"before").unwrap();
    write_new_synced_regular_file(&prepared, b"after", "output").unwrap();
    assert!(write_new_synced_regular_file(&prepared, b"other", "output").is_err());
    rename_prepared_synced(&prepared, &destination, "output").unwrap();
    assert_eq!(fs::read(&destination).unwrap(), b"after");
    assert!(!prepared.exists());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(any(unix, windows))]
#[test]
fn rejects_writing_through_a_link_like_parent() {
    let root = fixture("write-links");
    fs::create_dir_all(root.join("real")).unwrap();
    let linked = root.join("linked");
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("real"), &linked).unwrap();
    #[cfg(windows)]
    if std::os::windows::fs::symlink_dir(root.join("real"), &linked).is_err() {
        fs::remove_dir_all(root).unwrap();
        return;
    }
    assert!(write_new_synced_regular_file(&linked.join("output"), b"bytes", "output").is_err());
    assert!(!root.join("real/output").exists());
    fs::remove_dir_all(root).unwrap();
}
