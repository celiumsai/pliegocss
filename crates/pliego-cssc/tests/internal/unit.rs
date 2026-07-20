//! Internal CLI behavior tests.
use super::atomic_write::{commit_prepared, commit_prepared_with};
use super::catalog::check_catalog;
use super::explain::build_explanation;
use super::formatter::{
    format_line_document, format_utility_source, publish_utility_rewrites, run_rust_utility_format,
};
use super::source_candidates::candidates_from_rust;
use super::*;
use pliego_css_agent::{RepairCliFormat, RepairFixMode};
use pliego_css_build::artifacts::{CatalogOutputFormat, optimize_css, render_catalog};
use pliego_css_compiler::utility_catalog;
use pliego_css_control::projection::build_flat_token_measurements;
use pliego_css_ir::TokenKind;
use std::time::{SystemTime, UNIX_EPOCH};

const fn version(major: u32, minor: u32, patch: u32) -> u32 {
    (major << 16) | (minor << 8) | patch
}

impl CliFailure {
    fn contains(&self, pattern: &str) -> bool {
        self.human.contains(pattern)
    }
}

fn compile_resolved_candidates(
    theme: &ThemeRegistry,
    candidates: &[ResolvedCandidate],
    include_theme: bool,
    targets: TargetContract,
    format: CssFormat,
) -> Result<CompiledArtifact, String> {
    compile_resolved_candidates_with_manifest(
        theme,
        candidates,
        include_theme,
        targets,
        format,
        ArtifactGraphOptions::default(),
        &mut CssCaches::default(),
    )
}

fn watch_iteration(
    arguments: &WatchArgs,
    previous: Option<&WatchSnapshot>,
    cache: &mut RustScanCache,
) -> WatchIteration {
    let snapshot = capture_watch_snapshot(arguments);
    watch_iteration_from_snapshot(arguments, snapshot, previous, cache)
}

fn os(arguments: &[&str]) -> Vec<OsString> {
    arguments.iter().map(OsString::from).collect()
}

const DTCG_RESOLVER: &str = r##"{
  "version":"2025.10",
  "name":"CLI unit resolver",
  "$defs":{
    "light":{"color":{"$type":"color","brand":{"$value":{"colorSpace":"srgb","components":[0.9,0.9,0.9],"alpha":1}}}},
    "dark":{"color":{"$type":"color","brand":{"$value":{"colorSpace":"srgb","components":[0.1,0.1,0.1],"alpha":1}}}}
  },
  "sets":{},
  "modifiers":{"appearance":{"contexts":{"light":[{"$ref":"#/$defs/light"}],"dark":[{"$ref":"#/$defs/dark"}]},"default":"light"}},
  "resolutionOrder":[{"$ref":"#/modifiers/appearance"}]
}"##;
const PRODUCT_DTCG_RESOLVER: &str = include_str!("../../../../examples/product.resolver.json");

fn temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = env::temp_dir().join(format!("pliego-cssc-{name}-{}-{nonce}", std::process::id()));
    fs::create_dir(&path).expect("create temporary directory");
    path
}

fn json_keys(value: &serde_json::Value) -> Vec<&str> {
    let mut keys = value
        .as_object()
        .expect("JSON object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

fn assert_theme_inspection_shape(theme: &serde_json::Value) {
    assert_eq!(json_keys(theme), ["breakpoints", "id", "tokens"]);
    assert_eq!(json_keys(&theme["tokens"][0]), ["kind", "name", "value"]);
    assert_eq!(json_keys(&theme["breakpoints"][0]), ["minWidth", "name"]);
}

fn assert_manifest_style_shape(style: &serde_json::Value) {
    assert_eq!(json_keys(style), ["className", "origins", "styleId"]);
    assert_eq!(
        json_keys(&style["origins"][0]),
        [
            "byteEnd",
            "byteStart",
            "file",
            "macroKind",
            "reason",
            "source",
        ]
    );
}

#[test]
fn flat_token_control_graph_separates_identity_from_usage_coverage() {
    let theme = ThemeRegistry::seed();
    let unused = build_flat_token_measurements(&theme, &BTreeSet::new())
        .expect("seed graph must be measurable");
    assert_eq!(
        unused.observation,
        pliego_css_control::MeasurementState::Measured
    );
    assert_eq!(unused.coverage_basis_points, Some(0));
    assert_eq!(unused.aliases, 0);
    assert_eq!(unused.derived_values, 0);
    assert_eq!(unused.cycles, 0);

    let spacing = theme
        .token_by_name(TokenKind::Spacing, "4")
        .expect("seed spacing token");
    let used = BTreeSet::from([FlatTokenReference::new(spacing.kind, spacing.id)]);
    let measured =
        build_flat_token_measurements(&theme, &used).expect("used graph must be measurable");
    let declared = u64::try_from(theme.tokens().len()).expect("token count must fit u64");
    assert_eq!(measured.tokens, declared);
    assert_eq!(
        measured.coverage_basis_points,
        Some(u16::try_from(10_000 / declared).expect("coverage must fit u16"))
    );
    assert_eq!(measured.graph_hash, unused.graph_hash);
}

fn compile_styles(styles: &[String], include_theme: bool) -> Result<CompiledArtifact, CliFailure> {
    let candidates = styles
        .iter()
        .map(|style| Candidate::Direct {
            style: style.clone(),
            provenance: cli_provenance(style, "test"),
        })
        .collect::<Vec<_>>();
    compile_candidates(
        &ThemeRegistry::seed(),
        &candidates,
        include_theme,
        TargetContract::Modern,
        CssFormat::Minified,
    )
}

fn resolved_seed_styles(styles: &[&str]) -> Vec<ResolvedCandidate> {
    let candidates = styles
        .iter()
        .map(|style| Candidate::Direct {
            style: (*style).to_owned(),
            provenance: cli_provenance(style, "identity-test"),
        })
        .collect::<Vec<_>>();
    resolve_candidates(&ThemeRegistry::seed(), &candidates, 1)
        .expect("resolve identity test styles")
}

#[test]
fn rejects_distinct_identity_streams_that_share_a_style_id() {
    let theme = ThemeRegistry::seed();
    let mut resolved = resolved_seed_styles(&["flex", "grid"]);
    let forced_id = resolved[0].semantic.id;
    resolved[1].semantic.id = forced_id;

    let error = compile_resolved_candidates(
        &theme,
        &resolved,
        false,
        TargetContract::Modern,
        CssFormat::Minified,
    )
    .expect_err("must reject");

    assert!(error.contains("StyleId collision under format 2"));
    assert!(error.contains("distinct canonical streams"));
}

#[test]
fn rejects_one_identity_stream_mapped_to_distinct_style_ids() {
    let theme = ThemeRegistry::seed();
    let first = resolved_seed_styles(&["flex"])
        .into_iter()
        .next()
        .expect("resolved style");
    let mut second = first.clone();
    second.semantic.id = pliego_css_ir::StyleId::new(first.semantic.id.get() + 1);

    let error = compile_resolved_candidates(
        &theme,
        &[first, second],
        false,
        TargetContract::Modern,
        CssFormat::Minified,
    )
    .expect_err("must reject");

    assert!(error.contains("canonical style stream under format 2 mapped to both"));
}

#[test]
fn manifest_and_css_follow_canonical_stream_order_not_hash_order() {
    let theme = ThemeRegistry::seed();
    let resolved = resolved_seed_styles(&["grid", "flex", "block"]);
    let mut expected = resolved
        .iter()
        .map(|candidate| {
            (
                try_encode_style_identity_with_theme(&theme, &candidate.semantic)
                    .expect("encode canonical stream"),
                format!("{:032x}", candidate.semantic.id.get()),
                candidate.semantic.id.to_class_name(),
            )
        })
        .collect::<Vec<_>>();
    expected.sort_by(|left, right| left.0.cmp(&right.0));

    let artifact = compile_resolved_candidates(
        &theme,
        &resolved,
        false,
        TargetContract::Modern,
        CssFormat::Minified,
    )
    .expect("compile canonical stream order");
    let actual = artifact
        .styles
        .iter()
        .map(|style| style.style_id.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        expected
            .iter()
            .map(|(_, style_id, _)| style_id.clone())
            .collect::<Vec<_>>()
    );
    let css_positions = expected
        .iter()
        .map(|(_, _, class_name)| {
            artifact
                .css
                .find(&format!(".{class_name}{{"))
                .expect("canonical style rule must be present in CSS")
        })
        .collect::<Vec<_>>();
    assert!(
        css_positions
            .windows(2)
            .all(|positions| positions[0] < positions[1]),
        "CSS rules must retain canonical stream order: {css_positions:?}"
    );
}

fn warm_matches_cold(
    arguments: &WatchArgs,
    previous: Option<&WatchSnapshot>,
    cache: &mut RustScanCache,
) -> WatchIteration {
    let warm = watch_iteration(arguments, previous, cache);
    let mut cold_cache = RustScanCache::default();
    let cold = watch_iteration(arguments, None, &mut cold_cache);
    assert_eq!(warm.snapshot, cold.snapshot);
    assert_eq!(warm.outcome, cold.outcome);
    warm
}

fn write_test_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create test file parent");
    }
    fs::write(path, contents).expect("write test file");
}

fn watch_args(output: PathBuf) -> WatchArgs {
    WatchArgs {
        input: None,
        sources: Vec::new(),
        output,
        config: None,
        tokens: None,
        token_inputs: Vec::new(),
        seed: false,
        theme: false,
        targets: TargetContract::Modern,
        format: CssFormat::Minified,
        manifest: None,
        control_dir: None,
        reachability: None,
        physical_trace: false,
        pruning: ReachabilityPruning::Disabled,
    }
}

#[test]
fn parses_all_compile_sources_config_and_targets() {
    let parsed = parse_arguments(os(&[
        "build",
        "--style",
        "flex",
        "--compose",
        "grid",
        "gap-4",
        "--input",
        "styles.txt",
        "--source",
        "src/a.rs",
        "--source",
        "src/b.rs",
        "--config",
        "theme.toml",
        "--theme",
        "--targets",
        "none",
        "--output",
        "app.css",
        "--manifest",
        "app.json",
    ]))
    .expect("valid arguments");
    let Command::Compile(arguments) = parsed else {
        panic!("build aliases compile")
    };
    assert_eq!(
        arguments.sources,
        [PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")]
    );
    assert_eq!(arguments.config, Some(PathBuf::from("theme.toml")));
    assert_eq!(arguments.targets, TargetContract::None);
}

#[test]
fn parses_dtcg_options_without_collapsing_repeated_inputs() {
    let parsed = parse_arguments(os(&[
        "compile",
        "--style",
        "bg-brand",
        "--tokens",
        "tokens.json",
        "--token-input",
        "appearance=light",
        "--token-input",
        "appearance=dark",
    ]))
    .expect("CLI parsing preserves resolver input order");
    let Command::Compile(arguments) = parsed else {
        panic!("compile command")
    };
    assert_eq!(arguments.tokens, Some(PathBuf::from("tokens.json")));
    assert_eq!(
        arguments.token_inputs,
        [
            ("appearance".to_owned(), "light".to_owned()),
            ("appearance".to_owned(), "dark".to_owned()),
        ]
    );

    let watch = parse_arguments(os(&[
        "watch",
        "--input",
        "styles.txt",
        "--output",
        "app.css",
        "--tokens",
        "tokens.json",
        "--token-input",
        "appearance=dark",
    ]))
    .expect("watch accepts the same DTCG surface");
    assert!(matches!(
        watch,
        Command::Watch(WatchArgs {
            tokens: Some(_),
            token_inputs,
            ..
        }) if token_inputs == [("appearance".to_owned(), "dark".to_owned())]
    ));
}

#[test]
fn dtcg_options_are_closed_and_mutually_exclusive() {
    for arguments in [
        os(&[
            "compile",
            "--style",
            "flex",
            "--tokens",
            "tokens.json",
            "--config",
            "theme.toml",
        ]),
        os(&[
            "compile",
            "--style",
            "flex",
            "--tokens",
            "tokens.json",
            "--seed",
        ]),
        os(&[
            "compile",
            "--style",
            "flex",
            "--token-input",
            "appearance=dark",
        ]),
        os(&[
            "compile",
            "--style",
            "flex",
            "--tokens",
            "first.json",
            "--tokens",
            "second.json",
        ]),
    ] {
        assert!(parse_arguments(arguments).is_err());
    }
    for malformed in ["appearance", "appearance=", "=dark"] {
        let error = parse_arguments(os(&[
            "check",
            "--style",
            "flex",
            "--tokens",
            "tokens.json",
            "--token-input",
            malformed,
        ]))
        .expect_err("malformed token input must fail");
        assert!(error.contains("modifier=context"));
    }
}

#[test]
fn loaded_dtcg_theme_canonicalizes_defaults_and_casefolded_inputs() {
    let default = resolve_dtcg_theme(DTCG_RESOLVER, &[]).expect("default resolver selection");
    let explicit = resolve_dtcg_theme(
        DTCG_RESOLVER,
        &[("APPEARANCE".to_owned(), "LiGhT".to_owned())],
    )
    .expect("case-insensitive resolver selection");
    assert_eq!(default.registry().id(), explicit.registry().id());
    assert_eq!(default.selections(), explicit.selections());
    assert_eq!(
        default
            .graph()
            .to_canonical_json()
            .expect("canonical graph"),
        explicit
            .graph()
            .to_canonical_json()
            .expect("canonical graph")
    );
    let adapter = default.graph().adapter.as_ref().expect("resolver adapter");
    assert_eq!(adapter.name, "dtcg-resolver");
}

#[test]
fn bundle_dtcg_loader_retains_the_canonical_active_selection() {
    let directory = temp_dir("bundle-dtcg-loader");
    let resolver = directory.join("product.resolver.json");
    fs::write(&resolver, PRODUCT_DTCG_RESOLVER).expect("write bundle resolver");
    let request = BundleThemeDocument {
        kind: BundleThemeKind::DtcgResolver,
        path: Some(PathBuf::from("product.resolver.json")),
        inputs: Some(BTreeMap::from([
            ("APPEARANCE".into(), "LiGhT".into()),
            ("channel".into(), "dark".into()),
        ])),
    };
    let (loaded, input) = load_bundle_theme(&request, &directory).expect("load DTCG bundle theme");
    let LoadedTheme::Dtcg(theme) = loaded else {
        panic!("bundle DTCG request returned a flat registry");
    };
    assert_eq!(
        theme.selections(),
        &BTreeMap::from([
            ("appearance".into(), "light".into()),
            ("channel".into(), "dark".into()),
        ])
    );
    let input = input.expect("resolver ledger input");
    assert_eq!(input.role, "token-resolver");
    assert_eq!(input.bytes, PRODUCT_DTCG_RESOLVER.as_bytes());
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn token_resolver_reader_is_bounded_regular_and_utf8() {
    let directory = temp_dir("dtcg-reader");
    let oversized = directory.join("oversized.json");
    fs::File::create(&oversized)
        .expect("create oversized resolver")
        .set_len((MAX_DOCUMENT_BYTES + 1) as u64)
        .expect("extend oversized resolver");
    let error = read_bounded_utf8_document(&oversized, "token resolver")
        .expect_err("oversized resolver must fail before reading");
    assert!(error.contains("token resolver"));
    assert!(error.contains(&oversized.display().to_string()));
    assert!(error.contains("exceeds"));
    let error = load_bundle_plan(&oversized).expect_err("oversized bundle plan must fail closed");
    assert!(error.human.contains("bundle plan"));
    assert!(error.human.contains("exceeds"));

    let error = read_bounded_utf8_document(&directory, "token resolver")
        .expect_err("directory must not be accepted as a resolver");
    assert!(error.contains("token resolver"));
    assert!(error.contains(&directory.display().to_string()));
    assert!(error.contains("regular file"));

    let invalid_utf8 = directory.join("invalid-utf8.json");
    fs::write(&invalid_utf8, [0xff, 0xfe]).expect("write invalid UTF-8 resolver");
    let error = read_bounded_utf8_document(&invalid_utf8, "token resolver")
        .expect_err("invalid UTF-8 resolver must fail");
    assert!(error.contains("token resolver"));
    assert!(error.contains(&invalid_utf8.display().to_string()));
    assert!(error.contains("not valid UTF-8"));

    let valid = directory.join("valid.json");
    fs::write(&valid, b"{}\n").expect("write valid bounded document");
    assert_eq!(
        read_bounded_utf8_document(&valid, "token resolver").expect("read regular document"),
        b"{}\n"
    );
    #[cfg(unix)]
    {
        let linked = directory.join("linked.json");
        std::os::unix::fs::symlink(&valid, &linked).expect("link resolver");
        assert!(
            read_bounded_utf8_document(&linked, "token resolver")
                .expect_err("no-follow reader must reject a symbolic link")
                .contains("regular file")
        );
    }
    #[cfg(windows)]
    {
        use std::io::ErrorKind;
        use std::os::windows::fs::symlink_file;

        let linked = directory.join("linked.json");
        match symlink_file(&valid, &linked) {
            Ok(()) => {
                assert!(
                    read_bounded_utf8_document(&linked, "token resolver")
                        .expect_err("no-follow reader must reject a file symlink")
                        .contains("regular file")
                );
                fs::remove_file(linked).expect("remove file symlink");
            }
            Err(error)
                if error.kind() == ErrorKind::PermissionDenied
                    || error.raw_os_error() == Some(1314) => {}
            Err(error) => panic!("create file symlink: {error}"),
        }
    }

    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn compile_and_watch_control_options_are_strict() {
    assert!(matches!(
        parse_arguments(os(&[
            "compile",
            "--style",
            "flex",
            "--seed",
            "--output",
            "dist/app.css",
            "--manifest",
            "dist/app.manifest.json",
            "--control-dir",
            "dist",
            "--check",
        ])),
        Ok(Command::Compile(BuildArgs {
            control_dir: Some(_),
            check: true,
            ..
        }))
    ));
    assert!(matches!(
        parse_arguments(os(&[
            "watch",
            "--source",
            "src",
            "--seed",
            "--output",
            "dist/app.css",
            "--manifest",
            "dist/app.manifest.json",
            "--control-dir",
            "dist",
        ])),
        Ok(Command::Watch(WatchArgs {
            control_dir: Some(_),
            ..
        }))
    ));
    for arguments in [
        os(&[
            "compile",
            "--style",
            "flex",
            "--output",
            "dist/app.css",
            "--control-dir",
            "dist",
        ]),
        os(&[
            "compile",
            "--style",
            "flex",
            "--output",
            "dist/app.css",
            "--manifest",
            "dist/app.json",
            "--check",
        ]),
        os(&[
            "watch",
            "--source",
            "src",
            "--output",
            "dist/app.css",
            "--control-dir",
            "dist",
        ]),
    ] {
        assert!(parse_arguments(arguments).is_err());
    }
}

#[test]
fn manifest_schema_five_enables_physical_trace_for_every_artifact_command() {
    let compile = parse_arguments(os(&[
        "compile",
        "--style",
        "flex",
        "--output",
        "app.css",
        "--manifest",
        "app.json",
        "--manifest-version",
        "5",
        "--reachability",
        "reachability.json",
    ]))
    .expect("valid schema five compile");
    assert!(matches!(
        compile,
        Command::Compile(BuildArgs {
            physical_trace: true,
            ..
        })
    ));
    let watch = parse_arguments(os(&[
        "watch",
        "--input",
        "styles.txt",
        "--output",
        "app.css",
        "--manifest",
        "app.json",
        "--manifest-version",
        "5",
        "--reachability",
        "reachability.json",
    ]))
    .expect("valid schema five watch");
    assert!(matches!(
        watch,
        Command::Watch(WatchArgs {
            physical_trace: true,
            ..
        })
    ));

    let bundle = parse_arguments(os(&[
        "bundle",
        "--plan",
        "pliego.bundles.toml",
        "--output-dir",
        "dist",
        "--manifest-version",
        "5",
        "--reachability",
        "reachability.json",
    ]))
    .expect("valid schema five bundle");
    assert!(matches!(
        bundle,
        Command::Bundle(BundleArgs {
            physical_trace: true,
            ..
        })
    ));

    let semantic = parse_arguments(os(&[
        "compile",
        "--style",
        "flex",
        "--manifest",
        "app.json",
        "--manifest-version",
        "4",
        "--reachability",
        "reachability.json",
    ]))
    .expect("valid schema four compile");
    assert!(matches!(
        semantic,
        Command::Compile(BuildArgs {
            physical_trace: false,
            ..
        })
    ));
}

#[test]
fn manifest_schema_five_requires_manifest_and_reachability() {
    for arguments in [
        os(&[
            "compile",
            "--style",
            "flex",
            "--manifest",
            "app.json",
            "--manifest-version",
            "5",
        ]),
        os(&[
            "compile",
            "--style",
            "flex",
            "--manifest-version",
            "5",
            "--reachability",
            "reachability.json",
        ]),
        os(&[
            "compile",
            "--style",
            "flex",
            "--manifest",
            "app.json",
            "--manifest-version",
            "3",
            "--reachability",
            "reachability.json",
        ]),
        os(&[
            "compile",
            "--style",
            "flex",
            "--manifest",
            "app.json",
            "--manifest-version",
            "6",
        ]),
    ] {
        assert!(parse_arguments(arguments).is_err());
    }
}

#[test]
fn single_value_options_reject_duplicates_for_every_command() {
    for command in ["compile", "check", "inspect"] {
        for repeated in [
            vec!["--seed", "--seed"],
            vec!["--theme", "--theme"],
            vec!["--targets", "none", "--targets", "modern"],
            vec!["--format", "pretty", "--format", "minified"],
        ] {
            let mut arguments = vec![command, "--style", "flex"];
            arguments.extend(repeated);
            assert!(
                parse_arguments(os(&arguments))
                    .expect_err("must reject")
                    .contains("may only be provided once")
            );
        }
    }

    for repeated in [
        vec!["--seed", "--seed"],
        vec!["--theme", "--theme"],
        vec!["--targets", "none", "--targets", "modern"],
        vec!["--format", "pretty", "--format", "minified"],
    ] {
        let mut arguments = vec!["watch", "--input", "styles.txt", "--output", "app.css"];
        arguments.extend(repeated);
        assert!(
            parse_arguments(os(&arguments))
                .expect_err("must reject")
                .contains("may only be provided once")
        );
    }

    for arguments in [
        vec!["catalog", "--seed", "--seed"],
        vec!["catalog", "--format", "json", "--format", "markdown"],
        vec!["explain", "--style", "flex", "--seed", "--seed"],
        vec![
            "explain",
            "--style",
            "flex",
            "--targets",
            "none",
            "--targets",
            "modern",
        ],
        vec![
            "explain", "--style", "flex", "--format", "json", "--format", "text",
        ],
        vec![
            "explain-cascade",
            "--input",
            "app.css",
            "--input",
            "other.css",
            "--element",
            "button",
            "--property",
            "color",
        ],
        vec![
            "explain-cascade",
            "--input",
            "app.css",
            "--element",
            "button",
            "--property",
            "color",
            "--format",
            "json",
            "--format",
            "text",
        ],
        vec![
            "plan",
            "--findings",
            "findings.json",
            "--findings",
            "other.json",
            "--proposal",
            "proposal.json",
            "--source-root",
            ".",
        ],
        vec![
            "fix",
            "--plan",
            "plan.json",
            "--source-root",
            ".",
            "--dry-run",
            "--dry-run",
        ],
    ] {
        assert!(
            parse_arguments(os(&arguments))
                .expect_err("must reject")
                .contains("may only be provided once")
        );
    }
}

#[test]
fn global_diagnostic_format_is_position_independent_and_does_not_shadow_output_formats() {
    for arguments in [
        os(&[
            "--diagnostic-format",
            "json",
            "explain",
            "--style",
            "flex",
            "--format",
            "text",
        ]),
        os(&[
            "explain",
            "--style",
            "flex",
            "--format",
            "text",
            "--diagnostic-format=json",
        ]),
    ] {
        let global = parse_global_options(arguments).expect("valid global options");
        assert_eq!(global.diagnostic_format, DiagnosticFormat::Json);
        assert_eq!(
            global.arguments,
            os(&["explain", "--style", "flex", "--format", "text"])
        );
        assert!(matches!(
            parse_arguments(global.arguments),
            Ok(Command::Explain(ExplainArgs {
                format: ExplainFormat::Text,
                ..
            }))
        ));
    }

    let duplicate = parse_global_options(os(&[
        "check",
        "--diagnostic-format",
        "json",
        "--diagnostic-format=human",
    ]))
    .expect_err("must reject");
    assert_eq!(duplicate.diagnostic_format, DiagnosticFormat::Json);
    assert_eq!(duplicate.command.as_deref(), Some("check"));
    assert_eq!(duplicate.failure.diagnostics[0].code, "PCL001");
    let duplicate_json = render_failure_json(duplicate.command.as_deref(), &duplicate.failure);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&duplicate_json).expect("global diagnostic JSON")
            ["command"],
        "check"
    );

    let leading_duplicate = parse_global_options(os(&[
        "--diagnostic-format=json",
        "--diagnostic-format=human",
        "check",
    ]))
    .expect_err("must reject");
    assert_eq!(leading_duplicate.command.as_deref(), Some("check"));

    let value_named_like_global =
        parse_global_options(os(&["compile", "--style", "--diagnostic-format=json"]))
            .expect("recognized option values are not stolen by the global parser");
    assert_eq!(
        value_named_like_global,
        GlobalOptions {
            diagnostic_format: DiagnosticFormat::Human,
            arguments: os(&["compile", "--style", "--diagnostic-format=json"]),
        }
    );

    let watch = run(
        os(&["watch", "--input", "styles.txt", "--output", "app.css"]),
        DiagnosticFormat::Json,
    )
    .expect_err("must reject");
    assert_eq!(watch.diagnostics[0].code, "PCL001");
    assert!(watch.contains("long-lived event stream"));
}

#[allow(clippy::too_many_lines)]
#[test]
fn bundle_cli_parsing_is_strict_and_global_diagnostics_respect_option_arity() {
    assert_eq!(
        parse_arguments(os(&[
            "bundle",
            "--plan",
            "config/pliego.bundles.toml",
            "--output-dir",
            "dist/assets",
            "--check",
        ])),
        Ok(Command::Bundle(BundleArgs {
            plan: PathBuf::from("config/pliego.bundles.toml"),
            output_dir: PathBuf::from("dist/assets"),
            check: true,
            asset_plan: false,
            project_index: false,
            usage_report: false,
            control: false,
            observations: None,
            retention: None,
            critical_evidence: None,
            reachability: None,
            physical_trace: false,
            pruning: ReachabilityPruning::Disabled,
        }))
    );
    assert!(matches!(
        parse_arguments(os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--manifest-version",
            "4",
            "--reachability",
            "reachability.json",
            "--asset-plan",
            "--control",
        ])),
        Ok(Command::Bundle(BundleArgs {
            asset_plan: true,
            control: true,
            physical_trace: false,
            ..
        }))
    ));
    assert!(matches!(
        parse_arguments(os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--manifest-version",
            "5",
            "--reachability",
            "reachability.json",
            "--asset-plan",
            "--usage-report",
            "--critical-evidence",
            "critical.json",
        ])),
        Ok(Command::Bundle(BundleArgs {
            asset_plan: true,
            usage_report: true,
            critical_evidence: Some(_),
            ..
        }))
    ));
    assert!(matches!(
        parse_arguments(os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--manifest-version",
            "5",
            "--reachability",
            "reachability.json",
            "--asset-plan",
            "--project-index",
        ])),
        Ok(Command::Bundle(BundleArgs {
            asset_plan: true,
            project_index: true,
            physical_trace: true,
            ..
        }))
    ));
    assert!(matches!(
        parse_arguments(os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--usage-report",
        ])),
        Ok(Command::Bundle(BundleArgs {
            usage_report: true,
            observations: None,
            ..
        }))
    ));
    assert!(matches!(
        parse_arguments(os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--manifest-version",
            "4",
            "--reachability",
            "reachability.json",
            "--usage-report",
            "--observations",
            "observations.json",
        ])),
        Ok(Command::Bundle(BundleArgs {
            usage_report: true,
            observations: Some(_),
            ..
        }))
    ));
    assert!(matches!(
        parse_arguments(os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--manifest-version",
            "4",
            "--reachability",
            "reachability.json",
            "--prune-unreachable",
            "--usage-report",
            "--retention",
            "retention.json",
        ])),
        Ok(Command::Bundle(BundleArgs {
            usage_report: true,
            retention: Some(_),
            pruning: ReachabilityPruning::Unreachable,
            ..
        }))
    ));
    for arguments in [
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--observations",
            "observations.json",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--usage-report",
            "--usage-report",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--usage-report",
            "--observations",
            "one.json",
            "--observations",
            "two.json",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--retention",
            "retention.json",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--manifest-version",
            "4",
            "--reachability",
            "reachability.json",
            "--prune-unreachable",
            "--retention",
            "retention.json",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--manifest-version",
            "4",
            "--reachability",
            "reachability.json",
            "--usage-report",
            "--retention",
            "one.json",
            "--retention",
            "two.json",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--manifest-version",
            "5",
            "--reachability",
            "reachability.json",
            "--asset-plan",
            "--critical-evidence",
            "critical.json",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--manifest-version",
            "5",
            "--reachability",
            "reachability.json",
            "--asset-plan",
            "--usage-report",
            "--critical-evidence",
            "one.json",
            "--critical-evidence",
            "two.json",
        ]),
    ] {
        assert!(parse_arguments(arguments).is_err());
    }

    for arguments in [
        os(&["bundle", "--output-dir", "dist"]),
        os(&["bundle", "--plan", "plan.toml"]),
        os(&[
            "bundle",
            "--plan",
            "one.toml",
            "--plan",
            "two.toml",
            "--output-dir",
            "dist",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--check",
            "--check",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--watch",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--asset-plan",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--control",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--manifest-version",
            "4",
            "--reachability",
            "reachability.json",
            "--asset-plan",
            "--asset-plan",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--manifest-version",
            "5",
            "--reachability",
            "reachability.json",
            "--project-index",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--manifest-version",
            "4",
            "--reachability",
            "reachability.json",
            "--asset-plan",
            "--project-index",
        ]),
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "dist",
            "--manifest-version",
            "5",
            "--reachability",
            "reachability.json",
            "--asset-plan",
            "--project-index",
            "--project-index",
        ]),
    ] {
        assert!(parse_arguments(arguments).is_err());
    }

    let global = parse_global_options(os(&[
        "bundle",
        "--plan",
        "plan.toml",
        "--diagnostic-format=json",
        "--output-dir",
        "dist",
    ]))
    .expect("late global diagnostics option");
    assert_eq!(global.diagnostic_format, DiagnosticFormat::Json);
    assert_eq!(
        global.arguments,
        os(&["bundle", "--plan", "plan.toml", "--output-dir", "dist",])
    );

    let plan_named_like_global = parse_global_options(os(&[
        "bundle",
        "--plan",
        "--diagnostic-format=json",
        "--output-dir",
        "dist",
    ]))
    .expect("plan option value must not be stolen");
    assert_eq!(
        plan_named_like_global,
        GlobalOptions {
            diagnostic_format: DiagnosticFormat::Human,
            arguments: os(&[
                "bundle",
                "--plan",
                "--diagnostic-format=json",
                "--output-dir",
                "dist",
            ]),
        }
    );

    let output_named_like_global = parse_global_options(os(&[
        "--diagnostic-format",
        "json",
        "bundle",
        "--plan",
        "plan.toml",
        "--output-dir",
        "--diagnostic-format=human",
    ]))
    .expect("output option value must not become a duplicate global option");
    assert_eq!(
        output_named_like_global.diagnostic_format,
        DiagnosticFormat::Json
    );
    assert_eq!(
        output_named_like_global.arguments,
        os(&[
            "bundle",
            "--plan",
            "plan.toml",
            "--output-dir",
            "--diagnostic-format=human",
        ])
    );
}

#[allow(clippy::too_many_lines)]
#[test]
fn bundle_plan_schema_theme_names_and_relative_paths_are_strict() {
    let valid_schema_one: BundlePlanDocument = toml::from_str(
        r#"schema = 1
targets = "modern"
format = "pretty"
[theme]
kind = "seed"
[bundles.account-settings]
sources = ["src/account.rs"]
emit-theme = true
"#,
    )
    .expect("valid bundle plan");
    validate_bundle_plan(&valid_schema_one).expect("schema 1 plan");

    let plan = |schema, kind, path, inputs| BundlePlanDocument {
        schema,
        targets: TargetContract::Modern,
        format: CssFormat::Minified,
        theme: BundleThemeDocument { kind, path, inputs },
        bundles: BTreeMap::from([(
            "app".into(),
            BundleSpecDocument {
                sources: vec![PathBuf::from("src/app.rs")],
                emit_theme: false,
            },
        )]),
    };

    for (label, document) in [
        ("schema 1 seed", plan(1, BundleThemeKind::Seed, None, None)),
        (
            "schema 1 config",
            plan(
                1,
                BundleThemeKind::Config,
                Some(PathBuf::from("theme.toml")),
                None,
            ),
        ),
        ("schema 2 seed", plan(2, BundleThemeKind::Seed, None, None)),
        (
            "schema 2 config",
            plan(
                2,
                BundleThemeKind::Config,
                Some(PathBuf::from("theme.toml")),
                None,
            ),
        ),
        (
            "schema 2 DTCG defaults",
            plan(
                2,
                BundleThemeKind::DtcgResolver,
                Some(PathBuf::from("product.resolver.json")),
                None,
            ),
        ),
        (
            "schema 2 DTCG empty inputs",
            plan(
                2,
                BundleThemeKind::DtcgResolver,
                Some(PathBuf::from("product.resolver.json")),
                Some(BTreeMap::new()),
            ),
        ),
        (
            "schema 2 DTCG selected inputs",
            plan(
                2,
                BundleThemeKind::DtcgResolver,
                Some(PathBuf::from("product.resolver.json")),
                Some(BTreeMap::from([("appearance".into(), "dark".into())])),
            ),
        ),
    ] {
        validate_bundle_plan(&document).unwrap_or_else(|error| panic!("{label}: {error}"));
    }

    for (label, document, expected) in [
        (
            "unknown schema zero",
            plan(0, BundleThemeKind::Seed, None, None),
            "expected schema 1 or 2",
        ),
        (
            "unknown schema three",
            plan(3, BundleThemeKind::Seed, None, None),
            "expected schema 1 or 2",
        ),
        (
            "schema 1 DTCG",
            plan(
                1,
                BundleThemeKind::DtcgResolver,
                Some(PathBuf::from("product.resolver.json")),
                None,
            ),
            "schema 1 does not support",
        ),
        (
            "schema 1 empty inputs",
            plan(1, BundleThemeKind::Seed, None, Some(BTreeMap::new())),
            "schema 1 does not accept `theme.inputs`",
        ),
        (
            "seed path",
            plan(
                2,
                BundleThemeKind::Seed,
                Some(PathBuf::from("theme.toml")),
                None,
            ),
            "`seed` does not accept `path`",
        ),
        (
            "seed inputs",
            plan(2, BundleThemeKind::Seed, None, Some(BTreeMap::new())),
            "`seed` does not accept `inputs`",
        ),
        (
            "config missing path",
            plan(2, BundleThemeKind::Config, None, None),
            "`config` requires `path`",
        ),
        (
            "config inputs",
            plan(
                2,
                BundleThemeKind::Config,
                Some(PathBuf::from("theme.toml")),
                Some(BTreeMap::new()),
            ),
            "`config` does not accept `inputs`",
        ),
        (
            "DTCG missing path",
            plan(
                2,
                BundleThemeKind::DtcgResolver,
                None,
                Some(BTreeMap::new()),
            ),
            "`dtcg-resolver` requires `path`",
        ),
    ] {
        let error = validate_bundle_plan(&document).expect_err(label);
        assert!(
            error.contains(expected),
            "{label}: missing `{expected}` in `{error}`"
        );
    }

    let schema_one_empty_inputs: BundlePlanDocument = toml::from_str(
        r#"schema = 1
targets = "modern"
format = "minified"
[theme]
kind = "seed"
[theme.inputs]
[bundles.app]
sources = ["src/app.rs"]
"#,
    )
    .expect("empty inputs table must deserialize distinctly from absence");
    assert_eq!(schema_one_empty_inputs.theme.inputs, Some(BTreeMap::new()));
    assert!(
        validate_bundle_plan(&schema_one_empty_inputs)
            .expect_err("schema 1 must reject even an empty inputs table")
            .contains("schema 1 does not accept `theme.inputs`")
    );

    let parsed_dtcg: BundlePlanDocument = toml::from_str(
        r#"schema = 2
targets = "modern"
format = "minified"
[theme]
kind = "dtcg-resolver"
path = "product.resolver.json"
[theme.inputs]
appearance = "dark"
[bundles.app]
sources = ["src/app.rs"]
"#,
    )
    .expect("schema 2 DTCG plan must deserialize");
    assert_eq!(parsed_dtcg.theme.kind, BundleThemeKind::DtcgResolver);
    assert_eq!(
        parsed_dtcg.theme.inputs,
        Some(BTreeMap::from([("appearance".into(), "dark".into())]))
    );
    validate_bundle_plan(&parsed_dtcg).expect("schema 2 DTCG plan");

    for unknown in [
        r#"schema = 1
targets = "modern"
format = "minified"
surprise = true
[theme]
kind = "seed"
[bundles.app]
sources = ["src/app.rs"]
"#,
        r#"schema = 1
targets = "modern"
format = "minified"
[theme]
kind = "seed"
surprise = true
[bundles.app]
sources = ["src/app.rs"]
"#,
        r#"schema = 1
targets = "modern"
format = "minified"
[theme]
kind = "seed"
[bundles.app]
sources = ["src/app.rs"]
surprise = true
"#,
    ] {
        assert!(
            toml::from_str::<BundlePlanDocument>(unknown)
                .expect_err("must reject")
                .to_string()
                .contains("unknown field")
        );
    }

    for name in [
        "",
        "Home",
        "home_visit",
        "home--visit",
        "../home",
        "nul",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        assert!(
            validate_bundle_name(name)
                .expect_err("must reject")
                .contains("unsafe bundle name")
        );
    }
    for name in ["app", "account-settings", "bundle2"] {
        validate_bundle_name(name).expect("safe bundle name");
    }

    let root = temp_dir("bundle-relative-paths");
    assert_eq!(
        resolve_plan_relative_path(&root, Path::new("src/app.rs"), "source")
            .expect("normal relative path"),
        root.join("src/app.rs")
    );
    for path in [
        PathBuf::new(),
        PathBuf::from("."),
        PathBuf::from("src/./app.rs"),
        PathBuf::from("../app.rs"),
        root.join("absolute.rs"),
    ] {
        assert!(
            resolve_plan_relative_path(&root, &path, "source")
                .expect_err("must reject")
                .contains("non-empty relative path")
        );
    }
    fs::remove_dir_all(root).expect("remove temporary directory");
}

#[test]
fn bundle_plan_paths_reject_intermediate_symbolic_links() {
    let root = temp_dir("bundle-intermediate-symlink");
    let plan_dir = root.join("plan");
    let outside = root.join("outside");
    fs::create_dir_all(&plan_dir).expect("create plan directory");
    fs::create_dir_all(&outside).expect("create outside directory");
    write_test_file(
        &outside.join("view.rs"),
        "fn view(){ let _ = pc!(\"flex\"); }",
    );
    assert!(
        ensure_path_within_plan(&plan_dir, &outside.join("view.rs"), "bundle source")
            .expect_err("must reject")
            .contains("resolves outside")
    );
    let link = plan_dir.join("link");

    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).expect("create directory symlink");
    #[cfg(windows)]
    if std::os::windows::fs::symlink_dir(&outside, &link).is_err() {
        fs::remove_dir_all(root).expect("remove temporary directory");
        return;
    }

    let error = resolve_plan_relative_path(
        &plan_dir,
        Path::new("link/view.rs"),
        "bundle `app` source 1",
    )
    .expect_err("must reject");
    assert!(error.contains("traverses symbolic link"));
    fs::remove_dir_all(root).expect("remove temporary directory");
}

#[test]
fn diagnostic_json_has_a_stable_shape_and_retains_typed_style_failures() {
    let failure = compile_styles(&["flec".into()], false).expect_err("must reject");
    assert_eq!(failure.diagnostics.len(), 1);
    let diagnostic = &failure.diagnostics[0];
    assert_eq!(diagnostic.code, "PCS001");
    assert_eq!(diagnostic.category, "style");
    assert_eq!(diagnostic.suggestion.as_deref(), Some("flex"));
    assert_eq!(
        diagnostic.style_range,
        Some(CliByteRange {
            byte_start: 0,
            byte_end: 4,
        })
    );

    let rendered = render_failure_json(Some("check"), &failure);
    assert!(!rendered.contains("Usage:"));
    let json: serde_json::Value = serde_json::from_str(&rendered).expect("diagnostic JSON");
    assert_eq!(json["schemaVersion"], 1);
    assert_eq!(json["command"], "check");
    assert_eq!(json["diagnostics"][0]["code"], "PCS001");
    assert_eq!(
        json_keys(&json),
        ["command", "diagnostics", "schemaVersion"]
    );
    assert_eq!(
        json_keys(&json["diagnostics"][0]),
        [
            "category",
            "code",
            "message",
            "origin",
            "range",
            "replacement",
            "severity",
            "styleRange",
            "suggestion",
        ]
    );

    let arguments = BuildArgs {
        styles: vec!["flex".into(), "flec".into()],
        ..BuildArgs::default()
    };
    let candidates =
        collect_candidates(&ThemeRegistry::seed(), &arguments).expect("collect styles");
    let repeated = compile_candidates(
        &ThemeRegistry::seed(),
        &candidates,
        false,
        TargetContract::Modern,
        CssFormat::Minified,
    )
    .expect_err("must reject");
    assert_eq!(
        repeated.diagnostics[0]
            .origin
            .as_ref()
            .expect("CLI origin")
            .label,
        "explicit-style-2"
    );

    let composition_arguments = BuildArgs {
        compositions: vec![("flex".into(), "flec".into())],
        ..BuildArgs::default()
    };
    let compositions = collect_candidates(&ThemeRegistry::seed(), &composition_arguments)
        .expect("collect composition");
    let composition = compile_candidates(
        &ThemeRegistry::seed(),
        &compositions,
        false,
        TargetContract::Modern,
        CssFormat::Minified,
    )
    .expect_err("must reject");
    assert_eq!(
        composition.diagnostics[0]
            .origin
            .as_ref()
            .expect("composition origin")
            .label,
        "explicit-composition-1:branch-1"
    );

    let formatted = run_utility_formatter(&UtilityFormatArgs {
        styles: vec!["flex".into(), "hover:".into()],
        input: None,
        sources: Vec::new(),
        output: None,
        check: false,
        apply: false,
    })
    .expect_err("must reject");
    assert_eq!(
        formatted.diagnostics[0]
            .origin
            .as_ref()
            .expect("formatter origin")
            .label,
        "fmt-style-2"
    );
}

#[test]
fn parses_check_inspect_and_target_contracts() {
    assert!(matches!(
        parse_arguments(os(&["check", "--style", "flex"])),
        Ok(Command::Check(_))
    ));
    assert!(matches!(
        parse_arguments(os(&["inspect", "--source", "a.rs"])),
        Ok(Command::Inspect(_))
    ));
    assert_eq!(parse_targets("modern"), Ok(TargetContract::Modern));
    assert_eq!(
        parse_targets("baseline-widely"),
        Ok(TargetContract::BaselineWidely)
    );
    assert_eq!(parse_targets("none"), Ok(TargetContract::None));
    assert!(parse_targets("legacy").is_err());
    assert_eq!(parse_format("minified"), Ok(CssFormat::Minified));
    assert_eq!(parse_format("pretty"), Ok(CssFormat::Pretty));
    assert!(parse_format("compact").is_err());
    assert!(parse_arguments(os(&["check", "--style", "flex", "--output", "x.css"])).is_err());
    assert_eq!(
        parse_arguments(os(&["compile", "--help"])),
        Ok(Command::Help)
    );
    for command in ["version", "-V", "--version"] {
        assert_eq!(parse_arguments(os(&[command])), Ok(Command::Version));
    }
    assert!(parse_arguments(os(&["--version", "extra"])).is_err());
    assert!(
        parse_arguments(os(&[
            "compile",
            "--style",
            "flex",
            "--seed",
            "--config",
            "theme.toml"
        ]))
        .is_err()
    );
    assert!(
        parse_arguments(os(&[
            "explain",
            "--style",
            "flex",
            "--targets",
            "none",
            "--targets",
            "modern"
        ]))
        .is_err()
    );
    assert!(
        parse_arguments(os(&[
            "explain", "--style", "flex", "--format", "text", "--format", "json"
        ]))
        .is_err()
    );
}

#[test]
fn migration_inventory_requires_one_kind_and_input() {
    let Command::Inventory(kind, input) =
        parse_arguments(os(&["migration-inventory", "tailwind", "src/app.css"]))
            .expect("migration inventory arguments")
    else {
        panic!("migration inventory command")
    };
    assert_eq!(kind, MigrationSourceKind::Tailwind);
    assert_eq!(input, PathBuf::from("src/app.css"));

    for invalid in [
        vec!["migration-inventory", "src/app.css"],
        vec!["migration-inventory", "tailwind"],
        vec!["migration-inventory", "less", "src/app.css"],
        vec!["migration-inventory", "tailwind", "src/app.css", "app.json"],
    ] {
        assert!(parse_arguments(os(&invalid)).is_err(), "{invalid:?}");
    }
}

#[test]
fn migration_project_inventory_requires_one_declaration_file() {
    let Command::InventoryProject(input) = parse_arguments(os(&[
        "migration-project-inventory",
        "migration.project.json",
    ]))
    .expect("migration project inventory arguments") else {
        panic!("migration project inventory command")
    };
    assert_eq!(input, PathBuf::from("migration.project.json"));
    assert!(parse_arguments(os(&["migration-project-inventory"])).is_err());
    assert!(
        parse_arguments(os(&[
            "migration-project-inventory",
            "migration.project.json",
            "output.json"
        ]))
        .is_err()
    );
}

#[test]
fn format_defaults_to_minified_and_is_accepted_by_every_command() {
    let Command::Compile(defaults) =
        parse_arguments(os(&["compile", "--style", "flex"])).expect("compile defaults")
    else {
        panic!("compile command")
    };
    assert_eq!(defaults.format, CssFormat::Minified);

    for command in ["compile", "build", "check", "inspect"] {
        let parsed = parse_arguments(os(&[command, "--style", "flex", "--format", "pretty"]))
            .expect("pretty format is valid");
        let format = match parsed {
            Command::Compile(arguments)
            | Command::Check(arguments)
            | Command::Inspect(arguments) => arguments.format,
            Command::Audit(_)
            | Command::TransformCss(_)
            | Command::Watch(_)
            | Command::Bundle(_)
            | Command::Catalog(_)
            | Command::Compatibility(_)
            | Command::GenericCssUsage { .. }
            | Command::Inventory(_, _)
            | Command::InventoryProject(_)
            | Command::MigrationProjectPlan(_)
            | Command::MigrationSidecarApply { .. }
            | Command::MigrationSidecarRollback { .. }
            | Command::MigrationReplaceApply { .. }
            | Command::MigrationReplaceRollback { .. }
            | Command::MigrationGroupApply { .. }
            | Command::MigrationGroupRollback { .. }
            | Command::Explain(_)
            | Command::ExplainCascade(_)
            | Command::Plan(_)
            | Command::Fix(_)
            | Command::Format(_)
            | Command::Help
            | Command::Version => {
                panic!("style command expected")
            }
        };
        assert_eq!(format, CssFormat::Pretty, "command {command}");
    }

    let Command::Watch(watch) = parse_arguments(os(&[
        "watch",
        "--input",
        "styles.txt",
        "--output",
        "app.css",
        "--format",
        "pretty",
    ]))
    .expect("watch accepts pretty") else {
        panic!("watch command")
    };
    assert_eq!(watch.format, CssFormat::Pretty);
}

#[test]
fn parses_closed_standard_css_transform_command() {
    assert_eq!(
        parse_arguments(os(&[
            "transform-css",
            "--input",
            "src/app.css",
            "--output",
            "dist/app.css",
            "--targets",
            "modern",
            "--format",
            "pretty",
            "--control-dir",
            "dist/control",
        ])),
        Ok(Command::TransformCss(StandardCssTransformArgs {
            input: PathBuf::from("src/app.css"),
            output: PathBuf::from("dist/app.css"),
            targets: TargetContract::Modern,
            format: CssFormat::Pretty,
            check: false,
            control_dir: Some(PathBuf::from("dist/control")),
        }))
    );
    assert!(parse_arguments(os(&["transform-css", "--input", "app.css"])).is_err());
}

#[test]
fn parses_catalog_defaults_formats_and_exclusive_paths() {
    let Command::Catalog(defaults) = parse_arguments(os(&["catalog"])).expect("catalog defaults")
    else {
        panic!("catalog command")
    };
    assert_eq!(defaults, CatalogArgs::default());

    let Command::Catalog(explicit) = parse_arguments(os(&[
        "catalog",
        "--config",
        "theme.toml",
        "--format",
        "json",
        "--check",
        "catalog.json",
    ]))
    .expect("explicit catalog options") else {
        panic!("catalog command")
    };
    assert_eq!(explicit.config, Some(PathBuf::from("theme.toml")));
    assert_eq!(explicit.format, CatalogFormat::Json);
    assert_eq!(explicit.check, Some(PathBuf::from("catalog.json")));
    assert!(
        parse_arguments(os(&["catalog", "--format", "html"]))
            .expect_err("must reject")
            .contains("markdown` or `json")
    );
    assert!(
        parse_arguments(os(&[
            "catalog",
            "--output",
            "catalog.md",
            "--check",
            "catalog.md",
        ]))
        .expect_err("must reject")
        .contains("`--output` conflicts with `--check`")
    );
    assert!(
        parse_arguments(os(&["catalog", "--seed", "--config", "theme.toml",]))
            .expect_err("must reject")
            .contains("`--seed` conflicts with `--config`")
    );
}

#[test]
fn compatibility_command_requires_one_explicit_profile() {
    assert_eq!(
        parse_arguments(os(&["compatibility", "--targets", "baseline-widely"])),
        Ok(Command::Compatibility(CompatibilityArgs {
            targets: TargetContract::BaselineWidely,
        }))
    );
    for arguments in [
        os(&["compatibility"]),
        os(&["compatibility", "--targets", "modern", "--targets", "none"]),
        os(&["compatibility", "--format", "json"]),
    ] {
        assert!(parse_arguments(arguments).is_err());
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn audit_requires_one_explicit_compatibility_profile() {
    assert_eq!(
        parse_arguments(os(&[
            "audit",
            "--input",
            "src/app.css",
            "--targets",
            "modern",
            "--control-dir",
            "dist/audit",
            "--check",
        ])),
        Ok(Command::Audit(AuditArgs {
            input: AuditInput::Css(PathBuf::from("src/app.css")),
            format: AuditFormat::Human,
            targets: TargetContract::Modern,
            budget_policy: None,
            ownership: None,
            accessibility_policy: None,
            token_graph: None,
            budget_subjects: Vec::new(),
            control_dir: Some(PathBuf::from("dist/audit")),
            check: true,
        }))
    );
    assert_eq!(
        parse_arguments(os(&[
            "audit",
            "--input",
            "src/app.css",
            "--targets",
            "baseline-widely",
            "--format",
            "json"
        ])),
        Ok(Command::Audit(AuditArgs {
            input: AuditInput::Css(PathBuf::from("src/app.css")),
            format: AuditFormat::Json,
            targets: TargetContract::BaselineWidely,
            budget_policy: None,
            ownership: None,
            accessibility_policy: None,
            token_graph: None,
            budget_subjects: Vec::new(),
            control_dir: None,
            check: false,
        }))
    );
    assert_eq!(
        parse_arguments(os(&[
            "audit",
            "--input",
            "dist/route.css",
            "--targets",
            "modern",
            "--budget-policy",
            "config/pliego.budgets.json",
            "--budget-subject",
            "package=app",
            "--budget-subject",
            "route=/home",
        ])),
        Ok(Command::Audit(AuditArgs {
            input: AuditInput::Css(PathBuf::from("dist/route.css")),
            format: AuditFormat::Human,
            targets: TargetContract::Modern,
            budget_policy: Some(PathBuf::from("config/pliego.budgets.json")),
            ownership: None,
            accessibility_policy: None,
            token_graph: None,
            budget_subjects: vec![
                BudgetSubject::new(BudgetSubjectKind::Package, "app").expect("package"),
                BudgetSubject::new(BudgetSubjectKind::Route, "/home").expect("route"),
            ],
            control_dir: None,
            check: false,
        }))
    );
    assert_eq!(
        parse_arguments(os(&[
            "audit",
            "--asset-plan",
            "dist/pliego.assets.json",
            "--targets",
            "modern",
            "--budget-policy",
            "config/pliego.budgets.json",
            "--accessibility-policy",
            "config/pliego.accessibility.json",
            "--token-graph",
            "dist/pliego.tokens.json",
            "--control-dir",
            "dist/audit",
            "--check",
        ])),
        Ok(Command::Audit(AuditArgs {
            input: AuditInput::AssetPlan(PathBuf::from("dist/pliego.assets.json")),
            format: AuditFormat::Human,
            targets: TargetContract::Modern,
            budget_policy: Some(PathBuf::from("config/pliego.budgets.json")),
            ownership: None,
            accessibility_policy: Some(PathBuf::from("config/pliego.accessibility.json")),
            token_graph: Some(PathBuf::from("dist/pliego.tokens.json")),
            budget_subjects: Vec::new(),
            control_dir: Some(PathBuf::from("dist/audit")),
            check: true,
        }))
    );
    assert_eq!(
        parse_arguments(os(&[
            "audit",
            "--asset-plan",
            "dist/pliego.assets.json",
            "--ownership",
            "config/pliego.ownership.json",
            "--targets",
            "modern",
            "--budget-policy",
            "config/pliego.budgets.json",
        ])),
        Ok(Command::Audit(AuditArgs {
            input: AuditInput::AssetPlan(PathBuf::from("dist/pliego.assets.json")),
            format: AuditFormat::Human,
            targets: TargetContract::Modern,
            budget_policy: Some(PathBuf::from("config/pliego.budgets.json")),
            ownership: Some(PathBuf::from("config/pliego.ownership.json")),
            accessibility_policy: None,
            token_graph: None,
            budget_subjects: Vec::new(),
            control_dir: None,
            check: false,
        }))
    );
    for arguments in [
        os(&["audit", "--input", "src/app.css"]),
        os(&[
            "audit",
            "--input",
            "src/app.css",
            "--targets",
            "modern",
            "--targets",
            "none",
        ]),
        os(&[
            "audit",
            "--input",
            "src/app.css",
            "--targets",
            "modern",
            "--budget-subject",
            "package=app",
        ]),
        os(&[
            "audit",
            "--input",
            "src/app.css",
            "--targets",
            "modern",
            "--budget-policy",
            "one.json",
            "--budget-policy",
            "two.json",
        ]),
        os(&[
            "audit",
            "--input",
            "src/app.css",
            "--targets",
            "modern",
            "--budget-policy",
            "budgets.json",
            "--budget-subject",
            "layer=app",
        ]),
        os(&[
            "audit",
            "--input",
            "src/app.css",
            "--asset-plan",
            "dist/pliego.assets.json",
            "--targets",
            "modern",
        ]),
        os(&[
            "audit",
            "--asset-plan",
            "dist/pliego.assets.json",
            "--targets",
            "modern",
            "--budget-policy",
            "budgets.json",
            "--budget-subject",
            "package=app",
        ]),
        os(&[
            "audit",
            "--asset-plan",
            "dist/pliego.assets.json",
            "--targets",
            "modern",
            "--budget-policy",
            "budgets.json",
            "--budget-subject",
            "route=/home",
        ]),
        os(&[
            "audit",
            "--input",
            "src/app.css",
            "--targets",
            "modern",
            "--ownership",
            "ownership.json",
        ]),
        os(&[
            "audit",
            "--asset-plan",
            "dist/pliego.assets.json",
            "--targets",
            "modern",
            "--ownership",
            "one.json",
            "--ownership",
            "two.json",
        ]),
        os(&[
            "audit",
            "--asset-plan",
            "dist/pliego.assets.json",
            "--targets",
            "modern",
            "--budget-policy",
            "budgets.json",
            "--ownership",
            "ownership.json",
            "--budget-subject",
            "package=app",
        ]),
        os(&[
            "audit",
            "--input",
            "src/app.css",
            "--targets",
            "modern",
            "--check",
        ]),
        os(&[
            "audit",
            "--input",
            "src/app.css",
            "--targets",
            "modern",
            "--token-graph",
            "dist/pliego.tokens.json",
        ]),
    ] {
        assert!(parse_arguments(arguments).is_err());
    }
}

#[test]
fn asset_plan_budget_unavailability_is_a_canonical_error_finding() {
    let policy = parse_budget_policy(
        br#"{
          "schemaVersion":1,
          "policyVersion":1,
          "budgets":[{
            "id":"home-route",
            "subject":{"kind":"route","id":"/"},
            "limits":{"bytes":{"maximum":100,"baseline":null,"maxIncrease":null}}
          }]
        }"#,
    )
    .expect("policy");
    let source = FindingSource::new("dist/home.css", 0, 12).expect("source");
    let finding = unavailable_budget_finding(&policy, "home", source).expect("finding");
    let document = FindingDocument::new(
        FindingTool::new("pliegocss", "0.0.0").expect("tool"),
        "audit",
        vec![finding],
    )
    .expect("document");
    let value: serde_json::Value =
        serde_json::from_str(&document.to_json_pretty().expect("JSON")).expect("document JSON");
    assert_eq!(value["findings"][0]["code"], "PCSS-BUDGET-198");
    assert_eq!(
        value["findings"][0]["context"]["budget-policy-version"],
        "1"
    );
}

#[test]
fn sarif_preserves_rich_finding_and_generated_source_map() {
    use pliego_css_build::artifacts::{
        FindingEvidence, FindingException, FindingRisk, FindingSourceMap, FindingSuggestion,
    };

    let source = FindingSource::new("src/component style.css", 2, 8)
        .expect("source")
        .with_position(2, 3, 2, 9)
        .expect("position")
        .with_source_map(FindingSourceMap::new("dist/component.css", 10, 16).expect("source map"));
    let finding = Finding::new(
        "PCSS-A11Y-001",
        "accessibility",
        FindingSeverity::Warning,
        "manual interaction evidence is required",
        FindingVerification::ManualRequired,
        FindingCause::new(
            "accessibility.focus",
            "manual-state",
            "static CSS does not prove the rendered focus state",
        )
        .expect("cause"),
    )
    .and_then(|finding| finding.with_source(source))
    .and_then(|finding| finding.with_context("theme", "dark"))
    .and_then(|finding| {
        finding.with_evidence(
            FindingEvidence::new("ratio", "contrast", "3.42", "static-analysis")
                .and_then(|item| item.with_unit("ratio"))?,
        )
    })
    .and_then(|finding| {
        finding.with_suggestion(FindingSuggestion::new(
            1,
            "review-focus-state",
            "capture the keyboard focus state in a real browser",
            FindingRisk::Low,
            "selector",
        )?)
    })
    .and_then(|finding| {
        finding.with_exception(FindingException::new(
            "manual-review-window",
            "browser evidence is scheduled before release",
        )?)
    })
    .expect("finding");
    let source_less = Finding::new(
        "PCSS-TOOL-001",
        "tooling",
        FindingSeverity::Info,
        "repository-wide evidence has no single source span",
        FindingVerification::Verified,
        FindingCause::new(
            "tooling.repository",
            "global-evidence",
            "the evidence applies to the complete repository",
        )
        .expect("cause"),
    )
    .expect("source-less finding");
    let document = FindingDocument::new(
        FindingTool::new("pliegocss", "0.0.0").expect("tool"),
        "audit",
        vec![source_less, finding],
    )
    .expect("document");
    let canonical: serde_json::Value =
        serde_json::from_str(&document.to_json_pretty().expect("JSON")).expect("finding document");
    let sarif: serde_json::Value =
        serde_json::from_str(&render_sarif(&document).expect("SARIF")).expect("SARIF document");
    let result = &sarif["runs"][0]["results"][0];
    assert_eq!(
        result["properties"]["pliegoCssFinding"],
        canonical["findings"][0]
    );
    assert_eq!(result["level"], "warning");
    assert_eq!(
        result["relatedLocations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "dist/component.css"
    );
    assert!(sarif["runs"][0]["results"][1].get("locations").is_none());
}

#[test]
fn explain_parses_and_returns_theme_scoped_hover_metadata() {
    assert_eq!(
        parse_arguments(os(&[
            "explain",
            "--style",
            "hover:bg-accent/50 -mt-4",
            "--seed",
            "--targets",
            "none",
            "--format",
            "json"
        ])),
        Ok(Command::Explain(ExplainArgs {
            style: "hover:bg-accent/50 -mt-4".into(),
            config: None,
            seed: true,
            targets: TargetContract::None,
            format: ExplainFormat::Json,
        }))
    );
    assert!(parse_arguments(os(&["explain"])).is_err());
    assert!(
        parse_arguments(os(&[
            "explain",
            "--style",
            "flex",
            "--seed",
            "--config",
            "theme.toml"
        ]))
        .is_err()
    );

    let document = build_explanation(&ExplainArgs {
        style: " hover:bg-accent/50   -mt-4 [mask-type:luminance] ".into(),
        config: None,
        seed: true,
        targets: TargetContract::Modern,
        format: ExplainFormat::Json,
    })
    .expect("build explanation");
    assert_eq!(
        document.canonical,
        "hover:bg-accent/50 -mt-4 [mask-type:luminance]"
    );
    assert_eq!(document.utilities.len(), 3);
    assert_eq!(document.utilities[0].pattern, "bg-{color}");
    assert_eq!(document.utilities[0].match_name, "bg");
    assert_eq!(document.utilities[0].source, "hover:bg-accent/50");
    assert_eq!(document.utilities[0].byte_start, 1);
    assert_eq!(document.utilities[0].byte_end, 19);
    assert!(document.utilities[0].capabilities.modifier);
    assert_eq!(document.utilities[1].pattern, "mt-{value}");
    assert!(document.utilities[1].capabilities.negative);
    assert_eq!(document.utilities[2].form, "arbitrary-property");
    assert!(document.css.contains(":hover"));
    assert_eq!(document.style_id.len(), 32);
    assert!(document.class_name.starts_with("pc_"));
    let json = serde_json::to_value(&document).expect("serialize explanation");
    assert_eq!(json["schemaVersion"], 2);
    assert_eq!(json["styleIdFormatVersion"], STYLE_ID_FORMAT_VERSION);
    assert_eq!(json["classNameFormatVersion"], CLASS_NAME_FORMAT_VERSION);
    assert_eq!(json["themeIdFormatVersion"], THEME_ID_FORMAT_VERSION);
    assert_eq!(json["utilities"][0]["matchName"], "bg");
    assert_eq!(
        json_keys(&json),
        [
            "canonical",
            "className",
            "classNameFormatVersion",
            "css",
            "schemaVersion",
            "source",
            "styleId",
            "styleIdFormatVersion",
            "targets",
            "themeId",
            "themeIdFormatVersion",
            "utilities",
        ]
    );
    assert_eq!(
        json_keys(&json["utilities"][0]),
        [
            "byteEnd",
            "byteStart",
            "capabilities",
            "domain",
            "form",
            "matchName",
            "pattern",
            "source",
            "summary",
        ]
    );
    assert_eq!(
        json_keys(&json["utilities"][0]["capabilities"]),
        ["arbitraryValue", "customProperty", "modifier", "negative"]
    );
}

#[test]
fn explain_cascade_requires_a_complete_single_file_query() {
    assert_eq!(
        parse_arguments(os(&[
            "explain-cascade",
            "--input",
            "app.css",
            "--element",
            "button#save.action",
            "--property",
            "color",
            "--format",
            "json",
        ])),
        Ok(Command::ExplainCascade(CascadeExplainArgs {
            input: PathBuf::from("app.css"),
            element: "button#save.action".into(),
            property: "color".into(),
            format: ExplainFormat::Json,
        }))
    );
    for arguments in [
        vec![
            "explain-cascade",
            "--element",
            "button",
            "--property",
            "color",
        ],
        vec![
            "explain-cascade",
            "--input",
            "app.css",
            "--property",
            "color",
        ],
        vec![
            "explain-cascade",
            "--input",
            "app.css",
            "--element",
            "button",
        ],
    ] {
        assert!(parse_arguments(os(&arguments)).is_err());
    }
}

#[test]
fn repair_plan_and_fix_are_closed_over_dry_run_and_apply_inputs() {
    assert_eq!(
        parse_arguments(os(&[
            "plan",
            "--findings",
            "findings.json",
            "--proposal",
            "proposal.json",
            "--source-root",
            "src",
        ])),
        Ok(Command::Plan(RepairPlanCliArgs {
            findings: PathBuf::from("findings.json"),
            proposal: PathBuf::from("proposal.json"),
            source_root: PathBuf::from("src"),
            format: RepairCliFormat::Json,
        }))
    );
    assert_eq!(
        parse_arguments(os(&[
            "fix",
            "--plan",
            "pliego.css.plan.json",
            "--findings",
            "findings.json",
            "--source-root",
            ".",
            "--dry-run",
            "--format",
            "json",
        ])),
        Ok(Command::Fix(RepairFixCliArgs {
            plan: PathBuf::from("pliego.css.plan.json"),
            findings: PathBuf::from("findings.json"),
            source_root: PathBuf::from("."),
            mode: RepairFixMode::DryRun,
            format: RepairCliFormat::Json,
        }))
    );
    assert_eq!(
        parse_arguments(os(&[
            "fix",
            "--plan",
            "pliego.css.plan.json",
            "--findings",
            "findings.json",
            "--source-root",
            ".",
            "--apply",
            "--authorize",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--receipt",
            "pliego.css.change-receipt.json",
        ])),
        Ok(Command::Fix(RepairFixCliArgs {
            plan: PathBuf::from("pliego.css.plan.json"),
            findings: PathBuf::from("findings.json"),
            source_root: PathBuf::from("."),
            mode: RepairFixMode::Apply {
                authorization:
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                receipt: PathBuf::from("pliego.css.change-receipt.json"),
            },
            format: RepairCliFormat::Text,
        }))
    );
    for arguments in [
        vec!["plan", "--proposal", "proposal.json", "--source-root", "."],
        vec!["fix", "--plan", "plan.json", "--source-root", "."],
        vec![
            "fix",
            "--plan",
            "plan.json",
            "--findings",
            "findings.json",
            "--source-root",
            ".",
            "--dry-run",
            "--apply",
        ],
        vec![
            "fix",
            "--plan",
            "plan.json",
            "--findings",
            "findings.json",
            "--source-root",
            ".",
            "--apply",
            "--authorize",
            "sha256:abc",
        ],
    ] {
        assert!(parse_arguments(os(&arguments)).is_err());
    }
}

#[test]
fn parses_formatter_inputs_and_exclusive_output_modes() {
    assert_eq!(
        parse_arguments(os(&["fmt", "--style", "flex   gap-4", "--check"])),
        Ok(Command::Format(UtilityFormatArgs {
            styles: vec!["flex   gap-4".into()],
            input: None,
            sources: Vec::new(),
            output: None,
            check: true,
            apply: false,
        }))
    );
    assert_eq!(
        parse_arguments(os(&[
            "fmt",
            "--input",
            "styles.txt",
            "--output",
            "formatted.txt"
        ])),
        Ok(Command::Format(UtilityFormatArgs {
            styles: Vec::new(),
            input: Some(PathBuf::from("styles.txt")),
            sources: Vec::new(),
            output: Some(PathBuf::from("formatted.txt")),
            check: false,
            apply: false,
        }))
    );
    assert!(parse_arguments(os(&["fmt", "--style", "flex", "--input", "x"])).is_err());
    assert!(parse_arguments(os(&["fmt", "--style", "flex", "--check", "--output", "x"])).is_err());
    assert_eq!(
        parse_arguments(os(&[
            "fmt", "--source", "src", "--source", "tests", "--check"
        ])),
        Ok(Command::Format(UtilityFormatArgs {
            styles: Vec::new(),
            input: None,
            sources: vec![PathBuf::from("src"), PathBuf::from("tests")],
            output: None,
            check: true,
            apply: false,
        }))
    );
    assert_eq!(
        parse_arguments(os(&["fmt", "--source", "src", "--apply"])),
        Ok(Command::Format(UtilityFormatArgs {
            styles: Vec::new(),
            input: None,
            sources: vec![PathBuf::from("src")],
            output: None,
            check: false,
            apply: true,
        }))
    );
    assert!(parse_arguments(os(&["fmt", "--source", "src"])).is_err());
    assert!(parse_arguments(os(&["fmt", "--source", "src", "--check", "--apply"])).is_err());
    assert!(parse_arguments(os(&["fmt", "--style", "flex", "--apply"])).is_err());
    assert!(parse_arguments(os(&["fmt"])).is_err());
}

#[test]
fn formatter_is_idempotent_and_check_detects_line_document_drift() {
    assert_eq!(
        format_utility_source("  dark:hover:-mt-[2rem]!   [&>p]:block  ")
            .expect("format utility source"),
        "dark:hover:-mt-[2rem]! [&>p]:block"
    );

    let directory = temp_dir("utility-format");
    let input = directory.join("styles.txt");
    let output = directory.join("formatted.txt");
    fs::write(
        &input,
        "  # layout  \r\nflex    gap-4\r\n\r\n hover:bg-accent/50 \r\n",
    )
    .expect("write unformatted input");
    let check = UtilityFormatArgs {
        input: Some(input.clone()),
        check: true,
        ..UtilityFormatArgs::default()
    };
    assert!(
        run_utility_formatter(&check)
            .expect_err("must reject")
            .contains("formatting drift")
    );

    run_utility_formatter(&UtilityFormatArgs {
        input: Some(input.clone()),
        output: Some(output.clone()),
        ..UtilityFormatArgs::default()
    })
    .expect("format document");
    let canonical = "# layout\nflex gap-4\n\nhover:bg-accent/50\n";
    assert_eq!(
        fs::read_to_string(&output).expect("read formatted output"),
        canonical
    );
    fs::write(&input, canonical).expect("replace input with canonical document");
    run_utility_formatter(&check).expect("canonical document passes check");
    assert_eq!(
        format_line_document(&input, canonical).expect("format canonical document"),
        canonical
    );

    let escaped_edge = "content-['edge\\ ']\n";
    assert_eq!(
        format_line_document(&input, escaped_edge).expect("preserve escaped edge whitespace"),
        escaped_edge
    );
    let escaped_candidates =
        candidates_from_line_source(&input, escaped_edge).expect("scan escaped edge whitespace");
    assert_eq!(
        escaped_candidates[0].provenance().source,
        "content-['edge\\ ']"
    );

    let carriage_returns = "flex\rgap-4\r";
    assert!(
        format_line_document(&input, carriage_returns)
            .expect_err("must reject")
            .contains("isolated carriage return")
    );
    assert!(
        candidates_from_line_source(&input, carriage_returns)
            .expect_err("must reject")
            .contains("isolated carriage return")
    );

    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn source_formatter_reports_pc_and_each_pcx_literal_without_rewriting_rust() {
    let directory = temp_dir("rust-utility-format");
    let source = directory.join("view.rs");
    let unformatted = r#"fn view(active: bool) {
    let _ = pc!(" flex   gap-4 ");
    let _ = pcx!("grid  gap-4", if active { " block " } else { "hidden" });
}"#;
    fs::write(&source, unformatted).expect("write unformatted Rust source");

    let error =
        run_rust_utility_format(std::slice::from_ref(&source), false).expect_err("must reject");
    assert!(error.contains("formatting drift detected in 3 Rust utility literal(s)"));
    assert!(error.contains("FMT001: pc at"));
    assert!(error.contains("FMT001: pcx base at"));
    assert!(error.contains("FMT001: pcx clause 1 branch 1 at"));
    assert!(error.contains(&source.display().to_string()));
    assert_eq!(error.diagnostics.len(), 3);
    assert!(
        error
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "FMT001")
    );
    assert_eq!(
        error.diagnostics[0]
            .replacement
            .as_ref()
            .expect("decoded replacement")
            .kind,
        "decoded-style-value"
    );
    assert_eq!(
        fs::read_to_string(&source).expect("source is read-only"),
        unformatted
    );

    run_rust_utility_format(std::slice::from_ref(&source), true).expect("apply source fixes");
    assert_eq!(
        fs::read_to_string(&source).expect("formatted source"),
        r#"fn view(active: bool) {
    let _ = pc!("flex gap-4");
    let _ = pcx!("grid gap-4", if active { "block" } else { "hidden" });
}"#
    );
    run_rust_utility_format(&[source], false).expect("formatted Rust source passes");

    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn source_formatter_rejects_stale_snapshots_before_any_publication() {
    let directory = temp_dir("rust-utility-format-stale");
    let first = directory.join("first.rs");
    let second = directory.join("second.rs");
    fs::write(&first, "old first").expect("write first source");
    fs::write(&second, "changed second").expect("write second source");
    let rewrites = vec![
        pliego_css_source::UtilityFormatRewrite {
            path: first.clone(),
            before: b"old first".to_vec(),
            after: b"new first".to_vec(),
        },
        pliego_css_source::UtilityFormatRewrite {
            path: second.clone(),
            before: b"old second".to_vec(),
            after: b"new second".to_vec(),
        },
    ];
    let error = publish_utility_rewrites(&rewrites).expect_err("stale snapshot must fail");
    assert!(error.contains("changed"));
    assert_eq!(fs::read(&first).expect("first source"), b"old first");
    assert_eq!(fs::read(&second).expect("second source"), b"changed second");
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
#[allow(clippy::too_many_lines)]
fn catalog_render_is_deterministic_complete_and_uses_real_theme_css() {
    let directory = temp_dir("catalog-render");
    let config = directory.join("theme.toml");
    fs::write(
        &config,
        "schema=1\nextends='seed'\n[tokens.color]\naccent='#123456'\n",
    )
    .expect("write custom theme");
    let theme = load_theme(Some(&config)).expect("custom theme");
    let json = render_catalog(&theme, CatalogOutputFormat::Json).expect("JSON catalog");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("catalog JSON");
    let utilities = parsed["utilities"].as_array().expect("utilities");
    assert_eq!(utilities.len(), utility_catalog().len());
    assert!(
        utilities
            .windows(2)
            .all(|pair| pair[0]["pattern"].as_str() <= pair[1]["pattern"].as_str())
    );
    assert!(utilities.iter().all(|utility| {
        utility["css"]
            .as_str()
            .is_some_and(|css| !css.is_empty() && css.contains(".pc_"))
    }));
    let background = utilities
        .iter()
        .find(|utility| utility["pattern"] == "bg-{color}")
        .expect("background family");
    assert_eq!(background["example"], "bg-accent/20");
    assert!(
        background["css"]
            .as_str()
            .is_some_and(|css| css.contains("var(--color-accent)"))
    );
    assert!(parsed["theme"]["tokens"].as_array().is_some_and(|tokens| {
        tokens.iter().any(|token| {
            token["kind"] == "Color" && token["name"] == "accent" && token["value"] == "#123456"
        })
    }));

    let markdown = render_catalog(&theme, CatalogOutputFormat::Markdown).expect("Markdown catalog");
    assert_eq!(
        markdown,
        render_catalog(&theme, CatalogOutputFormat::Markdown).expect("same Markdown catalog")
    );
    assert!(markdown.starts_with(
            "<!-- Generated by pliego-cssc catalog. Do not edit manually. -->\n\n# PliegoCSS utility catalog\n"
        ));
    assert!(
        markdown.contains(
            "| Pattern | Match | Form | Domain | Capabilities | Example | CSS | Summary |"
        )
    );
    assert!(markdown.contains("Identity formats: StyleId <code>"));
    assert!(markdown.contains("\n## Tokens\n"));
    assert!(markdown.contains("\n## Breakpoints\n"));
    assert!(markdown.ends_with('\n'));

    assert_eq!(parsed["schemaVersion"], 3);
    assert_eq!(parsed["styleIdFormatVersion"], STYLE_ID_FORMAT_VERSION);
    assert_eq!(parsed["classNameFormatVersion"], CLASS_NAME_FORMAT_VERSION);
    assert_eq!(parsed["themeIdFormatVersion"], THEME_ID_FORMAT_VERSION);
    assert_eq!(
        json_keys(&parsed),
        [
            "classNameFormatVersion",
            "schemaVersion",
            "styleIdFormatVersion",
            "theme",
            "themeIdFormatVersion",
            "utilities",
        ]
    );
    assert_theme_inspection_shape(&parsed["theme"]);
    assert_eq!(
        json_keys(&parsed["utilities"][0]),
        [
            "capabilities",
            "css",
            "domain",
            "example",
            "form",
            "matchName",
            "pattern",
            "summary",
        ]
    );
    assert_eq!(
        json_keys(&parsed["utilities"][0]["capabilities"]),
        ["arbitraryValue", "customProperty", "modifier", "negative"]
    );
    assert_eq!(parsed["theme"]["id"], format!("{:032x}", theme.id().get()));
    assert_eq!(
        parsed["utilities"].as_array().expect("utilities").len(),
        utility_catalog().len()
    );
    assert!(
        parsed["utilities"]
            .as_array()
            .expect("utilities")
            .iter()
            .all(|utility| utility.get("matchName").is_some()
                && utility["form"] != "unknown"
                && utility["domain"] != "unknown")
    );
    assert!(
        parsed["theme"]["tokens"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
    );
    assert!(
        parsed["theme"]["breakpoints"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
    );
    assert!(json.ends_with('\n'));
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn catalog_write_if_changed_check_and_config_protection_are_exact() {
    let directory = temp_dir("catalog-check");
    let catalog = directory.join("catalog.md");
    let rendered = render_catalog(&ThemeRegistry::seed(), CatalogOutputFormat::Markdown)
        .expect("render seed catalog");
    assert!(write_if_changed(&catalog, rendered.as_bytes()).expect("first write"));
    assert!(!write_if_changed(&catalog, rendered.as_bytes()).expect("unchanged write"));
    check_catalog(&catalog, rendered.as_bytes()).expect("matching catalog");
    fs::write(&catalog, b"drift\n").expect("introduce drift");
    assert!(
        check_catalog(&catalog, rendered.as_bytes())
            .expect_err("must reject")
            .contains("catalog drift detected")
    );
    assert!(write_if_changed(&catalog, rendered.as_bytes()).expect("repair drift"));
    check_catalog(&catalog, rendered.as_bytes()).expect("repaired catalog");

    let config = directory.join("pliego.theme.toml");
    fs::write(&config, "schema=1\nextends='seed'\n").expect("write theme config");
    let error = run_catalog(&CatalogArgs {
        config: Some(config.clone()),
        output: Some(config.clone()),
        ..CatalogArgs::default()
    })
    .expect_err("must reject");
    assert!(error.contains("aliases input `resolved theme configuration`"));
    assert_eq!(
        fs::read_to_string(&config).expect("theme remains intact"),
        "schema=1\nextends='seed'\n"
    );
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn output_paths_cannot_alias_each_other_or_any_input() {
    let same_outputs = parse_arguments(os(&[
        "compile",
        "--style",
        "flex",
        "--output",
        "artifact.css",
        "--manifest",
        "./artifact.css",
    ]))
    .expect_err("must reject");
    assert!(same_outputs.contains("aliases `--manifest`"));

    let source_output = parse_arguments(os(&[
        "compile",
        "--source",
        "src/lib.rs",
        "--output",
        "./src/lib.rs",
    ]))
    .expect_err("must reject");
    assert!(source_output.contains("aliases input `--source`"));

    let watch_config = parse_arguments(os(&[
        "watch",
        "--input",
        "styles.txt",
        "--config",
        "theme.toml",
        "--output",
        "./theme.toml",
    ]))
    .expect_err("must reject");
    assert!(watch_config.contains("aliases input `--config`"));
}

#[test]
fn portable_path_keys_reject_ascii_case_only_aliases_on_every_host() {
    let directory = temp_dir("portable-case-alias");
    let input = directory.join("APP.CSS");
    let output = directory.join("app.css");
    fs::write(&input, b"schema=1\nextends='seed'\n").expect("write case-variant input");

    assert_eq!(
        path_key(&input).expect("input key"),
        path_key(&output).expect("output key"),
        "portable keys must model case-insensitive deployment filesystems"
    );
    let error = validate_path_roles(&[("--config", &input)], &[("--output", &output)])
        .expect_err("portable case-only alias must fail");
    assert!(error.contains("aliases input `--config`"));

    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn conventional_theme_is_discovered_near_sources_and_seed_is_explicit() {
    let directory = temp_dir("theme-discovery");
    let first = directory.join("first");
    let second = directory.join("second");
    fs::create_dir_all(first.join("src")).expect("create first package");
    fs::create_dir_all(second.join("src")).expect("create second package");
    fs::write(
        first.join("Cargo.toml"),
        "[package]\nname='first'\nversion='0.0.0'\n",
    )
    .expect("write first manifest");
    fs::write(
        second.join("Cargo.toml"),
        "[package]\nname='second'\nversion='0.0.0'\n",
    )
    .expect("write second manifest");
    fs::write(
        first.join("pliego.theme.toml"),
        "schema=1\nextends='seed'\n",
    )
    .expect("write first theme");
    fs::write(
        second.join("pliego.theme.toml"),
        "schema=1\nextends='seed'\n",
    )
    .expect("write second theme");

    let discovered = resolve_theme_config(
        None,
        false,
        [first.join("src/lib.rs")].iter().map(PathBuf::as_path),
    )
    .expect("discover nearest theme")
    .expect("theme exists");
    assert_eq!(
        path_key(&discovered).unwrap(),
        path_key(&first.join("pliego.theme.toml")).unwrap()
    );
    assert!(
        resolve_theme_config(
            None,
            false,
            [first.join("src"), second.join("src")]
                .iter()
                .map(PathBuf::as_path),
        )
        .expect_err("must reject")
        .contains("multiple `pliego.theme.toml`")
    );
    assert_eq!(
        resolve_theme_config(None, true, [first.join("src")].iter().map(PathBuf::as_path))
            .expect("explicit seed"),
        None
    );
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[cfg(unix)]
#[test]
fn nonexistent_outputs_under_a_symlinked_parent_share_one_physical_key() {
    use std::os::unix::fs::symlink;

    let directory = temp_dir("symlinked-output-parent");
    let real = directory.join("real");
    let linked = directory.join("linked");
    fs::create_dir(&real).expect("create real directory");
    symlink(&real, &linked).expect("create directory symlink");
    let direct = real.join("app.css");
    let through_link = linked.join("app.css");

    assert_eq!(
        path_key(&direct).expect("direct key"),
        path_key(&through_link).expect("symlinked key")
    );
    assert_eq!(
        publication_path_identity(&direct)
            .expect("direct publication key")
            .0,
        publication_path_identity(&through_link)
            .expect("symlinked publication key")
            .0
    );
    let error = validate_path_roles(&[], &[("--output", &direct), ("--manifest", &through_link)])
        .expect_err("must reject");
    assert!(error.contains("aliases"));

    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn auto_discovered_theme_cannot_be_used_as_an_output() {
    let directory = temp_dir("theme-output-alias");
    let source_root = directory.join("src");
    let config = directory.join("pliego.theme.toml");
    fs::create_dir_all(&source_root).expect("create source root");
    fs::write(
        directory.join("Cargo.toml"),
        "[package]\nname='alias'\nversion='0.0.0'\n",
    )
    .expect("write manifest");
    fs::write(&config, "schema=1\nextends='seed'\n").expect("write theme");
    let arguments = BuildArgs {
        sources: vec![source_root],
        output: Some(config.clone()),
        ..BuildArgs::default()
    };
    let error = load_build_theme(&arguments).expect_err("must reject");
    assert!(error.contains("resolved theme configuration"));
    assert_eq!(
        fs::read_to_string(&config).expect("theme remains intact"),
        "schema=1\nextends='seed'\n"
    );
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn custom_token_and_breakpoint_compile_with_active_theme() {
    let directory = temp_dir("theme");
    let config = directory.join("theme.toml");
    fs::write(&config, "schema=1\nextends=\"seed\"\n[tokens.color]\nbrand=\"#369\"\n[breakpoints]\ntablet=\"48rem\"\n").expect("write theme");
    let theme = load_theme(Some(&config)).expect("load theme");
    let artifact = compile_candidates(
        &theme,
        &[Candidate::Direct {
            style: "bg-brand tablet:flex".into(),
            provenance: cli_provenance("bg-brand tablet:flex", "test"),
        }],
        true,
        TargetContract::Modern,
        CssFormat::Minified,
    )
    .expect("custom theme compiles");
    assert!(artifact.css.contains("--color-brand:#369"));
    assert!(artifact.css.contains("width>=48rem"));
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn scanner_compiles_pc_and_every_pcx_composition_semantically() {
    let directory = temp_dir("scan");
    let source = directory.join("view.rs");
    fs::write(
            &source,
            r#"fn view(a: bool, b: bool) { let _ = pc!("flex"); let _ = pcx!("bg-surface", if a { "bg-accent" } else { "bg-transparent" }, if b { "text-white" } else { "text-muted" }); }"#,
        )
        .expect("write Rust source");
    let candidates = candidates_from_rust(&ThemeRegistry::seed(), &source).expect("scan source");
    assert_eq!(candidates.len(), 5);
    let artifact = compile_candidates(
        &ThemeRegistry::seed(),
        &candidates,
        false,
        TargetContract::Modern,
        CssFormat::Minified,
    )
    .expect("compile scan");
    assert_eq!(artifact.styles.len(), 5);
    assert_eq!(
        artifact
            .findings
            .iter()
            .filter(|finding| finding.macro_kind == "pcx")
            .count(),
        4
    );
    assert!(
        artifact
            .findings
            .iter()
            .any(|finding| finding.reason == "reachable-composition[c0:b1,c1:b0]")
    );
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn scanner_diagnostic_is_fatal_with_file_and_range() {
    let directory = temp_dir("scan-error");
    let source = directory.join("bad.rs");
    fs::write(&source, "fn view(dynamic: &str) { let _ = pc!(dynamic); }").expect("write source");
    let error = candidates_from_rust(&ThemeRegistry::seed(), &source).expect_err("must reject");
    assert!(error.contains("PSC001"));
    assert!(error.contains(&source.display().to_string()));
    assert!(error.contains("[bytes"));
    assert_eq!(error.diagnostics.len(), 1);
    assert_eq!(error.diagnostics[0].code, "PSC001");
    let range = error.diagnostics[0].range.as_ref().expect("source range");
    assert_eq!(range.file, source.display().to_string());
    assert!(range.byte_end > range.byte_start);
    assert_eq!(range.start_line, Some(1));
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn rust_parse_failure_is_typed_with_file_and_range() {
    let directory = temp_dir("rust-parse-error");
    let source = directory.join("bad.rs");
    fs::write(&source, "fn {").expect("write invalid Rust source");
    let error = candidates_from_rust(&ThemeRegistry::seed(), &source).expect_err("must reject");
    assert_eq!(error.diagnostics.len(), 1);
    let diagnostic = &error.diagnostics[0];
    assert_eq!(diagnostic.code, "PCR001");
    assert_eq!(diagnostic.category, "source");
    assert_eq!(diagnostic.severity, "error");
    assert_eq!(
        diagnostic
            .origin
            .as_ref()
            .map(|origin| origin.label.as_str()),
        Some("source-scan")
    );
    let range = diagnostic.range.as_ref().expect("source range");
    assert_eq!(range.file, source.display().to_string());
    assert_eq!(range.start_line, Some(1));
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn scanner_json_preserves_multiple_diagnostics_in_source_order() {
    let report = scan_source_named(
        "bad.rs",
        "fn view(a: &str, b: &str) { let _ = pc!(a); let _ = pc!(b); }",
    )
    .expect("valid Rust source");
    let failure =
        candidates_from_scan_report(&ThemeRegistry::seed(), &report).expect_err("must reject");
    assert_eq!(failure.diagnostics.len(), 2);
    assert!(
        failure
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "PSC001")
    );
    let first = failure.diagnostics[0].range.as_ref().expect("first range");
    let second = failure.diagnostics[1].range.as_ref().expect("second range");
    assert!(first.byte_start < second.byte_start);
}

#[test]
fn source_directory_is_lexical_recursive_filtered_and_deduplicated() {
    let directory = temp_dir("source-tree");
    let nested = directory.join("nested");
    let hidden = directory.join(".hidden");
    let target = directory.join("target");
    fs::create_dir_all(&nested).expect("create nested directory");
    fs::create_dir_all(&hidden).expect("create hidden directory");
    fs::create_dir_all(&target).expect("create target directory");
    let first = directory.join("a.rs");
    let second = nested.join("z.rs");
    fs::write(&first, "fn a() { let _ = pc!(\"flex\"); }").expect("write first Rust file");
    fs::write(&second, "fn z() { let _ = pc!(\"grid\"); }").expect("write nested Rust file");
    fs::write(hidden.join("ignored.rs"), "fn x(){pc!(\"block\");}")
        .expect("write hidden Rust file");
    fs::write(target.join("ignored.rs"), "fn x(){pc!(\"hidden\");}")
        .expect("write target Rust file");
    fs::write(directory.join("notes.txt"), "pc!(\"inline\")").expect("write non-Rust file");

    let files = expand_source_paths(&[directory.clone(), first.clone()]).expect("walk sources");
    assert_eq!(files, [first, second]);
    let arguments = BuildArgs {
        sources: vec![directory.clone()],
        ..BuildArgs::default()
    };
    let candidates =
        collect_candidates(&ThemeRegistry::seed(), &arguments).expect("collect source tree");
    let values = candidates
        .iter()
        .map(|candidate| candidate.provenance().source.as_str())
        .collect::<Vec<_>>();
    assert_eq!(values, ["flex", "grid"]);
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn scanner_rejects_cross_clause_conflict_with_shared_pcx003_analysis() {
    let directory = temp_dir("pcx003");
    let source = directory.join("ambiguous.rs");
    fs::write(
            &source,
            r#"fn view(a: bool, b: bool) { let _ = pcx!("flex", if a { "bg-accent" } else { "bg-surface" }, if b { "bg-white" } else { "bg-muted" }); }"#,
        )
        .expect("write ambiguous pcx source");
    let error = candidates_from_rust(&ThemeRegistry::seed(), &source).expect_err("must reject");
    assert!(error.contains("PCX003"));
    assert!(error.contains("independent clauses 1 and 2"));
    assert!(error.contains("BackgroundColor"));
    assert!(error.contains(&source.display().to_string()));
    assert!(error.contains("[bytes"));
    assert_eq!(error.diagnostics.len(), 1);
    assert_eq!(error.diagnostics[0].code, "PCX003");
    assert_eq!(error.diagnostics[0].category, "composition");
    assert!(error.diagnostics[0].range.is_some());
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn manifest_schema_three_and_inspection_two_retain_ordered_deduplicated_origins() {
    let artifact =
        compile_styles(&["flex gap-4".into(), "gap-4 flex".into()], false).expect("compile");
    let reversed =
        compile_styles(&["gap-4 flex".into(), "flex gap-4".into()], false).expect("compile");
    assert_eq!(artifact.styles.len(), 1);
    assert_eq!(artifact.styles[0].origins.len(), 2);
    let json: serde_json::Value = serde_json::from_str(&artifact.manifest).expect("manifest JSON");
    assert_eq!(json["schemaVersion"], 3);
    assert_eq!(json["styleIdFormatVersion"], STYLE_ID_FORMAT_VERSION);
    assert_eq!(json["classNameFormatVersion"], CLASS_NAME_FORMAT_VERSION);
    assert_eq!(json["themeIdFormatVersion"], THEME_ID_FORMAT_VERSION);
    assert_eq!(
        json_keys(&json),
        [
            "classNameFormatVersion",
            "cssBytes",
            "cssSha256",
            "format",
            "schemaVersion",
            "styleIdFormatVersion",
            "styles",
            "targets",
            "themeId",
            "themeIdFormatVersion",
        ]
    );
    assert_manifest_style_shape(&json["styles"][0]);
    assert_eq!(json["targets"], "modern");
    assert_eq!(json["format"], "minified");
    assert_eq!(json["cssBytes"], artifact.css.len());
    assert_eq!(json["cssSha256"], sha256_hex(artifact.css.as_bytes()));
    assert_eq!(json["styles"][0]["origins"][0]["source"], "flex gap-4");
    assert_eq!(json["styles"][0]["origins"][1]["source"], "gap-4 flex");
    assert_eq!(json["themeId"].as_str().expect("theme ID").len(), 32);
    assert_eq!(artifact.manifest, reversed.manifest);
    let inspection =
        serialize_inspection(&ThemeRegistry::seed(), TargetContract::Modern, &artifact)
            .expect("inspection");
    assert_eq!(
        inspection,
        serialize_inspection(&ThemeRegistry::seed(), TargetContract::Modern, &reversed,)
            .expect("reversed inspection")
    );
    let inspection: serde_json::Value = serde_json::from_str(&inspection).expect("inspection JSON");
    assert_eq!(inspection["schemaVersion"], 2);
    assert_eq!(inspection["styleIdFormatVersion"], STYLE_ID_FORMAT_VERSION);
    assert_eq!(
        inspection["classNameFormatVersion"],
        CLASS_NAME_FORMAT_VERSION
    );
    assert_eq!(inspection["themeIdFormatVersion"], THEME_ID_FORMAT_VERSION);
    assert_eq!(
        json_keys(&inspection),
        [
            "classNameFormatVersion",
            "cssBytes",
            "cssSha256",
            "findings",
            "format",
            "schemaVersion",
            "styleIdFormatVersion",
            "styles",
            "targets",
            "theme",
            "themeIdFormatVersion",
        ]
    );
    assert_theme_inspection_shape(&inspection["theme"]);
    assert_manifest_style_shape(&inspection["styles"][0]);
    assert_eq!(
        json_keys(&inspection["findings"][0]),
        [
            "byteEnd",
            "byteStart",
            "file",
            "macroKind",
            "reason",
            "source",
        ]
    );
}

#[test]
fn pretty_profile_is_lightning_printed_and_metadata_matches_exact_bytes() {
    let candidates = [Candidate::Direct {
        style: "flex gap-4".into(),
        provenance: cli_provenance("flex gap-4", "test"),
    }];
    let minified = compile_candidates(
        &ThemeRegistry::seed(),
        &candidates,
        false,
        TargetContract::Modern,
        CssFormat::Minified,
    )
    .expect("minified CSS");
    let pretty = compile_candidates(
        &ThemeRegistry::seed(),
        &candidates,
        false,
        TargetContract::Modern,
        CssFormat::Pretty,
    )
    .expect("pretty CSS");

    assert_eq!(minified.styles, pretty.styles);
    assert_ne!(minified.css, pretty.css);
    assert!(minified.css.contains("{gap:1rem;display:flex}"));
    assert!(pretty.css.contains(" {\n  gap: 1rem;\n  display: flex;\n}"));
    assert!(pretty.css.ends_with('\n'));
    assert_eq!(
        optimize_css(
            "a{color:red}",
            TargetContract::None.lightning(),
            CssFormat::Pretty.is_minified(),
        )
        .expect("Lightning pretty print"),
        "a {\n  color: red;\n}\n"
    );

    let manifest: serde_json::Value =
        serde_json::from_str(&pretty.manifest).expect("pretty manifest");
    assert_eq!(manifest["format"], "pretty");
    assert_eq!(manifest["cssBytes"], pretty.css.len());
    assert_eq!(manifest["cssSha256"], sha256_hex(pretty.css.as_bytes()));

    let inspection: serde_json::Value = serde_json::from_str(
        &serialize_inspection(&ThemeRegistry::seed(), TargetContract::Modern, &pretty)
            .expect("pretty inspection"),
    )
    .expect("inspection JSON");
    assert_eq!(inspection["format"], "pretty");
    assert_eq!(inspection["cssBytes"], pretty.css.len());
    assert_eq!(inspection["cssSha256"], sha256_hex(pretty.css.as_bytes()));
    assert_ne!(
        sha256_hex(minified.css.as_bytes()),
        sha256_hex(pretty.css.as_bytes())
    );
}

#[test]
fn css_transform_and_output_are_target_deterministic() {
    let nested = "a{& b{color:red}}";
    let first = optimize_css(
        nested,
        TargetContract::Modern.lightning(),
        CssFormat::Minified.is_minified(),
    )
    .expect("modern CSS");
    let second = optimize_css(
        nested,
        TargetContract::Modern.lightning(),
        CssFormat::Minified.is_minified(),
    )
    .expect("modern CSS again");
    let none = optimize_css(
        nested,
        TargetContract::None.lightning(),
        CssFormat::Minified.is_minified(),
    )
    .expect("untransformed CSS");
    assert_eq!(first, second);
    assert_eq!(first, "a b{color:red}");
    assert_eq!(none, "a{& b{color:red}}");
}

#[test]
fn only_adjacent_identical_media_rules_are_merged() {
    let adjacent = optimize_css(
        "@media (min-width:48rem){a{display:block}}@media (min-width:48rem){b{display:flex}}",
        TargetContract::None.lightning(),
        CssFormat::Minified.is_minified(),
    )
    .expect("merge adjacent media rules");
    assert_eq!(
        adjacent,
        "@media (width>=48rem){a{display:block}b{display:flex}}"
    );

    let separated = optimize_css(
            "@media (min-width:48rem){a{display:block}}x{color:red}@media (min-width:48rem){b{display:flex}}",
            TargetContract::None.lightning(),
            CssFormat::Minified.is_minified(),
        )
        .expect("retain separated media rules");
    assert_eq!(separated.matches("@media").count(), 2);
    assert!(separated.contains("}x{color:red}@media"));

    let distinct = optimize_css(
        "@media (min-width:40rem){a{display:block}}@media (min-width:48rem){b{display:flex}}",
        TargetContract::None.lightning(),
        CssFormat::Minified.is_minified(),
    )
    .expect("retain distinct media rules");
    assert_eq!(distinct.matches("@media").count(), 2);

    let ordered_lists = optimize_css(
        "@media screen,print{a{display:block}}@media print,screen{b{display:flex}}",
        TargetContract::None.lightning(),
        CssFormat::Minified.is_minified(),
    )
    .expect("retain differently ordered media lists");
    assert_eq!(ordered_lists.matches("@media").count(), 2);

    let cascade = optimize_css(
        "@media (min-width:48rem){a{color:red}}@media (min-width:48rem){a{color:blue}}",
        TargetContract::None.lightning(),
        CssFormat::Minified.is_minified(),
    )
    .expect("retain declaration cascade inside merged media");
    assert!(cascade.contains("a{color:red}a{color:#00f}"));

    let nested = optimize_css(
            "@media (min-width:40rem){@media (prefers-contrast:more){a{color:red}}}@media (min-width:40rem){@media (prefers-contrast:more){b{color:blue}}}",
            TargetContract::None.lightning(),
            CssFormat::Minified.is_minified(),
        )
        .expect("merge media rules exposed by a parent merge");
    assert_eq!(nested.matches("@media").count(), 2);
    assert!(nested.contains("a{color:red}b{color:#00f}"));
}

#[test]
fn merged_media_rules_preserve_physical_trace_in_both_formats() {
    let raw = "@media (min-width:48rem){a{display:block}}@media (min-width:48rem){b{display:flex}}";
    let styles = [
        TraceStyle::new(
            1,
            vec![TraceRule::new(vec![TraceDeclaration::new(
                vec![0],
                false,
                false,
            )])],
        ),
        TraceStyle::new(
            2,
            vec![TraceRule::new(vec![TraceDeclaration::new(
                vec![0],
                false,
                false,
            )])],
        ),
    ];

    for format in [CssFormat::Minified, CssFormat::Pretty] {
        let (css, trace_input) = optimize_css_with_trace(
            raw,
            TargetContract::Modern.lightning(),
            format.is_minified(),
        )
        .expect("optimize traced media rules");
        assert_eq!(css.matches("@media").count(), 1);
        assert_eq!(trace_input.matches("@media").count(), 1);
        build_physical_projection(
            &trace_input,
            &css,
            TargetContract::Modern.lightning(),
            format.is_minified(),
            false,
            &styles,
        )
        .expect("reconcile merged physical trace");
    }
}

#[test]
fn modern_browser_target_versions_are_frozen() {
    let mut modern = TargetContract::None.lightning();
    let modern_browsers = modern.browsers.get_or_insert_with(Default::default);
    modern_browsers.chrome = Some(version(111, 0, 0));
    modern_browsers.edge = Some(version(111, 0, 0));
    modern_browsers.firefox = Some(version(128, 0, 0));
    modern_browsers.safari = Some(version(16, 4, 0));
    assert_eq!(TargetContract::Modern.lightning().browsers, modern.browsers);

    let mut baseline = TargetContract::None.lightning();
    let baseline_browsers = baseline.browsers.get_or_insert_with(Default::default);
    baseline_browsers.chrome = Some(version(120, 0, 0));
    baseline_browsers.edge = Some(version(120, 0, 0));
    baseline_browsers.firefox = Some(version(121, 0, 0));
    baseline_browsers.ios_saf = Some(version(17, 2, 0));
    baseline_browsers.safari = Some(version(17, 2, 0));
    assert_eq!(
        TargetContract::BaselineWidely.lightning().browsers,
        baseline.browsers
    );
    assert!(TargetContract::None.lightning().browsers.is_none());
}

#[test]
fn baseline_profile_fails_closed_for_unclassified_css() {
    let theme = ThemeRegistry::seed();
    let typed = vec![Candidate::Direct {
        style: "flex items-center gap-4".into(),
        provenance: cli_provenance("flex items-center gap-4", "typed"),
    }];
    let artifact = compile_candidates(
        &theme,
        &typed,
        false,
        TargetContract::BaselineWidely,
        CssFormat::Minified,
    )
    .expect("typed baseline style");
    assert!(
        artifact
            .manifest
            .contains("\"targets\": \"baseline-widely\"")
    );

    for style in [
        "rtl:hover:flex",
        "aria-expanded:bg-accent",
        "data-state-open:block",
    ] {
        let candidates = vec![Candidate::Direct {
            style: style.into(),
            provenance: cli_provenance(style, "typed-selector"),
        }];
        compile_candidates(
            &theme,
            &candidates,
            false,
            TargetContract::BaselineWidely,
            CssFormat::Minified,
        )
        .expect("typed selector must satisfy the strict profile");
    }

    for (style, code) in [
        ("w-[13px]", "CMP001"),
        ("[mask-type:luminance]", "CMP002"),
        ("[&>span]:flex", "CMP003"),
    ] {
        let candidates = vec![Candidate::Direct {
            style: style.into(),
            provenance: cli_provenance(style, "strict-compatibility-test"),
        }];
        let error = compile_candidates(
            &theme,
            &candidates,
            false,
            TargetContract::BaselineWidely,
            CssFormat::Minified,
        )
        .expect_err("unclassified CSS must fail");
        assert_eq!(error.diagnostics[0].code, code);
        assert_eq!(error.diagnostics[0].category, "compatibility");
        assert!(error.human.contains(code), "{style}: {}", error.human);
        assert!(error.human.contains("strict-compatibility-test"));
    }
}

#[test]
fn output_is_stable_across_distinct_input_order() {
    let left = compile_styles(&["grid gap-4".into(), "flex gap-4".into()], true).expect("compile");
    let right = compile_styles(&["flex gap-4".into(), "grid gap-4".into()], true).expect("compile");
    assert_eq!(left.css, right.css);
    assert_eq!(left.styles, right.styles);
}

#[test]
fn watch_reacts_to_config_and_keeps_last_valid_artifact() {
    let directory = temp_dir("watch");
    let input = directory.join("styles.txt");
    let output = directory.join("app.css");
    let config = directory.join("theme.toml");
    fs::write(&input, "bg-accent\n").expect("write input");
    fs::write(&config, "schema=1\nextends=\"seed\"\n").expect("write config");
    let arguments = WatchArgs {
        input: Some(input.clone()),
        config: Some(config.clone()),
        theme: true,
        ..watch_args(output.clone())
    };
    let mut cache = RustScanCache::default();
    let first = watch_iteration(&arguments, None, &mut cache);
    let first_snapshot = first.snapshot.clone();
    let WatchOutcome::Compiled(artifact) = first.outcome else {
        panic!("initial compilation")
    };
    write_artifact(&arguments, &first_snapshot, &artifact).expect("write valid artifact");
    let valid = fs::read_to_string(&output).expect("read valid CSS");
    fs::write(&config, "schema=99\nextends=\"seed\"\n").expect("invalidate config");
    let invalid = watch_iteration(&arguments, Some(&first_snapshot), &mut cache);
    assert!(matches!(invalid.outcome, WatchOutcome::Failed(_)));
    assert_eq!(fs::read_to_string(&output).expect("retained CSS"), valid);
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn watch_compiles_selected_dtcg_snapshot_and_rejects_invalid_revisions() {
    let directory = temp_dir("watch-dtcg");
    let input = directory.join("styles.txt");
    let tokens = directory.join("tokens.json");
    fs::write(&input, "bg-brand\n").expect("write input");
    fs::write(&tokens, DTCG_RESOLVER).expect("write resolver");

    let light_arguments = WatchArgs {
        input: Some(input.clone()),
        tokens: Some(tokens.clone()),
        ..watch_args(directory.join("light.css"))
    };
    let mut light_cache = RustScanCache::default();
    let light = watch_iteration(&light_arguments, None, &mut light_cache);
    let WatchOutcome::Compiled(light_artifact) = light.outcome else {
        panic!("default DTCG selection must compile")
    };

    let dark_arguments = WatchArgs {
        input: Some(input),
        tokens: Some(tokens.clone()),
        token_inputs: vec![("appearance".to_owned(), "dark".to_owned())],
        ..watch_args(directory.join("dark.css"))
    };
    let mut dark_cache = RustScanCache::default();
    let dark = watch_iteration(&dark_arguments, None, &mut dark_cache);
    let dark_snapshot = dark.snapshot.clone();
    let WatchOutcome::Compiled(dark_artifact) = dark.outcome else {
        panic!("explicit DTCG selection must compile")
    };
    assert_ne!(light_artifact.css, dark_artifact.css);

    fs::write(&tokens, "{}").expect("invalidate resolver");
    let invalid = watch_iteration(&dark_arguments, Some(&dark_snapshot), &mut dark_cache);
    assert!(matches!(invalid.outcome, WatchOutcome::Failed(_)));
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn watch_reacts_to_rust_tree_and_discovered_theme_changes() {
    let directory = temp_dir("watch-rust");
    let source_root = directory.join("src");
    let source = source_root.join("view.rs");
    let output = directory.join("app.css");
    let config = directory.join("pliego.theme.toml");
    fs::create_dir_all(&source_root).expect("create source root");
    fs::write(
        directory.join("Cargo.toml"),
        "[package]\nname='watch-rust'\nversion='0.0.0'\n",
    )
    .expect("write package manifest");
    fs::write(&config, "schema=1\nextends='seed'\n").expect("write theme");
    fs::write(&source, "fn view(){ let _ = pc!(\"flex\"); }").expect("write source");
    let arguments = WatchArgs {
        sources: vec![source_root.clone()],
        theme: true,
        ..watch_args(output)
    };
    let mut cache = RustScanCache::default();
    let first = watch_iteration(&arguments, None, &mut cache);
    let first_snapshot = first.snapshot.clone();
    let WatchOutcome::Compiled(first_artifact) = first.outcome else {
        panic!("initial source compilation")
    };
    assert!(first_artifact.css.contains("display:flex"));

    fs::write(&source, "fn view(){ let _ = pc!(\"grid\"); }").expect("change source");
    let second = watch_iteration(&arguments, Some(&first_snapshot), &mut cache);
    let second_snapshot = second.snapshot.clone();
    let WatchOutcome::Compiled(second_artifact) = second.outcome else {
        panic!("source change must compile")
    };
    assert!(second_artifact.css.contains("display:grid"));

    fs::write(
        &config,
        "schema=1\nextends='seed'\n[tokens.color]\nbrand='#123456'\n",
    )
    .expect("change discovered theme");
    let third = watch_iteration(&arguments, Some(&second_snapshot), &mut cache);
    assert!(matches!(third.outcome, WatchOutcome::Compiled(_)));
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
#[allow(clippy::too_many_lines)]
fn warm_source_cache_matches_cold_compilation_across_tree_and_theme_mutations() {
    let directory = temp_dir("watch-cache-oracle");
    let source_root = directory.join("src");
    let first_source = source_root.join("a.rs");
    let second_source = source_root.join("b.rs");
    let renamed_source = source_root.join("c.rs");
    let invalid_source = source_root.join("bad.rs");
    let config = directory.join("pliego.theme.toml");
    fs::create_dir_all(&source_root).expect("create source root");
    fs::write(
        directory.join("Cargo.toml"),
        "[package]\nname='watch-cache-oracle'\nversion='0.0.0'\n",
    )
    .expect("write package manifest");
    fs::write(
        &config,
        "schema=1\nextends='seed'\n[tokens.color]\nbrand='#123456'\n",
    )
    .expect("write theme");
    fs::write(&first_source, "fn a(){ let _ = pc!(\"flex\"); }").expect("write first source");

    let arguments = WatchArgs {
        sources: vec![source_root.clone()],
        theme: true,
        manifest: Some(directory.join("app.manifest.json")),
        ..watch_args(directory.join("app.css"))
    };
    let mut cache = RustScanCache::default();

    let initial = warm_matches_cold(&arguments, None, &mut cache);
    assert_eq!(
        (
            initial.cache.scan_hits,
            initial.cache.scan_misses,
            initial.cache.semantic_hits,
            initial.cache.semantic_misses
        ),
        (0, 1, 0, 1)
    );
    assert_eq!(cache.css.0.len(), 1);
    let mut previous = initial.snapshot;

    fs::write(&first_source, "fn a(){ let _ = pc!(\"grid\"); }")
        .expect("edit first source with equal-length utility");
    let edited = warm_matches_cold(&arguments, Some(&previous), &mut cache);
    assert_eq!(
        (
            edited.cache.scan_hits,
            edited.cache.scan_misses,
            edited.cache.semantic_hits,
            edited.cache.semantic_misses
        ),
        (0, 1, 0, 1)
    );
    assert!(
        matches!(&edited.outcome, WatchOutcome::Compiled(artifact) if artifact.css.contains("display:grid") && !artifact.css.contains("display:flex"))
    );
    previous = edited.snapshot;

    fs::write(&second_source, "fn b(){ let _ = pc!(\"bg-brand\"); }").expect("add second source");
    let added = warm_matches_cold(&arguments, Some(&previous), &mut cache);
    assert_eq!(
        (
            added.cache.scan_hits,
            added.cache.scan_misses,
            added.cache.semantic_hits,
            added.cache.semantic_misses
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(cache.css.0.len(), 2);
    previous = added.snapshot;

    fs::write(
        &config,
        "schema=1\nextends='seed'\n[tokens.color]\nbrand='#abcdef'\n",
    )
    .expect("change equal-length theme value");
    let themed = warm_matches_cold(&arguments, Some(&previous), &mut cache);
    assert_eq!(
        (
            themed.cache.scan_hits,
            themed.cache.scan_misses,
            themed.cache.semantic_hits,
            themed.cache.semantic_misses
        ),
        (2, 0, 0, 2)
    );
    assert!(
        matches!(&themed.outcome, WatchOutcome::Compiled(artifact) if artifact.css.contains("--color-brand:#abcdef"))
    );
    previous = themed.snapshot;

    fs::write(&config, "schema=1\nextends='seed'\n").expect("remove custom token");
    let missing_token = warm_matches_cold(&arguments, Some(&previous), &mut cache);
    assert_eq!(
        (
            missing_token.cache.scan_hits,
            missing_token.cache.scan_misses,
            missing_token.cache.semantic_hits,
            missing_token.cache.semantic_misses
        ),
        (2, 0, 0, 2)
    );
    assert!(matches!(missing_token.outcome, WatchOutcome::Failed(_)));
    previous = missing_token.snapshot;

    fs::write(
        &config,
        "schema=1\nextends='seed'\n[tokens.color]\nbrand='#abcdef'\n",
    )
    .expect("restore custom token");
    let restored_token = warm_matches_cold(&arguments, Some(&previous), &mut cache);
    assert_eq!(
        (
            restored_token.cache.scan_hits,
            restored_token.cache.scan_misses,
            restored_token.cache.semantic_hits,
            restored_token.cache.semantic_misses
        ),
        (2, 0, 1, 1)
    );
    assert!(matches!(restored_token.outcome, WatchOutcome::Compiled(_)));
    previous = restored_token.snapshot;

    fs::rename(&second_source, &renamed_source).expect("rename second source");
    let renamed = warm_matches_cold(&arguments, Some(&previous), &mut cache);
    assert_eq!(
        (
            renamed.cache.scan_hits,
            renamed.cache.scan_misses,
            renamed.cache.semantic_hits,
            renamed.cache.semantic_misses,
            renamed.cache.removed
        ),
        (1, 1, 1, 1, 1)
    );
    let WatchOutcome::Compiled(renamed_artifact) = &renamed.outcome else {
        panic!("rename must compile")
    };
    let manifest: serde_json::Value =
        serde_json::from_str(&renamed_artifact.manifest).expect("manifest JSON");
    let origin_files = manifest["styles"]
        .as_array()
        .expect("styles")
        .iter()
        .flat_map(|style| style["origins"].as_array().expect("origins"))
        .filter_map(|origin| origin["file"].as_str())
        .collect::<Vec<_>>();
    assert!(origin_files.contains(&renamed_source.to_str().expect("UTF-8 path")));
    assert!(!origin_files.contains(&second_source.to_str().expect("UTF-8 path")));
    assert_eq!(cache.css.0.len(), 2);
    assert_eq!(cache.css.1.hits(), 2);
    previous = renamed.snapshot;

    fs::remove_file(&first_source).expect("remove first source");
    let removed = warm_matches_cold(&arguments, Some(&previous), &mut cache);
    assert_eq!(
        (
            removed.cache.scan_hits,
            removed.cache.scan_misses,
            removed.cache.semantic_hits,
            removed.cache.semantic_misses,
            removed.cache.removed
        ),
        (1, 0, 1, 0, 1)
    );
    assert!(
        matches!(&removed.outcome, WatchOutcome::Compiled(artifact) if !artifact.css.contains("display:grid"))
    );
    assert_eq!(cache.css.0.len(), 1);
    previous = removed.snapshot;

    fs::write(&invalid_source, "fn bad( {").expect("add invalid Rust source");
    let invalid = warm_matches_cold(&arguments, Some(&previous), &mut cache);
    assert!(matches!(invalid.outcome, WatchOutcome::Failed(_)));
    assert_eq!(
        (
            invalid.cache.scan_hits,
            invalid.cache.scan_misses,
            invalid.cache.semantic_hits,
            invalid.cache.semantic_misses
        ),
        (0, 1, 0, 1)
    );
    previous = invalid.snapshot;

    fs::remove_file(&invalid_source).expect("remove invalid Rust source");
    let recovered = warm_matches_cold(&arguments, Some(&previous), &mut cache);
    assert!(matches!(recovered.outcome, WatchOutcome::Compiled(_)));
    assert_eq!(
        (
            recovered.cache.scan_hits,
            recovered.cache.scan_misses,
            recovered.cache.semantic_hits,
            recovered.cache.semantic_misses,
            recovered.cache.removed
        ),
        (1, 0, 1, 0, 1)
    );

    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn watch_compiles_the_exact_snapshot_without_rereading_mutated_sources() {
    let directory = temp_dir("watch-single-snapshot");
    let source = directory.join("view.rs");
    fs::write(&source, "fn view(){ let _ = pc!(\"flex\"); }").expect("write valid source");
    let arguments = WatchArgs {
        sources: vec![source.clone()],
        seed: true,
        ..watch_args(directory.join("app.css"))
    };

    let snapshot = capture_watch_snapshot(&arguments);
    fs::write(&source, "fn view(){ let _ = pc!(\"flex\"); }")
        .expect("rewrite identical source bytes");
    let mut cache = RustScanCache::default();
    let unchanged = watch_iteration(&arguments, Some(&snapshot), &mut cache);
    assert!(matches!(unchanged.outcome, WatchOutcome::Unchanged));

    fs::write(&source, "fn view( {").expect("mutate source after snapshot");

    let mut stats = SourceCacheStats::default();
    let artifact = compile_watch_snapshot(&arguments, &snapshot, &mut cache, &mut stats)
        .expect("captured valid bytes compile even after the file changes");
    assert!(artifact.css.contains("display:flex"));
    assert_eq!(
        (
            stats.scan_hits,
            stats.scan_misses,
            stats.semantic_hits,
            stats.semantic_misses
        ),
        (0, 1, 0, 1)
    );
    assert_ne!(snapshot, capture_watch_snapshot(&arguments));

    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn watch_requires_two_equal_polls_before_compiling_a_changed_snapshot() {
    let directory = temp_dir("watch-snapshot-confirmation");
    let source = directory.join("view.rs");
    fs::write(&source, "fn view(){ let _ = pc!(\"flex\"); }").expect("write first revision");
    let arguments = WatchArgs {
        sources: vec![source.clone()],
        seed: true,
        ..watch_args(directory.join("app.css"))
    };

    let first = capture_watch_snapshot(&arguments);
    let mut pending = None;
    assert!(!watch_snapshot_confirmed(None, &mut pending, &first));
    assert!(watch_snapshot_confirmed(None, &mut pending, &first));

    fs::write(&source, "fn view(){ let _ = pc!(\"grid\"); }").expect("write transient revision");
    let transient = capture_watch_snapshot(&arguments);
    assert!(!watch_snapshot_confirmed(
        Some(&first),
        &mut pending,
        &transient
    ));

    fs::write(&source, "fn view(){ let _ = pc!(\"block\"); }").expect("replace transient revision");
    let stable = capture_watch_snapshot(&arguments);
    assert!(!watch_snapshot_confirmed(
        Some(&first),
        &mut pending,
        &stable
    ));
    assert!(watch_snapshot_confirmed(
        Some(&first),
        &mut pending,
        &stable
    ));
    assert!(watch_snapshot_confirmed(
        Some(&stable),
        &mut pending,
        &stable
    ));
    assert!(pending.is_none());

    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn failed_four_output_bundle_publication_restores_the_complete_previous_group() {
    let root = temp_dir("bundle-group-rollback");
    let previous = [
        ("global.css", b"old-global-css".as_slice()),
        ("global.manifest.json", b"old-global-manifest".as_slice()),
        ("home.css", b"old-home-css".as_slice()),
        ("home.manifest.json", b"old-home-manifest".as_slice()),
    ];
    for (name, bytes) in previous {
        fs::write(root.join(name), bytes).expect("write previous group output");
    }
    let outputs = [
        ("global.css", b"new-global-css".as_slice()),
        ("global.manifest.json", b"new-global-manifest".as_slice()),
        ("home.css", b"new-home-css".as_slice()),
        ("home.manifest.json", b"new-home-manifest".as_slice()),
    ]
    .into_iter()
    .map(|(name, bytes)| BundleOutputPayload {
        destination: root.join(name),
        bytes: bytes.to_vec(),
    })
    .collect::<Vec<_>>();

    let error = publish_output_group_with(&outputs, |index, temporary, destination| {
        if index == 3 {
            Err(std::io::Error::other("injected fourth publish failure"))
        } else {
            fs::rename(temporary, destination)
        }
    })
    .expect_err("must reject");
    assert!(error.contains("injected fourth publish failure"));
    assert!(error.contains("rollback restored 4 previous output(s)"));
    for (name, bytes) in previous {
        assert_eq!(
            fs::read(root.join(name)).expect("restored group output"),
            bytes
        );
    }
    assert!(
        fs::read_dir(&root)
            .expect("read rollback directory")
            .all(|entry| {
                let path = entry.expect("rollback directory entry").path();
                !path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| {
                        extension.eq_ignore_ascii_case("tmp")
                            || extension.eq_ignore_ascii_case("bak")
                    })
            })
    );

    fs::remove_dir_all(root).expect("remove temporary directory");
}

#[test]
fn output_preparation_failure_keeps_the_last_valid_css() {
    let directory = temp_dir("atomic-output");
    let output = directory.join("app.css");
    let missing_manifest = directory.join("missing/app.json");
    fs::write(&output, "last-valid").expect("write last valid CSS");
    let artifact = compile_styles(&["flex".into()], false).expect("compile artifact");
    let error =
        write_outputs(Some(&output), Some(&missing_manifest), &artifact).expect_err("must reject");
    assert!(error.contains("cannot prepare"));
    assert_eq!(fs::read_to_string(&output).unwrap(), "last-valid");
    assert!(
        fs::read_dir(&directory)
            .expect("read temporary directory")
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp"))
    );
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn failed_second_publication_restores_the_complete_previous_pair() {
    let directory = temp_dir("publication-rollback");
    let output = directory.join("app.css");
    let manifest = directory.join("app.json");
    fs::write(&output, "old-css").expect("write old CSS");
    fs::write(&manifest, "old-manifest").expect("write old manifest");

    let writes = vec![
        prepare_atomic_write(&output, b"new-css").expect("prepare CSS"),
        prepare_atomic_write(&manifest, b"new-manifest").expect("prepare manifest"),
    ];
    let error = commit_prepared_with(writes, |index, temporary, destination| {
        if index == 1 {
            Err(std::io::Error::other("injected second publish failure"))
        } else {
            fs::rename(temporary, destination)
        }
    })
    .expect_err("must reject");

    assert!(error.contains("injected second publish failure"));
    assert!(error.contains("rollback restored 2 previous output(s)"));
    assert_eq!(
        fs::read_to_string(&output).expect("restored CSS"),
        "old-css"
    );
    assert_eq!(
        fs::read_to_string(&manifest).expect("restored manifest"),
        "old-manifest"
    );
    assert!(
        fs::read_dir(&directory)
            .expect("read publication directory")
            .all(|entry| {
                let name = entry.expect("directory entry").file_name();
                let name = name.to_string_lossy();
                !name.contains(".pliego-")
            })
    );

    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn failed_pair_publication_removes_new_destinations_when_no_previous_pair_exists() {
    let directory = temp_dir("publication-new-pair-rollback");
    let output = directory.join("app.css");
    let manifest = directory.join("app.json");
    let writes = vec![
        prepare_atomic_write(&output, b"new-css").expect("prepare CSS"),
        prepare_atomic_write(&manifest, b"new-manifest").expect("prepare manifest"),
    ];

    commit_prepared_with(writes, |index, temporary, destination| {
        if index == 1 {
            Err(std::io::Error::other("injected second publish failure"))
        } else {
            fs::rename(temporary, destination)
        }
    })
    .expect_err("must reject");

    assert!(!output.exists());
    assert!(!manifest.exists());
    assert!(
        fs::read_dir(&directory)
            .expect("read publication directory")
            .all(|entry| !entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .contains(".pliego-"))
    );
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn publication_lock_rejects_a_second_writer_for_the_same_destination() {
    let directory = temp_dir("publication-lock");
    let destination = directory.join("app.css");
    let artifact = compile_styles(&["flex".into()], false).expect("compile artifact");
    fs::write(&destination, &artifact.css).expect("write identical destination");
    let destinations =
        publication_destinations([destination.as_path()]).expect("publication destinations");

    let first = acquire_publication_locks(&destinations).unwrap();
    assert!(
        write_outputs(Some(&destination), None, &artifact)
            .unwrap_err()
            .contains("another writer may be active")
    );
    drop(first);
    acquire_publication_locks(&destinations).unwrap();

    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn portable_publication_key_preserves_the_physical_lock_parent() {
    let directory = temp_dir("publication-lock-parent");
    let parent = directory.join("CaseSensitiveParent");
    fs::create_dir(&parent).expect("create mixed-case publication parent");
    let destination = parent.join("App.CSS");
    let destinations =
        publication_destinations([destination.as_path()]).expect("publication destinations");
    let (key, physical) = destinations.iter().next().expect("one destination");
    assert!(
        key.replace('\\', "/")
            .ends_with("casesensitiveparent/app.css")
    );
    assert!(physical.ends_with(Path::new("CaseSensitiveParent").join("App.CSS")));

    let locks = acquire_publication_locks(&destinations).expect("physical publication lock");
    assert!(parent.join(".App.CSS.pliego.lock").is_file());
    drop(locks);

    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn publication_key_does_not_follow_the_mutable_final_component() {
    let directory = temp_dir("publication-leaf-key");
    let destination = directory.join("app.css");
    let staged = directory.join(".app.css.pliego-1-0.bak");
    fs::write(&staged, "previous CSS").expect("write staged output");

    let expected = publication_path_identity(&destination)
        .expect("destination key")
        .0;
    assert!(expected.ends_with("app.css"));

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&staged, &destination).expect("link final component");
        assert_eq!(
            publication_path_identity(&destination)
                .expect("stable destination key")
                .0,
            expected
        );
        assert_ne!(
            path_key(&destination).expect("canonical final-component key"),
            expected
        );
    }

    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn duplicate_physical_publication_destinations_fail_before_mutation() {
    let directory = temp_dir("duplicate-publication-destination");
    let destination = directory.join("app.css");
    fs::write(&destination, "old").expect("write previous output");
    let writes = vec![
        prepare_atomic_write(&destination, b"first").expect("prepare first"),
        prepare_atomic_write(&destination, b"second").expect("prepare second"),
    ];

    let error = commit_prepared(writes).expect_err("must reject");
    assert!(error.contains("duplicate publication destination"));
    assert_eq!(
        fs::read_to_string(&destination).expect("previous output"),
        "old"
    );
    assert!(
        fs::read_dir(&directory)
            .expect("read publication directory")
            .all(|entry| !entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .contains(".tmp"))
    );
    fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn identical_outputs_are_not_republished() {
    let directory = temp_dir("write-if-changed");
    let output = directory.join("app.css");
    let manifest = directory.join("app.json");
    let first = compile_styles(&["flex gap-4".into()], false).expect("compile first");
    assert_eq!(
        write_outputs(Some(&output), Some(&manifest), &first).expect("first publication"),
        PublishedOutputs {
            css_changed: true,
            manifest_changed: true,
            control_changed: false,
            control_result: ControlPublicationResult::NotRequested,
        }
    );
    assert_eq!(
        write_outputs(Some(&output), Some(&manifest), &first).expect("identical publication"),
        PublishedOutputs::default()
    );

    let reordered =
        compile_styles(&["gap-4 flex".into()], false).expect("compile reordered origin");
    assert_eq!(first.css, reordered.css);
    assert_ne!(first.manifest, reordered.manifest);
    assert_eq!(
        write_outputs(Some(&output), Some(&manifest), &reordered)
            .expect("manifest-only publication"),
        PublishedOutputs {
            css_changed: false,
            manifest_changed: true,
            control_changed: false,
            control_result: ControlPublicationResult::NotRequested,
        }
    );
    fs::remove_dir_all(directory).expect("remove temporary directory");
}
