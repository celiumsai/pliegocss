//! Public emission contracts spanning parsing, lowering, identity, and CSS output.

//! Black-box contracts for deterministic semantic CSS emission.

use pliego_css_compiler::{EmitError, class_name, emit_css, emit_seed_theme, lower_style};
use pliego_css_ir::{
    Assignment, SemanticStyle, SemanticValue, SlotSet, Span, StyleId, TokenId, TokenKind, TokenRef,
    Utility,
};
use pliego_css_parser::parse_style_list;

fn compile(source: &str) -> SemanticStyle {
    let syntax = parse_style_list(source).expect("contract fixture must parse");
    lower_style(&syntax).expect("contract fixture must lower")
}

fn css(source: &str) -> String {
    emit_css(&compile(source)).expect("contract fixture must emit")
}

#[test]
fn semantic_reordering_is_byte_deterministic() {
    let left = css("p-4 px-2 flex gap-3");
    let right = css("gap-3 flex px-2 p-4");

    assert_eq!(left, right);
    assert!(left.contains("display:flex;"));
    assert!(left.contains("padding:1rem;padding-right:.5rem;padding-left:.5rem;"));
}

#[test]
fn commuting_variant_order_has_one_identity_and_output() {
    let left = compile("md:dark:hover:bg-accent/20");
    let right = compile("hover:dark:md:bg-accent/20");

    assert_eq!(left.id, right.id);
    assert_eq!(emit_css(&left), emit_css(&right));
}

#[test]
fn base_hidden_and_responsive_flex_share_one_class() {
    let style = compile("hidden md:flex");
    let class = class_name(style.id);
    let emitted = emit_css(&style).expect("responsive display must emit");

    assert_eq!(
        emitted,
        format!(".{class}{{display:none;}}@media (min-width:48rem){{.{class}{{display:flex;}}}}")
    );
}

#[test]
fn theme_motion_contrast_and_state_are_structured() {
    let emitted = css("dark:motion-reduce:contrast-more:hover:bg-accent");

    assert!(emitted.starts_with(
        "@media (prefers-reduced-motion:reduce) and (prefers-contrast:more){[data-theme=dark] .pc_"
    ));
    assert!(emitted.contains(":hover{background-color:var(--color-accent);}"));
    assert!(emitted.ends_with("}}"));
}

#[test]
fn negative_spacing_uses_a_css_calculation() {
    let emitted = css("-mt-4");

    assert!(emitted.contains("{margin-top:calc(1rem*-1);}"));
}

#[test]
fn token_alpha_is_preserved_in_emitted_color() {
    let emitted = css("bg-accent/20");

    assert!(
        emitted
            .contains("background-color:color-mix(in oklab,var(--color-accent) 20%,transparent);")
    );
}

#[test]
fn arbitrary_values_and_properties_survive_lowering() {
    let emitted = css("w-[calc(100%-1rem)] [mask-type:luminance]");

    assert!(emitted.contains("width:calc(100%-1rem);"));
    assert!(emitted.contains("mask-type:luminance;"));
}

#[test]
fn ring_and_shadow_compose_into_one_box_shadow() {
    let emitted = css("ring-2 ring-accent/20 shadow-md");

    assert!(emitted.contains("--pc-shadow:0 4px 6px -1px #0000001a;"));
    assert!(emitted.contains("--pc-ring-width:2px;"));
    assert!(
        emitted
            .contains("--pc-ring-color:color-mix(in oklab,var(--color-accent) 20%,transparent);")
    );
    assert!(emitted.contains(
        "box-shadow:0 0 0 var(--pc-ring-width,0) var(--pc-ring-color,currentColor),var(--pc-shadow,0 0 #0000);"
    ));
}

#[test]
fn style_ids_encode_as_css_safe_classes() {
    for id in [
        StyleId::new(1),
        StyleId::new(35),
        StyleId::new(36),
        StyleId::new(u128::MAX),
    ] {
        let class = class_name(id);
        let body = class.strip_prefix("pc_").expect("class prefix");

        assert!(!body.is_empty());
        assert!(
            body.bytes()
                .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase())
        );
    }
}

#[test]
fn unresolved_style_ids_fail_closed() {
    assert_eq!(
        emit_css(&SemanticStyle::default()),
        Err(EmitError::UnresolvedStyleId)
    );
}

#[test]
fn unknown_token_ids_report_their_namespace() {
    let mut style = SemanticStyle::new(StyleId::new(7));
    style.assignments.push(Assignment {
        utility: Utility::Padding,
        slots: SlotSet::PADDING,
        value: SemanticValue::Token(TokenRef {
            kind: TokenKind::Spacing,
            id: TokenId::new(u32::MAX),
        }),
        negative: false,
        condition: SemanticStyle::base_condition(),
        important: false,
        source: Span::new(0, 3),
    });

    assert_eq!(
        emit_css(&style),
        Err(EmitError::UnknownToken {
            kind: TokenKind::Spacing,
            id: u32::MAX,
        })
    );
}

#[test]
fn selector_transforms_are_exact_linear_ordered_and_revalidated() {
    let child = compile("[&>svg]:block");
    assert_eq!(
        emit_css(&child).expect("child selector must emit"),
        format!(".{}>svg{{display:block;}}", class_name(child.id))
    );
    let attribute = compile("[&[data-state=open]]:block");
    assert_eq!(
        emit_css(&attribute).expect("attribute selector must emit"),
        format!(
            ".{}[data-state=open]{{display:block;}}",
            class_name(attribute.id)
        )
    );
    let quoted = compile(r#"[&[data-x="&&"]]:block"#);
    assert_eq!(
        emit_css(&quoted).expect("quoted selector must emit"),
        format!(
            r#".{}[data-x="&&"]{{display:block;}}"#,
            class_name(quoted.id)
        )
    );

    let forward = css("[&>svg]:[& path]:block");
    let reverse = css("[& path]:[&>svg]:block");
    assert!(forward.contains(">svg path{display:block;}"));
    assert!(reverse.contains(" path>svg{display:block;}"));
    assert_ne!(forward, reverse);

    assert_eq!(
        css("[&[data-state=open]]:md:hover:block"),
        css("hover:md:[&[data-state=open]]:block")
    );
    assert_eq!(
        css("[&>svg]:block [&>path]:hidden"),
        css("[&>path]:hidden [&>svg]:block")
    );

    let source = format!("{}block", "[&x]:".repeat(4_096));
    let style = compile(&source);
    assert_eq!(
        emit_css(&style).expect("long selector chain must emit"),
        format!(
            ".{}{}{{display:block;}}",
            class_name(style.id),
            "x".repeat(4_096)
        )
    );

    let mut invalid = compile("[&>p]:block");
    invalid.selectors[0] = "&&".into();
    assert!(matches!(emit_css(&invalid), Err(EmitError::InvalidIr(_))));
}

#[test]
fn core_layout_fixture_emits_every_physical_declaration() {
    let emitted = css(
        "flex-1 min-w-0 pb-4 border-b tracking-tight cursor-pointer justify-end \
         antialiased transition-colors aspect-square outline-none resize-y",
    );
    for declaration in [
        "min-width:0;",
        "padding-bottom:1rem;",
        "flex-grow:1;",
        "flex-shrink:1;",
        "flex-basis:0%;",
        "justify-content:end;",
        "letter-spacing:-.025em;",
        "border-bottom-width:1px;",
        "border-bottom-style:solid;",
        "cursor:pointer;",
        "-webkit-font-smoothing:antialiased;",
        "-moz-osx-font-smoothing:grayscale;",
        "aspect-ratio:1/1;",
        "outline-style:none;",
        "resize:vertical;",
        "transition-duration:.15s;",
    ] {
        assert!(
            emitted.contains(declaration),
            "missing `{declaration}` in {emitted}"
        );
    }
}

#[test]
fn conditional_effects_initialize_before_state_rules() {
    let style = compile("focus:ring-2 focus:ring-accent");
    let class = class_name(style.id);
    let emitted = emit_css(&style).expect("conditional effects must emit");

    assert!(emitted.starts_with(&format!(
        ".{class}{{--pc-shadow:0 0 #0000;--pc-ring-width:0;--pc-ring-color:currentColor;}}"
    )));
    assert!(emitted.contains(&format!(".{class}:focus{{")));
}

#[test]
fn extended_fixture_and_seed_theme_are_deterministic() {
    let emitted = css("focus-visible:outline-2 focus-visible:outline-offset-2 \
         focus-visible:outline-accent placeholder:text-muted");
    assert!(emitted.contains("::placeholder{color:var(--color-muted);}"));
    assert!(emitted.contains("outline-width:2px;outline-style:solid;"));
    assert!(emitted.contains("outline-color:var(--color-accent);"));
    assert!(emitted.contains("outline-offset:2px;"));
    assert_eq!(emit_seed_theme(), emit_seed_theme());
    assert!(emit_seed_theme().contains("--color-accent:"));
}
