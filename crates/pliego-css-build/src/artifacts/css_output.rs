use lightningcss::rules::{CssRule, CssRuleList};
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::Targets;

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
