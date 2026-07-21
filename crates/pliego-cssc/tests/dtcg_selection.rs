//! End-to-end contract for selecting DTCG Resolver 2025.10 inputs during direct compile.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const RESOLVER: &str = include_str!("../../../examples/product.resolver.json");

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TemporaryDirectory(PathBuf);

struct WatchProcess(Option<Child>);

struct WatchPublication {
    manifest: pliego_css_control::ControlManifest,
    receipt: Vec<u8>,
}

impl TemporaryDirectory {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must follow Unix epoch")
            .as_nanos();
        for _ in 0..32 {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::current_dir()
                .expect("current directory")
                .join("target/pliego-cssc-dtcg-selection")
                .join(format!("{}-{timestamp}-{sequence}", std::process::id()));
            match fs::create_dir_all(&path) {
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

impl WatchProcess {
    fn stop(mut self) {
        let mut child = self.0.take().expect("watch process must exist");
        child.kill().expect("watch process must stop");
        child.wait().expect("watch process must be reaped");
    }
}

impl Drop for WatchProcess {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn defaults_explicit_contexts_and_casefolds_publish_one_exact_resolver_graph() {
    let directory = TemporaryDirectory::new();
    let root = directory.path();
    fs::create_dir(root.join("dist")).expect("output directory");
    fs::write(root.join("tokens.json"), RESOLVER).expect("resolver fixture");
    let resolver = pliego_css_config::parse_dtcg_resolver_str(RESOLVER)
        .expect("inline Resolver 2025.10 document must be valid");
    let expected_graph = resolver
        .graph()
        .to_canonical_json()
        .expect("resolver graph must be canonical");

    assert_success(&run_compile(root, "dist", &[]));
    let default = read_control(root, "dist");
    let default_css = fs::read(root.join("dist/app.css")).expect("default CSS");
    let default_snapshot = snapshot(&root.join("dist"));
    assert_exact_resolver_graph(root, "dist", &default, &expected_graph);

    assert_success(&run_compile(
        root,
        "dist",
        &["--token-input", "appearance=light"],
    ));
    let explicit_light = read_control(root, "dist");
    assert_eq!(
        snapshot(&root.join("dist")),
        default_snapshot,
        "an omitted default and its explicit canonical selection must converge"
    );
    assert_eq!(
        explicit_light.inputs.config_hash,
        default.inputs.config_hash
    );

    assert_success(&run_compile(
        root,
        "dist",
        &["--token-input", "appearance=dark"],
    ));
    let dark = read_control(root, "dist");
    let dark_css = fs::read(root.join("dist/app.css")).expect("dark CSS");
    let dark_snapshot = snapshot(&root.join("dist"));
    assert_ne!(
        dark_css, default_css,
        "light and dark must lower differently"
    );
    assert_ne!(dark.inputs.config_hash, default.inputs.config_hash);
    assert_exact_resolver_graph(root, "dist", &dark, &expected_graph);

    assert_success(&run_compile(
        root,
        "dist",
        &["--token-input", "APPEARANCE=DaRk"],
    ));
    assert_eq!(
        snapshot(&root.join("dist")),
        dark_snapshot,
        "case-equivalent modifier inputs must converge to exact artifact bytes"
    );

    assert_success(&run_compile(
        root,
        "dist",
        &[
            "--token-input",
            "appearance=dark",
            "--token-input",
            "channel=dark",
        ],
    ));
    let ordered_snapshot = snapshot(&root.join("dist"));
    assert_success(&run_compile(
        root,
        "dist",
        &[
            "--token-input",
            "channel=dark",
            "--token-input",
            "appearance=dark",
        ],
    ));
    assert_eq!(
        snapshot(&root.join("dist")),
        ordered_snapshot,
        "distinct modifier flags must converge independently of CLI order"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn watch_republishes_dtcg_defaults_and_keeps_the_last_valid_group() {
    let directory = TemporaryDirectory::new();
    let root = directory.path();
    let output = root.join("dist");
    let tokens = root.join("tokens.json");
    let stderr_log = root.join("watch.stderr.log");
    fs::create_dir(&output).expect("watch output directory");
    fs::write(root.join("styles.txt"), "flex bg-brand\n").expect("watch style input");
    fs::write(&tokens, RESOLVER).expect("initial resolver fixture");

    let initial_resolver = pliego_css_config::parse_dtcg_resolver_str(RESOLVER)
        .expect("initial resolver fixture must be valid");
    let initial_theme = initial_resolver
        .resolve_entries(std::iter::empty::<(String, String)>())
        .expect("omitted inputs must select the initial defaults");
    assert_eq!(
        initial_theme
            .selections()
            .get("appearance")
            .map(String::as_str),
        Some("light")
    );
    let initial_graph = initial_resolver
        .graph()
        .to_canonical_json()
        .expect("initial resolver graph must be canonical");

    let log = fs::File::create(&stderr_log).expect("watch stderr log");
    let child = Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .args([
            "watch",
            "--input",
            "styles.txt",
            "--output",
            "dist/app.css",
            "--manifest",
            "dist/app.manifest.json",
            "--control-dir",
            "dist",
            "--tokens",
            "tokens.json",
        ])
        .current_dir(root)
        .stdout(Stdio::null())
        .stderr(Stdio::from(log))
        .spawn()
        .expect("DTCG watch must start");
    let watch = WatchProcess(Some(child));

    let initial = wait_for("coherent initial DTCG watch group", || {
        read_committed_watch(root, &initial_graph)
    });
    let initial_snapshot = snapshot(&output);
    let initial_css = fs::read(output.join("app.css")).expect("initial watch CSS");
    wait_for("platform watch-scheduler startup", || {
        fs::read_to_string(&stderr_log).ok().and_then(|stderr| {
            #[cfg(any(target_os = "linux", windows))]
            let started = stderr.contains("filesystem events; snapshot fallback");
            #[cfg(not(any(target_os = "linux", windows)))]
            let started = stderr.contains(
                "polling every 100 ms; native filesystem events are not yet certified on this platform",
            );
            started.then_some(())
        })
    });

    let dark_source = RESOLVER.replacen("\"default\": \"light\"", "\"default\": \"dark\"", 1);
    assert_ne!(dark_source, RESOLVER, "fixture default must change");
    let dark_resolver = pliego_css_config::parse_dtcg_resolver_str(&dark_source)
        .expect("dark-default resolver revision must be valid");
    let dark_theme = dark_resolver
        .resolve_entries(std::iter::empty::<(String, String)>())
        .expect("omitted inputs must select the revised defaults");
    assert_eq!(
        dark_theme
            .selections()
            .get("appearance")
            .map(String::as_str),
        Some("dark")
    );
    let dark_graph = dark_resolver
        .graph()
        .to_canonical_json()
        .expect("dark resolver graph must be canonical");
    assert_ne!(
        dark_graph, initial_graph,
        "resolver graph must bind defaults"
    );

    let event_started = Instant::now();
    fs::write(&tokens, &dark_source).expect("publish dark resolver revision");
    let dark = wait_for("coherent dark-default DTCG watch group", || {
        let publication = read_committed_watch(root, &dark_graph)?;
        (publication.receipt != initial.receipt).then_some(publication)
    });
    let dark_snapshot = snapshot(&output);
    let dark_css = fs::read(output.join("app.css")).expect("dark watch CSS");
    assert!(
        event_started.elapsed() < Duration::from_millis(1_500),
        "the active scheduler must beat the 2 s authoritative fallback snapshot"
    );
    assert_ne!(
        dark_css, initial_css,
        "dark default must change generated CSS"
    );
    assert_ne!(
        dark.manifest.inputs.config_hash, initial.manifest.inputs.config_hash,
        "dark default must change configHash"
    );
    assert_ne!(
        dark_snapshot, initial_snapshot,
        "dark default must replace the complete published group"
    );

    fs::write(&tokens, "{}").expect("publish invalid resolver revision");
    wait_for("invalid resolver rejection", || {
        fs::read_to_string(&stderr_log).ok().and_then(|stderr| {
            stderr
                .contains("compile failed; keeping the last valid artifact")
                .then_some(())
        })
    });
    let retained = read_committed_watch(root, &dark_graph)
        .expect("invalid resolver revision must retain the coherent dark group");
    assert_eq!(retained.receipt, dark.receipt);
    assert_eq!(
        snapshot(&output),
        dark_snapshot,
        "invalid resolver revision must retain every byte of the last valid group"
    );

    fs::write(&tokens, RESOLVER).expect("restore valid resolver revision");
    let recovered = wait_for("byte-exact recovered DTCG watch group", || {
        let publication = read_committed_watch(root, &initial_graph)?;
        (publication.receipt == initial.receipt).then_some(publication)
    });
    watch.stop();
    assert_eq!(
        recovered.manifest.inputs.config_hash,
        initial.manifest.inputs.config_hash
    );
    assert_eq!(
        snapshot(&output),
        initial_snapshot,
        "restoring the resolver must recover the initial group byte for byte"
    );
}

#[test]
fn canonical_selection_changes_config_hash_even_when_theme_id_is_unchanged() {
    let resolver = pliego_css_config::parse_dtcg_resolver_str(RESOLVER)
        .expect("semantic selection fixture must be valid");
    let light = resolver
        .resolve_entries([("appearance", "light"), ("channel", "light")])
        .expect("light selection");
    let dark = resolver
        .resolve_entries([("appearance", "light"), ("channel", "dark")])
        .expect("dark selection");
    assert_eq!(
        light.registry().id(),
        dark.registry().id(),
        "the fixture must isolate selection identity from projected ThemeId"
    );
    assert_ne!(light.document(), dark.document());

    let directory = TemporaryDirectory::new();
    let root = directory.path();
    fs::create_dir(root.join("dist")).expect("output directory");
    fs::write(root.join("tokens.json"), RESOLVER).expect("resolver fixture");

    assert_success(&run_compile(
        root,
        "dist",
        &["--token-input", "channel=light"],
    ));
    let light_manifest = read_control(root, "dist");
    let light_css = fs::read(root.join("dist/app.css")).expect("light CSS");

    assert_success(&run_compile(
        root,
        "dist",
        &["--token-input", "channel=dark"],
    ));
    let dark_manifest = read_control(root, "dist");
    let dark_css = fs::read(root.join("dist/app.css")).expect("dark CSS");

    assert_eq!(
        light_css, dark_css,
        "unprojected semantic tokens must not change generated CSS"
    );
    assert_ne!(
        light_manifest.inputs.config_hash, dark_manifest.inputs.config_hash,
        "canonical resolver selections must participate in configHash independently of ThemeId"
    );
}

#[test]
fn duplicate_and_unknown_inputs_fail_without_publishing() {
    let directory = TemporaryDirectory::new();
    let root = directory.path();
    fs::write(root.join("tokens.json"), RESOLVER).expect("resolver fixture");

    let cases: &[(&str, &[&str], &str)] = &[
        (
            "exact-duplicate",
            &[
                "--token-input",
                "appearance=light",
                "--token-input",
                "appearance=dark",
            ],
            "modifier `appearance` is duplicated",
        ),
        (
            "casefold-duplicate",
            &[
                "--token-input",
                "appearance=light",
                "--token-input",
                "APPEARANCE=dark",
            ],
            "duplicated after case folding",
        ),
        (
            "unknown-modifier",
            &["--token-input", "motion=reduced"],
            "unknown modifier `motion`",
        ),
        (
            "unknown-context",
            &["--token-input", "appearance=sepia"],
            "invalid context `sepia` for modifier `appearance`",
        ),
    ];

    for (name, inputs, expected) in cases {
        let output_dir = format!("rejected/{name}");
        fs::create_dir_all(root.join(&output_dir)).expect("rejection output directory");
        let output = run_compile(root, &output_dir, inputs);
        assert_failure(&output, expected);
        assert!(
            snapshot(&root.join(&output_dir)).is_empty(),
            "invalid resolver input `{name}` published an artifact"
        );
    }
}

#[test]
fn invalid_theme_source_combinations_fail_without_publishing() {
    let directory = TemporaryDirectory::new();
    let root = directory.path();
    fs::write(root.join("tokens.json"), RESOLVER).expect("resolver fixture");
    fs::write(root.join("theme.toml"), "schema = 1\nextends = \"seed\"\n").expect("theme fixture");

    let cases: &[(&str, &[&str], &str)] = &[
        (
            "tokens-config",
            &["--tokens", "tokens.json", "--config", "theme.toml"],
            "`--tokens` conflicts",
        ),
        (
            "tokens-seed",
            &["--tokens", "tokens.json", "--seed"],
            "`--tokens` conflicts",
        ),
        (
            "input-without-tokens",
            &["--seed", "--token-input", "appearance=dark"],
            "`--token-input` requires `--tokens",
        ),
        (
            "duplicate-tokens",
            &["--tokens", "tokens.json", "--tokens", "tokens.json"],
            "`--tokens` may only be provided once",
        ),
        (
            "empty-modifier",
            &["--tokens", "tokens.json", "--token-input", "=dark"],
            "modifier=context",
        ),
        (
            "empty-context",
            &["--tokens", "tokens.json", "--token-input", "appearance="],
            "modifier=context",
        ),
        (
            "malformed-input",
            &["--tokens", "tokens.json", "--token-input", "appearance"],
            "modifier=context",
        ),
    ];

    for (name, theme_arguments, expected) in cases {
        let output_dir = format!("invalid/{name}");
        fs::create_dir_all(root.join(&output_dir)).expect("invalid output directory");
        let output = run_compile_with_theme(root, &output_dir, theme_arguments);
        assert_failure(&output, expected);
        assert!(
            snapshot(&root.join(&output_dir)).is_empty(),
            "invalid theme source `{name}` published an artifact"
        );
    }
}

fn run_compile(root: &Path, output_dir: &str, inputs: &[&str]) -> Output {
    let mut theme_arguments = vec!["--tokens", "tokens.json"];
    theme_arguments.extend_from_slice(inputs);
    run_compile_with_theme(root, output_dir, &theme_arguments)
}

fn run_compile_with_theme(root: &Path, output_dir: &str, theme_arguments: &[&str]) -> Output {
    let output = format!("{output_dir}/app.css");
    let manifest = format!("{output_dir}/app.manifest.json");
    let mut arguments = vec![
        "compile",
        "--style",
        "flex bg-brand",
        "--output",
        &output,
        "--manifest",
        &manifest,
        "--control-dir",
        output_dir,
    ];
    arguments.extend_from_slice(theme_arguments);
    Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("pliego-cssc must execute")
}

fn read_control(root: &Path, output_dir: &str) -> pliego_css_control::ControlManifest {
    let path = root.join(output_dir).join("pliego.css.manifest.json");
    pliego_css_control::parse_control_manifest(&fs::read(path).expect("control manifest"))
        .expect("control manifest must be canonical")
}

fn assert_exact_resolver_graph(
    root: &Path,
    output_dir: &str,
    manifest: &pliego_css_control::ControlManifest,
    expected: &[u8],
) {
    let resolver_bytes = fs::read(root.join("tokens.json")).expect("resolver input bytes");
    assert_eq!(resolver_bytes, RESOLVER.as_bytes());
    let resolver_input = manifest
        .inputs
        .files
        .iter()
        .find(|input| input.role == "token-resolver")
        .expect("control token-resolver input");
    assert_eq!(resolver_input.file, "tokens.json");
    assert_eq!(
        resolver_input.sha256,
        pliego_css_control::content_hash(&resolver_bytes)
    );

    let graph_bytes =
        fs::read(root.join(output_dir).join("pliego.tokens.json")).expect("published token graph");
    assert_eq!(graph_bytes, expected, "CLI must publish the resolver graph");
    let graph =
        pliego_css_config::parse_token_graph(&graph_bytes).expect("published canonical graph");
    let adapter = graph.adapter.as_ref().expect("DTCG resolver adapter");
    assert_eq!(adapter.name, "dtcg-resolver");
    assert_eq!(adapter.version, pliego_css_config::DTCG_RESOLVER_PROFILE);
    assert_eq!(graph.themes.len(), 4);

    let graph_output = manifest
        .outputs
        .iter()
        .find(|output| output.role == "token-graph")
        .expect("control token-graph output");
    assert_eq!(graph_output.artifact.file, "pliego.tokens.json");
    assert_eq!(
        graph_output.artifact.bytes,
        u64::try_from(expected.len()).expect("graph size must fit u64")
    );
    assert_eq!(
        graph_output.artifact.sha256,
        pliego_css_control::content_hash(expected)
    );
    assert_eq!(
        manifest.tokens.graph_hash.as_deref(),
        Some(graph_output.artifact.sha256.as_str())
    );
    assert!(
        manifest
            .receipt
            .required_checks
            .iter()
            .any(|check| check == "token-graph-integrity")
    );
    assert!(
        manifest.inputs.adapters.iter().any(|identity| {
            identity.name == adapter.name && identity.version == adapter.version
        })
    );
    assert_eq!(
        (manifest.tokens.aliases, manifest.tokens.deprecations),
        (1, 1)
    );

    let manifest_bytes = fs::read(
        root.join(output_dir)
            .join(pliego_css_control::CONTROL_MANIFEST_FILE),
    )
    .expect("control manifest bytes");
    assert_eq!(
        pliego_css_control::parse_control_manifest(&manifest_bytes)
            .expect("canonical control manifest"),
        *manifest
    );
    let receipt_bytes = fs::read(
        root.join(output_dir)
            .join(pliego_css_control::BUILD_RECEIPT_FILE),
    )
    .expect("build receipt bytes");
    let receipt = pliego_css_control::parse_build_receipt(&receipt_bytes, &manifest_bytes)
        .expect("manifest-bound build receipt");
    let integrity = receipt
        .checks
        .iter()
        .find(|check| check.id == "token-graph-integrity")
        .expect("token-graph integrity evidence");
    assert_eq!(integrity.status, pliego_css_control::CheckStatus::Passed);
    assert_eq!(integrity.evidence_kind, "integrity");
    assert!(integrity.required);
    assert_eq!(
        integrity.evidence_artifact.as_ref(),
        Some(&graph_output.artifact)
    );
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

fn wait_for<T>(label: &str, mut inspect: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(value) = inspect() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {label}");
        thread::sleep(Duration::from_millis(25));
    }
}

fn read_committed_watch(root: &Path, expected_graph: &[u8]) -> Option<WatchPublication> {
    let output = root.join("dist");
    publication_is_settled(&output)?;
    let graph = fs::read(output.join(pliego_css_control::TOKEN_GRAPH_FILE)).ok()?;
    if graph != expected_graph {
        return None;
    }

    let manifest_bytes = fs::read(output.join(pliego_css_control::CONTROL_MANIFEST_FILE)).ok()?;
    let manifest = pliego_css_control::parse_control_manifest(&manifest_bytes).ok()?;
    let graph_output = manifest
        .outputs
        .iter()
        .find(|candidate| candidate.role == "token-graph")?;
    let graph_hash = pliego_css_control::content_hash(&graph);
    if graph_output.artifact.file != pliego_css_control::TOKEN_GRAPH_FILE
        || graph_output.artifact.sha256 != graph_hash
        || graph_output.artifact.bytes != u64::try_from(graph.len()).ok()?
        || manifest.tokens.graph_hash.as_deref() != Some(graph_hash.as_str())
    {
        return None;
    }
    for generated in &manifest.outputs {
        let bytes = fs::read(output.join(&generated.artifact.file)).ok()?;
        if generated.artifact.bytes != u64::try_from(bytes.len()).ok()?
            || generated.artifact.sha256 != pliego_css_control::content_hash(&bytes)
        {
            return None;
        }
    }
    fs::read(output.join(pliego_css_control::projection::FINDINGS_FILE)).ok()?;

    let receipt = fs::read(output.join(pliego_css_control::BUILD_RECEIPT_FILE)).ok()?;
    pliego_css_control::parse_build_receipt(&receipt, &manifest_bytes).ok()?;
    publication_is_settled(&output)?;
    Some(WatchPublication { manifest, receipt })
}

fn publication_is_settled(directory: &Path) -> Option<()> {
    for entry in fs::read_dir(directory).ok()? {
        let name = entry.ok()?.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.')
            && name.contains(".pliego-")
            && (name.ends_with(".tmp") || name.ends_with(".bak"))
        {
            return None;
        }
    }
    Some(())
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
