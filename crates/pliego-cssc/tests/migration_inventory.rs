//! Black-box contract for read-only migration inventories.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "pliegocss-migration-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("temporary directory");
    path
}

fn run(directory: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .current_dir(directory)
        .args(arguments)
        .output()
        .expect("migration-inventory command")
}

#[test]
fn inventories_each_family_with_canonical_output() {
    let directory = temp_dir("families");
    let fixtures = [
        (
            "sass",
            "src/app.scss",
            "$gap: 1rem;\n@mixin card { padding: $gap; }\n",
            "sass-mixin",
            "not-applicable",
        ),
        (
            "tailwind",
            "src/app.css",
            "@import \"tailwindcss\";\n@source inline(\"bg-red-{100..900..100}\");\n",
            "tailwind-source",
            "implicit",
        ),
        (
            "css-modules",
            "src/card.module.css",
            ".card { composes: base from \"./base.module.css\"; }\n",
            "css-modules-composes",
            "not-applicable",
        ),
    ];
    fs::create_dir(directory.join("src")).expect("source directory");
    for (kind, input, source, construct, preflight) in &fixtures {
        fs::write(directory.join(input), source).expect("source fixture");
        let output = run(&directory, &["migration-inventory", kind, input]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.ends_with(b"\n"));
        let document: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("inventory JSON");
        assert_eq!(document["schemaVersion"], 1);
        assert_eq!(document["sourceKind"], *kind);
        assert_eq!(document["file"], *input);
        assert_eq!(document["sourceBytes"], source.len());
        assert_eq!(document["sourceSha256"].as_str().map(str::len), Some(64));
        assert_eq!(document["preflightReliance"], *preflight);
        assert!(
            document["constructs"]
                .as_array()
                .expect("constructs")
                .iter()
                .any(|item| item["kind"] == *construct)
        );
    }
    fs::remove_dir_all(directory).expect("remove fixture");
}

#[test]
fn rejects_kind_mismatch_and_unsafe_paths() {
    let directory = temp_dir("failures");
    fs::write(directory.join("app.css"), "@import \"tailwindcss\";\n").expect("source fixture");

    let mismatch = run(
        &directory,
        &["migration-inventory", "css-modules", "app.css"],
    );
    assert!(!mismatch.status.success());
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("does not accept `app.css`"));

    let unsafe_path = run(
        &directory,
        &["migration-inventory", "tailwind", "../app.css"],
    );
    assert!(!unsafe_path.status.success());
    assert!(
        String::from_utf8_lossy(&unsafe_path.stderr)
            .contains("requires a portable project-relative file")
    );

    fs::remove_dir_all(directory).expect("remove fixture");
}
