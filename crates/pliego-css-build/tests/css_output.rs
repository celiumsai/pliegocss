#![cfg(feature = "artifacts")]
//! Exact CSS-output optimizer contracts.

use lightningcss::targets::Targets;
use pliego_css_build::artifacts::{optimize_css, optimize_css_with_trace};

#[test]
fn optimization_and_trace_share_the_same_final_css() {
    let source = ".a{display:block}@media (min-width:40rem){.b{display:grid}}@media (min-width:40rem){.c{display:flex}}";
    let optimized = optimize_css(source, Targets::default(), true).expect("optimize CSS");
    let (traced, trace_input) =
        optimize_css_with_trace(source, Targets::default(), true).expect("optimize traced CSS");

    assert_eq!(traced, optimized);
    assert_eq!(optimized.matches("@media").count(), 1);
    assert_eq!(trace_input.matches("@media").count(), 1);
    assert!(trace_input.contains(".b"));
    assert!(trace_input.contains(".c"));
}

#[test]
fn invalid_css_fails_instead_of_publishing_partial_output() {
    let error = optimize_css(".bad > { color: blue; }", Targets::default(), true)
        .expect_err("reject invalid CSS");
    assert!(!error.is_empty());
}
