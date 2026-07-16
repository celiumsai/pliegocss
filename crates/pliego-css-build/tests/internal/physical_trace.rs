//! Internal physical trace tests.
use super::*;

fn optimize(source: &str, minify: bool) -> String {
    StyleSheet::parse(source, ParserOptions::default())
        .expect("parse trace fixture")
        .to_css(printer(minify, Targets::default()))
        .expect("print trace fixture")
        .code
}

fn declaration(ordinals: &[u32], important: bool, generated: bool) -> TraceDeclaration {
    TraceDeclaration::new(ordinals.to_vec(), important, generated)
}

fn physical_slice(css: &str, start: u64, end: u64) -> &str {
    &css[usize::try_from(start).expect("range start fits")
        ..usize::try_from(end).expect("range end fits")]
}

#[test]
fn reconciles_qualified_declarations_and_excludes_important_from_value_range() {
    let raw = ".pc-a{display:block;color:red!important;}";
    let css = optimize(raw, true);
    let styles = [TraceStyle::new(
        1,
        vec![TraceRule::new(vec![
            declaration(&[0], false, false),
            declaration(&[1], true, false),
        ])],
    )];
    let projection = build_physical_projection(raw, &css, Targets::default(), true, false, &styles)
        .expect("reconcile physical declarations");

    assert_eq!(projection.rules.len(), 1);
    assert_eq!(projection.declarations.len(), 2);
    let important = &projection.declarations[1];
    assert_eq!(important.property, "color");
    assert!(important.important);
    assert_eq!(
        physical_slice(&css, important.byte_start, important.byte_end),
        "color:red!important"
    );
    assert_eq!(
        physical_slice(&css, important.value_byte_start, important.value_byte_end,),
        "red"
    );
}

#[test]
fn records_theme_media_nesting_generated_lineage_and_utf16_offsets() {
    let raw = ":root{--brand:\"😀\";}@media (min-width:48rem){.pc-b:hover{--label:\"á;{}\";display:block!important;}}";
    let css = optimize(raw, false);
    let styles = [TraceStyle::new(
        2,
        vec![TraceRule::new(vec![
            declaration(&[0, 1], false, true),
            declaration(&[2], true, false),
        ])],
    )];
    let projection = build_physical_projection(raw, &css, Targets::default(), false, true, &styles)
        .expect("reconcile nested trace");

    assert_eq!(projection.producers.len(), 1);
    assert_eq!(projection.rules.len(), 3);
    assert_eq!(projection.rules[1].kind, "media");
    assert_eq!(projection.declarations.len(), 3);
    assert!(projection.declarations[1].generated);
    for rule in &projection.rules {
        let range = physical_slice(&css, rule.byte_start, rule.byte_end);
        assert!(!range.is_empty());
    }
    assert!(projection.edges.iter().any(|edge| {
        edge.kind == "ruleNestedInRule"
            && edge.from == "css-rule:00000002"
            && edge.to == "css-rule:00000001"
    }));
}

#[test]
fn reconciles_unquoted_css_escapes_without_treating_them_as_structure() {
    let raw = r".pc-a{--first:foo\;bar;--second:foo\(;}";
    let css = optimize(raw, true);
    let styles = [TraceStyle::new(
        3,
        vec![TraceRule::new(vec![
            declaration(&[0], false, false),
            declaration(&[1], false, false),
        ])],
    )];

    let projection = build_physical_projection(raw, &css, Targets::default(), true, false, &styles)
        .expect("reconcile escaped custom properties");

    assert_eq!(projection.declarations.len(), 2);
    assert_eq!(projection.declarations[0].property, "--first");
    assert_eq!(projection.declarations[1].property, "--second");
    assert_eq!(
        physical_slice(
            &css,
            projection.declarations[0].value_byte_start,
            projection.declarations[0].value_byte_end,
        ),
        r"foo\;bar"
    );
    assert_eq!(
        physical_slice(
            &css,
            projection.declarations[1].value_byte_start,
            projection.declarations[1].value_byte_end,
        ),
        r"foo\("
    );
}

#[test]
fn rejects_noncanonical_physical_rule_order_and_non_whitespace_gaps() {
    let rule = |start, end, header_end| ActualRule {
        kind: "qualified",
        header: String::new(),
        parent: None,
        byte_start: start,
        byte_end: end,
        header_end,
        declarations: Vec::new(),
    };
    assert!(validate_actual_ranges(".a{}.b{}\n", &[rule(4, 8, 6), rule(0, 4, 2)]).is_err());
    assert!(validate_actual_ranges(".a{}x.b{}\n", &[rule(0, 4, 2), rule(5, 9, 7)]).is_err());
}

#[test]
fn rejects_missing_lineage_and_unsupported_rules() {
    let raw = ".pc-a{display:block;color:red;}";
    let css = optimize(raw, true);
    let missing = [TraceStyle::new(
        1,
        vec![TraceRule::new(vec![declaration(&[0], false, false)])],
    )];
    assert!(
        build_physical_projection(raw, &css, Targets::default(), true, false, &missing,).is_err()
    );

    let keyframes = "@keyframes spin{to{opacity:0}}";
    assert!(
        build_physical_projection(
            keyframes,
            &optimize(keyframes, true),
            Targets::default(),
            true,
            false,
            &[],
        )
        .is_err()
    );

    let oversized = "x".repeat(MAX_TRACE_CSS_BYTES + 1);
    assert!(
        build_physical_projection(&oversized, "", Targets::default(), true, false, &[],).is_err()
    );
}
