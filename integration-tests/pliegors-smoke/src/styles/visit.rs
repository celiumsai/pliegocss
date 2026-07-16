use pliego_css::{Style, pc};
use pliego_ssg::ProductComponent;

pub fn component() -> ProductComponent {
    pliego_ssg::product_component!("pliegors-smoke::visit")
}

pub fn visit_style() -> Style {
    pc!("flex flex-col gap-4 bg-surface p-6")
}
