//! Emits one PliegoRS SSR fragment for the cross-repository extraction gate.

use pliego_css::pc;
use pliego_dom::render_html;
use pliego_macros::view;

fn main() {
    let page = view! {
        <main class={pc!("grid gap-4 p-6")}>
            "Contenido"
        </main>
    };
    println!("{}", render_html(&page));
}

#[cfg(test)]
mod tests {
    use super::*;
    use pliego_css::pcx;
    use pliego_dom::{IntoView, View, el};

    #[test]
    fn view_macro_and_dom_builder_render_the_same_style_identity() {
        let style = pc!("flex items-center gap-2");
        let from_view: View = view! { <div class={style}>"same"</div> };
        let from_builder = el("div")
            .class(String::from(style))
            .child("same")
            .into_view();
        assert_eq!(render_html(&from_view), render_html(&from_builder));
    }

    #[test]
    fn conditional_branches_render_precompiled_distinct_classes() {
        let render = |active| {
            let class = pcx!(
                "rounded-md",
                if active { "bg-accent" } else { "bg-surface" },
            );
            render_html(&view! { <button class={class}>"state"</button> })
        };
        let active = render(true);
        let inactive = render(false);
        assert_ne!(active, inactive);
        assert!(active.contains("class=\"pc_"));
        assert!(inactive.contains("class=\"pc_"));
    }
}
