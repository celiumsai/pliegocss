use pliego_css::{Style, pc};

fn main() {
    let styles: [Style; 4] = [
        pc!("flex items-center gap-4"),
        pc!("grid-cols-[1fr 300px]"),
        pc!("bg-[oklch(62% 0.2 25)]"),
        pc!("hover:[mask-type:alpha]"),
    ];
    assert_eq!(styles.len(), 4);
}
