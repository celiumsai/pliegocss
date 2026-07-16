//! Process boundary for closed post-change repair verification.

#![forbid(unsafe_code)]

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use pliego_css_agent::{
    RepairBrowserEvidenceResult, RepairCliFormat, RepairTestEvidenceResult,
    RepairVerificationResult, run_repair_pliegors_browser_checked, run_repair_rust_tests_checked,
    verify_repair_change_checked,
};

const USAGE: &str = "Usage:\n  pliego-css-agent verify --change-receipt FILE --check-policy FILE --source-root DIR --receipt FILE [--format text|json]\n  pliego-css-agent run-tests --change-receipt FILE --source-root DIR --check-id ID --evidence FILE\n  pliego-css-agent run-browser --change-receipt FILE --source-root DIR --check-id ID --evidence FILE";

struct VerifyArguments {
    change_receipt: PathBuf,
    check_policy: PathBuf,
    source_root: PathBuf,
    receipt: PathBuf,
    format: RepairCliFormat,
}

struct RunTestsArguments {
    change_receipt: PathBuf,
    source_root: PathBuf,
    check_id: String,
    evidence: PathBuf,
}

type RunBrowserArguments = RunTestsArguments;

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match run(&arguments) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(arguments: &[String]) -> Result<ExitCode, String> {
    if arguments == ["--version"] {
        println!("pliego-css-agent {}", env!("CARGO_PKG_VERSION"));
        return Ok(ExitCode::SUCCESS);
    }
    if arguments == ["--help"] || arguments == ["-h"] {
        println!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    }
    match arguments.first().map(String::as_str) {
        Some("verify") => {
            let parsed = parse_verify_arguments(arguments)?;
            let published = verify_repair_change_checked(
                &parsed.change_receipt,
                &parsed.check_policy,
                &parsed.source_root,
                &parsed.receipt,
            )
            .map_err(|error| error.to_string())?;
            print!("{}", published.render(parsed.format));
            Ok(
                if published.receipt().result() == RepairVerificationResult::Passed {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                },
            )
        }
        Some("run-tests") => {
            let parsed = parse_run_tests_arguments(arguments)?;
            let published = run_repair_rust_tests_checked(
                &parsed.change_receipt,
                &parsed.source_root,
                &parsed.check_id,
                &parsed.evidence,
            )
            .map_err(|error| error.to_string())?;
            print!("{}", published.to_human());
            Ok(
                if published.evidence().result() == RepairTestEvidenceResult::Passed {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                },
            )
        }
        Some("run-browser") => {
            let parsed = parse_run_browser_arguments(arguments)?;
            let published = run_repair_pliegors_browser_checked(
                &parsed.change_receipt,
                &parsed.source_root,
                &parsed.check_id,
                &parsed.evidence,
            )
            .map_err(|error| error.to_string())?;
            print!("{}", published.to_human());
            Ok(
                if published.evidence().result() == RepairBrowserEvidenceResult::Passed {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                },
            )
        }
        _ => Err(USAGE.into()),
    }
}

fn parse_verify_arguments(arguments: &[String]) -> Result<VerifyArguments, String> {
    if arguments.first().map(String::as_str) != Some("verify") {
        return Err(USAGE.into());
    }
    let mut change_receipt = None;
    let mut check_policy = None;
    let mut source_root = None;
    let mut receipt = None;
    let mut format = None;
    let mut index = 1;
    while index < arguments.len() {
        let option = arguments[index].as_str();
        let target = match option {
            "--change-receipt" => &mut change_receipt,
            "--check-policy" => &mut check_policy,
            "--source-root" => &mut source_root,
            "--receipt" => &mut receipt,
            "--format" => {
                if format.is_some() {
                    return Err("`--format` may only be provided once".into());
                }
                let value = value(arguments, index, option)?;
                format = Some(match value {
                    "text" => RepairCliFormat::Text,
                    "json" => RepairCliFormat::Json,
                    _ => return Err("`--format` must be `text` or `json`".into()),
                });
                index += 2;
                continue;
            }
            _ => return Err(format!("unknown verify option `{option}`")),
        };
        if target.is_some() {
            return Err(format!("`{option}` may only be provided once"));
        }
        *target = Some(PathBuf::from(value(arguments, index, option)?));
        index += 2;
    }
    Ok(VerifyArguments {
        change_receipt: change_receipt
            .ok_or_else(|| "`verify` requires `--change-receipt FILE`".to_owned())?,
        check_policy: check_policy
            .ok_or_else(|| "`verify` requires `--check-policy FILE`".to_owned())?,
        source_root: source_root
            .ok_or_else(|| "`verify` requires `--source-root DIR`".to_owned())?,
        receipt: receipt.ok_or_else(|| "`verify` requires `--receipt FILE`".to_owned())?,
        format: format.unwrap_or(RepairCliFormat::Text),
    })
}

fn parse_run_tests_arguments(arguments: &[String]) -> Result<RunTestsArguments, String> {
    parse_evidence_runner_arguments(arguments, "run-tests")
}

fn parse_run_browser_arguments(arguments: &[String]) -> Result<RunBrowserArguments, String> {
    parse_evidence_runner_arguments(arguments, "run-browser")
}

fn parse_evidence_runner_arguments(
    arguments: &[String],
    command: &str,
) -> Result<RunTestsArguments, String> {
    if arguments.first().map(String::as_str) != Some(command) {
        return Err(USAGE.into());
    }
    let mut change_receipt = None;
    let mut source_root = None;
    let mut check_id = None;
    let mut evidence = None;
    let mut index = 1;
    while index < arguments.len() {
        let option = arguments[index].as_str();
        match option {
            "--change-receipt" => {
                set_path(&mut change_receipt, arguments, &mut index, option)?;
            }
            "--source-root" => set_path(&mut source_root, arguments, &mut index, option)?,
            "--evidence" => set_path(&mut evidence, arguments, &mut index, option)?,
            "--check-id" => {
                if check_id.is_some() {
                    return Err("`--check-id` may only be provided once".into());
                }
                check_id = Some(value(arguments, index, option)?.to_owned());
                index += 2;
            }
            _ => return Err(format!("unknown {command} option `{option}`")),
        }
    }
    Ok(RunTestsArguments {
        change_receipt: change_receipt
            .ok_or_else(|| format!("`{command}` requires `--change-receipt FILE`"))?,
        source_root: source_root
            .ok_or_else(|| format!("`{command}` requires `--source-root DIR`"))?,
        check_id: check_id.ok_or_else(|| format!("`{command}` requires `--check-id ID`"))?,
        evidence: evidence.ok_or_else(|| format!("`{command}` requires `--evidence FILE`"))?,
    })
}

fn set_path(
    target: &mut Option<PathBuf>,
    arguments: &[String],
    index: &mut usize,
    option: &str,
) -> Result<(), String> {
    if target.is_some() {
        return Err(format!("`{option}` may only be provided once"));
    }
    *target = Some(PathBuf::from(value(arguments, *index, option)?));
    *index += 2;
    Ok(())
}

fn value<'a>(arguments: &'a [String], index: usize, option: &str) -> Result<&'a str, String> {
    arguments
        .get(index + 1)
        .map(String::as_str)
        .filter(|value| !value.is_empty() && !value.starts_with("--"))
        .ok_or_else(|| format!("`{option}` requires a value"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli_is_closed_and_complete() {
        let valid = [
            "verify",
            "--change-receipt",
            "change.json",
            "--check-policy",
            "checks.json",
            "--source-root",
            ".",
            "--receipt",
            "verified.json",
            "--format",
            "json",
        ]
        .map(str::to_owned);
        let parsed = parse_verify_arguments(&valid).unwrap();
        assert_eq!(parsed.format, RepairCliFormat::Json);
        assert!(parse_verify_arguments(&valid[..valid.len() - 1]).is_err());
        assert!(
            parse_verify_arguments(&["verify".into(), "--program".into(), "cargo".into()]).is_err()
        );
    }

    #[test]
    fn run_tests_cli_has_no_program_or_argument_surface() {
        let valid = [
            "run-tests",
            "--change-receipt",
            "change.json",
            "--source-root",
            ".",
            "--check-id",
            "tests.workspace",
            "--evidence",
            "test-evidence.json",
        ]
        .map(str::to_owned);
        let parsed = parse_run_tests_arguments(&valid).unwrap();
        assert_eq!(parsed.check_id, "tests.workspace");
        assert!(parse_run_tests_arguments(&valid[..valid.len() - 1]).is_err());
        assert!(
            parse_run_tests_arguments(&["run-tests".into(), "--program".into(), "cargo".into(),])
                .is_err()
        );
        let mut browser = valid;
        browser[0] = "run-browser".into();
        assert_eq!(
            parse_run_browser_arguments(&browser).unwrap().check_id,
            "tests.workspace"
        );
        assert!(
            parse_run_browser_arguments(&[
                "run-browser".into(),
                "--script".into(),
                "custom.mjs".into(),
            ])
            .is_err()
        );
    }
}
