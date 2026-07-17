//! `pliego-css-lsp` stdio process entry point.

fn main() {
    if let Err(error) = pliego_css_lsp::run_from_env() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
