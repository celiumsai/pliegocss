fn format_drift(active: bool) {
    let _ = pc!(" flex   gap-4 ");
    let _ = pcx!("grid  gap-4", if active { " block " } else { "hidden" });
}
