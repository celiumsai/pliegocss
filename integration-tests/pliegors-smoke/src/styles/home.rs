use pliego_css::{Style, pc};
use pliego_ssg::ProductComponent;

pub fn component() -> ProductComponent {
    pliego_ssg::product_component!("pliegors-smoke::home")
}

pub fn home_style() -> Style {
    pc!("grid gap-6 p-6 md:grid-cols-2")
}
