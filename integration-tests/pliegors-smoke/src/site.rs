//! Two deterministic PliegoRS pages used to prove route CSS and resumability seams.

use crate::styles::{counter_style, global_style, home_style, visit_style};
use pliego_dom::{IntoView, View, el};
use pliego_macros::view;
use pliego_resume::{Island, ResumeError, increment, text_binding};

use crate::product::VISIT_COUNTER_NAME;

pub fn home() -> View {
    let class = format!("{} {}", global_style(), home_style());
    view! {
        <main class={class}>
            <h1>"PliegoCSS home"</h1>
            <a href="/visit/">"Open resumable visit route"</a>
        </main>
    }
}

pub fn visit() -> Result<View, ResumeError> {
    let page_class = format!("{} {}", global_style(), visit_style());
    let control_class = String::from(counter_style());
    let increment_button = increment(
        el("button")
            .class(control_class.clone())
            .attr("type", "button")
            .child("+5"),
        "minutes",
        5,
    )?;
    let value = text_binding("minutes", "15")?.class(control_class);
    let controls = el("section")
        .attr("aria-label", "Visit duration")
        .child(value)
        .child(increment_button)
        .into_view();
    let island = Island::new(VISIT_COUNTER_NAME)?
        .state_i64("minutes", 15)?
        .child(controls)
        .into_view()?;
    Ok(view! {
        <main class={page_class}>
            <a href="/">"Home"</a>
            <h1>"Resumable visit"</h1>
            {island}
        </main>
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pliego_dom::render_html;

    #[test]
    fn routes_keep_distinct_style_identities_and_only_visit_has_an_island() {
        let home_html = render_html(&home());
        let visit_html = render_html(&visit().expect("valid visit island"));
        let home_class = String::from(home_style());
        let visit_class = String::from(visit_style());
        let counter_class = String::from(counter_style());

        assert!(home_html.contains(&home_class));
        assert!(!home_html.contains(&visit_class));
        assert!(!home_html.contains("<pliego-island"));
        assert!(visit_html.contains(&visit_class));
        assert!(visit_html.contains(&counter_class));
        assert!(!visit_html.contains(&home_class));
        assert!(visit_html.contains("data-pliego-id=\"visit-counter\""));
        assert!(visit_html.contains("data-pliego-action=\"increment\""));
        assert!(visit_html.contains("data-pliego-bind-text=\"minutes\""));
    }
}
