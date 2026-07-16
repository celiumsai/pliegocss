use pliego_css::{Style, pcx};

enum Variant {
    Primary,
    Ghost,
}

fn main() {
    let selected = false;
    let from_if: Style = pcx!(
        "inline-flex rounded-md bg-surface",
        if selected {
            "bg-accent text-white"
        } else {
            "text-ink"
        },
    );
    let from_match: Style = pcx!(
        "rounded-md",
        match Variant::Primary {
            Variant::Primary => "bg-accent text-white",
            Variant::Ghost => "bg-transparent text-accent",
        },
    );
    let from_multiple: Style = pcx!(
        "rounded-md",
        if selected {
            "opacity-100"
        } else {
            "opacity-50"
        },
        match Variant::Ghost {
            Variant::Primary => "text-sm",
            Variant::Ghost => "text-lg",
        },
    );
    let different_conditions: Style = pcx!(
        "block",
        if selected {
            "hover:opacity-100"
        } else {
            "hover:opacity-50"
        },
        if selected {
            "focus:opacity-100"
        } else {
            "focus:opacity-50"
        },
    );
    let _ = (
        from_if,
        from_match,
        from_multiple,
        different_conditions,
        Variant::Ghost,
    );
}
