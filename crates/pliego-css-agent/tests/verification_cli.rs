//! Black-box contract for the closed post-change verification executable.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use pliego_css_agent::{
    RepairTestEvidenceResult, RepairTool, build_repair_plan, parse_repair_proposal,
    parse_repair_test_evidence, prepare_repair_application,
};
use pliego_css_build::artifacts::{
    Finding, FindingCause, FindingDocument, FindingRisk, FindingSeverity, FindingSource,
    FindingSuggestion, FindingTool, FindingVerification, sha256_hex,
};
use pliego_css_config::parse_budget_policy;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn fixture() -> (String, String, BTreeMap<String, Vec<u8>>) {
    let source = b".button { color: #777; }\n".to_vec();
    let start = String::from_utf8(source.clone())
        .unwrap()
        .find("#777")
        .unwrap();
    let finding = Finding::new(
        "A11Y001",
        "accessibility",
        FindingSeverity::Error,
        "declared contrast is below policy",
        FindingVerification::Verified,
        FindingCause::new(
            "accessibility.contrast",
            "minimum-ratio",
            "ratio is too low",
        )
        .unwrap(),
    )
    .unwrap()
    .with_source(FindingSource::new("src/app.css", start, start + 4).unwrap())
    .unwrap()
    .with_suggestion(
        FindingSuggestion::new(
            1,
            "replace-color",
            "use the reviewed replacement",
            FindingRisk::Low,
            "declaration",
        )
        .unwrap()
        .with_prerequisites(vec!["audit".into()])
        .unwrap(),
    )
    .unwrap();
    let document = FindingDocument::new(
        FindingTool::new("pliegocss", "0.0.0").unwrap(),
        "audit",
        vec![finding],
    )
    .unwrap();
    let findings = document.to_json_pretty().unwrap();
    let value: serde_json::Value = serde_json::from_str(&findings).unwrap();
    let fingerprint = value["findings"][0]["fingerprint"].as_str().unwrap();
    let proposal = format!(
        "{{\n  \"schemaVersion\": \"1.0.0\",\n  \"findingDocumentSha256\": \"{}\",\n  \"changeBudget\": {{\n    \"maxFiles\": 1,\n    \"maxEdits\": 1,\n    \"maxInsertedBytes\": 4,\n    \"maxRemovedBytes\": 4\n  }},\n  \"requiredChecks\": [\"audit\"],\n  \"edits\": [\n    {{\n      \"findingFingerprint\": \"{}\",\n      \"suggestionId\": \"replace-color\",\n      \"file\": \"src/app.css\",\n      \"byteStart\": 17,\n      \"byteEnd\": 21,\n      \"replacement\": \"#111\"\n    }}\n  ]\n}}\n",
        sha256_hex(findings.as_bytes()),
        fingerprint
    );
    (
        findings,
        proposal,
        BTreeMap::from([("src/app.css".into(), source)]),
    )
}

fn temp_root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "pliegocss-agent-cli-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(path.join("src")).unwrap();
    path
}

fn agent_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_pliego-css-agent") {
        return PathBuf::from(path);
    }

    let mut path = std::env::current_exe().expect("integration test path must be available");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push(format!("pliego-css-agent{}", std::env::consts::EXE_SUFFIX));
    assert!(
        path.is_file(),
        "Cargo-built pliego-css-agent binary is missing at {}",
        path.display()
    );
    path
}

fn run(root: &Path, policy: &str, receipt: &str) -> std::process::Output {
    Command::new(agent_binary())
        .current_dir(root)
        .args([
            "verify",
            "--change-receipt",
            "change.json",
            "--check-policy",
            policy,
            "--source-root",
            ".",
            "--receipt",
            receipt,
            "--format",
            "json",
        ])
        .output()
        .unwrap()
}

#[test]
fn verification_cli_passes_repeats_and_blocks_browser_requirements() {
    let (findings, proposal, sources) = fixture();
    let proposal = parse_repair_proposal(proposal.as_bytes()).unwrap();
    let plan = build_repair_plan(
        RepairTool::new("pliegocss", "0.0.0").unwrap(),
        "findings.json",
        findings.as_bytes(),
        &proposal,
        &sources,
    )
    .unwrap();
    let authorization = format!("sha256:{}", plan.plan_sha256());
    let prepared = prepare_repair_application(
        &plan,
        "findings.json",
        findings.as_bytes(),
        &sources,
        &authorization,
    )
    .unwrap();
    let root = temp_root();
    fs::write(root.join("src/app.css"), &prepared.sources()["src/app.css"]).unwrap();
    fs::write(
        root.join("change.json"),
        prepared.receipt().to_json_pretty().unwrap(),
    )
    .unwrap();
    let policy = "{\n  \"schemaVersion\": \"1.0.0\",\n  \"checks\": [\n    {\n      \"id\": \"audit\",\n      \"kind\": \"standard-css-audit\",\n      \"source\": \"src/app.css\",\n      \"compatibilityProfile\": \"modern\"\n    }\n  ],\n  \"browserEvidence\": \"not-required\"\n}\n";
    fs::write(root.join("checks.json"), policy).unwrap();
    let first = run(&root, "checks.json", "verified.json");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(value["schemaVersion"], "1.4.0");
    assert_eq!(value["result"], "passed");
    assert_eq!(fs::read(root.join("verified.json")).unwrap(), first.stdout);
    let finding_documents = fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("pliego-css-findings-")
        })
        .collect::<Vec<_>>();
    assert_eq!(finding_documents.len(), 1);
    let finding_bytes = fs::read(finding_documents[0].path()).unwrap();
    assert_eq!(
        finding_bytes.len(),
        value["checks"][0]["findingDocumentBytes"]
    );
    assert_eq!(
        sha256_hex(&finding_bytes),
        value["checks"][0]["findingDocumentSha256"]
    );
    let repeated = run(&root, "checks.json", "verified.json");
    assert!(repeated.status.success());
    assert_eq!(repeated.stdout, first.stdout);

    let browser_policy = policy.replace("not-required", "required");
    fs::write(root.join("browser-checks.json"), browser_policy).unwrap();
    let blocked = run(&root, "browser-checks.json", "blocked.json");
    assert_eq!(blocked.status.code(), Some(1));
    let value: serde_json::Value = serde_json::from_slice(&blocked.stdout).unwrap();
    assert_eq!(value["result"], "blocked");
    assert_eq!(value["browserEvidence"], "required-not-collected");
    assert_eq!(fs::read(root.join("blocked.json")).unwrap(), blocked.stdout);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verification_cli_loads_and_binds_a_budget_policy_below_source_root() {
    let (findings, proposal, sources) = fixture();
    let proposal = proposal.replace(
        "\"requiredChecks\": [\"audit\"]",
        "\"requiredChecks\": [\"audit\", \"budget\"]",
    );
    let proposal = parse_repair_proposal(proposal.as_bytes()).unwrap();
    let plan = build_repair_plan(
        RepairTool::new("pliegocss", "0.0.0").unwrap(),
        "findings.json",
        findings.as_bytes(),
        &proposal,
        &sources,
    )
    .unwrap();
    let authorization = format!("sha256:{}", plan.plan_sha256());
    let prepared = prepare_repair_application(
        &plan,
        "findings.json",
        findings.as_bytes(),
        &sources,
        &authorization,
    )
    .unwrap();
    let root = temp_root();
    fs::create_dir_all(root.join("config")).unwrap();
    fs::write(root.join("src/app.css"), &prepared.sources()["src/app.css"]).unwrap();
    fs::write(
        root.join("change.json"),
        prepared.receipt().to_json_pretty().unwrap(),
    )
    .unwrap();

    let budget_source = br#"{
      "schemaVersion": 1,
      "policyVersion": 1,
      "budgets": [{
        "id": "repaired-file-bytes",
        "subject": {"kind": "file", "id": "src/app.css"},
        "limits": {"bytes": {"maximum": 0, "baseline": null, "maxIncrease": null}}
      }],
      "exceptions": []
    }"#;
    let budget_bytes = parse_budget_policy(budget_source)
        .unwrap()
        .to_json_pretty()
        .unwrap()
        .into_bytes();
    fs::write(root.join("config/css-budgets.json"), &budget_bytes).unwrap();
    let policy = format!(
        "{{\n  \"schemaVersion\": \"1.2.0\",\n  \"checks\": [\n    {{\n      \"id\": \"audit\",\n      \"kind\": \"standard-css-audit\",\n      \"source\": \"src/app.css\",\n      \"compatibilityProfile\": \"modern\"\n    }},\n    {{\n      \"id\": \"budget\",\n      \"kind\": \"css-budget-audit\",\n      \"source\": \"src/app.css\",\n      \"compatibilityProfile\": \"modern\",\n      \"budgetPolicy\": {{\n        \"file\": \"config/css-budgets.json\",\n        \"bytes\": {},\n        \"sha256\": \"{}\"\n      }}\n    }}\n  ],\n  \"browserEvidence\": \"not-required\"\n}}\n",
        budget_bytes.len(),
        sha256_hex(&budget_bytes)
    );
    fs::write(root.join("budget-checks.json"), policy).unwrap();

    let failed = run(&root, "budget-checks.json", "budget-verified.json");
    assert_eq!(failed.status.code(), Some(1));
    let value: serde_json::Value = serde_json::from_slice(&failed.stdout).unwrap();
    assert_eq!(value["schemaVersion"], "1.4.0");
    assert_eq!(value["result"], "failed");
    assert_eq!(value["checks"][1]["kind"], "css-budget-audit");
    assert_eq!(
        value["checks"][1]["budgetPolicy"]["sha256"],
        sha256_hex(&budget_bytes)
    );
    assert_eq!(
        fs::read(root.join("budget-verified.json")).unwrap(),
        failed.stdout
    );

    let mut drifted = budget_bytes;
    drifted.push(b' ');
    fs::write(root.join("config/css-budgets.json"), drifted).unwrap();
    let rejected = run(&root, "budget-checks.json", "budget-drift.json");
    assert_eq!(rejected.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("does not match its policy identity")
    );
    assert!(!root.join("budget-drift.json").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fixed_runner_evidence_reaches_verification_without_a_command_policy_surface() {
    let (findings, proposal, sources) = fixture();
    let proposal = proposal.replace(
        "\"requiredChecks\": [\"audit\"]",
        "\"requiredChecks\": [\"audit\", \"tests.workspace\"]",
    );
    let proposal = parse_repair_proposal(proposal.as_bytes()).unwrap();
    let plan = build_repair_plan(
        RepairTool::new("pliegocss", "0.0.0").unwrap(),
        "findings.json",
        findings.as_bytes(),
        &proposal,
        &sources,
    )
    .unwrap();
    let authorization = format!("sha256:{}", plan.plan_sha256());
    let prepared = prepare_repair_application(
        &plan,
        "findings.json",
        findings.as_bytes(),
        &sources,
        &authorization,
    )
    .unwrap();
    let root = temp_root();
    fs::write(root.join("src/app.css"), &prepared.sources()["src/app.css"]).unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn passes() { assert_eq!(2 + 2, 4); }\n}\n",
    )
    .unwrap();
    let manifest = "[package]\nname = \"pliego-agent-test-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n";
    let lockfile = "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"pliego-agent-test-fixture\"\nversion = \"0.0.0\"\n";
    fs::write(root.join("Cargo.toml"), manifest).unwrap();
    fs::write(root.join("Cargo.lock"), lockfile).unwrap();
    fs::write(
        root.join("change.json"),
        prepared.receipt().to_json_pretty().unwrap(),
    )
    .unwrap();

    let run_tests = Command::new(agent_binary())
        .current_dir(&root)
        .args([
            "run-tests",
            "--change-receipt",
            "change.json",
            "--source-root",
            ".",
            "--check-id",
            "tests.workspace",
            "--evidence",
            "test-evidence.json",
        ])
        .output()
        .unwrap();
    assert!(
        run_tests.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run_tests.stdout),
        String::from_utf8_lossy(&run_tests.stderr)
    );
    let evidence_bytes = fs::read(root.join("test-evidence.json")).unwrap();
    let evidence = parse_repair_test_evidence(&evidence_bytes).unwrap();
    assert_eq!(evidence.result(), RepairTestEvidenceResult::Passed);
    let policy = format!(
        "{{\n  \"schemaVersion\": \"1.3.0\",\n  \"checks\": [\n    {{\n      \"id\": \"audit\",\n      \"kind\": \"standard-css-audit\",\n      \"source\": \"src/app.css\",\n      \"compatibilityProfile\": \"modern\"\n    }},\n    {{\n      \"id\": \"tests.workspace\",\n      \"kind\": \"test-suite-evidence\",\n      \"source\": \"src/app.css\",\n      \"compatibilityProfile\": \"none\",\n      \"testEvidence\": {{\n        \"file\": \"test-evidence.json\",\n        \"bytes\": {},\n        \"sha256\": \"{}\"\n      }},\n      \"workspaceManifest\": {{\n        \"file\": \"Cargo.toml\",\n        \"bytes\": {},\n        \"sha256\": \"{}\"\n      }},\n      \"lockfile\": {{\n        \"file\": \"Cargo.lock\",\n        \"bytes\": {},\n        \"sha256\": \"{}\"\n      }}\n    }}\n  ],\n  \"browserEvidence\": \"not-required\"\n}}\n",
        evidence_bytes.len(),
        sha256_hex(&evidence_bytes),
        manifest.len(),
        sha256_hex(manifest.as_bytes()),
        lockfile.len(),
        sha256_hex(lockfile.as_bytes()),
    );
    fs::write(root.join("test-checks.json"), policy).unwrap();
    let verified = run(&root, "test-checks.json", "test-verified.json");
    assert!(
        verified.status.success(),
        "{}",
        String::from_utf8_lossy(&verified.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&verified.stdout).unwrap();
    assert_eq!(value["result"], "passed");
    assert_eq!(value["checks"][1]["kind"], "test-suite-evidence");
    assert_eq!(
        value["checks"][1]["testEvidence"]["sha256"],
        sha256_hex(&evidence_bytes)
    );
    fs::remove_dir_all(root).unwrap();
}
