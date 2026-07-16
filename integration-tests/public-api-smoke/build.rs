fn require_unit(_: ()) {}

fn main() {
    require_unit(pliego_css_build::theme!(
        tokens = "product.resolver.json",
        inputs = {
            "appearance" => "dark",
            "channel" => "light",
        },
    ));
}
