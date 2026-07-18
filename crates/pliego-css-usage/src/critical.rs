use std::collections::{BTreeMap, BTreeSet};

use pliego_css_build::artifacts::{sha256_hex, validate_asset_bundle_id};
use serde::{Deserialize, Serialize};

use super::{MAX_USAGE_DOCUMENT_BYTES, UsageSelection};

/// Wire version of explicit initial-render style evidence.
pub const CRITICAL_EVIDENCE_SCHEMA_VERSION: u8 = 1;
/// Wire version of the generated critical-CSS manifest.
pub const CRITICAL_CSS_MANIFEST_SCHEMA_VERSION: u8 = 1;
/// Fixed adjacent filename for the generated critical-CSS manifest.
pub const CRITICAL_CSS_MANIFEST_FILE: &str = "pliego.critical.json";

const EVIDENCE_KIND: &str = "pliegocss-critical-style-capture/1";
const INVALID: &str = "invalid critical style evidence";
const MAX_ITEMS: usize = 65_535;
const MAX_TEXT: usize = 4_096;

/// One bundle-qualified `StyleId` positively observed during an initial-render capture.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CriticalStyleInput {
    bundle_id: String,
    style_id: String,
}

impl CriticalStyleInput {
    /// Creates one positive critical-style observation.
    #[must_use]
    pub fn new(bundle_id: impl Into<String>, style_id: impl Into<String>) -> Self {
        Self {
            bundle_id: bundle_id.into(),
            style_id: style_id.into(),
        }
    }
}

/// Paint boundary at which a producer sampled positive styles.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CriticalCaptureStage {
    /// Browser First Contentful Paint boundary.
    FirstContentfulPaint,
    /// Browser Largest Contentful Paint boundary.
    LargestContentfulPaint,
    /// A named application-adapter readiness boundary.
    AdapterReady,
}

/// One finite browser/viewport initial-render capture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriticalCaptureInput {
    id: String,
    route_id: String,
    route_path: String,
    browser: String,
    viewport_width: u32,
    viewport_height: u32,
    stage: CriticalCaptureStage,
    styles: Vec<CriticalStyleInput>,
}

impl CriticalCaptureInput {
    /// Creates one capture. Style ordering is canonicalized; duplicates fail.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        route_id: impl Into<String>,
        route_path: impl Into<String>,
        browser: impl Into<String>,
        viewport_width: u32,
        viewport_height: u32,
        stage: CriticalCaptureStage,
        styles: Vec<CriticalStyleInput>,
    ) -> Self {
        Self {
            id: id.into(),
            route_id: route_id.into(),
            route_path: route_path.into(),
            browser: browser.into(),
            viewport_width,
            viewport_height,
            stage,
            styles,
        }
    }
}

/// Adapter-facing input for canonical critical-style evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriticalEvidenceInput {
    universe_sha256: String,
    reachability_sha256: String,
    producer_name: String,
    producer_version: String,
    unknown_dynamic_inputs: Vec<String>,
    captures: Vec<CriticalCaptureInput>,
}

impl CriticalEvidenceInput {
    /// Creates evidence bound to one exact usage universe and reachability document.
    #[must_use]
    pub fn new(
        universe_sha256: impl Into<String>,
        reachability_sha256: impl Into<String>,
        producer_name: impl Into<String>,
        producer_version: impl Into<String>,
        unknown_dynamic_inputs: Vec<String>,
        captures: Vec<CriticalCaptureInput>,
    ) -> Self {
        Self {
            universe_sha256: universe_sha256.into(),
            reachability_sha256: reachability_sha256.into(),
            producer_name: producer_name.into(),
            producer_version: producer_version.into(),
            unknown_dynamic_inputs,
            captures,
        }
    }
}

/// One expected route and its selected bundle set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriticalRouteInput {
    id: String,
    path: String,
    bundles: BTreeSet<String>,
}

impl CriticalRouteInput {
    /// Creates one route validation boundary from an Asset Plan.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        path: impl Into<String>,
        bundles: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            bundles: bundles.into_iter().collect(),
        }
    }
}

/// Validated route-to-style union derived from exact capture evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriticalSelection {
    evidence_sha256: String,
    routes: BTreeMap<String, BTreeSet<(String, String)>>,
}

impl CriticalSelection {
    /// Returns the exact raw evidence digest.
    #[must_use]
    pub fn evidence_sha256(&self) -> &str {
        &self.evidence_sha256
    }

    /// Iterates canonical route IDs with their bundle-qualified `StyleId` union.
    pub fn routes(&self) -> impl Iterator<Item = (&str, &BTreeSet<(String, String)>)> {
        self.routes.iter().map(|(id, styles)| (id.as_str(), styles))
    }

    /// Returns the union selected for one route.
    #[must_use]
    pub fn styles(&self, route_id: &str) -> Option<&BTreeSet<(String, String)>> {
        self.routes.get(route_id)
    }
}

/// One generated critical CSS route artifact supplied to the canonical manifest builder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriticalCssRouteInput {
    route_id: String,
    route_path: String,
    css_file: String,
    css: Vec<u8>,
    styles: Vec<(String, String)>,
}

impl CriticalCssRouteInput {
    /// Creates one integrity-bound route artifact input.
    #[must_use]
    pub fn new(
        route_id: impl Into<String>,
        route_path: impl Into<String>,
        css_file: impl Into<String>,
        css: Vec<u8>,
        styles: Vec<(String, String)>,
    ) -> Self {
        Self {
            route_id: route_id.into(),
            route_path: route_path.into(),
            css_file: css_file.into(),
            css,
            styles,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CriticalEvidence {
    schema_version: u8,
    evidence_kind: String,
    universe_sha256: String,
    reachability_sha256: String,
    producer: Producer,
    unknown_dynamic_inputs: Vec<String>,
    captures: Vec<Capture>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Producer {
    name: String,
    version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Capture {
    id: String,
    route_id: String,
    route_path: String,
    browser: String,
    viewport_width: u32,
    viewport_height: u32,
    stage: CriticalCaptureStage,
    styles: Vec<CriticalStyle>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CriticalStyle {
    bundle_id: String,
    style_id: String,
}

#[derive(Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CriticalCssManifest {
    schema_version: u8,
    manifest_kind: String,
    evidence_sha256: String,
    universe_sha256: String,
    reachability_sha256: String,
    theme_id: String,
    targets: String,
    format: String,
    routes: Vec<CriticalCssRoute>,
}

#[derive(Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CriticalCssRoute {
    route_id: String,
    route_path: String,
    css_file: String,
    css_bytes: usize,
    css_sha256: String,
    styles: Vec<CriticalStyle>,
}

/// Builds canonical critical-style evidence bytes.
///
/// # Errors
///
/// Returns an error for invalid identities, duplicates, bounds, or hashes.
pub fn build_critical_evidence(input: CriticalEvidenceInput) -> Result<Vec<u8>, String> {
    let mut document = CriticalEvidence {
        schema_version: CRITICAL_EVIDENCE_SCHEMA_VERSION,
        evidence_kind: EVIDENCE_KIND.into(),
        universe_sha256: input.universe_sha256,
        reachability_sha256: input.reachability_sha256,
        producer: Producer {
            name: input.producer_name,
            version: input.producer_version,
        },
        unknown_dynamic_inputs: input.unknown_dynamic_inputs,
        captures: input
            .captures
            .into_iter()
            .map(|capture| Capture {
                id: capture.id,
                route_id: capture.route_id,
                route_path: capture.route_path,
                browser: capture.browser,
                viewport_width: capture.viewport_width,
                viewport_height: capture.viewport_height,
                stage: capture.stage,
                styles: capture
                    .styles
                    .into_iter()
                    .map(|style| CriticalStyle {
                        bundle_id: style.bundle_id,
                        style_id: style.style_id,
                    })
                    .collect(),
            })
            .collect(),
    };
    document.canonicalize()?;
    encode(&document)
}

/// Parses canonical critical-style evidence.
///
/// # Errors
///
/// Returns an error unless the document is closed, bounded, valid, and byte-canonical.
pub fn parse_critical_evidence(source: &[u8]) -> Result<(), String> {
    valid(source.len() <= MAX_USAGE_DOCUMENT_BYTES)?;
    let mut document: CriticalEvidence =
        serde_json::from_slice(source).map_err(|_| INVALID.to_owned())?;
    document.canonicalize()?;
    valid(encode(&document)? == source)
}

/// Verifies evidence against the exact compiler selection and Asset Plan route membership.
///
/// # Errors
///
/// Returns an error for stale hashes, unknown routes, route-path drift, unselected styles, or
/// styles attributed to a bundle outside the named route.
pub fn verify_critical_evidence(
    source: &[u8],
    universe_sha256: &str,
    reachability_sha256: &str,
    selected: &UsageSelection,
    routes: &[CriticalRouteInput],
) -> Result<CriticalSelection, String> {
    parse_critical_evidence(source)?;
    let document: CriticalEvidence =
        serde_json::from_slice(source).map_err(|_| INVALID.to_owned())?;
    valid(document.universe_sha256 == universe_sha256)?;
    valid(document.reachability_sha256 == reachability_sha256)?;
    let expected = routes
        .iter()
        .map(|route| (route.id.as_str(), route))
        .collect::<BTreeMap<_, _>>();
    valid(expected.len() == routes.len())?;
    let mut output = BTreeMap::<String, BTreeSet<(String, String)>>::new();
    for capture in document.captures {
        let route = expected.get(capture.route_id.as_str()).ok_or(INVALID)?;
        valid(route.path == capture.route_path)?;
        let styles = output.entry(capture.route_id).or_default();
        for style in capture.styles {
            valid(route.bundles.contains(&style.bundle_id))?;
            valid(selected.contains(&style.bundle_id, &style.style_id))?;
            styles.insert((style.bundle_id, style.style_id));
        }
    }
    Ok(CriticalSelection {
        evidence_sha256: sha256_hex(source),
        routes: output,
    })
}

/// Builds the canonical integrity manifest for generated per-route critical CSS.
///
/// # Errors
///
/// Returns an error for invalid hashes, identities, filenames, duplicate routes/styles, empty
/// artifacts, or document-limit violations.
#[allow(clippy::too_many_arguments)]
pub fn build_critical_css_manifest(
    evidence_sha256: &str,
    universe_sha256: &str,
    reachability_sha256: &str,
    theme_id: &str,
    targets: &str,
    format: &str,
    routes: impl IntoIterator<Item = CriticalCssRouteInput>,
) -> Result<Vec<u8>, String> {
    hash(evidence_sha256)?;
    hash(universe_sha256)?;
    hash(reachability_sha256)?;
    hex(theme_id, 32)?;
    valid(matches!(targets, "modern" | "none"))?;
    valid(matches!(format, "minified" | "pretty"))?;
    let mut routes = routes
        .into_iter()
        .map(|input| {
            text(&input.route_id)?;
            text(&input.route_path)?;
            valid(
                input.css_file.starts_with("critical-")
                    && input.css_file.strip_suffix(".css").is_some(),
            )?;
            valid(!input.css.is_empty())?;
            let mut styles = input
                .styles
                .into_iter()
                .map(|(bundle_id, style_id)| {
                    validate_asset_bundle_id(&bundle_id).map_err(|_| INVALID.to_owned())?;
                    hex(&style_id, 32)?;
                    Ok(CriticalStyle {
                        bundle_id,
                        style_id,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            styles.sort();
            valid(styles.windows(2).all(|pair| pair[0] != pair[1]))?;
            Ok(CriticalCssRoute {
                route_id: input.route_id,
                route_path: input.route_path,
                css_file: input.css_file,
                css_bytes: input.css.len(),
                css_sha256: sha256_hex(&input.css),
                styles,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    routes.sort_by(|left, right| left.route_id.cmp(&right.route_id));
    valid(!routes.is_empty() && routes.len() <= MAX_ITEMS)?;
    valid(
        routes
            .windows(2)
            .all(|pair| pair[0].route_id != pair[1].route_id),
    )?;
    let document = CriticalCssManifest {
        schema_version: CRITICAL_CSS_MANIFEST_SCHEMA_VERSION,
        manifest_kind: "pliegocss-critical-css/1".into(),
        evidence_sha256: evidence_sha256.into(),
        universe_sha256: universe_sha256.into(),
        reachability_sha256: reachability_sha256.into(),
        theme_id: theme_id.into(),
        targets: targets.into(),
        format: format.into(),
        routes,
    };
    validate_css_manifest(&document)?;
    encode_css_manifest(&document)
}

/// Parses and byte-canonically validates a generated critical-CSS manifest.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, inconsistent, or unbounded input.
pub fn parse_critical_css_manifest(source: &[u8]) -> Result<(), String> {
    valid(source.len() <= MAX_USAGE_DOCUMENT_BYTES)?;
    let document: CriticalCssManifest =
        serde_json::from_slice(source).map_err(|_| INVALID.to_owned())?;
    validate_css_manifest(&document)?;
    valid(encode_css_manifest(&document)? == source)
}

fn validate_css_manifest(document: &CriticalCssManifest) -> Result<(), String> {
    valid(
        document.schema_version == CRITICAL_CSS_MANIFEST_SCHEMA_VERSION
            && document.manifest_kind == "pliegocss-critical-css/1",
    )?;
    hash(&document.evidence_sha256)?;
    hash(&document.universe_sha256)?;
    hash(&document.reachability_sha256)?;
    hex(&document.theme_id, 32)?;
    valid(matches!(document.targets.as_str(), "modern" | "none"))?;
    valid(matches!(document.format.as_str(), "minified" | "pretty"))?;
    valid(!document.routes.is_empty() && document.routes.len() <= MAX_ITEMS)?;
    valid(
        document
            .routes
            .windows(2)
            .all(|pair| pair[0].route_id < pair[1].route_id),
    )?;
    let mut files = BTreeSet::new();
    for route in &document.routes {
        text(&route.route_id)?;
        text(&route.route_path)?;
        valid(
            route.css_file.starts_with("critical-")
                && route.css_file.strip_suffix(".css").is_some(),
        )?;
        valid(files.insert(route.css_file.as_str()))?;
        valid(route.css_bytes > 0)?;
        hash(&route.css_sha256)?;
        valid(route.styles.windows(2).all(|pair| pair[0] < pair[1]))?;
        for style in &route.styles {
            validate_asset_bundle_id(&style.bundle_id).map_err(|_| INVALID.to_owned())?;
            hex(&style.style_id, 32)?;
        }
    }
    Ok(())
}

fn encode_css_manifest(document: &CriticalCssManifest) -> Result<Vec<u8>, String> {
    let mut output = serde_json::to_vec_pretty(document).map_err(|_| INVALID.to_owned())?;
    output.push(b'\n');
    valid(output.len() <= MAX_USAGE_DOCUMENT_BYTES)?;
    Ok(output)
}

impl CriticalEvidence {
    fn canonicalize(&mut self) -> Result<(), String> {
        valid(self.schema_version == 1 && self.evidence_kind == EVIDENCE_KIND)?;
        hash(&self.universe_sha256)?;
        hash(&self.reachability_sha256)?;
        text(&self.producer.name)?;
        text(&self.producer.version)?;
        valid(!self.captures.is_empty() && self.captures.len() <= MAX_ITEMS)?;
        canonical_texts(&mut self.unknown_dynamic_inputs)?;
        for capture in &mut self.captures {
            text(&capture.id)?;
            text(&capture.route_id)?;
            text(&capture.route_path)?;
            text(&capture.browser)?;
            valid(capture.viewport_width > 0 && capture.viewport_height > 0)?;
            valid(capture.styles.len() <= MAX_ITEMS)?;
            for style in &capture.styles {
                validate_asset_bundle_id(&style.bundle_id).map_err(|_| INVALID.to_owned())?;
                hex(&style.style_id, 32)?;
            }
            capture.styles.sort();
            valid(capture.styles.windows(2).all(|pair| pair[0] != pair[1]))?;
        }
        self.captures.sort();
        valid(
            self.captures
                .windows(2)
                .all(|pair| pair[0].id != pair[1].id),
        )
    }
}

fn encode(document: &CriticalEvidence) -> Result<Vec<u8>, String> {
    let mut output = serde_json::to_vec_pretty(document).map_err(|_| INVALID.to_owned())?;
    output.push(b'\n');
    valid(output.len() <= MAX_USAGE_DOCUMENT_BYTES)?;
    Ok(output)
}

fn canonical_texts(values: &mut [String]) -> Result<(), String> {
    valid(values.len() <= MAX_ITEMS)?;
    for value in values.iter() {
        text(value)?;
    }
    values.sort();
    valid(values.windows(2).all(|pair| pair[0] != pair[1]))
}

fn text(value: &str) -> Result<(), String> {
    valid(!value.is_empty() && value.len() <= MAX_TEXT && !value.chars().any(char::is_control))
}

fn hash(value: &str) -> Result<(), String> {
    hex(value, 64)
}

fn hex(value: &str, length: usize) -> Result<(), String> {
    valid(
        value.len() == length
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
    )
}

fn valid(condition: bool) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(INVALID.into())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{
        CriticalCaptureInput, CriticalCaptureStage, CriticalCssRouteInput, CriticalEvidenceInput,
        CriticalRouteInput, CriticalStyleInput, build_critical_css_manifest,
        build_critical_evidence, parse_critical_css_manifest, parse_critical_evidence,
        verify_critical_evidence,
    };
    use crate::UsageSelection;

    const STYLE: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn evidence_is_canonical_bound_and_bundle_qualified() {
        let capture = CriticalCaptureInput::new(
            "home-mobile",
            "route:home",
            "/",
            "chromium-150",
            390,
            844,
            CriticalCaptureStage::LargestContentfulPaint,
            vec![CriticalStyleInput::new("application", STYLE)],
        );
        let bytes = build_critical_evidence(CriticalEvidenceInput::new(
            "a".repeat(64),
            "b".repeat(64),
            "browser-gate",
            "1.0.0",
            vec!["authenticated-state".into()],
            vec![capture],
        ))
        .expect("evidence must build");
        parse_critical_evidence(&bytes).expect("builder bytes must be canonical");
        let selected = UsageSelection {
            selected: BTreeMap::from([("application".into(), BTreeSet::from([STYLE.into()]))]),
            policy_retained: BTreeMap::new(),
        };
        let routes = [CriticalRouteInput::new(
            "route:home",
            "/",
            ["application".into()],
        )];
        let selection =
            verify_critical_evidence(&bytes, &"a".repeat(64), &"b".repeat(64), &selected, &routes)
                .expect("evidence must verify");
        let (_, styles) = selection.routes().next().expect("route union must exist");
        assert!(styles.contains(&("application".into(), STYLE.into())));
        let manifest = build_critical_css_manifest(
            selection.evidence_sha256(),
            &"a".repeat(64),
            &"b".repeat(64),
            "0123456789abcdef0123456789abcdef",
            "modern",
            "minified",
            [CriticalCssRouteInput::new(
                "route:home",
                "/",
                "critical-0123456789abcdef.css",
                b".pc_test{display:block}\n".to_vec(),
                vec![("application".into(), STYLE.into())],
            )],
        )
        .expect("critical CSS manifest must build");
        parse_critical_css_manifest(&manifest).expect("critical manifest must be canonical");
        assert!(
            verify_critical_evidence(&bytes, &"c".repeat(64), &"b".repeat(64), &selected, &routes,)
                .is_err()
        );

        let mut noncanonical = bytes;
        noncanonical.pop();
        assert!(parse_critical_evidence(&noncanonical).is_err());
    }
}
