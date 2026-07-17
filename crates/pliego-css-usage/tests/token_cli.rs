//! Process contract for the read-only token usage query CLI.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use pliego_css_config::TokenGraph;
use pliego_css_ir::TokenKind;
use pliego_css_theme::ThemeRegistry;
use pliego_css_usage::{TokenUsageConsumerInput, build_token_usage_report};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn query_cli_renders_text_and_json_and_rejects_unknown_tokens() {
    let theme = ThemeRegistry::seed();
    let graph = TokenGraph::from_registry(&theme);
    let accent = theme
        .token_by_name(TokenKind::Color, "accent")
        .expect("seed theme must contain accent");
    let report = build_token_usage_report(
        &graph,
        &BTreeMap::new(),
        &theme,
        [TokenUsageConsumerInput::new(
            "application",
            "0123456789abcdef0123456789abcdef",
            accent.kind,
            accent.id,
        )],
        true,
    )
    .expect("report must build")
    .to_canonical_json()
    .expect("report must encode");
    let path = temporary_path();
    fs::write(&path, report).expect("report fixture must be written");

    let text = run(&path, "color.accent", "text");
    assert!(text.status.success());
    assert!(String::from_utf8_lossy(&text.stdout).contains("status: direct"));
    assert!(text.stderr.is_empty());

    let json = run(&path, "color.accent", "json");
    assert!(json.status.success());
    assert!(String::from_utf8_lossy(&json.stdout).contains("\"status\": \"direct\""));

    let missing = run(&path, "color.absent", "text");
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("matched no token"));
    fs::remove_file(path).expect("report fixture must be removed");
}

fn run(path: &Path, token: &str, format: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pliego-css-tokens"))
        .args([
            "explain",
            "--report",
            path.to_str().expect("temporary path must be UTF-8"),
            "--token",
            token,
            "--format",
            format,
        ])
        .output()
        .expect("query CLI must run")
}

fn temporary_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must follow Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "pliego-token-usage-{}-{timestamp}-{}.json",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}
