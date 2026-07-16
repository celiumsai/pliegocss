//! Black-box contracts for typed native writing modes.

use pliego_css_compiler::{decode_style_ir, emit_css, encode_style_ir, lower_style};
use pliego_css_ir::{DiagnosticCode, SemanticStyle};
use pliego_css_parser::parse_style_list;

fn compile(source: &str) -> SemanticStyle {
    let syntax = parse_style_list(source).expect("writing-mode fixture must parse");
    lower_style(&syntax).expect("writing-mode fixture must lower")
}

#[test]
fn writing_modes_emit_native_css_values() {
    for (source, expected) in [
        ("writing-horizontal", "writing-mode:horizontal-tb;"),
        ("writing-vertical-lr", "writing-mode:vertical-lr;"),
        ("writing-vertical-rl", "writing-mode:vertical-rl;"),
    ] {
        let css = emit_css(&compile(source)).expect("writing mode must emit");
        assert!(css.contains(expected), "{source}: {css}");
    }
}

#[test]
fn writing_mode_is_one_conflict_checked_semantic_slot() {
    let syntax = parse_style_list("writing-horizontal writing-vertical-rl").expect("must parse");
    let error = lower_style(&syntax).expect_err("two writing modes must conflict");

    assert_eq!(error.code, DiagnosticCode::Conflict);
}

#[test]
fn writing_mode_composes_with_direction_and_conditions() {
    let left = compile("md:rtl:writing-vertical-rl");
    let right = compile("rtl:md:writing-vertical-rl");

    assert_eq!(left.id, right.id);
    assert_eq!(emit_css(&left), emit_css(&right));
    assert!(
        emit_css(&left)
            .expect("must emit")
            .contains(":dir(rtl){writing-mode:vertical-rl;}")
    );
}

#[test]
fn semantic_binary_v2_round_trips_writing_mode() {
    let style = compile("cq-sm:hover:writing-vertical-lr");
    let bytes = encode_style_ir(&style).expect("must encode");
    let decoded = decode_style_ir(&bytes).expect("must decode");

    assert_eq!(decoded, style);
    assert_eq!(encode_style_ir(&decoded).expect("must reencode"), bytes);
}
