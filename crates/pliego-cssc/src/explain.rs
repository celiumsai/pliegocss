use serde::Serialize;

use pliego_css_build::artifacts::{utility_domain_name, utility_form_name};
use pliego_css_cascade::{CascadeExplanation, CascadeStatus, explain_stylesheet_cascade};
use pliego_css_compiler::{
    STYLE_ID_FORMAT_VERSION, UtilityDescriptor, utility_descriptor_for_style_item,
};
use pliego_css_ir::{CLASS_NAME_FORMAT_VERSION, StyleItem};
use pliego_css_parser::{format_style_list, parse_style_list};
use pliego_css_theme::THEME_ID_FORMAT_VERSION;

use super::{
    Candidate, CascadeExplainArgs, CliFailure, CssFormat, EXPLAIN_SCHEMA_VERSION, ExplainArgs,
    ExplainFormat, TargetContract, audit_logical_path, cli_provenance, compile_candidates,
    load_theme, read_audit_artifact, resolve_theme_config, style_failure,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExplainedUtility {
    pub(crate) source: String,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) pattern: String,
    pub(crate) match_name: String,
    pub(crate) form: String,
    domain: String,
    pub(crate) capabilities: CatalogCapabilities,
    summary: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExplainDocument {
    schema_version: u8,
    style_id_format_version: u16,
    class_name_format_version: u16,
    theme_id_format_version: u16,
    source: String,
    pub(crate) canonical: String,
    theme_id: String,
    targets: TargetContract,
    pub(crate) style_id: String,
    pub(crate) class_name: String,
    pub(crate) css: String,
    pub(crate) utilities: Vec<ExplainedUtility>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct CatalogCapabilities {
    pub(crate) negative: bool,
    arbitrary_value: bool,
    custom_property: bool,
    pub(crate) modifier: bool,
}

fn descriptor_capabilities(descriptor: &UtilityDescriptor) -> CatalogCapabilities {
    CatalogCapabilities {
        negative: descriptor.allows_negative(),
        arbitrary_value: descriptor.accepts_arbitrary_value(),
        custom_property: descriptor.accepts_custom_property(),
        modifier: descriptor.accepts_modifier(),
    }
}

pub(crate) fn run_explain(arguments: &ExplainArgs) -> Result<(), CliFailure> {
    let document = build_explanation(arguments)?;
    match arguments.format {
        ExplainFormat::Json => {
            let json = serde_json::to_string_pretty(&document).map_err(|error| {
                CliFailure::tool(format!("cannot serialize explanation: {error}"))
            })?;
            println!("{json}");
        }
        ExplainFormat::Text => print_explanation(&document),
    }
    Ok(())
}

pub(crate) fn run_cascade_explain(arguments: &CascadeExplainArgs) -> Result<(), CliFailure> {
    let logical_path = audit_logical_path(&arguments.input).map_err(CliFailure::invalid)?;
    let bytes = read_audit_artifact(&arguments.input, "cascade input")?;
    let css = std::str::from_utf8(&bytes).map_err(|error| {
        CliFailure::invalid(format!(
            "cascade input `{}` is not valid UTF-8: {error}",
            arguments.input.display()
        ))
    })?;
    let explanation =
        explain_stylesheet_cascade(&logical_path, css, &arguments.element, &arguments.property)
            .map_err(|error| CliFailure::tool(error.to_string()))?;
    match arguments.format {
        ExplainFormat::Json => {
            let json = serde_json::to_string_pretty(&explanation).map_err(|error| {
                CliFailure::tool(format!("cannot serialize cascade explanation: {error}"))
            })?;
            println!("{json}");
        }
        ExplainFormat::Text => print_cascade_explanation(&explanation)?,
    }
    Ok(())
}

fn print_cascade_explanation(document: &CascadeExplanation) -> Result<(), CliFailure> {
    let status = match document.status {
        CascadeStatus::Resolved => "resolved",
        CascadeStatus::NoMatch => "no-match",
        CascadeStatus::BrowserRequired => "browser-required",
    };
    println!("Status: {status}");
    println!("Scope: {}", document.scope);
    println!("Input: {}", document.input);
    println!("Element: {}", document.element.selector);
    println!("Property: {}", document.property);
    if let Some(winner_id) = &document.winner {
        let winner = document
            .candidates
            .iter()
            .find(|candidate| &candidate.id == winner_id)
            .ok_or_else(|| {
                CliFailure::tool(format!(
                    "cascade explanation winner `{winner_id}` has no candidate record"
                ))
            })?;
        println!(
            "Winner: {} via {} at {}:{}:{}",
            winner.effective_declaration,
            winner.selector,
            winner.source.path,
            winner.source.start_line,
            winner.source.start_column
        );
    }
    println!("Candidates:");
    for candidate in &document.candidates {
        let criterion = candidate
            .decisive_criterion
            .as_deref()
            .map_or(String::new(), |value| format!(" by {value}"));
        println!(
            "  {} [{}{}] {} via {} ({})",
            candidate.id,
            candidate.disposition,
            criterion,
            candidate.effective_declaration,
            candidate.selector,
            candidate.specificity
        );
    }
    if !document.blockers.is_empty() {
        println!("Browser evidence required:");
        for blocker in &document.blockers {
            println!("  {}: {}", blocker.code, blocker.message);
        }
    }
    Ok(())
}

pub(crate) fn build_explanation(arguments: &ExplainArgs) -> Result<ExplainDocument, CliFailure> {
    let config = resolve_theme_config(
        arguments.config.as_deref(),
        arguments.seed,
        std::iter::empty(),
    )?;
    let theme = load_theme(config.as_deref())?;
    let provenance = cli_provenance(&arguments.style, "explain");
    let syntax = parse_style_list(&arguments.style).map_err(|error| {
        let human = error.to_string();
        style_failure(error, human, &provenance)
    })?;
    let canonical = format_style_list(&syntax);
    let artifact = compile_candidates(
        &theme,
        &[Candidate::Direct {
            style: arguments.style.clone(),
            provenance,
        }],
        false,
        arguments.targets,
        CssFormat::Pretty,
    )?;
    let style = artifact
        .styles
        .first()
        .expect("one explained candidate produces one style");
    let utilities = syntax
        .items
        .iter()
        .map(|item| explain_style_item(&arguments.style, item))
        .collect::<Result<Vec<_>, _>>()
        .map_err(CliFailure::tool)?;
    Ok(ExplainDocument {
        schema_version: EXPLAIN_SCHEMA_VERSION,
        style_id_format_version: STYLE_ID_FORMAT_VERSION,
        class_name_format_version: CLASS_NAME_FORMAT_VERSION,
        theme_id_format_version: THEME_ID_FORMAT_VERSION,
        source: arguments.style.clone(),
        canonical,
        theme_id: theme.id().to_string(),
        targets: arguments.targets,
        style_id: style.style_id.clone(),
        class_name: style.class_name.clone(),
        css: artifact.css,
        utilities,
    })
}

fn explain_style_item(source: &str, item: &StyleItem) -> Result<ExplainedUtility, String> {
    let descriptor = utility_descriptor_for_style_item(item)
        .ok_or_else(|| "compiler accepted a utility without catalog metadata".to_owned())?;
    let item_source = source
        .get(item.span.start..item.span.end)
        .ok_or_else(|| "parser returned an invalid utility source range".to_owned())?;
    Ok(ExplainedUtility {
        source: item_source.to_owned(),
        byte_start: item.span.start,
        byte_end: item.span.end,
        pattern: descriptor.pattern().to_owned(),
        match_name: descriptor.match_name().to_owned(),
        form: utility_form_name(descriptor.form()).to_owned(),
        domain: utility_domain_name(descriptor.domain()).to_owned(),
        capabilities: descriptor_capabilities(descriptor),
        summary: descriptor.summary().to_owned(),
    })
}

fn print_explanation(document: &ExplainDocument) {
    println!("Source: {}", document.source);
    println!("Canonical: {}", document.canonical);
    println!("Theme: {}", document.theme_id);
    println!("Style ID: {}", document.style_id);
    println!("Class: {}", document.class_name);
    println!("Utilities:");
    for utility in &document.utilities {
        println!(
            "  {} -> {} ({}, {}): {}",
            utility.source, utility.pattern, utility.form, utility.domain, utility.summary
        );
    }
    println!("CSS:\n{}", document.css.trim_end());
}
