//! Static Rust source discovery for `PliegoCSS` macros.
//!
//! This is an exact-version tooling crate used by `pliego-cssc`. Scanner DTOs and composition
//! projections remain experimental rather than part of the minimal application API.
//!
//! This crate is deliberately independent from the CSS parser and compiler. It
//! answers one question: which *visible Rust string literals* can a build step
//! reach through [`pc!`](https://doc.rust-lang.org/reference/macros.html) and
//! `pcx!` invocations? A later pipeline stage can parse and compile the returned
//! strings.
//!
//! Rust is parsed with [`syn`], not searched with regular expressions. Inline
//! modules and expression macros are visited. Token trees belonging to other
//! macro calls are also inspected recursively, which covers forms such as
//! `view! { class = pc!("flex") }`. Declarative macro definitions are skipped
//! because their bodies are templates rather than reachable invocations.

#![forbid(unsafe_code)]

mod application;
mod discovery;
mod formatting;
mod migration;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use proc_macro2::{Group, Span, TokenStream, TokenTree};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Arm, Block, Expr, ExprIf, ExprLit, ExprMatch, Lit, LitStr, Stmt, Token};

pub use application::{
    ApplicationComponent, ApplicationIsland, ApplicationRoute, ApplicationTopology, CollectError,
    CollectedReachability,
};
pub use discovery::discover_migration_project;
pub use formatting::{
    UtilityFormatError, UtilityFormatFinding, UtilityFormatInspection, UtilityFormatRewrite,
    inspect_utility_format,
};
pub use migration::{
    MIGRATION_INVENTORY_SCHEMA_VERSION, MigrationAuxiliaryInventory, MigrationAuxiliaryKind,
    MigrationAuxiliaryObservation, MigrationAuxiliaryObservationKind, MigrationConstruct,
    MigrationConsumerInventory, MigrationConsumerKind, MigrationConsumerObservation,
    MigrationConsumerObservationKind, MigrationDependency, MigrationDependencyKind,
    MigrationDependencyResolution, MigrationDisposition, MigrationInventory,
    MigrationInventoryError, MigrationPreflightReliance, MigrationProject,
    MigrationProjectAuxiliary, MigrationProjectConsumer, MigrationProjectInventory,
    MigrationProjectSource, MigrationSourceKind, inventory_migration_auxiliary_file,
    inventory_migration_auxiliary_source, inventory_migration_consumer_file,
    inventory_migration_consumer_source, inventory_migration_file, inventory_migration_source,
};

/// A zero-based byte offset plus a human-readable source position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePosition {
    /// UTF-8 byte offset from the beginning of the source.
    pub byte: usize,
    /// One-based line number.
    pub line: usize,
    /// Zero-based column measured in Unicode scalar values.
    pub column: usize,
}

/// Half-open source range `[start, end)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceRange {
    /// Inclusive start position.
    pub start: SourcePosition,
    /// Exclusive end position.
    pub end: SourcePosition,
}

impl SourceRange {
    /// Returns the exact UTF-8 byte range represented by this source range.
    #[must_use]
    pub const fn byte_range(self) -> std::ops::Range<usize> {
        self.start.byte..self.end.byte
    }
}

/// One visible `PliegoCSS` string literal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleLiteral {
    /// Decoded Rust string value, with escapes and raw-string markers removed.
    pub value: String,
    /// Range of the complete Rust literal token, including its quotes.
    pub range: SourceRange,
}

/// Data extracted from a `pc!(...)` invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcInvocation {
    /// The sole statically visible style literal.
    pub style: StyleLiteral,
}

/// Identifies the conditional branch represented by a [`PcxBranch`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PcxBranchKind {
    /// The body selected when an `if` condition is true.
    IfThen,
    /// The body selected when an `if` condition is false.
    IfElse,
    /// A match arm in source order.
    MatchArm {
        /// Zero-based arm index.
        index: usize,
    },
}

/// One statically visible branch of a `pcx!(...)` invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcxBranch {
    /// Conditional branch identity.
    pub kind: PcxBranchKind,
    /// The branch's own literal.
    pub style: StyleLiteral,
    /// A deterministic base-plus-this-branch string for diagnostics and tooling.
    ///
    /// This is an extraction convenience, not a substitute for semantic
    /// composition in `pliego-css-compiler`.
    pub composed: String,
}

/// One independently evaluated clause in a `pcx!(...)` invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcxClause {
    /// Every visible branch in deterministic source order.
    pub branches: Vec<PcxBranch>,
}

/// One branch selected by a statically reachable Cartesian composition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcxSelection {
    /// Zero-based clause index in source order.
    pub clause: usize,
    /// Zero-based branch index inside that clause.
    pub branch: usize,
    /// The selected visible style literal.
    pub style: StyleLiteral,
}

/// Formats one reachable `pcx!` selection as a stable provenance reason.
#[must_use]
pub fn pcx_composition_reason(selections: &[PcxSelection]) -> String {
    let path = selections
        .iter()
        .map(|selection| format!("c{}:b{}", selection.clause, selection.branch))
        .collect::<Vec<_>>()
        .join(",");
    format!("reachable-composition[{path}]")
}

/// Returns whether a source directory is conventionally hidden or generated.
#[must_use]
pub fn is_ignored_source_directory(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy())
        .is_some_and(|name| name == "target" || name == ".git" || name.starts_with('.'))
}

/// Formats source paths for a deterministic human-readable watch label.
#[must_use]
pub fn format_source_paths<'a>(paths: impl IntoIterator<Item = &'a Path>) -> String {
    paths
        .into_iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Adds stable candidate context to one parser or lowering failure.
#[must_use]
pub fn format_style_failure(index: usize, source: &str, error: &str) -> String {
    format!("finding {index} (`{source}`): {error}")
}

/// One statically reachable result of independently selecting every clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcxComposition {
    /// One selected branch per clause, in clause order.
    pub selections: Vec<PcxSelection>,
    /// Deterministic base-plus-all-selected-branches text for diagnostics and manifests.
    ///
    /// Downstream compilers must still perform semantic branch-over-base composition
    /// in selection order instead of lowering this joined string as one style.
    pub composed: String,
}

/// Maximum number of statically reachable results accepted from one `pcx!` invocation.
pub const MAX_PCX_COMBINATIONS: usize = 64;

/// Data extracted from a `pcx!(base, clause, ...)` invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcxInvocation {
    /// Style literal applied before each conditional branch.
    pub base: StyleLiteral,
    /// Independent clauses in source order.
    pub clauses: Vec<PcxClause>,
    /// Every reachable Cartesian composition in lexicographic clause/branch order.
    pub compositions: Vec<PcxComposition>,
}

/// Extracted `PliegoCSS` macro data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvocationKind {
    /// A `pc!(...)` invocation.
    Pc(PcInvocation),
    /// A `pcx!(...)` invocation.
    Pcx(PcxInvocation),
}

/// One valid macro invocation found in Rust source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Invocation {
    /// Logical source name supplied to the scanner.
    pub source: String,
    /// Range of the macro invocation.
    pub range: SourceRange,
    /// Typed extracted payload.
    pub kind: InvocationKind,
}

/// A statically unsupported or malformed `PliegoCSS` macro invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanDiagnostic {
    /// Stable scanner-specific diagnostic code.
    pub code: &'static str,
    /// Human-readable explanation.
    pub message: String,
    /// Logical source name supplied to the scanner.
    pub source: String,
    /// Most specific available source range.
    pub range: SourceRange,
}

impl fmt::Display for ScanDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} at {}:{}:{}",
            self.code,
            self.message,
            self.source,
            self.range.start.line,
            self.range.start.column + 1
        )
    }
}

/// Complete deterministic result for one Rust source unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanReport {
    /// Logical source name shared by all findings in the report.
    pub source: String,
    /// Valid macro invocations, ordered by source offset.
    pub invocations: Vec<Invocation>,
    /// Invalid macro invocations, ordered by source offset.
    pub diagnostics: Vec<ScanDiagnostic>,
}

/// Failure to parse the containing Rust source unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceParseError {
    /// Logical source name supplied to the scanner.
    pub source: String,
    /// Parser explanation.
    pub message: String,
    /// Parser error location.
    pub range: SourceRange,
}

impl fmt::Display for SourceParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "could not parse Rust source {}:{}:{}: {}",
            self.source,
            self.range.start.line,
            self.range.start.column + 1,
            self.message
        )
    }
}

impl std::error::Error for SourceParseError {}

impl From<SourceParseError> for ScanDiagnostic {
    fn from(error: SourceParseError) -> Self {
        Self {
            code: "PCR001",
            message: error.message,
            source: error.source,
            range: error.range,
        }
    }
}

/// Failure to read or parse a Rust source file.
#[derive(Debug)]
pub enum ScanFileError {
    /// The source file could not be read.
    Read {
        /// Requested path.
        path: PathBuf,
        /// Underlying I/O failure.
        source: std::io::Error,
    },
    /// The file was read but is not valid Rust syntax.
    Parse(SourceParseError),
}

impl fmt::Display for ScanFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(formatter, "could not read {}: {source}", path.display())
            }
            Self::Parse(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ScanFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse(source) => Some(source),
        }
    }
}

/// Scans an in-memory Rust source unit named `<memory>`.
///
/// # Errors
///
/// Returns [`SourceParseError`] when the containing Rust file cannot be parsed.
/// Malformed `pc!` and `pcx!` invocations are instead retained in
/// [`ScanReport::diagnostics`] so other valid invocations remain usable.
pub fn scan_source(source: &str) -> Result<ScanReport, SourceParseError> {
    scan_source_named("<memory>", source)
}

/// Scans an in-memory Rust source unit with an explicit logical name.
///
/// # Errors
///
/// Returns [`SourceParseError`] when the containing Rust file cannot be parsed.
pub fn scan_source_named(
    source_name: impl Into<String>,
    source: &str,
) -> Result<ScanReport, SourceParseError> {
    let source_name = source_name.into();
    let file = syn::parse_file(source).map_err(|error| SourceParseError {
        source: source_name.clone(),
        message: error.to_string(),
        range: range_from_span(source, error.span()),
    })?;
    let mut scanner = Scanner {
        source,
        source_name: &source_name,
        invocations: Vec::new(),
        diagnostics: Vec::new(),
    };
    scanner.visit_file(&file);
    scanner
        .invocations
        .sort_by_key(|invocation| invocation.range.start.byte);
    scanner
        .diagnostics
        .sort_by_key(|diagnostic| diagnostic.range.start.byte);
    let Scanner {
        invocations,
        diagnostics,
        ..
    } = scanner;
    Ok(ScanReport {
        source: source_name,
        invocations,
        diagnostics,
    })
}

/// Reads and scans one UTF-8 Rust source file.
///
/// # Errors
///
/// Returns [`ScanFileError::Read`] for I/O or UTF-8 failures, and
/// [`ScanFileError::Parse`] for invalid Rust syntax.
pub fn scan_file(path: impl AsRef<Path>) -> Result<ScanReport, ScanFileError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|source| ScanFileError::Read {
        path: path.to_owned(),
        source,
    })?;
    scan_source_named(path.display().to_string(), &source).map_err(ScanFileError::Parse)
}

struct Scanner<'source> {
    source: &'source str,
    source_name: &'source str,
    invocations: Vec<Invocation>,
    diagnostics: Vec<ScanDiagnostic>,
}

impl Scanner<'_> {
    fn scan_macro(&mut self, name: &str, tokens: TokenStream, range: SourceRange) {
        let parsed = match name {
            "pc" => syn::parse2::<PcInput>(tokens).map(|input| {
                InvocationKind::Pc(PcInvocation {
                    style: self.style_literal(&input.literal),
                })
            }),
            "pcx" => syn::parse2::<PcxInput>(tokens)
                .and_then(|input| self.extract_pcx(input))
                .map(InvocationKind::Pcx),
            _ => return,
        };

        match parsed {
            Ok(kind) => self.invocations.push(Invocation {
                source: self.source_name.to_owned(),
                range,
                kind,
            }),
            Err(error) => self.diagnostics.push(ScanDiagnostic {
                code: diagnostic_code(name, &error.to_string()),
                message: diagnostic_message(error.to_string()),
                source: self.source_name.to_owned(),
                range: range_from_span(self.source, error.span()),
            }),
        }
    }

    fn extract_pcx(&self, input: PcxInput) -> syn::Result<PcxInvocation> {
        let base = self.style_literal(&input.base);
        let mut clauses = Vec::with_capacity(input.clauses.len());
        for expression in input.clauses {
            let branches = match expression {
                Expr::If(expression) => self.extract_if_branches(&base.value, expression)?,
                Expr::Match(expression) => self.extract_match_branches(&base.value, expression)?,
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "PSC003: `pcx!` clauses must be complete `if ... else ...` or `match ...` expressions",
                    ));
                }
            };
            clauses.push(PcxClause { branches });
        }

        let branch_counts = clauses
            .iter()
            .map(|clause| clause.branches.len())
            .collect::<Vec<_>>();
        if branch_counts
            .iter()
            .try_fold(1_usize, |count, branches| {
                count
                    .checked_mul(*branches)
                    .filter(|next| *next <= MAX_PCX_COMBINATIONS)
            })
            .is_none()
        {
            return Err(syn::Error::new(
                input.base.span(),
                format!(
                    "PSC006: `pcx!` expands to more than {MAX_PCX_COMBINATIONS} style combinations"
                ),
            ));
        }

        let compositions = cartesian_indices(&branch_counts)
            .into_iter()
            .map(|indices| {
                let mut composed = base.value.clone();
                let selections = indices
                    .into_iter()
                    .enumerate()
                    .map(|(clause_index, branch_index)| {
                        let style = clauses[clause_index].branches[branch_index].style.clone();
                        composed = compose_visible_source(&composed, &style.value);
                        PcxSelection {
                            clause: clause_index,
                            branch: branch_index,
                            style,
                        }
                    })
                    .collect();
                PcxComposition {
                    selections,
                    composed,
                }
            })
            .collect();

        Ok(PcxInvocation {
            base,
            clauses,
            compositions,
        })
    }

    fn extract_if_branches(&self, base: &str, expression: ExprIf) -> syn::Result<Vec<PcxBranch>> {
        let then_literal = visible_block_literal(&expression.then_branch)?;
        let Some((_, else_expression)) = expression.else_branch else {
            return Err(syn::Error::new_spanned(
                expression.then_branch,
                "PSC004: `pcx!` requires an `else` block with one visible string literal",
            ));
        };
        let Expr::Block(else_block) = *else_expression else {
            return Err(syn::Error::new_spanned(
                else_expression,
                "PSC004: the `else` branch must be a block containing one visible string literal",
            ));
        };
        let else_literal = visible_block_literal(&else_block.block)?;
        Ok(vec![
            self.branch(base, PcxBranchKind::IfThen, &then_literal),
            self.branch(base, PcxBranchKind::IfElse, &else_literal),
        ])
    }

    fn extract_match_branches(
        &self,
        base: &str,
        expression: ExprMatch,
    ) -> syn::Result<Vec<PcxBranch>> {
        if expression.arms.is_empty() {
            return Err(syn::Error::new_spanned(
                expression.expr,
                "PSC005: `pcx!` match requires at least one arm",
            ));
        }
        expression
            .arms
            .into_iter()
            .enumerate()
            .map(|(index, arm)| self.extract_match_arm(base, index, arm))
            .collect()
    }

    fn extract_match_arm(&self, base: &str, index: usize, arm: Arm) -> syn::Result<PcxBranch> {
        if !arm.attrs.is_empty() || arm.guard.is_some() {
            return Err(syn::Error::new_spanned(
                arm.pat,
                "PSC005: `pcx!` match arms do not support attributes or guards",
            ));
        }
        let literal = visible_expression_literal(&arm.body)?;
        Ok(self.branch(base, PcxBranchKind::MatchArm { index }, &literal))
    }

    fn branch(&self, base: &str, kind: PcxBranchKind, literal: &LitStr) -> PcxBranch {
        let style = self.style_literal(literal);
        let composed = compose_visible_source(base, &style.value);
        PcxBranch {
            kind,
            style,
            composed,
        }
    }

    fn style_literal(&self, literal: &LitStr) -> StyleLiteral {
        StyleLiteral {
            value: literal.value(),
            range: range_from_span(self.source, literal.span()),
        }
    }

    fn scan_nested_tokens(&mut self, stream: TokenStream) {
        let tokens: Vec<_> = stream.into_iter().collect();
        let mut index = 0;
        while index < tokens.len() {
            if let Some((name, group, path_start)) = nested_pliego_macro(&tokens, index) {
                let start = tokens[path_start].span().byte_range().start;
                let end = group.span().byte_range().end;
                self.scan_macro(
                    name,
                    group.stream(),
                    range_from_offsets(self.source, start, end),
                );
                self.scan_nested_tokens(group.stream());
                index += 3;
                continue;
            }
            if let TokenTree::Group(group) = &tokens[index] {
                self.scan_nested_tokens(group.stream());
            }
            index += 1;
        }
    }
}

impl<'ast> Visit<'ast> for Scanner<'_> {
    fn visit_macro(&mut self, macro_node: &'ast syn::Macro) {
        let name = macro_node
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
        match name.as_deref() {
            // `macro_rules!` bodies are templates rather than reachable calls.
            Some("macro_rules") => {}
            Some(name @ ("pc" | "pcx")) => {
                self.scan_macro(
                    name,
                    macro_node.tokens.clone(),
                    range_from_span(self.source, macro_node.span()),
                );
                self.scan_nested_tokens(macro_node.tokens.clone());
            }
            _ => self.scan_nested_tokens(macro_node.tokens.clone()),
        }
        // `syn` treats macro input as opaque tokens, so there is no useful
        // default traversal to invoke here.
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        // The default visitor descends into inline module contents and safely
        // ignores external `mod name;` declarations.
        visit::visit_item_mod(self, node);
    }
}

struct PcInput {
    literal: LitStr,
}

impl Parse for PcInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let literal = input.parse().map_err(|error| {
            syn::Error::new(
                error.span(),
                "PSC001: `pc!` requires one Rust string literal",
            )
        })?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(input.error("PSC001: `pc!` accepts exactly one Rust string literal"));
        }
        Ok(Self { literal })
    }
}

struct PcxInput {
    base: LitStr,
    clauses: Vec<Expr>,
}

impl Parse for PcxInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let base = input.parse().map_err(|error| {
            syn::Error::new(
                error.span(),
                "PSC002: `pcx!` base must be a visible Rust string literal",
            )
        })?;
        input.parse::<Token![,]>().map_err(|error| {
            syn::Error::new(
                error.span(),
                "PSC002: expected a comma after the `pcx!` base literal",
            )
        })?;
        let mut clauses = Vec::new();
        while !input.is_empty() {
            clauses.push(input.parse()?);
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
        }
        if clauses.is_empty() {
            return Err(input.error("PSC003: `pcx!` requires at least one conditional clause"));
        }
        Ok(Self { base, clauses })
    }
}

fn visible_block_literal(block: &Block) -> syn::Result<LitStr> {
    let [Stmt::Expr(expression, None)] = block.stmts.as_slice() else {
        return Err(syn::Error::new_spanned(
            block,
            "PSC004: each branch must contain exactly one visible string literal",
        ));
    };
    visible_expression_literal(expression)
}

fn visible_expression_literal(expression: &Expr) -> syn::Result<LitStr> {
    let Expr::Lit(ExprLit {
        lit: Lit::Str(literal),
        ..
    }) = expression
    else {
        return Err(syn::Error::new_spanned(
            expression,
            "PSC004: conditional branches must be visible Rust string literals",
        ));
    };
    Ok(literal.clone())
}

fn compose_visible_source(base: &str, branch: &str) -> String {
    if branch.trim().is_empty() {
        base.to_owned()
    } else {
        format!("{base} {branch}")
    }
}

fn cartesian_indices(branch_counts: &[usize]) -> Vec<Vec<usize>> {
    let mut combinations = vec![Vec::new()];
    for branch_count in branch_counts {
        let mut next = Vec::with_capacity(combinations.len() * branch_count);
        for prefix in combinations {
            for branch in 0..*branch_count {
                let mut combination = prefix.clone();
                combination.push(branch);
                next.push(combination);
            }
        }
        combinations = next;
    }
    combinations
}

fn nested_pliego_macro(tokens: &[TokenTree], index: usize) -> Option<(&str, &Group, usize)> {
    let TokenTree::Ident(ident) = tokens.get(index)? else {
        return None;
    };
    let name = match ident.to_string().as_str() {
        "pc" => "pc",
        "pcx" => "pcx",
        _ => return None,
    };
    let TokenTree::Punct(bang) = tokens.get(index + 1)? else {
        return None;
    };
    if bang.as_char() != '!' {
        return None;
    }
    let TokenTree::Group(group) = tokens.get(index + 2)? else {
        return None;
    };
    Some((name, group, qualified_path_start(tokens, index)))
}

fn qualified_path_start(tokens: &[TokenTree], mut index: usize) -> usize {
    while index >= 3 {
        let is_separator = matches!(tokens.get(index - 1), Some(TokenTree::Punct(punct)) if punct.as_char() == ':')
            && matches!(tokens.get(index - 2), Some(TokenTree::Punct(punct)) if punct.as_char() == ':');
        if !is_separator || !matches!(tokens.get(index - 3), Some(TokenTree::Ident(_))) {
            break;
        }
        index -= 3;
    }
    index
}

fn diagnostic_code(name: &str, message: &str) -> &'static str {
    ["PSC001", "PSC002", "PSC003", "PSC004", "PSC005", "PSC006"]
        .into_iter()
        .find(|code| message.contains(&format!("{code}: ")))
        .unwrap_or(match name {
            "pc" => "PSC001",
            "pcx" => "PSC002",
            _ => "PSC000",
        })
}

fn diagnostic_message(message: String) -> String {
    [
        "PSC001: ", "PSC002: ", "PSC003: ", "PSC004: ", "PSC005: ", "PSC006: ",
    ]
    .into_iter()
    .find_map(|prefix| {
        message
            .find(prefix)
            .map(|start| message[start + prefix.len()..].to_owned())
    })
    .unwrap_or(message)
}

fn range_from_span(source: &str, span: Span) -> SourceRange {
    let range = span.byte_range();
    range_from_offsets(source, range.start, range.end)
}

fn range_from_offsets(source: &str, start: usize, end: usize) -> SourceRange {
    let start = start.min(source.len());
    let end = end.clamp(start, source.len());
    SourceRange {
        start: position(source, start),
        end: position(source, end),
    }
}

fn position(source: &str, byte: usize) -> SourcePosition {
    let prefix = &source[..byte];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = prefix.rfind('\n').map_or(0, |offset| offset + 1);
    let column = source[line_start..byte].chars().count();
    SourcePosition { byte, line, column }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(source: &str) -> ScanReport {
        let report = scan_source(source).expect("Rust source should parse");
        assert_eq!(report.diagnostics, Vec::new());
        report
    }

    #[test]
    fn extracts_pc_literal_and_exact_ranges() {
        let source = "fn main() { let s = pc!(r#\"flex gap-4\"#); }";
        let report = valid(source);
        let [invocation] = report.invocations.as_slice() else {
            panic!("expected one invocation");
        };
        assert_eq!(
            &source[invocation.range.byte_range()],
            "pc!(r#\"flex gap-4\"#)"
        );
        let InvocationKind::Pc(pc) = &invocation.kind else {
            panic!("expected pc invocation");
        };
        assert_eq!(pc.style.value, "flex gap-4");
        assert_eq!(&source[pc.style.range.byte_range()], "r#\"flex gap-4\"#");
        assert_eq!(invocation.range.start.line, 1);
        assert_eq!(invocation.range.start.column, 20);
    }

    #[test]
    fn extracts_pcx_if_else_and_compositions() {
        let source = r#"
            fn pick(active: bool) {
                let _ = pcx!("flex text-sm", if active { "text-lg" } else { "opacity-50" });
            }
        "#;
        let report = valid(source);
        let InvocationKind::Pcx(pcx) = &report.invocations[0].kind else {
            panic!("expected pcx invocation");
        };
        assert_eq!(pcx.base.value, "flex text-sm");
        assert_eq!(pcx.clauses.len(), 1);
        assert_eq!(pcx.clauses[0].branches.len(), 2);
        assert_eq!(pcx.clauses[0].branches[0].kind, PcxBranchKind::IfThen);
        assert_eq!(pcx.clauses[0].branches[0].composed, "flex text-sm text-lg");
        assert_eq!(pcx.clauses[0].branches[1].kind, PcxBranchKind::IfElse);
        assert_eq!(
            pcx.clauses[0].branches[1].composed,
            "flex text-sm opacity-50"
        );
        assert_eq!(pcx.compositions.len(), 2);
    }

    #[test]
    fn extracts_every_pcx_match_arm_in_order() {
        let source = r#"
            fn pick(state: State) {
                let _ = pliego_css::pcx!("grid", match state {
                    State::Idle => "opacity-50",
                    State::Busy => "animate-spin",
                    _ => "hidden",
                });
            }
        "#;
        let report = valid(source);
        let InvocationKind::Pcx(pcx) = &report.invocations[0].kind else {
            panic!("expected pcx invocation");
        };
        let values: Vec<_> = pcx.clauses[0]
            .branches
            .iter()
            .map(|branch| branch.style.value.as_str())
            .collect();
        assert_eq!(values, ["opacity-50", "animate-spin", "hidden"]);
        assert_eq!(
            pcx.clauses[0].branches[2].kind,
            PcxBranchKind::MatchArm { index: 2 }
        );
    }

    #[test]
    fn extracts_multiple_clauses_and_cartesian_compositions() {
        let source = r#"
            fn pick(active: bool, size: Size) {
                view! { pcx!(
                    "inline-flex",
                    if active { "opacity-100" } else { "opacity-50" },
                    match size {
                        Size::Small => "text-sm",
                        Size::Large => "text-lg",
                    },
                ) }
            }
        "#;
        let report = valid(source);
        let InvocationKind::Pcx(pcx) = &report.invocations[0].kind else {
            panic!("expected pcx invocation");
        };
        assert_eq!(pcx.clauses.len(), 2);
        assert_eq!(pcx.compositions.len(), 4);
        let composed = pcx
            .compositions
            .iter()
            .map(|composition| composition.composed.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            composed,
            [
                "inline-flex opacity-100 text-sm",
                "inline-flex opacity-100 text-lg",
                "inline-flex opacity-50 text-sm",
                "inline-flex opacity-50 text-lg",
            ]
        );
        assert_eq!(
            pcx.compositions[3]
                .selections
                .iter()
                .map(|selection| (selection.clause, selection.branch))
                .collect::<Vec<_>>(),
            [(0, 1), (1, 1)]
        );
    }

    #[test]
    fn diagnoses_cartesian_expansion_limit() {
        let source = r#"
            fn styles(a: bool, b: bool, c: bool, d: bool, e: bool, f: bool, g: bool) {
                let _ = pcx!("",
                    if a { "" } else { "" }, if b { "" } else { "" },
                    if c { "" } else { "" }, if d { "" } else { "" },
                    if e { "" } else { "" }, if f { "" } else { "" },
                    if g { "" } else { "" },
                );
            }
        "#;
        let report = scan_source(source).expect("valid Rust");
        assert!(report.invocations.is_empty());
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "PSC006");
        assert!(!report.diagnostics[0].message.starts_with("PSC006:"));
        assert!(report.diagnostics[0].message.contains("more than 64"));
    }

    #[test]
    fn visits_inline_modules_expressions_and_other_macro_tokens() {
        let source = r#"
            fn outer() { consume(pc!("block")); }
            mod inline {
                fn nested() {
                    view! { <div class={pc!("flex")} data-x={pc!("grid")} /> }
                }
            }
        "#;
        let report = valid(source);
        let values: Vec<_> = report
            .invocations
            .iter()
            .map(|invocation| match &invocation.kind {
                InvocationKind::Pc(pc) => pc.style.value.as_str(),
                InvocationKind::Pcx(_) => panic!("unexpected pcx"),
            })
            .collect();
        assert_eq!(values, ["block", "flex", "grid"]);
    }

    #[test]
    fn visits_pliego_macros_nested_in_pcx_conditions() {
        let source = r#"
            fn styles(active: bool) {
                let _ = pcx!(
                    "block",
                    if uses(pc!("grid")) { "opacity-100" } else { "opacity-50" },
                );
            }
        "#;
        let report = valid(source);
        assert_eq!(report.invocations.len(), 2);
        assert!(matches!(report.invocations[0].kind, InvocationKind::Pcx(_)));
        let InvocationKind::Pc(pc) = &report.invocations[1].kind else {
            panic!("expected nested pc invocation");
        };
        assert_eq!(pc.style.value, "grid");
    }

    #[test]
    fn ignores_strings_comments_similar_names_and_macro_definitions() {
        let source = r#"
            // pc!("comment")
            const TEXT: &str = "pc!(\"string\")";
            fn pc(_: &str) {}
            fn main() { pc("function"); not_pc!("other"); }
            macro_rules! generated { () => { pc!("template") } }
        "#;
        let report = valid(source);
        assert!(report.invocations.is_empty());
    }

    #[test]
    fn retains_valid_calls_when_other_calls_are_dynamic() {
        let source = r#"
            fn styles(dynamic: &str, flag: bool) {
                let _ = pc!("flex");
                let _ = pc!(dynamic);
                let _ = pcx!("block", if flag { dynamic } else { "hidden" });
            }
        "#;
        let report = scan_source_named("src/lib.rs", source).expect("valid Rust");
        assert_eq!(report.invocations.len(), 1);
        assert_eq!(report.diagnostics.len(), 2);
        assert_eq!(report.diagnostics[0].code, "PSC001");
        assert!(report.diagnostics[0].message.contains("string literal"));
        assert_eq!(report.diagnostics[1].source, "src/lib.rs");
        assert_eq!(report.diagnostics[1].code, "PSC004");
        assert!(
            report.diagnostics[1]
                .message
                .contains("visible Rust string literals")
        );
    }

    #[test]
    fn embedded_scanner_code_survives_syn_context() {
        let report = scan_source_named("src/lib.rs", "fn view(){let _=pcx!(\"flex\",);}")
            .expect("valid Rust source");
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "PSC003");
        assert_eq!(
            report.diagnostics[0].message,
            "`pcx!` requires at least one conditional clause"
        );
    }

    #[test]
    fn rust_parse_error_converts_to_typed_scanner_diagnostic() {
        let error = scan_source_named("src/lib.rs", "fn {").expect_err("invalid Rust");
        let diagnostic = ScanDiagnostic::from(error);
        assert_eq!(diagnostic.code, "PCR001");
        assert_eq!(diagnostic.source, "src/lib.rs");
        assert_eq!(diagnostic.range.start.byte, 3);
        assert_eq!(diagnostic.range.end.byte, 3);
    }

    #[test]
    fn findings_are_source_ordered_and_deterministic() {
        let source = r#"
            fn styles(flag: bool) {
                wrapper!(pc!("first"), pcx!("base", if flag { "yes" } else { "no" }));
                let _ = pc!("last");
            }
        "#;
        let first = valid(source);
        let second = valid(source);
        assert_eq!(first, second);
        assert_eq!(first.invocations.len(), 3);
        assert!(
            first
                .invocations
                .windows(2)
                .all(|pair| { pair[0].range.start.byte < pair[1].range.start.byte })
        );
    }

    #[test]
    fn nested_qualified_macro_range_includes_its_path() {
        let source = r#"fn render() { view! { pliego_css::pc!("flex") } }"#;
        let report = valid(source);
        assert_eq!(
            &source[report.invocations[0].range.byte_range()],
            r#"pliego_css::pc!("flex")"#
        );
    }

    #[test]
    fn reports_invalid_containing_rust_without_panicking() {
        let error = scan_source("fn broken( {").expect_err("Rust parse should fail");
        assert!(!error.message.is_empty());
        assert_eq!(error.source, "<memory>");
    }
}
