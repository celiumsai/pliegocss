//! Black-box ordinary CSS transformation contract.

use std::fs;
use std::process::Command;

#[test]
fn transforms_and_checks_ordinary_css_without_mutating_input() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let dir = root.join("target/tests/standard-css-transform");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("app.css");
    let logical_input = std::path::Path::new("target/tests/standard-css-transform/app.css");
    let control = dir.join("control");
    fs::create_dir_all(&control).unwrap();
    let output = control.join("app.min.css");
    let source = ".card { user-select: none; color: color(display-p3 1 0 0); }\n";
    fs::write(&input, source).unwrap();
    let run = |check: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pliego-cssc"));
        command
            .current_dir(root)
            .args(["transform-css", "--input"])
            .arg(logical_input)
            .args(["--output"])
            .arg(&output)
            .args([
                "--targets",
                "modern",
                "--format",
                "minified",
                "--control-dir",
            ])
            .arg(&control);
        if check {
            command.arg("--check");
        }
        command.output().unwrap()
    };
    let first = run(false);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(fs::read_to_string(&input).unwrap(), source);
    let transformed = fs::read_to_string(&output).unwrap();
    assert!(transformed.ends_with('\n'));
    assert_ne!(transformed, source);
    assert!(run(true).status.success());
    for file in [
        "pliego.css.findings.json",
        "pliego.css.manifest.json",
        "pliego.css.receipt.json",
    ] {
        assert!(control.join(file).is_file(), "missing {file}");
    }
    fs::write(&output, "stale\n").unwrap();
    let stale = run(true);
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("drift detected"));
    fs::remove_dir_all(&dir).unwrap();
}
