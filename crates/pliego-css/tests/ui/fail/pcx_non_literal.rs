use pliego_css::pcx;

const ACTIVE: &str = "bg-accent";

fn main() {
    let _ = pcx!(
        "rounded-md",
        if true { ACTIVE } else { "bg-transparent" },
    );
}
