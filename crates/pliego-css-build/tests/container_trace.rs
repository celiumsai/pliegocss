#![cfg(feature = "artifacts")]
//! Public physical-lineage contract for nested container queries.

use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::Targets;
use pliego_css_build::artifacts::{
    TraceDeclaration, TraceRule, TraceStyle, build_physical_projection,
};

#[test]
fn reconciles_nested_viewport_and_container_rules() {
    let raw =
        "@media (min-width:48rem){@container (min-width:40rem){.pc_fixture:hover{display:grid;}}}";
    let css = StyleSheet::parse(raw, ParserOptions::default())
        .expect("fixture must parse")
        .to_css(PrinterOptions {
            minify: true,
            targets: Targets::default(),
            ..PrinterOptions::default()
        })
        .expect("fixture must optimize")
        .code;
    let styles = [TraceStyle::new(
        7,
        vec![TraceRule::new(vec![TraceDeclaration::new(
            vec![0],
            false,
            false,
        )])],
    )];

    build_physical_projection(raw, &css, Targets::default(), true, false, &styles).unwrap_or_else(
        |error| panic!("nested container lineage must reconcile: {error}; css={css}"),
    );
}
