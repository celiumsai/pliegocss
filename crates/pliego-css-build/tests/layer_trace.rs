#![cfg(feature = "artifacts")]
//! Public physical-lineage contract for ordered cascade layers.

use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::Targets;
use pliego_css_build::artifacts::{
    TraceDeclaration, TraceRule, TraceStyle, build_physical_projection,
};

#[test]
fn reconciles_layer_order_and_nested_layer_rules() {
    let raw = "@layer pliego.base,pliego.components,pliego.utilities,pliego.overrides;@layer pliego.components{@media (min-width:48rem){@container (min-width:40rem){.pc_fixture:hover{display:grid;}}}}";
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
        11,
        vec![TraceRule::new(vec![TraceDeclaration::new(
            vec![0],
            false,
            false,
        )])],
    )];

    build_physical_projection(raw, &css, Targets::default(), true, false, &styles)
        .unwrap_or_else(|error| panic!("layer lineage must reconcile: {error}; css={css}"));
}
