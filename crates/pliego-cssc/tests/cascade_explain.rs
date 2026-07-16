//! Black-box contract for bounded standard-CSS cascade explanation.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "pliegocss-cascade-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("temporary directory");
    path
}

fn run(directory: &PathBuf, format: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pliego-cssc"))
        .current_dir(directory)
        .args([
            "explain-cascade",
            "--input",
            "app.css",
            "--element",
            "button#save.action",
            "--property",
            "color",
            "--format",
            format,
        ])
        .output()
        .expect("cascade command")
}

fn object_keys(value: &serde_json::Value) -> Vec<&str> {
    let mut keys = value
        .as_object()
        .expect("JSON object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

#[test]
fn cascade_json_is_deterministic_range_exact_and_explains_the_winner() {
    let directory = temp_dir("resolved");
    let css = concat!(
        "@layer base, overrides;\n",
        "@layer overrides { button.action { color: purple !important; } }\n",
        "@layer base { #save { color: green !important; } }\n",
        "button#save.action { color: red; }\n",
    );
    fs::write(directory.join("app.css"), css).expect("CSS fixture");

    let first = run(&directory, "json");
    let second = run(&directory, "json");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(first.stdout, second.stdout);
    let document: serde_json::Value = serde_json::from_slice(&first.stdout).expect("cascade JSON");
    assert_eq!(
        object_keys(&document),
        [
            "blockers",
            "candidates",
            "element",
            "input",
            "limitations",
            "property",
            "schemaVersion",
            "scope",
            "specificityEscalation",
            "status",
            "winner",
        ]
    );
    assert_eq!(document["schemaVersion"], 1);
    assert_eq!(
        document["scope"],
        "single-author-stylesheet-direct-element-declarations"
    );
    assert_eq!(document["status"], "resolved");
    assert_eq!(document["property"], "color");
    assert_eq!(document["element"]["selector"], "button#save.action");
    assert_eq!(document["blockers"], serde_json::json!([]));
    let winner_id = document["winner"].as_str().expect("winner ID");
    let candidates = document["candidates"].as_array().expect("candidates");
    let winner = candidates
        .iter()
        .find(|candidate| candidate["id"] == winner_id)
        .expect("winner candidate");
    assert_eq!(
        object_keys(winner),
        [
            "decisiveCriterion",
            "declaration",
            "disposition",
            "effectiveDeclaration",
            "id",
            "important",
            "layer",
            "layerOrder",
            "selector",
            "source",
            "sourceOrder",
            "specificity",
        ]
    );
    assert_eq!(winner["declaration"], "color: green !important");
    assert_eq!(winner["layer"], "base");
    assert_eq!(winner["disposition"], "winner");
    assert_eq!(
        winner["specificity"],
        serde_json::json!({
            "ids": 1,
            "classes": 0,
            "types": 0
        })
    );
    for candidate in candidates {
        let start = usize::try_from(candidate["source"]["byteStart"].as_u64().expect("start"))
            .expect("start fits usize");
        let end = usize::try_from(candidate["source"]["byteEnd"].as_u64().expect("end"))
            .expect("end fits usize");
        assert_eq!(&css[start..end], candidate["declaration"].as_str().unwrap());
    }
    assert_eq!(
        document["specificityEscalation"]["winnerExceedsMinimum"],
        true
    );

    let text = run(&directory, "text");
    assert!(text.status.success());
    let text = String::from_utf8(text.stdout).expect("text output");
    assert!(text.contains("Status: resolved"));
    assert!(text.contains("Winner: color: green"));
    fs::remove_dir_all(directory).expect("remove fixture");
}

#[test]
fn cascade_runtime_context_succeeds_without_claiming_a_winner() {
    let directory = temp_dir("browser-required");
    fs::write(
        directory.join("app.css"),
        concat!(
            ".action:hover { color: red; }\n",
            "@media (width > 40rem) { .action { color: blue; } }\n",
        ),
    )
    .expect("CSS fixture");
    let output = run(&directory, "json");
    assert!(output.status.success());
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).expect("cascade JSON");
    assert_eq!(document["status"], "browser-required");
    assert!(document["winner"].is_null());
    let codes = document["blockers"]
        .as_array()
        .expect("blockers")
        .iter()
        .map(|blocker| blocker["code"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(codes, ["unsupported-selector", "media-context"]);
    fs::remove_dir_all(directory).expect("remove fixture");
}
