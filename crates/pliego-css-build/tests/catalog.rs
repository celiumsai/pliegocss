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
        "b5a2092fdfd079e2b586ac9bd850ead20599e4d9176cc358cc8bd1f10ea6273c"
    );
    assert_eq!(
        sha256_hex(markdown.as_bytes()),
        "2efd4bf3cc25162de2996ae364876f61e4c8c98626006782bd0648ee149d335e"
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
    assert_eq!(document["schemaVersion"], 3);
    assert!(
        document["utilities"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
}
