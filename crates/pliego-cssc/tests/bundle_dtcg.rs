//! End-to-end contract for DTCG Resolver 2025.10 themes in bundle schema 2.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const RESOLVER: &str = include_str!("../../../examples/product.resolver.json");
const SOURCE: &str = r#"fn view() {
    let _ = pc!("flex bg-brand");
}
"#;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TemporaryDirectory(PathBuf);

struct Fixture {
    directory: TemporaryDirectory,
}

struct Publication {
    css: Vec<u8>,
    style_manifest: Value,
    control: pliego_css_control::ControlManifest,
}

impl TemporaryDirectory {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must follow Unix epoch")
            .as_nanos();
        for _ in 0..32 {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pliego-cssc-bundle-dtcg-{}-{timestamp}-{sequence}",
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

impl Fixture {
    fn new() -> Self {
        let directory = TemporaryDirectory::new();
        let root = directory.path();
        fs::create_dir(root.join("src")).expect("source directory");
        fs::write(root.join("src/styles.rs"), SOURCE).expect("bundle source");
        fs::write(root.join("product.resolver.json"), RESOLVER).expect("resolver fixture");

        let start = SOURCE
            .find(r#"pc!("flex bg-brand")"#)
            .expect("fixture invocation");
        let end = start + r#"pc!("flex bg-brand")"#.len();
        let reachability = json!({
            "schema": 1,
            "applicationCoverage": "complete",
            "components": [{
                "id": "view",
                "sites": [{
                    "file": "src/styles.rs",
                    "byteStart": start,
                    "byteEnd": end,
                }],
            }],
            "routes": [{
                "id": "home",
                "path": "/",
                "components": ["view"],
            }],
            "islands": [],
        });
        let mut bytes = serde_json::to_vec_pretty(&reachability).expect("reachability JSON");
        bytes.push(b'\n');
        fs::write(root.join("reachability.json"), bytes).expect("reachability fixture");
        Self { directory }
    }

    fn root(&self) -> &Path {
        self.directory.path()
    }

    fn write_plan(&self, name: &str, schema: u8, kind: &str, inputs: &[(&str, &str)]) {
        let path = self.root().join(name);
        let mut plan = format!(
            r#"schema = {schema}
targets = "modern"
format = "minified"

[theme]
kind = "{kind}"
path = "product.resolver.json"
"#,
        );
        if !inputs.is_empty() {
            plan.push_str("\n[theme.inputs]\n");
            for (modifier, context) in inputs {
                writeln!(plan, "{modifier} = \"{context}\"").expect("write theme input");
            }
        }
        plan.push_str(
            r#"
[bundles.application]
sources = ["src/styles.rs"]
emit-theme = true
"#,
        );
        fs::write(path, plan).expect("bundle plan");
    }

    fn create_output(&self, name: &str) {
        fs::create_dir(self.root().join(name)).expect("bundle output directory");
    }
}

#[test]
fn schema_two_defaults_and_contexts_publish_the_complete_resolver_graph() {
    let fixture = Fixture::new();
    let root = fixture.root();
    let resolver = pliego_css_config::parse_dtcg_resolver_str(RESOLVER)
        .expect("tracked Resolver 2025.10 example");
    let expected_graph = resolver
        .graph()
        .to_canonical_json()
        .expect("canonical resolver graph");

    fixture.write_plan("default.toml", 2, "dtcg-resolver", &[]);
    fixture.create_output("default");
    assert_success(&run_bundle(root, "default.toml", "default", false));
    let default = read_publication(root, "default");
    assert_dtcg_control(root, "default", &default.control, &expected_graph);

    fixture.write_plan(
        "explicit-light.toml",
        2,
        "dtcg-resolver",
        &[("appearance", "light")],
    );
    fixture.create_output("explicit-light");
    assert_success(&run_bundle(
        root,
        "explicit-light.toml",
        "explicit-light",
        false,
    ));
    let explicit_light = read_publication(root, "explicit-light");
    assert_eq!(explicit_light.css, default.css);
    assert_eq!(
        explicit_light.style_manifest["themeId"], default.style_manifest["themeId"],
        "an omitted default and its explicit context must resolve to one theme"
    );
    assert_dtcg_control(
        root,
        "explicit-light",
        &explicit_light.control,
        &expected_graph,
    );

    fixture.write_plan("dark.toml", 2, "dtcg-resolver", &[("appearance", "dark")]);
    fixture.create_output("dark");
    assert_success(&run_bundle(root, "dark.toml", "dark", false));
    let dark = read_publication(root, "dark");
    assert_ne!(dark.css, default.css, "dark must lower different CSS");
    assert_ne!(
        dark.style_manifest["themeId"], default.style_manifest["themeId"],
        "appearance must select a different projected theme"
    );
    assert_dtcg_control(root, "dark", &dark.control, &expected_graph);

    let light_channel = resolver
        .resolve_entries([("appearance", "light"), ("channel", "light")])
        .expect("light semantic channel");
    let dark_channel = resolver
        .resolve_entries([("appearance", "light"), ("channel", "dark")])
        .expect("dark semantic channel");
    assert_eq!(
        light_channel.registry().id(),
        dark_channel.registry().id(),
        "fixture must isolate selection identity from ThemeId"
    );

    fixture.write_plan("channel.toml", 2, "dtcg-resolver", &[("channel", "light")]);
    fixture.create_output("channel");
    assert_success(&run_bundle(root, "channel.toml", "channel", false));
    let channel_light = read_publication(root, "channel");
    let channel_light_graph =
        fs::read(root.join("channel/pliego.tokens.json")).expect("light channel token graph");

    fixture.write_plan("channel.toml", 2, "dtcg-resolver", &[("channel", "dark")]);
    assert_success(&run_bundle(root, "channel.toml", "channel", false));
    let channel_dark = read_publication(root, "channel");
    let channel_dark_graph =
        fs::read(root.join("channel/pliego.tokens.json")).expect("dark channel token graph");
    assert_eq!(channel_light.css, channel_dark.css);
    assert_eq!(
        channel_light.style_manifest, channel_dark.style_manifest,
        "semantic-only selection changed the style artifact despite equal ThemeId"
    );
    assert_eq!(channel_light_graph, channel_dark_graph);
    assert_ne!(
        channel_light.control.inputs.config_hash, channel_dark.control.inputs.config_hash,
        "canonical selections must remain config identity when ThemeId and CSS coincide"
    );
}

#[test]
fn bundle_check_is_read_only_for_matching_and_drifted_dtcg_groups() {
    let fixture = Fixture::new();
    fixture.write_plan("bundle.toml", 2, "dtcg-resolver", &[("appearance", "dark")]);
    fixture.create_output("dist");
    assert_success(&run_bundle(fixture.root(), "bundle.toml", "dist", false));

    let published = snapshot(&fixture.root().join("dist"));
    assert_success(&run_bundle(fixture.root(), "bundle.toml", "dist", true));
    assert_eq!(
        snapshot(&fixture.root().join("dist")),
        published,
        "successful DTCG bundle --check mutated the group"
    );

    fs::write(fixture.root().join("dist/application.css"), "drift\n").expect("introduce CSS drift");
    let drifted = snapshot(&fixture.root().join("dist"));
    let output = run_bundle(fixture.root(), "bundle.toml", "dist", true);
    assert_failure(&output, "bundle output drift detected");
    assert_eq!(
        snapshot(&fixture.root().join("dist")),
        drifted,
        "failing DTCG bundle --check repaired or mutated drift"
    );
}

#[test]
fn invalid_schema_selection_and_resolver_never_publish_or_replace_a_valid_group() {
    let fixture = Fixture::new();
    let root = fixture.root();

    for (name, schema, inputs, expected) in [
        ("schema-one", 1, Vec::<(&str, &str)>::new(), "schema 1"),
        (
            "unknown-context",
            2,
            vec![("appearance", "sepia")],
            "invalid context `sepia`",
        ),
        (
            "unknown-modifier",
            2,
            vec![("motion", "reduced")],
            "unknown modifier `motion`",
        ),
        (
            "casefold-duplicate",
            2,
            vec![("appearance", "light"), ("Appearance", "dark")],
            "duplicated after case folding",
        ),
    ] {
        let plan = format!("{name}.toml");
        fixture.write_plan(&plan, schema, "dtcg-resolver", &inputs);
        fixture.create_output(name);
        let output = run_bundle(root, &plan, name, false);
        assert_failure(&output, expected);
        assert!(
            snapshot(&root.join(name)).is_empty(),
            "invalid case `{name}` published an artifact"
        );
    }

    fixture.write_plan("stable.toml", 2, "dtcg-resolver", &[("appearance", "dark")]);
    fixture.create_output("stable");
    assert_success(&run_bundle(root, "stable.toml", "stable", false));
    let stable = snapshot(&root.join("stable"));

    fixture.write_plan(
        "stable.toml",
        2,
        "dtcg-resolver",
        &[("appearance", "sepia")],
    );
    assert_failure(
        &run_bundle(root, "stable.toml", "stable", false),
        "invalid context `sepia`",
    );
    assert_eq!(snapshot(&root.join("stable")), stable);

    fixture.write_plan("stable.toml", 2, "dtcg-resolver", &[("appearance", "dark")]);
    fs::write(root.join("product.resolver.json"), "{}").expect("publish invalid resolver fixture");
    assert_failure(&run_bundle(root, "stable.toml", "stable", false), "DTCG");
    assert_eq!(snapshot(&root.join("stable")), stable);

    fs::write(root.join("product.resolver.json"), RESOLVER).expect("restore resolver fixture");
    fixture.write_plan("stable.toml", 1, "dtcg-resolver", &[]);
    assert_failure(
        &run_bundle(root, "stable.toml", "stable", false),
        "schema 1",
    );
    assert_eq!(
        snapshot(&root.join("stable")),
        stable,
        "invalid revisions replaced bytes from the last valid bundle group"
    );
}

fn run_bundle(root: &Path, plan: &str, output_dir: &str, check: bool) -> Output {
    let mut arguments = vec![
        "bundle",
        "--plan",
        plan,
        "--output-dir",
        output_dir,
        "--manifest-version",
        "4",
        "--reachability",
        "reachability.json",
        "--asset-plan",
        "--control",
    ];
    if check {
        arguments.push("--check");
    }
    Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("pliego-cssc must execute")
}

fn read_publication(root: &Path, output_dir: &str) -> Publication {
    let output = root.join(output_dir);
    let css = fs::read(output.join("application.css")).expect("bundle CSS");
    let style_manifest = serde_json::from_slice(
        &fs::read(output.join("application.manifest.json")).expect("style manifest"),
    )
    .expect("canonical style manifest");
    let control_bytes =
        fs::read(output.join(pliego_css_control::CONTROL_MANIFEST_FILE)).expect("control manifest");
    let control = pliego_css_control::parse_control_manifest(&control_bytes)
        .expect("canonical control manifest");
    let receipt =
        fs::read(output.join(pliego_css_control::BUILD_RECEIPT_FILE)).expect("control receipt");
    pliego_css_control::parse_build_receipt(&receipt, &control_bytes)
        .expect("manifest-bound control receipt");
    Publication {
        css,
        style_manifest,
        control,
    }
}

fn assert_dtcg_control(
    root: &Path,
    output_dir: &str,
    control: &pliego_css_control::ControlManifest,
    expected_graph: &[u8],
) {
    let resolver_bytes = fs::read(root.join("product.resolver.json")).expect("resolver bytes");
    let resolver_input = control
        .inputs
        .files
        .iter()
        .find(|input| input.role == "token-resolver")
        .expect("token-resolver ledger entry");
    assert_eq!(resolver_input.file, "product.resolver.json");
    assert_eq!(
        resolver_input.sha256,
        pliego_css_control::content_hash(&resolver_bytes)
    );

    let graph_bytes = fs::read(
        root.join(output_dir)
            .join(pliego_css_control::TOKEN_GRAPH_FILE),
    )
    .expect("published token graph");
    assert_eq!(
        graph_bytes, expected_graph,
        "bundle truncated resolver graph"
    );
    let graph = pliego_css_config::parse_token_graph(&graph_bytes)
        .expect("published graph must be canonical");
    let adapter = graph.adapter.as_ref().expect("DTCG adapter identity");
    assert_eq!(adapter.name, "dtcg-resolver");
    assert_eq!(adapter.version, pliego_css_config::DTCG_RESOLVER_PROFILE);
    assert_eq!(
        graph.themes.len(),
        4,
        "bundle must publish every resolver branch"
    );
    assert_eq!(
        (control.tokens.aliases, control.tokens.deprecations),
        (1, 1)
    );

    let graph_output = control
        .outputs
        .iter()
        .find(|output| output.role == "token-graph")
        .expect("token-graph control output");
    assert_eq!(
        graph_output.artifact.file,
        pliego_css_control::TOKEN_GRAPH_FILE
    );
    assert_eq!(
        graph_output.artifact.sha256,
        pliego_css_control::content_hash(expected_graph)
    );
    assert_eq!(
        graph_output.artifact.bytes,
        u64::try_from(expected_graph.len()).expect("graph length must fit u64")
    );
    assert_eq!(
        control.tokens.graph_hash.as_deref(),
        Some(graph_output.artifact.sha256.as_str())
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
        "missing `{expected}` in stderr:\n{stderr}"
    );
}
