use pliego_css::pcx;

fn main() {
    let flag = true;
    let _ = pcx!(
        "p-4",
        if flag { "opacity-50" } else { "opacity-100" },
        if flag { "text-sm" } else { "text-lg" },
        if flag { "font-normal" } else { "font-bold" },
        if flag { "bg-white" } else { "bg-accent" },
        if flag { "rounded-sm" } else { "rounded-lg" },
        if flag { "shadow-sm" } else { "shadow-md" },
        if flag { "block" } else { "hidden" },
    );
}
