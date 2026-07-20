use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pliego_css_agent::{
    RepairCliFormat, RepairFixCliArgs, RepairFixMode, RepairPlanCliArgs, RepairSourceTransition,
    RepairTool, apply_repair_plan_checked, build_repair_plan, parse_repair_plan,
    parse_repair_proposal, verify_repair_plan,
};

use super::{
    CliFailure, command_logical_path_for, ensure_path_within_plan, existing_artifact_directory,
    existing_regular_file, read_bounded_utf8_document, read_command_artifact,
    reject_command_link_components, resolve_plan_relative_path,
};

struct LoadedRepairSources {
    root: PathBuf,
    paths: BTreeMap<String, PathBuf>,
    bytes: BTreeMap<String, Vec<u8>>,
}

pub(crate) fn run_repair_plan(arguments: &RepairPlanCliArgs) -> Result<(), CliFailure> {
    let finding_file = command_logical_path_for(
        &arguments.findings,
        "--findings",
        "plan",
        "finding document",
    )
    .map_err(CliFailure::invalid)?;
    let finding_bytes = read_command_artifact(&arguments.findings, "plan", "finding document")?;
    let proposal_bytes = read_command_artifact(&arguments.proposal, "plan", "repair proposal")?;
    let proposal = parse_repair_proposal(&proposal_bytes)
        .map_err(|error| CliFailure::invalid(error.to_string()))?;
    let files = proposal.files();
    let sources = load_repair_sources(&arguments.source_root, &files, "plan")?;
    let plan = build_repair_plan(
        RepairTool::new("pliegocss", env!("CARGO_PKG_VERSION"))
            .map_err(|error| CliFailure::tool(error.to_string()))?,
        &finding_file,
        &finding_bytes,
        &proposal,
        &sources.bytes,
    )
    .map_err(|error| CliFailure::tool(error.to_string()))?;
    match arguments.format {
        RepairCliFormat::Json => print!(
            "{}",
            plan.to_json_pretty()
                .map_err(|error| CliFailure::tool(error.to_string()))?
        ),
        RepairCliFormat::Text => print!(
            "{}",
            plan.to_human()
                .map_err(|error| CliFailure::tool(error.to_string()))?
        ),
    }
    Ok(())
}

pub(crate) fn run_repair_fix(arguments: &RepairFixCliArgs) -> Result<(), CliFailure> {
    let plan_bytes = read_command_artifact(&arguments.plan, "fix", "repair plan")?;
    let plan =
        parse_repair_plan(&plan_bytes).map_err(|error| CliFailure::invalid(error.to_string()))?;
    let finding_file =
        command_logical_path_for(&arguments.findings, "--findings", "fix", "finding document")
            .map_err(CliFailure::invalid)?;
    let finding_bytes = read_command_artifact(&arguments.findings, "fix", "finding document")?;
    let files = plan
        .sources()
        .iter()
        .map(RepairSourceTransition::file)
        .collect::<Vec<_>>();
    let sources = load_repair_sources(&arguments.source_root, &files, "fix")?;
    match &arguments.mode {
        RepairFixMode::DryRun => {
            let report = verify_repair_plan(&plan, &finding_file, &finding_bytes, &sources.bytes)
                .map_err(|error| CliFailure::tool(error.to_string()))?;
            match arguments.format {
                RepairCliFormat::Json => print!(
                    "{}",
                    report
                        .to_json_pretty()
                        .map_err(|error| CliFailure::tool(error.to_string()))?
                ),
                RepairCliFormat::Text => print!("{}", report.to_human()),
            }
        }
        RepairFixMode::Apply {
            authorization,
            receipt,
        } => {
            let published = apply_repair_plan_checked(
                &plan,
                &finding_file,
                &finding_bytes,
                &sources.bytes,
                authorization,
                &sources.root,
                &sources.paths,
                receipt,
                [&arguments.plan, &arguments.findings].map(PathBuf::as_path),
            )
            .map_err(|error| CliFailure::tool(error.to_string()))?;
            print!("{}", published.render(arguments.format));
        }
    }
    Ok(())
}

fn load_repair_sources(
    source_root: &Path,
    files: &[&str],
    command: &str,
) -> Result<LoadedRepairSources, CliFailure> {
    reject_command_link_components(source_root, command, "source root")
        .map_err(CliFailure::invalid)?;
    let root = existing_artifact_directory(source_root, "repair source root")
        .map_err(CliFailure::invalid)?;
    let mut paths = BTreeMap::new();
    let mut bytes_by_file = BTreeMap::new();
    for file in files {
        let relative = Path::new(file);
        let resolved = resolve_plan_relative_path(&root, relative, "repair source")
            .map_err(CliFailure::invalid)?;
        let resolved =
            existing_regular_file(&resolved, "repair source").map_err(CliFailure::invalid)?;
        ensure_path_within_plan(&root, &resolved, "repair source").map_err(CliFailure::invalid)?;
        let bytes =
            read_bounded_utf8_document(&resolved, "repair source").map_err(CliFailure::invalid)?;
        if paths.insert((*file).to_owned(), resolved).is_some()
            || bytes_by_file.insert((*file).to_owned(), bytes).is_some()
        {
            return Err(CliFailure::invalid(format!(
                "repair source `{file}` is duplicated"
            )));
        }
    }
    Ok(LoadedRepairSources {
        root,
        paths,
        bytes: bytes_by_file,
    })
}
