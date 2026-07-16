//! Public CLI regression coverage for deterministic bundle compilation and publication.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use pliego_css_build::artifacts::sha256_hex;
use pliego_css_compiler::STYLE_ID_FORMAT_VERSION;
use pliego_css_ir::CLASS_NAME_FORMAT_VERSION;
use pliego_css_theme::THEME_ID_FORMAT_VERSION;
use serde_json::Value;

struct TemporaryTree(PathBuf);

impl TemporaryTree {
    fn new() -> Self {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("pliego-cssc must live under the workspace crates directory");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos();
        let path = workspace
            .join("target/tests/bundle-regression")
            .join(format!("{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).expect("bundle regression directory must be created");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryTree {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!(
                "warning: cannot remove bundle regression directory `{}`: {error}",
                self.0.display()
            );
        }
    }
}

#[test]
fn bundle_public_cli_regression_contract() {
    let temporary = TemporaryTree::new();
    verify_seed_determinism_provenance_and_check(temporary.path());
    verify_late_failure_publishes_nothing(temporary.path());
    verify_plan_relative_config_theme(temporary.path());
    verify_plan_output_alias_is_rejected(temporary.path());
}

#[allow(clippy::too_many_lines)]
fn verify_seed_determinism_provenance_and_check(base: &Path) {
    let root = base.join("seed");
    fs::create_dir_all(root.join("dist")).expect("seed output directory must be created");
    write_file(
        &root.join("src/global.rs"),
        r#"fn global() { let _ = pc!("font-sans bg-surface"); }"#,
    );
    write_file(
        &root.join("src/home.rs"),
        r#"fn home() { let _ = pc!("grid gap-4"); }"#,
    );
    write_file(
        &root.join("src/visit.rs"),
        r#"fn visit() { let _ = pc!("flex gap-4"); }"#,
    );
    write_file(
        &root.join("src/shared.rs"),
        r#"fn shared() { let _ = pc!("block"); }"#,
    );
    let plan = root.join("pliego.bundles.toml");
    write_file(&plan, seed_bundle_plan(false));

    let first_run = run_bundle(&root, "pliego.bundles.toml", "dist", false);
    assert_success(&first_run);
    let baseline = output_snapshot(&root.join("dist"));
    assert_eq!(
        baseline.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "global.css",
            "global.manifest.json",
            "home.css",
            "home.manifest.json",
            "visit.css",
            "visit.manifest.json",
        ]
    );

    let global_css = utf8(&baseline["global.css"]);
    let home_css = utf8(&baseline["home.css"]);
    let visit_css = utf8(&baseline["visit.css"]);
    assert!(global_css.contains(":root{"));
    assert!(!home_css.contains(":root{"));
    assert!(!visit_css.contains(":root{"));
    assert!(global_css.contains("display:block"));
    assert!(home_css.contains("display:block"));
    assert!(visit_css.contains("display:block"));
    assert!(home_css.contains("display:grid"));
    assert!(!home_css.contains("display:flex"));
    assert!(visit_css.contains("display:flex"));
    assert!(!visit_css.contains("display:grid"));

    let shared_source =
        fs::read(root.join("src/shared.rs")).expect("shared source must be readable");
    let shared_style_ids = ["global", "home", "visit"]
        .into_iter()
        .map(|name| {
            let css = &baseline[&format!("{name}.css")];
            let manifest = manifest(&baseline[&format!("{name}.manifest.json")]);
            assert_eq!(manifest["schemaVersion"], 3);
            assert_eq!(manifest["styleIdFormatVersion"], STYLE_ID_FORMAT_VERSION);
            assert_eq!(
                manifest["classNameFormatVersion"],
                CLASS_NAME_FORMAT_VERSION
            );
            assert_eq!(manifest["themeIdFormatVersion"], THEME_ID_FORMAT_VERSION);
            assert_eq!(manifest["targets"], "modern");
            assert_eq!(manifest["format"], "minified");
            assert_eq!(
                manifest["cssBytes"].as_u64(),
                Some(u64::try_from(css.len()).expect("CSS length must fit u64"))
            );
            assert_eq!(manifest["cssSha256"], sha256_hex(css));

            let style = manifest["styles"]
                .as_array()
                .expect("manifest styles must be an array")
                .iter()
                .find(|style| {
                    style["origins"]
                        .as_array()
                        .expect("style origins must be an array")
                        .iter()
                        .any(|origin| origin["source"] == "block")
                })
                .expect("every bundle must retain the shared block style");
            let origin = style["origins"]
                .as_array()
                .expect("shared origins must be an array")
                .iter()
                .find(|origin| origin["source"] == "block")
                .expect("shared origin must exist");
            assert_eq!(origin["file"], "src/shared.rs");
            assert_eq!(origin["macroKind"], "pc");
            assert_eq!(origin["reason"], "visible-literal");
            let start = usize::try_from(
                origin["byteStart"]
                    .as_u64()
                    .expect("origin byteStart must be an integer"),
            )
            .expect("origin byteStart must fit usize");
            let end = usize::try_from(
                origin["byteEnd"]
                    .as_u64()
                    .expect("origin byteEnd must be an integer"),
            )
            .expect("origin byteEnd must fit usize");
            assert_eq!(&shared_source[start..end], br#"pc!("block")"#);
            style["styleId"]
                .as_str()
                .expect("shared style ID must be a string")
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(shared_style_ids.len(), 1);

    let identical = run_bundle(&root, "pliego.bundles.toml", "dist", false);
    assert_success(&identical);
    assert!(utf8(&identical.stdout).contains("0 changed"));
    assert_eq!(output_snapshot(&root.join("dist")), baseline);

    write_file(&plan, seed_bundle_plan(true));
    let reordered = run_bundle(&root, "pliego.bundles.toml", "dist", false);
    assert_success(&reordered);
    assert!(utf8(&reordered.stdout).contains("0 changed"));
    assert_eq!(output_snapshot(&root.join("dist")), baseline);

    let exact_check = run_bundle(&root, "pliego.bundles.toml", "dist", true);
    assert_success(&exact_check);
    assert!(utf8(&exact_check.stdout).contains("output(s) match"));
    assert_eq!(output_snapshot(&root.join("dist")), baseline);

    fs::write(root.join("dist/home.css"), b"drift").expect("CSS drift must be written");
    let drifted = output_snapshot(&root.join("dist"));
    let failed_check = run_bundle(&root, "pliego.bundles.toml", "dist", true);
    assert_failure_contains(&failed_check, "bundle output drift detected in 1 artifact");
    assert!(utf8(&failed_check.stderr).contains("home.css"));
    assert_eq!(
        output_snapshot(&root.join("dist")),
        drifted,
        "bundle --check must not repair or mutate drift"
    );
}

fn verify_late_failure_publishes_nothing(base: &Path) {
    let root = base.join("late-failure");
    fs::create_dir_all(root.join("dist")).expect("late-failure output directory must be created");
    write_file(
        &root.join("src/alpha.rs"),
        r#"fn alpha() { let _ = pc!("flex"); }"#,
    );
    write_file(
        &root.join("src/omega.rs"),
        r#"fn omega() { let _ = pc!("flec"); }"#,
    );
    write_file(
        &root.join("pliego.bundles.toml"),
        r#"schema = 1
targets = "modern"
format = "minified"
[theme]
kind = "seed"
[bundles.alpha]
sources = ["src/alpha.rs"]
[bundles.omega]
sources = ["src/omega.rs"]
"#,
    );
    for (name, bytes) in [
        ("alpha.css", b"old-alpha-css".as_slice()),
        ("alpha.manifest.json", b"old-alpha-manifest".as_slice()),
        ("omega.css", b"old-omega-css".as_slice()),
        ("omega.manifest.json", b"old-omega-manifest".as_slice()),
    ] {
        fs::write(root.join("dist").join(name), bytes).expect("sentinel output must be written");
    }
    let before = output_snapshot(&root.join("dist"));
    let failed = run_bundle(&root, "pliego.bundles.toml", "dist", false);
    assert_failure_contains(&failed, "PCS001");
    assert_eq!(
        output_snapshot(&root.join("dist")),
        before,
        "a late compile failure must not publish an earlier valid bundle"
    );
}

fn verify_plan_relative_config_theme(base: &Path) {
    let root = base.join("config-theme");
    fs::create_dir_all(root.join("dist")).expect("config output directory must be created");
    write_file(
        &root.join("config/pliego.theme.toml"),
        r##"schema = 1
extends = "seed"
[tokens.color]
brand = "#123456"
"##,
    );
    write_file(
        &root.join("config/src/app.rs"),
        r#"fn app() { let _ = pc!("bg-brand"); }"#,
    );
    write_file(
        &root.join("config/pliego.bundles.toml"),
        r#"schema = 1
targets = "modern"
format = "minified"
[theme]
kind = "config"
path = "pliego.theme.toml"
[bundles.app]
sources = ["src/app.rs"]
emit-theme = true
"#,
    );

    let first = run_bundle(&root, "config/pliego.bundles.toml", "dist", false);
    assert_success(&first);
    let css = fs::read_to_string(root.join("dist/app.css")).expect("app CSS must be readable");
    assert!(css.contains("--color-brand:#123456"));
    assert!(css.contains("background-color:var(--color-brand)"));

    fs::write(
        root.join("config/pliego.theme.toml"),
        "schema = 2\nextends = \"seed\"\n",
    )
    .expect("configured theme must be invalidated");
    let before = output_snapshot(&root.join("dist"));
    let failed = run_bundle(&root, "config/pliego.bundles.toml", "dist", false);
    assert_failure_contains(&failed, "unsupported theme schema 2; expected 1");
    assert_eq!(output_snapshot(&root.join("dist")), before);
}

fn verify_plan_output_alias_is_rejected(base: &Path) {
    let root = base.join("alias");
    fs::create_dir_all(&root).expect("alias directory must be created");
    write_file(
        &root.join("src/global.rs"),
        r#"fn global() { let _ = pc!("flex"); }"#,
    );
    let plan = r#"schema = 1
targets = "modern"
format = "minified"
[theme]
kind = "seed"
[bundles.global]
sources = ["src/global.rs"]
"#;
    write_file(&root.join("global.css"), plan);
    let before = fs::read(root.join("global.css")).expect("aliased plan must be readable");

    let failed = run_bundle(&root, "global.css", ".", false);
    assert_failure_contains(&failed, "aliases input `bundle plan`");
    assert_eq!(
        fs::read(root.join("global.css")).expect("aliased plan must survive"),
        before
    );
    assert!(!root.join("global.manifest.json").exists());
}

fn run_bundle(root: &Path, plan: &str, output_dir: &str, check: bool) -> Output {
    let mut arguments = vec!["bundle", "--plan", plan, "--output-dir", output_dir];
    if check {
        arguments.push("--check");
    }
    Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("pliego-cssc bundle must execute")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "bundle command failed with {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

fn assert_failure_contains(output: &Output, expected: &str) {
    assert!(
        !output.status.success(),
        "bundle command unexpectedly succeeded"
    );
    assert!(
        output.stdout.is_empty(),
        "failed bundle command wrote stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "bundle stderr did not contain `{expected}`:\n{stderr}"
    );
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("test file parent must be created");
    }
    fs::write(path, contents).expect("test file must be written");
}

fn output_snapshot(output_dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(output_dir)
        .expect("bundle output directory must be readable")
        .map(|entry| {
            let entry = entry.expect("bundle output entry must be readable");
            let name = entry.file_name().to_string_lossy().into_owned();
            (
                name,
                fs::read(entry.path()).expect("bundle output must be readable"),
            )
        })
        .filter(|(name, _)| !name.ends_with(".pliego.lock"))
        .collect()
}

fn manifest(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("bundle manifest must be valid JSON")
}

fn utf8(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("command output and generated CSS must be UTF-8")
}

fn seed_bundle_plan(reordered: bool) -> &'static str {
    if reordered {
        r#"schema = 1
targets = "modern"
format = "minified"

[theme]
kind = "seed"

[bundles.visit]
sources = ["src/shared.rs", "src/visit.rs"]
emit-theme = false

[bundles.home]
sources = ["src/shared.rs", "src/home.rs"]
emit-theme = false

[bundles.global]
sources = ["src/shared.rs", "src/global.rs"]
emit-theme = true
"#
    } else {
        r#"schema = 1
targets = "modern"
format = "minified"

[theme]
kind = "seed"

[bundles.global]
sources = ["src/global.rs", "src/shared.rs"]
emit-theme = true

[bundles.home]
sources = ["src/home.rs", "src/shared.rs"]
emit-theme = false

[bundles.visit]
sources = ["src/visit.rs", "src/shared.rs"]
emit-theme = false
"#
    }
}
