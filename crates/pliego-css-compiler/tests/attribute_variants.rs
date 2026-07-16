//! Black-box contracts for configurable typed ARIA/data variants.

use pliego_css_compiler::{emit_css, lower_style};
use pliego_css_ir::{
    DiagnosticCode, SemanticStyle, classified_attribute_name, is_classified_selector_transform,
};
use pliego_css_parser::parse_style_list;

fn compile(source: &str) -> SemanticStyle {
    let syntax = parse_style_list(source).expect("attribute fixture must parse");
    lower_style(&syntax).expect("attribute fixture must lower")
}

fn reject(source: &str) -> DiagnosticCode {
    let syntax = parse_style_list(source).expect("rejection fixture must parse");
    lower_style(&syntax)
        .expect_err("attribute fixture must be rejected")
        .code
}

#[test]
fn configurable_attributes_emit_canonical_native_selectors() {
    let style = compile("aria-[sort=ascending]:data-[density=compact]:data-[loading]:hover:block");
    let css = emit_css(&style).expect("must emit");

    assert!(css.contains(
        ":hover[aria-sort=ascending][data-density=compact][data-loading]{display:block;}"
    ));

    let max_fragment = "a".repeat(64);
    let boundary = compile(&format!(
        "aria-[{max_fragment}={max_fragment}]:data-[{max_fragment}]:block"
    ));
    let boundary_css = emit_css(&boundary).expect("64-byte fragments must emit");
    assert!(boundary_css.contains(&format!(
        "[aria-{max_fragment}={max_fragment}][data-{max_fragment}]"
    )));
}

#[test]
fn configurable_and_short_forms_share_semantic_identity() {
    for (configured, short) in [
        ("aria-[expanded=true]:block", "aria-expanded:block"),
        ("data-[state=open]:block", "data-state-open:block"),
        ("data-[loading]:block", "data-loading:block"),
    ] {
        let configured = compile(configured);
        let short = compile(short);
        assert_eq!(configured.id, short.id);
        assert_eq!(emit_css(&configured), emit_css(&short));
    }
}

#[test]
fn same_attribute_duplicates_and_contradictions_fail_closed() {
    for source in [
        "aria-[sort=ascending]:aria-[sort=descending]:block",
        "aria-expanded:aria-[expanded=false]:block",
        "data-[state=open]:data-state-closed:block",
        "data-[loading]:data-loading:block",
    ] {
        assert_eq!(
            reject(source),
            DiagnosticCode::InvalidVariantChain,
            "{source}"
        );
    }
}

#[test]
fn malformed_configurable_attributes_have_a_stable_diagnostic() {
    for source in [
        "aria-[sort]:block",
        "aria-[sort=]:block",
        "aria-[Sort=ascending]:block",
        "data-[-state=open]:block",
        "data-[state=two=values]:block",
        "data-[state=Open]:block",
    ] {
        assert_eq!(
            reject(source),
            DiagnosticCode::InvalidVariantChain,
            "{source}"
        );
    }

    let overlong = format!("data-[{}=open]:block", "a".repeat(65));
    assert_eq!(
        reject(&overlong),
        DiagnosticCode::InvalidVariantChain,
        "overlong attribute"
    );
}

#[test]
fn only_canonical_attribute_selectors_receive_classification() {
    for selector in [
        "&[aria-sort=ascending]",
        "&[data-density=compact]",
        "&[data-loading]",
    ] {
        assert!(is_classified_selector_transform(selector));
        assert!(classified_attribute_name(selector).is_some());
    }
    for selector in [
        "&[aria-sort]",
        "&[data-State=open]",
        "&[data-state=two=values]",
        "&[role=button]",
    ] {
        assert!(!is_classified_selector_transform(selector), "{selector}");
        assert!(classified_attribute_name(selector).is_none(), "{selector}");
    }
}
