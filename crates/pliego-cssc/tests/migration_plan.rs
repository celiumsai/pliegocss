//! Black-box reversible migration plan CLI contract.

use std::process::Command;

#[test]
fn migration_plan_is_read_only_and_inventory_bound() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = root.join("integration-tests/representative/vite-tailwind-inventory");
    let before = std::fs::read(fixture.join("src/app.css")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .current_dir(&fixture)
        .args(["migration-project-plan", "."])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["mode"], "inventory-only");
    assert_eq!(value["reversible"], true);
    assert_eq!(value["edits"].as_array().map(Vec::len), Some(0));
    assert_eq!(value["sources"].as_array().map(Vec::len), Some(1));
    assert_eq!(std::fs::read(fixture.join("src/app.css")).unwrap(), before);
}

#[test]
fn migration_sidecar_apply_and_rollback_are_exact() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = root.join("target/tests/migration-sidecar-cli");
    let _ = std::fs::remove_dir_all(&fixture);
    std::fs::create_dir_all(&fixture).unwrap();
    let output = std::path::Path::new("target/tests/migration-sidecar-cli/pliego.migration.css");
    let receipt =
        std::path::Path::new("target/tests/migration-sidecar-cli/pliego.migration.receipt.json");
    let binary = env!("CARGO_BIN_EXE_pliego-cssc");
    let apply = Command::new(binary)
        .current_dir(&root)
        .args(["migration-sidecar-apply", "--output"])
        .arg(output)
        .args(["--receipt"])
        .arg(receipt)
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    assert!(root.join(output).is_file() && root.join(receipt).is_file());
    let rollback = Command::new(binary)
        .current_dir(&root)
        .args(["migration-sidecar-rollback", "--output"])
        .arg(output)
        .args(["--receipt"])
        .arg(receipt)
        .output()
        .unwrap();
    assert!(
        rollback.status.success(),
        "{}",
        String::from_utf8_lossy(&rollback.stderr)
    );
    assert!(!root.join(output).exists() && !root.join(receipt).exists());
    std::fs::remove_dir_all(&fixture).unwrap();
}

#[test]
fn migration_replacement_cli_round_trips_a_disposable_copy() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = root.join("target/tests/migration-replacement-cli");
    let _ = std::fs::remove_dir_all(&fixture);
    std::fs::create_dir_all(&fixture).unwrap();
    let destination = std::path::Path::new("target/tests/migration-replacement-cli/app.css");
    let before_file = std::path::Path::new("target/tests/migration-replacement-cli/before.css");
    let after_file = std::path::Path::new("target/tests/migration-replacement-cli/after.css");
    let receipt = std::path::Path::new("target/tests/migration-replacement-cli/receipt.json");
    let before = b"@import \"tailwindcss\";\n";
    let after = b"@import \"tailwindcss\";\n/* PliegoCSS migration prepared */\n";
    std::fs::write(root.join(destination), before).unwrap();
    std::fs::write(root.join(before_file), before).unwrap();
    std::fs::write(root.join(after_file), after).unwrap();
    let binary = env!("CARGO_BIN_EXE_pliego-cssc");
    let apply = Command::new(binary)
        .current_dir(&root)
        .args(["migration-replace-apply", "--file"])
        .arg(destination)
        .args(["--before"])
        .arg(before_file)
        .args(["--after"])
        .arg(after_file)
        .args(["--receipt"])
        .arg(receipt)
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    assert_eq!(std::fs::read(root.join(destination)).unwrap(), after);
    let rollback = Command::new(binary)
        .current_dir(&root)
        .args(["migration-replace-rollback", "--file"])
        .arg(destination)
        .args(["--receipt"])
        .arg(receipt)
        .output()
        .unwrap();
    assert!(
        rollback.status.success(),
        "{}",
        String::from_utf8_lossy(&rollback.stderr)
    );
    assert_eq!(std::fs::read(root.join(destination)).unwrap(), before);
    assert!(!root.join(receipt).exists());
    std::fs::remove_dir_all(&fixture).unwrap();
}

#[test]
fn migration_group_cli_round_trips_two_files() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = root.join("target/tests/migration-group-cli");
    let _ = std::fs::remove_dir_all(&fixture);
    std::fs::create_dir_all(&fixture).unwrap();
    std::fs::write(fixture.join("a.txt"), b"before-a\n").unwrap();
    std::fs::write(fixture.join("b.txt"), b"before-b\n").unwrap();
    std::fs::write(fixture.join("a.after"), b"after-a\n").unwrap();
    std::fs::write(fixture.join("b.after"), b"after-b\n").unwrap();
    std::fs::write(
        fixture.join("group.json"),
        br#"{"schemaVersion":1,"entries":[{"file":"a.txt","after":"a.after"},{"file":"b.txt","after":"b.after"}]}"#,
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_pliego-cssc");
    let apply = Command::new(binary)
        .current_dir(&fixture)
        .args([
            "migration-group-apply",
            "--manifest",
            "group.json",
            "--receipt",
            "group.receipt.json",
        ])
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    assert_eq!(std::fs::read(fixture.join("a.txt")).unwrap(), b"after-a\n");
    let rollback = Command::new(binary)
        .current_dir(&fixture)
        .args([
            "migration-group-rollback",
            "--receipt",
            "group.receipt.json",
        ])
        .output()
        .unwrap();
    assert!(
        rollback.status.success(),
        "{}",
        String::from_utf8_lossy(&rollback.stderr)
    );
    assert_eq!(std::fs::read(fixture.join("a.txt")).unwrap(), b"before-a\n");
    assert_eq!(std::fs::read(fixture.join("b.txt")).unwrap(), b"before-b\n");
    std::fs::remove_dir_all(fixture).unwrap();
}

#[test]
fn migration_group_cli_rejects_paths_outside_working_directory() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = root.join("target/tests/migration-group-paths");
    let _ = std::fs::remove_dir_all(&fixture);
    std::fs::create_dir_all(&fixture).unwrap();
    std::fs::write(fixture.join("after"), b"after\n").unwrap();
    std::fs::write(
        fixture.join("group.json"),
        br#"{"schemaVersion":1,"entries":[{"file":"../escape.txt","after":"after"}]}"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .current_dir(&fixture)
        .args([
            "migration-group-apply",
            "--manifest",
            "group.json",
            "--receipt",
            "receipt.json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!root.join("target/tests/escape.txt").exists());
    std::fs::remove_dir_all(fixture).unwrap();
}
