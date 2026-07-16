use pliego_css::{Style, pc};
use pliego_ssg::ProductComponent;

pub fn component() -> ProductComponent {
    pliego_ssg::product_component!("pliegors-smoke::counter")
}

pub fn counter_style() -> Style {
    pc!(
        "inline-flex items-center gap-2 rounded-lg bg-accent px-4 py-2 \
         text-white hover:bg-accent-strong"
    )
}
