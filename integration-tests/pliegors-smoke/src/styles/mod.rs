//! Route-partitioned PliegoCSS style entry points for the SSG integration gate.

mod counter;
mod dead;
mod global;
mod home;
mod visit;

pub use counter::{component as counter_component, counter_style};
pub use dead::component as dead_component;
pub use global::{component as global_component, global_style};
pub use home::{component as home_component, home_style};
pub use visit::{component as visit_component, visit_style};
