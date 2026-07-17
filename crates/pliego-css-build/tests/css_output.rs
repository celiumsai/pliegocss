#![cfg(feature = "artifacts")]
//! Exact CSS-output optimizer contracts.

use lightningcss::targets::Targets;
use pliego_css_build::artifacts::{FixedCssOutputCache, optimize_css, optimize_css_with_trace};

#[test]
fn fixed_settings_cache_skips_only_exact_raw_stylesheets() {
    let mut cache = FixedCssOutputCache::default();
    let first = cache
        .optimize("a{color:red}", Targets::default(), true)
        .expect("initial optimization");
    let repeated = cache
        .optimize("a{color:red}", Targets::default(), true)
        .expect("cached optimization");
    assert_eq!(first, repeated);
    assert_eq!(cache.hits(), 1);

    let changed = cache
        .optimize("a{color:blue}", Targets::default(), true)
        .expect("changed optimization");
    assert_ne!(changed, first);
    assert_eq!(cache.hits(), 1);

    let reformatted = cache
        .optimize("a{color:blue}", Targets::default(), false)
        .expect("format change optimization");
    assert_ne!(reformatted, changed);
    assert_eq!(cache.hits(), 1);
}

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
