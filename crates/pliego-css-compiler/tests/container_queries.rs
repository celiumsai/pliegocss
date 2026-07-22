//! Black-box contracts for typed size-container utilities and conditions.

use pliego_css_compiler::{
    IR_BINARY_FORMAT_VERSION, class_name, decode_style_ir, emit_css, emit_css_with_theme,
    encode_style_ir, lower_style, lower_style_with_theme,
};
use pliego_css_ir::{BreakpointId, DiagnosticCode, SemanticStyle};
use pliego_css_parser::parse_style_list;
use pliego_css_theme::{BreakpointDefinition, ThemeRegistry};

fn compile(source: &str) -> SemanticStyle {
    let syntax = parse_style_list(source).expect("container fixture must parse");
    lower_style(&syntax).expect("container fixture must lower")
}

#[test]
fn establishes_and_queries_an_inline_size_container() {
    let style = compile("container-inline cq-sm:grid");
    let class = class_name(style.id);

    assert_eq!(
        format!("{:032x}", style.id.get()),
        "78f8ca7e9a1d8b7737ee243d87faf881"
    );
    assert_eq!(class, "pc_75tq511l3l9q0mx595ykqafsx");

    assert_eq!(
        emit_css(&style).expect("container fixture must emit"),
        format!(
            ".{class}{{container-type:inline-size;}}@container (min-width:40rem){{.{class}{{display:grid;}}}}"
        )
    );
}

#[test]
fn container_viewport_and_selector_dimensions_commute() {
    let left = compile("md:cq-sm:hover:grid");
    let right = compile("hover:cq-sm:md:grid");
    let class = class_name(left.id);

    assert_eq!(left.id, right.id);
    assert_eq!(emit_css(&left), emit_css(&right));
    assert_eq!(
        emit_css(&left).expect("nested condition must emit"),
        format!(
            "@media (min-width:48rem){{@container (min-width:40rem){{.{class}:hover{{display:grid;}}}}}}"
        )
    );
}

#[test]
fn rejects_duplicate_and_unknown_container_breakpoints() {
    let duplicate = parse_style_list("cq-sm:cq-md:grid").expect("must parse");
    let error = lower_style(&duplicate).expect_err("duplicate container query must fail");
    assert_eq!(error.code, DiagnosticCode::InvalidVariantChain);

    let unknown = parse_style_list("cq-mx:grid").expect("must parse");
    let error = lower_style(&unknown).expect_err("unknown container query must fail");
    assert_eq!(error.code, DiagnosticCode::UnknownName);
    assert_eq!(error.suggestion.as_deref(), Some("cq-md"));
}

#[test]
fn active_theme_owns_viewport_and_container_widths() {
    let seed = ThemeRegistry::seed();
    let theme = ThemeRegistry::from_definitions(
        seed.tokens().iter().cloned(),
        [
            BreakpointDefinition::new(BreakpointId::new(0), 0, "sm", "32rem"),
            BreakpointDefinition::new(BreakpointId::new(1), 1, "md", "44rem"),
        ],
    )
    .expect("custom theme must be valid");
    let syntax = parse_style_list("md:cq-sm:grid").expect("must parse");
    let style = lower_style_with_theme(&theme, &syntax).expect("must lower");
    let emitted = emit_css_with_theme(&theme, &style).expect("must emit");

    assert!(emitted.starts_with("@media (min-width:44rem){@container (min-width:32rem){"));
}

#[test]
fn semantic_binary_v2_round_trips_container_dimensions() {
    assert_eq!(IR_BINARY_FORMAT_VERSION, 2);
    let style = compile("container-inline md:cq-sm:hover:grid");
    let encoded = encode_style_ir(&style).expect("must encode");
    let decoded = decode_style_ir(&encoded).expect("must decode");

    assert_eq!(decoded, style);
    assert_eq!(encode_style_ir(&decoded).expect("must re-encode"), encoded);
}
