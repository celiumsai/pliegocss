//! End-to-end contract for opt-in pruning with fail-closed exact-origin matching.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pliego_css_usage::{
    UsageObservationCoverage, UsageObservationInput, UsageObservationScopeInput,
    UsageObservedStyleInput, UsageRetentionEntryInput, UsageRetentionInput,
    build_usage_observation, build_usage_retention,
};
use serde_json::{Value, json};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"fn route_view() {
    let _ = pc!("block text-accent");
}

fn island_view() {
    let _ = pc!("flex bg-muted");
}

fn dead_view() {
    let _ = pc!("hidden bg-accent-strong");
}

fn shared_live_view() {
    let _ = pc!("p-4");
}

fn shared_dead_view() {
    let _ = pc!("p-4");
}

fn coowned_view() {
    let _ = pc!("rounded-md");
}
"#;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Site {
    start: usize,
    end: usize,
}

#[derive(Debug)]
struct Sites {
    route: Site,
    island: Site,
    dead: Site,
    shared_live: Site,
    shared_dead: Site,
    coowned: Site,
}

struct TemporaryDirectory(PathBuf);

struct WatchProcess(Option<Child>);

impl TemporaryDirectory {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the system clock must be after the Unix epoch")
            .as_nanos();
        for _ in 0..32 {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pliego-cssc-reachability-pruning-{}-{timestamp}-{sequence}",
                std::process::id(),
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("cannot create `{}`: {error}", path.display()),
            }
        }
        panic!("cannot allocate a unique temporary directory");
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!(
                "warning: cannot remove temporary directory `{}`: {error}",
                self.0.display(),
            );
        }
    }
}

impl WatchProcess {
    fn stop(mut self) -> Output {
        let mut child = self.0.take().expect("watch process must exist");
        child.kill().expect("watch process must stop");
        child
            .wait_with_output()
            .expect("watch process output must be readable")
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

struct Fixture {
    directory: TemporaryDirectory,
    sites: Sites,
}

impl Fixture {
    fn new() -> Self {
        let directory = TemporaryDirectory::new();
        let root = directory.path();
        fs::create_dir_all(root.join("src")).expect("fixture source directory must be created");
        fs::create_dir(root.join("out")).expect("fixture output directory must be created");
        fs::write(root.join("src/styles.rs"), SOURCE).expect("fixture source must be written");

        let shared = all_sites(SOURCE, r#"pc!("p-4")"#);
        assert_eq!(shared.len(), 2, "the shared fixture needs two exact sites");
        let sites = Sites {
            route: unique_site(SOURCE, r#"pc!("block text-accent")"#),
            island: unique_site(SOURCE, r#"pc!("flex bg-muted")"#),
            dead: unique_site(SOURCE, r#"pc!("hidden bg-accent-strong")"#),
            shared_live: shared[0],
            shared_dead: shared[1],
            coowned: unique_site(SOURCE, r#"pc!("rounded-md")"#),
        };
        write_json(root.join("reachability.json"), &reachability(&sites));
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
        .expect("bundle plan must be written");
        Self { directory, sites }
    }

    fn root(&self) -> &Path {
        self.directory.path()
    }
}

struct Artifact {
    css: Vec<u8>,
    manifest_bytes: Vec<u8>,
    manifest: Value,
}

struct Evidence {
    css: Vec<u8>,
    manifest_bytes: Vec<u8>,
    route_style: String,
    island_style: String,
    dead_style: String,
    shared_style: String,
    coowned_style: String,
}

#[test]
fn reachability_pruning_is_fail_closed_deterministic_and_shared_by_artifact_commands() {
    let fixture = Fixture::new();
    let evidence = assert_primary_pruning(&fixture);
    assert_schema_five_closure(&fixture, &evidence);
    assert_sidecar_changes(&fixture, &evidence);
    assert_omitted_origin_is_fail_closed(&fixture);
    assert_empty_root_contract(&fixture);
    assert_cli_guards(&fixture);
    assert_watch_contract(&fixture, &evidence);
    assert_bundle_contract(&fixture, &evidence);
}

fn assert_primary_pruning(fixture: &Fixture) -> Evidence {
    let baseline = compile_success(
        fixture.root(),
        "baseline",
        "reachability.json",
        4,
        false,
        false,
    );
    let pruned = compile_success(
        fixture.root(),
        "pruned-v4",
        "reachability.json",
        4,
        true,
        false,
    );
    let route_style = style_id(style_for_site(&baseline.manifest, fixture.sites.route));
    let island_style = style_id(style_for_site(&baseline.manifest, fixture.sites.island));
    let dead_style = style_id(style_for_site(&baseline.manifest, fixture.sites.dead));
    let shared_style = style_id(style_for_site(
        &baseline.manifest,
        fixture.sites.shared_live,
    ));
    let coowned_style = style_id(style_for_site(&baseline.manifest, fixture.sites.coowned));

    let baseline_ids = manifest_style_ids(&baseline.manifest);
    let pruned_ids = manifest_style_ids(&pruned.manifest);
    assert!(baseline_ids.contains(&dead_style));
    assert!(
        pruned_ids.contains(&route_style),
        "route-root style was pruned"
    );
    assert!(
        pruned_ids.contains(&island_style),
        "island-root style was pruned"
    );
    assert!(
        pruned_ids.contains(&shared_style),
        "a shared identity with one reachable origin was pruned"
    );
    assert!(
        pruned_ids.contains(&coowned_style),
        "a co-owned site with one reachable owner was pruned"
    );
    assert!(
        !pruned_ids.contains(&dead_style),
        "the dead style survived pruning"
    );

    let shared = style_by_id(&pruned.manifest, &shared_style);
    let shared_origins = origin_sites(shared);
    assert!(shared_origins.contains(&fixture.sites.shared_live));
    assert!(shared_origins.contains(&fixture.sites.shared_dead));
    assert_eq!(
        shared_origins.len(),
        2,
        "deduplication lost a reachable or unreachable shared origin"
    );

    let css = css_text(&pruned.css);
    let dead_class = class_name(style_by_id(&baseline.manifest, &dead_style));
    assert!(
        !css.contains(&format!(".{dead_class}")),
        "dead class remained in the CSS artifact"
    );
    assert!(
        token_names(&baseline.manifest).contains("accent-strong"),
        "baseline fixture did not exercise the dead token"
    );
    assert!(
        !token_names(&pruned.manifest).contains("accent-strong"),
        "dead token remained in the semantic graph"
    );
    assert_coownership_edges(&pruned.manifest, &coowned_style);
    assert_root_edges(&pruned.manifest);

    Evidence {
        css: pruned.css,
        manifest_bytes: pruned.manifest_bytes,
        route_style,
        island_style,
        dead_style,
        shared_style,
        coowned_style,
    }
}

fn assert_schema_five_closure(fixture: &Fixture, evidence: &Evidence) {
    let artifact = compile_success(
        fixture.root(),
        "pruned-v5",
        "reachability.json",
        5,
        true,
        false,
    );
    assert_eq!(artifact.css, evidence.css, "schema 5 changed pruned CSS");
    assert_eq!(artifact.manifest["schemaVersion"], 5);
    let graph = &artifact.manifest["graph"];
    assert_eq!(graph["schemaVersion"], 2);
    assert_eq!(graph["physicalCoverage"], "compiler-verified-complete");

    let declarations = ids(graph, "declarations");
    let physical_declarations = ids(graph, "physicalDeclarations");
    let physical_rules = ids(graph, "physicalRules");
    let edges = graph["edges"]
        .as_array()
        .expect("graph edges must be an array");
    for declaration in &declarations {
        assert!(
            edges.iter().any(|edge| {
                edge["kind"] == "declarationContributesToPhysicalDeclaration"
                    && edge["from"] == declaration.as_str()
                    && physical_declarations.contains(value_string(edge, "to"))
            }),
            "semantic declaration `{declaration}` has no physical contribution"
        );
    }
    for declaration in &physical_declarations {
        assert!(
            edges.iter().any(|edge| {
                edge["kind"] == "physicalDeclarationBelongsToRule"
                    && edge["from"] == declaration.as_str()
                    && physical_rules.contains(value_string(edge, "to"))
            }),
            "physical declaration `{declaration}` belongs to no physical rule"
        );
    }
    assert_eq!(
        artifact.manifest["cssBytes"].as_u64(),
        Some(u64::try_from(artifact.css.len()).expect("CSS length must fit in u64")),
    );
}

fn assert_sidecar_changes(fixture: &Fixture, evidence: &Evidence) {
    let mut reordered = reachability(&fixture.sites);
    reverse_array(&mut reordered, "components");
    reverse_array(&mut reordered, "routes");
    reverse_array(&mut reordered, "islands");
    for component in array_mut(&mut reordered, "components") {
        reverse_array(component, "sites");
    }
    for route in array_mut(&mut reordered, "routes") {
        reverse_array(route, "components");
    }
    for island in array_mut(&mut reordered, "islands") {
        reverse_array(island, "components");
    }
    write_json(fixture.root().join("reordered.json"), &reordered);
    let reordered = compile_success(
        fixture.root(),
        "reordered",
        "reordered.json",
        4,
        true,
        false,
    );
    assert_eq!(reordered.css, evidence.css);
    assert_eq!(reordered.manifest_bytes, evidence.manifest_bytes);

    let mut renamed = reachability(&fixture.sites);
    object_by_id_mut(&mut renamed, "routes", "home")["path"] = json!("/renamed");
    write_json(fixture.root().join("renamed-route.json"), &renamed);
    let renamed = compile_success(
        fixture.root(),
        "renamed-route",
        "renamed-route.json",
        4,
        true,
        false,
    );
    assert_eq!(renamed.css, evidence.css, "route.path changed CSS bytes");
    assert_ne!(
        renamed.manifest_bytes, evidence.manifest_bytes,
        "route.path did not change graph bytes"
    );

    let mut expanded = reachability(&fixture.sites);
    array_mut(
        object_by_id_mut(&mut expanded, "routes", "home"),
        "components",
    )
    .push(json!("dead"));
    write_json(fixture.root().join("expanded-membership.json"), &expanded);
    let expanded = compile_success(
        fixture.root(),
        "expanded-membership",
        "expanded-membership.json",
        4,
        true,
        false,
    );
    assert_ne!(
        expanded.css, evidence.css,
        "route membership did not change CSS"
    );
    assert!(
        manifest_style_ids(&expanded.manifest).contains(&evidence.dead_style),
        "new route membership did not make the dead style reachable"
    );
}

fn assert_omitted_origin_is_fail_closed(fixture: &Fixture) {
    let mut omitted = reachability(&fixture.sites);
    object_by_id_mut(&mut omitted, "components", "dead")["sites"] = json!([]);
    write_json(fixture.root().join("omitted-origin.json"), &omitted);
    fs::write(fixture.root().join("out/protected.css"), "protected css\n")
        .expect("protected CSS must be written");
    fs::write(
        fixture.root().join("out/protected.manifest.json"),
        "protected manifest\n",
    )
    .expect("protected manifest must be written");
    let before = directory_snapshot(&fixture.root().join("out"));
    let output = run(
        fixture.root(),
        &arguments(&[
            "compile",
            "--source",
            "src",
            "--seed",
            "--output",
            "out/protected.css",
            "--manifest",
            "out/protected.manifest.json",
            "--manifest-version",
            "4",
            "--reachability",
            "omitted-origin.json",
            "--prune-unreachable",
        ]),
    );
    assert_failure(&output, "origin has no component");
    assert_eq!(
        directory_snapshot(&fixture.root().join("out")),
        before,
        "a fail-closed pruning error mutated outputs"
    );
}

fn assert_empty_root_contract(fixture: &Fixture) {
    let mut empty = reachability(&fixture.sites);
    empty["routes"] = json!([]);
    empty["islands"] = json!([]);
    write_json(fixture.root().join("empty-roots.json"), &empty);

    let plain = compile_success(
        fixture.root(),
        "empty-plain",
        "empty-roots.json",
        4,
        true,
        false,
    );
    assert_eq!(plain.css, b"\n");
    assert!(manifest_styles(&plain.manifest).is_empty());
    assert!(graph_array(&plain.manifest, "declarations").is_empty());
    assert!(graph_array(&plain.manifest, "tokens").is_empty());

    let plain_v5 = compile_success(
        fixture.root(),
        "empty-plain-v5",
        "empty-roots.json",
        5,
        true,
        false,
    );
    assert_eq!(plain_v5.css, plain.css);
    assert!(manifest_styles(&plain_v5.manifest).is_empty());
    assert!(graph_array(&plain_v5.manifest, "physicalRules").is_empty());
    assert!(graph_array(&plain_v5.manifest, "physicalDeclarations").is_empty());

    let themed = compile_success(
        fixture.root(),
        "empty-themed",
        "empty-roots.json",
        4,
        true,
        true,
    );
    assert_ne!(themed.css, b"\n");
    assert!(css_text(&themed.css).contains(":root{"));
    assert!(manifest_styles(&themed.manifest).is_empty());
    assert!(graph_array(&themed.manifest, "declarations").is_empty());
    assert!(graph_array(&themed.manifest, "tokens").is_empty());

    let themed_v5 = compile_success(
        fixture.root(),
        "empty-themed-v5",
        "empty-roots.json",
        5,
        true,
        true,
    );
    assert_eq!(themed_v5.css, themed.css);
    assert!(manifest_styles(&themed_v5.manifest).is_empty());
    assert!(graph_array(&themed_v5.manifest, "declarations").is_empty());
    assert!(graph_array(&themed_v5.manifest, "tokens").is_empty());
    assert!(!graph_array(&themed_v5.manifest, "physicalRules").is_empty());
    assert!(!graph_array(&themed_v5.manifest, "physicalDeclarations").is_empty());
}

fn assert_cli_guards(fixture: &Fixture) {
    let missing = run(
        fixture.root(),
        &arguments(&[
            "compile",
            "--source",
            "src",
            "--seed",
            "--output",
            "out/invalid-missing.css",
            "--prune-unreachable",
        ]),
    );
    assert_failure(&missing, "`--prune-unreachable` requires `--reachability`");
    assert!(!fixture.root().join("out/invalid-missing.css").exists());

    let repeated = run(
        fixture.root(),
        &arguments(&[
            "compile",
            "--source",
            "src",
            "--seed",
            "--output",
            "out/invalid-repeated.css",
            "--manifest",
            "out/invalid-repeated.manifest.json",
            "--manifest-version",
            "4",
            "--reachability",
            "reachability.json",
            "--prune-unreachable",
            "--prune-unreachable",
        ]),
    );
    assert_failure(&repeated, "`--prune-unreachable` may only be provided once");
    assert!(!fixture.root().join("out/invalid-repeated.css").exists());
}

#[allow(clippy::too_many_lines)]
fn assert_watch_contract(fixture: &Fixture, evidence: &Evidence) {
    let sidecar = fixture.root().join("watch-reachability.json");
    write_json(&sidecar, &reachability(&fixture.sites));
    let child = Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .args([
            "watch",
            "--source",
            "src",
            "--seed",
            "--output",
            "out/watch.css",
            "--manifest",
            "out/watch.manifest.json",
            "--control-dir",
            "out",
            "--manifest-version",
            "4",
            "--reachability",
            "watch-reachability.json",
            "--prune-unreachable",
        ])
        .current_dir(fixture.root())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("watch must start");
    let watch = WatchProcess(Some(child));
    wait_for("initial watch publication", || {
        fs::read(fixture.root().join("out/watch.css")).is_ok_and(|css| css == evidence.css)
            && watch_publication_is_readable(fixture.root())
    });
    let initial_manifest = fs::read(fixture.root().join("out/watch.manifest.json"))
        .expect("initial watch manifest must exist");
    let initial_receipt = fs::read(fixture.root().join("out/pliego.css.receipt.json"))
        .expect("initial watch receipt must exist");

    let mut renamed = reachability(&fixture.sites);
    object_by_id_mut(&mut renamed, "routes", "home")["path"] = json!("/watch-renamed");
    write_json(&sidecar, &renamed);
    wait_for("topology-only watch publication", || {
        fs::read(fixture.root().join("out/watch.manifest.json"))
            .is_ok_and(|manifest| manifest != initial_manifest)
            && watch_publication_is_readable(fixture.root())
    });
    assert_ne!(
        fs::read(fixture.root().join("out/pliego.css.receipt.json"))
            .expect("topology receipt must exist"),
        initial_receipt,
        "exact reachability change did not update the watch receipt",
    );
    assert_eq!(
        fs::read(fixture.root().join("out/watch.css")).expect("watch CSS must remain readable"),
        evidence.css,
        "a route path-only edit changed watch CSS",
    );

    let mut expanded = renamed;
    array_mut(
        object_by_id_mut(&mut expanded, "routes", "home"),
        "components",
    )
    .push(json!("dead"));
    write_json(&sidecar, &expanded);
    wait_for("membership watch publication", || {
        fs::read(fixture.root().join("out/watch.css")).is_ok_and(|css| css != evidence.css)
            && watch_publication_is_readable(fixture.root())
    });
    let expanded_css = fs::read(fixture.root().join("out/watch.css"))
        .expect("expanded watch CSS must be readable");
    let expanded_manifest = fs::read(fixture.root().join("out/watch.manifest.json"))
        .expect("expanded watch manifest must be readable");
    let expanded_receipt = fs::read(fixture.root().join("out/pliego.css.receipt.json"))
        .expect("expanded watch receipt must be readable");

    let mut invalid = expanded.clone();
    object_by_id_mut(&mut invalid, "components", "dead")["sites"] = json!([]);
    write_json(&sidecar, &invalid);
    thread::sleep(Duration::from_millis(750));
    assert_eq!(
        fs::read(fixture.root().join("out/watch.css")).expect("last valid watch CSS must survive"),
        expanded_css,
    );
    assert_eq!(
        fs::read(fixture.root().join("out/watch.manifest.json"))
            .expect("last valid watch manifest must survive"),
        expanded_manifest,
    );
    assert_eq!(
        fs::read(fixture.root().join("out/pliego.css.receipt.json"))
            .expect("last valid watch receipt must survive"),
        expanded_receipt,
    );

    reverse_array(&mut expanded, "components");
    reverse_array(&mut expanded, "routes");
    reverse_array(&mut expanded, "islands");
    for component in array_mut(&mut expanded, "components") {
        reverse_array(component, "sites");
    }
    write_json(&sidecar, &expanded);
    wait_for("canonical topology receipt publication", || {
        fs::read(fixture.root().join("out/pliego.css.receipt.json"))
            .is_ok_and(|receipt| receipt != expanded_receipt)
            && watch_publication_is_readable(fixture.root())
    });
    assert_eq!(
        fs::read(fixture.root().join("out/watch.css")).expect("canonical watch CSS must survive"),
        expanded_css,
    );
    assert_eq!(
        fs::read(fixture.root().join("out/watch.manifest.json"))
            .expect("canonical watch manifest must survive"),
        expanded_manifest,
    );
    let control_manifest_bytes = fs::read(fixture.root().join("out/pliego.css.manifest.json"))
        .expect("watch control manifest must exist");
    let control_manifest = pliego_css_control::parse_control_manifest(&control_manifest_bytes)
        .expect("watch control manifest must remain canonical");
    let receipt_bytes = fs::read(fixture.root().join("out/pliego.css.receipt.json"))
        .expect("watch receipt must exist");
    let receipt = pliego_css_control::parse_build_receipt(&receipt_bytes, &control_manifest_bytes)
        .expect("watch receipt must bind exact control manifest");
    assert_eq!(receipt.result, pliego_css_control::ReceiptResult::Passed);
    assert!(
        control_manifest.inputs.files.iter().any(|input| {
            input.file == "watch-reachability.json" && input.role == "reachability"
        })
    );

    let output = watch.stop();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("compile failed; keeping the last valid artifact: origin has no component"),
        "watch did not report the fail-closed origin error:\n{stderr}",
    );
    assert!(
        stderr.contains("compiled `src` -> `out/watch.css`"),
        "watch did not publish the exact reordered snapshot receipt:\n{stderr}",
    );
}

#[allow(clippy::too_many_lines)]
fn assert_bundle_contract(fixture: &Fixture, evidence: &Evidence) {
    fs::create_dir(fixture.root().join("bundle")).expect("bundle output directory must be created");
    let bundle_arguments = arguments(&[
        "bundle",
        "--plan",
        "pliego.bundles.toml",
        "--output-dir",
        "bundle",
        "--manifest-version",
        "5",
        "--reachability",
        "reachability.json",
        "--prune-unreachable",
        "--asset-plan",
        "--project-index",
        "--usage-report",
        "--control",
    ]);
    assert_success(&run(fixture.root(), &bundle_arguments));
    assert_eq!(
        fs::read(fixture.root().join("bundle/application.css")).expect("bundle CSS must exist"),
        evidence.css,
    );
    let manifest = read_json(fixture.root().join("bundle/application.manifest.json"));
    let ids = manifest_style_ids(&manifest);
    for expected in [
        &evidence.route_style,
        &evidence.island_style,
        &evidence.shared_style,
        &evidence.coowned_style,
    ] {
        assert!(
            ids.contains(expected),
            "bundle lost reachable style `{expected}`"
        );
    }
    assert!(!ids.contains(&evidence.dead_style));

    let control_manifest_bytes = fs::read(fixture.root().join("bundle/pliego.css.manifest.json"))
        .expect("control manifest must exist");
    let control_manifest = pliego_css_control::parse_control_manifest(&control_manifest_bytes)
        .expect("control manifest must be canonical");
    let receipt_bytes = fs::read(fixture.root().join("bundle/pliego.css.receipt.json"))
        .expect("control receipt must exist");
    let receipt = pliego_css_control::parse_build_receipt(&receipt_bytes, &control_manifest_bytes)
        .expect("control receipt must bind the exact manifest");
    let output_files = control_manifest
        .outputs
        .iter()
        .map(|output| output.artifact.file.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        output_files,
        BTreeSet::from([
            "application.css",
            "application.css.map",
            "application.manifest.json",
            "pliego.assets.json",
            "pliego.css.findings.json",
            "pliego.index.json",
            "pliego.usage.json",
            "pliego.tokens.json",
        ])
    );
    assert_eq!(receipt.outputs.len(), control_manifest.outputs.len());
    assert_eq!(receipt.result, pliego_css_control::ReceiptResult::Passed);
    let css_output = control_manifest
        .outputs
        .iter()
        .find(|output| output.artifact.file == "application.css")
        .expect("control manifest must describe generated CSS");
    assert_eq!(
        css_output.relationships,
        [
            "application.css.map",
            "application.manifest.json",
            "pliego.tokens.json",
        ]
    );
    let source_map = css_output
        .source_map
        .as_ref()
        .expect("generated CSS must reference its source map");
    let source_map_output = control_manifest
        .outputs
        .iter()
        .find(|output| output.artifact.file == "application.css.map")
        .expect("control manifest must describe the CSS source map");
    assert_eq!(source_map, &source_map_output.artifact);
    assert_eq!(source_map_output.relationships, ["application.css"]);
    let asset_plan_output = control_manifest
        .outputs
        .iter()
        .find(|output| output.artifact.file == "pliego.assets.json")
        .expect("control manifest must describe the Asset Plan");
    assert_eq!(
        asset_plan_output.relationships,
        ["application.css", "application.manifest.json"]
    );
    let project_index_output = control_manifest
        .outputs
        .iter()
        .find(|output| output.artifact.file == "pliego.index.json")
        .expect("control manifest must describe the Project Index");
    assert_eq!(project_index_output.relationships, ["pliego.assets.json"]);
    let usage_output = control_manifest
        .outputs
        .iter()
        .find(|output| output.artifact.file == "pliego.usage.json")
        .expect("control manifest must describe the usage analysis");
    assert_eq!(
        usage_output.relationships,
        [
            "application.css",
            "application.manifest.json",
            "pliego.assets.json",
            "pliego.index.json",
        ]
    );
    let usage = read_json(fixture.root().join("bundle/pliego.usage.json"));
    assert_eq!(usage["schemaVersion"], 1);
    assert_eq!(usage["stateModelVersion"], 1);
    assert_eq!(usage["analysisUnit"], "bundle-style-id");
    assert_eq!(usage["ruleSelection"], "reachable-style-ids");
    assert_eq!(usage["summary"]["usageDead"], 1);
    assert_eq!(usage["summary"]["removalRemoved"], 1);
    let usage_styles = usage["styles"]
        .as_array()
        .expect("usage styles must be an array");
    let dead = usage_styles
        .iter()
        .find(|style| style["styleId"] == evidence.dead_style)
        .expect("pruned StyleId must remain in the usage universe");
    assert_eq!(dead["selected"], false);
    assert_eq!(dead["staticReachability"], "unreachable");
    assert_eq!(dead["observationState"], "unavailable");
    assert_eq!(dead["usageState"], "dead");
    assert_eq!(dead["removalDisposition"], "removed");
    let shared = usage_styles
        .iter()
        .find(|style| style["styleId"] == evidence.shared_style)
        .expect("shared StyleId must remain in the usage universe");
    assert_eq!(shared["selected"], true);
    assert_eq!(shared["staticReachability"], "reachable");
    assert_eq!(shared["observationState"], "unavailable");
    assert_eq!(shared["usageState"], "unknown");
    pliego_css_usage::parse_usage_analysis(
        &fs::read(fixture.root().join("bundle/pliego.usage.json"))
            .expect("usage analysis must be readable"),
    )
    .expect("usage analysis must satisfy its closed contract");
    let token_graph_output = control_manifest
        .outputs
        .iter()
        .find(|output| output.artifact.file == "pliego.tokens.json")
        .expect("control manifest must describe the canonical token graph");
    assert_eq!(
        token_graph_output.relationships,
        ["application.css", "application.manifest.json"]
    );
    let token_graph_bytes = fs::read(fixture.root().join("bundle/pliego.tokens.json"))
        .expect("canonical token graph must exist");
    pliego_css_config::parse_token_graph(&token_graph_bytes)
        .expect("bundle token graph must be canonical");
    assert_eq!(
        control_manifest.tokens.graph_version.as_deref(),
        Some("pliegocss-token-graph/1")
    );
    assert_eq!(
        control_manifest.rules.observation,
        pliego_css_control::MeasurementState::Measured
    );
    for input in ["pliego.bundles.toml", "reachability.json", "src/styles.rs"] {
        assert!(
            control_manifest
                .inputs
                .files
                .iter()
                .any(|item| item.file == input),
            "control manifest omitted exact input `{input}`"
        );
    }

    let original_source_hash = control_manifest.inputs.source_hash.clone();
    let original_config_hash = control_manifest.inputs.config_hash.clone();
    let reachability_path = fixture.root().join("reachability.json");
    let mut reachability_bytes =
        fs::read(&reachability_path).expect("reachability must be readable");
    reachability_bytes.push(b'\n');
    fs::write(&reachability_path, reachability_bytes).expect("reachability whitespace must change");
    assert_success(&run(fixture.root(), &bundle_arguments));
    let changed_manifest = pliego_css_control::parse_control_manifest(
        &fs::read(fixture.root().join("bundle/pliego.css.manifest.json"))
            .expect("changed control manifest must exist"),
    )
    .expect("changed control manifest must remain canonical");
    assert_eq!(changed_manifest.inputs.source_hash, original_source_hash);
    assert_ne!(changed_manifest.inputs.config_hash, original_config_hash);

    let before = directory_snapshot(&fixture.root().join("bundle"));
    let mut check_arguments = bundle_arguments.clone();
    check_arguments.push("--check".into());
    assert_success(&run(fixture.root(), &check_arguments));
    assert_eq!(
        directory_snapshot(&fixture.root().join("bundle")),
        before,
        "successful bundle --check mutated outputs"
    );

    assert_multi_bundle_contract(fixture, evidence);

    fs::write(fixture.root().join("bundle/application.css"), "drift\n")
        .expect("bundle drift must be written");
    let drifted = directory_snapshot(&fixture.root().join("bundle"));
    let output = run(fixture.root(), &check_arguments);
    assert_failure(&output, "bundle output drift detected");
    assert_eq!(
        directory_snapshot(&fixture.root().join("bundle")),
        drifted,
        "failing bundle --check repaired or mutated drift"
    );
    assert_success(&run(fixture.root(), &bundle_arguments));
    fs::write(
        fixture.root().join("bundle/pliego.tokens.json"),
        "token graph drift\n",
    )
    .expect("token-graph drift must be written");
    let graph_drifted = directory_snapshot(&fixture.root().join("bundle"));
    let output = run(fixture.root(), &check_arguments);
    assert_failure(&output, "bundle output drift detected");
    assert_eq!(
        directory_snapshot(&fixture.root().join("bundle")),
        graph_drifted,
        "failing token-graph check repaired or mutated drift"
    );
    assert_success(&run(fixture.root(), &bundle_arguments));
    fs::write(
        fixture.root().join("bundle/pliego.css.receipt.json"),
        "receipt drift\n",
    )
    .expect("receipt drift must be written");
    let receipt_drifted = directory_snapshot(&fixture.root().join("bundle"));
    let output = run(fixture.root(), &check_arguments);
    assert_failure(&output, "bundle output drift detected");
    assert_eq!(
        directory_snapshot(&fixture.root().join("bundle")),
        receipt_drifted,
        "failing control receipt check repaired or mutated drift"
    );

    fs::create_dir(fixture.root().join("bundle-observed"))
        .expect("observed bundle output directory must be created");
    let reachability_bytes =
        fs::read(fixture.root().join("reachability.json")).expect("reachability must be readable");
    write_json(
        fixture.root().join("observations.json"),
        &json!({
            "schemaVersion": 1,
            "universeSha256": usage["universeSha256"],
            "reachabilitySha256": pliego_css_build::artifacts::sha256_hex(&reachability_bytes),
            "coverage": "sampled",
            "producer": {"name": "browser-fixture", "version": "1"},
            "contexts": {
                "routes": ["/"],
                "islands": [],
                "themes": ["seed"],
                "browsers": ["chromium"],
                "viewports": ["1280x720"],
                "states": ["default"]
            },
            "unknownDynamicInputs": ["authenticated-user"],
            "observedStyles": [{
                "bundleId": "application",
                "styleId": evidence.route_style,
            }]
        }),
    );
    let observed_arguments = arguments(&[
        "bundle",
        "--plan",
        "pliego.bundles.toml",
        "--output-dir",
        "bundle-observed",
        "--manifest-version",
        "5",
        "--reachability",
        "reachability.json",
        "--prune-unreachable",
        "--asset-plan",
        "--project-index",
        "--usage-report",
        "--observations",
        "observations.json",
        "--control",
    ]);
    assert_success(&run(fixture.root(), &observed_arguments));
    let observed_usage = read_json(fixture.root().join("bundle-observed/pliego.usage.json"));
    let styles = observed_usage["styles"].as_array().unwrap();
    let observed_route = styles
        .iter()
        .find(|style| style["styleId"] == evidence.route_style)
        .unwrap();
    assert_eq!(observed_route["observationState"], "observed");
    assert_eq!(observed_route["usageState"], "observed");
    let unobserved_shared = styles
        .iter()
        .find(|style| style["styleId"] == evidence.shared_style)
        .unwrap();
    assert_eq!(unobserved_shared["staticReachability"], "reachable");
    assert_eq!(unobserved_shared["observationState"], "unobserved");
    assert_eq!(unobserved_shared["usageState"], "unobserved");
    assert_eq!(unobserved_shared["removalDisposition"], "retain");
    let observed_control = pliego_css_control::parse_control_manifest(
        &fs::read(
            fixture
                .root()
                .join("bundle-observed/pliego.css.manifest.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        observed_control.inputs.files.iter().any(|input| {
            input.file == "observations.json" && input.role == "usage-observation"
        })
    );

    let before_contradiction = directory_snapshot(&fixture.root().join("bundle-observed"));
    let mut contradictory = read_json(fixture.root().join("observations.json"));
    contradictory["observedStyles"] = json!([{
        "bundleId": "application",
        "styleId": evidence.dead_style,
    }]);
    write_json(fixture.root().join("observations.json"), &contradictory);
    let failure = run(fixture.root(), &observed_arguments);
    assert_failure(&failure, "usage observation contradicts unreachable style");
    assert_eq!(
        directory_snapshot(&fixture.root().join("bundle-observed")),
        before_contradiction,
        "contradictory usage evidence published a partial output group"
    );
}

#[allow(clippy::too_many_lines)]
fn assert_multi_bundle_contract(fixture: &Fixture, evidence: &Evidence) {
    const DEAD_ONLY: &str = "fn dead_only() { let _ = pc!(\"grid bg-accent-strong\"); }\n";
    fs::write(fixture.root().join("src/dead-only.rs"), DEAD_ONLY)
        .expect("dead-only bundle source must be written");
    let dead_site = unique_site(DEAD_ONLY, r#"pc!("grid bg-accent-strong")"#);
    let mut sidecar = reachability(&fixture.sites);
    array_mut(&mut sidecar, "components").push(json!({
        "id": "dead-only",
        "sites": [{
            "file": "src/dead-only.rs",
            "byteStart": dead_site.start,
            "byteEnd": dead_site.end,
        }],
    }));
    write_json(fixture.root().join("multi-reachability.json"), &sidecar);
    fs::write(
        fixture.root().join("multi.bundles.toml"),
        r#"schema = 1
targets = "modern"
format = "minified"

[theme]
kind = "seed"

[bundles.application]
sources = ["src/styles.rs"]
emit-theme = false

[bundles.dead]
sources = ["src/dead-only.rs"]
emit-theme = false

[bundles.themed-dead]
sources = ["src/dead-only.rs"]
emit-theme = true
"#,
    )
    .expect("multi-bundle plan must be written");
    fs::create_dir(fixture.root().join("bundle-multi"))
        .expect("multi-bundle output directory must be created");
    let bundle_arguments = arguments(&[
        "bundle",
        "--plan",
        "multi.bundles.toml",
        "--output-dir",
        "bundle-multi",
        "--manifest-version",
        "4",
        "--reachability",
        "multi-reachability.json",
        "--prune-unreachable",
    ]);
    assert_success(&run(fixture.root(), &bundle_arguments));
    assert_eq!(
        fs::read(fixture.root().join("bundle-multi/application.css"))
            .expect("application bundle CSS must exist"),
        evidence.css,
    );
    assert_eq!(
        fs::read(fixture.root().join("bundle-multi/dead.css")).expect("dead bundle CSS must exist"),
        b"\n",
    );
    let dead_manifest = read_json(fixture.root().join("bundle-multi/dead.manifest.json"));
    assert!(manifest_styles(&dead_manifest).is_empty());
    let themed_css = fs::read(fixture.root().join("bundle-multi/themed-dead.css"))
        .expect("themed dead bundle CSS must exist");
    assert!(css_text(&themed_css).contains(":root{"));
    let themed_manifest = read_json(
        fixture
            .root()
            .join("bundle-multi/themed-dead.manifest.json"),
    );
    assert!(manifest_styles(&themed_manifest).is_empty());
    assert!(graph_array(&themed_manifest, "declarations").is_empty());
    assert!(graph_array(&themed_manifest, "tokens").is_empty());

    let before = directory_snapshot(&fixture.root().join("bundle-multi"));
    let mut check_arguments = bundle_arguments;
    check_arguments.push("--check".into());
    assert_success(&run(fixture.root(), &check_arguments));
    assert_eq!(
        directory_snapshot(&fixture.root().join("bundle-multi")),
        before,
        "multi-bundle --check mutated fully pruned outputs",
    );

    fs::create_dir(fixture.root().join("bundle-multi-report"))
        .expect("multi-bundle report directory must be created");
    let report_arguments = arguments(&[
        "bundle",
        "--plan",
        "multi.bundles.toml",
        "--output-dir",
        "bundle-multi-report",
        "--manifest-version",
        "5",
        "--reachability",
        "multi-reachability.json",
        "--prune-unreachable",
        "--usage-report",
    ]);
    assert_success(&run(fixture.root(), &report_arguments));
    let report = read_json(fixture.root().join("bundle-multi-report/pliego.usage.json"));
    let report_styles = report["styles"]
        .as_array()
        .expect("multi-bundle usage styles must be an array");
    let retained_style_id = report_styles
        .iter()
        .find(|style| style["bundleId"] == "dead")
        .and_then(|style| style["styleId"].as_str())
        .expect("dead bundle style must remain in the pre-pruning universe")
        .to_owned();
    assert!(report_styles.iter().any(|style| {
        style["bundleId"] == "themed-dead" && style["styleId"] == retained_style_id
    }));

    let reachability_bytes = fs::read(fixture.root().join("multi-reachability.json"))
        .expect("multi-bundle reachability must be readable");
    let retention_bytes = build_usage_retention(UsageRetentionInput::new(
        report["universeSha256"]
            .as_str()
            .expect("usage universe digest must be a string"),
        pliego_css_build::artifacts::sha256_hex(&reachability_bytes),
        vec![UsageRetentionEntryInput::new(
            "external-email-renderer",
            "dead",
            &retained_style_id,
            "The external email renderer is outside the application route graph.",
        )],
    ))
    .expect("exact retention policy must build");
    fs::write(
        fixture.root().join("multi-retention.json"),
        &retention_bytes,
    )
    .expect("retention sidecar must be written");
    fs::create_dir(fixture.root().join("bundle-multi-retained"))
        .expect("retained bundle directory must be created");
    let retained_arguments = arguments(&[
        "bundle",
        "--plan",
        "multi.bundles.toml",
        "--output-dir",
        "bundle-multi-retained",
        "--manifest-version",
        "5",
        "--reachability",
        "multi-reachability.json",
        "--prune-unreachable",
        "--asset-plan",
        "--project-index",
        "--usage-report",
        "--retention",
        "multi-retention.json",
        "--control",
    ]);
    assert_success(&run(fixture.root(), &retained_arguments));

    let retained_manifest = read_json(
        fixture
            .root()
            .join("bundle-multi-retained/dead.manifest.json"),
    );
    assert_eq!(
        manifest_style_ids(&retained_manifest),
        BTreeSet::from([retained_style_id.clone()]),
        "the policy-retained StyleId was not emitted as a whole style",
    );
    let retained_class = class_name(style_by_id(&retained_manifest, &retained_style_id));
    assert!(
        css_text(
            &fs::read(fixture.root().join("bundle-multi-retained/dead.css"))
                .expect("retained CSS must exist")
        )
        .contains(&format!(".{retained_class}")),
        "retained manifest and CSS selection drifted",
    );
    let unretained_manifest = read_json(
        fixture
            .root()
            .join("bundle-multi-retained/themed-dead.manifest.json"),
    );
    assert!(
        manifest_styles(&unretained_manifest).is_empty(),
        "bundle qualification leaked the retention grant into another bundle",
    );
    let retained_application = read_json(
        fixture
            .root()
            .join("bundle-multi-retained/application.manifest.json"),
    );
    assert!(
        !manifest_style_ids(&retained_application).contains(&evidence.dead_style),
        "an unrelated dead application style survived retained selection",
    );

    let retained_usage = read_json(
        fixture
            .root()
            .join("bundle-multi-retained/pliego.usage.json"),
    );
    assert_eq!(retained_usage["schemaVersion"], 2);
    assert_eq!(retained_usage["stateModelVersion"], 2);
    assert_eq!(
        retained_usage["ruleSelection"],
        "reachable-or-retained-style-ids"
    );
    assert_eq!(retained_usage["retention"]["state"], "bound");
    assert_eq!(retained_usage["retention"]["entries"], 1);
    assert_eq!(retained_usage["summary"]["removalPolicyRetained"], 1);
    let retained_usage_styles = retained_usage["styles"]
        .as_array()
        .expect("retained usage styles must be an array");
    let retained = retained_usage_styles
        .iter()
        .find(|style| style["bundleId"] == "dead" && style["styleId"] == retained_style_id)
        .expect("retained usage entry must exist");
    assert_eq!(retained["selected"], true);
    assert_eq!(retained["staticReachability"], "unreachable");
    assert_eq!(retained["usageState"], "dead");
    assert_eq!(retained["retention"]["state"], "retained");
    assert_eq!(retained["retention"]["entryId"], "external-email-renderer");
    assert_eq!(retained["removalDisposition"], "policy-retained");
    let same_style_other_bundle = retained_usage_styles
        .iter()
        .find(|style| style["bundleId"] == "themed-dead" && style["styleId"] == retained_style_id)
        .expect("same StyleId in the other bundle must remain explainable");
    assert_eq!(same_style_other_bundle["selected"], false);
    assert_eq!(
        same_style_other_bundle["retention"]["state"],
        "not-retained"
    );
    assert_eq!(same_style_other_bundle["removalDisposition"], "removed");
    pliego_css_usage::parse_usage_analysis(
        &fs::read(
            fixture
                .root()
                .join("bundle-multi-retained/pliego.usage.json"),
        )
        .expect("retained usage report must be readable"),
    )
    .expect("schema-two usage analysis must satisfy its closed contract");

    for file in ["pliego.assets.json", "pliego.index.json"] {
        let document = read_json(fixture.root().join("bundle-multi-retained").join(file));
        assert_eq!(document["schemaVersion"], 2, "{file} did not upgrade");
        assert_eq!(
            document["ruleSelection"], "reachable-or-retained-style-ids",
            "{file} lost retained selection semantics",
        );
    }
    let retained_control_bytes = fs::read(
        fixture
            .root()
            .join("bundle-multi-retained/pliego.css.manifest.json"),
    )
    .expect("retained control manifest must exist");
    let retained_control = pliego_css_control::parse_control_manifest(&retained_control_bytes)
        .expect("retained control manifest must be canonical");
    assert!(
        retained_control.inputs.files.iter().any(|input| {
            input.file == "multi-retention.json" && input.role == "usage-retention"
        })
    );

    let retained_snapshot = directory_snapshot(&fixture.root().join("bundle-multi-retained"));
    let mut retained_check = retained_arguments.clone();
    retained_check.push("--check".into());
    assert_success(&run(fixture.root(), &retained_check));
    assert_eq!(
        directory_snapshot(&fixture.root().join("bundle-multi-retained")),
        retained_snapshot,
        "retained bundle --check mutated outputs",
    );

    let observation_bytes = build_usage_observation(UsageObservationInput::new(
        retained_usage["universeSha256"]
            .as_str()
            .expect("retained universe digest must be a string"),
        pliego_css_build::artifacts::sha256_hex(&reachability_bytes),
        UsageObservationCoverage::Sampled,
        "browser-fixture",
        "1",
        UsageObservationScopeInput::new(
            vec!["/".into()],
            vec![],
            vec!["seed".into()],
            vec!["chromium".into()],
            vec!["1280x720".into()],
            vec!["default".into()],
        ),
        vec![],
        vec![UsageObservedStyleInput::new("dead", &retained_style_id)],
    ))
    .expect("contradictory observation must be structurally valid");
    fs::write(
        fixture.root().join("multi-observations.json"),
        observation_bytes,
    )
    .expect("contradictory observation must be written");
    let mut contradictory_arguments = retained_arguments.clone();
    contradictory_arguments.extend([
        "--observations".to_owned(),
        "multi-observations.json".to_owned(),
    ]);
    let contradiction = run(fixture.root(), &contradictory_arguments);
    assert_failure(
        &contradiction,
        "usage observation contradicts unreachable style",
    );
    assert_eq!(
        directory_snapshot(&fixture.root().join("bundle-multi-retained")),
        retained_snapshot,
        "contradictory retained evidence published a partial output group",
    );

    let mut stale_retention: Value =
        serde_json::from_slice(&retention_bytes).expect("retention sidecar must be JSON");
    stale_retention["universeSha256"] = Value::String("0".repeat(64));
    write_json(
        fixture.root().join("multi-retention.json"),
        &stale_retention,
    );
    let stale = run(fixture.root(), &retained_arguments);
    assert_failure(
        &stale,
        "usage retention universeSha256 does not match current universe",
    );
    assert_eq!(
        directory_snapshot(&fixture.root().join("bundle-multi-retained")),
        retained_snapshot,
        "stale retention evidence published a partial output group",
    );
    fs::write(fixture.root().join("multi-retention.json"), retention_bytes)
        .expect("valid retention sidecar must be restored");
}

fn compile_success(
    root: &Path,
    stem: &str,
    reachability: &str,
    version: u8,
    prune: bool,
    theme: bool,
) -> Artifact {
    let css_name = format!("out/{stem}.css");
    let manifest_name = format!("out/{stem}.manifest.json");
    let mut arguments = vec![
        "compile".into(),
        "--source".into(),
        "src".into(),
        "--seed".into(),
        "--output".into(),
        css_name.clone(),
        "--manifest".into(),
        manifest_name.clone(),
        "--manifest-version".into(),
        version.to_string(),
        "--reachability".into(),
        reachability.into(),
    ];
    if prune {
        arguments.push("--prune-unreachable".into());
    }
    if theme {
        arguments.push("--theme".into());
    }
    assert_success(&run(root, &arguments));
    let css = fs::read(root.join(css_name)).expect("compiled CSS must be readable");
    let manifest_bytes = fs::read(root.join(manifest_name)).expect("manifest must be readable");
    let manifest = serde_json::from_slice(&manifest_bytes).expect("manifest must be valid JSON");
    Artifact {
        css,
        manifest_bytes,
        manifest,
    }
}

fn reachability(sites: &Sites) -> Value {
    json!({
        "schema": 1,
        "applicationCoverage": "complete",
        "components": [
            component("route", sites.route),
            component("island", sites.island),
            component("dead", sites.dead),
            component("shared-live", sites.shared_live),
            component("shared-dead", sites.shared_dead),
            component("co-live", sites.coowned),
            component("co-dead", sites.coowned),
        ],
        "routes": [{
            "id": "home",
            "path": "/",
            "components": ["route", "shared-live", "co-live"],
        }],
        "islands": [{
            "id": "preview",
            "name": "Preview",
            "components": ["island"],
        }],
    })
}

fn component(id: &str, site: Site) -> Value {
    json!({
        "id": id,
        "sites": [{
            "file": "src/styles.rs",
            "byteStart": site.start,
            "byteEnd": site.end,
        }],
    })
}

fn all_sites(source: &str, invocation: &str) -> Vec<Site> {
    source
        .match_indices(invocation)
        .map(|(start, matched)| Site {
            start,
            end: start + matched.len(),
        })
        .collect()
}

fn unique_site(source: &str, invocation: &str) -> Site {
    let sites = all_sites(source, invocation);
    assert_eq!(
        sites.len(),
        1,
        "fixture invocation `{invocation}` must be unique"
    );
    sites[0]
}

fn style_for_site(manifest: &Value, site: Site) -> &Value {
    manifest_styles(manifest)
        .iter()
        .find(|style| origin_sites(style).contains(&site))
        .unwrap_or_else(|| panic!("no style owns bytes {}..{}", site.start, site.end))
}

fn style_by_id<'a>(manifest: &'a Value, id: &str) -> &'a Value {
    manifest_styles(manifest)
        .iter()
        .find(|style| style["styleId"] == id)
        .unwrap_or_else(|| panic!("style `{id}` is missing"))
}

fn style_id(style: &Value) -> String {
    value_string(style, "styleId").to_owned()
}

fn class_name(style: &Value) -> &str {
    value_string(style, "className")
}

fn origin_sites(style: &Value) -> BTreeSet<Site> {
    style["origins"]
        .as_array()
        .expect("style origins must be an array")
        .iter()
        .map(|origin| Site {
            start: usize::try_from(
                origin["byteStart"]
                    .as_u64()
                    .expect("origin byteStart must be an integer"),
            )
            .expect("origin byteStart must fit usize"),
            end: usize::try_from(
                origin["byteEnd"]
                    .as_u64()
                    .expect("origin byteEnd must be an integer"),
            )
            .expect("origin byteEnd must fit usize"),
        })
        .collect()
}

fn manifest_styles(manifest: &Value) -> &[Value] {
    manifest["styles"]
        .as_array()
        .expect("manifest styles must be an array")
}

fn manifest_style_ids(manifest: &Value) -> BTreeSet<String> {
    manifest_styles(manifest).iter().map(style_id).collect()
}

fn token_names(manifest: &Value) -> BTreeSet<&str> {
    graph_array(manifest, "tokens")
        .iter()
        .map(|token| value_string(token, "name"))
        .collect()
}

fn graph_array<'a>(manifest: &'a Value, field: &str) -> &'a [Value] {
    manifest["graph"][field]
        .as_array()
        .unwrap_or_else(|| panic!("graph `{field}` must be an array"))
}

fn ids(graph: &Value, field: &str) -> BTreeSet<String> {
    graph[field]
        .as_array()
        .unwrap_or_else(|| panic!("graph `{field}` must be an array"))
        .iter()
        .map(|node| value_string(node, "id").to_owned())
        .collect()
}

fn assert_coownership_edges(manifest: &Value, style_id: &str) {
    let graph = &manifest["graph"];
    let declarations = graph["declarations"]
        .as_array()
        .expect("declarations must be an array")
        .iter()
        .filter(|declaration| declaration["styleId"] == style_id)
        .map(|declaration| value_string(declaration, "id"))
        .collect::<Vec<_>>();
    assert!(!declarations.is_empty());
    let edges = graph["edges"].as_array().expect("edges must be an array");
    for owner in ["component:co-live", "component:co-dead"] {
        for declaration in &declarations {
            assert!(
                edges.iter().any(|edge| {
                    edge["kind"] == "componentUsesDeclaration"
                        && edge["from"] == owner
                        && edge["to"] == *declaration
                }),
                "co-owner `{owner}` lost declaration `{declaration}`"
            );
        }
    }
}

fn assert_root_edges(manifest: &Value) {
    let edges = graph_array(manifest, "edges");
    for (kind, from, to) in [
        ("routeUsesComponent", "route:home", "component:route"),
        ("islandUsesComponent", "island:preview", "component:island"),
    ] {
        assert!(
            edges
                .iter()
                .any(|edge| { edge["kind"] == kind && edge["from"] == from && edge["to"] == to }),
            "missing root edge {kind}: {from} -> {to}"
        );
    }
}

fn value_string<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("`{field}` must be a string"))
}

fn object_by_id_mut<'a>(document: &'a mut Value, field: &str, id: &str) -> &'a mut Value {
    array_mut(document, field)
        .iter_mut()
        .find(|item| item["id"] == id)
        .unwrap_or_else(|| panic!("`{field}` has no `{id}` object"))
}

fn array_mut<'a>(document: &'a mut Value, field: &str) -> &'a mut Vec<Value> {
    document[field]
        .as_array_mut()
        .unwrap_or_else(|| panic!("`{field}` must be an array"))
}

fn reverse_array(document: &mut Value, field: &str) {
    array_mut(document, field).reverse();
}

fn write_json(path: impl AsRef<Path>, value: &Value) {
    let path = path.as_ref();
    let mut bytes = serde_json::to_vec_pretty(value).expect("fixture JSON must serialize");
    bytes.push(b'\n');
    fs::write(path, bytes)
        .unwrap_or_else(|error| panic!("cannot write `{}`: {error}", path.display()));
}

fn read_json(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    let bytes =
        fs::read(path).unwrap_or_else(|error| panic!("cannot read `{}`: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("invalid JSON in `{}`: {error}", path.display()))
}

fn directory_snapshot(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut snapshot = BTreeMap::new();
    for entry in fs::read_dir(path).expect("snapshot directory must be readable") {
        let entry = entry.expect("snapshot entry must be readable");
        if entry
            .file_type()
            .expect("snapshot file type must be readable")
            .is_file()
        {
            snapshot.insert(
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path()).expect("snapshot file must be readable"),
            );
        }
    }
    snapshot
}

fn css_text(css: &[u8]) -> &str {
    std::str::from_utf8(css).expect("CSS output must be UTF-8")
}

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn wait_for(label: &str, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !predicate() {
        assert!(Instant::now() < deadline, "timed out waiting for {label}");
        thread::sleep(Duration::from_millis(25));
    }
}

fn watch_publication_is_readable(root: &Path) -> bool {
    let output = root.join("out");
    for file in [
        "watch.css",
        "watch.manifest.json",
        "watch.css.map",
        pliego_css_control::TOKEN_GRAPH_FILE,
        pliego_css_control::projection::FINDINGS_FILE,
    ] {
        if fs::read(output.join(file)).is_err() {
            return false;
        }
    }
    let Ok(manifest) = fs::read(output.join(pliego_css_control::CONTROL_MANIFEST_FILE)) else {
        return false;
    };
    let Ok(receipt) = fs::read(output.join(pliego_css_control::BUILD_RECEIPT_FILE)) else {
        return false;
    };
    pliego_css_control::parse_control_manifest(&manifest).is_ok()
        && pliego_css_control::parse_build_receipt(&receipt, &manifest).is_ok()
}

fn run(root: &Path, arguments: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("pliego-cssc must execute")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed with {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn assert_failure(output: &Output, expected: &str) {
    assert!(!output.status.success(), "command unexpectedly succeeded");
    assert!(
        output.stdout.is_empty(),
        "failed command wrote stdout: {}",
        String::from_utf8_lossy(&output.stdout),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "stderr did not contain `{expected}`:\n{stderr}"
    );
}
