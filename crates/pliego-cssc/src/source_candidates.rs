use std::fs;
use std::path::Path;

use pliego_css_compiler::{analyze_cross_clause_conflicts, lower_style_with_theme};
use pliego_css_ir::SemanticStyle;
use pliego_css_parser::{format_style_list, parse_style_list};
use pliego_css_source::{
    InvocationKind, ScanFileError, ScanReport, SourceRange, StyleLiteral, expand_source_paths,
    pcx_composition_reason, scan_file,
};
use pliego_css_theme::ThemeRegistry;

use super::{
    BuildArgs, Candidate, CliDiagnostic, CliDiagnosticOrigin, CliFailure, Provenance,
    cli_provenance, diagnostic_range_from_line, diagnostic_range_from_source,
    reject_isolated_carriage_returns, scan_failure, style_failure, style_literal_failure,
};

pub(crate) fn collect_candidates(
    theme: &ThemeRegistry,
    arguments: &BuildArgs,
) -> Result<Vec<Candidate>, CliFailure> {
    let mut candidates = cli_candidates(&arguments.styles, &arguments.compositions);
    if let Some(input) = &arguments.input {
        candidates.extend(candidates_from_lines(input)?);
    }
    let sources = expand_source_paths(&arguments.sources)?;
    for source in sources {
        candidates.extend(candidates_from_rust(theme, &source)?);
    }
    if candidates.is_empty() {
        return Err(CliFailure::tool("no styles found in supplied inputs"));
    }
    Ok(candidates)
}

pub(crate) fn cli_candidates(
    styles: &[String],
    compositions: &[(String, String)],
) -> Vec<Candidate> {
    let mut candidates = styles
        .iter()
        .enumerate()
        .map(|(index, style)| Candidate::Direct {
            style: style.clone(),
            provenance: cli_provenance(style, &format!("explicit-style-{}", index + 1)),
        })
        .collect::<Vec<_>>();
    candidates.extend(
        compositions
            .iter()
            .enumerate()
            .map(|(index, (base, branch))| Candidate::Composition {
                base: base.clone(),
                branches: vec![branch.clone()],
                provenance: cli_provenance(
                    &format!("{base} {branch}"),
                    &format!("explicit-composition-{}", index + 1),
                ),
            }),
    );
    candidates
}

pub(crate) fn candidates_from_lines(path: &Path) -> Result<Vec<Candidate>, CliFailure> {
    let source = fs::read_to_string(path)
        .map_err(|error| CliFailure::tool(format!("cannot read `{}`: {error}", path.display())))?;
    candidates_from_line_source(path, &source)
}

pub(crate) fn candidates_from_line_source(
    path: &Path,
    source: &str,
) -> Result<Vec<Candidate>, CliFailure> {
    reject_isolated_carriage_returns(path, source).map_err(CliFailure::tool)?;
    let mut candidates = Vec::new();
    let mut offset = 0;
    for (line_index, line) in source.split_inclusive('\n').enumerate() {
        let without_lf = line.strip_suffix('\n').unwrap_or(line);
        let without_newline = without_lf.strip_suffix('\r').unwrap_or(without_lf);
        let trimmed = without_newline.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            let syntax = parse_style_list(without_newline).map_err(|error| {
                let human = format!("{}:{}: {error}", path.display(), line_index + 1);
                let provenance = Provenance {
                    source: without_newline.to_owned(),
                    file: Some(path.display().to_string()),
                    byte_start: Some(offset),
                    byte_end: Some(offset + without_newline.len()),
                    macro_kind: "input".into(),
                    reason: "line-oriented-input".into(),
                };
                let mut failure = style_failure(error, human, &provenance);
                failure.diagnostics[0].range = Some(diagnostic_range_from_line(
                    path,
                    offset,
                    line_index,
                    without_newline,
                ));
                failure
            })?;
            let canonical = format_style_list(&syntax);
            let byte_start = syntax
                .items
                .first()
                .expect("non-empty style syntax")
                .span
                .start;
            let byte_end = syntax
                .items
                .last()
                .expect("non-empty style syntax")
                .span
                .end;
            candidates.push(Candidate::Direct {
                style: without_newline.to_owned(),
                provenance: Provenance {
                    source: canonical,
                    file: Some(path.display().to_string()),
                    byte_start: Some(offset + byte_start),
                    byte_end: Some(offset + byte_end),
                    macro_kind: "input".into(),
                    reason: "line-oriented-input".into(),
                },
            });
        }
        offset += line.len();
    }
    Ok(candidates)
}

pub(crate) fn candidates_from_rust(
    theme: &ThemeRegistry,
    path: &Path,
) -> Result<Vec<Candidate>, CliFailure> {
    let report = scan_file(path).map_err(|error| match error {
        ScanFileError::Parse(error) => scan_failure(&[error.into()]),
        error @ ScanFileError::Read { .. } => CliFailure::tool(error.to_string()),
    })?;
    candidates_from_scan_report(theme, &report)
}

pub(crate) fn candidates_from_scan_report(
    theme: &ThemeRegistry,
    report: &ScanReport,
) -> Result<Vec<Candidate>, CliFailure> {
    if !report.diagnostics.is_empty() {
        return Err(scan_failure(&report.diagnostics));
    }
    let mut candidates = Vec::new();
    for invocation in &report.invocations {
        match &invocation.kind {
            InvocationKind::Pc(pc) => candidates.push(Candidate::Direct {
                style: pc.style.value.clone(),
                provenance: scanner_provenance(
                    &invocation.source,
                    invocation.range,
                    "pc",
                    "visible-literal",
                    pc.style.value.clone(),
                ),
            }),
            InvocationKind::Pcx(pcx) => {
                let compiled_clauses = pcx
                    .clauses
                    .iter()
                    .map(|clause| {
                        clause
                            .branches
                            .iter()
                            .map(|branch| {
                                lower_scanned_style(theme, &branch.style, &invocation.source)
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if let Err(conflict) = analyze_cross_clause_conflicts(&compiled_clauses) {
                    let literal =
                        &pcx.clauses[conflict.right_clause].branches[conflict.right_branch].style;
                    let slots = conflict
                        .slots
                        .iter()
                        .map(|slot| format!("{slot:?}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let human = format!(
                        "PCX003: independent clauses {} and {} can both assign [{slots}] under the same condition; express the combined state space in one `match` at {}:{}:{} [bytes {}..{})",
                        conflict.left_clause + 1,
                        conflict.right_clause + 1,
                        invocation.source,
                        literal.range.start.line,
                        literal.range.start.column + 1,
                        literal.range.start.byte,
                        literal.range.end.byte,
                    );
                    return Err(CliFailure {
                        human,
                        diagnostics: vec![CliDiagnostic {
                            code: "PCX003".into(),
                            category: "composition".into(),
                            severity: "error".into(),
                            message: format!(
                                "independent clauses {} and {} can both assign [{slots}] under the same condition",
                                conflict.left_clause + 1,
                                conflict.right_clause + 1,
                            ),
                            suggestion: Some(
                                "express the combined state space in one `match`".into(),
                            ),
                            origin: Some(CliDiagnosticOrigin {
                                kind: "rust".into(),
                                label: "pcx-cross-clause-conflict".into(),
                            }),
                            range: Some(diagnostic_range_from_source(
                                &invocation.source,
                                literal.range,
                            )),
                            style_range: None,
                            replacement: None,
                        }],
                    });
                }
                for composition in &pcx.compositions {
                    let reason = pcx_composition_reason(&composition.selections);
                    let branches = composition
                        .selections
                        .iter()
                        .map(|selection| selection.style.value.clone())
                        .collect();
                    candidates.push(Candidate::Composition {
                        base: pcx.base.value.clone(),
                        branches,
                        provenance: scanner_provenance(
                            &invocation.source,
                            invocation.range,
                            "pcx",
                            &reason,
                            composition.composed.clone(),
                        ),
                    });
                }
            }
        }
    }
    Ok(candidates)
}

pub(crate) fn lower_scanned_style(
    theme: &ThemeRegistry,
    literal: &StyleLiteral,
    file: &str,
) -> Result<SemanticStyle, CliFailure> {
    let location = format!(
        "{file}:{}:{} [bytes {}..{})",
        literal.range.start.line,
        literal.range.start.column + 1,
        literal.range.start.byte,
        literal.range.end.byte
    );
    let syntax = parse_style_list(&literal.value).map_err(|error| {
        let human = format!("{error} at {location}");
        style_literal_failure(error, human, file, literal.range, "pcx-branch")
    })?;
    lower_style_with_theme(theme, &syntax).map_err(|error| {
        let human = format!("{error} at {location}");
        style_literal_failure(error, human, file, literal.range, "pcx-branch")
    })
}

pub(crate) fn scanner_provenance(
    file: &str,
    range: SourceRange,
    macro_kind: &str,
    reason: &str,
    source: String,
) -> Provenance {
    Provenance {
        source,
        file: Some(file.to_owned()),
        byte_start: Some(range.start.byte),
        byte_end: Some(range.end.byte),
        macro_kind: macro_kind.to_owned(),
        reason: reason.to_owned(),
    }
}
