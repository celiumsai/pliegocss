//! Black-box contract for standards-first CSS audit.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use pliego_css_config::parse_dtcg_resolver_str;
use pliego_css_control::{parse_build_receipt, parse_control_manifest};

const PASSING_BUDGET_POLICY: &[u8] = br#"{
  "schemaVersion": 1,
  "policyVersion": 1,
  "budgets": [{
    "id": "app-rules",
    "subject": { "kind": "file", "id": "app.css" },
    "limits": { "rules": { "maximum": 10 } }
  }]
}"#;

const ACCESSIBILITY_POLICY: &[u8] = br##"{
  "schemaVersion": 1,
  "policyVersion": 1,
  "checks": {
    "contrast": { "violation": "fail", "unverified": "warn", "manualRequired": "warn" },
    "motion": { "violation": "fail", "unverified": "warn", "manualRequired": "warn" },
    "focusVisibility": { "violation": "fail", "unverified": "warn", "manualRequired": "warn" },
    "forcedColors": { "violation": "fail", "unverified": "warn", "manualRequired": "warn" },
    "inputModality": { "violation": "fail", "unverified": "warn", "manualRequired": "warn" }
  },
  "contrastPairs": [{
    "id": "body-on-surface",
    "foreground": { "literal": { "value": "#777" } },
    "background": { "literal": { "value": "#fff" } },
    "minimumRatioMilli": 7000
  }],
  "exceptions": []
}"##;

const DTCG_RESOLVER: &str = r##"{
  "version":"2025.10",
  "name":"audit graph",
  "$defs":{"base":{"color":{"$type":"color","brand":{"$value":{"colorSpace":"srgb","components":[0.1,0.2,0.3],"alpha":1}}}}},
  "sets":{"base":{"sources":[{"$ref":"#/$defs/base"}]}},
  "modifiers":{},
  "resolutionOrder":[{"$ref":"#/sets/base"}]
}"##;

fn assert_exact_policy_input(manifest: &pliego_css_control::ControlManifest, bytes: &[u8]) {
    let policy = manifest
        .inputs
        .files
        .iter()
        .find(|input| input.file == "budgets.json")
        .expect("budget policy in the exact-input ledger");
    assert_eq!(policy.sha256, pliego_css_control::content_hash(bytes));
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "pliegocss-audit-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("temporary directory");
    path
}

#[test]
fn audit_accepts_normal_css_and_reports_canonical_json() {
    let directory = temp_dir("valid");
    fs::write(
        directory.join("app.css"),
        "@layer app { .button:hover { color: red !important; } }\n",
    )
    .expect("CSS fixture");
    let run = |check: bool| {
        let mut arguments = vec![
            "audit",
            "--input",
            "app.css",
            "--targets",
            "baseline-widely",
            "--format",
            "json",
        ];
        if check {
            arguments.push("--check");
        }
        Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
            .current_dir(&directory)
            .args(arguments)
            .output()
            .expect("audit command")
    };
    let output = run(false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("finding JSON");
    assert_eq!(value["schemaVersion"], "1.0.0");
    assert_eq!(value["command"], "audit");
    let inventory = value["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|finding| finding["code"] == "PCSS-AUDIT-000")
        .expect("inventory");
    assert_eq!(inventory["context"]["backend"], "lightningcss");
    assert_eq!(inventory["context"]["target-profile"], "baseline-widely");
    fs::remove_dir_all(directory).expect("remove fixture");
}

#[test]
fn accessibility_policy_has_human_json_sarif_parity_and_fails_the_gate() {
    let directory = temp_dir("accessibility-parity");
    fs::write(directory.join("app.css"), ".button { color: red; }\n").expect("CSS fixture");
    fs::write(
        directory.join("pliego.accessibility.json"),
        ACCESSIBILITY_POLICY,
    )
    .expect("accessibility policy fixture");
    let run = |format: &str| {
        Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
            .current_dir(&directory)
            .args([
                "audit",
                "--input",
                "app.css",
                "--targets",
                "none",
                "--accessibility-policy",
                "pliego.accessibility.json",
                "--format",
                format,
            ])
            .output()
            .expect("audit command")
    };

    let json = run("json");
    assert!(!json.status.success());
    assert!(json.stderr.is_empty());
    let document: serde_json::Value = serde_json::from_slice(&json.stdout).expect("finding JSON");
    let contrast = document["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|finding| finding["code"] == "PCSS-A11Y-101")
        .expect("contrast violation");
    assert_eq!(contrast["severity"], "error");
    assert_eq!(contrast["verification"], "verified");
    assert_eq!(contrast["source"]["file"], "pliego.accessibility.json");
    assert!(
        contrast["context"]["subject-id"]
            .as_str()
            .is_some_and(|value| value.starts_with("sha256:"))
    );

    let human = run("human");
    assert!(!human.status.success());
    assert!(String::from_utf8_lossy(&human.stdout).contains("error[PCSS-A11Y-101]"));

    let sarif = run("sarif");
    assert!(!sarif.status.success());
    let sarif: serde_json::Value = serde_json::from_slice(&sarif.stdout).expect("SARIF JSON");
    assert!(
        sarif["runs"][0]["results"]
            .as_array()
            .expect("SARIF results")
            .iter()
            .any(|result| result["ruleId"] == "PCSS-A11Y-101")
    );
    fs::remove_dir_all(directory).expect("remove fixture");
}

#[test]
fn accessibility_control_binds_policy_and_graph_and_counts_declared_pairs() {
    let directory = temp_dir("accessibility-control");
    fs::create_dir(directory.join("control")).expect("control directory");
    fs::write(directory.join("app.css"), ".button { color: red; }\n").expect("CSS fixture");
    fs::write(
        directory.join("pliego.accessibility.json"),
        ACCESSIBILITY_POLICY,
    )
    .expect("accessibility policy fixture");
    let resolver = parse_dtcg_resolver_str(DTCG_RESOLVER).expect("DTCG resolver");
    let graph = resolver
        .graph()
        .to_canonical_json()
        .expect("canonical token graph");
    fs::write(directory.join("pliego.tokens.input.json"), &graph).expect("token graph fixture");
    let run = |check: bool| {
        let mut arguments = vec![
            "audit",
            "--input",
            "app.css",
            "--targets",
            "none",
            "--accessibility-policy",
            "pliego.accessibility.json",
            "--token-graph",
            "pliego.tokens.input.json",
            "--control-dir",
            "control",
            "--format",
            "json",
        ];
        if check {
            arguments.push("--check");
        }
        Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
            .current_dir(&directory)
            .args(arguments)
            .output()
            .expect("audit command")
    };
    let output = run(false);
    assert!(!output.status.success(), "contrast policy must fail");
    let manifest =
        fs::read(directory.join("control/pliego.css.manifest.json")).expect("control manifest");
    let parsed = parse_control_manifest(&manifest).expect("valid control manifest");
    assert_eq!(parsed.tool.contracts["accessibility-policy"], "1");
    assert!(
        parsed
            .backend
            .stages
            .contains(&"accessibility-analysis".to_owned())
    );
    assert_eq!(parsed.tokens.contrast_pairs, 1);
    assert_eq!(parsed.tokens.coverage_basis_points, Some(0));
    assert!(
        parsed
            .inputs
            .files
            .iter()
            .any(|input| input.file == "pliego.accessibility.json"
                && input.role == "accessibility-policy")
    );
    assert!(
        parsed
            .inputs
            .files
            .iter()
            .any(|input| input.file == "pliego.tokens.input.json" && input.role == "token-graph")
    );
    assert_eq!(
        fs::read(directory.join("control/pliego.tokens.json")).expect("published token graph"),
        graph
    );
    let checked = run(true);
    assert!(
        !checked.status.success(),
        "policy violation remains nonzero"
    );
    assert!(checked.stderr.is_empty(), "check must find no drift");

    let mut changed_policy = ACCESSIBILITY_POLICY.to_vec();
    changed_policy.push(b'\n');
    fs::write(directory.join("pliego.accessibility.json"), changed_policy)
        .expect("changed policy bytes");
    let drift = run(true);
    assert!(!drift.status.success());
    assert!(String::from_utf8_lossy(&drift.stderr).contains("drift detected"));
    fs::remove_dir_all(directory).expect("remove fixture");
}

#[cfg(unix)]
#[test]
fn accessibility_policy_symlink_is_rejected_before_analysis() {
    use std::os::unix::fs::symlink;

    let directory = temp_dir("accessibility-symlink");
    fs::write(directory.join("app.css"), ".button { color: red; }\n").expect("CSS fixture");
    fs::write(directory.join("real-policy.json"), ACCESSIBILITY_POLICY)
        .expect("real policy fixture");
    symlink("real-policy.json", directory.join("policy-link.json")).expect("policy symlink");
    let output = Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .current_dir(&directory)
        .args([
            "audit",
            "--input",
            "app.css",
            "--targets",
            "none",
            "--accessibility-policy",
            "policy-link.json",
        ])
        .output()
        .expect("audit command");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("must not traverse symbolic links or reparse points")
    );
    fs::remove_dir_all(directory).expect("remove fixture");
}

#[test]
fn syntax_failure_uses_the_same_json_and_a_nonzero_exit() {
    let directory = temp_dir("invalid");
    fs::write(directory.join("broken.css"), ".button > { color: red; }\n").expect("CSS fixture");
    let output = Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .current_dir(&directory)
        .args([
            "audit",
            "--input",
            "broken.css",
            "--targets",
            "baseline-widely",
            "--format",
            "json",
        ])
        .output()
        .expect("audit command");
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("finding JSON");
    assert_eq!(value["findings"][0]["code"], "PCSS-SYNTAX-001");
    assert_eq!(value["findings"][0]["severity"], "error");
    assert_eq!(value["findings"][0]["source"]["file"], "broken.css");
    fs::remove_dir_all(directory).expect("remove fixture");
}

#[test]
fn budget_policy_and_explicit_ownership_fail_the_cli_gate() {
    let directory = temp_dir("budget");
    fs::write(
        directory.join("route.css"),
        ".one { color: red; }\n.two { color: red; }\n",
    )
    .expect("CSS fixture");
    fs::write(
        directory.join("budgets.json"),
        r#"{
          "schemaVersion":1,
          "policyVersion":1,
          "budgets":[{
            "id":"route-duplicates",
            "subject":{"kind":"route","id":"/home"},
            "limits":{"semanticDuplicates":{"maximum":0,"baseline":0,"maxIncrease":0}}
          }]
        }"#,
    )
    .expect("budget fixture");
    let output = Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .current_dir(&directory)
        .args([
            "audit",
            "--input",
            "route.css",
            "--targets",
            "modern",
            "--budget-policy",
            "budgets.json",
            "--budget-subject",
            "route=/home",
            "--format",
            "json",
        ])
        .output()
        .expect("audit command");
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("finding JSON");
    let budget = value["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|finding| finding["context"]["budget-id"] == "route-duplicates")
        .expect("budget finding");
    assert_eq!(budget["code"], "PCSS-BUDGET-101");
    assert_eq!(budget["context"]["subject-kind"], "route");
    assert_eq!(budget["context"]["subject-id"], "/home");
    fs::remove_dir_all(directory).expect("remove fixture");
}

#[test]
fn sarif_projects_every_canonical_finding_without_semantic_drift() {
    let directory = temp_dir("sarif");
    fs::write(
        directory.join("component style.css"),
        ".button > { color: red; }\n",
    )
    .expect("CSS fixture");
    let run = |format: &str| {
        Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
            .current_dir(&directory)
            .args([
                "audit",
                "--input",
                "component style.css",
                "--targets",
                "baseline-widely",
                "--format",
                format,
            ])
            .output()
            .expect("audit command")
    };
    let canonical_output = run("json");
    let sarif_output = run("sarif");
    let repeated_sarif = run("sarif");
    assert!(!canonical_output.status.success());
    assert!(!sarif_output.status.success());
    assert_eq!(sarif_output.status, repeated_sarif.status);
    assert_eq!(sarif_output.stdout, repeated_sarif.stdout);
    assert!(sarif_output.stdout.ends_with(b"\n"));
    let canonical: serde_json::Value =
        serde_json::from_slice(&canonical_output.stdout).expect("finding JSON");
    let sarif: serde_json::Value =
        serde_json::from_slice(&sarif_output.stdout).expect("SARIF JSON");
    assert_eq!(sarif["version"], "2.1.0");
    assert_eq!(
        sarif["runs"][0]["properties"]["pliegoCssFindingSchemaVersion"],
        canonical["schemaVersion"]
    );
    let findings = canonical["findings"].as_array().expect("findings");
    let results = sarif["runs"][0]["results"].as_array().expect("results");
    assert_eq!(findings.len(), results.len());
    for (finding, result) in findings.iter().zip(results) {
        assert_eq!(result["properties"]["pliegoCssFinding"], *finding);
        assert_eq!(result["ruleId"], finding["code"]);
        assert_eq!(
            result["partialFingerprints"]["pliegoCssFinding/v1"],
            finding["fingerprint"]
        );
    }
    assert_eq!(results[0]["level"], "error");
    assert_eq!(
        results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "component%20style.css"
    );
    assert_eq!(
        results[0]["locations"][0]["physicalLocation"]["region"]["byteOffset"],
        findings[0]["source"]["byteStart"]
    );
    fs::remove_dir_all(directory).expect("remove fixture");
}

#[test]
fn audit_publishes_and_checks_one_canonical_control_group() {
    let directory = temp_dir("control");
    fs::create_dir(directory.join("control")).expect("control directory");
    fs::write(
        directory.join("app.css"),
        "@layer app { .card { color: red; } }\n@layer app { .other { color: blue; } }\n",
    )
    .expect("CSS fixture");
    fs::write(directory.join("budgets.json"), PASSING_BUDGET_POLICY)
        .expect("budget policy fixture");
    let run = |check: bool| {
        let mut arguments = vec![
            "audit",
            "--input",
            "app.css",
            "--targets",
            "none",
            "--format",
            "json",
            "--budget-policy",
            "budgets.json",
            "--control-dir",
            "control",
        ];
        if check {
            arguments.push("--check");
        }
        Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
            .current_dir(&directory)
            .args(arguments)
            .output()
            .expect("audit command")
    };

    let first = run(false);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let findings =
        fs::read(directory.join("control/pliego.css.findings.json")).expect("finding document");
    let manifest =
        fs::read(directory.join("control/pliego.css.manifest.json")).expect("control manifest");
    let receipt =
        fs::read(directory.join("control/pliego.css.receipt.json")).expect("build receipt");
    assert_eq!(findings, first.stdout);
    let parsed_manifest = parse_control_manifest(&manifest).expect("valid control manifest");
    let parsed_receipt =
        parse_build_receipt(&receipt, &manifest).expect("valid manifest-bound receipt");
    assert_eq!(
        parsed_manifest.rules.observation,
        pliego_css_control::MeasurementState::Measured
    );
    assert_eq!(parsed_manifest.rules.declarations, 2);
    assert_eq!(parsed_manifest.decisions.len(), 1);
    assert_eq!(parsed_manifest.targets.profile, "none");
    assert!(parsed_manifest.targets.browsers.is_empty());
    assert_exact_policy_input(&parsed_manifest, PASSING_BUDGET_POLICY);
    assert_eq!(
        parsed_manifest.tokens.observation,
        pliego_css_control::MeasurementState::Unavailable
    );
    assert_eq!(
        parsed_receipt.result,
        pliego_css_control::ReceiptResult::Passed
    );

    let repeated = run(false);
    assert!(repeated.status.success());
    assert_eq!(
        fs::read(directory.join("control/pliego.css.manifest.json")).unwrap(),
        manifest
    );
    assert_eq!(
        fs::read(directory.join("control/pliego.css.receipt.json")).unwrap(),
        receipt
    );
    assert!(run(true).status.success());

    fs::write(
        directory.join("control/pliego.css.findings.json"),
        b"tampered\n",
    )
    .expect("tamper finding document");
    let drift = run(true);
    assert!(!drift.status.success());
    assert!(String::from_utf8_lossy(&drift.stderr).contains("drift detected"));
    assert_eq!(
        fs::read(directory.join("control/pliego.css.findings.json")).unwrap(),
        b"tampered\n"
    );
    assert!(run(false).status.success());
    assert_eq!(
        fs::read(directory.join("control/pliego.css.findings.json")).unwrap(),
        findings
    );
    fs::remove_dir_all(directory).expect("remove fixture");
}

#[test]
fn failed_syntax_audit_emits_an_honest_failed_receipt() {
    let directory = temp_dir("control-failed");
    fs::create_dir(directory.join("control")).expect("control directory");
    fs::write(directory.join("broken.css"), ".card > { color: red; }\n").expect("CSS fixture");
    let output = Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .current_dir(&directory)
        .args([
            "audit",
            "--input",
            "broken.css",
            "--targets",
            "baseline-widely",
            "--format",
            "json",
            "--control-dir",
            "control",
        ])
        .output()
        .expect("audit command");
    assert!(!output.status.success());
    let manifest =
        fs::read(directory.join("control/pliego.css.manifest.json")).expect("control manifest");
    let receipt =
        fs::read(directory.join("control/pliego.css.receipt.json")).expect("build receipt");
    let parsed_manifest = parse_control_manifest(&manifest).expect("valid control manifest");
    let parsed_receipt =
        parse_build_receipt(&receipt, &manifest).expect("valid manifest-bound receipt");
    assert_eq!(
        parsed_manifest.rules.observation,
        pliego_css_control::MeasurementState::Unavailable
    );
    assert!(parsed_manifest.rules.unavailable_reason.is_some());
    assert_eq!(
        parsed_receipt.result,
        pliego_css_control::ReceiptResult::Failed
    );
    assert_eq!(parsed_receipt.findings.unexcepted_errors, 1);
    fs::remove_dir_all(directory).expect("remove fixture");
}
