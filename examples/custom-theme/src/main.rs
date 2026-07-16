//! Minimal custom-theme application.

use pliego_css::{Style, pc};

const PANEL: Style = pc!("tablet:grid gap-gutter bg-brand p-gutter rounded-xl");

fn main() {
    println!("{PANEL}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_tokens_and_breakpoint_compile_into_one_static_identity() {
        assert!(!PANEL.is_empty());
        assert!(PANEL.to_string().starts_with("pc_"));
    }
}
