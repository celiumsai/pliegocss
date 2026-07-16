//! Minimal two-process development fixture for `pliego-cssc watch` plus `pliego dev`.

use pliego_css::{Style, pc};
use pliego_dom::{IntoView, el};
use pliego_ssg::{Asset, Head, Page, Site};
use std::path::PathBuf;

const PROBE_STYLE: &str = "p-4";
const PAGE_STYLE: Style = pc!("flex p-4 bg-surface text-ink");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/site"));
    let css = std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/pliego.css"))?;
    let page = el("main")
        .id("style-probe")
        .attr("data-style-probe", PROBE_STYLE)
        .class(String::from(PAGE_STYLE))
        .child(el("h1").child("PliegoCSS live development loop"))
        .child(el("p").child(format!("Current spacing utility: {PROBE_STYLE}")))
        .into_view();

    Site::new()
        .page(Page::new(
            "/",
            Head::new("PliegoCSS development loop").stylesheet("/assets/pliego.css"),
            page,
        ))
        .asset(Asset::new("assets/pliego.css", css))
        .build(output)?;
    Ok(())
}
