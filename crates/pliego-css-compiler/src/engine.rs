//! Pure, incremental analysis and physical-rule planning shared by every adapter.

use core::fmt;
use std::collections::BTreeMap;

use pliego_css_ir::{Diagnostic, SemanticStyle, StyleId};
use pliego_css_parser::parse_style_list;
use pliego_css_theme::{ThemeId, ThemeRegistry};

use super::{
    CrossClauseConflict, CssFragmentCache, EmitError, IdentityError, StyleLineage,
    analyze_cross_clause_conflicts, compose_style_override_with_theme, emit_css_with_theme_traced,
    emit_theme, lower_style_with_theme, try_encode_style_identity_with_theme,
};

/// Maximum number of statically materialized `pcx!` branch combinations.
pub const MAX_PCX_COMBINATIONS: usize = 64;
const MAX_SEMANTIC_CACHE_ITEMS: usize = 4_096;

/// One source-independent input accepted by the shared compiler engine.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileInput {
    /// Parse and lower one visible utility-list literal.
    Literal(String),
    /// Compose independently lowered branches over one base literal.
    Composition {
        /// Base utility-list literal.
        base: String,
        /// Ordered branch literals whose semantic slots override the base.
        branches: Vec<String>,
    },
    /// A validated semantic style handed off by an advanced adapter.
    Semantic(SemanticStyle),
}

impl CompileInput {
    /// Constructs one literal input.
    #[must_use]
    pub fn literal(source: impl Into<String>) -> Self {
        Self::Literal(source.into())
    }

    /// Constructs one composition input.
    #[must_use]
    pub fn composition(
        base: impl Into<String>,
        branches: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self::Composition {
            base: base.into(),
            branches: branches.into_iter().map(Into::into).collect(),
        }
    }
}

/// Complete, I/O-free request to the shared compiler engine.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompileRequest {
    /// Inputs to lower or validate.
    pub inputs: Vec<CompileInput>,
    /// Emit the active theme before physical style rules.
    pub include_theme: bool,
    /// Record declaration lineage for physical-rule evidence.
    pub physical_trace: bool,
}

impl CompileRequest {
    /// Constructs a request from ordered inputs.
    #[must_use]
    pub fn new(inputs: impl IntoIterator<Item = CompileInput>) -> Self {
        Self {
            inputs: inputs.into_iter().collect(),
            include_theme: false,
            physical_trace: false,
        }
    }
}

/// Cache activity caused by one engine request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AnalysisCacheStats {
    /// Literal analyses served from the semantic cache.
    pub semantic_hits: usize,
    /// Literal analyses computed during this request.
    pub semantic_misses: usize,
    /// Physical CSS fragments served from the emission cache.
    pub physical_hits: usize,
    /// Physical CSS fragments emitted during this request.
    pub physical_misses: usize,
    /// Physical fragments removed after the complete snapshot changed.
    pub physical_pruned: usize,
}

impl AnalysisCacheStats {
    fn delta(self, before: Self) -> Self {
        Self {
            semantic_hits: self.semantic_hits - before.semantic_hits,
            semantic_misses: self.semantic_misses - before.semantic_misses,
            physical_hits: self.physical_hits - before.physical_hits,
            physical_misses: self.physical_misses - before.physical_misses,
            physical_pruned: self.physical_pruned - before.physical_pruned,
        }
    }
}

/// One collision-checked physical style in canonical emission order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledStyle {
    /// Stable semantic IR and `StyleId`.
    pub semantic: SemanticStyle,
    /// Canonical identity bytes used for ordering, deduplication, and collision checks.
    pub identity_stream: Vec<u8>,
    /// Unoptimized deterministic CSS fragment.
    pub css: String,
    /// Optional physical-declaration lineage.
    pub lineage: Option<StyleLineage>,
}

/// Complete result returned by [`AnalysisHost::compile`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileResult {
    /// Theme identity used for every semantic and physical decision.
    pub theme_id: ThemeId,
    /// Unique styles in canonical identity-stream order.
    pub styles: Vec<CompiledStyle>,
    /// Unoptimized deterministic theme and style CSS.
    pub css: String,
    /// Cache activity caused by this request.
    pub cache: AnalysisCacheStats,
}

/// Failure returned by the pure compiler engine.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileError {
    /// Parser or semantic lowering rejected one input segment.
    Diagnostic {
        /// Zero-based request input index.
        input: usize,
        /// `base`, `literal`, or `branch-N`.
        segment: String,
        /// Stable compiler diagnostic.
        diagnostic: Diagnostic,
    },
    /// An adapter supplied malformed or theme-incompatible semantic IR.
    InvalidSemantic {
        /// Zero-based request input index.
        input: usize,
        /// Structural or identity failure.
        message: String,
    },
    /// Two canonical streams resolved to one `StyleId`.
    StyleIdCollision {
        /// Colliding style identity.
        style_id: StyleId,
    },
    /// One canonical stream resolved to two `StyleId` values.
    CanonicalStreamCollision,
    /// CSS emission failed after semantic validation.
    Emit(EmitError),
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diagnostic {
                input,
                segment,
                diagnostic,
            } => write!(formatter, "input {} {segment}: {diagnostic}", input + 1),
            Self::InvalidSemantic { input, message } => {
                write!(
                    formatter,
                    "input {} semantic IR is invalid: {message}",
                    input + 1
                )
            }
            Self::StyleIdCollision { style_id } => write!(
                formatter,
                "StyleId collision under the active format for {:032x}",
                style_id.get()
            ),
            Self::CanonicalStreamCollision => {
                formatter.write_str("one canonical style stream mapped to multiple StyleId values")
            }
            Self::Emit(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompileError {}

impl From<EmitError> for CompileError {
    fn from(error: EmitError) -> Self {
        Self::Emit(error)
    }
}

/// One physical-rule plan before target-specific CSS optimization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalRulePlan {
    /// Unique styles in canonical identity-stream order.
    pub styles: Vec<CompiledStyle>,
    /// Deterministic theme and style CSS.
    pub css: String,
}

/// Stateful planner that keeps physical emission separate from semantic identity.
#[derive(Default)]
pub struct PhysicalRulePlanner {
    fragments: CssFragmentCache,
}

impl PhysicalRulePlanner {
    /// Returns the number of cached physical fragments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fragments.len()
    }

    /// Returns whether the physical fragment cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fragments.is_empty()
    }

    /// Plans, collision-checks, deduplicates, and emits one complete semantic snapshot.
    ///
    /// # Errors
    ///
    /// Returns structural, identity, collision, or emitter failures without performing I/O.
    pub fn plan(
        &mut self,
        theme: &ThemeRegistry,
        styles: impl IntoIterator<Item = SemanticStyle>,
        include_theme: bool,
        physical_trace: bool,
        stats: &mut AnalysisCacheStats,
    ) -> Result<PhysicalRulePlan, CompileError> {
        let mut unique = BTreeMap::<Vec<u8>, SemanticStyle>::new();
        let mut streams_by_id = BTreeMap::<StyleId, Vec<u8>>::new();
        for (input, style) in styles.into_iter().enumerate() {
            style
                .validate()
                .map_err(|error| CompileError::InvalidSemantic {
                    input,
                    message: error.to_string(),
                })?;
            let stream = try_encode_style_identity_with_theme(theme, &style).map_err(
                |error: IdentityError| CompileError::InvalidSemantic {
                    input,
                    message: error.to_string(),
                },
            )?;
            match streams_by_id.entry(style.id) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(stream.clone());
                }
                std::collections::btree_map::Entry::Occupied(entry) if entry.get() != &stream => {
                    return Err(CompileError::StyleIdCollision { style_id: style.id });
                }
                std::collections::btree_map::Entry::Occupied(_) => {}
            }
            match unique.entry(stream) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(style);
                }
                std::collections::btree_map::Entry::Occupied(entry)
                    if entry.get().id != style.id =>
                {
                    return Err(CompileError::CanonicalStreamCollision);
                }
                std::collections::btree_map::Entry::Occupied(_) => {}
            }
        }

        let mut css = String::new();
        if include_theme {
            css.push_str(&emit_theme(theme));
            css.push('\n');
        }
        let mut planned = Vec::with_capacity(unique.len());
        for (stream, semantic) in &unique {
            let (fragment, lineage) = if physical_trace {
                let (fragment, lineage) = emit_css_with_theme_traced(theme, semantic)?;
                stats.physical_misses += 1;
                (fragment, Some(lineage))
            } else {
                let (fragment, hit) = self.fragments.emit(stream, theme, semantic)?;
                if hit {
                    stats.physical_hits += 1;
                } else {
                    stats.physical_misses += 1;
                }
                (fragment.to_owned(), None)
            };
            css.push_str(&fragment);
            css.push('\n');
            planned.push(CompiledStyle {
                semantic: semantic.clone(),
                identity_stream: stream.clone(),
                css: fragment,
                lineage,
            });
        }
        stats.physical_pruned += self.fragments.retain(unique.keys());
        Ok(PhysicalRulePlan {
            styles: planned,
            css,
        })
    }

    fn clear(&mut self) {
        self.fragments = CssFragmentCache::default();
    }
}

/// Source-independent `pcx` request shared by macros, scanners, and editors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcxRequest {
    /// Base utility-list literal.
    pub base: String,
    /// Ordered clauses, each containing all statically visible branch literals.
    pub clauses: Vec<Vec<String>>,
}

impl PcxRequest {
    /// Constructs a shared conditional-analysis request.
    #[must_use]
    pub fn new(
        base: impl Into<String>,
        clauses: impl IntoIterator<Item = impl IntoIterator<Item = impl Into<String>>>,
    ) -> Self {
        Self {
            base: base.into(),
            clauses: clauses
                .into_iter()
                .map(|clause| clause.into_iter().map(Into::into).collect())
                .collect(),
        }
    }
}

/// One fully composed static `pcx` branch combination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcxCombination {
    /// Zero-based selected branch index for every clause.
    pub selections: Vec<usize>,
    /// Semantic result after applying the selections over the base.
    pub semantic: SemanticStyle,
}

/// Complete shared `pcx` frontend result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcxAnalysis {
    /// Lowered base style.
    pub base: SemanticStyle,
    /// Lowered styles for every visible clause branch.
    pub clauses: Vec<Vec<SemanticStyle>>,
    /// All bounded branch combinations in stable cartesian order.
    pub combinations: Vec<PcxCombination>,
}

/// Failure returned by the shared `pcx` frontend.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PcxError {
    /// The base literal failed parser or semantic validation.
    Base(Diagnostic),
    /// One visible branch failed parser or semantic validation.
    Branch {
        /// Zero-based clause index.
        clause: usize,
        /// Zero-based branch index.
        branch: usize,
        /// Stable compiler diagnostic.
        diagnostic: Diagnostic,
    },
    /// One clause did not contain a visible branch.
    EmptyClause {
        /// Zero-based clause index.
        clause: usize,
    },
    /// Independently selectable clauses overlap semantically.
    Conflict(CrossClauseConflict),
    /// Static materialization would exceed [`MAX_PCX_COMBINATIONS`].
    ExpansionLimit {
        /// Maximum accepted combination count.
        maximum: usize,
    },
}

impl fmt::Display for PcxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Base(diagnostic) => write!(formatter, "pcx base: {diagnostic}"),
            Self::Branch {
                clause,
                branch,
                diagnostic,
            } => write!(
                formatter,
                "pcx clause {} branch {}: {diagnostic}",
                clause + 1,
                branch + 1
            ),
            Self::EmptyClause { clause } => {
                write!(
                    formatter,
                    "pcx clause {} has no visible branches",
                    clause + 1
                )
            }
            Self::Conflict(conflict) => write!(
                formatter,
                "PCX003: independent clauses {} and {} overlap",
                conflict.left_clause + 1,
                conflict.right_clause + 1
            ),
            Self::ExpansionLimit { maximum } => write!(
                formatter,
                "PCX004: pcx expands to more than {maximum} style combinations"
            ),
        }
    }
}

impl std::error::Error for PcxError {}

/// Long-lived semantic and physical analysis state shared by CLI, watch, and LSP adapters.
pub struct AnalysisHost {
    theme: ThemeRegistry,
    semantic: BTreeMap<String, Result<SemanticStyle, Diagnostic>>,
    planner: PhysicalRulePlanner,
    stats: AnalysisCacheStats,
}

impl Default for AnalysisHost {
    fn default() -> Self {
        Self::new(ThemeRegistry::seed())
    }
}

impl AnalysisHost {
    /// Starts one analysis host for an exact theme registry.
    #[must_use]
    pub fn new(theme: ThemeRegistry) -> Self {
        Self {
            theme,
            semantic: BTreeMap::new(),
            planner: PhysicalRulePlanner::default(),
            stats: AnalysisCacheStats::default(),
        }
    }

    /// Returns the exact active theme.
    #[must_use]
    pub const fn theme(&self) -> &ThemeRegistry {
        &self.theme
    }

    /// Replaces the active theme and invalidates every theme-bound cache when its ID changes.
    pub fn set_theme(&mut self, theme: ThemeRegistry) {
        if self.theme.id() != theme.id() {
            self.semantic.clear();
            self.planner.clear();
        }
        self.theme = theme;
    }

    /// Returns cumulative cache activity for observability and watch-mode reporting.
    #[must_use]
    pub const fn stats(&self) -> AnalysisCacheStats {
        self.stats
    }

    /// Returns the number of retained physical CSS fragments.
    #[must_use]
    pub fn physical_cache_len(&self) -> usize {
        self.planner.len()
    }

    /// Parses and lowers one literal through the shared semantic cache.
    ///
    /// # Errors
    ///
    /// Returns the stable parser or semantic diagnostic.
    pub fn analyze_literal(&mut self, source: &str) -> Result<SemanticStyle, Diagnostic> {
        if let Some(cached) = self.semantic.get(source) {
            self.stats.semantic_hits += 1;
            return cached.clone();
        }
        if self.semantic.len() >= MAX_SEMANTIC_CACHE_ITEMS {
            self.semantic.clear();
        }
        self.stats.semantic_misses += 1;
        let result = parse_style_list(source)
            .and_then(|syntax| lower_style_with_theme(&self.theme, &syntax));
        self.semantic.insert(source.to_owned(), result.clone());
        result
    }

    /// Runs the shared bounded conditional frontend.
    ///
    /// # Errors
    ///
    /// Returns literal diagnostics, empty clauses, cross-clause conflicts, or expansion overflow.
    pub fn analyze_pcx(&mut self, request: &PcxRequest) -> Result<PcxAnalysis, PcxError> {
        let base = self
            .analyze_literal(&request.base)
            .map_err(PcxError::Base)?;
        let mut clauses = Vec::with_capacity(request.clauses.len());
        for (clause_index, clause) in request.clauses.iter().enumerate() {
            if clause.is_empty() {
                return Err(PcxError::EmptyClause {
                    clause: clause_index,
                });
            }
            let mut branches = Vec::with_capacity(clause.len());
            for (branch_index, source) in clause.iter().enumerate() {
                branches.push(self.analyze_literal(source).map_err(|diagnostic| {
                    PcxError::Branch {
                        clause: clause_index,
                        branch: branch_index,
                        diagnostic,
                    }
                })?);
            }
            clauses.push(branches);
        }
        analyze_cross_clause_conflicts(&clauses).map_err(PcxError::Conflict)?;
        let branch_counts = clauses.iter().map(Vec::len).collect::<Vec<_>>();
        let valid_count = branch_counts.iter().try_fold(1_usize, |count, branches| {
            count
                .checked_mul(*branches)
                .filter(|next| *next <= MAX_PCX_COMBINATIONS)
        });
        if valid_count.is_none() {
            return Err(PcxError::ExpansionLimit {
                maximum: MAX_PCX_COMBINATIONS,
            });
        }
        let mut combinations = Vec::new();
        for selections in cartesian_indices(&branch_counts) {
            let mut semantic = base.clone();
            for (clause, branch) in clauses.iter().zip(&selections) {
                semantic = compose_style_override_with_theme(
                    &self.theme,
                    semantic,
                    clause[*branch].clone(),
                );
            }
            combinations.push(PcxCombination {
                selections,
                semantic,
            });
        }
        Ok(PcxAnalysis {
            base,
            clauses,
            combinations,
        })
    }

    /// Executes one complete pure compiler request.
    ///
    /// # Errors
    ///
    /// Returns stable input diagnostics or physical planning failures.
    pub fn compile(&mut self, request: &CompileRequest) -> Result<CompileResult, CompileError> {
        let before = self.stats;
        let mut semantics = Vec::with_capacity(request.inputs.len());
        for (input_index, input) in request.inputs.iter().enumerate() {
            let semantic = match input {
                CompileInput::Literal(source) => {
                    self.analyze_literal(source)
                        .map_err(|diagnostic| CompileError::Diagnostic {
                            input: input_index,
                            segment: "literal".into(),
                            diagnostic,
                        })?
                }
                CompileInput::Composition { base, branches } => {
                    let mut semantic = self.analyze_literal(base).map_err(|diagnostic| {
                        CompileError::Diagnostic {
                            input: input_index,
                            segment: "base".into(),
                            diagnostic,
                        }
                    })?;
                    for (branch_index, branch) in branches.iter().enumerate() {
                        let branch = self.analyze_literal(branch).map_err(|diagnostic| {
                            CompileError::Diagnostic {
                                input: input_index,
                                segment: format!("branch-{}", branch_index + 1),
                                diagnostic,
                            }
                        })?;
                        semantic = compose_style_override_with_theme(&self.theme, semantic, branch);
                    }
                    semantic
                }
                CompileInput::Semantic(semantic) => semantic.clone(),
            };
            semantics.push(semantic);
        }
        let plan = self.planner.plan(
            &self.theme,
            semantics,
            request.include_theme,
            request.physical_trace,
            &mut self.stats,
        )?;
        Ok(CompileResult {
            theme_id: self.theme.id(),
            styles: plan.styles,
            css: plan.css,
            cache: self.stats.delta(before),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_request_is_deterministic_and_incremental() {
        let mut host = AnalysisHost::default();
        let request = CompileRequest {
            inputs: vec![
                CompileInput::literal("flex p-4"),
                CompileInput::literal("grid gap-2"),
                CompileInput::literal("flex p-4"),
            ],
            include_theme: false,
            physical_trace: false,
        };
        let first = host.compile(&request).unwrap();
        let second = host.compile(&request).unwrap();
        assert_eq!(first.css, second.css);
        assert_eq!(first.styles.len(), 2);
        assert_eq!(first.cache.semantic_misses, 2);
        assert_eq!(first.cache.semantic_hits, 1);
        assert_eq!(second.cache.semantic_hits, 3);
        assert_eq!(second.cache.semantic_misses, 0);
        assert_eq!(second.cache.physical_hits, 2);
    }

    #[test]
    fn pcx_frontend_materializes_shared_combinations() {
        let mut host = AnalysisHost::default();
        let analysis = host
            .analyze_pcx(&PcxRequest::new(
                "flex",
                [["p-2", "p-4"], ["text-ink", "text-muted"]],
            ))
            .unwrap();
        assert_eq!(analysis.combinations.len(), 4);
        assert_eq!(analysis.combinations[0].selections, [0, 0]);
        assert_eq!(analysis.combinations[3].selections, [1, 1]);
    }

    #[test]
    fn pcx_frontend_rejects_cross_clause_overlap() {
        let mut host = AnalysisHost::default();
        let error = host
            .analyze_pcx(&PcxRequest::new("flex", [["p-2", "p-4"], ["px-1", "px-3"]]))
            .unwrap_err();
        assert!(matches!(error, PcxError::Conflict(_)));
    }

    #[test]
    fn pcx_frontend_enforces_the_shared_expansion_limit() {
        let mut host = AnalysisHost::default();
        let error = host
            .analyze_pcx(&PcxRequest::new(
                "p-1",
                [
                    ["[g2-a:0]", "[g2-a:1]"],
                    ["[g2-b:0]", "[g2-b:1]"],
                    ["[g2-c:0]", "[g2-c:1]"],
                    ["[g2-d:0]", "[g2-d:1]"],
                    ["[g2-e:0]", "[g2-e:1]"],
                    ["[g2-f:0]", "[g2-f:1]"],
                    ["[g2-g:0]", "[g2-g:1]"],
                ],
            ))
            .unwrap_err();
        assert_eq!(
            error,
            PcxError::ExpansionLimit {
                maximum: MAX_PCX_COMBINATIONS
            }
        );
    }

    #[test]
    fn physical_planner_prunes_styles_absent_from_the_complete_snapshot() {
        let mut host = AnalysisHost::default();
        host.compile(&CompileRequest::new([
            CompileInput::literal("flex"),
            CompileInput::literal("grid"),
        ]))
        .unwrap();
        assert_eq!(host.physical_cache_len(), 2);

        let result = host
            .compile(&CompileRequest::new([CompileInput::literal("grid")]))
            .unwrap();
        assert_eq!(result.cache.physical_hits, 1);
        assert_eq!(result.cache.physical_pruned, 1);
        assert_eq!(host.physical_cache_len(), 1);
    }

    #[test]
    fn theme_change_invalidates_semantic_cache() {
        let mut host = AnalysisHost::default();
        host.analyze_literal("flex").unwrap();
        host.analyze_literal("flex").unwrap();
        let before = host.stats();
        let seed = ThemeRegistry::seed();
        let mut tokens = seed.tokens().to_vec();
        tokens[0].value = "1px".into();
        let theme = ThemeRegistry::from_definitions(tokens, seed.breakpoints().to_vec()).unwrap();
        assert_ne!(theme.id(), seed.id());
        host.set_theme(theme);
        host.analyze_literal("flex").unwrap();
        assert_eq!(host.stats().semantic_hits, before.semantic_hits);
        assert_eq!(host.stats().semantic_misses, before.semantic_misses + 1);
    }
}
