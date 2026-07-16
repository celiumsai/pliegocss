//! Black-box contracts for typed, ordered cascade layers.

use pliego_css_compiler::{decode_style_ir, emit_css, encode_style_ir, lower_style};
use pliego_css_ir::{CASCADE_LAYER_ORDER_CSS, DiagnosticCode, SemanticStyle};
use pliego_css_parser::parse_style_list;

fn compile(source: &str) -> SemanticStyle {
    let syntax = parse_style_list(source).expect("layer fixture must parse");
    lower_style(&syntax).expect("layer fixture must lower")
}

#[test]
fn explicit_layers_emit_one_fixed_order_before_native_blocks() {
    let style = compile("layer-utilities:grid layer-components:flex");
    let css = emit_css(&style).expect("layers must emit");

    assert!(css.starts_with(CASCADE_LAYER_ORDER_CSS));
    let components = css
        .find("@layer pliego.components{")
        .expect("components layer");
    let utilities = css
        .find("@layer pliego.utilities{")
        .expect("utilities layer");
    assert!(components < utilities, "{css}");
    assert!(css.contains("{display:flex;}"));
    assert!(css.contains("{display:grid;}"));
}

#[test]
fn legacy_unlayered_output_remains_byte_identical() {
    let css = emit_css(&compile("block")).expect("must emit");

    assert!(!css.contains("@layer"));
    assert!(css.ends_with("{display:block;}"));
}

#[test]
fn layer_dimension_commutes_and_rejects_duplicates() {
    let left = compile("layer-components:md:hover:flex");
    let right = compile("hover:md:layer-components:flex");
    assert_eq!(left.id, right.id);
    assert_eq!(emit_css(&left), emit_css(&right));

    let syntax = parse_style_list("layer-base:layer-utilities:block").expect("must parse");
    let error = lower_style(&syntax).expect_err("two layers must fail");
    assert_eq!(error.code, DiagnosticCode::InvalidVariantChain);
}

#[test]
fn unlayered_and_layered_assignments_have_explicit_native_precedence() {
    let style = compile("layer-overrides:flex grid");
    let css = emit_css(&style).expect("must emit");

    assert!(css.contains("@layer pliego.overrides{"));
    assert!(css.contains("{display:flex;}"));
    assert!(css.contains("{display:grid;}"));
}

#[test]
fn layered_effects_initialize_in_the_lowest_layer() {
    let css = emit_css(&compile(
        "layer-components:ring-2 layer-utilities:shadow-sm",
    ))
    .expect("layered effects must emit");

    assert!(css.contains("@layer pliego.base{.pc_"));
    assert!(css.contains("--pc-shadow:0 0 #0000;"));
    assert!(css.contains("@layer pliego.components{"));
    assert!(css.contains("@layer pliego.utilities{"));
}

#[test]
fn an_unlayered_base_rule_cannot_promote_effect_initializers_above_layers() {
    let css = emit_css(&compile("block layer-components:ring-2"))
        .expect("mixed layered effects must emit");

    assert!(css.contains("@layer pliego.base{.pc_"));
    assert_eq!(css.matches("--pc-ring-width:0;").count(), 1);
    assert!(css.contains("{display:block;}"));
}

#[test]
fn semantic_binary_v2_round_trips_layer_dimension() {
    let style = compile("layer-components:md:cq-sm:rtl:writing-vertical-rl");
    let bytes = encode_style_ir(&style).expect("must encode");
    let decoded = decode_style_ir(&bytes).expect("must decode");

    assert_eq!(decoded, style);
    assert_eq!(encode_style_ir(&decoded).expect("must reencode"), bytes);
}
