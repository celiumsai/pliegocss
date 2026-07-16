//! Minimal compile-checked `PliegoCSS` example.

use pliego_css::{Style, pc};

fn card() -> Style {
    pc!(
        "flex flex-col gap-4 rounded-lg border border-line bg-surface p-6 \
         md:flex-row md:items-center hover:bg-surface-raised"
    )
}

fn main() {
    let style = card();
    println!("{}", style.class_name());
}
