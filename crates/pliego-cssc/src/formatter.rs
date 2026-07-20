use std::fs;
use std::path::{Path, PathBuf};

use pliego_css_ir::Diagnostic;
use pliego_css_parser::{format_style_list, parse_style_list};
use pliego_css_source::{UtilityFormatError, expand_source_paths, inspect_utility_format};

use super::{
    CliFailure, Provenance, UtilityFormatArgs, acquire_publication_locks, cli_provenance,
    commit_prepared_locked_with, diagnostic_range_from_line, format_failure, prepare_atomic_write,
    publication_destinations, scan_failure, style_failure, style_literal_failure, write_if_changed,
};

pub(crate) fn run_utility_formatter(arguments: &UtilityFormatArgs) -> Result<(), CliFailure> {
    if !arguments.sources.is_empty() {
        return run_rust_utility_format(&arguments.sources, arguments.apply);
    }
    if let Some(input) = &arguments.input {
        let source = fs::read_to_string(input).map_err(|error| {
            CliFailure::tool(format!("cannot read `{}`: {error}", input.display()))
        })?;
        let rendered = format_line_document(input, &source)?;
        if arguments.check {
            if source == rendered {
                println!("ok: `{}` is formatted", input.display());
                return Ok(());
            }
            return Err(CliFailure::tool(format!(
                "formatting drift detected in `{}`; run `pliego-cssc fmt --input {} --output <path>` to inspect the canonical document",
                input.display(),
                input.display()
            )));
        }
        return publish_formatted(arguments.output.as_deref(), &rendered).map_err(Into::into);
    }

    let formatted = arguments
        .styles
        .iter()
        .enumerate()
        .map(|(index, source)| {
            format_utility_source(source).map_err(|error| {
                let human = format!("style {}: {error}", index + 1);
                style_failure(
                    error,
                    human,
                    &cli_provenance(source, &format!("fmt-style-{}", index + 1)),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if arguments.check {
        if let Some((index, (actual, expected))) = arguments
            .styles
            .iter()
            .zip(&formatted)
            .enumerate()
            .find(|(_, (actual, expected))| actual != expected)
        {
            return Err(CliFailure::tool(format!(
                "style {} is not formatted; expected `{expected}`, found `{actual}`",
                index + 1
            )));
        }
        println!("ok: {} style(s) are formatted", formatted.len());
        return Ok(());
    }
    let mut rendered = formatted.join("\n");
    rendered.push('\n');
    publish_formatted(arguments.output.as_deref(), &rendered).map_err(Into::into)
}

pub(crate) fn run_rust_utility_format(sources: &[PathBuf], apply: bool) -> Result<(), CliFailure> {
    let files = expand_source_paths(sources)?;
    let inspection =
        inspect_utility_format(&files, format_utility_source).map_err(|error| match error {
            UtilityFormatError::Diagnostics(diagnostics) => scan_failure(&diagnostics),
            UtilityFormatError::Format {
                file,
                role,
                range,
                error,
            } => {
                let human = format!("{error} in {file}");
                style_literal_failure(error, human, &file, range, &role)
            }
            error => CliFailure::tool(error.to_string()),
        })?;
    if inspection.findings.is_empty() {
        println!(
            "ok: {} Rust utility literal(s) are formatted across {} file(s)",
            inspection.checked, inspection.files
        );
        Ok(())
    } else if apply {
        publish_utility_rewrites(&inspection.rewrites).map_err(CliFailure::tool)?;
        println!("fixed");
        Ok(())
    } else {
        Err(format_failure(&inspection.findings))
    }
}

pub(crate) fn publish_utility_rewrites(
    rewrites: &[pliego_css_source::UtilityFormatRewrite],
) -> Result<(), String> {
    let destinations = publication_destinations(rewrites.iter().map(|item| item.path.as_path()))?;
    let _locks = acquire_publication_locks(&destinations)?;
    for rewrite in rewrites {
        if fs::read(&rewrite.path)
            .map_err(|error| format!("read `{}`: {error}", rewrite.path.display()))?
            != rewrite.before
        {
            return Err(format!("`{}` changed", rewrite.path.display()));
        }
    }
    let mut writes = Vec::with_capacity(rewrites.len());
    for rewrite in rewrites {
        let permissions = fs::metadata(&rewrite.path)
            .map_err(|error| format!("stat `{}`: {error}", rewrite.path.display()))?
            .permissions();
        let write = prepare_atomic_write(&rewrite.path, &rewrite.after)?;
        if let Some(temporary) = &write.temporary {
            fs::set_permissions(temporary, permissions)
                .map_err(|error| format!("chmod `{}`: {error}", rewrite.path.display()))?;
        }
        writes.push(write);
    }
    commit_prepared_locked_with(writes, |_, temporary, destination| {
        fs::rename(temporary, destination)
    })
}

pub(crate) fn format_utility_source(source: &str) -> Result<String, Diagnostic> {
    let syntax = parse_style_list(source)?;
    Ok(format_style_list(&syntax))
}

pub(crate) fn format_line_document(path: &Path, source: &str) -> Result<String, CliFailure> {
    if source.is_empty() {
        return Ok(String::new());
    }
    reject_isolated_carriage_returns(path, source).map_err(CliFailure::tool)?;
    let mut lines = Vec::new();
    let mut offset = 0;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        let without_lf = line.strip_suffix('\n').unwrap_or(line);
        let without_newline = without_lf.strip_suffix('\r').unwrap_or(without_lf);
        let trimmed = without_newline.trim();
        if trimmed.is_empty() {
            lines.push(String::new());
        } else if trimmed.starts_with('#') {
            lines.push(trimmed.to_owned());
        } else {
            lines.push(format_utility_source(without_newline).map_err(|error| {
                let human = format!("{}:{}: {error}", path.display(), index + 1);
                let provenance = Provenance {
                    source: without_newline.to_owned(),
                    file: Some(path.display().to_string()),
                    byte_start: Some(offset),
                    byte_end: Some(offset + without_newline.len()),
                    macro_kind: "input".into(),
                    reason: "fmt-line".into(),
                };
                let mut failure = style_failure(error, human, &provenance);
                failure.diagnostics[0].range = Some(diagnostic_range_from_line(
                    path,
                    offset,
                    index,
                    without_newline,
                ));
                failure
            })?);
        }
        offset += line.len();
    }
    let mut rendered = lines.join("\n");
    rendered.push('\n');
    Ok(rendered)
}

pub(crate) fn reject_isolated_carriage_returns(path: &Path, source: &str) -> Result<(), String> {
    let bytes = source.as_bytes();
    if let Some(offset) = bytes.iter().enumerate().find_map(|(index, byte)| {
        (*byte == b'\r' && bytes.get(index + 1) != Some(&b'\n')).then_some(index)
    }) {
        Err(format!(
            "line-oriented input `{}` contains an isolated carriage return at byte {offset}; use LF or CRLF line endings",
            path.display()
        ))
    } else {
        Ok(())
    }
}

fn publish_formatted(output: Option<&Path>, rendered: &str) -> Result<(), String> {
    if let Some(output) = output {
        write_if_changed(output, rendered.as_bytes()).map(|_| ())
    } else {
        print!("{rendered}");
        Ok(())
    }
}
