use pliego_css::{Style, pc, pcx};

const SHELL: Style = pc!(
    "grid gap-4 rounded-lg border border-line bg-surface p-6 \
     md:grid-cols-2 dark:bg-ink dark:text-white"
);
const PRIMARY: Style = pc!(
    "inline-flex items-center justify-center rounded-lg bg-accent px-4 py-2 \
     font-semibold text-white hover:bg-accent-strong focus-visible:outline-2"
);

fn action(disabled: bool) -> Style {
    pcx!(
        "inline-flex",
        if disabled {
            "cursor-not-allowed opacity-50"
        } else {
            "cursor-pointer"
        },
    )
}

fn main() {
    let styles = [SHELL, PRIMARY, action(false)];
    assert!(styles.iter().all(|style| !style.is_empty()));
    println!("{} {} {}", styles[0], styles[1], styles[2]);
}
