//! PliegoRS product registry shared by SSG and PliegoCSS build adapters.

use pliego_ssg::{ProductIsland, ProductRegistry, ProductRoute};

use crate::styles::{
    counter_component, dead_component, global_component, home_component, visit_component,
};

pub const HOME_ROUTE_ID: &str = "home";
pub const VISIT_ROUTE_ID: &str = "visit";
pub const VISIT_COUNTER_ID: &str = "visit-counter";
pub const VISIT_COUNTER_NAME: &str = "visit-counter";

/// Returns the complete product-owned component, route, island, and Cargo-source registry.
#[must_use]
pub fn application_registry() -> ProductRegistry {
    let counter = counter_component();
    let dead = dead_component();
    let global = global_component();
    let home = home_component();
    let visit = visit_component();
    let counter_id = counter.id().to_owned();
    let global_id = global.id().to_owned();
    let home_id = home.id().to_owned();
    let visit_id = visit.id().to_owned();

    ProductRegistry::new()
        .component(counter)
        .component(dead)
        .component(global)
        .component(home)
        .component(visit)
        .island(
            ProductIsland::new(VISIT_COUNTER_ID, VISIT_COUNTER_NAME).component(counter_id),
        )
        .route(
            ProductRoute::new(HOME_ROUTE_ID, "/")
                .component(global_id.clone())
                .component(home_id),
        )
        .route(
            ProductRoute::new(VISIT_ROUTE_ID, "/visit")
                .component(global_id)
                .component(visit_id)
                .island(VISIT_COUNTER_ID),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_registry_is_complete_and_cargo_bound() {
        let registry = application_registry();
        registry.validate().unwrap();
        assert_eq!(registry.components().len(), 5);
        assert_eq!(registry.routes().len(), 2);
        assert_eq!(registry.islands().len(), 1);
        assert!(registry.components().iter().all(|component| {
            component.source_units().len() == 1
                && component.source_units()[0].starts_with("src/styles/")
        }));
    }
}
