use pliego_css::{Style, pc};
use pliego_ssg::ProductComponent;

pub fn component() -> ProductComponent {
    pliego_ssg::product_component!("pliegors-smoke::global")
}

pub fn global_style() -> Style {
    pc!("bg-canvas text-ink font-sans antialiased")
}
