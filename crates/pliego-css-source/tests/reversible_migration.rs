//! Reversible migration plan contract.

use pliego_css_source::{
    MigrationProject, MigrationProjectSource, MigrationSourceKind, apply_reversible_addition,
    apply_reversible_replacement, apply_reversible_replacement_group,
    build_reversible_migration_plan, inventory_migration_source,
    prepare_static_template_class_alias, prepare_tailwind_marker_replacement,
    prepare_tailwind_theme_root_projection, prepare_tailwind_utility_projection,
    rollback_reversible_addition, rollback_reversible_replacement,
    rollback_reversible_replacement_group,
};

#[test]
fn reversible_plan_binds_inventory_and_refuses_automatic_edits() {
    let project = MigrationProject::new().source(MigrationProjectSource::new(
        MigrationSourceKind::Tailwind,
        "src/app.css",
    ));
    let inventory = inventory_migration_source(
        MigrationSourceKind::Tailwind,
        "src/app.css",
        "@import \"tailwindcss\";\n@theme { --color-brand: red; }\n@utility content-auto { content-visibility: auto; }\n",
    )
    .expect("inventory");
    let plan = build_reversible_migration_plan(&project, &[inventory]).expect("plan");
    let value: serde_json::Value = serde_json::from_slice(plan.as_bytes()).expect("JSON");
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["mode"], "inventory-only");
    assert_eq!(value["reversible"], true);
    assert_eq!(value["edits"].as_array().map(Vec::len), Some(0));
    assert_eq!(value["sources"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["sources"][0]["file"], "src/app.css");
    assert_eq!(value["inventorySha256"].as_str().map(str::len), Some(64));
    assert_eq!(value["rollback"]["strategy"], "restore-exact-source-bytes");
    assert_eq!(
        value["rollback"]["preconditions"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(value["proposals"].as_array().map(Vec::len), Some(4));
    assert_eq!(value["proposals"][0]["kind"], "add");
    assert_eq!(value["proposals"][0]["file"], "pliego.migration.css");
    assert_eq!(
        value["proposals"][0]["afterSha256"].as_str().map(str::len),
        Some(64)
    );
    assert_eq!(value["proposals"][1]["kind"], "replace");
    assert_eq!(value["proposals"][1]["file"], "src/app.css");
    assert_eq!(
        value["proposals"][1]["beforeSha256"].as_str().map(str::len),
        Some(64)
    );
    assert_eq!(
        value["proposals"][1]["afterSha256"].as_str().map(str::len),
        Some(64)
    );
    assert_eq!(value["proposals"][1]["automatic"], false);
    assert_eq!(value["proposals"][2]["kind"], "theme-root-projection");
    assert_eq!(value["proposals"][2]["file"], "src/app.css");
    assert_eq!(
        value["proposals"][2]["afterSha256"].as_str().map(str::len),
        Some(64)
    );
    assert_eq!(value["proposals"][2]["automatic"], false);
    assert_eq!(value["proposals"][3]["kind"], "utility-projection");
    assert_eq!(value["proposals"][3]["file"], "src/app.css");
    assert_eq!(
        value["proposals"][3]["afterSha256"].as_str().map(str::len),
        Some(64)
    );
    assert_eq!(value["proposals"][3]["automatic"], false);
}

#[test]
fn additive_edit_applies_and_rolls_back_exact_bytes() {
    let root = std::path::Path::new("target/tests")
        .join(format!("pliegocss-migration-add-{}", std::process::id()));
    let destination = root.join("pliego.migration.css");
    std::fs::create_dir_all(&root).unwrap();
    let after = b"/* PliegoCSS migration sidecar */\n";
    let receipt = apply_reversible_addition(&destination, after).expect("apply");
    assert_eq!(std::fs::read(&destination).unwrap(), after);
    assert!(apply_reversible_addition(&destination, after).is_err());
    rollback_reversible_addition(&destination, &receipt).expect("rollback");
    assert!(!destination.exists());
    assert!(rollback_reversible_addition(&destination, &receipt).is_err());
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn exact_replacement_requires_before_bytes_and_rolls_back() {
    let root = std::path::Path::new("target/tests").join(format!(
        "pliegocss-migration-replace-{}",
        std::process::id()
    ));
    let destination = root.join("app.css");
    std::fs::create_dir_all(&root).unwrap();
    let before = b"@import \"tailwindcss\";\n";
    let after = b"@import \"tailwindcss\";\n/* PliegoCSS migration prepared */\n";
    std::fs::write(&destination, before).unwrap();
    let receipt = apply_reversible_replacement(&destination, before, after).expect("apply");
    let receipt = pliego_css_source::ReversibleReplacementReceipt::from_json(
        &receipt.to_json().expect("receipt JSON"),
    )
    .expect("receipt round trip");
    assert_eq!(std::fs::read(&destination).unwrap(), after);
    assert!(apply_reversible_replacement(&destination, before, after).is_err());
    rollback_reversible_replacement(&destination, &receipt).expect("rollback");
    assert_eq!(std::fs::read(&destination).unwrap(), before);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn tailwind_marker_codemod_is_idempotent_and_preserves_existing_bytes() {
    let before = b"@import \"tailwindcss\";\n@theme { --color-brand: red; }\n";
    let after = prepare_tailwind_marker_replacement(before).expect("prepare");
    assert!(after.starts_with(before));
    assert_eq!(
        after,
        b"@import \"tailwindcss\";\n@theme { --color-brand: red; }\n/* PliegoCSS migration prepared */\n"
    );
    assert!(prepare_tailwind_marker_replacement(&after).is_err());
    assert!(prepare_tailwind_marker_replacement(b"body { color: red; }\n").is_err());
}

#[test]
fn tailwind_theme_projection_adds_equivalent_root_custom_properties() {
    let before =
        b"@import \"tailwindcss\";\n\n@theme {\n  --color-brand-500: oklch(68% 0.17 230);\n}\n";
    let after = prepare_tailwind_theme_root_projection(before).expect("projection");
    assert!(after.starts_with(before));
    assert!(
        std::str::from_utf8(&after)
            .unwrap()
            .contains(":root {\n  --color-brand-500: oklch(68% 0.17 230);\n}\n")
    );
    assert!(prepare_tailwind_theme_root_projection(&after).is_err());
    assert!(prepare_tailwind_theme_root_projection(b"@theme inline { --x: 1; }\n").is_err());
}

#[test]
fn tailwind_theme_projection_preserves_multiple_ordered_var_values() {
    let before = b"@theme {\n  --space-base: 1rem;\n  --space-card: calc(var(--space-base) * 2);\n  --color-card: var(--color-brand-500, oklch(68% 0.17 230));\n}\n";
    let after = prepare_tailwind_theme_root_projection(before).expect("projection");
    let text = std::str::from_utf8(&after).unwrap();
    let projected = text
        .split("/* PliegoCSS projected Tailwind theme */")
        .nth(1)
        .unwrap();
    let base = projected.find("--space-base: 1rem").unwrap();
    let card = projected
        .find("--space-card: calc(var(--space-base) * 2)")
        .unwrap();
    let color = projected
        .find("--color-card: var(--color-brand-500, oklch(68% 0.17 230))")
        .unwrap();
    assert!(base < card && card < color);
}

#[test]
fn tailwind_static_utility_projects_to_standard_class() {
    let before = b"@utility content-auto {\n  content-visibility: auto;\n  contain-intrinsic-size: auto 1000px;\n}\n";
    let after = prepare_tailwind_utility_projection(before).expect("projection");
    assert!(after.starts_with(before));
    assert!(std::str::from_utf8(&after).unwrap().contains(
        ".content-auto {\n  content-visibility: auto;\n  contain-intrinsic-size: auto 1000px;\n}\n"
    ));
    assert!(prepare_tailwind_utility_projection(&after).is_err());
    assert!(
        prepare_tailwind_utility_projection(b"@utility tab-* { tab-size: --value(--tab-*); }\n")
            .is_err()
    );
}

#[test]
fn multiple_static_utilities_project_in_source_order() {
    let before = b"@utility content-auto { content-visibility: auto; }\n@utility balance-text { text-wrap: balance; }\n";
    let after = prepare_tailwind_utility_projection(before).expect("projection");
    let text = std::str::from_utf8(&after).unwrap();
    let first = text
        .find(".content-auto { content-visibility: auto; }")
        .unwrap();
    let second = text.find(".balance-text { text-wrap: balance; }").unwrap();
    assert!(first < second);
    assert!(
        prepare_tailwind_utility_projection(
            b"@utility safe { display: block; }\n@utility tab-* { tab-size: --value(--tab-*); }\n"
        )
        .is_err()
    );
}

#[test]
fn static_template_class_alias_is_exact_and_dynamic_closed() {
    let before = br#"<article class="content-auto rounded-lg">x</article>"#;
    let after = prepare_static_template_class_alias(before, "content-auto", "pc-content-auto")
        .expect("alias");
    assert_eq!(
        std::str::from_utf8(&after).unwrap(),
        r#"<article class="content-auto pc-content-auto rounded-lg">x</article>"#
    );
    assert!(
        prepare_static_template_class_alias(&after, "content-auto", "pc-content-auto").is_err()
    );
    assert!(
        prepare_static_template_class_alias(
            br"<article className={`content-auto ${state}`}>x</article>",
            "content-auto",
            "pc-content-auto"
        )
        .is_err()
    );
}

#[test]
fn replacement_group_applies_and_rolls_back_all_files() {
    let root = std::path::Path::new("target/tests")
        .join(format!("pliegocss-migration-group-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let css = root.join("app.css");
    let html = root.join("index.html");
    let css_before = b".content-auto { content-visibility: auto; }\n";
    let css_after = b".pc-content-auto { content-visibility: auto; }\n";
    let html_before = b"<div class=\"content-auto\"></div>\n";
    let html_after = b"<div class=\"content-auto pc-content-auto\"></div>\n";
    std::fs::write(&css, css_before).unwrap();
    std::fs::write(&html, html_before).unwrap();
    let receipts = apply_reversible_replacement_group(&[
        (&css, css_before.as_slice(), css_after.as_slice()),
        (&html, html_before.as_slice(), html_after.as_slice()),
    ])
    .expect("group apply");
    assert_eq!(receipts.len(), 2);
    let group = pliego_css_source::ReversibleReplacementGroupReceipt::new(receipts.clone())
        .expect("group receipt");
    let group = pliego_css_source::ReversibleReplacementGroupReceipt::from_json(
        &group.to_json().expect("group JSON"),
    )
    .expect("group round trip");
    assert_eq!(group.entries().len(), 2);
    rollback_reversible_replacement_group(group.entries()).expect("group rollback");
    assert_eq!(std::fs::read(&css).unwrap(), css_before);
    assert_eq!(std::fs::read(&html).unwrap(), html_before);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn replacement_group_preflights_all_before_bytes_before_publication() {
    let root = std::path::Path::new("target/tests").join(format!(
        "pliegocss-migration-group-preflight-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let first = root.join("first.txt");
    let second = root.join("second.txt");
    std::fs::write(&first, b"first-before\n").unwrap();
    std::fs::write(&second, b"second-current\n").unwrap();
    let result = apply_reversible_replacement_group(&[
        (
            &first,
            b"first-before\n".as_slice(),
            b"first-after\n".as_slice(),
        ),
        (
            &second,
            b"second-stale\n".as_slice(),
            b"second-after\n".as_slice(),
        ),
    ]);
    assert!(result.is_err());
    assert_eq!(std::fs::read(&first).unwrap(), b"first-before\n");
    assert_eq!(std::fs::read(&second).unwrap(), b"second-current\n");
    std::fs::remove_dir_all(root).unwrap();
}
