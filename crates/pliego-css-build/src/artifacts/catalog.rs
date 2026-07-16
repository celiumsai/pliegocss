use lightningcss::targets::Targets;
use pliego_css_compiler::{
    STYLE_ID_FORMAT_VERSION, UtilityDescriptor, UtilityDomain, UtilityForm, emit_css_with_theme,
    lower_style_with_theme, utility_catalog,
};
use pliego_css_ir::CLASS_NAME_FORMAT_VERSION;
use pliego_css_parser::parse_style_list;
use pliego_css_theme::{THEME_ID_FORMAT_VERSION, ThemeRegistry};
use serde::Serialize;

use super::optimize_css;

const CATALOG_SCHEMA_VERSION: u8 = 3;

/// Serialized utility-catalog representation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CatalogOutputFormat {
    /// Generated Markdown reference.
    #[default]
    Markdown,
    /// Canonical pretty-printed JSON document.
    Json,
}

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
    name: String,
    min_width: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
struct CatalogCapabilities {
    negative: bool,
    arbitrary_value: bool,
    custom_property: bool,
    modifier: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogUtility {
    pattern: String,
    match_name: String,
    form: String,
    domain: String,
    capabilities: CatalogCapabilities,
    example: String,
    summary: String,
    css: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogDocument {
    schema_version: u8,
    style_id_format_version: u16,
    class_name_format_version: u16,
    theme_id_format_version: u16,
    theme: ThemeInspection,
    utilities: Vec<CatalogUtility>,
}

/// Renders the deterministic utility catalog for one exact theme registry.
///
/// # Errors
///
/// Returns an error when a compiler-owned catalog example cannot be parsed, lowered, emitted, or
/// optimized, or when the canonical JSON representation cannot be serialized.
pub fn render_catalog(
    theme: &ThemeRegistry,
    format: CatalogOutputFormat,
) -> Result<String, String> {
    let document = build_catalog_document(theme)?;
    match format {
        CatalogOutputFormat::Markdown => Ok(render_catalog_markdown(&document)),
        CatalogOutputFormat::Json => {
            let mut json = serde_json::to_string_pretty(&document)
                .map_err(|error| format!("cannot serialize utility catalog: {error}"))?;
            json.push('\n');
            Ok(json)
        }
    }
}

fn build_catalog_document(theme: &ThemeRegistry) -> Result<CatalogDocument, String> {
    let mut utilities = utility_catalog()
        .iter()
        .map(|descriptor| {
            let syntax = parse_style_list(descriptor.example()).map_err(|error| {
                format!(
                    "catalog example `{}` for `{}` does not parse: {error}",
                    descriptor.example(),
                    descriptor.pattern()
                )
            })?;
            let semantic = lower_style_with_theme(theme, &syntax).map_err(|error| {
                format!(
                    "catalog example `{}` for `{}` does not lower: {error}",
                    descriptor.example(),
                    descriptor.pattern()
                )
            })?;
            let emitted = emit_css_with_theme(theme, &semantic).map_err(|error| {
                format!(
                    "catalog example `{}` for `{}` does not emit: {error}",
                    descriptor.example(),
                    descriptor.pattern()
                )
            })?;
            let css = optimize_css(&emitted, Targets::default(), true).map_err(|error| {
                format!(
                    "catalog example `{}` for `{}` fails Lightning CSS: {error}",
                    descriptor.example(),
                    descriptor.pattern()
                )
            })?;
            Ok(CatalogUtility {
                pattern: descriptor.pattern().to_owned(),
                match_name: descriptor.match_name().to_owned(),
                form: utility_form_name(descriptor.form()).to_owned(),
                domain: utility_domain_name(descriptor.domain()).to_owned(),
                capabilities: descriptor_capabilities(descriptor),
                example: descriptor.example().to_owned(),
                summary: descriptor.summary().to_owned(),
                css,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    utilities.sort_by(|left, right| left.pattern.cmp(&right.pattern));
    Ok(CatalogDocument {
        schema_version: CATALOG_SCHEMA_VERSION,
        style_id_format_version: STYLE_ID_FORMAT_VERSION,
        class_name_format_version: CLASS_NAME_FORMAT_VERSION,
        theme_id_format_version: THEME_ID_FORMAT_VERSION,
        theme: catalog_theme(theme),
        utilities,
    })
}

fn catalog_theme(theme: &ThemeRegistry) -> ThemeInspection {
    let mut inspection = inspect_theme(theme);
    inspection.tokens.sort_by(|left, right| {
        (left.kind.as_str(), left.name.as_str()).cmp(&(right.kind.as_str(), right.name.as_str()))
    });
    inspection.breakpoints.sort_by(|left, right| {
        (left.name.as_str(), left.min_width.as_str())
            .cmp(&(right.name.as_str(), right.min_width.as_str()))
    });
    inspection
}

fn inspect_theme(theme: &ThemeRegistry) -> ThemeInspection {
    ThemeInspection {
        id: format!("{:032x}", theme.id().get()),
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
                name: breakpoint.name.clone(),
                min_width: breakpoint.min_width.clone(),
            })
            .collect(),
    }
}

/// Returns the frozen catalog spelling for one compiler utility form.
#[doc(hidden)]
#[must_use]
pub const fn utility_form_name(form: UtilityForm) -> &'static str {
    match form {
        UtilityForm::Fixed => "fixed",
        UtilityForm::Parameterized => "parameterized",
        UtilityForm::ArbitraryProperty => "arbitrary-property",
        _ => "unknown",
    }
}

/// Returns the frozen catalog spelling for one compiler utility domain.
#[doc(hidden)]
#[must_use]
pub const fn utility_domain_name(domain: UtilityDomain) -> &'static str {
    match domain {
        UtilityDomain::None => "none",
        UtilityDomain::ArbitraryProperty => "arbitrary-property",
        UtilityDomain::Color => "color",
        UtilityDomain::FontFamilyOrWeight => "font-family-or-weight",
        UtilityDomain::FontSizeOrColor => "font-size-or-color",
        UtilityDomain::GridColumnSpan => "grid-column-span",
        UtilityDomain::GridTemplate => "grid-template",
        UtilityDomain::LineHeight => "line-height",
        UtilityDomain::Margin => "margin",
        UtilityDomain::OpacityPercentage => "opacity-percentage",
        UtilityDomain::Radius => "radius",
        UtilityDomain::BorderWidthOrColor => "border-width-or-color",
        UtilityDomain::RingWidthOrColor => "ring-width-or-color",
        UtilityDomain::Shadow => "shadow",
        UtilityDomain::Size => "size",
        UtilityDomain::Spacing => "spacing",
        _ => "unknown",
    }
}

fn descriptor_capabilities(descriptor: &UtilityDescriptor) -> CatalogCapabilities {
    CatalogCapabilities {
        negative: descriptor.allows_negative(),
        arbitrary_value: descriptor.accepts_arbitrary_value(),
        custom_property: descriptor.accepts_custom_property(),
        modifier: descriptor.accepts_modifier(),
    }
}

fn render_catalog_markdown(document: &CatalogDocument) -> String {
    let mut output = String::new();
    output.push_str("<!-- Generated by pliego-cssc catalog. Do not edit manually. -->\n\n");
    output.push_str("# PliegoCSS utility catalog\n\n");
    output.push_str("Theme ID: <code>");
    output.push_str(&markdown_cell(&document.theme.id));
    output.push_str("</code>\n\nIdentity formats: StyleId <code>");
    output.push_str(&document.style_id_format_version.to_string());
    output.push_str("</code>, class name <code>");
    output.push_str(&document.class_name_format_version.to_string());
    output.push_str("</code>, ThemeId <code>");
    output.push_str(&document.theme_id_format_version.to_string());
    output.push_str("</code>\n\n## Utilities\n\n");
    output.push_str(
        "| Pattern | Match | Form | Domain | Capabilities | Example | CSS | Summary |\n|---|---|---|---|---|---|---|---|\n",
    );
    for utility in &document.utilities {
        output.push_str("| <code>");
        output.push_str(&markdown_cell(&utility.pattern));
        output.push_str("</code> | <code>");
        output.push_str(&markdown_cell(&utility.match_name));
        output.push_str("</code> | ");
        output.push_str(&markdown_cell(&utility.form));
        output.push_str(" | ");
        output.push_str(&markdown_cell(&utility.domain));
        output.push_str(" | ");
        output.push_str(&markdown_cell(&catalog_capabilities(&utility.capabilities)));
        output.push_str(" | <code>");
        output.push_str(&markdown_cell(&utility.example));
        output.push_str("</code> | <code>");
        output.push_str(&markdown_cell(&utility.css));
        output.push_str("</code> | ");
        output.push_str(&markdown_cell(&utility.summary));
        output.push_str(" |\n");
    }

    output.push_str("\n## Tokens\n\n| Kind | Name | Value |\n|---|---|---|\n");
    for token in &document.theme.tokens {
        output.push_str("| ");
        output.push_str(&markdown_cell(&token.kind));
        output.push_str(" | <code>");
        output.push_str(&markdown_cell(&token.name));
        output.push_str("</code> | <code>");
        output.push_str(&markdown_cell(&token.value));
        output.push_str("</code> |\n");
    }

    output.push_str("\n## Breakpoints\n\n| Name | Minimum width |\n|---|---|\n");
    for breakpoint in &document.theme.breakpoints {
        output.push_str("| <code>");
        output.push_str(&markdown_cell(&breakpoint.name));
        output.push_str("</code> | <code>");
        output.push_str(&markdown_cell(&breakpoint.min_width));
        output.push_str("</code> |\n");
    }
    output
}

fn catalog_capabilities(capabilities: &CatalogCapabilities) -> String {
    let mut labels = Vec::new();
    if capabilities.negative {
        labels.push("negative");
    }
    if capabilities.arbitrary_value {
        labels.push("arbitrary-value");
    }
    if capabilities.custom_property {
        labels.push("custom-property");
    }
    if capabilities.modifier {
        labels.push("modifier");
    }
    if labels.is_empty() {
        "none".to_owned()
    } else {
        labels.join(", ")
    }
}

fn markdown_cell(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('|', "&#124;")
        .replace('\r', "")
        .replace('\n', "<br>")
}
