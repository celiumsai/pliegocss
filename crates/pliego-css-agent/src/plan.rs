use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use pliego_css_build::artifacts::{
    Finding, FindingDocument, FindingRisk, FindingSuggestion, FindingVerification,
    parse_finding_document, sha256_hex,
};

use super::validation::{
    validate_finding_code, validate_fingerprint, validate_logical_path, validate_sha256,
    validate_slug,
};
use super::{
    MAX_DOCUMENT_BYTES, MAX_EDITS, MAX_FILES, MAX_REPLACEMENT_BYTES, MAX_SOURCE_BYTES,
    REPAIR_PLAN_SCHEMA_VERSION, RepairArtifactIdentity, RepairChangeBudget, RepairContractError,
    RepairProposalDocument, RepairProposalEdit, RepairSourceTransition, RepairTool,
    apply_file_edits, build_patch, enforce_budget, summarize_edits, validate_checks,
    validate_planned_edit_order, validate_source_order,
};

/// Exact edit after it has been reconciled with a source snapshot and finding suggestion.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairPlannedEdit {
    pub(crate) finding_fingerprint: String,
    pub(crate) finding_code: String,
    pub(crate) suggestion_id: String,
    pub(crate) suggestion_rank: u16,
    pub(crate) suggestion_scope: String,
    pub(crate) file: String,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) removed: String,
    pub(crate) removed_sha256: String,
    pub(crate) replacement: String,
    pub(crate) replacement_sha256: String,
}

impl RepairPlannedEdit {
    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
        validate_fingerprint("edit.findingFingerprint", &self.finding_fingerprint)?;
        validate_finding_code(&self.finding_code)?;
        validate_slug("edit.suggestionId", &self.suggestion_id)?;
        if self.suggestion_rank == 0 {
            return Err(RepairContractError::new(
                "edit.suggestionRank must be one-based",
            ));
        }
        validate_slug("edit.suggestionScope", &self.suggestion_scope)?;
        validate_logical_path("edit.file", &self.file)?;
        if self.byte_end < self.byte_start {
            return Err(RepairContractError::new(
                "edit.byteEnd precedes edit.byteStart",
            ));
        }
        if self.removed.len() != self.byte_end - self.byte_start {
            return Err(RepairContractError::new(
                "edit.removed UTF-8 bytes do not match its source range",
            ));
        }
        if self.removed.len() > MAX_SOURCE_BYTES || self.replacement.len() > MAX_REPLACEMENT_BYTES {
            return Err(RepairContractError::new(
                "planned edit exceeds defensive byte limits",
            ));
        }
        if self.removed.contains('\0') || self.replacement.contains('\0') {
            return Err(RepairContractError::new("planned edit contains a NUL byte"));
        }
        if sha256_hex(self.removed.as_bytes()) != self.removed_sha256 {
            return Err(RepairContractError::new(
                "edit.removedSha256 does not match edit.removed",
            ));
        }
        if sha256_hex(self.replacement.as_bytes()) != self.replacement_sha256 {
            return Err(RepairContractError::new(
                "edit.replacementSha256 does not match edit.replacement",
            ));
        }
        Ok(())
    }
}

/// Derived changed-file and byte counts checked against the authorized budget.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairChangeSummary {
    pub(crate) files: u32,
    pub(crate) edits: u32,
    pub(crate) inserted_bytes: u64,
    pub(crate) removed_bytes: u64,
}

/// Exact deterministic patch projection over planned UTF-8 byte edits.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairPatch {
    pub(crate) format: String,
    pub(crate) sha256: String,
    pub(crate) text: String,
}

impl RepairPatch {
    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
        if self.format != "pliegocss-byte-edits/1" {
            return Err(RepairContractError::new(
                "patch.format must be `pliegocss-byte-edits/1`",
            ));
        }
        validate_sha256("patch.sha256", &self.sha256)?;
        if self.text.is_empty() || self.text.len() > MAX_DOCUMENT_BYTES {
            return Err(RepairContractError::new(
                "patch.text must be non-empty and at most 16 MiB",
            ));
        }
        if sha256_hex(self.text.as_bytes()) != self.sha256 {
            return Err(RepairContractError::new(
                "patch.sha256 does not match patch.text",
            ));
        }
        Ok(())
    }
}

/// Closed, integrity-bound, dry-run-only change plan.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepairPlanDocument {
    pub(crate) schema_version: String,
    pub(crate) plan_sha256: String,
    pub(crate) mode: String,
    pub(crate) tool: RepairTool,
    pub(crate) finding_document: RepairArtifactIdentity,
    pub(crate) change_budget: RepairChangeBudget,
    pub(crate) change_summary: RepairChangeSummary,
    pub(crate) risk: FindingRisk,
    pub(crate) required_checks: Vec<String>,
    pub(crate) sources: Vec<RepairSourceTransition>,
    pub(crate) edits: Vec<RepairPlannedEdit>,
    pub(crate) patch: RepairPatch,
}

impl RepairPlanDocument {
    /// Returns the canonical plan payload hash.
    #[must_use]
    pub fn plan_sha256(&self) -> &str {
        &self.plan_sha256
    }

    /// Returns source transitions in canonical path order.
    #[must_use]
    pub fn sources(&self) -> &[RepairSourceTransition] {
        &self.sources
    }

    /// Serializes the validated plan with one trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`RepairContractError`] when validation or serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, RepairContractError> {
        self.validate()?;
        let mut output = serde_json::to_string_pretty(self)
            .map_err(|error| RepairContractError::new(format!("cannot serialize plan: {error}")))?;
        output.push('\n');
        if output.len() > MAX_DOCUMENT_BYTES {
            return Err(RepairContractError::new("repair plan exceeds 16 MiB"));
        }
        Ok(output)
    }

    /// Renders the bounded summary plus exact byte-edit patch.
    ///
    /// # Errors
    ///
    /// Returns [`RepairContractError`] when the plan is invalid.
    pub fn to_human(&self) -> Result<String, RepairContractError> {
        self.validate()?;
        Ok(format!(
            "Plan: sha256:{}\nMode: {}\nRisk: {}\nChanges: {} file(s), {} edit(s), {} inserted byte(s), {} removed byte(s)\nRequired checks: {}\n{}",
            self.plan_sha256,
            self.mode,
            self.risk.as_str(),
            self.change_summary.files,
            self.change_summary.edits,
            self.change_summary.inserted_bytes,
            self.change_summary.removed_bytes,
            self.required_checks.join(", "),
            self.patch.text
        ))
    }

    pub(crate) fn validate(&self) -> Result<(), RepairContractError> {
        if self.schema_version != REPAIR_PLAN_SCHEMA_VERSION {
            return Err(RepairContractError::new(format!(
                "unsupported repair plan schema `{}`; expected {REPAIR_PLAN_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        validate_sha256("plan.planSha256", &self.plan_sha256)?;
        if self.mode != "dry-run-only" {
            return Err(RepairContractError::new(
                "plan.mode must be `dry-run-only` in schema 1.0.0",
            ));
        }
        self.tool.validate()?;
        self.finding_document.validate("findingDocument")?;
        self.change_budget.validate()?;
        if self.risk != FindingRisk::Low {
            return Err(RepairContractError::new(
                "schema 1.0.0 accepts only low-risk repair plans",
            ));
        }
        validate_checks(&self.required_checks)?;
        if self.sources.is_empty() || self.sources.len() > MAX_FILES {
            return Err(RepairContractError::new(
                "plan.sources must contain between 1 and 65,535 files",
            ));
        }
        for source in &self.sources {
            source.validate()?;
        }
        validate_source_order(&self.sources)?;
        if self.edits.is_empty() || self.edits.len() > MAX_EDITS {
            return Err(RepairContractError::new(
                "plan.edits must contain between 1 and 65,535 edits",
            ));
        }
        for edit in &self.edits {
            edit.validate()?;
        }
        validate_planned_edit_order(&self.edits)?;
        let summary = summarize_edits(&self.edits)?;
        if summary != self.change_summary {
            return Err(RepairContractError::new(
                "plan.changeSummary does not match plan.edits",
            ));
        }
        enforce_budget(summary, self.change_budget)?;
        if usize::try_from(summary.files).ok() != Some(self.sources.len()) {
            return Err(RepairContractError::new(
                "plan.sources do not exactly cover edited files",
            ));
        }
        let source_files = self
            .sources
            .iter()
            .map(|source| source.file.as_str())
            .collect::<BTreeSet<_>>();
        if self
            .edits
            .iter()
            .any(|edit| !source_files.contains(edit.file.as_str()))
        {
            return Err(RepairContractError::new(
                "plan edit references a file absent from plan.sources",
            ));
        }
        let expected_patch = build_patch(&self.edits)?;
        if expected_patch != self.patch {
            return Err(RepairContractError::new(
                "plan.patch does not match plan.edits",
            ));
        }
        self.patch.validate()?;
        let expected_hash = plan_payload_sha256(self)?;
        if expected_hash != self.plan_sha256 {
            return Err(RepairContractError::new(format!(
                "plan.planSha256 `{}` does not match `{expected_hash}`",
                self.plan_sha256
            )));
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RepairPlanPayload<'a> {
    schema_version: &'a str,
    mode: &'a str,
    tool: &'a RepairTool,
    finding_document: &'a RepairArtifactIdentity,
    pub(crate) change_budget: RepairChangeBudget,
    pub(crate) change_summary: RepairChangeSummary,
    pub(crate) risk: FindingRisk,
    required_checks: &'a [String],
    sources: &'a [RepairSourceTransition],
    edits: &'a [RepairPlannedEdit],
    patch: &'a RepairPatch,
}

fn plan_payload_sha256(plan: &RepairPlanDocument) -> Result<String, RepairContractError> {
    let bytes = serde_json::to_vec(&RepairPlanPayload {
        schema_version: &plan.schema_version,
        mode: &plan.mode,
        tool: &plan.tool,
        finding_document: &plan.finding_document,
        change_budget: plan.change_budget,
        change_summary: plan.change_summary,
        risk: plan.risk,
        required_checks: &plan.required_checks,
        sources: &plan.sources,
        edits: &plan.edits,
        patch: &plan.patch,
    })
    .map_err(|error| RepairContractError::new(format!("cannot hash repair plan: {error}")))?;
    Ok(sha256_hex(&bytes))
}

/// Parses and validates a canonical repair-plan document.
///
/// # Errors
///
/// Returns [`RepairContractError`] for malformed JSON, an unsupported schema, hash drift,
/// noncanonical collections, invalid edits, summaries, budgets, or patch bytes.
pub fn parse_repair_plan(source: &[u8]) -> Result<RepairPlanDocument, RepairContractError> {
    if source.len() > MAX_DOCUMENT_BYTES {
        return Err(RepairContractError::new("repair plan exceeds 16 MiB"));
    }
    let plan: RepairPlanDocument = serde_json::from_slice(source)
        .map_err(|error| RepairContractError::new(format!("invalid repair plan JSON: {error}")))?;
    plan.validate()?;
    Ok(plan)
}

/// Builds a canonical dry-run-only repair plan from exact finding, proposal, and source bytes.
///
/// # Errors
///
/// Returns [`RepairContractError`] unless every edit references a verified, unexcepted, low-risk
/// suggestion, stays inside its finding range, matches the source snapshot, remains non-overlapping,
/// and fits the declared change budget.
pub fn build_repair_plan(
    tool: RepairTool,
    finding_file: &str,
    finding_bytes: &[u8],
    proposal: &RepairProposalDocument,
    sources: &BTreeMap<String, Vec<u8>>,
) -> Result<RepairPlanDocument, RepairContractError> {
    proposal.validate()?;
    validate_logical_path("findingDocument.file", finding_file)?;
    if finding_bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(RepairContractError::new("finding document exceeds 16 MiB"));
    }
    let finding_sha256 = sha256_hex(finding_bytes);
    if finding_sha256 != proposal.finding_document_sha256 {
        return Err(RepairContractError::new(format!(
            "proposal finding-document hash `{}` does not match `{finding_sha256}`",
            proposal.finding_document_sha256
        )));
    }
    let finding_document = parse_finding_document(finding_bytes)
        .map_err(|error| RepairContractError::new(format!("invalid finding document: {error}")))?;
    let proposed_files = proposal.files();
    if proposed_files.len() != sources.len() {
        return Err(RepairContractError::new(
            "source snapshot set must exactly equal proposal edit files",
        ));
    }
    let planned_edits = reconcile_edits(&finding_document, proposal, sources)?;
    validate_planned_edit_order(&planned_edits)?;
    let summary = summarize_edits(&planned_edits)?;
    enforce_budget(summary, proposal.change_budget)?;
    let transitions = build_source_transitions(&proposed_files, sources, &planned_edits)?;
    let patch = build_patch(&planned_edits)?;
    let mut plan = RepairPlanDocument {
        schema_version: REPAIR_PLAN_SCHEMA_VERSION.into(),
        plan_sha256: String::new(),
        mode: "dry-run-only".into(),
        tool,
        finding_document: RepairArtifactIdentity {
            file: finding_file.into(),
            bytes: finding_bytes.len(),
            sha256: finding_sha256,
        },
        change_budget: proposal.change_budget,
        change_summary: summary,
        risk: FindingRisk::Low,
        required_checks: proposal.required_checks.clone(),
        sources: transitions,
        edits: planned_edits,
        patch,
    };
    plan.plan_sha256 = plan_payload_sha256(&plan)?;
    plan.validate()?;
    Ok(plan)
}

fn reconcile_edits(
    finding_document: &FindingDocument,
    proposal: &RepairProposalDocument,
    sources: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<RepairPlannedEdit>, RepairContractError> {
    let findings = finding_document
        .findings()
        .iter()
        .map(|finding| (finding.fingerprint(), finding))
        .collect::<BTreeMap<_, _>>();
    let mut planned_edits = Vec::with_capacity(proposal.edits.len());
    for edit in &proposal.edits {
        let finding = findings
            .get(edit.finding_fingerprint.as_str())
            .ok_or_else(|| {
                RepairContractError::new(format!(
                    "edit references unknown finding `{}`",
                    edit.finding_fingerprint
                ))
            })?;
        let suggestion = resolve_suggestion(finding, &edit.suggestion_id)?;
        validate_edit_authority(finding, suggestion, edit, &proposal.required_checks)?;
        let source = sources.get(&edit.file).ok_or_else(|| {
            RepairContractError::new(format!("missing source snapshot `{}`", edit.file))
        })?;
        if source.len() > MAX_SOURCE_BYTES {
            return Err(RepairContractError::new(format!(
                "source snapshot `{}` exceeds 16 MiB",
                edit.file
            )));
        }
        let source_text = std::str::from_utf8(source).map_err(|error| {
            RepairContractError::new(format!(
                "source snapshot `{}` is not UTF-8: {error}",
                edit.file
            ))
        })?;
        if source_text.contains('\0') {
            return Err(RepairContractError::new(format!(
                "source snapshot `{}` contains a NUL byte",
                edit.file
            )));
        }
        let removed = source_text
            .get(edit.byte_start..edit.byte_end)
            .ok_or_else(|| {
                RepairContractError::new(format!(
                    "edit range {}..{} is not on UTF-8 boundaries inside `{}`",
                    edit.byte_start, edit.byte_end, edit.file
                ))
            })?;
        planned_edits.push(RepairPlannedEdit {
            finding_fingerprint: edit.finding_fingerprint.clone(),
            finding_code: finding.code().to_owned(),
            suggestion_id: suggestion.id().to_owned(),
            suggestion_rank: suggestion.rank(),
            suggestion_scope: suggestion.scope().to_owned(),
            file: edit.file.clone(),
            byte_start: edit.byte_start,
            byte_end: edit.byte_end,
            removed: removed.to_owned(),
            removed_sha256: sha256_hex(removed.as_bytes()),
            replacement: edit.replacement.clone(),
            replacement_sha256: sha256_hex(edit.replacement.as_bytes()),
        });
    }
    Ok(planned_edits)
}

fn build_source_transitions(
    proposed_files: &[&str],
    sources: &BTreeMap<String, Vec<u8>>,
    planned_edits: &[RepairPlannedEdit],
) -> Result<Vec<RepairSourceTransition>, RepairContractError> {
    let mut transitions = Vec::with_capacity(sources.len());
    for (file, source) in sources {
        if !proposed_files.contains(&file.as_str()) {
            return Err(RepairContractError::new(format!(
                "unexpected source snapshot `{file}`"
            )));
        }
        let after = apply_file_edits(file, source, planned_edits)?;
        transitions.push(RepairSourceTransition {
            file: file.clone(),
            before_bytes: source.len(),
            before_sha256: sha256_hex(source),
            after_bytes: after.len(),
            after_sha256: sha256_hex(&after),
        });
    }
    Ok(transitions)
}

fn resolve_suggestion<'a>(
    finding: &'a Finding,
    suggestion_id: &str,
) -> Result<&'a FindingSuggestion, RepairContractError> {
    finding
        .suggestions()
        .iter()
        .find(|suggestion| suggestion.id() == suggestion_id)
        .ok_or_else(|| {
            RepairContractError::new(format!(
                "finding `{}` has no suggestion `{suggestion_id}`",
                finding.fingerprint()
            ))
        })
}

fn validate_edit_authority(
    finding: &Finding,
    suggestion: &FindingSuggestion,
    edit: &RepairProposalEdit,
    required_checks: &[String],
) -> Result<(), RepairContractError> {
    if finding.verification() != FindingVerification::Verified {
        return Err(RepairContractError::new(format!(
            "finding `{}` is not verified",
            finding.fingerprint()
        )));
    }
    if finding.is_excepted() {
        return Err(RepairContractError::new(format!(
            "finding `{}` has a reviewed exception",
            finding.fingerprint()
        )));
    }
    if suggestion.risk() != FindingRisk::Low {
        return Err(RepairContractError::new(format!(
            "suggestion `{}` is {}; schema 1.0.0 accepts only low-risk edits",
            suggestion.id(),
            suggestion.risk().as_str()
        )));
    }
    for prerequisite in suggestion.prerequisites() {
        if required_checks.binary_search(prerequisite).is_err() {
            return Err(RepairContractError::new(format!(
                "suggestion `{}` prerequisite `{prerequisite}` is absent from requiredChecks",
                suggestion.id()
            )));
        }
    }
    let source = finding.source().ok_or_else(|| {
        RepairContractError::new(format!(
            "finding `{}` has no exact source range",
            finding.fingerprint()
        ))
    })?;
    if source.file() != edit.file
        || edit.byte_start < source.byte_start()
        || edit.byte_end > source.byte_end()
    {
        return Err(RepairContractError::new(format!(
            "edit {}:{}..{} escapes finding source {}:{}..{}",
            edit.file,
            edit.byte_start,
            edit.byte_end,
            source.file(),
            source.byte_start(),
            source.byte_end()
        )));
    }
    Ok(())
}
