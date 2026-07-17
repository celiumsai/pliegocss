use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{InvocationKind, ScanDiagnostic, SourceRange, StyleLiteral, scan_source_named};

/// One non-canonical Rust utility literal and its decoded replacement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityFormatFinding {
    /// Logical source path.
    pub file: String,
    /// Macro position such as `pc` or `pcx base`.
    pub role: String,
    /// Complete Rust literal-token range, including delimiters.
    pub range: SourceRange,
    /// Decoded value found in the source.
    pub actual: String,
    /// Canonical decoded value.
    pub expected: String,
}

impl UtilityFormatFinding {
    /// Renders the stable human finding used by source-formatting tools.
    #[must_use]
    pub fn human(&self) -> String {
        format!(
            "FMT001: {} at {}:{}:{} [bytes {}..{}): expected {:?}, found {:?}",
            self.role,
            self.file,
            self.range.start.line,
            self.range.start.column + 1,
            self.range.start.byte,
            self.range.end.byte,
            self.expected,
            self.actual,
        )
    }
}

/// One exact source snapshot and its fully validated replacement bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityFormatRewrite {
    /// Physical source path.
    pub path: PathBuf,
    /// Exact bytes observed before formatting.
    pub before: Vec<u8>,
    /// Complete Rust source after non-overlapping replacements.
    pub after: Vec<u8>,
}

/// Complete read-only inspection used by check and opt-in apply modes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityFormatInspection {
    /// Number of Rust source files inspected.
    pub files: usize,
    /// Number of utility literals inspected.
    pub checked: usize,
    /// Canonical formatting drift in source order.
    pub findings: Vec<UtilityFormatFinding>,
    /// Changed source snapshots, ordered by path.
    pub rewrites: Vec<UtilityFormatRewrite>,
}

/// Failure to inspect or prepare collision-free source rewrites.
#[derive(Debug)]
pub enum UtilityFormatError<E> {
    /// A source file could not be read or decoded as UTF-8.
    Read {
        /// Requested source path.
        path: PathBuf,
        /// Bounded I/O or UTF-8 explanation.
        message: String,
    },
    /// The containing Rust source could not be parsed.
    Parse(crate::SourceParseError),
    /// One or more macro invocations are not statically supported.
    Diagnostics(Vec<ScanDiagnostic>),
    /// A decoded utility value could not be formatted.
    Format {
        /// Logical source path.
        file: String,
        /// Macro literal role.
        role: String,
        /// Complete Rust literal range.
        range: SourceRange,
        /// Formatter-specific failure.
        error: E,
    },
    /// Scanner ranges collided or failed post-rewrite validation.
    Collision {
        /// Affected source path.
        path: PathBuf,
        /// Collision or postcondition explanation.
        message: String,
    },
}

impl<E: fmt::Display> fmt::Display for UtilityFormatError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, message } => {
                write!(formatter, "cannot read `{}`: {message}", path.display())
            }
            Self::Parse(error) => error.fmt(formatter),
            Self::Diagnostics(diagnostics) => write!(
                formatter,
                "{} unsupported PliegoCSS macro invocation(s)",
                diagnostics.len()
            ),
            Self::Format {
                file,
                role,
                range,
                error,
            } => write!(
                formatter,
                "{error} at {file}:{}:{} [bytes {}..{}) for {role}",
                range.start.line,
                range.start.column + 1,
                range.start.byte,
                range.end.byte
            ),
            Self::Collision { path, message } => {
                write!(
                    formatter,
                    "cannot safely format `{}`: {message}",
                    path.display()
                )
            }
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for UtilityFormatError<E> {}

/// Inspects exact Rust source snapshots and prepares validated, non-overlapping rewrites.
///
/// The callback receives decoded utility values. Rewrites always replace the complete Rust literal
/// token with a valid escaped string token and preserve every byte outside those ranges.
///
/// # Errors
///
/// Returns an error for unreadable or invalid Rust, unsupported macro inputs, formatter failures,
/// overlapping scanner ranges, or a rewritten document that does not rescan canonically.
pub fn inspect_utility_format<E>(
    paths: &[PathBuf],
    mut canonicalize: impl FnMut(&str) -> Result<String, E>,
) -> Result<UtilityFormatInspection, UtilityFormatError<E>> {
    let mut checked = 0;
    let mut findings = Vec::new();
    let mut rewrites = Vec::new();
    for path in paths {
        let before = fs::read(path).map_err(|error| UtilityFormatError::Read {
            path: path.clone(),
            message: error.to_string(),
        })?;
        let source = std::str::from_utf8(&before).map_err(|error| UtilityFormatError::Read {
            path: path.clone(),
            message: format!("source is not valid UTF-8: {error}"),
        })?;
        let file = path.display().to_string();
        let report = scan_source_named(file.clone(), source).map_err(UtilityFormatError::Parse)?;
        if !report.diagnostics.is_empty() {
            return Err(UtilityFormatError::Diagnostics(report.diagnostics));
        }
        let mut file_findings = Vec::new();
        visit_literals(&report, |role, literal| {
            checked += 1;
            let expected =
                canonicalize(&literal.value).map_err(|error| UtilityFormatError::Format {
                    file: file.clone(),
                    role: role.to_owned(),
                    range: literal.range,
                    error,
                })?;
            if literal.value != expected {
                file_findings.push(UtilityFormatFinding {
                    file: file.clone(),
                    role: role.to_owned(),
                    range: literal.range,
                    actual: literal.value.clone(),
                    expected,
                });
            }
            Ok(())
        })?;
        if !file_findings.is_empty() {
            let after = rewrite_source(path, source, &file_findings)?;
            validate_rewrite(path, &file, &after, &mut canonicalize)?;
            rewrites.push(UtilityFormatRewrite {
                path: path.clone(),
                before,
                after: after.into_bytes(),
            });
            findings.extend(file_findings);
        }
    }
    Ok(UtilityFormatInspection {
        files: paths.len(),
        checked,
        findings,
        rewrites,
    })
}

fn visit_literals<E>(
    report: &crate::ScanReport,
    mut visit: impl FnMut(&str, &StyleLiteral) -> Result<(), UtilityFormatError<E>>,
) -> Result<(), UtilityFormatError<E>> {
    for invocation in &report.invocations {
        match &invocation.kind {
            InvocationKind::Pc(pc) => visit("pc", &pc.style)?,
            InvocationKind::Pcx(pcx) => {
                visit("pcx base", &pcx.base)?;
                for (clause_index, clause) in pcx.clauses.iter().enumerate() {
                    for (branch_index, branch) in clause.branches.iter().enumerate() {
                        visit(
                            &format!(
                                "pcx clause {} branch {}",
                                clause_index + 1,
                                branch_index + 1
                            ),
                            &branch.style,
                        )?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn rewrite_source<E>(
    path: &Path,
    source: &str,
    findings: &[UtilityFormatFinding],
) -> Result<String, UtilityFormatError<E>> {
    let mut ordered = findings.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|finding| (finding.range.start.byte, finding.range.end.byte));
    for pair in ordered.windows(2) {
        if pair[0].range.end.byte > pair[1].range.start.byte {
            return Err(UtilityFormatError::Collision {
                path: path.to_owned(),
                message: format!(
                    "literal ranges {}..{} and {}..{} overlap",
                    pair[0].range.start.byte,
                    pair[0].range.end.byte,
                    pair[1].range.start.byte,
                    pair[1].range.end.byte
                ),
            });
        }
    }
    let mut output = source.to_owned();
    for finding in ordered.into_iter().rev() {
        let range = finding.range.byte_range();
        if range.end > output.len()
            || !output.is_char_boundary(range.start)
            || !output.is_char_boundary(range.end)
        {
            return Err(UtilityFormatError::Collision {
                path: path.to_owned(),
                message: format!("literal range {}..{} is stale", range.start, range.end),
            });
        }
        output.replace_range(range, &format!("{:?}", finding.expected));
    }
    Ok(output)
}

fn validate_rewrite<E>(
    path: &Path,
    file: &str,
    source: &str,
    canonicalize: &mut impl FnMut(&str) -> Result<String, E>,
) -> Result<(), UtilityFormatError<E>> {
    let report = scan_source_named(file, source).map_err(UtilityFormatError::Parse)?;
    if !report.diagnostics.is_empty() {
        return Err(UtilityFormatError::Diagnostics(report.diagnostics));
    }
    visit_literals(&report, |role, literal| {
        let expected =
            canonicalize(&literal.value).map_err(|error| UtilityFormatError::Format {
                file: file.to_owned(),
                role: role.to_owned(),
                range: literal.range,
                error,
            })?;
        if literal.value != expected {
            return Err(UtilityFormatError::Collision {
                path: path.to_owned(),
                message: format!("post-rewrite {role} literal is not canonical"),
            });
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::SourcePosition;

    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

    fn temp_file(label: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "pliego-css-source-format-{label}-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).expect("create format fixture");
        directory.join("view.rs")
    }

    fn position(byte: usize) -> SourcePosition {
        SourcePosition {
            byte,
            line: 1,
            column: byte,
        }
    }

    #[test]
    fn prepares_complete_valid_rust_without_touching_surrounding_bytes() {
        let path = temp_file("rewrite");
        let source = r##"fn view(active: bool) {
    let untouched = "  preserve  ";
    let _ = pc!(r#" flex   gap-4 "#);
    let _ = pcx!("grid  gap-4", if active { " block " } else { "hidden" });
}"##;
        fs::write(&path, source).expect("write Rust source");
        let report = inspect_utility_format(std::slice::from_ref(&path), |value| {
            Ok::<_, ()>(value.split_whitespace().collect::<Vec<_>>().join(" "))
        })
        .expect("inspect utility format");
        assert_eq!(report.files, 1);
        assert_eq!(report.checked, 4);
        assert_eq!(report.findings.len(), 3);
        assert_eq!(report.rewrites.len(), 1);
        let rendered = std::str::from_utf8(&report.rewrites[0].after).expect("UTF-8 rewrite");
        assert!(rendered.contains("let untouched = \"  preserve  \";"));
        assert!(rendered.contains("pc!(\"flex gap-4\")"));
        assert!(rendered.contains("pcx!(\"grid gap-4\""));
        assert!(rendered.contains("{ \"block\" } else { \"hidden\" }"));
        assert_eq!(
            fs::read(&path).expect("read-only inspection"),
            source.as_bytes()
        );
        fs::remove_dir_all(path.parent().expect("fixture parent")).expect("remove fixture");
    }

    #[test]
    fn rejects_overlapping_ranges_without_producing_bytes() {
        let path = PathBuf::from("view.rs");
        let findings = [
            UtilityFormatFinding {
                file: "view.rs".into(),
                role: "pc".into(),
                range: SourceRange {
                    start: position(0),
                    end: position(4),
                },
                actual: "a".into(),
                expected: "a".into(),
            },
            UtilityFormatFinding {
                file: "view.rs".into(),
                role: "pc".into(),
                range: SourceRange {
                    start: position(3),
                    end: position(5),
                },
                actual: "b".into(),
                expected: "b".into(),
            },
        ];
        let error = rewrite_source::<std::convert::Infallible>(&path, "12345", &findings)
            .expect_err("overlap");
        assert!(matches!(error, UtilityFormatError::Collision { .. }));
        assert!(error.to_string().contains("overlap"));
    }
}
