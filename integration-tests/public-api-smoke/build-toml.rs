fn require_unit(_: ()) {}

fn main() {
    require_unit(pliego_css_build::theme!("pliego.theme.toml"));
}
