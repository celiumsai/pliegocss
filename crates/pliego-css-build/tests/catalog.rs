#![cfg(feature = "catalog")]
//! Frozen generated-catalog byte contracts.

use pliego_css_build::artifacts::{CatalogOutputFormat, render_catalog, sha256_hex};
use pliego_css_theme::ThemeRegistry;

#[test]
fn seed_catalog_bytes_match_the_existing_cli_contract() {
    let theme = ThemeRegistry::seed();
    let json = render_catalog(&theme, CatalogOutputFormat::Json).expect("render JSON catalog");
    let markdown =
        render_catalog(&theme, CatalogOutputFormat::Markdown).expect("render Markdown catalog");

    assert_eq!(
        sha256_hex(json.as_bytes()),
        "6ac31ef2bb0c1051829e2eaa6bfda389937a0e55f1489f4ad3f00d9d46e7fe93"
    );
    assert_eq!(
        sha256_hex(markdown.as_bytes()),
        "e0ebf303c80937697d09723d890487aaa2ed8db6059a766f61b3be71765ba1da"
    );
    assert!(json.ends_with('\n'));
    assert!(markdown.ends_with('\n'));
}

#[test]
fn json_catalog_is_closed_over_the_frozen_top_level_shape() {
    let rendered = render_catalog(&ThemeRegistry::seed(), CatalogOutputFormat::Json)
        .expect("render JSON catalog");
    let document: serde_json::Value = serde_json::from_str(&rendered).expect("parse JSON catalog");
    let object = document.as_object().expect("catalog object");
    let mut keys = object.keys().map(String::as_str).collect::<Vec<_>>();
    keys.sort_unstable();

    assert_eq!(
        keys,
        [
            "classNameFormatVersion",
            "schemaVersion",
            "styleIdFormatVersion",
            "theme",
            "themeIdFormatVersion",
            "utilities",
        ]
    );
    assert_eq!(document["schemaVersion"], 4);
    assert!(
        document["utilities"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
}
