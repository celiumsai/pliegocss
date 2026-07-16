use pliego_css::pcx;

fn main() {
    let first = true;
    let second = false;
    let _ = pcx!(
        "block",
        if first { "opacity-50" } else { "opacity-100" },
        if second { "opacity-100" } else { "opacity-50" },
    );
}
