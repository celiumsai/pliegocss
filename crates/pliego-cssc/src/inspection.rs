use serde::Serialize;

use pliego_css_build::artifacts::sha256_hex;
use pliego_css_compiler::STYLE_ID_FORMAT_VERSION;
use pliego_css_ir::CLASS_NAME_FORMAT_VERSION;
use pliego_css_theme::{THEME_ID_FORMAT_VERSION, ThemeRegistry};

use super::{
    CompiledArtifact, CssFormat, INSPECTION_SCHEMA_VERSION, ManifestStyle, Provenance,
    TargetContract,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ThemeInspection {
    id: String,
    tokens: Vec<TokenInspection>,
    breakpoints: Vec<BreakpointInspection>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TokenInspection {
    kind: String,
    name: String,
    value: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BreakpointInspection {
    cascade_rank: u16,
    name: String,
    min_width: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Inspection {
    schema_version: u8,
    style_id_format_version: u16,
    class_name_format_version: u16,
    theme_id_format_version: u16,
    theme: ThemeInspection,
    targets: TargetContract,
    format: CssFormat,
    css_sha256: String,
    css_bytes: usize,
    findings: Vec<Provenance>,
    styles: Vec<ManifestStyle>,
}

fn inspect_theme(theme: &ThemeRegistry) -> ThemeInspection {
    ThemeInspection {
        id: theme.id().to_string(),
        tokens: theme
            .tokens()
            .iter()
            .map(|token| TokenInspection {
                kind: format!("{:?}", token.kind),
                name: token.name.clone(),
                value: token.value.clone(),
            })
            .collect(),
        breakpoints: theme
            .breakpoints()
            .iter()
            .map(|breakpoint| BreakpointInspection {
                cascade_rank: breakpoint.cascade_rank,
                name: breakpoint.name.clone(),
                min_width: breakpoint.min_width.clone(),
            })
            .collect(),
    }
}

pub(crate) fn serialize_inspection(
    theme: &ThemeRegistry,
    targets: TargetContract,
    artifact: &CompiledArtifact,
) -> Result<String, String> {
    let inspection = Inspection {
        schema_version: INSPECTION_SCHEMA_VERSION,
        style_id_format_version: STYLE_ID_FORMAT_VERSION,
        class_name_format_version: CLASS_NAME_FORMAT_VERSION,
        theme_id_format_version: THEME_ID_FORMAT_VERSION,
        theme: inspect_theme(theme),
        targets,
        format: artifact.format,
        css_sha256: sha256_hex(artifact.css.as_bytes()),
        css_bytes: artifact.css.len(),
        findings: artifact.findings.clone(),
        styles: artifact.styles.clone(),
    };
    let mut json = serde_json::to_string_pretty(&inspection)
        .map_err(|error| format!("cannot serialize inspection: {error}"))?;
    json.push('\n');
    Ok(json)
}
