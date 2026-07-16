//! End-to-end contract for direct compile control groups.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = "fn view() { let _ = pc!(\"flex gap-4\"); }\n";

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must follow Unix epoch")
            .as_nanos();
        for _ in 0..32 {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pliego-cssc-compile-control-{}-{timestamp}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("cannot create `{}`: {error}", path.display()),
            }
        }
        panic!("cannot allocate temporary directory")
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn compile_control_group_is_exact_grouped_and_check_only() {
    let directory = TemporaryDirectory::new();
    let root = directory.path();
    fs::create_dir(root.join("src")).expect("source directory");
    fs::create_dir(root.join("dist")).expect("output directory");
    fs::write(root.join("src/app.rs"), SOURCE).expect("source fixture");
    let arguments = [
        "compile",
        "--source",
        "src",
        "--seed",
        "--output",
        "dist/app.css",
        "--manifest",
        "dist/app.manifest.json",
        "--control-dir",
        "dist",
    ];
    assert_success(&run(root, &arguments));

    let manifest_bytes =
        fs::read(root.join("dist/pliego.css.manifest.json")).expect("control manifest must exist");
    let manifest = pliego_css_control::parse_control_manifest(&manifest_bytes)
        .expect("control manifest must be canonical");
    let receipt_bytes =
        fs::read(root.join("dist/pliego.css.receipt.json")).expect("control receipt must exist");
    let receipt = pliego_css_control::parse_build_receipt(&receipt_bytes, &manifest_bytes)
        .expect("receipt must bind exact manifest bytes");
    assert_eq!(receipt.result, pliego_css_control::ReceiptResult::Passed);
    assert_seed_token_graph(root, &manifest);
    assert_eq!(
        manifest
            .inputs
            .files
            .iter()
            .map(|input| (input.file.as_str(), input.role.as_str()))
            .collect::<Vec<_>>(),
        [
            ("pliego.compile.json", "compiler-config"),
            ("src/app.rs", "rust-source"),
        ]
    );
    assert_eq!(
        manifest
            .outputs
            .iter()
            .map(|output| (output.artifact.file.as_str(), output.role.as_str()))
            .collect::<Vec<_>>(),
        [
            ("app.css", "generated-css"),
            ("app.css.map", "css-source-map"),
            ("app.manifest.json", "style-manifest"),
            ("pliego.css.findings.json", "audit-findings"),
            ("pliego.tokens.json", "token-graph"),
        ]
    );
    assert_control_source_map(root, &manifest);
    let source_hash = manifest.inputs.source_hash;
    let config_hash = manifest.inputs.config_hash;

    fs::write(root.join("src/app.rs"), format!("{SOURCE}\n")).expect("change exact source bytes");
    assert_success(&run(root, &arguments));
    let changed = pliego_css_control::parse_control_manifest(
        &fs::read(root.join("dist/pliego.css.manifest.json")).expect("changed manifest"),
    )
    .expect("changed manifest remains canonical");
    assert_ne!(changed.inputs.source_hash, source_hash);
    assert_eq!(changed.inputs.config_hash, config_hash);

    let before_check = snapshot(&root.join("dist"));
    let mut check = arguments.to_vec();
    check.push("--check");
    assert_success(&run(root, &check));
    assert_eq!(snapshot(&root.join("dist")), before_check);

    assert_source_map_drift_is_read_only(root, &arguments, &check);
    assert_token_graph_drift_is_read_only(root, &arguments, &check);

    fs::write(root.join("dist/pliego.css.receipt.json"), "drift\n").expect("receipt drift");
    let drifted = snapshot(&root.join("dist"));
    assert_failure(&run(root, &check), "controlled compile drift detected");
    assert_eq!(snapshot(&root.join("dist")), drifted);

    fs::create_dir(root.join("literal")).expect("literal output directory");
    assert_success(&run(
        root,
        &[
            "compile",
            "--style",
            "flex gap-4",
            "--seed",
            "--output",
            "literal/app.css",
            "--manifest",
            "literal/app.manifest.json",
            "--control-dir",
            "literal",
        ],
    ));
    let literal = pliego_css_control::parse_control_manifest(
        &fs::read(root.join("literal/pliego.css.manifest.json")).expect("literal manifest"),
    )
    .expect("literal manifest must be canonical");
    assert_literal_control(root, &literal);

    assert_output_cannot_escape_control_root(root);
}

fn assert_control_source_map(root: &Path, manifest: &pliego_css_control::ControlManifest) {
    let css_output = manifest
        .outputs
        .iter()
        .find(|output| output.role == "generated-css")
        .expect("generated CSS output");
    let source_map_output = manifest
        .outputs
        .iter()
        .find(|output| output.role == "css-source-map")
        .expect("source-map output");
    assert_eq!(
        css_output.source_map.as_ref(),
        Some(&source_map_output.artifact)
    );
    assert_eq!(source_map_output.relationships, ["app.css"]);
    assert_eq!(
        css_output.relationships,
        ["app.css.map", "app.manifest.json", "pliego.tokens.json"]
    );
    let source_map: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("dist/app.css.map")).expect("source map must exist"),
    )
    .expect("source map must be JSON");
    assert_eq!(source_map["version"], 3);
    assert_eq!(source_map["sources"], serde_json::json!(["src/app.rs"]));
    assert!(!source_map["mappings"].as_str().unwrap().is_empty());
}

fn assert_literal_control(root: &Path, manifest: &pliego_css_control::ControlManifest) {
    assert!(
        manifest
            .inputs
            .files
            .iter()
            .any(|input| { input.file == "pliego.cli-input.json" && input.role == "cli-source" })
    );
    let source_map: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("literal/app.css.map")).expect("literal source map"),
    )
    .expect("literal source map must be JSON");
    assert_eq!(
        source_map["sources"],
        serde_json::json!(["pliego.cli-input.json"])
    );
}

fn assert_source_map_drift_is_read_only(root: &Path, arguments: &[&str], check: &[&str]) {
    fs::write(root.join("dist/app.css.map"), "drift\n").expect("source-map drift");
    let drifted = snapshot(&root.join("dist"));
    assert_failure(&run(root, check), "app.css.map");
    assert_eq!(snapshot(&root.join("dist")), drifted);
    assert_success(&run(root, arguments));
}

fn assert_token_graph_drift_is_read_only(root: &Path, arguments: &[&str], check: &[&str]) {
    fs::write(root.join("dist/pliego.tokens.json"), "drift\n").expect("token-graph drift");
    let drifted = snapshot(&root.join("dist"));
    assert_failure(&run(root, check), "pliego.tokens.json");
    assert_eq!(snapshot(&root.join("dist")), drifted);
    assert_success(&run(root, arguments));
}

fn assert_seed_token_graph(root: &Path, manifest: &pliego_css_control::ControlManifest) {
    let tokens = &manifest.tokens;
    assert_eq!(
        tokens.observation,
        pliego_css_control::MeasurementState::Measured
    );
    assert_eq!(
        tokens.graph_version.as_deref(),
        Some("pliegocss-token-graph/1")
    );
    let declared = u64::try_from(pliego_css_theme::ThemeRegistry::seed().tokens().len())
        .expect("seed token count must fit u64");
    assert_eq!(tokens.tokens, declared);
    assert_eq!((tokens.aliases, tokens.themes), (0, 1));
    assert_eq!(
        tokens.coverage_basis_points,
        Some(u16::try_from(10_000 / declared).expect("coverage must fit u16"))
    );
    let graph_bytes =
        fs::read(root.join("dist/pliego.tokens.json")).expect("canonical token graph must exist");
    let graph = pliego_css_config::parse_token_graph(&graph_bytes)
        .expect("published token graph must be canonical");
    assert_eq!(graph.themes.len(), 1);
    let graph_output = manifest
        .outputs
        .iter()
        .find(|output| output.role == "token-graph")
        .expect("control manifest must describe the token graph");
    assert_eq!(
        Some(graph_output.artifact.sha256.as_str()),
        tokens.graph_hash.as_deref()
    );
    assert_eq!(graph_output.relationships, ["app.css", "app.manifest.json"]);
}

fn assert_output_cannot_escape_control_root(root: &Path) {
    fs::create_dir(root.join("escaped")).expect("escaped output directory");
    let before_escape = snapshot(&root.join("dist"));
    let output = run(
        root,
        &[
            "compile",
            "--style",
            "flex",
            "--seed",
            "--output",
            "escaped/app.css",
            "--manifest",
            "dist/app.manifest.json",
            "--control-dir",
            "dist",
        ],
    );
    assert_failure(&output, "must be inside `--control-dir`");
    assert_eq!(snapshot(&root.join("dist")), before_escape);
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
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &Output, expected: &str) {
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "missing `{expected}` in {stderr}"
    );
}

fn snapshot(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(directory)
        .expect("snapshot directory")
        .map(|entry| {
            let entry = entry.expect("snapshot entry");
            (
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path()).expect("snapshot file"),
            )
        })
        .collect()
}
