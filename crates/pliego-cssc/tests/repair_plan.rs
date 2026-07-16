//! Black-box contract for bounded repair planning and dry-run verification.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use pliego_css_build::artifacts::{
    Finding, FindingCause, FindingDocument, FindingRisk, FindingSeverity, FindingSource,
    FindingSuggestion, FindingTool, FindingVerification, sha256_hex,
};

fn temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("pliegocss-repair-{}-{nonce}", std::process::id()));
    fs::create_dir_all(path.join("src")).expect("temporary project");
    path
}

fn run(directory: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .current_dir(directory)
        .args(arguments)
        .output()
        .expect("pliego-cssc")
}

fn fixture(directory: &Path) -> Vec<u8> {
    let css = b".button { color: #777; }\n".to_vec();
    let start = std::str::from_utf8(&css)
        .expect("CSS")
        .find("#777")
        .expect("literal");
    fs::write(directory.join("src/app.css"), &css).expect("source");
    let finding = Finding::new(
        "A11Y001",
        "accessibility",
        FindingSeverity::Error,
        "declared contrast is below policy",
        FindingVerification::Verified,
        FindingCause::new(
            "accessibility.contrast",
            "minimum-ratio",
            "ratio is below the configured minimum",
        )
        .expect("cause"),
    )
    .expect("finding")
    .with_source(FindingSource::new("src/app.css", start, start + 4).expect("source range"))
    .expect("finding source")
    .with_suggestion(
        FindingSuggestion::new(
            1,
            "replace-color",
            "replace the local literal",
            FindingRisk::Low,
            "declaration",
        )
        .expect("suggestion")
        .with_prerequisites(vec!["audit".into()])
        .expect("prerequisites"),
    )
    .expect("finding suggestion");
    let fingerprint = finding.fingerprint().to_owned();
    let findings = FindingDocument::new(
        FindingTool::new("pliegocss", "0.0.0").expect("tool"),
        "audit",
        vec![finding],
    )
    .expect("finding document")
    .to_json_pretty()
    .expect("finding JSON")
    .into_bytes();
    fs::write(directory.join("findings.json"), &findings).expect("findings");
    let proposal = serde_json::json!({
        "schemaVersion": "1.0.0",
        "findingDocumentSha256": sha256_hex(&findings),
        "changeBudget": {
            "maxFiles": 1,
            "maxEdits": 1,
            "maxInsertedBytes": 4,
            "maxRemovedBytes": 4
        },
        "requiredChecks": ["audit"],
        "edits": [{
            "findingFingerprint": fingerprint,
            "suggestionId": "replace-color",
            "file": "src/app.css",
            "byteStart": start,
            "byteEnd": start + 4,
            "replacement": "#111"
        }]
    });
    fs::write(
        directory.join("proposal.json"),
        serde_json::to_vec_pretty(&proposal).expect("proposal JSON"),
    )
    .expect("proposal");
    css
}

fn run_fix(directory: &Path, findings: &str, json: bool) -> Output {
    let mut arguments = vec![
        "fix",
        "--plan",
        "pliego.css.plan.json",
        "--findings",
        findings,
        "--source-root",
        ".",
        "--dry-run",
    ];
    if json {
        arguments.extend(["--format", "json"]);
    }
    run(directory, &arguments)
}

fn run_apply(directory: &Path, authorization: &str) -> Output {
    run(
        directory,
        &[
            "fix",
            "--plan",
            "pliego.css.plan.json",
            "--findings",
            "findings.json",
            "--source-root",
            ".",
            "--apply",
            "--authorize",
            authorization,
            "--receipt",
            "pliego.css.change-receipt.json",
            "--format",
            "json",
        ],
    )
}

fn assert_plan_contract(bytes: &[u8]) -> serde_json::Value {
    let plan: serde_json::Value = serde_json::from_slice(bytes).expect("plan JSON");
    assert_eq!(plan["schemaVersion"], "1.0.0");
    assert_eq!(plan["mode"], "dry-run-only");
    assert_eq!(plan["risk"], "low");
    assert_eq!(plan["changeSummary"]["files"], 1);
    assert_eq!(plan["changeSummary"]["edits"], 1);
    assert_eq!(plan["changeSummary"]["insertedBytes"], 4);
    assert_eq!(plan["changeSummary"]["removedBytes"], 4);
    assert_eq!(plan["patch"]["format"], "pliegocss-byte-edits/1");
    assert!(
        plan["patch"]["text"]
            .as_str()
            .expect("patch")
            .contains("-\"#777\"\n+\"#111\"")
    );
    plan
}

#[test]
fn plan_and_fix_are_deterministic_authorized_and_idempotence_aware() {
    let directory = temp_dir();
    let original = fixture(&directory);
    let arguments = [
        "plan",
        "--findings",
        "findings.json",
        "--proposal",
        "proposal.json",
        "--source-root",
        ".",
        "--format",
        "json",
    ];
    let first = run(&directory, &arguments);
    let second = run(&directory, &arguments);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(first.stdout, second.stdout);
    let plan = assert_plan_contract(&first.stdout);
    fs::write(directory.join("pliego.css.plan.json"), &first.stdout).expect("plan");
    fs::copy(
        directory.join("findings.json"),
        directory.join("other-findings.json"),
    )
    .expect("finding copy");
    let wrong_finding = run_fix(&directory, "other-findings.json", false);
    assert!(!wrong_finding.status.success());
    assert!(
        String::from_utf8_lossy(&wrong_finding.stderr).contains("finding document does not match")
    );

    let dry_run = run_fix(&directory, "findings.json", true);
    assert!(dry_run.status.success());
    let report: serde_json::Value = serde_json::from_slice(&dry_run.stdout).expect("dry-run JSON");
    assert_eq!(report["state"], "ready");
    assert_eq!(fs::read(directory.join("src/app.css")).unwrap(), original);

    let bad_apply = run_apply(&directory, "sha256:wrong");
    assert!(!bad_apply.status.success());
    assert_eq!(fs::read(directory.join("src/app.css")).unwrap(), original);
    assert!(!directory.join("pliego.css.change-receipt.json").exists());

    let plan_sha256 = plan["planSha256"].as_str().expect("plan hash");
    let authorization = format!("sha256:{plan_sha256}");
    let apply = run_apply(&directory, &authorization);
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&apply.stdout).expect("receipt JSON");
    assert_eq!(receipt["schemaVersion"], "1.0.0");
    assert_eq!(receipt["state"], "applied");
    assert_eq!(receipt["result"], "checks-pending");
    assert_eq!(receipt["checks"][0]["status"], "not-run");
    assert_eq!(receipt["browserEvidence"], "not-collected");
    assert_eq!(
        fs::read(directory.join("src/app.css")).unwrap(),
        b".button { color: #111; }\n"
    );
    let receipt_bytes =
        fs::read(directory.join("pliego.css.change-receipt.json")).expect("receipt");

    let applied = run_fix(&directory, "findings.json", true);
    assert!(applied.status.success());
    let report: serde_json::Value = serde_json::from_slice(&applied.stdout).expect("applied JSON");
    assert_eq!(report["state"], "already-applied");

    let repeated = run_apply(&directory, &authorization);
    assert!(repeated.status.success());
    assert_eq!(repeated.stdout, receipt_bytes);
    assert_eq!(
        fs::read(directory.join("pliego.css.change-receipt.json")).unwrap(),
        receipt_bytes
    );

    fs::write(directory.join("src/app.css"), b".button { color: #999; }\n").expect("stale");
    let stale = run_fix(&directory, "findings.json", false);
    assert!(!stale.status.success());
    let stale_apply = run_apply(&directory, &authorization);
    assert!(!stale_apply.status.success());
    assert_eq!(
        fs::read(directory.join("pliego.css.change-receipt.json")).unwrap(),
        receipt_bytes
    );
    assert!(String::from_utf8_lossy(&stale.stderr).contains("matches neither"));
    fs::remove_dir_all(directory).expect("remove fixture");
}
