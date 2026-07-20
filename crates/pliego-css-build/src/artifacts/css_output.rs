use lightningcss::rules::{CssRule, CssRuleList};
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::Targets;

use pliego_css_config::compatibility_data::CompatibilityProfile;

use super::sha256_hex;

const MAX_STANDARD_CSS_BYTES: usize = 16 * 1024 * 1024;

/// Deterministic printer mode for ordinary CSS transformation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardCssFormat {
    /// Compact production CSS.
    Minified,
    /// Stable readable CSS.
    Pretty,
}

/// Exact result of transforming one ordinary CSS artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandardCssOutput {
    css: String,
    source_sha256: String,
    output_sha256: String,
}

impl StandardCssOutput {
    /// Returns the transformed CSS, including one final LF.
    #[must_use]
    pub fn css(&self) -> &str {
        &self.css
    }

    /// Returns the SHA-256 of the exact input bytes.
    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    /// Returns the SHA-256 of the exact output bytes.
    #[must_use]
    pub fn output_sha256(&self) -> &str {
        &self.output_sha256
    }
}

/// Transforms ordinary CSS through the existing deterministic Lightning CSS boundary.
///
/// # Errors
///
/// Returns an error for unsafe logical paths, inputs over 16 MiB, or CSS parse/print failures.
pub fn transform_standard_css(
    logical_path: &str,
    css: &str,
    profile: CompatibilityProfile,
    format: StandardCssFormat,
) -> Result<StandardCssOutput, String> {
    validate_standard_css_path(logical_path)?;
    if css.len() > MAX_STANDARD_CSS_BYTES {
        return Err("standard CSS input exceeds 16 MiB".into());
    }
    let mut output = optimize_css(
        css,
        profile.lightning(),
        format == StandardCssFormat::Minified,
    )?;
    if !output.ends_with('\n') {
        output.push('\n');
    }
    Ok(StandardCssOutput {
        source_sha256: sha256_hex(css.as_bytes()),
        output_sha256: sha256_hex(output.as_bytes()),
        css: output,
    })
}

fn validate_standard_css_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        || path.as_bytes().get(1) == Some(&b':')
    {
        return Err("standard CSS path must be a portable relative path".into());
    }
    Ok(())
}

/// Reuses final CSS when raw input is unchanged within one fixed-settings run.
#[doc(hidden)]
#[derive(Default)]
pub struct FixedCssOutputCache {
    source: String,
    output: String,
    targets: Targets,
    minify: bool,
    initialized: bool,
    hits: usize,
}

impl FixedCssOutputCache {
    /// Optimizes changed input or returns the previous exact output.
    ///
    /// # Errors
    ///
    /// Returns an error when changed CSS cannot be parsed or printed by Lightning CSS.
    pub fn optimize(
        &mut self,
        css: &str,
        targets: Targets,
        minify: bool,
    ) -> Result<String, String> {
        if self.initialized
            && self.source == css
            && self.targets == targets
            && self.minify == minify
        {
            self.hits += 1;
            return Ok(self.output.clone());
        }
        let output = optimize_css(css, targets, minify)?;
        self.source.clear();
        self.source.push_str(css);
        self.output.clone_from(&output);
        self.targets = targets;
        self.minify = minify;
        self.initialized = true;
        Ok(output)
    }

    /// Returns the cumulative number of exact raw-input hits.
    #[must_use]
    pub const fn hits(&self) -> usize {
        self.hits
    }
}

/// Optimizes emitted CSS with deterministic adjacent-media merging.
///
/// # Errors
///
/// Returns an error when Lightning CSS cannot parse or print the stylesheet.
pub fn optimize_css(css: &str, targets: Targets, minify: bool) -> Result<String, String> {
    optimize_css_inner(css, targets, minify, false).map(|(css, _)| css)
}

/// Optimizes emitted CSS and returns the pre-target, pretty-printed trace input.
///
/// The trace input is captured after deterministic adjacent-media merging and before target
/// lowering, matching the physical-lineage contract consumed by `build_physical_projection`.
///
/// # Errors
///
/// Returns an error when Lightning CSS cannot parse or print either representation.
pub fn optimize_css_with_trace(
    css: &str,
    targets: Targets,
    minify: bool,
) -> Result<(String, String), String> {
    let (css, trace_input) = optimize_css_inner(css, targets, minify, true)?;
    Ok((
        css,
        trace_input.ok_or_else(|| "physical trace input is missing".to_owned())?,
    ))
}

fn optimize_css_inner(
    css: &str,
    targets: Targets,
    minify: bool,
    capture_trace_input: bool,
) -> Result<(String, Option<String>), String> {
    let mut stylesheet =
        StyleSheet::parse(css, ParserOptions::default()).map_err(|error| error.to_string())?;
    merge_adjacent_media_rules(&mut stylesheet.rules);
    let trace_input = capture_trace_input
        .then(|| {
            stylesheet.to_css(PrinterOptions {
                minify: false,
                targets: Targets::default(),
                ..PrinterOptions::default()
            })
        })
        .transpose()
        .map_err(|error| error.to_string())?
        .map(|result| result.code);
    let code = stylesheet
        .to_css(PrinterOptions {
            minify,
            targets,
            ..PrinterOptions::default()
        })
        .map(|result| result.code)
        .map_err(|error| error.to_string())?;
    Ok((code, trace_input))
}

fn merge_adjacent_media_rules(rules: &mut CssRuleList<'_>) {
    let mut merged = Vec::with_capacity(rules.0.len());
    for rule in std::mem::take(&mut rules.0) {
        match rule {
            CssRule::Media(next) => match merged.last_mut() {
                Some(CssRule::Media(previous)) if previous.query == next.query => {
                    previous.rules.0.extend(next.rules.0);
                }
                _ => merged.push(CssRule::Media(next)),
            },
            other => merged.push(other),
        }
    }
    rules.0 = merged;
    for rule in &mut rules.0 {
        if let CssRule::Media(media) = rule {
            merge_adjacent_media_rules(&mut media.rules);
        }
    }
}
