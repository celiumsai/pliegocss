use pliego_css::{Style, pc};
use pliego_ssg::ProductComponent;

pub fn component() -> ProductComponent {
    pliego_ssg::product_component!("pliegors-smoke::dead")
}

#[allow(dead_code)]
pub fn retired_style() -> Style {
    pc!("flex")
}
