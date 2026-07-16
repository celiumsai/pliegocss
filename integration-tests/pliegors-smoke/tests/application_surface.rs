//! Public application-surface tests kept outside the product library source graph.

use pliego_css::pc;
use pliego_dom::{IntoView, el, render_html};
use pliego_macros::view;

#[test]
fn pc_style_flows_through_view_and_ssr() {
    let page = view! {
        <main class={pc!("grid gap-4 p-6")}>
            "Contenido"
        </main>
    };

    let html = render_html(&page);
    assert!(html.starts_with("<main class=\"pc_"));
    assert!(html.ends_with("\">Contenido</main>"));
}

#[test]
fn pc_style_converts_for_the_dom_builder() {
    let style = pc!("flex gap-4");
    let page = el("main")
        .class(String::from(style))
        .child("Contenido")
        .into_view();

    assert_eq!(
        render_html(&page),
        format!("<main class=\"{style}\">Contenido</main>")
    );
}
