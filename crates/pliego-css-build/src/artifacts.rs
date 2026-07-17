//! Versioned build artifacts shared by the CLI and build integrations.

#[cfg(feature = "usage-artifacts")]
mod asset_plan;
#[cfg(feature = "artifacts")]
mod audit;
#[cfg(feature = "catalog")]
mod catalog;
#[cfg(feature = "artifacts")]
mod css_output;
#[cfg(feature = "usage-artifacts")]
mod finding;
#[cfg(feature = "artifacts")]
mod manifest_graph;
#[cfg(feature = "artifacts")]
mod physical_trace;
#[cfg(feature = "artifacts")]
mod project_index;
#[cfg(feature = "usage-artifacts")]
mod reachability;

#[cfg(feature = "usage-artifacts")]
pub use asset_plan::{
    AssetPlanBundle, AssetRuleSelection, build_asset_plan, sha256_hex, validate_asset_bundle_id,
};
#[cfg(feature = "artifacts")]
pub use audit::{
    CssAuditOutcome, audit_standard_css, audit_standard_css_with_budgets, css_layer_finding_source,
    evaluate_budget_observation_findings,
};
#[cfg(feature = "catalog")]
pub use catalog::{CatalogOutputFormat, render_catalog, utility_domain_name, utility_form_name};
#[cfg(feature = "artifacts")]
pub use css_output::{FixedCssOutputCache, optimize_css, optimize_css_with_trace};
#[cfg(feature = "usage-artifacts")]
pub use finding::{
    FINDING_SCHEMA_VERSION, Finding, FindingCause, FindingContractError, FindingDocument,
    FindingEvidence, FindingException, FindingRisk, FindingSeverity, FindingSource,
    FindingSourceMap, FindingSuggestion, FindingTool, FindingVerification, parse_finding_document,
};
#[cfg(feature = "artifacts")]
pub use manifest_graph::{
    GraphOrigin, GraphStyle, ManifestGraph, build_manifest_graph,
    build_manifest_graph_with_physical,
};
#[cfg(feature = "artifacts")]
pub use physical_trace::{
    PhysicalProjection, TraceDeclaration, TraceRule, TraceStyle, build_physical_projection,
};
#[cfg(feature = "artifacts")]
pub use pliego_css_config::compatibility_data::{CompatibilityProfile, build_compatibility_policy};
#[cfg(feature = "artifacts")]
pub use project_index::{ProjectIndexDocument, build_project_index};
#[cfg(feature = "usage-artifacts")]
pub use reachability::{
    MAX_DOCUMENT_BYTES, ReachabilityDocument, ReachabilityIndex, ReachabilityOriginEvidence,
    parse_reachability_document,
};
