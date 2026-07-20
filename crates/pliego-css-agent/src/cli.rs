use std::path::PathBuf;

/// Human or canonical JSON projection selected by the repair CLI.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RepairCliFormat {
    /// Concise human projection.
    #[default]
    Text,
    /// Canonical machine-readable JSON.
    Json,
}

/// Parsed arguments for the read-only `plan` command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairPlanCliArgs {
    /// Canonical finding document path.
    pub findings: PathBuf,
    /// Exact agent-authored proposal path.
    pub proposal: PathBuf,
    /// Root against which proposal logical source paths resolve.
    pub source_root: PathBuf,
    /// Output projection; JSON is the command default.
    pub format: RepairCliFormat,
}

/// Explicit execution mode for `fix`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepairFixMode {
    /// Verify all bound inputs without mutation.
    DryRun,
    /// Apply exact edits with an external exact-plan authorization and emit a receipt.
    Apply {
        /// Literal `sha256:<planSha256>` authorization token.
        authorization: String,
        /// Required adjacent Change Receipt destination.
        receipt: PathBuf,
    },
}

/// Parsed arguments for the `fix` command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairFixCliArgs {
    /// Integrity-bound repair plan path.
    pub plan: PathBuf,
    /// Original finding document bound by the plan.
    pub findings: PathBuf,
    /// Root against which plan logical source paths resolve.
    pub source_root: PathBuf,
    /// Read-only or explicitly authorized apply mode.
    pub mode: RepairFixMode,
    /// Output projection; text is the command default.
    pub format: RepairCliFormat,
}

/// Closed repair command surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepairCliCommand {
    /// Create a dry-run-only plan.
    Plan(RepairPlanCliArgs),
    /// Verify or explicitly apply a plan.
    Fix(RepairFixCliArgs),
}

/// Parses the closed `plan` or `fix` option surface.
///
/// # Errors
///
/// Returns a stable invocation message for missing, duplicate, conflicting, or unknown options.
pub fn parse_repair_cli_arguments(
    command: &str,
    arguments: &[String],
) -> Result<RepairCliCommand, String> {
    let fix = match command {
        "plan" => false,
        "fix" => true,
        _ => return Err(format!("unknown repair command `{command}`")),
    };
    let mut findings = None;
    let mut proposal = None;
    let mut plan = None;
    let mut source_root = None;
    let mut dry_run = false;
    let mut apply = false;
    let mut authorization = None;
    let mut receipt = None;
    let mut format = None;
    let mut index = 0;
    while index < arguments.len() {
        let option = arguments[index].as_str();
        match option {
            "--findings" => set_cli_path(&mut findings, arguments, &mut index, option)?,
            "--proposal" if !fix => set_cli_path(&mut proposal, arguments, &mut index, option)?,
            "--plan" if fix => set_cli_path(&mut plan, arguments, &mut index, option)?,
            "--source-root" => set_cli_path(&mut source_root, arguments, &mut index, option)?,
            "--dry-run" if fix && !dry_run => {
                dry_run = true;
                index += 1;
            }
            "--dry-run" if fix => return Err("`--dry-run` may only be provided once".into()),
            "--apply" if fix && !apply => {
                apply = true;
                index += 1;
            }
            "--apply" if fix => return Err("`--apply` may only be provided once".into()),
            "--authorize" if fix => {
                if authorization.is_some() {
                    return Err("`--authorize` may only be provided once".into());
                }
                authorization = Some(cli_value(arguments, index, option)?.to_owned());
                index += 2;
            }
            "--receipt" if fix => set_cli_path(&mut receipt, arguments, &mut index, option)?,
            "--format" => {
                if format.is_some() {
                    return Err("`--format` may only be provided once".into());
                }
                format = Some(match cli_value(arguments, index, option)? {
                    "text" => RepairCliFormat::Text,
                    "json" => RepairCliFormat::Json,
                    value => {
                        return Err(format!(
                            "unknown repair format `{value}`; expected `text` or `json`"
                        ));
                    }
                });
                index += 2;
            }
            _ => return Err(format!("unknown {command} option `{option}`")),
        }
    }
    let source_root =
        source_root.ok_or_else(|| format!("`{command}` requires `--source-root DIR`"))?;
    if fix {
        let mode = match (dry_run, apply) {
            (true, false) if authorization.is_none() && receipt.is_none() => RepairFixMode::DryRun,
            (false, true) => RepairFixMode::Apply {
                authorization: authorization.ok_or_else(|| {
                    "`fix --apply` requires `--authorize sha256:PLAN_HASH`".to_owned()
                })?,
                receipt: receipt
                    .ok_or_else(|| "`fix --apply` requires `--receipt FILE`".to_owned())?,
            },
            (true, true) => return Err("`--dry-run` and `--apply` are mutually exclusive".into()),
            (true, false) => {
                return Err("`--authorize` and `--receipt` require `--apply`".into());
            }
            (false, false) => {
                return Err("`fix` requires exactly one of `--dry-run` or `--apply`".into());
            }
        };
        return Ok(RepairCliCommand::Fix(RepairFixCliArgs {
            plan: plan.ok_or_else(|| "`fix` requires `--plan FILE`".to_owned())?,
            findings: findings.ok_or_else(|| "`fix` requires `--findings FILE`".to_owned())?,
            source_root,
            mode,
            format: format.unwrap_or_default(),
        }));
    }
    Ok(RepairCliCommand::Plan(RepairPlanCliArgs {
        findings: findings.ok_or_else(|| "`plan` requires `--findings FILE`".to_owned())?,
        proposal: proposal.ok_or_else(|| "`plan` requires `--proposal FILE`".to_owned())?,
        source_root,
        format: format.unwrap_or(RepairCliFormat::Json),
    }))
}

fn set_cli_path(
    slot: &mut Option<PathBuf>,
    arguments: &[String],
    index: &mut usize,
    option: &str,
) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("`{option}` may only be provided once"));
    }
    *slot = Some(PathBuf::from(cli_value(arguments, *index, option)?));
    *index += 2;
    Ok(())
}

fn cli_value<'a>(arguments: &'a [String], index: usize, option: &str) -> Result<&'a str, String> {
    arguments
        .get(index + 1)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("`{option}` requires a value"))
}
