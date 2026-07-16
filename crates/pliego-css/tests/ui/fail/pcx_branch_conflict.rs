use pliego_css::pcx;

fn main() {
    let _ = pcx!(
        "rounded-md",
        if true { "flex grid" } else { "inline-flex" },
    );
}
