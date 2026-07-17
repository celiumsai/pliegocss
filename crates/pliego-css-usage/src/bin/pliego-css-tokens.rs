//! Read-only token-usage query CLI.

#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use pliego_css_usage::{explain_token_usage, explain_token_usage_text, parse_token_usage_report};

const USAGE: &str =
    "usage: pliego-css-tokens explain --report FILE --token KIND.NAME [--format text|json]";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        return Err(USAGE.into());
    };
    if matches!(command.as_str(), "help" | "-h" | "--help") {
        println!("{USAGE}");
        return Ok(());
    }
    if matches!(command.as_str(), "version" | "-V" | "--version") {
        println!("pliego-css-tokens {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if command != "explain" {
        return Err(format!("unknown command `{command}`\n{USAGE}"));
    }

    let mut report = None::<PathBuf>;
    let mut token = None::<String>;
    let mut format = None::<String>;
    while let Some(option) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for `{option}`"))?;
        match option.as_str() {
            "--report" if report.is_none() => report = Some(value.into()),
            "--token" if token.is_none() => token = Some(value),
            "--format" if format.is_none() => format = Some(value),
            "--report" | "--token" | "--format" => {
                return Err(format!("`{option}` may only be provided once"));
            }
            _ => return Err(format!("unknown option `{option}`")),
        }
    }
    let report = report.ok_or_else(|| "missing `--report`".to_owned())?;
    let token = token.ok_or_else(|| "missing `--token`".to_owned())?;
    let bytes = fs::read(&report)
        .map_err(|error| format!("cannot read token usage `{}`: {error}", report.display()))?;
    let report = parse_token_usage_report(&bytes)?;
    match format.as_deref().unwrap_or("text") {
        "text" => print!("{}", explain_token_usage_text(&report, &token)?),
        "json" => print!("{}", explain_token_usage(&report, &token)?),
        value => return Err(format!("unknown format `{value}`; expected text or json")),
    }
    Ok(())
}
