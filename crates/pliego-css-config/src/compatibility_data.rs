//! Frozen official compatibility-data identity used by artifact and audit policies.

#![allow(missing_docs)]

use lightningcss::targets::{Browsers, Targets};
use serde::{Deserialize, Serialize};

pub const BASELINE_SNAPSHOT_DATE: &str = "2026-07-14";
pub const WEB_FEATURES_VERSION: &str = "3.32.0";
pub const WEB_FEATURES_DATA_SHA256: &str =
    "58bc2056041c93e313c3a58658a8120f857cf4888d0592e6e4a9b0484748441e";
pub const WEB_FEATURES_TARBALL: &str =
    "https://registry.npmjs.org/web-features/-/web-features-3.32.0.tgz";
pub const WEB_FEATURES_INTEGRITY: &str = "sha512-PQBbTofqV8FtMP65oT9tLPjbN4FSB2dRdNxLM0A9j4bNifVpFhEP/ATXSMMJAqPPWb/pgUOh6B+98yzfNEVbNw==";
pub const BASELINE_MAPPING_VERSION: &str = "2.10.43";
pub const BASELINE_MAPPING_TARBALL: &str =
    "https://registry.npmjs.org/baseline-browser-mapping/-/baseline-browser-mapping-2.10.43.tgz";
pub const BASELINE_MAPPING_INTEGRITY: &str = "sha512-AjYpR78kDWAY3Efj+cDTFH9t9SCoL7OoTp1BOb0mQV7S+6CiLwnWM3FyxhJtdPufDFKzmCSFoUncKjWgJEZTCQ==";
pub const BASELINE_MAPPING_QUERY: &str =
    "widelyAvailableOnDate=2026-07-14;includeDownstreamBrowsers=false";
pub const BASELINE_MAPPING_RESULT_SHA256: &str =
    "05b26b78a9c0f3ad82ce4565ef058ac34fac7ebae45fc260daf3343ffece53fd";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaselineFeatureStatus {
    Widely,
    Newly,
    Limited,
}

impl BaselineFeatureStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Widely => "high",
            Self::Newly => "low",
            Self::Limited => "false",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WebFeatureSnapshot {
    pub id: &'static str,
    pub name: &'static str,
    pub compat_key: &'static str,
    pub baseline: BaselineFeatureStatus,
    pub baseline_low_date: Option<&'static str>,
    pub baseline_high_date: Option<&'static str>,
    pub chrome: Option<&'static str>,
    pub edge: Option<&'static str>,
    pub firefox: Option<&'static str>,
    pub safari: Option<&'static str>,
}

impl WebFeatureSnapshot {
    #[must_use]
    pub fn supports_modern_profile(self) -> bool {
        [
            (self.chrome, "111"),
            (self.edge, "111"),
            (self.firefox, "128"),
            (self.safari, "16.4"),
        ]
        .into_iter()
        .all(|(required, target)| {
            required.is_some_and(|required| version_is_at_least(target, required))
        })
    }
}

#[must_use]
pub fn web_feature_snapshot(id: &str) -> Option<&'static WebFeatureSnapshot> {
    WEB_FEATURE_SNAPSHOTS
        .iter()
        .find(|feature| feature.id == id)
}

fn version_is_at_least(target: &str, required: &str) -> bool {
    let mut target = target
        .split('.')
        .map(|part| part.parse::<u32>().unwrap_or(0));
    let mut required = required
        .split('.')
        .map(|part| part.parse::<u32>().unwrap_or(u32::MAX));
    for _ in 0..3 {
        match (target.next().unwrap_or(0), required.next().unwrap_or(0)) {
            (left, right) if left > right => return true,
            (left, right) if left < right => return false,
            _ => {}
        }
    }
    true
}

const WEB_FEATURE_SNAPSHOTS: &[WebFeatureSnapshot] = &[
    WebFeatureSnapshot {
        id: "cascade-layers",
        name: "Cascade layers",
        compat_key: "css.at-rules.layer",
        baseline: BaselineFeatureStatus::Widely,
        baseline_low_date: Some("2022-03-14"),
        baseline_high_date: Some("2024-09-14"),
        chrome: Some("99"),
        edge: Some("99"),
        firefox: Some("97"),
        safari: Some("15.4"),
    },
    WebFeatureSnapshot {
        id: "container-queries",
        name: "Container queries",
        compat_key: "css.at-rules.container",
        baseline: BaselineFeatureStatus::Widely,
        baseline_low_date: Some("2023-02-14"),
        baseline_high_date: Some("2025-08-14"),
        chrome: Some("105"),
        edge: Some("105"),
        firefox: Some("110"),
        safari: Some("16"),
    },
    WebFeatureSnapshot {
        id: "container-style-queries",
        name: "Container style queries",
        compat_key: "css.at-rules.container.style_queries_for_custom_properties",
        baseline: BaselineFeatureStatus::Newly,
        baseline_low_date: Some("2026-05-19"),
        baseline_high_date: None,
        chrome: Some("111"),
        edge: Some("111"),
        firefox: Some("151"),
        safari: Some("18"),
    },
    WebFeatureSnapshot {
        id: "container-scroll-state-queries",
        name: "Container scroll-state queries",
        compat_key: "css.at-rules.container.scroll-state_queries",
        baseline: BaselineFeatureStatus::Limited,
        baseline_low_date: None,
        baseline_high_date: None,
        chrome: Some("133"),
        edge: Some("133"),
        firefox: None,
        safari: None,
    },
    WebFeatureSnapshot {
        id: "nesting",
        name: "Nesting",
        compat_key: "css.selectors.nesting",
        baseline: BaselineFeatureStatus::Widely,
        baseline_low_date: Some("2023-12-11"),
        baseline_high_date: Some("2026-06-11"),
        chrome: Some("120"),
        edge: Some("120"),
        firefox: Some("117"),
        safari: Some("17.2"),
    },
    WebFeatureSnapshot {
        id: "registered-custom-properties",
        name: "Registered custom properties",
        compat_key: "css.at-rules.property",
        baseline: BaselineFeatureStatus::Newly,
        baseline_low_date: Some("2024-07-09"),
        baseline_high_date: None,
        chrome: Some("85"),
        edge: Some("85"),
        firefox: Some("128"),
        safari: Some("16.4"),
    },
    WebFeatureSnapshot {
        id: "scope",
        name: "@scope",
        compat_key: "css.at-rules.scope",
        baseline: BaselineFeatureStatus::Newly,
        baseline_low_date: Some("2025-12-09"),
        baseline_high_date: None,
        chrome: Some("118"),
        edge: Some("118"),
        firefox: Some("146"),
        safari: Some("17.4"),
    },
    WebFeatureSnapshot {
        id: "starting-style",
        name: "@starting-style",
        compat_key: "css.at-rules.starting-style",
        baseline: BaselineFeatureStatus::Newly,
        baseline_low_date: Some("2024-08-06"),
        baseline_high_date: None,
        chrome: Some("117"),
        edge: Some("117"),
        firefox: Some("129"),
        safari: Some("17.5"),
    },
];

const POLICY_SCHEMA_VERSION: u8 = 2;
const POLICY_VERSION: u8 = 7;
const BASELINE_SOURCE: &str = "https://web-platform-dx.github.io/supported-browsers/";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Compatibility contract applied while parsing and emitting CSS.
pub enum CompatibilityProfile {
    /// Fixed browser targets suitable for current applications.
    #[default]
    Modern,
    /// Strictly accept only classified features that were Baseline Widely Available.
    BaselineWidely,
    /// Parse and emit CSS without a managed compatibility guarantee.
    None,
}

impl CompatibilityProfile {
    /// Return the stable CLI and artifact identifier for this profile.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Modern => "modern",
            Self::BaselineWidely => "baseline-widely",
            Self::None => "none",
        }
    }

    /// Return whether unknown CSS must fail closed under this profile.
    #[must_use]
    pub const fn rejects_unclassified_css(self) -> bool {
        matches!(self, Self::BaselineWidely)
    }

    /// Convert this profile into the browser targets consumed by Lightning CSS.
    #[must_use]
    pub fn lightning(self) -> Targets {
        match self {
            Self::Modern => Browsers {
                chrome: Some(version(111, 0, 0)),
                edge: Some(version(111, 0, 0)),
                firefox: Some(version(128, 0, 0)),
                safari: Some(version(16, 4, 0)),
                ..Browsers::default()
            }
            .into(),
            Self::BaselineWidely => Browsers {
                chrome: Some(version(120, 0, 0)),
                edge: Some(version(120, 0, 0)),
                firefox: Some(version(121, 0, 0)),
                safari: Some(version(17, 2, 0)),
                ios_saf: Some(version(17, 2, 0)),
                ..Browsers::default()
            }
            .into(),
            Self::None => Targets::default(),
        }
    }
}

/// Serialize the deterministic compatibility-policy artifact for `profile`.
///
/// # Errors
///
/// Returns an error if the policy document cannot be serialized.
pub fn build_compatibility_policy(profile: CompatibilityProfile) -> Result<String, String> {
    let (guarantee, baseline_status, snapshot_date, browsers) = match profile {
        CompatibilityProfile::BaselineWidely => (
            "baseline-widely",
            "widely-available",
            Some(BASELINE_SNAPSHOT_DATE),
            BASELINE_BROWSERS,
        ),
        CompatibilityProfile::Modern => (
            "fixed-browser-targets",
            "not-applicable",
            None,
            MODERN_BROWSERS,
        ),
        CompatibilityProfile::None => ("none", "not-applicable", None, NO_BROWSERS),
    };
    let features = feature_decisions(profile);
    let document = CompatibilityPolicy {
        schema_version: POLICY_SCHEMA_VERSION,
        policy_version: POLICY_VERSION,
        profile: profile.as_str(),
        guarantee,
        compatibility_data: CompatibilityData {
            captured_on: BASELINE_SNAPSHOT_DATE,
            web_features: PackageDataReference {
                name: "web-features",
                version: WEB_FEATURES_VERSION,
                artifact: WEB_FEATURES_TARBALL,
                integrity: WEB_FEATURES_INTEGRITY,
                content: "data.json",
                content_sha256: WEB_FEATURES_DATA_SHA256,
            },
            baseline_browser_mapping: BaselineMappingReference {
                name: "baseline-browser-mapping",
                version: BASELINE_MAPPING_VERSION,
                artifact: BASELINE_MAPPING_TARBALL,
                integrity: BASELINE_MAPPING_INTEGRITY,
                query: BASELINE_MAPPING_QUERY,
                result_sha256: BASELINE_MAPPING_RESULT_SHA256,
            },
        },
        baseline: BaselineReference {
            status: baseline_status,
            snapshot_date,
            source: BASELINE_SOURCE,
        },
        browser_targets: browsers,
        reset: ResetPolicy {
            profile: "none",
            action: CompatibilityAction::Warn,
            implicit: false,
            ownership: "host-owned-explicit-import",
        },
        scope: ScopePolicy {
            profile: "standard-class",
            action: CompatibilityAction::Allow,
            boundary: "host-owned",
        },
        features: &features,
    };
    let mut output = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("cannot serialize compatibility policy: {error}"))?;
    output.push('\n');
    Ok(output)
}

const fn version(major: u32, minor: u32, patch: u32) -> u32 {
    (major << 16) | (minor << 8) | patch
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompatibilityPolicy<'a> {
    schema_version: u8,
    policy_version: u8,
    profile: &'static str,
    guarantee: &'static str,
    compatibility_data: CompatibilityData,
    baseline: BaselineReference<'a>,
    browser_targets: &'a [BrowserTarget],
    reset: ResetPolicy,
    scope: ScopePolicy,
    features: &'a [FeatureDecision],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompatibilityData {
    captured_on: &'static str,
    web_features: PackageDataReference,
    baseline_browser_mapping: BaselineMappingReference,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageDataReference {
    name: &'static str,
    version: &'static str,
    artifact: &'static str,
    integrity: &'static str,
    content: &'static str,
    content_sha256: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BaselineMappingReference {
    name: &'static str,
    version: &'static str,
    artifact: &'static str,
    integrity: &'static str,
    query: &'static str,
    result_sha256: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BaselineReference<'a> {
    status: &'static str,
    snapshot_date: Option<&'a str>,
    source: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserTarget {
    id: &'static str,
    minimum: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResetPolicy {
    profile: &'static str,
    action: CompatibilityAction,
    implicit: bool,
    ownership: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScopePolicy {
    profile: &'static str,
    action: CompatibilityAction,
    boundary: &'static str,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum CompatibilityAction {
    Allow,
    Transform,
    Warn,
    Error,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum CapabilityTier {
    Core,
    Extended,
    EscapeHatch,
    Experimental,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FeatureDecision {
    id: &'static str,
    tier: CapabilityTier,
    action: CompatibilityAction,
    enforcement: &'static str,
}

const BASELINE_BROWSERS: &[BrowserTarget] = &[
    BrowserTarget {
        id: "chrome",
        minimum: "120.0.0",
    },
    BrowserTarget {
        id: "edge",
        minimum: "120.0.0",
    },
    BrowserTarget {
        id: "firefox",
        minimum: "121.0.0",
    },
    BrowserTarget {
        id: "ios-safari",
        minimum: "17.2.0",
    },
    BrowserTarget {
        id: "safari",
        minimum: "17.2.0",
    },
];

const MODERN_BROWSERS: &[BrowserTarget] = &[
    BrowserTarget {
        id: "chrome",
        minimum: "111.0.0",
    },
    BrowserTarget {
        id: "edge",
        minimum: "111.0.0",
    },
    BrowserTarget {
        id: "firefox",
        minimum: "128.0.0",
    },
    BrowserTarget {
        id: "safari",
        minimum: "16.4.0",
    },
];

const NO_BROWSERS: &[BrowserTarget] = &[];

macro_rules! decision {
    ($id:literal, $tier:ident, $action:ident, $enforcement:literal) => {
        FeatureDecision {
            id: $id,
            tier: CapabilityTier::$tier,
            action: CompatibilityAction::$action,
            enforcement: $enforcement,
        }
    };
}

const FEATURE_TEMPLATES: &[FeatureDecision] = &[
    decision!("typed-utility-catalog", Core, Allow, "compiler-validated"),
    decision!("custom-property-references", Core, Allow, "typed-reference"),
    decision!(
        "typed-attribute-variants",
        Extended,
        Allow,
        "compiler-validated-selector"
    ),
    decision!(
        "configurable-attribute-variants",
        Extended,
        Allow,
        "compiler-bounded-attribute-selector"
    ),
    decision!(
        "writing-direction-variants",
        Core,
        Allow,
        "compiler-validated-selector"
    ),
    decision!(
        "writing-mode-utilities",
        Core,
        Allow,
        "compiler-validated-native-property"
    ),
    decision!(
        "container-queries",
        Extended,
        Allow,
        "typed-condition-native-css"
    ),
    decision!(
        "vendor-prefixes",
        Core,
        Transform,
        "lightning-target-driven"
    ),
    decision!(
        "selector-syntax",
        Extended,
        Transform,
        "lightning-target-driven"
    ),
    decision!(
        "media-query-syntax",
        Core,
        Transform,
        "lightning-target-driven"
    ),
    decision!(
        "color-syntax",
        Extended,
        Transform,
        "lightning-target-driven"
    ),
    decision!(
        "logical-properties",
        Extended,
        Transform,
        "lightning-target-driven"
    ),
    decision!("arbitrary-values", EscapeHatch, Error, "CMP001"),
    decision!("arbitrary-properties", EscapeHatch, Error, "CMP002"),
    decision!("arbitrary-selectors", EscapeHatch, Error, "CMP003"),
    decision!("light-dark", Experimental, Error, "unclassified-syntax"),
    decision!("custom-media", Experimental, Error, "draft-parser-disabled"),
    decision!("browser-reset", Core, Warn, "host-owned-explicit-import"),
    decision!(
        "cascade-layers",
        Extended,
        Allow,
        "compiler-fixed-order-native-css"
    ),
    decision!("standard-class-scope", Core, Allow, "native-css-class"),
    decision!("component-scope", Experimental, Error, "not-implemented"),
];

fn feature_decisions(profile: CompatibilityProfile) -> Vec<FeatureDecision> {
    FEATURE_TEMPLATES
        .iter()
        .map(|template| {
            let (action, enforcement) = match profile {
                CompatibilityProfile::BaselineWidely => (template.action, template.enforcement),
                CompatibilityProfile::Modern => match template.id {
                    "arbitrary-values" | "arbitrary-properties" | "arbitrary-selectors" => {
                        (CompatibilityAction::Allow, "unclassified-legacy-profile")
                    }
                    "light-dark" => (CompatibilityAction::Transform, "lightning-target-driven"),
                    _ => (template.action, template.enforcement),
                },
                CompatibilityProfile::None => match template.id {
                    "vendor-prefixes" => (CompatibilityAction::Allow, "disabled"),
                    "selector-syntax"
                    | "media-query-syntax"
                    | "color-syntax"
                    | "logical-properties"
                    | "arbitrary-values"
                    | "arbitrary-properties"
                    | "arbitrary-selectors"
                    | "light-dark" => (CompatibilityAction::Allow, "unmanaged"),
                    _ => (template.action, template.enforcement),
                },
            };
            FeatureDecision {
                id: template.id,
                tier: template.tier,
                action,
                enforcement,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_and_feature_snapshot_are_frozen() {
        assert_eq!(
            CompatibilityProfile::BaselineWidely.lightning().browsers,
            Some(Browsers {
                chrome: Some(version(120, 0, 0)),
                edge: Some(version(120, 0, 0)),
                firefox: Some(version(121, 0, 0)),
                ios_saf: Some(version(17, 2, 0)),
                safari: Some(version(17, 2, 0)),
                ..Browsers::default()
            })
        );
        let policy = build_compatibility_policy(CompatibilityProfile::BaselineWidely)
            .expect("serialize policy");
        let json: serde_json::Value = serde_json::from_str(&policy).expect("parse policy");
        assert_eq!(json["schemaVersion"], 2);
        assert_eq!(json["policyVersion"], 7);
        assert_eq!(
            json["compatibilityData"]["capturedOn"],
            BASELINE_SNAPSHOT_DATE
        );
        assert_eq!(
            json["compatibilityData"]["webFeatures"]["version"],
            WEB_FEATURES_VERSION
        );
        assert_eq!(json["features"].as_array().map(Vec::len), Some(21));
        assert!(policy.ends_with('\n'));

        let nesting = web_feature_snapshot("nesting").expect("nesting snapshot");
        assert_eq!(nesting.baseline, BaselineFeatureStatus::Widely);
        assert!(!nesting.supports_modern_profile());
    }
}
