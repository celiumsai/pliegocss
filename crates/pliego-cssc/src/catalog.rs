use std::fs;
use std::path::Path;

use pliego_css_build::artifacts::{CatalogOutputFormat, render_catalog};

use super::{
    CatalogArgs, CatalogFormat, load_theme, resolve_theme_config, validate_path_roles,
    write_if_changed,
};

pub(crate) fn run_catalog(arguments: &CatalogArgs) -> Result<(), String> {
    let resolved_config = resolve_theme_config(
        arguments.config.as_deref(),
        arguments.seed,
        std::iter::empty::<&Path>(),
    )?;
    if let (Some(config), Some(output)) = (resolved_config.as_deref(), arguments.output.as_deref())
    {
        validate_path_roles(
            &[("resolved theme configuration", config)],
            &[("--output", output)],
        )?;
    }
    let theme = load_theme(resolved_config.as_deref())?;
    let format = match arguments.format {
        CatalogFormat::Markdown => CatalogOutputFormat::Markdown,
        CatalogFormat::Json => CatalogOutputFormat::Json,
    };
    let rendered = render_catalog(&theme, format)?;
    if let Some(check) = &arguments.check {
        check_catalog(check, rendered.as_bytes())
    } else if let Some(output) = &arguments.output {
        write_if_changed(output, rendered.as_bytes()).map(|_| ())
    } else {
        print!("{rendered}");
        Ok(())
    }
}

pub(crate) fn check_catalog(path: &Path, expected: &[u8]) -> Result<(), String> {
    let actual = fs::read(path)
        .map_err(|error| format!("cannot check catalog `{}`: {error}", path.display()))?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "catalog drift detected in `{}`; regenerate it with `pliego-cssc catalog --output` using the same theme and format options",
            path.display()
        ))
    }
}
