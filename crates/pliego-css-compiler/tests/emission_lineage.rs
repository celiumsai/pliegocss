//! Black-box contracts for structured emitter lineage.

use pliego_css_compiler::{emit_css, emit_css_with_theme_traced, lower_style};
use pliego_css_ir::{SemanticStyle, Utility};
use pliego_css_parser::parse_style_list;
use pliego_css_theme::ThemeRegistry;

fn compile(source: &str) -> SemanticStyle {
    lower_style(&parse_style_list(source).expect("lineage fixture must parse"))
        .expect("lineage fixture must lower")
}

fn ordinal(style: &SemanticStyle, utility: Utility) -> u32 {
    let index = style
        .assignments
        .iter()
        .position(|assignment| assignment.utility == utility)
        .expect("fixture utility must have an assignment");
    u32::try_from(index).expect("fixture assignment ordinal must fit u32")
}

#[test]
fn traced_emission_is_byte_identical_and_keeps_qualified_rule_order() {
    let style = compile("block focus:ring-2");
    let expected = emit_css(&style).expect("ordinary CSS emission");
    let (actual, lineage) =
        emit_css_with_theme_traced(&ThemeRegistry::seed(), &style).expect("traced CSS emission");
    let display = ordinal(&style, Utility::Display);
    let ring = ordinal(&style, Utility::RingWidth);

    assert_eq!(actual, expected);
    assert_eq!(lineage.style_id, style.id);
    assert_eq!(lineage.rules.len(), 2);

    let base = &lineage.rules[0].declarations;
    assert_eq!(base.len(), 4);
    for initializer in &base[..3] {
        assert_eq!(initializer.semantic_ordinals, [ring]);
        assert!(initializer.generated);
    }
    assert_eq!(base[3].semantic_ordinals, [display]);
    assert!(!base[3].generated);

    let focus = &lineage.rules[1].declarations;
    assert_eq!(focus.len(), 2);
    assert_eq!(focus[0].semantic_ordinals, [ring]);
    assert!(!focus[0].generated);
    assert_eq!(focus[1].semantic_ordinals, [ring]);
    assert!(focus[1].generated);
}

#[test]
fn multi_output_assignments_repeat_their_canonical_ordinal() {
    let style = compile("text-base antialiased transition-colors border");
    let (_, lineage) =
        emit_css_with_theme_traced(&ThemeRegistry::seed(), &style).expect("traced CSS emission");
    let declarations = &lineage.rules[0].declarations;

    for (utility, expected_count) in [
        (Utility::FontSize, 2),
        (Utility::FontSmoothing, 2),
        (Utility::TransitionProperty, 3),
        (Utility::BorderWidth, 2),
    ] {
        let ordinal = ordinal(&style, utility);
        let outputs = declarations
            .iter()
            .filter(|declaration| declaration.semantic_ordinals == [ordinal])
            .collect::<Vec<_>>();
        assert_eq!(
            outputs.len(),
            expected_count,
            "unexpected {utility:?} fanout"
        );
        assert!(outputs.iter().all(|declaration| !declaration.generated));
    }
}

#[test]
fn composed_effects_record_all_contributors_as_generated() {
    let style = compile("focus:ring-2 focus:ring-accent/20 focus:shadow-md");
    let (_, lineage) =
        emit_css_with_theme_traced(&ThemeRegistry::seed(), &style).expect("traced CSS emission");
    let initializers = &lineage.rules[0].declarations;
    let effects = &lineage.rules[1].declarations;
    let effect_ordinals = style
        .assignments
        .iter()
        .enumerate()
        .filter(|(_, assignment)| {
            matches!(
                assignment.utility,
                Utility::BoxShadow | Utility::RingWidth | Utility::RingColor
            )
        })
        .map(|(ordinal, _)| u32::try_from(ordinal).expect("fixture ordinal fits u32"))
        .collect::<Vec<_>>();

    assert_eq!(lineage.rules.len(), 2);
    assert_eq!(initializers.len(), 3);
    for initializer in initializers {
        assert_eq!(initializer.semantic_ordinals, effect_ordinals);
        assert!(initializer.generated);
        assert!(!initializer.important);
    }
    assert_eq!(effects.len(), 4);
    let composed_shadow = effects.last().expect("composed box-shadow lineage");
    assert_eq!(composed_shadow.semantic_ordinals, effect_ordinals);
    assert!(composed_shadow.generated);
    assert!(!composed_shadow.important);
}

#[test]
fn important_is_repeated_across_every_direct_output() {
    let style = compile("transition-colors!");
    let (css, lineage) =
        emit_css_with_theme_traced(&ThemeRegistry::seed(), &style).expect("traced CSS emission");
    let declarations = &lineage.rules[0].declarations;

    assert_eq!(css.matches("!important").count(), 3);
    assert_eq!(declarations.len(), 3);
    assert!(declarations.iter().all(|declaration| declaration.important));
    assert!(
        declarations
            .iter()
            .all(|declaration| !declaration.generated)
    );
    assert!(
        declarations
            .iter()
            .all(|declaration| declaration.semantic_ordinals == [0])
    );
}
