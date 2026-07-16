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
        "02221dc2336517805578366f8d5af2d45ae28eeae7cd9de9f398479069cb018f"
    );
    assert_eq!(
        sha256_hex(markdown.as_bytes()),
        "a58b55b2b947ae62e341027c1d81e40edbcb9bd2f3c55532b796104618f3d5c0"
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
