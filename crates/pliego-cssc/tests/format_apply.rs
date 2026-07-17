//! Black-box contract for explicit Rust source formatting fixes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

fn fixture(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "pliego-cssc-format-{label}-{}-{}",
        std::process::id(),
        NEXT_TEST.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).expect("create formatter fixture");
    root
}

fn run(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("run pliego-cssc")
}

#[test]
fn apply_is_explicit_multi_file_and_idempotent() {
    let root = fixture("apply");
    let first = root.join("first.rs");
    let second = root.join("second.rs");
    fs::write(&first, "fn a(){let _=pc!(\" flex   gap-4 \");}\n").expect("write first");
    #[cfg(unix)]
    fs::set_permissions(&first, fs::Permissions::from_mode(0o744))
        .expect("set executable source permissions");
    fs::write(
        &second,
        "fn b(on:bool){let _=pcx!(\"grid  gap-4\",if on{\" block \"}else{\"hidden\"});}\n",
    )
    .expect("write second");

    let check = run(&root, &["fmt", "--source", ".", "--check"]);
    assert_eq!(check.status.code(), Some(1));
    assert!(String::from_utf8(check.stderr).unwrap().contains("FMT001"));

    let apply = run(&root, &["fmt", "--source", ".", "--apply"]);
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    assert_eq!(String::from_utf8(apply.stdout).unwrap(), "fixed\n");
    assert_eq!(
        fs::read_to_string(&first).expect("first result"),
        "fn a(){let _=pc!(\"flex gap-4\");}\n"
    );
    assert_eq!(
        fs::read_to_string(&second).expect("second result"),
        "fn b(on:bool){let _=pcx!(\"grid gap-4\",if on{\"block\"}else{\"hidden\"});}\n"
    );
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&first)
            .expect("first metadata")
            .permissions()
            .mode()
            & 0o777,
        0o744
    );

    let replay = run(&root, &["fmt", "--source", ".", "--apply"]);
    assert!(replay.status.success());
    assert!(
        String::from_utf8(replay.stdout)
            .unwrap()
            .starts_with("ok: 4 Rust utility literal")
    );
    fs::remove_dir_all(root).expect("remove formatter fixture");
}

#[test]
fn unsupported_macro_input_keeps_every_source_unchanged() {
    let root = fixture("unsupported");
    let valid = root.join("valid.rs");
    let invalid = root.join("invalid.rs");
    let before = "fn valid(){let _=pc!(\" flex   gap-4 \");}\n";
    fs::write(&valid, before).expect("write valid source");
    fs::write(&invalid, "fn invalid(v:&str){let _=pc!(v);}\n").expect("write invalid source");

    let output = run(&root, &["fmt", "--source", ".", "--apply"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(fs::read_to_string(&valid).expect("valid unchanged"), before);
    assert!(fs::read_to_string(&invalid).unwrap().contains("pc!(v)"));
    fs::remove_dir_all(root).expect("remove formatter fixture");
}
