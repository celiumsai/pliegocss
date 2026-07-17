//! Incremental CSS-fragment cache contracts.

use pliego_css_compiler::{
    CssFragmentCache, emit_css_with_theme, lower_style_with_theme,
    try_encode_style_identity_with_theme,
};
use pliego_css_parser::parse_style_list;
use pliego_css_theme::ThemeRegistry;

fn compile(theme: &ThemeRegistry, source: &str) -> (Vec<u8>, pliego_css_ir::SemanticStyle) {
    let syntax = parse_style_list(source).expect("fixture syntax");
    let style = lower_style_with_theme(theme, &syntax).expect("fixture semantics");
    let stream = try_encode_style_identity_with_theme(theme, &style).expect("fixture identity");
    (stream, style)
}

#[test]
fn unchanged_stream_reuses_exact_fragment_and_stale_streams_are_pruned() {
    let theme = ThemeRegistry::seed();
    let (flex_stream, flex) = compile(&theme, "flex gap-4");
    let (grid_stream, grid) = compile(&theme, "grid gap-4");
    let mut cache = CssFragmentCache::default();

    let (cold, cold_hit) = cache
        .emit(&flex_stream, &theme, &flex)
        .expect("cold emission");
    assert!(!cold_hit);
    assert_eq!(
        cold,
        emit_css_with_theme(&theme, &flex).expect("ordinary emission")
    );
    let cold = cold.to_owned();

    let (warm, warm_hit) = cache
        .emit(&flex_stream, &theme, &flex)
        .expect("warm emission");
    assert!(warm_hit);
    assert_eq!(warm, cold);

    let (_, grid_hit) = cache
        .emit(&grid_stream, &theme, &grid)
        .expect("second emission");
    assert!(!grid_hit);
    assert_eq!(cache.retain([&grid_stream]), 1);

    let (_, rebuilt_hit) = cache
        .emit(&flex_stream, &theme, &flex)
        .expect("rebuilt emission");
    assert!(!rebuilt_hit);
}
