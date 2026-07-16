//! End-to-end contract for report-only usage analysis without application evidence.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must follow the Unix epoch")
            .as_nanos();
        for _ in 0..32 {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pliego-cssc-usage-report-{}-{timestamp}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("cannot create `{}`: {error}", path.display()),
            }
        }
        panic!("cannot allocate a usage-report fixture directory");
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("warning: cannot remove `{}`: {error}", self.0.display());
        }
    }
}

#[test]
fn usage_report_without_application_evidence_is_unknown_read_only_and_checkable() {
    let fixture = TemporaryDirectory::new();
    let root = &fixture.0;
    fs::create_dir(root.join("src")).unwrap();
    fs::create_dir(root.join("baseline")).unwrap();
    fs::create_dir(root.join("reported")).unwrap();
    fs::write(
        root.join("src/styles.rs"),
        "fn view() { let _ = pc!(\"block p-4\"); }\n",
    )
    .unwrap();
    fs::write(
        root.join("pliego.bundles.toml"),
        r#"schema = 1
targets = "modern"
format = "minified"

[theme]
kind = "seed"

[bundles.application]
sources = ["src/styles.rs"]
emit-theme = false
"#,
    )
    .unwrap();

    let baseline = run(
        root,
        &[
            "bundle",
            "--plan",
            "pliego.bundles.toml",
            "--output-dir",
            "baseline",
        ],
    );
    assert_success(&baseline);
    let arguments = [
        "bundle",
        "--plan",
        "pliego.bundles.toml",
        "--output-dir",
        "reported",
        "--usage-report",
    ];
    assert_success(&run(root, &arguments));
    assert_eq!(
        fs::read(root.join("baseline/application.css")).unwrap(),
        fs::read(root.join("reported/application.css")).unwrap(),
    );
    assert_eq!(
        fs::read(root.join("baseline/application.manifest.json")).unwrap(),
        fs::read(root.join("reported/application.manifest.json")).unwrap(),
    );

    let report_path = root.join("reported/pliego.usage.json");
    let report_bytes = fs::read(&report_path).unwrap();
    let report: Value = serde_json::from_slice(&report_bytes).unwrap();
    assert_eq!(report["application"]["state"], "unavailable");
    assert_eq!(report["observation"]["state"], "unavailable");
    assert_eq!(report["summary"]["entries"], 1);
    assert_eq!(report["summary"]["staticUnknown"], 1);
    assert_eq!(report["summary"]["usageUnknown"], 1);
    assert_eq!(report["summary"]["removalBlocked"], 1);
    assert_eq!(report["styles"][0]["staticReachability"], "unknown");
    assert_eq!(report["styles"][0]["observationState"], "unavailable");
    assert_eq!(report["styles"][0]["usageState"], "unknown");
    assert_eq!(report["styles"][0]["removalDisposition"], "blocked");
    pliego_css_usage::parse_usage_analysis(&report_bytes).unwrap();

    assert_success(&run(root, &arguments));
    assert_eq!(fs::read(&report_path).unwrap(), report_bytes);
    let mut check = arguments.to_vec();
    check.push("--check");
    assert_success(&run(root, &check));
    assert_eq!(fs::read(&report_path).unwrap(), report_bytes);

    fs::write(&report_path, b"drift\n").unwrap();
    let failure = run(root, &check);
    assert!(!failure.status.success());
    assert!(stderr(&failure).contains("bundle output drift detected"));
    assert_eq!(fs::read(&report_path).unwrap(), b"drift\n");
}

fn run(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("pliego-cssc must execute")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr(output),
    );
    assert!(output.stderr.is_empty());
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
