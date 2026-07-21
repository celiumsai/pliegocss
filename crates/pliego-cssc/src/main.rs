//! `PliegoCSS` CLI.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt as _;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use pliego_css_agent::{
    RepairCliCommand, RepairFixCliArgs, RepairPlanCliArgs, parse_repair_cli_arguments,
};
use pliego_css_build::artifacts::{
    AssetPlanBundle, AssetRuleSelection, CompatibilityProfile, Finding, FindingCause,
    FindingDocument, FindingSeverity, FindingSource, FindingTool, FindingVerification,
    FixedCssOutputCache, GraphOrigin, GraphStyle as ManifestGraphStyle, MAX_DOCUMENT_BYTES,
    ManifestGraph, ProjectIndexDocument, ReachabilityDocument, StandardCssFormat, TraceDeclaration,
    TraceRule, TraceStyle, audit_standard_css, audit_standard_css_with_budgets, build_asset_plan,
    build_manifest_graph, build_manifest_graph_with_physical, build_physical_projection,
    build_project_index, css_layer_finding_source, evaluate_budget_observation_findings,
    optimize_css_with_trace, parse_finding_document, parse_reachability_document, sha256_hex,
    transform_standard_css, validate_asset_bundle_id,
};
use pliego_css_compiler::{
    CssFragmentCache, STYLE_ID_FORMAT_VERSION, compose_style_override_with_theme,
    emit_css_with_theme_traced, emit_theme, emit_theme_references, emit_used_theme,
    lower_style_with_theme, referenced_tokens, try_encode_style_identity_with_theme,
};
use pliego_css_config::{
    BudgetObservation, BudgetPolicy, BudgetSubject, BudgetSubjectKind, CssBudgetInventory,
    CssBudgetMetrics, DtcgTheme, TokenGraph, parse_budget_policy, parse_dtcg_resolver_str,
    parse_token_graph,
};
use pliego_css_ir::{CLASS_NAME_FORMAT_VERSION, Diagnostic, SemanticStyle, TokenRef};
use pliego_css_ownership::{
    AssetRuleSelection as OwnershipRuleSelection, Ownership, parse_asset_plan, parse_ownership,
};
use pliego_css_parser::parse_style_list;
use pliego_css_source::{
    MigrationSourceKind, ScanDiagnostic, ScanReport, SourceRange, UtilityFormatFinding,
    expand_bundle_source_paths, expand_source_paths, format_source_paths, format_style_failure,
    scan_source_named,
};
use pliego_css_theme::{THEME_ID_FORMAT_VERSION, ThemeRegistry};
use pliego_css_usage::{
    CRITICAL_CSS_MANIFEST_FILE, CriticalCssRouteInput, CriticalRouteInput, PreparedUsageAnalysis,
    TOKEN_USAGE_FILE, UsageCandidateInput, UsageSelection, UsageStyleInput,
    build_critical_css_manifest, build_generic_css_usage_report, build_token_usage_report,
    collect_selected_bundle_token_references, collect_selected_token_usage_consumers,
    collect_usage_style_inputs, prepare_usage_analysis, verify_critical_evidence,
};
use pliego_css_watch::{
    FileSnapshot as WatchFileSnapshot, WatchScheduler, confirmed as watch_snapshot_confirmed,
    snapshot_file,
};
use serde::{Deserialize, Serialize};

use pliego_css_control::accessibility::parse_accessibility_policy;
use pliego_css_control::projection as control;
use pliego_css_control::projection::{
    AccessibilityPolicySource, AccessibilityStylesheetInput, AuditControlGroup, AuditControlInput,
    AuditSourceInput, ControlOutputInput, ControlSourceMapInput, FlatTokenReference,
    SourceMapOrigin, SourceMapStyle, TokenObservationInput, build_audit_control_group,
    build_audit_token_graph_measurements, build_css_source_map, build_token_graph_measurements,
    evaluate_accessibility, render_sarif,
};

mod atomic_write;
mod catalog;
mod compatibility;
mod explain;
mod formatter;
mod inspection;
mod migration;
mod provenance;
mod publication;
mod repair;
mod source_candidates;
use atomic_write::{prepare_atomic_write, prepare_atomic_write_if_changed};
use catalog::run_catalog;
use compatibility::{enforce_compatibility_policy, run_compatibility};
use explain::{run_cascade_explain, run_explain};
use formatter::{reject_isolated_carriage_returns, run_utility_formatter};
use inspection::serialize_inspection;
use migration::{
    run_migration_inventory, run_migration_project, run_reversible_group_apply,
    run_reversible_group_rollback, run_reversible_migration_project_plan,
    run_reversible_replace_apply, run_reversible_replace_rollback, run_reversible_sidecar_apply,
    run_reversible_sidecar_rollback,
};
use provenance::{cli_provenance, normalize_provenance, provenance_is_reachable};
use publication::{
    acquire_publication_locks, commit_prepared_locked_with, publication_destinations,
};
use repair::{run_repair_fix, run_repair_plan};
use source_candidates::{
    candidates_from_line_source, candidates_from_scan_report, cli_candidates, collect_candidates,
};

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const EVENT_FALLBACK_INTERVAL: Duration = Duration::from_secs(2);
const INSPECTION_SCHEMA_VERSION: u8 = 2;
const EXPLAIN_SCHEMA_VERSION: u8 = 2;
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const USAGE: &str = r"PliegoCSS command-line compiler and control tool

Usage:
  pliego-cssc [--diagnostic-format human|json] <command> [options]
  pliego-cssc help
  pliego-cssc --version

Core:
  audit                       Audit ordinary CSS and emit human, JSON, or SARIF findings
  transform-css               Transform or minify ordinary CSS
  compile | build             Compile utility styles or Rust source to static CSS
  check                       Validate styles without writing output
  inspect                     Inspect resolved styles and identities as JSON
  watch                       Recompile source changes and keep the last valid output
  bundle                      Compile an explicit multi-bundle plan
  catalog                     Emit the generated utility catalog
  compatibility               Print the selected browser compatibility policy
  explain                     Explain utility lowering and physical declarations
  explain-cascade             Explain a winning CSS declaration for one element/property

Adoption and control:
  migration-inventory         Inventory one Sass, Tailwind, or CSS Modules source
  migration-project-inventory Inventory migration surfaces in a project
  migration-project-plan      Emit a read-only inventory-bound migration checkpoint
  migration-sidecar-apply     Apply an approved sidecar migration
  migration-sidecar-rollback  Roll back an applied sidecar migration
  migration-replace-apply     Apply one exact before/after replacement
  migration-replace-rollback  Roll back one exact replacement
  migration-group-apply       Atomically apply an approved replacement group
  migration-group-rollback    Roll back an applied replacement group
  generic-css-usage           Combine audit findings with positive usage observations
  plan                        Build a bounded repair plan
  fix                         Apply an explicitly authorized repair plan
  fmt                         Format utility literals

Global options:
  --diagnostic-format <mode>   human (default) or json
  -h, --help                  Show this help; COMMAND --help shows the command synopsis
  -V, --version               Show the installed version

Run `pliego-cssc <command> --help` for a command synopsis.
Full reference: https://github.com/celiumsai/pliegocss/blob/main/docs/reference/cli.md
";

fn command_help(command: Option<&str>) -> &'static str {
    match command {
        Some("audit") => {
            "Audit ordinary CSS\n\nUsage:\n  pliego-cssc audit --input <CSS|ASSET_PLAN> [--format human|json|sarif] [--targets modern|baseline-widely|none] [--check]\n"
        }
        Some("transform-css") => {
            "Transform ordinary CSS\n\nUsage:\n  pliego-cssc transform-css --input <CSS> --output <CSS> [--targets modern|baseline-widely|none] [--format minified|pretty] [--check]\n"
        }
        Some("compile" | "build") => {
            "Compile typed styles to static CSS\n\nUsage:\n  pliego-cssc compile (--style <UTILITY_LIST> | --source <PATH> | --input <FILE>)... [--seed | --config <TOML> | --tokens <JSON>] [--theme] [--output <CSS>] [--manifest <JSON>]\n\nRepeatable: --style, --source, --compose, --token-input\n"
        }
        Some("check") => {
            "Validate typed styles without writing output\n\nUsage:\n  pliego-cssc check (--style <UTILITY_LIST> | --source <PATH> | --input <FILE>)... [--seed | --config <TOML> | --tokens <JSON>]\n"
        }
        Some("inspect") => {
            "Inspect typed styles as canonical JSON\n\nUsage:\n  pliego-cssc inspect (--style <UTILITY_LIST> | --source <PATH> | --input <FILE>)... [--seed | --config <TOML> | --tokens <JSON>]\n"
        }
        Some("watch") => {
            "Watch source inputs and republish the last valid artifact group\n\nUsage:\n  pliego-cssc watch (--source <PATH> | --input <FILE>)... --output <CSS> [--manifest <JSON>] [--seed | --config <TOML> | --tokens <JSON>]\n"
        }
        Some("bundle") => {
            "Compile an explicit bundle plan\n\nUsage:\n  pliego-cssc bundle --plan <PLIEGO.BUNDLES.TOML> --output-dir <DIR> [--manifest-version 3|4|5] [--reachability <JSON>] [--check] [--control]\n"
        }
        Some("catalog") => {
            "Emit the generated utility catalog\n\nUsage:\n  pliego-cssc catalog [--seed | --config <TOML>] [--format markdown|json] [--output <PATH>] [--check <PATH>]\n"
        }
        Some("compatibility") => {
            "Print a compatibility policy\n\nUsage:\n  pliego-cssc compatibility [--targets modern|baseline-widely|none]\n"
        }
        Some("explain") => {
            "Explain utility lowering\n\nUsage:\n  pliego-cssc explain --style <UTILITY_LIST> [--seed | --config <TOML>] [--targets modern|baseline-widely|none] [--format text|json]\n"
        }
        Some("explain-cascade") => {
            "Explain one CSS cascade result\n\nUsage:\n  pliego-cssc explain-cascade --input <CSS> --element <SELECTOR> --property <NAME> [--format text|json]\n"
        }
        Some("generic-css-usage") => {
            "Build generic CSS usage evidence\n\nUsage:\n  pliego-cssc generic-css-usage --findings <JSON> --observed <JSON> --scope <ID> --output <JSON> [--control-dir <DIR>]\n"
        }
        Some("migration-inventory") => {
            "Inventory one migration source\n\nUsage:\n  pliego-cssc migration-inventory <sass|tailwind|css-modules> <PATH>\n"
        }
        Some("migration-project-inventory") => {
            "Inventory migration surfaces in a project\n\nUsage:\n  pliego-cssc migration-project-inventory <PROJECT_PATH>\n"
        }
        Some("migration-project-plan") => {
            "Emit a read-only migration checkpoint\n\nUsage:\n  pliego-cssc migration-project-plan <DECLARATION_JSON|DIRECTORY>\n"
        }
        Some("migration-sidecar-apply") => {
            "Apply an approved sidecar migration\n\nUsage:\n  pliego-cssc migration-sidecar-apply --output <PATH> --receipt <JSON>\n"
        }
        Some("migration-sidecar-rollback") => {
            "Roll back an applied sidecar migration\n\nUsage:\n  pliego-cssc migration-sidecar-rollback --output <PATH> --receipt <JSON>\n"
        }
        Some("migration-replace-apply") => {
            "Apply one exact replacement\n\nUsage:\n  pliego-cssc migration-replace-apply --file <PATH> --before <PATH> --after <PATH> --receipt <JSON>\n"
        }
        Some("migration-replace-rollback") => {
            "Roll back one exact replacement\n\nUsage:\n  pliego-cssc migration-replace-rollback --file <PATH> --receipt <JSON>\n"
        }
        Some("migration-group-apply") => {
            "Apply an approved replacement group\n\nUsage:\n  pliego-cssc migration-group-apply --manifest <JSON> --receipt <JSON>\n"
        }
        Some("migration-group-rollback") => {
            "Roll back an applied replacement group\n\nUsage:\n  pliego-cssc migration-group-rollback --receipt <JSON>\n"
        }
        Some("plan") => {
            "Build a bounded repair plan\n\nUsage:\n  pliego-cssc plan --findings <JSON> --proposal <JSON> --source-root <DIR> [--format human|json]\n"
        }
        Some("fix") => {
            "Apply an explicitly authorized repair plan\n\nUsage:\n  pliego-cssc fix --plan <JSON> --findings <JSON> --source-root <DIR> --authorize <SHA256> --receipt <JSON> [--format human|json]\n"
        }
        Some("fmt") => {
            "Format utility literals\n\nUsage:\n  pliego-cssc fmt (--style <UTILITY_LIST> | --input <PATH> | --source <PATH>)... [--output <PATH>] [--check]\n"
        }
        None | Some(_) => USAGE,
    }
}

type TargetContract = CompatibilityProfile;
type CssCaches = (CssFragmentCache, FixedCssOutputCache);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum DiagnosticFormat {
    #[default]
    Human,
    Json,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliDiagnosticOrigin {
    kind: String,
    label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliSourceRange {
    file: String,
    byte_start: usize,
    byte_end: usize,
    start_line: Option<usize>,
    start_column: Option<usize>,
    end_line: Option<usize>,
    end_column: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliByteRange {
    byte_start: usize,
    byte_end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliReplacement {
    kind: String,
    value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliDiagnostic {
    code: String,
    category: String,
    severity: String,
    message: String,
    suggestion: Option<String>,
    origin: Option<CliDiagnosticOrigin>,
    range: Option<CliSourceRange>,
    style_range: Option<CliByteRange>,
    replacement: Option<CliReplacement>,
}

#[derive(Debug, Eq, PartialEq)]
struct CliFailure {
    human: String,
    diagnostics: Vec<CliDiagnostic>,
}

impl CliFailure {
    fn invalid(message: impl Into<String>) -> Self {
        Self::single("PCL001", "invocation", message)
    }

    fn tool(message: impl Into<String>) -> Self {
        Self::single("PCL002", "tool", message)
    }

    fn compilation(message: String) -> Self {
        for code in ["CMP001", "CMP002", "CMP003"] {
            if message.starts_with(code) {
                return Self::single(code, "compatibility", message);
            }
        }
        Self::tool(message)
    }

    fn single(code: &str, category: &str, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            human: message.clone(),
            diagnostics: vec![CliDiagnostic {
                code: code.into(),
                category: category.into(),
                severity: "error".into(),
                message,
                suggestion: None,
                origin: None,
                range: None,
                style_range: None,
                replacement: None,
            }],
        }
    }
}

impl fmt::Display for CliFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.human)
    }
}

impl std::error::Error for CliFailure {}

impl From<String> for CliFailure {
    fn from(message: String) -> Self {
        Self::tool(message)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDocument<'a> {
    schema_version: u8,
    command: Option<&'a str>,
    diagnostics: &'a [CliDiagnostic],
}

#[derive(Debug, Eq, PartialEq)]
struct GlobalOptions {
    diagnostic_format: DiagnosticFormat,
    arguments: Vec<OsString>,
}

#[derive(Debug, Eq, PartialEq)]
struct GlobalOptionsError {
    diagnostic_format: DiagnosticFormat,
    command: Option<String>,
    failure: CliFailure,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum CssFormat {
    #[default]
    Minified,
    Pretty,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ReachabilityPruning {
    #[default]
    Disabled,
    Unreachable,
}

impl ReachabilityPruning {
    const fn is_enabled(self) -> bool {
        matches!(self, Self::Unreachable)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum CatalogFormat {
    #[default]
    Markdown,
    Json,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ExplainFormat {
    #[default]
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum AuditFormat {
    #[default]
    Human,
    Json,
    Sarif,
}

#[derive(Debug, Eq, PartialEq)]
struct AuditArgs {
    input: AuditInput,
    format: AuditFormat,
    targets: TargetContract,
    budget_policy: Option<PathBuf>,
    ownership: Option<PathBuf>,
    accessibility_policy: Option<PathBuf>,
    token_graph: Option<PathBuf>,
    budget_subjects: Vec<BudgetSubject>,
    control_dir: Option<PathBuf>,
    check: bool,
}

#[derive(Debug, Eq, PartialEq)]
struct StandardCssTransformArgs {
    input: PathBuf,
    output: PathBuf,
    targets: TargetContract,
    format: CssFormat,
    check: bool,
    control_dir: Option<PathBuf>,
}

#[derive(Debug, Eq, PartialEq)]
enum AuditInput {
    Css(PathBuf),
    AssetPlan(PathBuf),
}

#[derive(Debug, Default, Eq, PartialEq)]
struct CatalogArgs {
    config: Option<PathBuf>,
    seed: bool,
    format: CatalogFormat,
    output: Option<PathBuf>,
    check: Option<PathBuf>,
}

#[derive(Debug, Eq, PartialEq)]
struct CompatibilityArgs {
    targets: TargetContract,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct UtilityFormatArgs {
    styles: Vec<String>,
    input: Option<PathBuf>,
    sources: Vec<PathBuf>,
    output: Option<PathBuf>,
    check: bool,
    apply: bool,
}

#[derive(Debug, Eq, PartialEq)]
struct ExplainArgs {
    style: String,
    config: Option<PathBuf>,
    seed: bool,
    targets: TargetContract,
    format: ExplainFormat,
}

#[derive(Debug, Eq, PartialEq)]
struct CascadeExplainArgs {
    input: PathBuf,
    element: String,
    property: String,
    format: ExplainFormat,
}

impl CssFormat {
    const fn is_minified(self) -> bool {
        matches!(self, Self::Minified)
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Minified => "minified",
            Self::Pretty => "pretty",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
struct BuildArgs {
    styles: Vec<String>,
    compositions: Vec<(String, String)>,
    input: Option<PathBuf>,
    sources: Vec<PathBuf>,
    config: Option<PathBuf>,
    tokens: Option<PathBuf>,
    token_inputs: Vec<(String, String)>,
    seed: bool,
    theme: bool,
    targets: TargetContract,
    format: CssFormat,
    output: Option<PathBuf>,
    manifest: Option<PathBuf>,
    control_dir: Option<PathBuf>,
    check: bool,
    reachability: Option<PathBuf>,
    physical_trace: bool,
    pruning: ReachabilityPruning,
}

impl Default for BuildArgs {
    fn default() -> Self {
        Self {
            styles: Vec::new(),
            compositions: Vec::new(),
            input: None,
            sources: Vec::new(),
            config: None,
            tokens: None,
            token_inputs: Vec::new(),
            seed: false,
            theme: false,
            targets: TargetContract::Modern,
            format: CssFormat::Minified,
            output: None,
            manifest: None,
            control_dir: None,
            check: false,
            reachability: None,
            physical_trace: false,
            pruning: ReachabilityPruning::Disabled,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct WatchArgs {
    input: Option<PathBuf>,
    sources: Vec<PathBuf>,
    output: PathBuf,
    config: Option<PathBuf>,
    tokens: Option<PathBuf>,
    token_inputs: Vec<(String, String)>,
    seed: bool,
    theme: bool,
    targets: TargetContract,
    format: CssFormat,
    manifest: Option<PathBuf>,
    control_dir: Option<PathBuf>,
    reachability: Option<PathBuf>,
    physical_trace: bool,
    pruning: ReachabilityPruning,
}

#[derive(Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
struct BundleArgs {
    plan: PathBuf,
    output_dir: PathBuf,
    check: bool,
    asset_plan: bool,
    project_index: bool,
    usage_report: bool,
    control: bool,
    observations: Option<PathBuf>,
    retention: Option<PathBuf>,
    critical_evidence: Option<PathBuf>,
    reachability: Option<PathBuf>,
    physical_trace: bool,
    pruning: ReachabilityPruning,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum BundleThemeKind {
    Seed,
    Config,
    DtcgResolver,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct BundleThemeDocument {
    kind: BundleThemeKind,
    path: Option<PathBuf>,
    inputs: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct BundleSpecDocument {
    sources: Vec<PathBuf>,
    #[serde(default)]
    emit_theme: bool,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct BundlePlanDocument {
    schema: u8,
    targets: TargetContract,
    format: CssFormat,
    theme: BundleThemeDocument,
    bundles: BTreeMap<String, BundleSpecDocument>,
}

#[derive(Debug, Eq, PartialEq)]
enum Command {
    Audit(AuditArgs),
    TransformCss(StandardCssTransformArgs),
    Compile(BuildArgs),
    Check(BuildArgs),
    Inspect(BuildArgs),
    Watch(WatchArgs),
    Bundle(BundleArgs),
    Catalog(CatalogArgs),
    Compatibility(CompatibilityArgs),
    GenericCssUsage {
        findings: PathBuf,
        observed: PathBuf,
        scope: String,
        output: PathBuf,
        control_dir: Option<PathBuf>,
    },
    Inventory(MigrationSourceKind, PathBuf),
    InventoryProject(PathBuf),
    MigrationProjectPlan(PathBuf),
    MigrationSidecarApply {
        output: PathBuf,
        receipt: PathBuf,
    },
    MigrationSidecarRollback {
        output: PathBuf,
        receipt: PathBuf,
    },
    MigrationReplaceApply {
        file: PathBuf,
        before: PathBuf,
        after: PathBuf,
        receipt: PathBuf,
    },
    MigrationReplaceRollback {
        file: PathBuf,
        receipt: PathBuf,
    },
    MigrationGroupApply {
        manifest: PathBuf,
        receipt: PathBuf,
    },
    MigrationGroupRollback {
        receipt: PathBuf,
    },
    Explain(ExplainArgs),
    ExplainCascade(CascadeExplainArgs),
    Plan(RepairPlanCliArgs),
    Fix(RepairFixCliArgs),
    Format(UtilityFormatArgs),
    Help(Option<String>),
    Version,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Provenance {
    source: String,
    file: Option<String>,
    byte_start: Option<usize>,
    byte_end: Option<usize>,
    macro_kind: String,
    reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Candidate {
    Direct {
        style: String,
        provenance: Provenance,
    },
    Composition {
        base: String,
        branches: Vec<String>,
        provenance: Provenance,
    },
}

impl Candidate {
    fn provenance(&self) -> &Provenance {
        match self {
            Self::Direct { provenance, .. } | Self::Composition { provenance, .. } => provenance,
        }
    }
}

fn diagnostic_origin_from_provenance(provenance: &Provenance) -> CliDiagnosticOrigin {
    CliDiagnosticOrigin {
        kind: provenance.macro_kind.clone(),
        label: provenance.reason.clone(),
    }
}

fn diagnostic_segment_provenance(provenance: &Provenance, segment: &str) -> Provenance {
    let mut diagnostic = provenance.clone();
    diagnostic.reason = format!("{}:{segment}", provenance.reason);
    diagnostic
}

fn diagnostic_range_from_provenance(provenance: &Provenance) -> Option<CliSourceRange> {
    Some(CliSourceRange {
        file: provenance.file.clone()?,
        byte_start: provenance.byte_start?,
        byte_end: provenance.byte_end?,
        start_line: None,
        start_column: None,
        end_line: None,
        end_column: None,
    })
}

fn diagnostic_range_from_source(file: &str, range: SourceRange) -> CliSourceRange {
    CliSourceRange {
        file: file.to_owned(),
        byte_start: range.start.byte,
        byte_end: range.end.byte,
        start_line: Some(range.start.line),
        start_column: Some(range.start.column + 1),
        end_line: Some(range.end.line),
        end_column: Some(range.end.column + 1),
    }
}

fn diagnostic_range_from_line(
    path: &Path,
    offset: usize,
    line_index: usize,
    line: &str,
) -> CliSourceRange {
    CliSourceRange {
        file: path.display().to_string(),
        byte_start: offset,
        byte_end: offset + line.len(),
        start_line: Some(line_index + 1),
        start_column: Some(1),
        end_line: Some(line_index + 1),
        end_column: Some(line.chars().count() + 1),
    }
}

fn style_failure(error: Diagnostic, human: String, provenance: &Provenance) -> CliFailure {
    CliFailure {
        human,
        diagnostics: vec![CliDiagnostic {
            code: error.code.as_str().into(),
            category: "style".into(),
            severity: "error".into(),
            message: error.message,
            suggestion: error.suggestion,
            origin: Some(diagnostic_origin_from_provenance(provenance)),
            range: diagnostic_range_from_provenance(provenance),
            style_range: Some(CliByteRange {
                byte_start: error.span.start,
                byte_end: error.span.end,
            }),
            replacement: None,
        }],
    }
}

fn style_literal_failure(
    error: Diagnostic,
    human: String,
    file: &str,
    range: SourceRange,
    label: &str,
) -> CliFailure {
    let mut failure = style_failure(
        error,
        human,
        &Provenance {
            source: String::new(),
            file: Some(file.to_owned()),
            byte_start: Some(range.start.byte),
            byte_end: Some(range.end.byte),
            macro_kind: "rust".into(),
            reason: label.into(),
        },
    );
    failure.diagnostics[0].range = Some(diagnostic_range_from_source(file, range));
    failure
}

fn scan_failure(diagnostics: &[ScanDiagnostic]) -> CliFailure {
    let human = diagnostics
        .iter()
        .map(|diagnostic| {
            format!(
                "{} [bytes {}..{})",
                diagnostic, diagnostic.range.start.byte, diagnostic.range.end.byte
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let diagnostics = diagnostics
        .iter()
        .map(|diagnostic| CliDiagnostic {
            code: diagnostic.code.into(),
            category: "source".into(),
            severity: "error".into(),
            message: diagnostic.message.clone(),
            suggestion: None,
            origin: Some(CliDiagnosticOrigin {
                kind: "rust".into(),
                label: "source-scan".into(),
            }),
            range: Some(diagnostic_range_from_source(
                &diagnostic.source,
                diagnostic.range,
            )),
            style_range: None,
            replacement: None,
        })
        .collect();
    CliFailure { human, diagnostics }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedCandidate {
    semantic: SemanticStyle,
    provenance: Provenance,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestStyle {
    style_id: String,
    class_name: String,
    origins: Vec<Provenance>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u8,
    style_id_format_version: u16,
    class_name_format_version: u16,
    theme_id_format_version: u16,
    theme_id: String,
    targets: TargetContract,
    format: CssFormat,
    css_sha256: String,
    css_bytes: usize,
    styles: Vec<ManifestStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    graph: Option<ManifestGraph>,
}

fn format_finding_diagnostic(finding: &UtilityFormatFinding) -> CliDiagnostic {
    CliDiagnostic {
        code: "FMT001".into(),
        category: "format".into(),
        severity: "error".into(),
        message: format!(
            "{} utility literal is not canonically formatted",
            finding.role
        ),
        suggestion: Some(format!(
            "expected {:?}, found {:?}",
            finding.expected, finding.actual
        )),
        origin: Some(CliDiagnosticOrigin {
            kind: "rust".into(),
            label: finding.role.clone(),
        }),
        range: Some(diagnostic_range_from_source(&finding.file, finding.range)),
        style_range: None,
        replacement: Some(CliReplacement {
            kind: "decoded-style-value".into(),
            value: finding.expected.clone(),
        }),
    }
}

fn format_failure(findings: &[UtilityFormatFinding]) -> CliFailure {
    CliFailure {
        human: format!(
            "formatting drift detected in {} Rust utility literal(s):\n{}",
            findings.len(),
            findings
                .iter()
                .map(UtilityFormatFinding::human)
                .collect::<Vec<_>>()
                .join("\n")
        ),
        diagnostics: findings.iter().map(format_finding_diagnostic).collect(),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct CompiledArtifact {
    css: String,
    manifest: String,
    format: CssFormat,
    emits_theme: bool,
    styles: Vec<ManifestStyle>,
    findings: Vec<Provenance>,
    token_references: BTreeSet<FlatTokenReference>,
}

#[derive(Debug, Eq, PartialEq)]
enum WatchOutcome {
    Unchanged,
    Compiled(CompiledArtifact),
    Failed(String),
}

#[derive(Debug, Eq, PartialEq)]
struct WatchIteration {
    snapshot: WatchSnapshot,
    cache: SourceCacheStats,
    outcome: WatchOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WatchSnapshot {
    input: Option<WatchFileSnapshot>,
    sources: Result<Vec<WatchFileSnapshot>, String>,
    theme: Result<Option<WatchFileSnapshot>, String>,
    reachability: Option<WatchFileSnapshot>,
}

struct ControlInputSnapshot<'a> {
    logical_path: String,
    role: &'static str,
    bytes: &'a [u8],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CliInputIdentity<'a> {
    styles: &'a [String],
    compositions: &'a [(String, String)],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompilationConfigIdentity<'a> {
    operation: &'a str,
    format: &'a str,
    emit_theme: bool,
    manifest_version: u8,
    prune_unreachable: bool,
    theme_selection: &'a str,
    theme_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    token_inputs: Option<&'a BTreeMap<String, String>>,
}

#[derive(Clone, Copy)]
struct CompilationControlSettings<'a> {
    operation: &'a str,
    styles: &'a [String],
    compositions: &'a [(String, String)],
    theme_request: ControlThemeRequest,
    emit_theme: bool,
    targets: TargetContract,
    format: CssFormat,
    physical_trace: bool,
    pruning: ReachabilityPruning,
}

#[derive(Clone, Copy)]
enum ControlThemeRequest {
    ExplicitConfig,
    ExplicitTokens,
    ForcedSeed,
    Automatic,
}

impl ControlThemeRequest {
    const fn from_arguments(
        explicit_tokens: bool,
        explicit_config: bool,
        forced_seed: bool,
    ) -> Self {
        if explicit_tokens {
            Self::ExplicitTokens
        } else if explicit_config {
            Self::ExplicitConfig
        } else if forced_seed {
            Self::ForcedSeed
        } else {
            Self::Automatic
        }
    }
}

#[derive(Debug)]
enum LoadedTheme {
    Registry(ThemeRegistry),
    Dtcg(DtcgTheme),
}

impl LoadedTheme {
    const fn registry(&self) -> &ThemeRegistry {
        match self {
            Self::Registry(registry) => registry,
            Self::Dtcg(theme) => theme.registry(),
        }
    }

    const fn selections(&self) -> Option<&BTreeMap<String, String>> {
        match self {
            Self::Registry(_) => None,
            Self::Dtcg(theme) => Some(theme.selections()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SourceCacheStats {
    discovered: usize,
    scan_hits: usize,
    scan_misses: usize,
    semantic_hits: usize,
    semantic_misses: usize,
    removed: usize,
}

impl SourceCacheStats {
    const fn has_sources(self) -> bool {
        self.discovered != 0 || self.removed != 0
    }
}

#[derive(Clone, Debug)]
struct CachedRustScan {
    source_name: String,
    contents: Vec<u8>,
    report: Result<ScanReport, String>,
    semantics: Option<CachedRustSemantics>,
}

#[derive(Clone, Debug)]
struct CachedRustSemantics {
    theme_id: u128,
    resolved: Vec<ResolvedCandidate>,
}

#[derive(Default)]
struct RustScanCache {
    entries: BTreeMap<String, CachedRustScan>,
    css: CssCaches,
}

fn main() -> ExitCode {
    let global = match parse_global_options(env::args_os().skip(1)) {
        Ok(global) => global,
        Err(error) => {
            emit_failure(
                error.diagnostic_format,
                error.command.as_deref(),
                &error.failure,
            );
            return ExitCode::FAILURE;
        }
    };
    let command = global
        .arguments
        .first()
        .and_then(|argument| argument.to_str())
        .map(str::to_owned);
    match run(global.arguments, global.diagnostic_format) {
        Ok(exit_code) => exit_code,
        Err(failure) => {
            emit_failure(global.diagnostic_format, command.as_deref(), &failure);
            ExitCode::FAILURE
        }
    }
}

fn parse_global_options(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<GlobalOptions, GlobalOptionsError> {
    let mut diagnostic_format = DiagnosticFormat::Human;
    let mut configured = false;
    let mut output = Vec::new();
    let mut command = None;
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    let recovered_command = recover_leading_command(&arguments);
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        let text = argument.to_str();
        let exact = text == Some("--diagnostic-format");
        let inline = text.and_then(|value| value.strip_prefix("--diagnostic-format="));
        if exact || inline.is_some() {
            if configured {
                return Err(GlobalOptionsError {
                    diagnostic_format,
                    command: command.or_else(|| recovered_command.clone()),
                    failure: CliFailure::invalid("`--diagnostic-format` may only be provided once"),
                });
            }
            let value = if exact {
                index += 1;
                arguments
                    .get(index)
                    .ok_or_else(|| GlobalOptionsError {
                        diagnostic_format,
                        command: command.clone().or_else(|| recovered_command.clone()),
                        failure: CliFailure::invalid(
                            "`--diagnostic-format` requires `human` or `json`",
                        ),
                    })?
                    .clone()
                    .into_string()
                    .map_err(|_| GlobalOptionsError {
                        diagnostic_format,
                        command: command.clone().or_else(|| recovered_command.clone()),
                        failure: CliFailure::invalid(
                            "`--diagnostic-format` value must be valid UTF-8",
                        ),
                    })?
            } else {
                inline.expect("inline value exists").to_owned()
            };
            diagnostic_format = match value.as_str() {
                "human" => DiagnosticFormat::Human,
                "json" => DiagnosticFormat::Json,
                _ => {
                    return Err(GlobalOptionsError {
                        diagnostic_format,
                        command: command.or_else(|| recovered_command.clone()),
                        failure: CliFailure::invalid(
                            "`--diagnostic-format` must be `human` or `json`",
                        ),
                    });
                }
            };
            configured = true;
            index += 1;
            continue;
        }

        output.push(argument.clone());
        if command.is_none() {
            command = text.map(str::to_owned);
        } else {
            let arity = command_option_arity(command.as_deref(), text);
            for _ in 0..arity {
                index += 1;
                if let Some(value) = arguments.get(index) {
                    output.push(value.clone());
                }
            }
        }
        index += 1;
    }
    Ok(GlobalOptions {
        diagnostic_format,
        arguments: output,
    })
}

fn recover_leading_command(arguments: &[OsString]) -> Option<String> {
    let mut index = 0;
    while index < arguments.len() {
        let text = arguments[index].to_str()?;
        if text == "--diagnostic-format" {
            index += 2;
            continue;
        }
        if text.starts_with("--diagnostic-format=") {
            index += 1;
            continue;
        }
        return Some(text.to_owned());
    }
    None
}

fn command_option_arity(command: Option<&str>, option: Option<&str>) -> usize {
    let (Some(command), Some(option)) = (command, option) else {
        return 0;
    };
    match command {
        "compile" | "build" | "check" | "inspect" => match option {
            "--compose" => 2,
            "--style" | "--input" | "--source" | "--config" | "--tokens" | "--token-input"
            | "--targets" | "--format" | "--output" | "--manifest" | "--manifest-version"
            | "--reachability" | "--control-dir" => 1,
            _ => 0,
        },
        "watch" => match option {
            "--input" | "--source" | "--output" | "--config" | "--tokens" | "--token-input"
            | "--targets" | "--format" | "--manifest" | "--manifest-version" | "--reachability"
            | "--control-dir" => 1,
            _ => 0,
        },
        "bundle" => match option {
            "--plan" | "--output-dir" | "--manifest-version" | "--reachability" => 1,
            _ => 0,
        },
        "catalog" => match option {
            "--config" | "--format" | "--output" | "--check" => 1,
            _ => 0,
        },
        "explain" => match option {
            "--style" | "--config" | "--targets" | "--format" => 1,
            _ => 0,
        },
        "explain-cascade" => match option {
            "--input" | "--element" | "--property" | "--format" => 1,
            _ => 0,
        },
        "plan" => match option {
            "--findings" | "--proposal" | "--source-root" | "--format" => 1,
            _ => 0,
        },
        "fix" => match option {
            "--plan" | "--findings" | "--source-root" | "--authorize" | "--receipt"
            | "--format" => 1,
            _ => 0,
        },
        "fmt" => match option {
            "--style" | "--input" | "--source" | "--output" => 1,
            _ => 0,
        },
        "audit" => match option {
            "--input" | "--format" => 1,
            _ => 0,
        },
        "transform-css" => match option {
            "--input" | "--output" | "--targets" | "--format" | "--control-dir" => 1,
            _ => 0,
        },
        _ => 0,
    }
}

fn emit_failure(format: DiagnosticFormat, command: Option<&str>, failure: &CliFailure) {
    match format {
        DiagnosticFormat::Human => eprintln!("error: {failure}\n\n{USAGE}"),
        DiagnosticFormat::Json => {
            let rendered = render_failure_json(command, failure);
            eprintln!("{rendered}");
        }
    }
}

fn render_failure_json(command: Option<&str>, failure: &CliFailure) -> String {
    serde_json::to_string(&DiagnosticDocument {
        schema_version: 1,
        command,
        diagnostics: &failure.diagnostics,
    })
    .expect("diagnostic documents contain only serializable values")
}

#[allow(clippy::too_many_lines)]
fn run(
    arguments: impl IntoIterator<Item = OsString>,
    diagnostic_format: DiagnosticFormat,
) -> Result<ExitCode, CliFailure> {
    let command = parse_arguments(arguments).map_err(CliFailure::invalid)?;
    if diagnostic_format == DiagnosticFormat::Json && matches!(command, Command::Watch(_)) {
        return Err(CliFailure::invalid(
            "`watch` does not support `--diagnostic-format json`; use `human` because watch emits a long-lived event stream",
        ));
    }
    let result = match command {
        Command::Help(command) => {
            println!("{}", command_help(command.as_deref()));
            Ok(())
        }
        Command::Version => {
            println!("pliego-cssc {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Compile(arguments) => {
            if arguments.control_dir.is_some() {
                run_controlled_compile(&arguments)?;
            } else {
                let theme = load_build_theme(&arguments)?;
                let candidates = collect_candidates(theme.registry(), &arguments)?;
                let reachability = load_reachability(arguments.reachability.as_deref())?;
                let artifact = compile_candidates_with_manifest(
                    theme.registry(),
                    &candidates,
                    arguments.theme,
                    arguments.targets,
                    arguments.format,
                    ArtifactGraphOptions {
                        reachability: reachability.as_ref(),
                        physical_trace: arguments.physical_trace,
                        pruning: arguments.pruning,
                        ..ArtifactGraphOptions::default()
                    },
                )?;
                write_outputs(
                    arguments.output.as_deref(),
                    arguments.manifest.as_deref(),
                    &artifact,
                )?;
                if arguments.output.is_none() {
                    print!("{}", artifact.css);
                }
            }
            Ok(())
        }
        Command::Check(arguments) => {
            let theme = load_build_theme(&arguments)?;
            let candidates = collect_candidates(theme.registry(), &arguments)?;
            let artifact = compile_candidates(
                theme.registry(),
                &candidates,
                false,
                arguments.targets,
                arguments.format,
            )?;
            println!(
                "ok: {} finding(s), {} semantic style(s), theme {}, format {}",
                artifact.findings.len(),
                artifact.styles.len(),
                theme.registry().id(),
                arguments.format.as_str()
            );
            Ok(())
        }
        Command::Inspect(arguments) => {
            let theme = load_build_theme(&arguments)?;
            let candidates = collect_candidates(theme.registry(), &arguments)?;
            let artifact = compile_candidates(
                theme.registry(),
                &candidates,
                false,
                arguments.targets,
                arguments.format,
            )?;
            let json = serialize_inspection(theme.registry(), arguments.targets, &artifact)?;
            print!("{json}");
            Ok(())
        }
        Command::Watch(arguments) => watch(&arguments).map_err(Into::into),
        Command::Bundle(arguments) => run_bundle(&arguments),
        Command::Catalog(arguments) => run_catalog(&arguments).map_err(Into::into),
        Command::Compatibility(arguments) => run_compatibility(arguments.targets),
        Command::GenericCssUsage {
            findings,
            observed,
            scope,
            output,
            control_dir,
        } => run_generic_css_usage(
            &findings,
            &observed,
            &scope,
            &output,
            control_dir.as_deref(),
        ),
        Command::Inventory(kind, input) => run_migration_inventory(kind, &input),
        Command::InventoryProject(input) => run_migration_project(&input),
        Command::MigrationProjectPlan(input) => run_reversible_migration_project_plan(&input),
        Command::MigrationSidecarApply { output, receipt } => {
            run_reversible_sidecar_apply(&output, &receipt)
        }
        Command::MigrationSidecarRollback { output, receipt } => {
            run_reversible_sidecar_rollback(&output, &receipt)
        }
        Command::MigrationReplaceApply {
            file,
            before,
            after,
            receipt,
        } => run_reversible_replace_apply(&file, &before, &after, &receipt),
        Command::MigrationReplaceRollback { file, receipt } => {
            run_reversible_replace_rollback(&file, &receipt)
        }
        Command::MigrationGroupApply { manifest, receipt } => {
            run_reversible_group_apply(&manifest, &receipt)
        }
        Command::MigrationGroupRollback { receipt } => run_reversible_group_rollback(&receipt),
        Command::Explain(arguments) => run_explain(&arguments),
        Command::ExplainCascade(arguments) => run_cascade_explain(&arguments),
        Command::Plan(arguments) => run_repair_plan(&arguments),
        Command::Fix(arguments) => run_repair_fix(&arguments),
        Command::Format(arguments) => run_utility_formatter(&arguments),
        Command::Audit(arguments) => return run_audit(&arguments),
        Command::TransformCss(arguments) => run_standard_css_transform(&arguments),
    };
    result?;
    Ok(ExitCode::SUCCESS)
}

#[allow(clippy::too_many_lines)]
fn parse_arguments(arguments: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let arguments = arguments
        .into_iter()
        .map(|argument| {
            argument
                .into_string()
                .map_err(|_| "arguments must be valid UTF-8".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let Some(command) = arguments.first() else {
        return Err("missing command".into());
    };
    if matches!(command.as_str(), "help" | "-h" | "--help") {
        return Ok(Command::Help(None));
    }
    if matches!(command.as_str(), "version" | "-V" | "--version") {
        if arguments.len() != 1 {
            return Err("version does not accept additional arguments".into());
        }
        return Ok(Command::Version);
    }
    if arguments
        .get(1)
        .is_some_and(|argument| matches!(argument.as_str(), "-h" | "--help"))
    {
        return Ok(Command::Help(Some(command.clone())));
    }
    if command == "watch" {
        return parse_watch_arguments(&arguments[1..]);
    }
    if command == "bundle" {
        return parse_bundle_arguments(&arguments[1..]);
    }
    if command == "catalog" {
        return parse_catalog_arguments(&arguments[1..]);
    }
    if command == "compatibility" {
        return parse_compatibility_arguments(&arguments[1..]);
    }
    if command == "generic-css-usage" {
        return parse_generic_css_usage_arguments(&arguments[1..]);
    }
    if matches!(
        command.as_str(),
        "migration-inventory"
            | "migration-project-inventory"
            | "migration-project-plan"
            | "migration-sidecar-apply"
            | "migration-sidecar-rollback"
            | "migration-replace-apply"
            | "migration-replace-rollback"
            | "migration-group-apply"
            | "migration-group-rollback"
    ) {
        return parse_migration_arguments(command, &arguments);
    }
    if command == "explain" {
        return parse_explain_arguments(&arguments[1..]);
    }
    if command == "explain-cascade" {
        return parse_cascade_explain_arguments(&arguments[1..]);
    }
    if matches!(command.as_str(), "plan" | "fix") {
        return parse_repair_cli_arguments(command, &arguments[1..]).map(|repair| match repair {
            RepairCliCommand::Plan(arguments) => Command::Plan(arguments),
            RepairCliCommand::Fix(arguments) => Command::Fix(arguments),
        });
    }
    if command == "fmt" {
        return parse_utility_format_arguments(&arguments[1..]);
    }
    if command == "audit" {
        return parse_audit_arguments(&arguments[1..]);
    }
    if command == "transform-css" {
        return parse_standard_css_transform_arguments(&arguments[1..]);
    }
    let command_kind = match command.as_str() {
        "compile" | "build" => 0,
        "check" => 1,
        "inspect" => 2,
        _ => return Err(format!("unknown command `{command}`")),
    };
    let (mut parsed, manifest_version) = parse_build_arguments(&arguments[1..])?;
    if command_kind != 0
        && (parsed.output.is_some()
            || parsed.manifest.is_some()
            || parsed.control_dir.is_some()
            || parsed.check
            || parsed.theme
            || manifest_version.is_some()
            || parsed.reachability.is_some()
            || parsed.pruning.is_enabled())
    {
        return Err(format!(
            "`{command}` does not accept output, manifest, theme, reachability, or pruning options"
        ));
    }
    if command_kind == 0 {
        parsed.physical_trace = validate_manifest_graph_options(
            manifest_version.as_deref(),
            parsed.reachability.as_deref(),
            parsed.manifest.is_some(),
            parsed.pruning,
        )?;
        validate_build_paths(&parsed)?;
    }
    Ok(match command_kind {
        0 => Command::Compile(parsed),
        1 => Command::Check(parsed),
        _ => Command::Inspect(parsed),
    })
}

#[allow(clippy::too_many_lines)]
fn parse_migration_arguments(command: &str, arguments: &[String]) -> Result<Command, String> {
    if command == "migration-group-apply" {
        let [_, manifest_flag, manifest, receipt_flag, receipt] = arguments else {
            return Err("--manifest FILE --receipt FILE required".into());
        };
        if manifest_flag != "--manifest" || receipt_flag != "--receipt" {
            return Err("--manifest FILE --receipt FILE required".into());
        }
        return Ok(Command::MigrationGroupApply {
            manifest: manifest.into(),
            receipt: receipt.into(),
        });
    }
    if command == "migration-group-rollback" {
        let [_, receipt_flag, receipt] = arguments else {
            return Err("--receipt FILE required".into());
        };
        if receipt_flag != "--receipt" {
            return Err("--receipt FILE required".into());
        }
        return Ok(Command::MigrationGroupRollback {
            receipt: receipt.into(),
        });
    }
    if command == "migration-replace-apply" {
        let [
            _,
            file_flag,
            file,
            before_flag,
            before,
            after_flag,
            after,
            receipt_flag,
            receipt,
        ] = arguments
        else {
            return Err("--file FILE --before FILE --after FILE --receipt FILE required".into());
        };
        if file_flag != "--file"
            || before_flag != "--before"
            || after_flag != "--after"
            || receipt_flag != "--receipt"
        {
            return Err("--file FILE --before FILE --after FILE --receipt FILE required".into());
        }
        return Ok(Command::MigrationReplaceApply {
            file: file.into(),
            before: before.into(),
            after: after.into(),
            receipt: receipt.into(),
        });
    }
    if command == "migration-replace-rollback" {
        let [_, file_flag, file, receipt_flag, receipt] = arguments else {
            return Err("--file FILE --receipt FILE required".into());
        };
        if file_flag != "--file" || receipt_flag != "--receipt" {
            return Err("--file FILE --receipt FILE required".into());
        }
        return Ok(Command::MigrationReplaceRollback {
            file: file.into(),
            receipt: receipt.into(),
        });
    }
    if matches!(
        command,
        "migration-sidecar-apply" | "migration-sidecar-rollback"
    ) {
        let [_, output_flag, output, receipt_flag, receipt] = arguments else {
            return Err("--output FILE --receipt FILE required".into());
        };
        if output_flag != "--output" || receipt_flag != "--receipt" {
            return Err("--output FILE --receipt FILE required".into());
        }
        return Ok(if command == "migration-sidecar-apply" {
            Command::MigrationSidecarApply {
                output: PathBuf::from(output),
                receipt: PathBuf::from(receipt),
            }
        } else {
            Command::MigrationSidecarRollback {
                output: PathBuf::from(output),
                receipt: PathBuf::from(receipt),
            }
        });
    }
    if command == "migration-project-plan" {
        let [_, input] = arguments else {
            return Err("PATH required".into());
        };
        return Ok(Command::MigrationProjectPlan(PathBuf::from(input)));
    }
    if command == "migration-project-inventory" {
        let [_, input] = arguments else {
            return Err("PATH required".into());
        };
        return Ok(Command::InventoryProject(PathBuf::from(input)));
    }
    let [_, kind, input] = arguments else {
        return Err("KIND FILE required".into());
    };
    Ok(Command::Inventory(
        kind.parse().map_err(str::to_owned)?,
        PathBuf::from(input),
    ))
}

fn parse_standard_css_transform_arguments(arguments: &[String]) -> Result<Command, String> {
    let mut input = None;
    let mut output = None;
    let mut targets = TargetContract::Modern;
    let mut format = CssFormat::Minified;
    let mut check = false;
    let mut control_dir = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--input" => {
                set_path_once(&mut input, arguments, index, "--input")?;
                index += 2;
            }
            "--output" => {
                set_path_once(&mut output, arguments, index, "--output")?;
                index += 2;
            }
            "--targets" => {
                targets = parse_targets(
                    arguments
                        .get(index + 1)
                        .ok_or_else(|| "--targets requires a value".to_owned())?,
                )?;
                index += 2;
            }
            "--format" => {
                format = match arguments.get(index + 1).map(String::as_str) {
                    Some("minified") => CssFormat::Minified,
                    Some("pretty") => CssFormat::Pretty,
                    Some(value) => return Err(format!("unsupported CSS format `{value}`")),
                    None => return Err("--format requires a value".into()),
                };
                index += 2;
            }
            "--check" => {
                if check {
                    return Err("--check may only be supplied once".into());
                }
                check = true;
                index += 1;
            }
            "--control-dir" => {
                set_path_once(&mut control_dir, arguments, index, "--control-dir")?;
                index += 2;
            }
            option => return Err(format!("unknown transform-css option `{option}`")),
        }
    }
    Ok(Command::TransformCss(StandardCssTransformArgs {
        input: input.ok_or_else(|| "transform-css requires --input FILE.css".to_owned())?,
        output: output.ok_or_else(|| "transform-css requires --output FILE.css".to_owned())?,
        targets,
        format,
        check,
        control_dir,
    }))
}

fn run_standard_css_transform(arguments: &StandardCssTransformArgs) -> Result<(), CliFailure> {
    let logical_path = audit_logical_path(&arguments.input).map_err(CliFailure::invalid)?;
    let bytes = read_audit_artifact(&arguments.input, "standard CSS input")?;
    let css = std::str::from_utf8(&bytes).map_err(|error| {
        CliFailure::invalid(format!(
            "standard CSS input `{}` is not valid UTF-8: {error}",
            arguments.input.display()
        ))
    })?;
    let format = match arguments.format {
        CssFormat::Minified => StandardCssFormat::Minified,
        CssFormat::Pretty => StandardCssFormat::Pretty,
    };
    let transformed = transform_standard_css(&logical_path, css, arguments.targets, format)
        .map_err(CliFailure::tool)?;
    if let Some(control_dir) = &arguments.control_dir {
        let control_dir = existing_artifact_directory(control_dir, "transform control output")
            .map_err(CliFailure::invalid)?;
        let output_file = control_output_logical_path(&control_dir, &arguments.output)?;
        let outcome =
            audit_standard_css(&logical_path, css, arguments.targets).map_err(CliFailure::tool)?;
        let relationships = Vec::new();
        let generated_outputs = [ControlOutputInput {
            file: &output_file,
            role: "standard-css",
            media_type: "text/css",
            bytes: transformed.css().as_bytes(),
            source_map: None,
            relationships: &relationships,
        }];
        let source_inputs = [AuditSourceInput {
            logical_path: &logical_path,
            role: "source",
            bytes: &bytes,
        }];
        let group = build_audit_control_group(AuditControlInput {
            source_inputs: &source_inputs,
            config_inputs: &[],
            generated_outputs: &generated_outputs,
            document: outcome.document(),
            passed: outcome.passed(),
            rule_metrics: outcome.inventory().map(CssBudgetInventory::file),
            rules_unavailable_reason: "standard CSS syntax ingestion failed",
            token_observation: TokenObservationInput::Unavailable(
                "standard CSS transform has no typed token graph input",
            ),
            token_graph: None,
            output_relationships: std::slice::from_ref(&output_file),
            asset_plan_verified: false,
            targets: arguments.targets,
            budget_policy: None,
            budget_policy_path: None,
            budget_policy_bytes: None,
            budget_subjects: &[],
        })
        .map_err(CliFailure::tool)?;
        let mut payloads = vec![BundleOutputPayload {
            destination: arguments.output.clone(),
            bytes: transformed.css().as_bytes().to_vec(),
        }];
        payloads.extend(group.artifacts().map(|artifact| BundleOutputPayload {
            destination: control_dir.join(artifact.file),
            bytes: artifact.bytes.to_vec(),
        }));
        validate_path_roles(
            &[("transform input", arguments.input.as_path())],
            &payloads
                .iter()
                .map(|payload| ("transform output", payload.destination.as_path()))
                .collect::<Vec<_>>(),
        )
        .map_err(CliFailure::invalid)?;
        if arguments.check {
            return check_output_group(&payloads).map_err(CliFailure::invalid);
        }
        publish_output_group(&payloads).map_err(CliFailure::tool)?;
        return Ok(());
    }
    if arguments.check {
        let existing = fs::read(&arguments.output).map_err(|error| {
            CliFailure::invalid(format!(
                "cannot read transform output `{}` for --check: {error}",
                arguments.output.display()
            ))
        })?;
        if existing != transformed.css().as_bytes() {
            return Err(CliFailure::invalid(format!(
                "standard CSS output `{}` is out of date",
                arguments.output.display()
            )));
        }
        return Ok(());
    }
    if let Some(write) =
        prepare_atomic_write_if_changed(&arguments.output, transformed.css().as_bytes())
            .map_err(CliFailure::tool)?
    {
        write.commit().map_err(CliFailure::tool)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn parse_audit_arguments(arguments: &[String]) -> Result<Command, String> {
    let mut input = None;
    let mut asset_plan = None;
    let mut format = AuditFormat::Human;
    let mut format_seen = false;
    let mut targets = None;
    let mut budget_policy = None;
    let mut ownership = None;
    let mut accessibility_policy = None;
    let mut token_graph = None;
    let mut budget_subjects = Vec::new();
    let mut control_dir = None;
    let mut check = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--input" => {
                set_path_once(&mut input, arguments, index, "--input")?;
                index += 2;
            }
            "--asset-plan" => {
                set_path_once(&mut asset_plan, arguments, index, "--asset-plan")?;
                index += 2;
            }
            "--format" => {
                set_flag_once(&mut format_seen, "--format")?;
                format = match option_value(arguments, index, "--format")? {
                    "human" => AuditFormat::Human,
                    "json" => AuditFormat::Json,
                    "sarif" => AuditFormat::Sarif,
                    _ => return Err("`audit --format` must be `human`, `json`, or `sarif`".into()),
                };
                index += 2;
            }
            "--targets" => {
                if targets.is_some() {
                    return Err("`--targets` may only be provided once".into());
                }
                targets = Some(parse_targets(option_value(arguments, index, "--targets")?)?);
                index += 2;
            }
            "--budget-policy" => {
                set_path_once(&mut budget_policy, arguments, index, "--budget-policy")?;
                index += 2;
            }
            "--ownership" => {
                set_path_once(&mut ownership, arguments, index, "--ownership")?;
                index += 2;
            }
            "--accessibility-policy" => {
                set_path_once(
                    &mut accessibility_policy,
                    arguments,
                    index,
                    "--accessibility-policy",
                )?;
                index += 2;
            }
            "--token-graph" => {
                set_path_once(&mut token_graph, arguments, index, "--token-graph")?;
                index += 2;
            }
            "--budget-subject" => {
                budget_subjects.push(parse_budget_subject(option_value(
                    arguments,
                    index,
                    "--budget-subject",
                )?)?);
                index += 2;
            }
            "--control-dir" => {
                set_path_once(&mut control_dir, arguments, index, "--control-dir")?;
                index += 2;
            }
            "--check" => {
                set_flag_once(&mut check, "--check")?;
                index += 1;
            }
            option => return Err(format!("unknown audit option `{option}`")),
        }
    }
    if budget_policy.is_none() && !budget_subjects.is_empty() {
        return Err("`audit --budget-subject` requires `--budget-policy FILE`".into());
    }
    if accessibility_policy.is_none() && token_graph.is_some() {
        return Err("`audit --token-graph` requires `--accessibility-policy FILE`".into());
    }
    if check && control_dir.is_none() {
        return Err("`audit --check` requires `--control-dir DIR`".into());
    }
    let input = match (input, asset_plan) {
        (Some(path), None) => {
            if ownership.is_some() {
                return Err("`audit --ownership` requires `--asset-plan FILE`".into());
            }
            AuditInput::Css(path)
        }
        (None, Some(path)) => {
            if !budget_subjects.is_empty() {
                return Err(
                    "`audit --budget-subject` is accepted only with `--input FILE`; Asset Plan package and route subjects require `--ownership FILE`"
                        .into(),
                );
            }
            AuditInput::AssetPlan(path)
        }
        (Some(_), Some(_)) => {
            return Err(
                "`audit` accepts exactly one of `--input FILE` or `--asset-plan FILE`".into(),
            );
        }
        (None, None) => {
            return Err("`audit` requires `--input FILE` or `--asset-plan FILE`".into());
        }
    };
    Ok(Command::Audit(AuditArgs {
        input,
        format,
        targets: targets.ok_or_else(|| "`audit` requires `--targets PROFILE`".to_owned())?,
        budget_policy,
        ownership,
        accessibility_policy,
        token_graph,
        budget_subjects,
        control_dir,
        check,
    }))
}

#[allow(clippy::too_many_lines)]
fn run_audit(arguments: &AuditArgs) -> Result<ExitCode, CliFailure> {
    let budget_policy = arguments
        .budget_policy
        .as_deref()
        .map(read_budget_policy)
        .transpose()
        .map_err(CliFailure::invalid)?;
    let accessibility_policy = arguments
        .accessibility_policy
        .as_deref()
        .map(read_accessibility_policy)
        .transpose()?;
    let token_graph = arguments
        .token_graph
        .as_deref()
        .map(read_audit_token_graph)
        .transpose()?;
    let policy = budget_policy.as_ref().map(|snapshot| &snapshot.policy);
    let (document, passed) = match &arguments.input {
        AuditInput::Css(path) => {
            let logical_path = audit_logical_path(path).map_err(CliFailure::invalid)?;
            let bytes = read_audit_artifact(path, "input")?;
            let css = std::str::from_utf8(&bytes).map_err(|error| {
                CliFailure::invalid(format!(
                    "audit input `{}` is not valid UTF-8: {error}",
                    path.display()
                ))
            })?;
            let outcome = audit_standard_css_with_budgets(
                &logical_path,
                css,
                arguments.targets,
                policy,
                &arguments.budget_subjects,
            )
            .map_err(CliFailure::tool)?;
            let mut findings = outcome.document().findings().to_vec();
            let mut passed = outcome.passed();
            let mut contrast_pairs = 0;
            if let Some(snapshot) = &accessibility_policy {
                let accessibility = evaluate_accessibility(
                    &[AccessibilityStylesheetInput {
                        logical_path: &logical_path,
                        css,
                    }],
                    AccessibilityPolicySource {
                        logical_path: &snapshot.logical_path,
                        bytes: &snapshot.bytes,
                    },
                    token_graph.as_ref().map(|snapshot| &snapshot.graph),
                )
                .map_err(CliFailure::tool)?;
                passed &= accessibility.passed;
                contrast_pairs = accessibility.contrast_pairs;
                findings.extend(accessibility.findings);
            }
            let document = FindingDocument::new(
                FindingTool::new("pliegocss", env!("CARGO_PKG_VERSION"))
                    .map_err(|error| CliFailure::tool(error.to_string()))?,
                "audit",
                findings,
            )
            .map_err(|error| CliFailure::tool(error.to_string()))?;
            if let Some(output_dir) = &arguments.control_dir {
                let source_inputs = [AuditSourceInput {
                    logical_path: &logical_path,
                    role: "source",
                    bytes: &bytes,
                }];
                let mut config_inputs = Vec::with_capacity(2);
                if let Some(snapshot) = &accessibility_policy {
                    config_inputs.push(AuditSourceInput {
                        logical_path: &snapshot.logical_path,
                        role: "accessibility-policy",
                        bytes: &snapshot.bytes,
                    });
                }
                if let Some(snapshot) = &token_graph {
                    config_inputs.push(AuditSourceInput {
                        logical_path: &snapshot.logical_path,
                        role: "token-graph",
                        bytes: &snapshot.bytes,
                    });
                }
                let token_measurements = token_graph
                    .as_ref()
                    .map(|snapshot| {
                        build_audit_token_graph_measurements(&snapshot.graph, contrast_pairs)
                    })
                    .transpose()
                    .map_err(CliFailure::tool)?;
                let token_observation = match &token_measurements {
                    Some(measurements) => TokenObservationInput::Measured(measurements),
                    None => TokenObservationInput::Unavailable(
                        "direct CSS audit has no typed token graph input",
                    ),
                };
                let group = build_audit_control_group(AuditControlInput {
                    source_inputs: &source_inputs,
                    config_inputs: &config_inputs,
                    generated_outputs: &[],
                    document: &document,
                    passed,
                    rule_metrics: outcome.inventory().map(CssBudgetInventory::file),
                    rules_unavailable_reason: "standard CSS syntax ingestion failed",
                    token_observation,
                    token_graph: token_graph.as_ref().map(|snapshot| &snapshot.graph),
                    output_relationships: &[],
                    asset_plan_verified: false,
                    targets: arguments.targets,
                    budget_policy: policy,
                    budget_policy_path: budget_policy
                        .as_ref()
                        .map(|snapshot| snapshot.logical_path.as_str()),
                    budget_policy_bytes: budget_policy
                        .as_ref()
                        .map(|snapshot| snapshot.bytes.as_slice()),
                    budget_subjects: &arguments.budget_subjects,
                })
                .map_err(CliFailure::tool)?;
                let mut control_inputs = vec![("--input", path.as_path())];
                if let Some(policy) = arguments.budget_policy.as_deref() {
                    control_inputs.push(("--budget-policy", policy));
                }
                if let Some(snapshot) = &accessibility_policy {
                    control_inputs
                        .push(("--accessibility-policy", snapshot.physical_path.as_path()));
                }
                if let Some(snapshot) = &token_graph {
                    control_inputs.push(("--token-graph", snapshot.physical_path.as_path()));
                }
                publish_audit_control_group(&group, output_dir, arguments.check, &control_inputs)?;
            }
            (document, passed)
        }
        AuditInput::AssetPlan(path) => run_asset_plan_audit(
            path,
            arguments.targets,
            budget_policy.as_ref(),
            arguments.ownership.as_deref(),
            accessibility_policy.as_ref(),
            token_graph.as_ref(),
            arguments.control_dir.as_deref(),
            arguments.check,
        )?,
    };
    let rendered = match arguments.format {
        AuditFormat::Human => document.to_human().map_err(|error| error.to_string()),
        AuditFormat::Json => document.to_json_pretty().map_err(|error| error.to_string()),
        AuditFormat::Sarif => render_sarif(&document),
    }
    .map_err(CliFailure::tool)?;
    print!("{rendered}");
    Ok(if passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

struct AuditedAssetBundle {
    id: String,
    emits_theme: bool,
    css_path: PathBuf,
    manifest_path: PathBuf,
    logical_css_path: String,
    logical_manifest_path: String,
    css: String,
    manifest: Vec<u8>,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn run_asset_plan_audit(
    path: &Path,
    targets: TargetContract,
    budget_policy: Option<&AuditBudgetPolicy>,
    ownership_path: Option<&Path>,
    accessibility_policy: Option<&AuditAccessibilityPolicy>,
    token_graph: Option<&AuditTokenGraph>,
    control_dir: Option<&Path>,
    check: bool,
) -> Result<(FindingDocument, bool), CliFailure> {
    let policy = budget_policy.map(|snapshot| &snapshot.policy);
    if ownership_path.is_none()
        && policy.is_some_and(|policy| {
            policy.uses_subject_kind(BudgetSubjectKind::Package)
                || policy.uses_subject_kind(BudgetSubjectKind::Route)
        })
    {
        return Err(CliFailure::invalid(
            "asset-plan package or route budgets require `audit --ownership FILE`",
        ));
    }
    let logical_plan_path =
        audit_logical_path_for(path, "--asset-plan", "asset plan").map_err(CliFailure::invalid)?;
    let plan_bytes = read_audit_artifact(path, "asset plan")?;
    let plan =
        parse_asset_plan(&plan_bytes).map_err(|error| CliFailure::invalid(error.to_string()))?;
    let base = path.parent().unwrap_or_else(|| Path::new(""));
    let mut bundles = Vec::with_capacity(plan.bundles().len());
    for bundle in plan.bundles() {
        let css_path = base.join(bundle.css_file());
        let manifest_path = base.join(bundle.manifest_file());
        let logical_css_path = audit_logical_path_for(&css_path, "--asset-plan", "bundle CSS")
            .map_err(CliFailure::invalid)?;
        let logical_manifest_path =
            audit_logical_path_for(&manifest_path, "--asset-plan", "bundle manifest")
                .map_err(CliFailure::invalid)?;
        let css_bytes = read_audit_artifact(&css_path, "asset-plan bundle CSS")?;
        let css = String::from_utf8(css_bytes).map_err(|error| {
            CliFailure::invalid(format!(
                "asset-plan bundle CSS `{}` is not valid UTF-8: {error}",
                css_path.display()
            ))
        })?;
        bundles.push(AuditedAssetBundle {
            id: bundle.id().to_owned(),
            emits_theme: bundle.emits_theme(),
            css_path,
            manifest_path: manifest_path.clone(),
            logical_css_path,
            logical_manifest_path,
            css,
            manifest: read_audit_artifact(&manifest_path, "asset-plan bundle manifest")?,
        });
    }
    let regeneration_inputs = bundles
        .iter()
        .map(|bundle| {
            AssetPlanBundle::new(
                &bundle.id,
                bundle.emits_theme,
                bundle.css.as_bytes(),
                &bundle.manifest,
            )
        })
        .collect::<Vec<_>>();
    let rule_selection = match plan.rule_selection() {
        OwnershipRuleSelection::AllCompiled => AssetRuleSelection::AllCompiled,
        OwnershipRuleSelection::ReachableStyleIds => AssetRuleSelection::ReachableStyleIds,
        OwnershipRuleSelection::ReachableOrRetainedStyleIds => {
            AssetRuleSelection::ReachableOrRetainedStyleIds
        }
    };
    let regenerated = build_asset_plan(&regeneration_inputs, rule_selection)
        .map_err(|error| CliFailure::invalid(format!("invalid asset-plan ledger: {error}")))?;
    if regenerated != plan_bytes {
        return Err(CliFailure::invalid(
            "asset plan does not match canonical regeneration from its adjacent CSS and manifests",
        ));
    }
    let ownership_source = ownership_path.map(read_audit_ownership).transpose()?;
    let ownership = ownership_source
        .as_ref()
        .map(|snapshot| parse_ownership(&snapshot.bytes, &plan))
        .transpose()
        .map_err(|error| CliFailure::invalid(error.to_string()))?;

    let plan_source = FindingSource::new(&logical_plan_path, 0, plan_bytes.len())
        .map_err(|error| CliFailure::tool(error.to_string()))?;
    let mut findings = Vec::new();
    let mut passed = true;
    let mut observations = Vec::new();
    let mut bundle_metrics = BTreeMap::<String, CssBudgetMetrics>::new();
    let mut plan_metrics = CssBudgetMetrics::default();
    let mut layer_metrics = BTreeMap::<String, (CssBudgetMetrics, FindingSource)>::new();
    let mut unavailable = Vec::new();
    for bundle in &bundles {
        let outcome = audit_standard_css(&bundle.logical_css_path, &bundle.css, targets)
            .map_err(CliFailure::tool)?;
        passed &= outcome.passed();
        findings.extend(outcome.document().findings().iter().cloned());
        let source = FindingSource::new(&bundle.logical_css_path, 0, bundle.css.len())
            .map_err(|error| CliFailure::tool(error.to_string()))?;
        let Some(inventory) = outcome.inventory() else {
            unavailable.push((bundle.id.clone(), source));
            continue;
        };
        let file_subject = BudgetSubject::new(BudgetSubjectKind::File, &bundle.logical_css_path)
            .map_err(|error| CliFailure::tool(error.to_string()))?;
        observations.push((
            BudgetObservation::new(file_subject, inventory.file().measurements()),
            source.clone(),
        ));
        plan_metrics
            .try_merge(inventory.file())
            .map_err(|error| CliFailure::tool(error.to_string()))?;
        bundle_metrics.insert(bundle.id.clone(), inventory.file().clone());
        for layer in inventory.layers() {
            if let Some((metrics, _)) = layer_metrics.get_mut(layer.name()) {
                metrics
                    .try_merge(layer.metrics())
                    .map_err(|error| CliFailure::tool(error.to_string()))?;
            } else {
                let source = css_layer_finding_source(
                    &bundle.logical_css_path,
                    &bundle.css,
                    layer.first_location(),
                )
                .map_err(CliFailure::tool)?;
                layer_metrics.insert(layer.name().to_owned(), (layer.metrics().clone(), source));
            }
        }
    }
    if policy.is_some() && unavailable.is_empty() {
        if let Some(ownership) = &ownership {
            observations.extend(ownership_budget_observations(
                ownership,
                &bundle_metrics,
                &plan_source,
            )?);
        }
        for (name, (metrics, source)) in layer_metrics {
            let subject = BudgetSubject::new(BudgetSubjectKind::Layer, name)
                .map_err(|error| CliFailure::tool(error.to_string()))?;
            observations.push((
                BudgetObservation::new(subject, metrics.measurements()),
                source,
            ));
        }
    }
    if let Some(policy) = policy {
        if unavailable.is_empty() {
            let (budget_findings, budget_failed) =
                evaluate_budget_observation_findings(policy, &observations, &plan_source)
                    .map_err(CliFailure::tool)?;
            passed &= !budget_failed;
            findings.extend(budget_findings);
        } else {
            passed = false;
            for (bundle, source) in &unavailable {
                findings.push(unavailable_budget_finding(policy, bundle, source.clone())?);
            }
        }
    }
    let mut contrast_pairs = 0;
    if let Some(snapshot) = accessibility_policy {
        let stylesheets = bundles
            .iter()
            .map(|bundle| AccessibilityStylesheetInput {
                logical_path: &bundle.logical_css_path,
                css: &bundle.css,
            })
            .collect::<Vec<_>>();
        let accessibility = evaluate_accessibility(
            &stylesheets,
            AccessibilityPolicySource {
                logical_path: &snapshot.logical_path,
                bytes: &snapshot.bytes,
            },
            token_graph.map(|snapshot| &snapshot.graph),
        )
        .map_err(CliFailure::tool)?;
        passed &= accessibility.passed;
        contrast_pairs = accessibility.contrast_pairs;
        findings.extend(accessibility.findings);
    }
    let document = FindingDocument::new(
        FindingTool::new("pliegocss", env!("CARGO_PKG_VERSION"))
            .map_err(|error| CliFailure::tool(error.to_string()))?,
        "audit",
        findings,
    )
    .map_err(|error| CliFailure::tool(error.to_string()))?;
    if let Some(output_dir) = control_dir {
        let mut source_inputs = Vec::with_capacity(1 + bundles.len() * 2);
        source_inputs.push(AuditSourceInput {
            logical_path: &logical_plan_path,
            role: "asset-plan",
            bytes: &plan_bytes,
        });
        for bundle in &bundles {
            source_inputs.push(AuditSourceInput {
                logical_path: &bundle.logical_css_path,
                role: "source",
                bytes: bundle.css.as_bytes(),
            });
            source_inputs.push(AuditSourceInput {
                logical_path: &bundle.logical_manifest_path,
                role: "style-manifest",
                bytes: &bundle.manifest,
            });
        }
        let mut config_inputs = Vec::with_capacity(3);
        if let Some(snapshot) = &ownership_source {
            config_inputs.push(AuditSourceInput {
                logical_path: &snapshot.logical_path,
                role: "ownership",
                bytes: &snapshot.bytes,
            });
        }
        if let Some(snapshot) = accessibility_policy {
            config_inputs.push(AuditSourceInput {
                logical_path: &snapshot.logical_path,
                role: "accessibility-policy",
                bytes: &snapshot.bytes,
            });
        }
        if let Some(snapshot) = token_graph {
            config_inputs.push(AuditSourceInput {
                logical_path: &snapshot.logical_path,
                role: "token-graph",
                bytes: &snapshot.bytes,
            });
        }
        let token_measurements = token_graph
            .map(|snapshot| build_audit_token_graph_measurements(&snapshot.graph, contrast_pairs))
            .transpose()
            .map_err(CliFailure::tool)?;
        let token_observation = match &token_measurements {
            Some(measurements) => TokenObservationInput::Measured(measurements),
            None => TokenObservationInput::Unavailable(
                "asset plan audit has no typed token graph input",
            ),
        };
        let relationships = vec![logical_plan_path.clone()];
        let group = build_audit_control_group(AuditControlInput {
            source_inputs: &source_inputs,
            config_inputs: &config_inputs,
            generated_outputs: &[],
            document: &document,
            passed,
            rule_metrics: unavailable.is_empty().then_some(&plan_metrics),
            rules_unavailable_reason:
                "one or more asset-plan bundle stylesheets failed syntax ingestion",
            token_observation,
            token_graph: token_graph.map(|snapshot| &snapshot.graph),
            output_relationships: &relationships,
            asset_plan_verified: true,
            targets,
            budget_policy: policy,
            budget_policy_path: budget_policy.map(|snapshot| snapshot.logical_path.as_str()),
            budget_policy_bytes: budget_policy.map(|snapshot| snapshot.bytes.as_slice()),
            budget_subjects: &[],
        })
        .map_err(CliFailure::tool)?;
        let mut control_inputs = vec![("--asset-plan", path)];
        for bundle in &bundles {
            control_inputs.push(("--asset-plan", bundle.css_path.as_path()));
            control_inputs.push(("--asset-plan", bundle.manifest_path.as_path()));
        }
        if let Some(snapshot) = budget_policy {
            control_inputs.push(("--budget-policy", snapshot.physical_path.as_path()));
        }
        if let Some(snapshot) = &ownership_source {
            control_inputs.push(("--ownership", snapshot.physical_path.as_path()));
        }
        if let Some(snapshot) = accessibility_policy {
            control_inputs.push(("--accessibility-policy", snapshot.physical_path.as_path()));
        }
        if let Some(snapshot) = token_graph {
            control_inputs.push(("--token-graph", snapshot.physical_path.as_path()));
        }
        publish_audit_control_group(&group, output_dir, check, &control_inputs)?;
    }
    Ok((document, passed))
}

fn ownership_budget_observations(
    ownership: &Ownership,
    bundle_metrics: &BTreeMap<String, CssBudgetMetrics>,
    source: &FindingSource,
) -> Result<Vec<(BudgetObservation, FindingSource)>, CliFailure> {
    let mut observations =
        Vec::with_capacity(ownership.packages().len() + ownership.composed_routes().len());
    for package in ownership.packages() {
        let metrics = merge_owned_bundle_metrics(package.bundle_ids(), bundle_metrics)?;
        let subject = BudgetSubject::new(BudgetSubjectKind::Package, package.package_id())
            .map_err(|error| CliFailure::tool(error.to_string()))?;
        observations.push((
            BudgetObservation::new(subject, metrics.measurements()),
            source.clone(),
        ));
    }
    for route in ownership.composed_routes() {
        let metrics = merge_owned_bundle_metrics(route.bundle_ids(), bundle_metrics)?;
        let subject = BudgetSubject::new(BudgetSubjectKind::Route, route.path())
            .map_err(|error| CliFailure::tool(error.to_string()))?;
        observations.push((
            BudgetObservation::new(subject, metrics.measurements()),
            source.clone(),
        ));
    }
    Ok(observations)
}

fn merge_owned_bundle_metrics(
    bundle_ids: &[String],
    bundle_metrics: &BTreeMap<String, CssBudgetMetrics>,
) -> Result<CssBudgetMetrics, CliFailure> {
    let mut merged = CssBudgetMetrics::default();
    for bundle_id in bundle_ids {
        let metrics = bundle_metrics.get(bundle_id).ok_or_else(|| {
            CliFailure::tool(format!(
                "validated ownership references unmeasured bundle `{bundle_id}`"
            ))
        })?;
        merged
            .try_merge(metrics)
            .map_err(|error| CliFailure::tool(error.to_string()))?;
    }
    Ok(merged)
}

fn unavailable_budget_finding(
    policy: &BudgetPolicy,
    bundle: &str,
    source: FindingSource,
) -> Result<Finding, CliFailure> {
    let policy_json = policy
        .to_json_pretty()
        .map_err(|error| CliFailure::tool(error.to_string()))?;
    Finding::new(
        "PCSS-BUDGET-198",
        "budget",
        FindingSeverity::Error,
        format!("budget measurement unavailable for asset-plan bundle `{bundle}`"),
        FindingVerification::Verified,
        FindingCause::new(
            "budget.measurement",
            "syntax-unavailable",
            "route/package/layer budgets require every integrity-bound bundle CSS to pass standard syntax ingestion",
        )
        .map_err(|error| CliFailure::tool(error.to_string()))?,
    )
    .and_then(|finding| finding.with_source(source))
    .and_then(|finding| finding.with_context("bundle-id", bundle))
    .and_then(|finding| {
        finding.with_context(
            "budget-policy-sha256",
            format!("sha256:{}", sha256_hex(policy_json.as_bytes())),
        )
    })
    .and_then(|finding| {
        finding.with_context("budget-policy-version", policy.policy_version().to_string())
    })
    .map_err(|error| CliFailure::tool(error.to_string()))
}

fn read_audit_artifact(path: &Path, role: &str) -> Result<Vec<u8>, CliFailure> {
    read_command_artifact(path, "audit", role)
}

fn reject_audit_link_components(path: &Path, role: &str) -> Result<(), String> {
    reject_command_link_components(path, "audit", role)
}

fn read_command_artifact(path: &Path, command: &str, role: &str) -> Result<Vec<u8>, CliFailure> {
    reject_command_link_components(path, command, role).map_err(CliFailure::invalid)?;
    read_bounded_utf8_document(path, &format!("{command} {role}")).map_err(CliFailure::invalid)
}

fn reject_command_link_components(path: &Path, command: &str, role: &str) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => continue,
            Component::Normal(value) => current.push(value),
            _ => {
                return Err(format!(
                    "{command} {role} `{}` must be a project-relative path without parent components",
                    path.display()
                ));
            }
        }
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            format!(
                "cannot inspect {command} {role} path component `{}`: {error}",
                current.display()
            )
        })?;
        if is_link_like(&metadata) {
            return Err(format!(
                "{command} {role} `{}` must not traverse symbolic links or reparse points",
                path.display()
            ));
        }
    }
    Ok(())
}

fn parse_budget_subject(value: &str) -> Result<BudgetSubject, String> {
    let (kind, id) = value.split_once('=').ok_or_else(|| {
        "`audit --budget-subject` must use `package=NAME` or `route=ID`".to_owned()
    })?;
    let kind = match kind {
        "package" => BudgetSubjectKind::Package,
        "route" => BudgetSubjectKind::Route,
        _ => {
            return Err(
                "`audit --budget-subject` kind must be `package` or `route`; file and layer subjects are automatic"
                    .into(),
            );
        }
    };
    BudgetSubject::new(kind, id).map_err(|error| error.to_string())
}

struct AuditBudgetPolicy {
    policy: BudgetPolicy,
    physical_path: PathBuf,
    logical_path: String,
    bytes: Vec<u8>,
}

struct AuditOwnership {
    physical_path: PathBuf,
    logical_path: String,
    bytes: Vec<u8>,
}

struct AuditAccessibilityPolicy {
    physical_path: PathBuf,
    logical_path: String,
    bytes: Vec<u8>,
}

struct AuditTokenGraph {
    graph: TokenGraph,
    physical_path: PathBuf,
    logical_path: String,
    bytes: Vec<u8>,
}

fn read_accessibility_policy(path: &Path) -> Result<AuditAccessibilityPolicy, CliFailure> {
    let logical_path =
        audit_logical_path_for(path, "--accessibility-policy", "accessibility policy")
            .map_err(CliFailure::invalid)?;
    let bytes = read_audit_artifact(path, "accessibility policy")?;
    parse_accessibility_policy(&bytes).map_err(|error| CliFailure::invalid(error.to_string()))?;
    Ok(AuditAccessibilityPolicy {
        physical_path: path.to_owned(),
        logical_path,
        bytes,
    })
}

fn read_audit_token_graph(path: &Path) -> Result<AuditTokenGraph, CliFailure> {
    let logical_path = audit_logical_path_for(path, "--token-graph", "token graph")
        .map_err(CliFailure::invalid)?;
    let bytes = read_audit_artifact(path, "token graph")?;
    let graph =
        parse_token_graph(&bytes).map_err(|error| CliFailure::invalid(error.to_string()))?;
    Ok(AuditTokenGraph {
        graph,
        physical_path: path.to_owned(),
        logical_path,
        bytes,
    })
}

fn read_budget_policy(path: &Path) -> Result<AuditBudgetPolicy, String> {
    let logical_path = audit_logical_path_for(path, "--budget-policy", "budget policy")?;
    reject_audit_link_components(path, "budget policy")?;
    let bytes = read_bounded_utf8_document(path, "audit budget policy")?;
    let policy = parse_budget_policy(&bytes).map_err(|error| error.to_string())?;
    Ok(AuditBudgetPolicy {
        policy,
        physical_path: path.to_owned(),
        logical_path,
        bytes,
    })
}

fn read_audit_ownership(path: &Path) -> Result<AuditOwnership, CliFailure> {
    let logical_path = audit_logical_path_for(path, "--ownership", "ownership contract")
        .map_err(CliFailure::invalid)?;
    let bytes = read_audit_artifact(path, "ownership contract")?;
    Ok(AuditOwnership {
        physical_path: path.to_owned(),
        logical_path,
        bytes,
    })
}

fn audit_logical_path(path: &Path) -> Result<String, String> {
    audit_logical_path_for(path, "--input", "input")
}

fn audit_logical_path_for(path: &Path, option: &str, noun: &str) -> Result<String, String> {
    command_logical_path_for(path, option, "audit", noun)
}

fn command_logical_path_for(
    path: &Path,
    option: &str,
    command: &str,
    noun: &str,
) -> Result<String, String> {
    if path.is_absolute() {
        return Err(format!(
            "`{command} {option}` must be a project-relative logical path"
        ));
    }
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::CurDir => None,
            Component::Normal(value) => Some(value.to_str().map(str::to_owned).ok_or_else(|| {
                format!("{command} {noun} `{}` is not valid UTF-8", path.display())
            })),
            _ => Some(Err(format!(
                "{command} {noun} `{}` must not contain parent or root components",
                path.display()
            ))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if components.is_empty() {
        return Err(format!("`{command} {option}` must name a file"));
    }
    Ok(components.join("/"))
}

#[allow(clippy::too_many_lines)]
fn parse_bundle_arguments(arguments: &[String]) -> Result<Command, String> {
    let mut plan = None;
    let mut output_dir = None;
    let mut check = false;
    let mut asset_plan = false;
    let mut project_index = false;
    let mut usage_report = false;
    let mut control = false;
    let mut observations = None;
    let mut retention = None;
    let mut critical_evidence = None;
    let mut manifest_version = None;
    let mut reachability = None;
    let mut pruning = ReachabilityPruning::Disabled;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--plan" => {
                set_path_once(&mut plan, arguments, index, "--plan")?;
                index += 2;
            }
            "--output-dir" => {
                set_path_once(&mut output_dir, arguments, index, "--output-dir")?;
                index += 2;
            }
            "--check" => {
                if check {
                    return Err("`--check` may only be provided once".into());
                }
                check = true;
                index += 1;
            }
            "--asset-plan" => {
                set_flag_once(&mut asset_plan, "--asset-plan")?;
                index += 1;
            }
            "--project-index" => {
                set_flag_once(&mut project_index, "--project-index")?;
                index += 1;
            }
            "--usage-report" => {
                set_flag_once(&mut usage_report, "--usage-report")?;
                index += 1;
            }
            "--observations" => {
                set_path_once(&mut observations, arguments, index, "--observations")?;
                index += 2;
            }
            "--retention" => {
                set_path_once(&mut retention, arguments, index, "--retention")?;
                index += 2;
            }
            "--critical-evidence" => {
                set_path_once(
                    &mut critical_evidence,
                    arguments,
                    index,
                    "--critical-evidence",
                )?;
                index += 2;
            }
            "--control" => {
                set_flag_once(&mut control, "--control")?;
                index += 1;
            }
            "--manifest-version" => {
                set_string_once(
                    &mut manifest_version,
                    arguments,
                    index,
                    "--manifest-version",
                )?;
                index += 2;
            }
            "--reachability" => {
                set_path_once(&mut reachability, arguments, index, "--reachability")?;
                index += 2;
            }
            "--prune-unreachable" => {
                set_pruning_once(&mut pruning)?;
                index += 1;
            }
            option => return Err(format!("unknown bundle option `{option}`")),
        }
    }
    let physical_trace = validate_manifest_graph_options(
        manifest_version.as_deref(),
        reachability.as_deref(),
        true,
        pruning,
    )?;
    if asset_plan && !matches!(manifest_version.as_deref(), Some("4" | "5")) {
        return Err("`--asset-plan` requires manifest version 4 or 5 and `--reachability`".into());
    }
    if project_index && (!asset_plan || manifest_version.as_deref() != Some("5")) {
        return Err(
            "`--project-index` requires `--asset-plan`, manifest version 5, and `--reachability`"
                .into(),
        );
    }
    if observations.is_some() && (!usage_report || reachability.is_none()) {
        return Err(
            "`--observations` requires `--usage-report` and exact `--reachability` evidence".into(),
        );
    }
    if retention.is_some() && (!usage_report || reachability.is_none() || !pruning.is_enabled()) {
        return Err(
            "`--retention` requires `--usage-report`, `--prune-unreachable`, and exact `--reachability` evidence"
                .into(),
        );
    }
    if critical_evidence.is_some()
        && (!usage_report
            || !asset_plan
            || reachability.is_none()
            || manifest_version.as_deref() != Some("5"))
    {
        return Err(
            "`--critical-evidence` requires `--usage-report`, `--asset-plan`, manifest version 5, and exact `--reachability` evidence"
                .into(),
        );
    }
    if control && !asset_plan {
        return Err("`--control` requires `--asset-plan`".into());
    }
    Ok(Command::Bundle(BundleArgs {
        plan: plan.ok_or_else(|| "`bundle` requires `--plan`".to_owned())?,
        output_dir: output_dir.ok_or_else(|| "`bundle` requires `--output-dir`".to_owned())?,
        check,
        asset_plan,
        project_index,
        usage_report,
        control,
        observations,
        retention,
        critical_evidence,
        reachability,
        physical_trace,
        pruning,
    }))
}

struct BundleSourceSnapshot {
    logical_path: String,
    bytes: Vec<u8>,
    report: ScanReport,
}

struct BundleFileInput {
    path: PathBuf,
    bytes: Vec<u8>,
    role: &'static str,
}

struct BundleOutputPayload {
    destination: PathBuf,
    bytes: Vec<u8>,
}

struct ResolvedBundleSources {
    paths: BTreeMap<String, (PathBuf, String)>,
    by_bundle: BTreeMap<String, BTreeSet<String>>,
}

struct CompiledBundleGroup {
    payloads: Vec<BundleOutputPayload>,
    style_references: usize,
    shared_styles: usize,
    token_references: BTreeSet<FlatTokenReference>,
}

#[allow(clippy::too_many_lines)]
fn run_bundle(arguments: &BundleArgs) -> Result<(), CliFailure> {
    let (plan, plan_path, plan_dir, plan_bytes) = load_bundle_plan(&arguments.plan)?;
    let output_dir = existing_artifact_directory(&arguments.output_dir, "bundle output")?;
    let (theme, theme_input) = load_bundle_theme(&plan.theme, &plan_dir)?;
    let theme_path = theme_input.as_ref().map(|input| input.path.as_path());
    let sources = resolve_bundle_sources(&plan, &plan_dir)?;
    let outputs = bundle_output_roles(&plan, &output_dir, arguments);
    validate_bundle_io_paths(
        &plan_path,
        BundleSidecarPaths {
            theme: theme_path,
            reachability: arguments.reachability.as_deref(),
            observation: arguments.observations.as_deref(),
            retention: arguments.retention.as_deref(),
            critical: arguments.critical_evidence.as_deref(),
        },
        &sources.paths,
        &outputs,
    )?;
    let reachability_path = arguments
        .reachability
        .as_deref()
        .map(|path| -> Result<PathBuf, CliFailure> {
            if arguments.control {
                let path = existing_regular_file(path, "reachability document")?;
                ensure_path_within_plan(&plan_dir, &path, "reachability document")?;
                Ok(path)
            } else {
                Ok(path.to_owned())
            }
        })
        .transpose()?;
    let reachability_bytes = reachability_path
        .as_deref()
        .map(read_reachability)
        .transpose()?;
    let reachability = reachability_bytes
        .as_deref()
        .map(parse_reachability_document)
        .transpose()
        .map_err(CliFailure::tool)?;
    let observation_path = arguments
        .observations
        .as_deref()
        .map(|path| -> Result<PathBuf, CliFailure> {
            let path = existing_regular_file(path, "usage observation")?;
            ensure_path_within_plan(&plan_dir, &path, "usage observation")?;
            Ok(path)
        })
        .transpose()?;
    let observation_bytes = observation_path
        .as_deref()
        .map(|path| read_bounded_utf8_document(path, "usage observation"))
        .transpose()?;
    let retention_path = arguments
        .retention
        .as_deref()
        .map(|path| -> Result<PathBuf, CliFailure> {
            let path = existing_regular_file(path, "usage retention")?;
            ensure_path_within_plan(&plan_dir, &path, "usage retention")?;
            Ok(path)
        })
        .transpose()?;
    let retention_bytes = retention_path
        .as_deref()
        .map(|path| read_bounded_utf8_document(path, "usage retention"))
        .transpose()?;
    let critical_path = arguments
        .critical_evidence
        .as_deref()
        .map(|path| -> Result<PathBuf, CliFailure> {
            let path = existing_regular_file(path, "critical style evidence")?;
            ensure_path_within_plan(&plan_dir, &path, "critical style evidence")?;
            Ok(path)
        })
        .transpose()?;
    let critical_bytes = critical_path
        .as_deref()
        .map(|path| read_bounded_utf8_document(path, "critical style evidence"))
        .transpose()?;
    let registry_graph = TokenGraph::from_registry(theme.registry());
    let registry_selections = BTreeMap::new();
    let (token_graph, token_selections) = if let LoadedTheme::Dtcg(theme) = &theme {
        (theme.graph(), theme.selections())
    } else {
        (&registry_graph, &registry_selections)
    };
    let snapshots = capture_bundle_sources(sources.paths)?;
    let mut group = compile_bundle_group(
        &plan,
        &output_dir,
        theme.registry(),
        &sources.by_bundle,
        &snapshots,
        ArtifactGraphOptions {
            reachability: reachability.as_ref(),
            physical_trace: arguments.physical_trace,
            pruning: arguments.pruning,
            ..ArtifactGraphOptions::default()
        },
        arguments,
        reachability_bytes.as_deref(),
        observation_bytes.as_deref(),
        retention_bytes.as_deref(),
        critical_bytes.as_deref(),
        token_graph,
        token_selections,
    )?;
    let control_passed = if arguments.control {
        append_bundle_control(
            &mut group,
            &plan,
            &plan_path,
            &plan_dir,
            &plan_bytes,
            theme_input.as_ref(),
            reachability_path.as_deref(),
            reachability_bytes.as_deref(),
            observation_path.as_deref(),
            observation_bytes.as_deref(),
            retention_path.as_deref(),
            retention_bytes.as_deref(),
            critical_path.as_deref(),
            critical_bytes.as_deref(),
            &snapshots,
            &output_dir,
            &theme,
            token_graph,
            token_selections,
        )?
    } else {
        true
    };

    let changed = if arguments.check {
        check_bundle_outputs(&group.payloads)?;
        0
    } else {
        publish_output_group(&group.payloads)?
    };
    if arguments.check {
        println!(
            "ok: {} bundle(s), {} style reference(s), {} shared style(s), {} output(s) match",
            plan.bundles.len(),
            group.style_references,
            group.shared_styles,
            group.payloads.len(),
        );
    } else {
        println!(
            "ok: {} bundle(s), {} style reference(s), {} shared style(s), {} output(s), {changed} changed",
            plan.bundles.len(),
            group.style_references,
            group.shared_styles,
            group.payloads.len(),
        );
    }
    if control_passed {
        Ok(())
    } else {
        Err(CliFailure::tool("bundle control audit failed"))
    }
}

fn load_bundle_plan(
    path: &Path,
) -> Result<(BundlePlanDocument, PathBuf, PathBuf, Vec<u8>), CliFailure> {
    let plan_path = existing_regular_file(path, "bundle plan")?;
    let plan_dir = plan_path
        .parent()
        .expect("an absolute file path has a parent")
        .to_path_buf();
    let plan_source = read_bounded_utf8_document(&plan_path, "bundle plan")?;
    let plan_text =
        std::str::from_utf8(&plan_source).expect("bounded bundle plan was validated as UTF-8");
    let plan: BundlePlanDocument = toml::from_str(plan_text).map_err(|error| {
        CliFailure::tool(format!(
            "cannot parse bundle plan `{}`: {error}",
            plan_path.display()
        ))
    })?;
    validate_bundle_plan(&plan)?;
    Ok((plan, plan_path, plan_dir, plan_source))
}

fn resolve_bundle_sources(
    plan: &BundlePlanDocument,
    plan_dir: &Path,
) -> Result<ResolvedBundleSources, CliFailure> {
    let mut source_paths = BTreeMap::<String, (PathBuf, String)>::new();
    let mut files_by_bundle = BTreeMap::<String, BTreeSet<String>>::new();
    for (name, bundle) in &plan.bundles {
        let declared = bundle
            .sources
            .iter()
            .enumerate()
            .map(|(index, source)| {
                resolve_plan_relative_path(
                    plan_dir,
                    source,
                    &format!("bundle `{name}` source {}", index + 1),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let expanded = expand_bundle_source_paths(&declared)?;
        if expanded.is_empty() {
            return Err(CliFailure::tool(format!(
                "bundle `{name}` does not resolve to any Rust source files"
            )));
        }
        let mut keys = BTreeSet::new();
        for source in expanded {
            ensure_path_within_plan(plan_dir, &source, &format!("bundle `{name}` source"))?;
            let key = path_key(&source)?;
            let logical_path = bundle_logical_source_path(plan_dir, &source)?;
            match source_paths.entry(key.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert((source, logical_path));
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    if logical_path < entry.get().1 {
                        entry.insert((source, logical_path));
                    }
                }
            }
            keys.insert(key);
        }
        files_by_bundle.insert(name.clone(), keys);
    }
    Ok(ResolvedBundleSources {
        paths: source_paths,
        by_bundle: files_by_bundle,
    })
}

fn bundle_output_roles(
    plan: &BundlePlanDocument,
    output_dir: &Path,
    arguments: &BundleArgs,
) -> Vec<(String, PathBuf)> {
    let mut outputs = plan
        .bundles
        .keys()
        .flat_map(|name| {
            let mut bundle_outputs = vec![
                (
                    format!("bundle `{name}` CSS"),
                    output_dir.join(format!("{name}.css")),
                ),
                (
                    format!("bundle `{name}` manifest"),
                    output_dir.join(format!("{name}.manifest.json")),
                ),
            ];
            if arguments.control {
                bundle_outputs.push((
                    format!("bundle `{name}` source map"),
                    output_dir.join(format!("{name}.css.map")),
                ));
            }
            bundle_outputs
        })
        .collect::<Vec<_>>();
    if arguments.asset_plan {
        outputs.push(("asset plan".into(), output_dir.join("pliego.assets.json")));
    }
    if arguments.project_index {
        outputs.push(("project index".into(), output_dir.join("pliego.index.json")));
    }
    if arguments.usage_report {
        outputs.push(("usage report".into(), output_dir.join("pliego.usage.json")));
        outputs.push(("tokens".into(), output_dir.join(TOKEN_USAGE_FILE)));
    }
    if arguments.critical_evidence.is_some() {
        outputs.push((
            "critical CSS manifest".into(),
            output_dir.join(CRITICAL_CSS_MANIFEST_FILE),
        ));
    }
    if arguments.control {
        for file in [
            pliego_css_control::TOKEN_GRAPH_FILE,
            control::FINDINGS_FILE,
            pliego_css_control::CONTROL_MANIFEST_FILE,
            pliego_css_control::BUILD_RECEIPT_FILE,
        ] {
            outputs.push(("control artifact".into(), output_dir.join(file)));
        }
    }
    outputs
}

#[derive(Clone, Copy)]
struct BundleSidecarPaths<'a> {
    theme: Option<&'a Path>,
    reachability: Option<&'a Path>,
    observation: Option<&'a Path>,
    retention: Option<&'a Path>,
    critical: Option<&'a Path>,
}

fn validate_bundle_io_paths(
    plan_path: &Path,
    sidecars: BundleSidecarPaths<'_>,
    source_paths: &BTreeMap<String, (PathBuf, String)>,
    outputs: &[(String, PathBuf)],
) -> Result<(), String> {
    for (_, output) in outputs {
        validate_bundle_output_destination(output)?;
    }
    let mut inputs = vec![("bundle plan".to_owned(), plan_path.to_path_buf())];
    if let Some(config_path) = sidecars.theme {
        inputs.push((
            "bundle theme configuration".to_owned(),
            config_path.to_path_buf(),
        ));
    }
    if let Some(reachability_path) = sidecars.reachability {
        inputs.push((
            "reachability document".to_owned(),
            reachability_path.to_path_buf(),
        ));
    }
    if let Some(observation_path) = sidecars.observation {
        inputs.push((
            "usage observation".to_owned(),
            observation_path.to_path_buf(),
        ));
    }
    if let Some(retention_path) = sidecars.retention {
        inputs.push(("usage retention".to_owned(), retention_path.to_path_buf()));
    }
    if let Some(critical_path) = sidecars.critical {
        inputs.push((
            "critical style evidence".to_owned(),
            critical_path.to_path_buf(),
        ));
    }
    inputs.extend(
        source_paths
            .values()
            .map(|(path, _)| ("bundle source".to_owned(), path.clone())),
    );
    let input_refs = inputs
        .iter()
        .map(|(role, path)| (role.as_str(), path.as_path()))
        .collect::<Vec<_>>();
    let output_refs = outputs
        .iter()
        .map(|(role, path)| (role.as_str(), path.as_path()))
        .collect::<Vec<_>>();
    validate_path_roles(&input_refs, &output_refs)
}

fn capture_bundle_sources(
    source_paths: BTreeMap<String, (PathBuf, String)>,
) -> Result<BTreeMap<String, BundleSourceSnapshot>, CliFailure> {
    let mut snapshots = BTreeMap::new();
    for (key, (path, logical_path)) in source_paths {
        let source = fs::read(&path).map_err(|error| {
            CliFailure::tool(format!("cannot read source `{}`: {error}", path.display()))
        })?;
        let text = std::str::from_utf8(&source).map_err(|error| {
            CliFailure::tool(format!(
                "Rust source `{}` is not valid UTF-8: {error}",
                path.display()
            ))
        })?;
        let report = scan_source_named(logical_path.clone(), text)
            .map_err(|error| CliFailure::tool(error.to_string()))?;
        snapshots.insert(
            key,
            BundleSourceSnapshot {
                logical_path,
                bytes: source,
                report,
            },
        );
    }
    Ok(snapshots)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn compile_bundle_group(
    plan: &BundlePlanDocument,
    output_dir: &Path,
    theme: &ThemeRegistry,
    files_by_bundle: &BTreeMap<String, BTreeSet<String>>,
    snapshots: &BTreeMap<String, BundleSourceSnapshot>,
    graph: ArtifactGraphOptions<'_>,
    arguments: &BundleArgs,
    reachability_source: Option<&[u8]>,
    observation_source: Option<&[u8]>,
    retention_source: Option<&[u8]>,
    critical_source: Option<&[u8]>,
    token_graph: &TokenGraph,
    token_selections: &BTreeMap<String, String>,
) -> Result<CompiledBundleGroup, CliFailure> {
    let mut payloads = Vec::with_capacity(
        plan.bundles.len() * 2
            + usize::from(arguments.asset_plan)
            + usize::from(arguments.project_index)
            + 2 * usize::from(arguments.usage_report)
            + plan.bundles.len() * usize::from(arguments.control),
    );
    let mut source_maps = Vec::with_capacity(plan.bundles.len() * usize::from(arguments.control));
    let mut style_occurrences = BTreeMap::<String, usize>::new();
    let mut style_references = 0;
    let mut token_references = BTreeSet::new();
    let mut usage_styles = Vec::new();
    let mut token_consumers = Vec::new();
    let mut resolved_by_bundle = BTreeMap::new();
    let mut emitted_themes = Vec::with_capacity(plan.bundles.len());

    for name in plan.bundles.keys() {
        let mut candidates = Vec::new();
        for key in files_by_bundle
            .get(name)
            .expect("validated bundle has a source set")
        {
            let snapshot = snapshots
                .get(key)
                .expect("every resolved bundle source has a snapshot");
            candidates.extend(candidates_from_scan_report(theme, &snapshot.report)?);
        }
        if candidates.is_empty() {
            let sources = files_by_bundle[name]
                .iter()
                .map(|key| snapshots[key].logical_path.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(CliFailure::tool(format!(
                "bundle `{name}` found no PliegoCSS styles in: {sources}"
            )));
        }
        let resolved = resolve_candidates(theme, &candidates, 1)?;
        if arguments.usage_report || graph.pruning.is_enabled() {
            usage_styles.extend(bundle_usage_inputs(name, &resolved)?);
        }
        resolved_by_bundle.insert(name.clone(), resolved);
    }

    let rule_selection = if retention_source.is_some() {
        AssetRuleSelection::ReachableOrRetainedStyleIds
    } else if graph.pruning.is_enabled() {
        AssetRuleSelection::ReachableStyleIds
    } else {
        AssetRuleSelection::AllCompiled
    };
    let prepared_usage = (arguments.usage_report || graph.pruning.is_enabled())
        .then(|| {
            prepare_usage_analysis(
                &usage_styles,
                reachability_source,
                observation_source,
                retention_source,
                rule_selection,
            )
        })
        .transpose()?;
    let global_theme_references = graph.pruning.is_enabled().then(|| {
        collect_selected_bundle_token_references(
            prepared_usage.as_ref().expect("selection").selection(),
            resolved_by_bundle.iter().map(|(bundle, styles)| {
                (bundle.as_str(), styles.iter().map(|item| &item.semantic))
            }),
        )
    });

    for (name, bundle) in &plan.bundles {
        let resolved = resolved_by_bundle
            .get(name)
            .expect("every validated bundle was resolved");
        let selection = graph
            .pruning
            .is_enabled()
            .then(|| {
                prepared_usage
                    .as_ref()
                    .map(PreparedUsageAnalysis::selection)
            })
            .flatten();
        if let Some(prepared) = &prepared_usage {
            token_consumers.extend(collect_selected_token_usage_consumers(
                name,
                prepared.selection(),
                resolved.iter().map(|candidate| &candidate.semantic),
            )?);
        }
        let bundle_graph = ArtifactGraphOptions {
            selection,
            bundle_id: selection.map(|_| name.as_str()),
            theme_references: global_theme_references.as_ref(),
            ..graph
        };
        let artifact = compile_resolved_candidates_with_manifest(
            theme,
            resolved,
            bundle.emit_theme,
            plan.targets,
            plan.format,
            bundle_graph,
            &mut CssCaches::default(),
        )
        .map_err(CliFailure::compilation)?;
        if arguments.control {
            let source_inputs = files_by_bundle[name]
                .iter()
                .map(|key| {
                    let snapshot = &snapshots[key];
                    AuditSourceInput {
                        logical_path: &snapshot.logical_path,
                        role: "source",
                        bytes: &snapshot.bytes,
                    }
                })
                .collect::<Vec<_>>();
            source_maps.push(BundleOutputPayload {
                destination: output_dir.join(format!("{name}.css.map")),
                bytes: build_artifact_source_map(&artifact, &source_inputs)?,
            });
        }
        style_references += artifact.styles.len();
        emitted_themes.push(artifact.emits_theme);
        token_references.extend(artifact.token_references.iter().copied());
        for style in &artifact.styles {
            *style_occurrences.entry(style.style_id.clone()).or_default() += 1;
        }
        payloads.push(BundleOutputPayload {
            destination: output_dir.join(format!("{name}.css")),
            bytes: artifact.css.into_bytes(),
        });
        payloads.push(BundleOutputPayload {
            destination: output_dir.join(format!("{name}.manifest.json")),
            bytes: artifact.manifest.into_bytes(),
        });
    }
    if arguments.asset_plan {
        let inputs = plan
            .bundles
            .iter()
            .zip(payloads.chunks_exact(2))
            .zip(&emitted_themes)
            .map(|(((name, _), outputs), emits_theme)| {
                AssetPlanBundle::new(name, *emits_theme, &outputs[0].bytes, &outputs[1].bytes)
            })
            .collect::<Vec<_>>();
        let asset_plan = build_asset_plan(&inputs, rule_selection)?;
        let project_index = arguments
            .project_index
            .then(|| {
                let documents = snapshots
                    .values()
                    .map(|snapshot| {
                        ProjectIndexDocument::new(&snapshot.logical_path, &snapshot.bytes)
                    })
                    .collect::<Vec<_>>();
                build_project_index(&inputs, &documents, rule_selection)
            })
            .transpose()?;
        let critical_outputs = critical_source
            .map(|source| {
                build_critical_bundle_outputs(
                    source,
                    &asset_plan,
                    prepared_usage
                        .as_ref()
                        .expect("critical evidence requires usage preparation"),
                    reachability_source.expect("critical evidence requires reachability"),
                    &resolved_by_bundle,
                    theme,
                    plan,
                    output_dir,
                )
            })
            .transpose()?;
        payloads.push(BundleOutputPayload {
            destination: output_dir.join("pliego.assets.json"),
            bytes: asset_plan,
        });
        if let Some(project_index) = project_index {
            payloads.push(BundleOutputPayload {
                destination: output_dir.join("pliego.index.json"),
                bytes: project_index,
            });
        }
        if let Some(critical_outputs) = critical_outputs {
            payloads.extend(critical_outputs);
        }
    }
    if arguments.usage_report {
        payloads.push(BundleOutputPayload {
            destination: output_dir.join("pliego.usage.json"),
            bytes: prepared_usage
                .as_ref()
                .expect("usage report preparation follows the output option")
                .build_analysis()?,
        });
        payloads.push(BundleOutputPayload {
            destination: output_dir.join(TOKEN_USAGE_FILE),
            bytes: build_token_usage_report(
                token_graph,
                token_selections,
                theme,
                token_consumers,
                emitted_themes.iter().any(|emitted| *emitted),
            )?
            .to_canonical_json()?,
        });
    }
    payloads.extend(source_maps);
    let shared_styles = style_occurrences
        .values()
        .filter(|occurrences| **occurrences > 1)
        .count();
    Ok(CompiledBundleGroup {
        payloads,
        style_references,
        shared_styles,
        token_references,
    })
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_critical_bundle_outputs(
    evidence: &[u8],
    asset_plan: &[u8],
    usage: &PreparedUsageAnalysis,
    reachability: &[u8],
    candidates: &BTreeMap<String, Vec<ResolvedCandidate>>,
    theme: &ThemeRegistry,
    plan: &BundlePlanDocument,
    output_dir: &Path,
) -> Result<Vec<BundleOutputPayload>, CliFailure> {
    let assets =
        parse_asset_plan(asset_plan).map_err(|error| CliFailure::tool(error.to_string()))?;
    let routes = assets
        .routes()
        .iter()
        .map(|route| {
            CriticalRouteInput::new(route.id(), route.path(), route.bundle_ids().iter().cloned())
        })
        .collect::<Vec<_>>();
    let reachability_sha256 = sha256_hex(reachability);
    let selection = verify_critical_evidence(
        evidence,
        usage.universe_sha256(),
        &reachability_sha256,
        usage.selection(),
        &routes,
    )?;
    let mut payloads = Vec::new();
    let mut manifest_routes = Vec::new();
    for route in assets.routes() {
        let Some(styles) = selection.styles(route.id()) else {
            continue;
        };
        if styles.is_empty() {
            return Err(CliFailure::tool(format!(
                "critical route `{}` has no positive StyleId observations",
                route.id()
            )));
        }
        let mut selected = Vec::new();
        for (bundle_id, style_id) in styles {
            selected.extend(
                candidates[bundle_id]
                    .iter()
                    .filter(|candidate| {
                        format!("{:032x}", candidate.semantic.id.get()) == *style_id
                    })
                    .cloned(),
            );
        }
        if selected.is_empty() {
            return Err(CliFailure::tool(
                "critical selection resolved no compiler styles",
            ));
        }
        let references = referenced_tokens(selected.iter().map(|item| &item.semantic));
        let artifact = compile_resolved_candidates_with_manifest(
            theme,
            &selected,
            true,
            plan.targets,
            plan.format,
            ArtifactGraphOptions {
                theme_references: Some(&references),
                ..ArtifactGraphOptions::default()
            },
            &mut CssCaches::default(),
        )
        .map_err(CliFailure::compilation)?;
        let digest = sha256_hex(route.id().as_bytes());
        let file = format!("critical-{}.css", &digest[..16]);
        let css = artifact.css.into_bytes();
        manifest_routes.push(CriticalCssRouteInput::new(
            route.id(),
            route.path(),
            &file,
            css.clone(),
            styles.iter().cloned().collect(),
        ));
        payloads.push(BundleOutputPayload {
            destination: output_dir.join(file),
            bytes: css,
        });
    }
    let format = match plan.format {
        CssFormat::Minified => "minified",
        CssFormat::Pretty => "pretty",
    };
    payloads.push(BundleOutputPayload {
        destination: output_dir.join(CRITICAL_CSS_MANIFEST_FILE),
        bytes: build_critical_css_manifest(
            selection.evidence_sha256(),
            usage.universe_sha256(),
            &reachability_sha256,
            &theme.id().to_string(),
            plan.targets.as_str(),
            format,
            manifest_routes,
        )?,
    });
    Ok(payloads)
}

fn bundle_usage_inputs(
    bundle_id: &str,
    candidates: &[ResolvedCandidate],
) -> Result<Vec<UsageStyleInput>, CliFailure> {
    collect_usage_style_inputs(
        bundle_id,
        candidates.iter().map(|candidate| {
            let origin = &candidate.provenance;
            UsageCandidateInput::new(
                format!("{:032x}", candidate.semantic.id.get()),
                candidate.semantic.id.to_class_name(),
                &origin.source,
                origin.file.as_deref(),
                origin.byte_start,
                origin.byte_end,
                &origin.macro_kind,
                &origin.reason,
            )
        }),
    )
    .map_err(CliFailure::tool)
}

struct BundleControlOutput<'a> {
    file: String,
    role: &'static str,
    media_type: &'static str,
    bytes: &'a [u8],
    source_map: Option<String>,
    relationships: Vec<String>,
}

fn project_bundle_control_outputs<'a>(
    outputs: &'a [BundleControlOutput<'a>],
) -> Result<Vec<ControlOutputInput<'a>>, CliFailure> {
    let source_maps = outputs
        .iter()
        .filter(|output| output.role == "css-source-map")
        .map(|output| (output.file.as_str(), output.bytes))
        .collect::<BTreeMap<_, _>>();
    outputs
        .iter()
        .map(|output| {
            let source_map = output
                .source_map
                .as_deref()
                .map(|file| {
                    source_maps
                        .get(file)
                        .map(|bytes| ControlSourceMapInput { file, bytes })
                        .ok_or_else(|| {
                            CliFailure::tool(format!(
                                "bundle CSS `{}` references missing source map `{file}`",
                                output.file
                            ))
                        })
                })
                .transpose()?;
            Ok(ControlOutputInput {
                file: &output.file,
                role: output.role,
                media_type: output.media_type,
                bytes: output.bytes,
                source_map,
                relationships: &output.relationships,
            })
        })
        .collect()
}

fn audit_bundle_outputs(
    group: &CompiledBundleGroup,
    plan: &BundlePlanDocument,
) -> Result<(FindingDocument, CssBudgetMetrics, bool, bool), CliFailure> {
    let mut findings = Vec::new();
    let mut metrics = CssBudgetMetrics::default();
    let mut passed = true;
    let mut measured = true;
    for (name, outputs) in plan
        .bundles
        .keys()
        .zip(group.payloads[..plan.bundles.len() * 2].chunks_exact(2))
    {
        let css = std::str::from_utf8(&outputs[0].bytes).map_err(|error| {
            CliFailure::tool(format!(
                "bundle `{name}` emitted invalid UTF-8 CSS: {error}"
            ))
        })?;
        let outcome = audit_standard_css(&format!("{name}.css"), css, plan.targets)
            .map_err(CliFailure::tool)?;
        passed &= outcome.passed();
        findings.extend(outcome.document().findings().iter().cloned());
        if let Some(inventory) = outcome.inventory() {
            metrics
                .try_merge(inventory.file())
                .map_err(|error| CliFailure::tool(error.to_string()))?;
        } else {
            measured = false;
        }
    }
    let document = FindingDocument::new(
        FindingTool::new("pliegocss", env!("CARGO_PKG_VERSION"))
            .map_err(|error| CliFailure::tool(error.to_string()))?,
        "bundle",
        findings,
    )
    .map_err(|error| CliFailure::tool(error.to_string()))?;
    Ok((document, metrics, measured, passed))
}

fn bundle_control_outputs<'a>(
    group: &'a CompiledBundleGroup,
    output_dir: &Path,
) -> Result<Vec<BundleControlOutput<'a>>, CliFailure> {
    let mut outputs = group
        .payloads
        .iter()
        .map(|output| {
            let file = output
                .destination
                .strip_prefix(output_dir)
                .map_err(|_| CliFailure::tool("bundle output escaped its output directory"))?
                .to_str()
                .ok_or_else(|| CliFailure::tool("bundle output path is not valid UTF-8"))?
                .replace('\\', "/");
            let (role, media_type) = if file.strip_suffix(".css.map").is_some() {
                ("css-source-map", "application/json")
            } else if file.strip_suffix(".css").is_some() {
                ("generated-css", "text/css")
            } else if file.strip_suffix(".manifest.json").is_some() {
                ("style-manifest", "application/json")
            } else if file == "pliego.assets.json" {
                ("asset-plan", "application/json")
            } else if file == "pliego.usage.json" {
                ("usage-analysis", "application/json")
            } else if file == TOKEN_USAGE_FILE {
                ("token-usage", "application/json")
            } else {
                ("project-index", "application/json")
            };
            Ok(BundleControlOutput {
                file,
                role,
                media_type,
                bytes: &output.bytes,
                source_map: None,
                relationships: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>, CliFailure>>()?;
    let artifact_files = outputs
        .iter()
        .map(|output| output.file.clone())
        .collect::<BTreeSet<_>>();
    for output in &mut outputs {
        output.relationships = if let Some(css) = output.file.strip_suffix(".map") {
            vec![css.to_owned()]
        } else if let Some(stem) = output.file.strip_suffix(".css") {
            let source_map = format!("{stem}.css.map");
            output.source_map = artifact_files
                .contains(&source_map)
                .then_some(source_map.clone());
            vec![format!("{stem}.manifest.json"), source_map]
        } else if let Some(stem) = output.file.strip_suffix(".manifest.json") {
            vec![format!("{stem}.css")]
        } else if output.file == "pliego.assets.json" {
            artifact_files
                .iter()
                .filter(|file| {
                    file.strip_suffix(".css").is_some()
                        || file.strip_suffix(".manifest.json").is_some()
                })
                .cloned()
                .collect()
        } else if matches!(output.file.as_str(), "pliego.usage.json" | TOKEN_USAGE_FILE) {
            artifact_files
                .iter()
                .filter(|file| {
                    file.strip_suffix(".css").is_some()
                        || file.strip_suffix(".manifest.json").is_some()
                        || matches!(file.as_str(), "pliego.assets.json" | "pliego.index.json")
                })
                .cloned()
                .collect()
        } else {
            vec!["pliego.assets.json".to_owned()]
        };
    }
    Ok(outputs)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn append_bundle_control(
    group: &mut CompiledBundleGroup,
    plan: &BundlePlanDocument,
    plan_path: &Path,
    plan_dir: &Path,
    plan_bytes: &[u8],
    theme_input: Option<&BundleFileInput>,
    reachability_path: Option<&Path>,
    reachability_bytes: Option<&[u8]>,
    observation_path: Option<&Path>,
    observation_bytes: Option<&[u8]>,
    retention_path: Option<&Path>,
    retention_bytes: Option<&[u8]>,
    critical_path: Option<&Path>,
    critical_bytes: Option<&[u8]>,
    snapshots: &BTreeMap<String, BundleSourceSnapshot>,
    output_dir: &Path,
    theme: &LoadedTheme,
    token_graph: &TokenGraph,
    token_selections: &BTreeMap<String, String>,
) -> Result<bool, CliFailure> {
    let (document, metrics, measured, passed) = audit_bundle_outputs(group, plan)?;

    let plan_logical = bundle_logical_source_path(plan_dir, plan_path)?;
    let theme_logical = theme_input
        .map(|input| bundle_logical_source_path(plan_dir, &input.path))
        .transpose()?;
    let reachability_logical = reachability_path
        .map(|path| bundle_logical_source_path(plan_dir, path))
        .transpose()?;
    let observation_logical = observation_path
        .map(|path| bundle_logical_source_path(plan_dir, path))
        .transpose()?;
    let retention_logical = retention_path
        .map(|path| bundle_logical_source_path(plan_dir, path))
        .transpose()?;
    let critical_logical = critical_path
        .map(|path| bundle_logical_source_path(plan_dir, path))
        .transpose()?;
    let source_inputs = snapshots
        .values()
        .map(|snapshot| AuditSourceInput {
            logical_path: &snapshot.logical_path,
            role: "source",
            bytes: &snapshot.bytes,
        })
        .collect::<Vec<_>>();
    let mut config_inputs = Vec::with_capacity(6);
    config_inputs.push(AuditSourceInput {
        logical_path: &plan_logical,
        role: "bundle-plan",
        bytes: plan_bytes,
    });
    if let (Some(input), Some(logical)) = (theme_input, theme_logical.as_deref()) {
        config_inputs.push(AuditSourceInput {
            logical_path: logical,
            role: input.role,
            bytes: &input.bytes,
        });
    }
    if let (Some(bytes), Some(logical)) = (reachability_bytes, reachability_logical.as_deref()) {
        config_inputs.push(AuditSourceInput {
            logical_path: logical,
            role: "reachability",
            bytes,
        });
    }
    if let (Some(bytes), Some(logical)) = (observation_bytes, observation_logical.as_deref()) {
        config_inputs.push(AuditSourceInput {
            logical_path: logical,
            role: "usage-observation",
            bytes,
        });
    }
    if let (Some(bytes), Some(logical)) = (retention_bytes, retention_logical.as_deref()) {
        config_inputs.push(AuditSourceInput {
            logical_path: logical,
            role: "usage-retention",
            bytes,
        });
    }
    if let (Some(bytes), Some(logical)) = (critical_bytes, critical_logical.as_deref()) {
        config_inputs.push(AuditSourceInput {
            logical_path: logical,
            role: "critical-style-evidence",
            bytes,
        });
    }
    let registry = theme.registry();
    let token_measurements = build_token_graph_measurements(
        token_graph,
        token_selections,
        registry,
        &group.token_references,
    )
    .map_err(CliFailure::tool)?;
    let control_group = {
        let outputs = bundle_control_outputs(group, output_dir)?;
        let generated_outputs = project_bundle_control_outputs(&outputs)?;
        let relationships = outputs
            .iter()
            .map(|output| output.file.clone())
            .collect::<Vec<_>>();
        build_audit_control_group(AuditControlInput {
            source_inputs: &source_inputs,
            config_inputs: &config_inputs,
            generated_outputs: &generated_outputs,
            document: &document,
            passed,
            rule_metrics: measured.then_some(&metrics),
            rules_unavailable_reason: "one or more generated bundle stylesheets failed syntax ingestion",
            token_observation: TokenObservationInput::Measured(&token_measurements),
            token_graph: Some(token_graph),
            output_relationships: &relationships,
            asset_plan_verified: true,
            targets: plan.targets,
            budget_policy: None,
            budget_policy_path: None,
            budget_policy_bytes: None,
            budget_subjects: &[],
        })
        .map_err(CliFailure::tool)?
    };
    group.payloads.extend(
        control_group
            .artifacts()
            .map(|artifact| BundleOutputPayload {
                destination: output_dir.join(artifact.file),
                bytes: artifact.bytes.to_vec(),
            }),
    );
    Ok(passed)
}

fn validate_bundle_plan(plan: &BundlePlanDocument) -> Result<(), String> {
    if !matches!(plan.schema, 1 | 2) {
        return Err(format!(
            "unsupported bundle plan schema {}; expected schema 1 or 2",
            plan.schema
        ));
    }
    if plan.bundles.is_empty() {
        return Err("bundle plan must declare at least one bundle".into());
    }
    for (name, bundle) in &plan.bundles {
        validate_bundle_name(name)?;
        if bundle.sources.is_empty() {
            return Err(format!("bundle `{name}` must declare at least one source"));
        }
    }
    if plan.schema == 1 {
        if plan.theme.kind == BundleThemeKind::DtcgResolver {
            return Err("bundle plan schema 1 does not support theme kind `dtcg-resolver`".into());
        }
        if plan.theme.inputs.is_some() {
            return Err("bundle plan schema 1 does not accept `theme.inputs`".into());
        }
    }
    match (&plan.theme.kind, &plan.theme.path, &plan.theme.inputs) {
        (BundleThemeKind::Seed, None, None)
        | (BundleThemeKind::Config, Some(_), None)
        | (BundleThemeKind::DtcgResolver, Some(_), _) => Ok(()),
        (BundleThemeKind::Seed, Some(_), _) => {
            Err("bundle theme kind `seed` does not accept `path`".into())
        }
        (BundleThemeKind::Seed, None, Some(_)) => {
            Err("bundle theme kind `seed` does not accept `inputs`".into())
        }
        (BundleThemeKind::Config, None, _) => {
            Err("bundle theme kind `config` requires `path`".into())
        }
        (BundleThemeKind::Config, Some(_), Some(_)) => {
            Err("bundle theme kind `config` does not accept `inputs`".into())
        }
        (BundleThemeKind::DtcgResolver, None, _) => {
            Err("bundle theme kind `dtcg-resolver` requires `path`".into())
        }
    }
}

fn validate_bundle_name(name: &str) -> Result<(), String> {
    validate_asset_bundle_id(name)
}

fn existing_regular_file(path: &Path, role: &str) -> Result<PathBuf, String> {
    let absolute = absolute_path(path)?;
    let metadata = fs::symlink_metadata(&absolute)
        .map_err(|error| format!("cannot inspect {role} `{}`: {error}", path.display()))?;
    if is_link_like(&metadata) {
        return Err(format!(
            "{role} `{}` is a symbolic link; bundle inputs must be regular files",
            path.display()
        ));
    }
    if !metadata.is_file() {
        return Err(format!("{role} `{}` is not a regular file", path.display()));
    }
    fs::canonicalize(&absolute)
        .map_err(|error| format!("cannot canonicalize {role} `{}`: {error}", path.display()))
}

fn existing_artifact_directory(path: &Path, role: &str) -> Result<PathBuf, String> {
    let absolute = absolute_path(path)?;
    let metadata = fs::symlink_metadata(&absolute).map_err(|error| {
        format!(
            "cannot inspect {role} directory `{}`: {error}; create it before running the command",
            path.display()
        )
    })?;
    if is_link_like(&metadata) {
        return Err(format!(
            "{role} directory `{}` is a symbolic link",
            path.display()
        ));
    }
    if !metadata.is_dir() {
        return Err(format!(
            "{role} directory `{}` is not a directory",
            path.display()
        ));
    }
    fs::canonicalize(&absolute).map_err(|error| {
        format!(
            "cannot canonicalize {role} directory `{}`: {error}",
            path.display()
        )
    })
}

fn validate_bundle_output_destination(path: &Path) -> Result<(), String> {
    validate_output_destination(path, "bundle output")
}

fn validate_output_destination(path: &Path, role: &str) -> Result<(), String> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
    {
        return Err(format!("{role} may not use a Rust source extension"));
    }
    match fs::symlink_metadata(path) {
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && path
                    .parent()
                    .is_none_or(|parent| parent.as_os_str().is_empty() || parent.is_dir()) =>
        {
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("cannot prepare {role} `{}`", path.display()))
        }
        Err(error) => Err(format!(
            "cannot inspect {role} `{}`: {error}",
            path.display()
        )),
        Ok(metadata) if is_link_like(&metadata) => Err(format!(
            "{role} `{}` is a symbolic link or reparse point",
            path.display()
        )),
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(format!("{role} `{}` is not a regular file", path.display())),
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(normalize_path(path))
    } else {
        env::current_dir()
            .map(|directory| normalize_path(&directory.join(path)))
            .map_err(|error| format!("cannot resolve `{}`: {error}", path.display()))
    }
}

fn resolve_plan_relative_path(plan_dir: &Path, path: &Path, role: &str) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || has_dot_path_segment(path)
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "{role} path `{}` must be a non-empty relative path without `.` or `..` components",
            path.display()
        ));
    }
    reject_symlink_components(plan_dir, path, role)?;
    Ok(plan_dir.join(path))
}

fn has_dot_path_segment(path: &Path) -> bool {
    let path = path.to_string_lossy();
    #[cfg(windows)]
    let mut segments = path.split(['/', '\\']);
    #[cfg(not(windows))]
    let mut segments = path.split('/');
    segments.any(|segment| matches!(segment, "." | ".."))
}

fn reject_symlink_components(plan_dir: &Path, relative: &Path, role: &str) -> Result<(), String> {
    let mut current = plan_dir.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if is_link_like(&metadata) => {
                return Err(format!(
                    "{role} path `{}` traverses symbolic link `{}`",
                    relative.display(),
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(format!(
                    "cannot inspect {role} path component `{}`: {error}",
                    current.display()
                ));
            }
        }
    }
    Ok(())
}

fn is_link_like(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || {
        #[cfg(windows)]
        {
            metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

fn ensure_path_within_plan(plan_dir: &Path, path: &Path, role: &str) -> Result<(), String> {
    let plan_key = PathBuf::from(path_key(plan_dir)?);
    let resolved_key = PathBuf::from(path_key(path)?);
    if resolved_key.starts_with(&plan_key) && resolved_key != plan_key {
        Ok(())
    } else {
        Err(format!(
            "{role} path `{}` resolves outside bundle plan directory `{}`",
            path.display(),
            plan_dir.display()
        ))
    }
}

fn bundle_logical_source_path(plan_dir: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(plan_dir).map_err(|_| {
        format!(
            "bundle source `{}` escapes plan directory `{}`",
            path.display(),
            plan_dir.display()
        )
    })?;
    relative
        .components()
        .map(|component| match component {
            Component::Normal(value) => value.to_str().map(str::to_owned).ok_or_else(|| {
                format!("bundle source path `{}` is not valid UTF-8", path.display())
            }),
            _ => Err(format!(
                "bundle source `{}` does not have a stable plan-relative path",
                path.display()
            )),
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|components| components.join("/"))
}

fn load_bundle_theme(
    theme: &BundleThemeDocument,
    plan_dir: &Path,
) -> Result<(LoadedTheme, Option<BundleFileInput>), String> {
    match theme.kind {
        BundleThemeKind::Seed => Ok((LoadedTheme::Registry(ThemeRegistry::seed()), None)),
        BundleThemeKind::Config => {
            let configured = theme
                .path
                .as_deref()
                .expect("validated config theme has a path");
            let resolved = resolve_plan_relative_path(plan_dir, configured, "bundle theme")?;
            let resolved = existing_regular_file(&resolved, "bundle theme configuration")?;
            ensure_path_within_plan(plan_dir, &resolved, "bundle theme configuration")?;
            let bytes = fs::read(&resolved).map_err(|error| {
                format!(
                    "cannot read bundle theme configuration `{}`: {error}",
                    resolved.display()
                )
            })?;
            let text = std::str::from_utf8(&bytes).map_err(|error| {
                format!(
                    "bundle theme configuration `{}` is not valid UTF-8: {error}",
                    resolved.display()
                )
            })?;
            let registry = pliego_css_config::parse_str(text).map_err(|error| error.to_string())?;
            Ok((
                LoadedTheme::Registry(registry),
                Some(BundleFileInput {
                    path: resolved,
                    bytes,
                    role: "theme-config",
                }),
            ))
        }
        BundleThemeKind::DtcgResolver => {
            let configured = theme
                .path
                .as_deref()
                .expect("validated DTCG bundle theme has a path");
            let resolved =
                resolve_plan_relative_path(plan_dir, configured, "bundle token resolver")?;
            let resolved = existing_regular_file(&resolved, "bundle token resolver")?;
            ensure_path_within_plan(plan_dir, &resolved, "bundle token resolver")?;
            let bytes = read_bounded_utf8_document(&resolved, "bundle token resolver")?;
            let text = std::str::from_utf8(&bytes)
                .expect("bounded bundle token resolver was validated as UTF-8");
            let selected = parse_dtcg_resolver_str(text)
                .and_then(|resolver| {
                    resolver.resolve_entries(
                        theme
                            .inputs
                            .iter()
                            .flat_map(|inputs| inputs.iter())
                            .map(|(modifier, context)| (modifier.clone(), context.clone())),
                    )
                })
                .map_err(|error| error.to_string())?;
            Ok((
                LoadedTheme::Dtcg(selected),
                Some(BundleFileInput {
                    path: resolved,
                    bytes,
                    role: "token-resolver",
                }),
            ))
        }
    }
}

fn check_bundle_outputs(outputs: &[BundleOutputPayload]) -> Result<(), String> {
    let mut drift = Vec::new();
    for output in outputs {
        match fs::read(&output.destination) {
            Ok(actual) if actual == output.bytes => {}
            Ok(_) => drift.push(format!("`{}` differs", output.destination.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                drift.push(format!("`{}` is missing", output.destination.display()));
            }
            Err(error) => {
                return Err(format!(
                    "cannot check bundle output `{}`: {error}",
                    output.destination.display()
                ));
            }
        }
    }
    if drift.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "bundle output drift detected in {} artifact(s): {}",
            drift.len(),
            drift.join("; ")
        ))
    }
}

fn parse_catalog_arguments(arguments: &[String]) -> Result<Command, String> {
    let mut parsed = CatalogArgs::default();
    let mut format_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--config" => {
                set_path_once(&mut parsed.config, arguments, index, "--config")?;
                index += 2;
            }
            "--seed" => {
                set_flag_once(&mut parsed.seed, "--seed")?;
                index += 1;
            }
            "--format" => {
                set_flag_once(&mut format_seen, "--format")?;
                parsed.format = parse_catalog_format(option_value(arguments, index, "--format")?)?;
                index += 2;
            }
            "--output" => {
                set_path_once(&mut parsed.output, arguments, index, "--output")?;
                index += 2;
            }
            "--check" => {
                set_path_once(&mut parsed.check, arguments, index, "--check")?;
                index += 2;
            }
            unknown => return Err(format!("unknown catalog option `{unknown}`")),
        }
    }
    if parsed.seed && parsed.config.is_some() {
        return Err("`--seed` conflicts with `--config`".into());
    }
    if parsed.output.is_some() && parsed.check.is_some() {
        return Err("`--output` conflicts with `--check`".into());
    }
    Ok(Command::Catalog(parsed))
}

fn parse_compatibility_arguments(arguments: &[String]) -> Result<Command, String> {
    let mut targets = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--targets" => {
                if targets.is_some() {
                    return Err("`--targets` may only be provided once".into());
                }
                targets = Some(parse_targets(option_value(arguments, index, "--targets")?)?);
                index += 2;
            }
            unknown => return Err(format!("unknown compatibility option `{unknown}`")),
        }
    }
    Ok(Command::Compatibility(CompatibilityArgs {
        targets: targets.ok_or("`compatibility` requires `--targets`")?,
    }))
}

fn run_generic_css_usage(
    findings_path: &Path,
    observed_path: &Path,
    scope: &str,
    output: &Path,
    control_dir: Option<&Path>,
) -> Result<(), CliFailure> {
    let findings_bytes = read_command_artifact(findings_path, "generic-css-usage", "findings")?;
    let document = parse_finding_document(&findings_bytes)
        .map_err(|error| CliFailure::invalid(error.to_string()))?;
    let inventory_finding = document
        .findings()
        .iter()
        .find(|finding| finding.code() == "PCSS-AUDIT-000")
        .ok_or_else(|| CliFailure::invalid("generic-css-usage findings lack PCSS-AUDIT-000"))?;
    let mut inventory = Vec::new();
    for evidence in inventory_finding.evidence() {
        if evidence.kind() != "identity" || !evidence.name().starts_with("generic-css-declaration-")
        {
            continue;
        }
        let (identity, occurrences) = evidence
            .value()
            .split_once(";occurrences=")
            .ok_or_else(|| CliFailure::invalid("malformed generic CSS declaration evidence"))?;
        inventory.push((
            identity.to_owned(),
            occurrences
                .parse::<usize>()
                .map_err(|_| CliFailure::invalid("malformed generic CSS occurrence count"))?,
        ));
    }
    if inventory.is_empty() {
        return Err(CliFailure::invalid(
            "generic-css-usage findings contain no declaration identities",
        ));
    }
    let observed_bytes = read_command_artifact(observed_path, "generic-css-usage", "observed")?;
    let observed = serde_json::from_slice::<Vec<String>>(&observed_bytes)
        .map_err(|error| CliFailure::invalid(format!("invalid observed identity list: {error}")))?;
    let bytes =
        build_generic_css_usage_report(inventory, observed, scope).map_err(CliFailure::invalid)?;
    if let Some(control_dir) = control_dir {
        let control_dir =
            existing_artifact_directory(control_dir, "generic CSS usage control output")
                .map_err(CliFailure::invalid)?;
        let output_file = control_output_logical_path(&control_dir, output)?;
        let relationships = Vec::new();
        let outputs = [ControlOutputInput {
            file: &output_file,
            role: "generic-css-usage",
            media_type: "application/json",
            bytes: &bytes,
            source_map: None,
            relationships: &relationships,
        }];
        let source_inputs = [
            AuditSourceInput {
                logical_path: "generic-css-findings.json",
                role: "findings",
                bytes: &findings_bytes,
            },
            AuditSourceInput {
                logical_path: "generic-css-observed.json",
                role: "positive-observations",
                bytes: &observed_bytes,
            },
        ];
        let group = build_audit_control_group(AuditControlInput {
            source_inputs: &source_inputs,
            config_inputs: &[],
            generated_outputs: &outputs,
            document: &document,
            passed: false,
            rule_metrics: None,
            rules_unavailable_reason:
                "generic CSS usage consumes declaration identities from audit findings",
            token_observation: TokenObservationInput::Unavailable(
                "generic CSS usage has no typed token graph input",
            ),
            token_graph: None,
            output_relationships: std::slice::from_ref(&output_file),
            asset_plan_verified: false,
            targets: CompatibilityProfile::None,
            budget_policy: None,
            budget_policy_path: None,
            budget_policy_bytes: None,
            budget_subjects: &[],
        })
        .map_err(CliFailure::tool)?;
        let mut payloads = vec![BundleOutputPayload {
            destination: output.to_owned(),
            bytes: bytes.clone(),
        }];
        payloads.extend(group.artifacts().map(|artifact| BundleOutputPayload {
            destination: control_dir.join(artifact.file),
            bytes: artifact.bytes.to_vec(),
        }));
        publish_output_group(&payloads).map_err(CliFailure::tool)?;
        return Ok(());
    }
    if let Some(prepared) =
        prepare_atomic_write_if_changed(output, &bytes).map_err(CliFailure::from)?
    {
        prepared.commit().map_err(CliFailure::from)?;
    }
    Ok(())
}

fn parse_generic_css_usage_arguments(arguments: &[String]) -> Result<Command, String> {
    let mut findings = None;
    let mut observed = None;
    let mut scope = None;
    let mut output = None;
    let mut control_dir = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--findings" => set_path_once(&mut findings, arguments, index, "--findings")?,
            "--observed" => set_path_once(&mut observed, arguments, index, "--observed")?,
            "--scope" => set_string_once(&mut scope, arguments, index, "--scope")?,
            "--output" => set_path_once(&mut output, arguments, index, "--output")?,
            "--control-dir" => {
                set_path_once(&mut control_dir, arguments, index, "--control-dir")?;
            }
            unknown => return Err(format!("unknown generic-css-usage option `{unknown}`")),
        }
        index += 2;
    }
    Ok(Command::GenericCssUsage {
        findings: findings.ok_or("`generic-css-usage` requires `--findings`")?,
        observed: observed.ok_or("`generic-css-usage` requires `--observed`")?,
        scope: scope.ok_or("`generic-css-usage` requires `--scope`")?,
        output: output.ok_or("`generic-css-usage` requires `--output`")?,
        control_dir,
    })
}

fn parse_explain_arguments(arguments: &[String]) -> Result<Command, String> {
    let mut style = None;
    let mut config = None;
    let mut seed = false;
    let mut targets = None;
    let mut format = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--style" => {
                set_string_once(&mut style, arguments, index, "--style")?;
                index += 2;
            }
            "--config" => {
                set_path_once(&mut config, arguments, index, "--config")?;
                index += 2;
            }
            "--seed" => {
                if seed {
                    return Err("`--seed` may only be provided once".into());
                }
                seed = true;
                index += 1;
            }
            "--targets" => {
                if targets.is_some() {
                    return Err("`--targets` may only be provided once".into());
                }
                targets = Some(parse_targets(option_value(arguments, index, "--targets")?)?);
                index += 2;
            }
            "--format" => {
                if format.is_some() {
                    return Err("`--format` may only be provided once".into());
                }
                format = Some(match option_value(arguments, index, "--format")? {
                    "text" => ExplainFormat::Text,
                    "json" => ExplainFormat::Json,
                    value => {
                        return Err(format!(
                            "unknown explain format `{value}`; expected `text` or `json`"
                        ));
                    }
                });
                index += 2;
            }
            option => return Err(format!("unknown explain option `{option}`")),
        }
    }
    if seed && config.is_some() {
        return Err("`--seed` conflicts with `--config`".into());
    }
    Ok(Command::Explain(ExplainArgs {
        style: style.ok_or_else(|| "`explain` requires `--style`".to_owned())?,
        config,
        seed,
        targets: targets.unwrap_or(TargetContract::Modern),
        format: format.unwrap_or_default(),
    }))
}

fn parse_cascade_explain_arguments(arguments: &[String]) -> Result<Command, String> {
    let mut input = None;
    let mut element = None;
    let mut property = None;
    let mut format = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--input" => {
                set_path_once(&mut input, arguments, index, "--input")?;
                index += 2;
            }
            "--element" => {
                set_string_once(&mut element, arguments, index, "--element")?;
                index += 2;
            }
            "--property" => {
                set_string_once(&mut property, arguments, index, "--property")?;
                index += 2;
            }
            "--format" => {
                if format.is_some() {
                    return Err("`--format` may only be provided once".into());
                }
                format = Some(match option_value(arguments, index, "--format")? {
                    "text" => ExplainFormat::Text,
                    "json" => ExplainFormat::Json,
                    value => {
                        return Err(format!(
                            "unknown explain-cascade format `{value}`; expected `text` or `json`"
                        ));
                    }
                });
                index += 2;
            }
            option => return Err(format!("unknown explain-cascade option `{option}`")),
        }
    }
    Ok(Command::ExplainCascade(CascadeExplainArgs {
        input: input.ok_or_else(|| "`explain-cascade` requires `--input FILE`".to_owned())?,
        element: element
            .ok_or_else(|| "`explain-cascade` requires `--element SELECTOR`".to_owned())?,
        property: property
            .ok_or_else(|| "`explain-cascade` requires `--property LONGHAND`".to_owned())?,
        format: format.unwrap_or_default(),
    }))
}

fn parse_utility_format_arguments(arguments: &[String]) -> Result<Command, String> {
    let mut parsed = UtilityFormatArgs::default();
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--style" => {
                parsed
                    .styles
                    .push(option_value(arguments, index, "--style")?.to_owned());
                index += 2;
            }
            "--input" => {
                set_path_once(&mut parsed.input, arguments, index, "--input")?;
                index += 2;
            }
            "--source" => {
                parsed
                    .sources
                    .push(PathBuf::from(option_value(arguments, index, "--source")?));
                index += 2;
            }
            "--output" => {
                set_path_once(&mut parsed.output, arguments, index, "--output")?;
                index += 2;
            }
            "--check" => {
                if parsed.check {
                    return Err("`--check` may only be provided once".into());
                }
                parsed.check = true;
                index += 1;
            }
            "--apply" => {
                if parsed.apply {
                    return Err("duplicate `--apply`".into());
                }
                parsed.apply = true;
                index += 1;
            }
            option => return Err(format!("unknown fmt option `{option}`")),
        }
    }
    let input_forms = usize::from(!parsed.styles.is_empty())
        + usize::from(parsed.input.is_some())
        + usize::from(!parsed.sources.is_empty());
    if input_forms != 1 {
        return Err(
            "`fmt` requires exactly one of repeatable `--style`, one `--input`, or repeatable `--source`"
                .into(),
        );
    }
    if (parsed.check || parsed.apply) && parsed.output.is_some() {
        return Err("`--output` conflicts with check/apply".into());
    }
    if parsed.check && parsed.apply {
        return Err("`--check` conflicts with `--apply`".into());
    }
    if let (Some(input), Some(output)) = (&parsed.input, &parsed.output) {
        validate_path_roles(&[("--input", input)], &[("--output", output)])?;
    }
    if !parsed.sources.is_empty() && (parsed.check == parsed.apply || parsed.output.is_some()) {
        return Err("choose `--check` or `--apply`".into());
    }
    if parsed.apply && parsed.sources.is_empty() {
        return Err("`--apply` requires `--source`".into());
    }
    Ok(Command::Format(parsed))
}

#[allow(clippy::too_many_lines)]
fn parse_build_arguments(arguments: &[String]) -> Result<(BuildArgs, Option<String>), String> {
    let mut parsed = BuildArgs::default();
    let mut manifest_version = None;
    let mut targets_seen = false;
    let mut format_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--style" => {
                parsed
                    .styles
                    .push(option_value(arguments, index, "--style")?.to_owned());
                index += 2;
            }
            "--compose" => {
                let base = option_value(arguments, index, "--compose base")?;
                let branch = option_value(arguments, index + 1, "--compose branch")?;
                parsed
                    .compositions
                    .push((base.to_owned(), branch.to_owned()));
                index += 3;
            }
            "--input" => {
                set_path_once(&mut parsed.input, arguments, index, "--input")?;
                index += 2;
            }
            "--source" => {
                parsed
                    .sources
                    .push(PathBuf::from(option_value(arguments, index, "--source")?));
                index += 2;
            }
            "--config" => {
                set_path_once(&mut parsed.config, arguments, index, "--config")?;
                index += 2;
            }
            "--tokens" => {
                set_path_once(&mut parsed.tokens, arguments, index, "--tokens")?;
                index += 2;
            }
            "--token-input" => {
                parsed.token_inputs.push(parse_token_input(option_value(
                    arguments,
                    index,
                    "--token-input",
                )?)?);
                index += 2;
            }
            "--seed" => {
                set_flag_once(&mut parsed.seed, "--seed")?;
                index += 1;
            }
            "--theme" => {
                set_flag_once(&mut parsed.theme, "--theme")?;
                index += 1;
            }
            "--targets" => {
                set_flag_once(&mut targets_seen, "--targets")?;
                parsed.targets = parse_targets(option_value(arguments, index, "--targets")?)?;
                index += 2;
            }
            "--format" => {
                set_flag_once(&mut format_seen, "--format")?;
                parsed.format = parse_format(option_value(arguments, index, "--format")?)?;
                index += 2;
            }
            "--output" => {
                set_path_once(&mut parsed.output, arguments, index, "--output")?;
                index += 2;
            }
            "--manifest" => {
                set_path_once(&mut parsed.manifest, arguments, index, "--manifest")?;
                index += 2;
            }
            "--control-dir" => {
                set_path_once(&mut parsed.control_dir, arguments, index, "--control-dir")?;
                index += 2;
            }
            "--check" => {
                set_flag_once(&mut parsed.check, "--check")?;
                index += 1;
            }
            "--manifest-version" => {
                set_string_once(
                    &mut manifest_version,
                    arguments,
                    index,
                    "--manifest-version",
                )?;
                index += 2;
            }
            "--reachability" => {
                set_path_once(&mut parsed.reachability, arguments, index, "--reachability")?;
                index += 2;
            }
            "--prune-unreachable" => {
                set_pruning_once(&mut parsed.pruning)?;
                index += 1;
            }
            "-h" | "--help" => return Err("help must be the first command option".into()),
            unknown => return Err(format!("unknown option `{unknown}`")),
        }
    }
    if parsed.styles.is_empty()
        && parsed.compositions.is_empty()
        && parsed.input.is_none()
        && parsed.sources.is_empty()
    {
        return Err("provide at least one `--style`, `--compose`, `--input`, or `--source`".into());
    }
    if parsed.seed && parsed.config.is_some() {
        return Err("`--seed` conflicts with `--config`".into());
    }
    if parsed.tokens.is_some() && (parsed.config.is_some() || parsed.seed) {
        return Err("`--tokens` conflicts with `--config` and `--seed`".into());
    }
    if parsed.tokens.is_none() && !parsed.token_inputs.is_empty() {
        return Err("`--token-input` requires `--tokens FILE`".into());
    }
    if parsed.check && parsed.control_dir.is_none() {
        return Err("compile/build `--check` requires `--control-dir DIR`".into());
    }
    if parsed.control_dir.is_some() && (parsed.output.is_none() || parsed.manifest.is_none()) {
        return Err("`--control-dir` requires both `--output` and `--manifest`".into());
    }
    Ok((parsed, manifest_version))
}

#[allow(clippy::too_many_lines)]
fn parse_watch_arguments(arguments: &[String]) -> Result<Command, String> {
    let mut input = None;
    let mut sources = Vec::new();
    let mut output = None;
    let mut config = None;
    let mut tokens = None;
    let mut token_inputs = Vec::new();
    let mut seed = false;
    let mut theme = false;
    let mut targets = TargetContract::Modern;
    let mut format = CssFormat::Minified;
    let mut targets_seen = false;
    let mut format_seen = false;
    let mut manifest = None;
    let mut control_dir = None;
    let mut manifest_version = None;
    let mut reachability = None;
    let mut pruning = ReachabilityPruning::Disabled;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--input" => {
                set_path_once(&mut input, arguments, index, "--input")?;
                index += 2;
            }
            "--source" => {
                sources.push(PathBuf::from(option_value(arguments, index, "--source")?));
                index += 2;
            }
            "--output" => {
                set_path_once(&mut output, arguments, index, "--output")?;
                index += 2;
            }
            "--config" => {
                set_path_once(&mut config, arguments, index, "--config")?;
                index += 2;
            }
            "--tokens" => {
                set_path_once(&mut tokens, arguments, index, "--tokens")?;
                index += 2;
            }
            "--token-input" => {
                token_inputs.push(parse_token_input(option_value(
                    arguments,
                    index,
                    "--token-input",
                )?)?);
                index += 2;
            }
            "--seed" => {
                set_flag_once(&mut seed, "--seed")?;
                index += 1;
            }
            "--theme" => {
                set_flag_once(&mut theme, "--theme")?;
                index += 1;
            }
            "--targets" => {
                set_flag_once(&mut targets_seen, "--targets")?;
                targets = parse_targets(option_value(arguments, index, "--targets")?)?;
                index += 2;
            }
            "--format" => {
                set_flag_once(&mut format_seen, "--format")?;
                format = parse_format(option_value(arguments, index, "--format")?)?;
                index += 2;
            }
            "--manifest" => {
                set_path_once(&mut manifest, arguments, index, "--manifest")?;
                index += 2;
            }
            "--control-dir" => {
                set_path_once(&mut control_dir, arguments, index, "--control-dir")?;
                index += 2;
            }
            "--manifest-version" => {
                set_string_once(
                    &mut manifest_version,
                    arguments,
                    index,
                    "--manifest-version",
                )?;
                index += 2;
            }
            "--reachability" => {
                set_path_once(&mut reachability, arguments, index, "--reachability")?;
                index += 2;
            }
            "--prune-unreachable" => {
                set_pruning_once(&mut pruning)?;
                index += 1;
            }
            unknown => return Err(format!("unknown watch option `{unknown}`")),
        }
    }
    if input.is_none() && sources.is_empty() {
        return Err("watch requires at least one `--input` or `--source`".to_owned());
    }
    let output = output.ok_or_else(|| "watch requires `--output`".to_owned())?;
    if seed && config.is_some() {
        return Err("`--seed` conflicts with `--config`".into());
    }
    if tokens.is_some() && (config.is_some() || seed) {
        return Err("`--tokens` conflicts with `--config` and `--seed`".into());
    }
    if tokens.is_none() && !token_inputs.is_empty() {
        return Err("`--token-input` requires `--tokens FILE`".into());
    }
    if control_dir.is_some() && manifest.is_none() {
        return Err("watch `--control-dir` requires `--manifest`".into());
    }
    let physical_trace = validate_manifest_graph_options(
        manifest_version.as_deref(),
        reachability.as_deref(),
        manifest.is_some(),
        pruning,
    )?;
    let parsed = WatchArgs {
        input,
        sources,
        output,
        config,
        tokens,
        token_inputs,
        seed,
        theme,
        targets,
        format,
        manifest,
        control_dir,
        reachability,
        physical_trace,
        pruning,
    };
    validate_watch_paths(&parsed)?;
    Ok(Command::Watch(parsed))
}

fn validate_build_paths(arguments: &BuildArgs) -> Result<(), String> {
    let mut inputs = Vec::new();
    if let Some(path) = &arguments.input {
        inputs.push(("--input", path.as_path()));
    }
    if let Some(path) = &arguments.config {
        inputs.push(("--config", path.as_path()));
    }
    if let Some(path) = &arguments.tokens {
        inputs.push(("--tokens", path.as_path()));
    }
    if let Some(path) = &arguments.reachability {
        inputs.push(("--reachability", path.as_path()));
    }
    inputs.extend(
        arguments
            .sources
            .iter()
            .map(|path| ("--source", path.as_path())),
    );
    let mut outputs = Vec::new();
    if let Some(path) = &arguments.output {
        outputs.push(("--output", path.as_path()));
    }
    if let Some(path) = &arguments.manifest {
        outputs.push(("--manifest", path.as_path()));
    }
    let source_map = arguments
        .control_dir
        .as_ref()
        .and(arguments.output.as_deref())
        .map(css_source_map_path);
    if let Some(path) = &source_map {
        outputs.push(("--control-dir source map", path.as_path()));
    }
    let control_paths = arguments.control_dir.as_ref().map(|directory| {
        [
            directory.join(pliego_css_control::TOKEN_GRAPH_FILE),
            directory.join(control::FINDINGS_FILE),
            directory.join(pliego_css_control::CONTROL_MANIFEST_FILE),
            directory.join(pliego_css_control::BUILD_RECEIPT_FILE),
        ]
    });
    if let Some(paths) = &control_paths {
        outputs.extend(paths.iter().map(|path| ("--control-dir", path.as_path())));
    }
    validate_path_roles(&inputs, &outputs)
}

fn validate_watch_paths(arguments: &WatchArgs) -> Result<(), String> {
    let mut inputs = Vec::new();
    if let Some(path) = &arguments.input {
        inputs.push(("--input", path.as_path()));
    }
    inputs.extend(
        arguments
            .sources
            .iter()
            .map(|path| ("--source", path.as_path())),
    );
    if let Some(path) = &arguments.config {
        inputs.push(("--config", path.as_path()));
    }
    if let Some(path) = &arguments.tokens {
        inputs.push(("--tokens", path.as_path()));
    }
    if let Some(path) = &arguments.reachability {
        inputs.push(("--reachability", path.as_path()));
    }
    let mut outputs = vec![("--output", arguments.output.as_path())];
    if let Some(path) = &arguments.manifest {
        outputs.push(("--manifest", path.as_path()));
    }
    let source_map = arguments
        .control_dir
        .as_ref()
        .map(|_| css_source_map_path(&arguments.output));
    if let Some(path) = &source_map {
        outputs.push(("--control-dir source map", path.as_path()));
    }
    let control_paths = arguments.control_dir.as_ref().map(|directory| {
        [
            directory.join(pliego_css_control::TOKEN_GRAPH_FILE),
            directory.join(control::FINDINGS_FILE),
            directory.join(pliego_css_control::CONTROL_MANIFEST_FILE),
            directory.join(pliego_css_control::BUILD_RECEIPT_FILE),
        ]
    });
    if let Some(paths) = &control_paths {
        outputs.extend(paths.iter().map(|path| ("--control-dir", path.as_path())));
    }
    validate_path_roles(&inputs, &outputs)
}

fn validate_path_roles(inputs: &[(&str, &Path)], outputs: &[(&str, &Path)]) -> Result<(), String> {
    let input_keys = inputs
        .iter()
        .map(|(role, path)| Ok((*role, path, path_key(path)?)))
        .collect::<Result<Vec<_>, String>>()?;
    let output_keys = outputs
        .iter()
        .map(|(role, path)| Ok((*role, path, path_key(path)?)))
        .collect::<Result<Vec<_>, String>>()?;

    for (index, (left_role, left_path, left_key)) in output_keys.iter().enumerate() {
        for (right_role, right_path, right_key) in &output_keys[index + 1..] {
            if left_key == right_key {
                return Err(format!(
                    "`{left_role}` path `{}` aliases `{right_role}` path `{}`",
                    left_path.display(),
                    right_path.display()
                ));
            }
        }
        for (input_role, input_path, input_key) in &input_keys {
            if left_key == input_key {
                return Err(format!(
                    "`{left_role}` path `{}` aliases input `{input_role}` path `{}`",
                    left_path.display(),
                    input_path.display()
                ));
            }
        }
    }
    Ok(())
}

fn path_key(path: &Path) -> Result<String, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| format!("cannot resolve `{}`: {error}", path.display()))?
            .join(path)
    };
    let resolved = canonicalize_existing_ancestor(&absolute);
    Ok(resolved.to_string_lossy().to_ascii_lowercase())
}

fn publication_path_identity(path: &Path) -> Result<(String, PathBuf), String> {
    let absolute = absolute_path(path)?;
    let file_name = absolute
        .file_name()
        .ok_or_else(|| format!("invalid publication destination `{}`", path.display()))?;
    let parent = absolute
        .parent()
        .ok_or_else(|| format!("invalid publication destination `{}`", path.display()))?;
    let mut resolved = canonicalize_existing_ancestor(parent);
    resolved.push(file_name);
    let key = resolved.to_string_lossy().to_ascii_lowercase();
    Ok((key, resolved))
}

fn canonicalize_existing_ancestor(path: &Path) -> PathBuf {
    let normalized = normalize_path(path);
    let mut current = normalized.as_path();
    let mut missing = Vec::new();
    loop {
        if let Ok(mut resolved) = fs::canonicalize(current) {
            for component in missing.iter().rev() {
                resolved.push(component);
            }
            return normalize_path(&resolved);
        }
        let (Some(name), Some(parent)) = (current.file_name(), current.parent()) else {
            return normalized;
        };
        missing.push(name.to_os_string());
        current = parent;
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn set_path_once(
    slot: &mut Option<PathBuf>,
    arguments: &[String],
    index: usize,
    option: &str,
) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("`{option}` may only be provided once"));
    }
    *slot = Some(PathBuf::from(option_value(arguments, index, option)?));
    Ok(())
}

fn set_flag_once(slot: &mut bool, option: &str) -> Result<(), String> {
    if *slot {
        return Err(format!("`{option}` may only be provided once"));
    }
    *slot = true;
    Ok(())
}

fn set_pruning_once(slot: &mut ReachabilityPruning) -> Result<(), String> {
    if slot.is_enabled() {
        return Err("`--prune-unreachable` may only be provided once".into());
    }
    *slot = ReachabilityPruning::Unreachable;
    Ok(())
}

fn validate_manifest_graph_options(
    version: Option<&str>,
    reachability: Option<&Path>,
    has_manifest: bool,
    pruning: ReachabilityPruning,
) -> Result<bool, String> {
    if version.is_some_and(|version| !matches!(version, "3" | "4" | "5")) {
        return Err("manifest version must be 3, 4, or 5".into());
    }
    if version.is_some() && !has_manifest {
        return Err("`--manifest-version` requires manifest output".into());
    }
    if reachability.is_some() && !matches!(version, Some("4" | "5")) {
        return Err("`--reachability` requires `--manifest-version 4` or `5`".into());
    }
    if matches!(version, Some("4" | "5")) && reachability.is_none() {
        return Err(format!(
            "manifest version {} requires `--reachability`",
            version.unwrap_or_default()
        ));
    }
    if pruning.is_enabled() && reachability.is_none() {
        return Err("`--prune-unreachable` requires `--reachability`".into());
    }
    Ok(version == Some("5"))
}

fn option_value<'a>(
    arguments: &'a [String],
    index: usize,
    option: &str,
) -> Result<&'a str, String> {
    arguments
        .get(index + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("missing value after `{option}`"))
}

fn set_string_once(
    slot: &mut Option<String>,
    arguments: &[String],
    index: usize,
    option: &str,
) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("`{option}` may only be provided once"));
    }
    *slot = Some(option_value(arguments, index, option)?.to_owned());
    Ok(())
}

fn parse_targets(value: &str) -> Result<TargetContract, String> {
    match value {
        "modern" => Ok(TargetContract::Modern),
        "baseline-widely" => Ok(TargetContract::BaselineWidely),
        "none" => Ok(TargetContract::None),
        _ => Err(format!(
            "unknown target contract `{value}`; expected `baseline-widely`, `modern`, or `none`"
        )),
    }
}

fn parse_format(value: &str) -> Result<CssFormat, String> {
    match value {
        "minified" => Ok(CssFormat::Minified),
        "pretty" => Ok(CssFormat::Pretty),
        _ => Err(format!(
            "unknown CSS format `{value}`; expected `minified` or `pretty`"
        )),
    }
}

fn parse_token_input(value: &str) -> Result<(String, String), String> {
    let (modifier, context) = value
        .split_once('=')
        .ok_or_else(|| format!("invalid `--token-input` `{value}`; expected `modifier=context`"))?;
    if modifier.is_empty() || context.is_empty() {
        return Err(format!(
            "invalid `--token-input` `{value}`; expected non-empty `modifier=context`"
        ));
    }
    Ok((modifier.to_owned(), context.to_owned()))
}

fn parse_catalog_format(value: &str) -> Result<CatalogFormat, String> {
    match value {
        "markdown" => Ok(CatalogFormat::Markdown),
        "json" => Ok(CatalogFormat::Json),
        _ => Err(format!(
            "unknown catalog format `{value}`; expected `markdown` or `json`"
        )),
    }
}

fn load_theme(config: Option<&Path>) -> Result<ThemeRegistry, String> {
    config.map_or_else(
        || Ok(ThemeRegistry::seed()),
        |path| pliego_css_config::parse_path(path).map_err(|error| error.to_string()),
    )
}

fn load_reachability(path: Option<&Path>) -> Result<Option<ReachabilityDocument>, String> {
    path.map(|path| {
        let bytes = read_reachability(path)?;
        parse_reachability_document(&bytes).map_err(|error| format!("{}: {error}", path.display()))
    })
    .transpose()
}

fn read_reachability(path: &Path) -> Result<Vec<u8>, String> {
    pliego_css_io::read_bounded_regular_file(path, MAX_DOCUMENT_BYTES, "reachability input")
}

fn resolve_dtcg_theme(source: &str, inputs: &[(String, String)]) -> Result<DtcgTheme, String> {
    parse_dtcg_resolver_str(source)
        .and_then(|resolver| resolver.resolve_entries(inputs.iter().cloned()))
        .map_err(|error| error.to_string())
}

fn read_bounded_utf8_document(path: &Path, role: &str) -> Result<Vec<u8>, String> {
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {role} `{}`: {error}", path.display()))?;
    if is_link_like(&path_metadata) || !path_metadata.is_file() {
        return Err(format!(
            "{role} `{}` must be a regular file of at most {MAX_DOCUMENT_BYTES} bytes",
            path.display()
        ));
    }

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    #[cfg(windows)]
    options.custom_flags(0x0020_0000);
    let file = options
        .open(path)
        .map_err(|error| format!("cannot open {role} `{}`: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect opened {role} `{}`: {error}", path.display()))?;
    if is_link_like(&metadata) || !metadata.is_file() {
        return Err(format!(
            "{role} `{}` changed before it could be opened as a regular file",
            path.display()
        ));
    }
    if metadata.len() > MAX_DOCUMENT_BYTES as u64 {
        return Err(format!(
            "{role} `{}` exceeds {MAX_DOCUMENT_BYTES} bytes",
            path.display()
        ));
    }

    let mut bytes = Vec::new();
    file.take((MAX_DOCUMENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {role} `{}`: {error}", path.display()))?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(format!(
            "{role} `{}` exceeds {MAX_DOCUMENT_BYTES} bytes",
            path.display()
        ));
    }
    std::str::from_utf8(&bytes)
        .map_err(|error| format!("{role} `{}` is not valid UTF-8: {error}", path.display()))?;
    Ok(bytes)
}

fn load_build_theme(arguments: &BuildArgs) -> Result<LoadedTheme, String> {
    if let Some(tokens) = arguments.tokens.as_deref() {
        validate_resolved_theme_outputs(
            Some(tokens),
            arguments.output.as_deref(),
            arguments.manifest.as_deref(),
        )?;
        let bytes = read_bounded_utf8_document(tokens, "token resolver")?;
        let source = std::str::from_utf8(&bytes).expect("bounded document was validated as UTF-8");
        return resolve_dtcg_theme(source, &arguments.token_inputs).map(LoadedTheme::Dtcg);
    }
    let hints = arguments
        .input
        .iter()
        .chain(arguments.sources.iter())
        .map(PathBuf::as_path);
    let config = resolve_theme_config(arguments.config.as_deref(), arguments.seed, hints)?;
    validate_resolved_theme_outputs(
        config.as_deref(),
        arguments.output.as_deref(),
        arguments.manifest.as_deref(),
    )?;
    load_theme(config.as_deref()).map(LoadedTheme::Registry)
}

fn capture_build_snapshot(arguments: &BuildArgs) -> WatchSnapshot {
    WatchSnapshot {
        input: arguments
            .input
            .as_ref()
            .map(|path| snapshot_file(path, "line-oriented input")),
        sources: expand_source_paths(&arguments.sources).map(|paths| {
            paths
                .into_iter()
                .map(|path| snapshot_file(&path, "Rust source"))
                .collect()
        }),
        theme: arguments.tokens.as_ref().map_or_else(
            || {
                resolve_theme_config(
                    arguments.config.as_deref(),
                    arguments.seed,
                    arguments
                        .input
                        .iter()
                        .chain(arguments.sources.iter())
                        .map(PathBuf::as_path),
                )
                .map(|config| config.map(|path| snapshot_file(&path, "theme configuration")))
            },
            |path| Ok(Some(snapshot_token_file(path))),
        ),
        reachability: arguments
            .reachability
            .as_ref()
            .map(|path| WatchFileSnapshot {
                path: path.clone(),
                contents: read_reachability(path),
            }),
    }
}

fn load_build_theme_snapshot(
    arguments: &BuildArgs,
    snapshot: &WatchSnapshot,
) -> Result<LoadedTheme, String> {
    let config = snapshot.theme.as_ref().map_err(Clone::clone)?;
    validate_resolved_theme_outputs(
        config.as_ref().map(|file| file.path.as_path()),
        arguments.output.as_deref(),
        arguments.manifest.as_deref(),
    )?;
    if arguments.tokens.is_some() {
        let file = config
            .as_ref()
            .expect("explicit token resolver must have a snapshot");
        return resolve_dtcg_theme(file.utf8("token resolver")?, &arguments.token_inputs)
            .map(LoadedTheme::Dtcg);
    }
    config.as_ref().map_or_else(
        || Ok(LoadedTheme::Registry(ThemeRegistry::seed())),
        |file| {
            pliego_css_config::parse_str(file.utf8("theme configuration")?)
                .map(LoadedTheme::Registry)
                .map_err(|error| error.to_string())
        },
    )
}

fn compile_build_snapshot(
    arguments: &BuildArgs,
    snapshot: &WatchSnapshot,
) -> Result<(LoadedTheme, CompiledArtifact), CliFailure> {
    let theme = load_build_theme_snapshot(arguments, snapshot).map_err(CliFailure::tool)?;
    let registry = theme.registry();
    let mut candidates = cli_candidates(&arguments.styles, &arguments.compositions);
    if let Some(input) = &snapshot.input {
        let logical_path = control_input_logical_path(&input.path)?;
        candidates.extend(candidates_from_line_source(
            Path::new(&logical_path),
            input
                .utf8("line-oriented input")
                .map_err(CliFailure::tool)?,
        )?);
    }
    for source in snapshot
        .sources
        .as_ref()
        .map_err(|error| CliFailure::tool(error.clone()))?
    {
        let text = source.utf8("Rust source").map_err(CliFailure::tool)?;
        let report = scan_source_named(control_input_logical_path(&source.path)?, text)
            .map_err(|error| CliFailure::tool(error.to_string()))?;
        candidates.extend(candidates_from_scan_report(registry, &report)?);
    }
    if candidates.is_empty() {
        return Err(CliFailure::tool("no styles found in supplied inputs"));
    }
    let reachability = snapshot
        .reachability
        .as_ref()
        .map(|file| parse_reachability_document(file.bytes()?))
        .transpose()
        .map_err(CliFailure::tool)?;
    let artifact = compile_candidates_with_manifest(
        registry,
        &candidates,
        arguments.theme,
        arguments.targets,
        arguments.format,
        ArtifactGraphOptions {
            reachability: reachability.as_ref(),
            physical_trace: arguments.physical_trace,
            pruning: arguments.pruning,
            ..ArtifactGraphOptions::default()
        },
    )?;
    Ok((theme, artifact))
}

fn run_controlled_compile(arguments: &BuildArgs) -> Result<(), CliFailure> {
    let control_dir = existing_artifact_directory(
        arguments
            .control_dir
            .as_deref()
            .expect("controlled compile has a control directory"),
        "control output",
    )
    .map_err(CliFailure::invalid)?;
    let output = arguments
        .output
        .as_deref()
        .expect("controlled compile has CSS output");
    let manifest = arguments
        .manifest
        .as_deref()
        .expect("controlled compile has manifest output");
    let output_logical = control_output_logical_path(&control_dir, output)?;
    let manifest_logical = control_output_logical_path(&control_dir, manifest)?;
    let source_map = css_source_map_path(output);
    let source_map_logical = control_output_logical_path(&control_dir, &source_map)?;
    let snapshot = capture_build_snapshot(arguments);
    validate_control_snapshot_paths(&snapshot, output, manifest, &control_dir)?;
    let (theme, artifact) = compile_build_snapshot(arguments, &snapshot)?;
    let (group, passed, source_map_bytes) = build_compilation_control_group(
        CompilationControlSettings {
            operation: "compile",
            styles: &arguments.styles,
            compositions: &arguments.compositions,
            theme_request: ControlThemeRequest::from_arguments(
                arguments.tokens.is_some(),
                arguments.config.is_some(),
                arguments.seed,
            ),
            emit_theme: arguments.theme,
            targets: arguments.targets,
            format: arguments.format,
            physical_trace: arguments.physical_trace,
            pruning: arguments.pruning,
        },
        &snapshot,
        &theme,
        &artifact,
        &output_logical,
        &manifest_logical,
        &source_map_logical,
    )?;
    let mut payloads = vec![
        BundleOutputPayload {
            destination: output.to_owned(),
            bytes: artifact.css.into_bytes(),
        },
        BundleOutputPayload {
            destination: manifest.to_owned(),
            bytes: artifact.manifest.into_bytes(),
        },
        BundleOutputPayload {
            destination: source_map,
            bytes: source_map_bytes,
        },
    ];
    payloads.extend(group.artifacts().map(|artifact| BundleOutputPayload {
        destination: control_dir.join(artifact.file),
        bytes: artifact.bytes.to_vec(),
    }));
    if arguments.check {
        check_controlled_compile_outputs(&payloads)?;
        println!("ok: {} controlled compile output(s) match", payloads.len());
    } else {
        let changed = publish_output_group(&payloads).map_err(CliFailure::tool)?;
        println!(
            "ok: {} controlled compile output(s), {changed} changed",
            payloads.len()
        );
    }
    if passed {
        Ok(())
    } else {
        Err(CliFailure::tool("compile control audit failed"))
    }
}

fn validate_control_snapshot_paths(
    snapshot: &WatchSnapshot,
    output: &Path,
    manifest: &Path,
    control_dir: &Path,
) -> Result<(), CliFailure> {
    let mut inputs = Vec::new();
    if let Some(input) = &snapshot.input {
        inputs.push(("control input", input.path.as_path()));
    }
    inputs.extend(
        snapshot
            .sources
            .as_ref()
            .map_err(|error| CliFailure::tool(error.clone()))?
            .iter()
            .map(|source| ("control source", source.path.as_path())),
    );
    if let Some(theme) = snapshot
        .theme
        .as_ref()
        .map_err(|error| CliFailure::tool(error.clone()))?
    {
        inputs.push(("control theme", theme.path.as_path()));
    }
    if let Some(reachability) = &snapshot.reachability {
        inputs.push(("control reachability", reachability.path.as_path()));
    }
    let control_paths = [
        control_dir.join(pliego_css_control::TOKEN_GRAPH_FILE),
        control_dir.join(control::FINDINGS_FILE),
        control_dir.join(pliego_css_control::CONTROL_MANIFEST_FILE),
        control_dir.join(pliego_css_control::BUILD_RECEIPT_FILE),
    ];
    let mut outputs = vec![
        ("control CSS", output),
        ("control style manifest", manifest),
    ];
    let source_map = css_source_map_path(output);
    outputs.push(("control CSS source map", source_map.as_path()));
    outputs.extend(
        control_paths
            .iter()
            .map(|path| ("control artifact", path.as_path())),
    );
    validate_path_roles(&inputs, &outputs).map_err(CliFailure::invalid)
}

fn control_output_logical_path(control_dir: &Path, output: &Path) -> Result<String, CliFailure> {
    validate_output_destination(output, "controlled output").map_err(CliFailure::invalid)?;
    let absolute = absolute_path(output).map_err(CliFailure::invalid)?;
    let parent = absolute
        .parent()
        .ok_or_else(|| CliFailure::invalid("controlled output has no parent directory"))?;
    let parent = fs::canonicalize(parent).map_err(|error| {
        CliFailure::invalid(format!(
            "cannot canonicalize controlled output parent `{}`: {error}",
            parent.display()
        ))
    })?;
    let mut resolved = parent;
    resolved.push(
        absolute
            .file_name()
            .expect("controlled output path has a parent and filename"),
    );
    let relative = resolved.strip_prefix(control_dir).map_err(|_| {
        CliFailure::invalid(format!(
            "controlled output `{}` must be inside `--control-dir` `{}`",
            output.display(),
            control_dir.display()
        ))
    })?;
    portable_logical_components(relative, "controlled output").map_err(CliFailure::invalid)
}

fn css_source_map_path(output: &Path) -> PathBuf {
    let mut path = output.as_os_str().to_os_string();
    path.push(".map");
    PathBuf::from(path)
}

fn control_input_logical_path(path: &Path) -> Result<String, CliFailure> {
    let relative = if path.is_absolute() {
        let current = env::current_dir().map_err(|error| {
            CliFailure::tool(format!("cannot inspect current directory: {error}"))
        })?;
        normalize_path(path)
            .strip_prefix(normalize_path(&current))
            .map(Path::to_path_buf)
            .map_err(|_| {
                CliFailure::invalid(format!(
                    "control input `{}` resolves outside the current workspace",
                    path.display()
                ))
            })?
    } else {
        path.to_owned()
    };
    let logical =
        portable_logical_components(&relative, "control input").map_err(CliFailure::invalid)?;
    reject_audit_link_components(Path::new(&logical), "control input")
        .map_err(CliFailure::invalid)?;
    Ok(logical)
}

fn portable_logical_components(path: &Path, role: &str) -> Result<String, String> {
    let components = path
        .components()
        .map(|component| match component {
            Component::Normal(value) => value
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{role} path `{}` is not valid UTF-8", path.display())),
            _ => Err(format!(
                "{role} path `{}` must be portable and contain no root, `.` or `..` components",
                path.display()
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if components.is_empty() {
        return Err(format!("{role} path must not be empty"));
    }
    Ok(components.join("/"))
}

#[allow(clippy::too_many_lines)]
fn build_compilation_control_group(
    settings: CompilationControlSettings<'_>,
    snapshot: &WatchSnapshot,
    theme: &LoadedTheme,
    artifact: &CompiledArtifact,
    output_logical: &str,
    manifest_logical: &str,
    source_map_logical: &str,
) -> Result<(AuditControlGroup, bool, Vec<u8>), CliFailure> {
    let outcome = audit_standard_css(output_logical, &artifact.css, settings.targets)
        .map_err(CliFailure::tool)?;
    let passed = outcome.passed();
    let document = FindingDocument::new(
        FindingTool::new("pliegocss", env!("CARGO_PKG_VERSION"))
            .map_err(|error| CliFailure::tool(error.to_string()))?,
        settings.operation,
        outcome.document().findings().to_vec(),
    )
    .map_err(|error| CliFailure::tool(error.to_string()))?;

    let cli_source = serde_json::to_vec(&CliInputIdentity {
        styles: settings.styles,
        compositions: settings.compositions,
    })
    .map_err(|error| CliFailure::tool(format!("cannot encode CLI source identity: {error}")))?;
    let registry = theme.registry();
    let theme_id = registry.id().to_string();
    let theme_selection = match settings.theme_request {
        ControlThemeRequest::ExplicitConfig => "explicit-config",
        ControlThemeRequest::ExplicitTokens => "explicit-tokens",
        ControlThemeRequest::ForcedSeed => "forced-seed",
        ControlThemeRequest::Automatic
            if snapshot
                .theme
                .as_ref()
                .map_err(|error| CliFailure::tool(error.clone()))?
                .is_some() =>
        {
            "discovered-config"
        }
        ControlThemeRequest::Automatic => "default-seed",
    };
    let manifest_version = if settings.physical_trace {
        5
    } else if snapshot.reachability.is_some() {
        4
    } else {
        3
    };
    let compiler_config = serde_json::to_vec(&CompilationConfigIdentity {
        operation: settings.operation,
        format: settings.format.as_str(),
        emit_theme: settings.emit_theme,
        manifest_version,
        prune_unreachable: settings.pruning.is_enabled(),
        theme_selection,
        theme_id: &theme_id,
        token_inputs: theme.selections(),
    })
    .map_err(|error| CliFailure::tool(format!("cannot encode compiler identity: {error}")))?;

    let mut source_snapshots = Vec::new();
    if !settings.styles.is_empty() || !settings.compositions.is_empty() {
        source_snapshots.push(ControlInputSnapshot {
            logical_path: "pliego.cli-input.json".to_owned(),
            role: "cli-source",
            bytes: &cli_source,
        });
    }
    if let Some(input) = &snapshot.input {
        source_snapshots.push(ControlInputSnapshot {
            logical_path: control_input_logical_path(&input.path)?,
            role: "line-input",
            bytes: input.bytes().map_err(CliFailure::tool)?,
        });
    }
    for source in snapshot
        .sources
        .as_ref()
        .map_err(|error| CliFailure::tool(error.clone()))?
    {
        source_snapshots.push(ControlInputSnapshot {
            logical_path: control_input_logical_path(&source.path)?,
            role: "rust-source",
            bytes: source.bytes().map_err(CliFailure::tool)?,
        });
    }
    let mut config_snapshots = vec![ControlInputSnapshot {
        logical_path: format!("pliego.{}.json", settings.operation),
        role: "compiler-config",
        bytes: &compiler_config,
    }];
    if let Some(config) = snapshot
        .theme
        .as_ref()
        .map_err(|error| CliFailure::tool(error.clone()))?
    {
        config_snapshots.push(ControlInputSnapshot {
            logical_path: control_input_logical_path(&config.path)?,
            role: if matches!(settings.theme_request, ControlThemeRequest::ExplicitTokens) {
                "token-resolver"
            } else {
                "theme-config"
            },
            bytes: config.bytes().map_err(CliFailure::tool)?,
        });
    }
    if let Some(reachability) = &snapshot.reachability {
        config_snapshots.push(ControlInputSnapshot {
            logical_path: control_input_logical_path(&reachability.path)?,
            role: "reachability",
            bytes: reachability.bytes().map_err(CliFailure::tool)?,
        });
    }
    let source_inputs = source_snapshots
        .iter()
        .map(|input| AuditSourceInput {
            logical_path: &input.logical_path,
            role: input.role,
            bytes: input.bytes,
        })
        .collect::<Vec<_>>();
    let config_inputs = config_snapshots
        .iter()
        .map(|input| AuditSourceInput {
            logical_path: &input.logical_path,
            role: input.role,
            bytes: input.bytes,
        })
        .collect::<Vec<_>>();
    let source_map = build_artifact_source_map(artifact, &source_inputs)?;
    let css_relationships = vec![manifest_logical.to_owned(), source_map_logical.to_owned()];
    let manifest_relationships = vec![output_logical.to_owned()];
    let source_map_relationships = vec![output_logical.to_owned()];
    let generated_outputs = [
        ControlOutputInput {
            file: output_logical,
            role: "generated-css",
            media_type: "text/css",
            bytes: artifact.css.as_bytes(),
            source_map: Some(ControlSourceMapInput {
                file: source_map_logical,
                bytes: &source_map,
            }),
            relationships: &css_relationships,
        },
        ControlOutputInput {
            file: manifest_logical,
            role: "style-manifest",
            media_type: "application/json",
            bytes: artifact.manifest.as_bytes(),
            source_map: None,
            relationships: &manifest_relationships,
        },
        ControlOutputInput {
            file: source_map_logical,
            role: "css-source-map",
            media_type: "application/json",
            bytes: &source_map,
            source_map: None,
            relationships: &source_map_relationships,
        },
    ];
    let output_relationships = vec![
        output_logical.to_owned(),
        manifest_logical.to_owned(),
        source_map_logical.to_owned(),
    ];
    let registry_graph;
    let registry_selections = BTreeMap::new();
    let (token_graph, selections) = match theme {
        LoadedTheme::Registry(_) => {
            registry_graph = TokenGraph::from_registry(registry);
            (&registry_graph, &registry_selections)
        }
        LoadedTheme::Dtcg(theme) => (theme.graph(), theme.selections()),
    };
    let token_measurements = build_token_graph_measurements(
        token_graph,
        selections,
        registry,
        &artifact.token_references,
    )
    .map_err(CliFailure::tool)?;
    let group = build_audit_control_group(AuditControlInput {
        source_inputs: &source_inputs,
        config_inputs: &config_inputs,
        generated_outputs: &generated_outputs,
        document: &document,
        passed,
        rule_metrics: outcome.inventory().map(CssBudgetInventory::file),
        rules_unavailable_reason: "generated CSS failed syntax ingestion",
        token_observation: TokenObservationInput::Measured(&token_measurements),
        token_graph: Some(token_graph),
        output_relationships: &output_relationships,
        asset_plan_verified: false,
        targets: settings.targets,
        budget_policy: None,
        budget_policy_path: None,
        budget_policy_bytes: None,
        budget_subjects: &[],
    })
    .map_err(CliFailure::tool)?;
    Ok((group, passed, source_map))
}

fn build_artifact_source_map(
    artifact: &CompiledArtifact,
    inputs: &[AuditSourceInput<'_>],
) -> Result<Vec<u8>, CliFailure> {
    let origins = artifact
        .styles
        .iter()
        .map(|style| {
            style
                .origins
                .iter()
                .map(|origin| SourceMapOrigin {
                    file: origin.file.as_deref(),
                    byte_start: origin.byte_start,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let styles = artifact
        .styles
        .iter()
        .zip(&origins)
        .map(|(style, origins)| SourceMapStyle {
            class_name: &style.class_name,
            origins,
        })
        .collect::<Vec<_>>();
    build_css_source_map(&artifact.css, &styles, inputs).map_err(CliFailure::tool)
}

fn check_controlled_compile_outputs(outputs: &[BundleOutputPayload]) -> Result<(), CliFailure> {
    let mut drift = Vec::new();
    for output in outputs {
        match fs::read(&output.destination) {
            Ok(actual) if actual == output.bytes => {}
            Ok(_) => drift.push(format!("`{}` differs", output.destination.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                drift.push(format!("`{}` is missing", output.destination.display()));
            }
            Err(error) => {
                return Err(CliFailure::tool(format!(
                    "cannot check controlled compile output `{}`: {error}",
                    output.destination.display()
                )));
            }
        }
    }
    if drift.is_empty() {
        Ok(())
    } else {
        Err(CliFailure::tool(format!(
            "controlled compile drift detected in {} artifact(s): {}",
            drift.len(),
            drift.join(", ")
        )))
    }
}

fn validate_resolved_theme_outputs(
    config: Option<&Path>,
    output: Option<&Path>,
    manifest: Option<&Path>,
) -> Result<(), String> {
    let Some(config) = config else {
        return Ok(());
    };
    let inputs = [("resolved theme configuration", config)];
    let mut outputs = Vec::new();
    if let Some(output) = output {
        outputs.push(("--output", output));
    }
    if let Some(manifest) = manifest {
        outputs.push(("--manifest", manifest));
    }
    validate_path_roles(&inputs, &outputs)
}

fn resolve_watch_theme_config(arguments: &WatchArgs) -> Result<Option<PathBuf>, String> {
    let hints = arguments
        .input
        .iter()
        .chain(arguments.sources.iter())
        .map(PathBuf::as_path);
    resolve_theme_config(arguments.config.as_deref(), arguments.seed, hints)
}

fn resolve_theme_config<'a>(
    explicit: Option<&Path>,
    seed: bool,
    hints: impl IntoIterator<Item = &'a Path>,
) -> Result<Option<PathBuf>, String> {
    if let Some(explicit) = explicit {
        return Ok(Some(explicit.to_path_buf()));
    }
    if seed {
        return Ok(None);
    }

    let mut candidates = BTreeMap::new();
    let current =
        env::current_dir().map_err(|error| format!("cannot inspect current directory: {error}"))?;
    let local = current.join("pliego.theme.toml");
    if local.is_file() {
        candidates.insert(path_key(&local)?, local);
    }
    for hint in hints {
        if let Some(candidate) = nearest_theme_config(hint)? {
            candidates.insert(path_key(&candidate)?, candidate);
        }
    }

    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates.into_values().next()),
        _ => Err(format!(
            "multiple `pliego.theme.toml` files match the supplied sources: {}; select one with `--config` or force the built-in registry with `--seed`",
            candidates
                .into_values()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn nearest_theme_config(hint: &Path) -> Result<Option<PathBuf>, String> {
    let absolute = if hint.is_absolute() {
        hint.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| format!("cannot resolve `{}`: {error}", hint.display()))?
            .join(hint)
    };
    let mut directory = if absolute.is_dir() {
        absolute
    } else {
        absolute.parent().map(Path::to_path_buf).unwrap_or(absolute)
    };
    loop {
        let candidate = directory.join("pliego.theme.toml");
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
        if directory.join("Cargo.toml").is_file() {
            return Ok(None);
        }
        let Some(parent) = directory.parent() else {
            return Ok(None);
        };
        directory = parent.to_path_buf();
    }
}

#[derive(Clone, Copy, Default)]
struct ArtifactGraphOptions<'a> {
    reachability: Option<&'a ReachabilityDocument>,
    selection: Option<&'a UsageSelection>,
    bundle_id: Option<&'a str>,
    theme_references: Option<&'a BTreeSet<TokenRef>>,
    physical_trace: bool,
    pruning: ReachabilityPruning,
}

fn compile_candidates(
    theme: &ThemeRegistry,
    candidates: &[Candidate],
    include_theme: bool,
    targets: TargetContract,
    format: CssFormat,
) -> Result<CompiledArtifact, CliFailure> {
    compile_candidates_with_manifest(
        theme,
        candidates,
        include_theme,
        targets,
        format,
        ArtifactGraphOptions::default(),
    )
}

fn compile_candidates_with_manifest(
    theme: &ThemeRegistry,
    candidates: &[Candidate],
    include_theme: bool,
    targets: TargetContract,
    format: CssFormat,
    graph: ArtifactGraphOptions<'_>,
) -> Result<CompiledArtifact, CliFailure> {
    let resolved = resolve_candidates(theme, candidates, 1)?;
    compile_resolved_candidates_with_manifest(
        theme,
        &resolved,
        include_theme,
        targets,
        format,
        graph,
        &mut CssCaches::default(),
    )
    .map_err(CliFailure::compilation)
}

fn resolve_candidates(
    theme: &ThemeRegistry,
    candidates: &[Candidate],
    first_index: usize,
) -> Result<Vec<ResolvedCandidate>, CliFailure> {
    candidates
        .iter()
        .enumerate()
        .map(|(offset, candidate)| {
            let index = first_index + offset;
            let semantic = match candidate {
                Candidate::Direct { style, provenance } => {
                    lower_source(theme, style, index, provenance)?
                }
                Candidate::Composition {
                    base,
                    branches,
                    provenance,
                } => {
                    let base_provenance = diagnostic_segment_provenance(provenance, "base");
                    let mut semantic = lower_source(theme, base, index, &base_provenance)?;
                    for (branch_index, branch) in branches.iter().enumerate() {
                        let branch_provenance = diagnostic_segment_provenance(
                            provenance,
                            &format!("branch-{}", branch_index + 1),
                        );
                        let branch = lower_source(theme, branch, index, &branch_provenance)?;
                        semantic = compose_style_override_with_theme(theme, semantic, branch);
                    }
                    semantic
                }
            };
            Ok(ResolvedCandidate {
                semantic,
                provenance: candidate.provenance().clone(),
            })
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn compile_resolved_candidates_with_manifest(
    theme: &ThemeRegistry,
    candidates: &[ResolvedCandidate],
    include_theme: bool,
    targets: TargetContract,
    format: CssFormat,
    graph: ArtifactGraphOptions<'_>,
    cache: &mut CssCaches,
) -> Result<CompiledArtifact, String> {
    if graph.physical_trace && graph.reachability.is_none() {
        return Err("physical CSS trace requires reachability".into());
    }
    match (graph.selection, graph.bundle_id) {
        (Some(_), Some(_)) if !graph.pruning.is_enabled() => {
            return Err("prepared usage selection requires reachability pruning".into());
        }
        (Some(_), Some(_)) | (None, None) => {}
        _ => {
            return Err(
                "prepared usage selection requires both selection and bundle identifier".into(),
            );
        }
    }
    let reachability_index = graph
        .pruning
        .is_enabled()
        .then(|| {
            graph
                .reachability
                .ok_or("reachability pruning requires reachability")
                .map(ReachabilityDocument::source_index)
        })
        .transpose()?;
    let mut reachable_streams = BTreeSet::new();
    let mut semantic_styles: BTreeMap<Vec<u8>, (SemanticStyle, Vec<Provenance>)> = BTreeMap::new();
    let mut streams_by_id = BTreeMap::new();
    for candidate in candidates {
        let identity_stream = try_encode_style_identity_with_theme(theme, &candidate.semantic)
            .map_err(|error| error.to_string())?;
        if let Some(index) = &reachability_index {
            if provenance_is_reachable(index, &candidate.provenance)? {
                reachable_streams.insert(identity_stream.clone());
            }
        }
        match streams_by_id.entry(candidate.semantic.id) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(identity_stream.clone());
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                if entry.get() != &identity_stream {
                    return Err(format!(
                        "StyleId collision under format {} for {:032x}; distinct canonical streams cannot share one CSS class",
                        STYLE_ID_FORMAT_VERSION,
                        candidate.semantic.id.get()
                    ));
                }
            }
        }
        match semantic_styles.entry(identity_stream) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert((
                    candidate.semantic.clone(),
                    vec![candidate.provenance.clone()],
                ));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if entry.get().0.id != candidate.semantic.id {
                    return Err(format!(
                        "canonical style stream under format {} mapped to both {:032x} and {:032x}",
                        STYLE_ID_FORMAT_VERSION,
                        entry.get().0.id.get(),
                        candidate.semantic.id.get()
                    ));
                }
                entry.get_mut().1.push(candidate.provenance.clone());
            }
        }
    }
    let expected_selected_styles = if let (Some(selection), Some(bundle_id)) =
        (graph.selection, graph.bundle_id)
    {
        let selected = streams_by_id
            .keys()
            .map(|style_id| format!("{:032x}", style_id.get()))
            .filter(|style_id| selection.contains(bundle_id, style_id))
            .collect::<BTreeSet<_>>();
        semantic_styles
            .retain(|_, (semantic, _)| selected.contains(&format!("{:032x}", semantic.id.get())));
        Some(selected)
    } else {
        None
    };
    if expected_selected_styles.is_none() && reachability_index.is_some() {
        semantic_styles.retain(|stream, _| reachable_streams.contains(stream));
    }
    enforce_compatibility_policy(targets, &semantic_styles)?;
    let token_references = collect_token_references(&semantic_styles);

    let mut output = String::new();
    let mut emits_theme = false;
    if include_theme {
        let theme_css = if let Some(references) = graph.theme_references {
            emit_theme_references(theme, references)
        } else if graph.pruning.is_enabled() {
            emit_used_theme(theme, semantic_styles.values().map(|(style, _)| style))
        } else {
            emit_theme(theme)
        };
        emits_theme = theme_css != ":root{}";
        output.push_str(&theme_css);
        output.push('\n');
    }
    let mut emission_lineage = Vec::new();
    for (stream, (semantic, _)) in &semantic_styles {
        if graph.physical_trace {
            let (css, lineage) =
                emit_css_with_theme_traced(theme, semantic).map_err(|error| error.to_string())?;
            emission_lineage.push(TraceStyle::new(
                lineage.style_id.get(),
                lineage
                    .rules
                    .into_iter()
                    .map(|rule| {
                        TraceRule::new(
                            rule.declarations
                                .into_iter()
                                .map(|declaration| {
                                    TraceDeclaration::new(
                                        declaration.semantic_ordinals,
                                        declaration.important,
                                        declaration.generated,
                                    )
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            ));
            output.push_str(&css);
        } else {
            let (css, _) = cache
                .0
                .emit(stream, theme, semantic)
                .map_err(|error| error.to_string())?;
            output.push_str(css);
        }
        output.push('\n');
    }
    cache.0.retain(semantic_styles.keys());
    let (mut css, trace_input) = if graph.physical_trace {
        let (css, trace_input) =
            optimize_css_with_trace(&output, targets.lightning(), format.is_minified())?;
        (css, Some(trace_input))
    } else {
        (
            cache
                .1
                .optimize(&output, targets.lightning(), format.is_minified())?,
            None,
        )
    };
    if !css.ends_with('\n') {
        css.push('\n');
    }
    let manifest_graph = if let Some(reachability) = graph.reachability {
        let graph_origins = semantic_styles
            .values()
            .map(|(_, origins)| {
                origins
                    .iter()
                    .map(|origin| {
                        GraphOrigin::new(origin.file.as_deref(), origin.byte_start, origin.byte_end)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let graph_styles = semantic_styles
            .values()
            .zip(&graph_origins)
            .map(|((style, _), origins)| ManifestGraphStyle::new(style, origins))
            .collect::<Vec<_>>();
        Some(if graph.physical_trace {
            let physical = build_physical_projection(
                trace_input
                    .as_deref()
                    .ok_or("physical trace input is missing")?,
                &css,
                targets.lightning(),
                format.is_minified(),
                include_theme,
                &emission_lineage,
            )?;
            build_manifest_graph_with_physical(theme, &graph_styles, reachability, physical)?
        } else {
            build_manifest_graph(theme, &graph_styles, reachability)?
        })
    } else {
        None
    };
    let schema_version = if graph.physical_trace {
        5
    } else if manifest_graph.is_some() {
        4
    } else {
        3
    };
    let styles = semantic_styles
        .into_iter()
        .map(|(_, (semantic, mut origins))| {
            normalize_provenance(&mut origins);
            ManifestStyle {
                style_id: format!("{:032x}", semantic.id.get()),
                class_name: semantic.id.to_class_name(),
                origins,
            }
        })
        .collect::<Vec<_>>();
    if let Some(expected) = expected_selected_styles {
        let actual = styles
            .iter()
            .map(|style| style.style_id.clone())
            .collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(
                "compiled manifest selection does not match prepared usage selection".into(),
            );
        }
    }
    let manifest = Manifest {
        schema_version,
        style_id_format_version: STYLE_ID_FORMAT_VERSION,
        class_name_format_version: CLASS_NAME_FORMAT_VERSION,
        theme_id_format_version: THEME_ID_FORMAT_VERSION,
        theme_id: theme.id().to_string(),
        targets,
        format,
        css_sha256: sha256_hex(css.as_bytes()),
        css_bytes: css.len(),
        styles: styles.clone(),
        graph: manifest_graph,
    };
    let mut manifest = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("cannot serialize manifest: {error}"))?;
    manifest.push('\n');
    let mut findings = candidates
        .iter()
        .map(|candidate| candidate.provenance.clone())
        .collect::<Vec<_>>();
    normalize_provenance(&mut findings);
    Ok(CompiledArtifact {
        css,
        manifest,
        format,
        emits_theme,
        styles,
        findings,
        token_references,
    })
}

fn collect_token_references(
    styles: &BTreeMap<Vec<u8>, (SemanticStyle, Vec<Provenance>)>,
) -> BTreeSet<FlatTokenReference> {
    referenced_tokens(styles.values().map(|(style, _)| style))
        .into_iter()
        .map(|reference| FlatTokenReference::new(reference.kind, reference.id))
        .collect()
}

fn lower_source(
    theme: &ThemeRegistry,
    source: &str,
    index: usize,
    provenance: &Provenance,
) -> Result<SemanticStyle, CliFailure> {
    let syntax = parse_style_list(source).map_err(|error| {
        let human = format_style_failure(index, source, &error.to_string());
        style_failure(error, human, provenance)
    })?;
    lower_style_with_theme(theme, &syntax).map_err(|error| {
        let human = format_style_failure(index, source, &error.to_string());
        style_failure(error, human, provenance)
    })
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<bool, String> {
    match fs::read(path) {
        Ok(current) if current == bytes => return Ok(false),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect `{}`: {error}", path.display())),
    }
    prepare_atomic_write(path, bytes)?.commit()?;
    Ok(true)
}

fn watch(arguments: &WatchArgs) -> Result<(), String> {
    validate_output_destination(&arguments.output, "output")?;
    if let Some(path) = arguments.manifest.as_deref() {
        validate_output_destination(path, "output")?;
    }
    if let Some(control_dir) = arguments.control_dir.as_deref() {
        let control_dir = existing_artifact_directory(control_dir, "control output")?;
        control_output_logical_path(&control_dir, &arguments.output)
            .map_err(|error| error.to_string())?;
        control_output_logical_path(
            &control_dir,
            arguments
                .manifest
                .as_deref()
                .expect("watch control requires a manifest"),
        )
        .map_err(|error| error.to_string())?;
    }
    let inputs = format_source_paths(
        arguments
            .input
            .iter()
            .map(PathBuf::as_path)
            .chain(arguments.sources.iter().map(PathBuf::as_path)),
    );
    let (wake, mode) = WatchScheduler::new(
        watch_event_paths(arguments),
        POLL_INTERVAL,
        EVENT_FALLBACK_INTERVAL,
    );
    eprintln!(
        "watching `{}` -> `{}` ({mode})",
        inputs,
        arguments.output.display()
    );
    let mut previous = None;
    let mut pending = None;
    let mut cache = RustScanCache::default();
    loop {
        let snapshot = capture_watch_snapshot(arguments);
        if !watch_snapshot_confirmed(previous.as_ref(), &mut pending, &snapshot) {
            // Native backends can queue both truncate and write notifications for one logical
            // save. Waiting on that queue again may return immediately and incorrectly confirm
            // the transient empty file as stable. Always give a changed snapshot one complete
            // debounce interval before recapturing it.
            std::thread::sleep(POLL_INTERVAL);
            continue;
        }
        let iteration =
            watch_iteration_from_snapshot(arguments, snapshot, previous.as_ref(), &mut cache);
        let snapshot = iteration.snapshot;
        if iteration.cache.has_sources() {
            eprintln!(
                "source cache: {} discovered, {} scan hits, {} parsed, {} semantic hits, {} lowered, {} removed",
                iteration.cache.discovered,
                iteration.cache.scan_hits,
                iteration.cache.scan_misses,
                iteration.cache.semantic_hits,
                iteration.cache.semantic_misses,
                iteration.cache.removed
            );
        }
        let mut wait = EVENT_FALLBACK_INTERVAL;
        match iteration.outcome {
            WatchOutcome::Unchanged => {
                pending = None;
                previous = Some(snapshot);
            }
            WatchOutcome::Compiled(artifact) => {
                match write_artifact(arguments, &snapshot, &artifact) {
                    Ok(published) => {
                        pending = None;
                        previous = Some(snapshot);
                        if published.any_changed() {
                            eprintln!("compiled `{}` -> `{}`", inputs, arguments.output.display());
                        } else {
                            eprintln!(
                                "validated `{inputs}`; generated artifact bytes are unchanged"
                            );
                        }
                        if published.control_result == ControlPublicationResult::Failed {
                            eprintln!("warning: generated control receipt records a failed audit");
                        }
                    }
                    Err(error) => {
                        wait = POLL_INTERVAL;
                        eprintln!(
                            "write failed; retrying without waiting for another input change: {error}"
                        );
                    }
                }
            }
            WatchOutcome::Failed(error) => {
                pending = None;
                previous = Some(snapshot);
                eprintln!("compile failed; keeping the last valid artifact: {error}");
            }
        }
        wake.wait(wait);
    }
}

fn watch_event_paths(arguments: &WatchArgs) -> Vec<PathBuf> {
    arguments
        .input
        .iter()
        .chain(&arguments.sources)
        .chain(arguments.tokens.iter())
        .chain(arguments.reachability.iter())
        .cloned()
        .chain(
            (arguments.tokens.is_none())
                .then(|| resolve_watch_theme_config(arguments).ok().flatten())
                .flatten(),
        )
        .collect()
}

fn watch_iteration_from_snapshot(
    arguments: &WatchArgs,
    snapshot: WatchSnapshot,
    previous: Option<&WatchSnapshot>,
    cache: &mut RustScanCache,
) -> WatchIteration {
    if previous == Some(&snapshot) {
        return WatchIteration {
            snapshot,
            cache: SourceCacheStats::default(),
            outcome: WatchOutcome::Unchanged,
        };
    }
    let mut cache_stats = SourceCacheStats::default();
    let outcome = compile_watch_snapshot(arguments, &snapshot, cache, &mut cache_stats);
    WatchIteration {
        snapshot,
        cache: cache_stats,
        outcome: match outcome {
            Ok(artifact) => WatchOutcome::Compiled(artifact),
            Err(error) => WatchOutcome::Failed(error),
        },
    }
}

fn compile_watch_snapshot(
    arguments: &WatchArgs,
    snapshot: &WatchSnapshot,
    cache: &mut RustScanCache,
    cache_stats: &mut SourceCacheStats,
) -> Result<CompiledArtifact, String> {
    let inputs = snapshot
        .input
        .iter()
        .chain(snapshot.theme.as_ref().ok().and_then(Option::as_ref))
        .chain(snapshot.reachability.iter())
        .map(|input| ("watched input", input.path.as_path()))
        .collect::<Vec<_>>();
    let outputs = [
        Some(arguments.output.as_path()),
        arguments.manifest.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(|path| ("output", path))
    .collect::<Vec<_>>();
    validate_path_roles(&inputs, &outputs)?;
    let theme = load_watch_theme_snapshot(arguments, snapshot)?;
    let registry = theme.registry();
    let reachability = snapshot
        .reachability
        .as_ref()
        .map(|file| parse_reachability_document(file.bytes()?))
        .transpose()?;
    let mut resolved = Vec::new();
    if let Some(input) = &snapshot.input {
        let candidates = if arguments.control_dir.is_some() {
            let logical =
                control_input_logical_path(&input.path).map_err(|error| error.to_string())?;
            candidates_from_line_source(Path::new(&logical), input.utf8("watched input")?)
        } else {
            candidates_from_line_source(&input.path, input.utf8("watched input")?)
        }
        .map_err(|error| error.human)?;
        resolved.extend(resolve_candidates(registry, &candidates, 1).map_err(|error| error.human)?);
    }
    let first_source_index = resolved.len() + 1;
    resolved.extend(cache.resolved_from_snapshot(
        registry,
        &snapshot.sources,
        cache_stats,
        first_source_index,
        arguments.control_dir.is_some(),
    )?);
    if resolved.is_empty() {
        return Err("no styles found in watched inputs".into());
    }
    compile_resolved_candidates_with_manifest(
        registry,
        &resolved,
        arguments.theme,
        arguments.targets,
        arguments.format,
        ArtifactGraphOptions {
            reachability: reachability.as_ref(),
            physical_trace: arguments.physical_trace,
            pruning: arguments.pruning,
            ..ArtifactGraphOptions::default()
        },
        &mut cache.css,
    )
}

fn capture_watch_snapshot(arguments: &WatchArgs) -> WatchSnapshot {
    WatchSnapshot {
        input: arguments
            .input
            .as_ref()
            .map(|path| snapshot_file(path, "watched input")),
        sources: expand_source_paths(&arguments.sources).map(|paths| {
            paths
                .into_iter()
                .map(|path| snapshot_file(&path, "Rust source"))
                .collect()
        }),
        theme: arguments.tokens.as_ref().map_or_else(
            || {
                resolve_watch_theme_config(arguments)
                    .map(|config| config.map(|path| snapshot_file(&path, "theme configuration")))
            },
            |path| Ok(Some(snapshot_token_file(path))),
        ),
        reachability: arguments
            .reachability
            .as_ref()
            .map(|path| WatchFileSnapshot {
                path: path.clone(),
                contents: read_reachability(path),
            }),
    }
}

fn snapshot_token_file(path: &Path) -> WatchFileSnapshot {
    WatchFileSnapshot {
        path: path.to_path_buf(),
        contents: read_bounded_utf8_document(path, "token resolver"),
    }
}

fn load_watch_theme_snapshot(
    arguments: &WatchArgs,
    snapshot: &WatchSnapshot,
) -> Result<LoadedTheme, String> {
    let config = snapshot.theme.as_ref().map_err(Clone::clone)?;
    validate_resolved_theme_outputs(
        config.as_ref().map(|file| file.path.as_path()),
        Some(&arguments.output),
        arguments.manifest.as_deref(),
    )?;
    if arguments.tokens.is_some() {
        let file = config
            .as_ref()
            .expect("explicit token resolver must have a snapshot");
        return resolve_dtcg_theme(file.utf8("token resolver")?, &arguments.token_inputs)
            .map(LoadedTheme::Dtcg);
    }
    config.as_ref().map_or_else(
        || Ok(LoadedTheme::Registry(ThemeRegistry::seed())),
        |file| {
            pliego_css_config::parse_str(file.utf8("theme configuration")?)
                .map(LoadedTheme::Registry)
                .map_err(|error| error.to_string())
        },
    )
}

impl RustScanCache {
    fn resolved_from_snapshot(
        &mut self,
        theme: &ThemeRegistry,
        sources: &Result<Vec<WatchFileSnapshot>, String>,
        stats: &mut SourceCacheStats,
        first_index: usize,
        portable_sources: bool,
    ) -> Result<Vec<ResolvedCandidate>, String> {
        let sources = sources.as_ref().map_err(Clone::clone)?;
        stats.discovered = sources.len();
        let theme_id = theme.id().get();

        let mut active = BTreeSet::new();
        for source in sources {
            active.insert(path_key(&source.path)?);
        }
        let before = self.entries.len();
        self.entries.retain(|key, _| active.contains(key));
        stats.removed = before - self.entries.len();

        let mut resolved = Vec::new();
        for source in sources {
            let key = path_key(&source.path)?;
            let source_name = if portable_sources {
                control_input_logical_path(&source.path).map_err(|error| error.to_string())?
            } else {
                source.path.display().to_string()
            };
            let bytes = source.bytes()?;
            let cache_hit = self.entries.get(&key).is_some_and(|entry| {
                entry.source_name == source_name && entry.contents.as_slice() == bytes
            });
            if cache_hit {
                stats.scan_hits += 1;
            } else {
                stats.scan_misses += 1;
                let report = std::str::from_utf8(bytes)
                    .map_err(|error| {
                        format!(
                            "Rust source `{}` is not valid UTF-8: {error}",
                            source.path.display()
                        )
                    })
                    .and_then(|text| {
                        scan_source_named(source_name.clone(), text)
                            .map_err(|error| error.to_string())
                    });
                self.entries.insert(
                    key.clone(),
                    CachedRustScan {
                        source_name,
                        contents: bytes.to_vec(),
                        report,
                        semantics: None,
                    },
                );
            }
            let cached = self
                .entries
                .get(&key)
                .and_then(|entry| entry.semantics.as_ref())
                .filter(|semantics| semantics.theme_id == theme_id)
                .map(|semantics| semantics.resolved.clone());
            let source_resolved = if let Some(cached) = cached {
                stats.semantic_hits += 1;
                cached
            } else {
                stats.semantic_misses += 1;
                let report = self
                    .entries
                    .get(&key)
                    .expect("active source must have a cache entry")
                    .report
                    .as_ref()
                    .map_err(Clone::clone)?;
                let candidates =
                    candidates_from_scan_report(theme, report).map_err(|error| error.human)?;
                let source_resolved =
                    resolve_candidates(theme, &candidates, first_index + resolved.len())
                        .map_err(|error| error.human)?;
                self.entries
                    .get_mut(&key)
                    .expect("active source must have a cache entry")
                    .semantics = Some(CachedRustSemantics {
                    theme_id,
                    resolved: source_resolved.clone(),
                });
                source_resolved
            };
            resolved.extend(source_resolved);
        }
        Ok(resolved)
    }
}

fn write_artifact(
    arguments: &WatchArgs,
    snapshot: &WatchSnapshot,
    artifact: &CompiledArtifact,
) -> Result<PublishedOutputs, String> {
    if let Some(control_dir) = arguments.control_dir.as_deref() {
        return write_controlled_watch_artifact(arguments, snapshot, artifact, control_dir);
    }
    write_outputs(
        Some(&arguments.output),
        arguments.manifest.as_deref(),
        artifact,
    )
}

fn write_controlled_watch_artifact(
    arguments: &WatchArgs,
    snapshot: &WatchSnapshot,
    artifact: &CompiledArtifact,
    control_dir: &Path,
) -> Result<PublishedOutputs, String> {
    let control_dir = existing_artifact_directory(control_dir, "control output")?;
    let manifest = arguments
        .manifest
        .as_deref()
        .expect("watch control requires a manifest");
    let output_logical = control_output_logical_path(&control_dir, &arguments.output)
        .map_err(|error| error.to_string())?;
    let manifest_logical =
        control_output_logical_path(&control_dir, manifest).map_err(|error| error.to_string())?;
    validate_control_snapshot_paths(snapshot, &arguments.output, manifest, &control_dir)
        .map_err(|error| error.to_string())?;
    let theme = load_watch_theme_snapshot(arguments, snapshot)?;
    let source_map = css_source_map_path(&arguments.output);
    let source_map_logical = control_output_logical_path(&control_dir, &source_map)
        .map_err(|error| error.to_string())?;
    let (group, passed, source_map_bytes) = build_compilation_control_group(
        CompilationControlSettings {
            operation: "watch",
            styles: &[],
            compositions: &[],
            theme_request: ControlThemeRequest::from_arguments(
                arguments.tokens.is_some(),
                arguments.config.is_some(),
                arguments.seed,
            ),
            emit_theme: arguments.theme,
            targets: arguments.targets,
            format: arguments.format,
            physical_trace: arguments.physical_trace,
            pruning: arguments.pruning,
        },
        snapshot,
        &theme,
        artifact,
        &output_logical,
        &manifest_logical,
        &source_map_logical,
    )
    .map_err(|error| error.to_string())?;
    let mut payloads = vec![
        BundleOutputPayload {
            destination: arguments.output.clone(),
            bytes: artifact.css.as_bytes().to_vec(),
        },
        BundleOutputPayload {
            destination: manifest.to_owned(),
            bytes: artifact.manifest.as_bytes().to_vec(),
        },
        BundleOutputPayload {
            destination: source_map,
            bytes: source_map_bytes,
        },
    ];
    payloads.extend(group.artifacts().map(|artifact| BundleOutputPayload {
        destination: control_dir.join(artifact.file),
        bytes: artifact.bytes.to_vec(),
    }));
    let css_changed = output_would_change(&payloads[0])?;
    let manifest_changed = output_would_change(&payloads[1])?;
    let control_changed = payloads[2..]
        .iter()
        .map(output_would_change)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .any(|changed| changed);
    publish_output_group(&payloads)?;
    Ok(PublishedOutputs {
        css_changed,
        manifest_changed,
        control_changed,
        control_result: if passed {
            ControlPublicationResult::Passed
        } else {
            ControlPublicationResult::Failed
        },
    })
}

fn output_would_change(output: &BundleOutputPayload) -> Result<bool, String> {
    match fs::read(&output.destination) {
        Ok(current) => Ok(current != output.bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(format!(
            "cannot inspect controlled output `{}`: {error}",
            output.destination.display()
        )),
    }
}

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PublishedOutputs {
    css_changed: bool,
    manifest_changed: bool,
    control_changed: bool,
    control_result: ControlPublicationResult,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ControlPublicationResult {
    #[default]
    NotRequested,
    Passed,
    Failed,
}

impl PublishedOutputs {
    const fn any_changed(self) -> bool {
        self.css_changed || self.manifest_changed || self.control_changed
    }
}

fn write_outputs(
    output: Option<&Path>,
    manifest: Option<&Path>,
    artifact: &CompiledArtifact,
) -> Result<PublishedOutputs, String> {
    for path in output.into_iter().chain(manifest) {
        validate_output_destination(path, "output destination")?;
    }
    let destinations = publication_destinations(output.into_iter().chain(manifest))?;
    let _locks = acquire_publication_locks(&destinations)?;
    for path in output.into_iter().chain(manifest) {
        validate_output_destination(path, "output destination")?;
    }
    let css = output
        .map(|path| prepare_atomic_write_if_changed(path, artifact.css.as_bytes()))
        .transpose()?
        .flatten();
    let manifest = manifest
        .map(|path| prepare_atomic_write_if_changed(path, artifact.manifest.as_bytes()))
        .transpose()?
        .flatten();

    let published = PublishedOutputs {
        css_changed: css.is_some(),
        manifest_changed: manifest.is_some(),
        control_changed: false,
        control_result: ControlPublicationResult::NotRequested,
    };

    let writes = css.into_iter().chain(manifest).collect();
    commit_prepared_locked_with(writes, |_, temporary, destination| {
        fs::rename(temporary, destination)
    })?;
    Ok(published)
}

fn publish_output_group(outputs: &[BundleOutputPayload]) -> Result<usize, String> {
    publish_output_group_with(outputs, |_, temporary, destination| {
        fs::rename(temporary, destination)
    })
}

fn check_output_group(outputs: &[BundleOutputPayload]) -> Result<(), String> {
    for output in outputs {
        validate_bundle_output_destination(&output.destination)?;
        let actual = fs::read(&output.destination).map_err(|error| {
            format!(
                "cannot check control artifact `{}`: {error}",
                output.destination.display()
            )
        })?;
        if actual != output.bytes {
            return Err(format!(
                "control artifact drift detected in `{}`",
                output.destination.display()
            ));
        }
    }
    Ok(())
}

fn publish_audit_control_group(
    group: &AuditControlGroup,
    output_dir: &Path,
    check: bool,
    inputs: &[(&str, &Path)],
) -> Result<(), CliFailure> {
    let output_dir =
        existing_artifact_directory(output_dir, "control output").map_err(CliFailure::invalid)?;
    let payloads = group
        .artifacts()
        .map(|artifact| BundleOutputPayload {
            destination: output_dir.join(artifact.file),
            bytes: artifact.bytes.to_vec(),
        })
        .collect::<Vec<_>>();
    validate_path_roles(
        inputs,
        &payloads
            .iter()
            .map(|payload| ("--control-dir", payload.destination.as_path()))
            .collect::<Vec<_>>(),
    )
    .map_err(CliFailure::invalid)?;
    if check {
        check_output_group(&payloads).map_err(CliFailure::tool)
    } else {
        publish_output_group(&payloads)
            .map(|_| ())
            .map_err(CliFailure::tool)
    }
}

fn publish_output_group_with(
    outputs: &[BundleOutputPayload],
    publish: impl FnMut(usize, &Path, &Path) -> std::io::Result<()>,
) -> Result<usize, String> {
    for output in outputs {
        validate_bundle_output_destination(&output.destination)?;
    }
    let destinations =
        publication_destinations(outputs.iter().map(|output| output.destination.as_path()))?;
    let _locks = acquire_publication_locks(&destinations)?;
    for output in outputs {
        validate_bundle_output_destination(&output.destination)?;
    }
    let writes = outputs
        .iter()
        .map(|output| prepare_atomic_write_if_changed(&output.destination, &output.bytes))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let changed = writes.len();
    commit_prepared_locked_with(writes, publish)?;
    Ok(changed)
}

#[cfg(all(test, not(feature = "package-verify")))]
#[path = "../tests/internal/unit.rs"]
mod tests;
