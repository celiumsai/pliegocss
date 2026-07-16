//! Procedural macros for `PliegoCSS`.
//!
//! This is an implementation crate whose internal macros return validated identity values. The
//! `pliego-css` facade wraps them with declarative `$crate` hygiene. Applications must import `pc!`
//! and `pcx!` through that facade instead of depending on this crate directly.

#![forbid(unsafe_code)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    Arm, Block, Expr, ExprIf, ExprLit, ExprMatch, Lit, LitStr, Result, Stmt, Token,
    parse_macro_input,
};

use pliego_css_ir::SemanticStyle;
use pliego_css_theme::ThemeRegistry;

const THEME_PATH_ENV: &str = "PLIEGO_CSS_THEME_PATH";
const THEME_ID_ENV: &str = "PLIEGO_CSS_THEME_ID";

struct StyleInput {
    literal: LitStr,
}

impl Parse for StyleInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let literal = input.parse()?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(input.error("`pc!` accepts exactly one Rust string literal"));
        }
        Ok(Self { literal })
    }
}

/// Validates a utility list at compile time and returns its deterministic identity bits.
#[doc(hidden)]
#[proc_macro]
pub fn pc_id(tokens: TokenStream) -> TokenStream {
    let StyleInput { literal } = parse_macro_input!(tokens as StyleInput);
    let theme = match active_theme(&literal) {
        Ok(theme) => theme,
        Err(error) => return error,
    };
    let source = literal.value();
    let syntax = match pliego_css_parser::parse_style_list(&source) {
        Ok(syntax) => syntax,
        Err(diagnostic) => return compile_error(&literal, diagnostic),
    };
    let semantic = match pliego_css_compiler::lower_style_with_theme(&theme, &syntax) {
        Ok(semantic) => semantic,
        Err(diagnostic) => return compile_error(&literal, diagnostic),
    };
    let id = semantic.id.get();

    quote!(#id).into()
}

struct ConditionalInput {
    base: LitStr,
    clauses: Vec<Expr>,
}

impl Parse for ConditionalInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let base = input.parse()?;
        input.parse::<Token![,]>()?;
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
            return Err(input.error("`pcx!` requires at least one conditional clause"));
        }
        Ok(Self { base, clauses })
    }
}

const MAX_PCX_COMBINATIONS: usize = 64;

struct CompiledBranch {
    literal: LitStr,
    style: SemanticStyle,
}

struct CompiledClause {
    selector: TokenStream2,
    branches: Vec<CompiledBranch>,
    span: proc_macro2::Span,
}

/// Compiles every visible conditional combination and returns the selected identity bits.
#[doc(hidden)]
#[proc_macro]
pub fn pcx_id(tokens: TokenStream) -> TokenStream {
    let ConditionalInput { base, clauses } = parse_macro_input!(tokens as ConditionalInput);
    let theme = match active_theme(&base) {
        Ok(theme) => theme,
        Err(error) => return error,
    };
    let base_style = match compile_visible_literal(&theme, &base) {
        Ok(style) => style,
        Err(error) => return error,
    };

    let mut compiled_clauses = Vec::with_capacity(clauses.len());
    for clause in clauses {
        match compile_clause(&theme, clause) {
            Ok(clause) => compiled_clauses.push(clause),
            Err(error) => return error,
        }
    }

    if let Err(error) = validate_cross_clause_conflicts(&compiled_clauses) {
        return error.into_compile_error().into();
    }

    let branch_counts = compiled_clauses
        .iter()
        .map(|clause| clause.branches.len())
        .collect::<Vec<_>>();
    let combination_count = branch_counts.iter().try_fold(1_usize, |count, branches| {
        count
            .checked_mul(*branches)
            .filter(|next| *next <= MAX_PCX_COMBINATIONS)
    });
    if combination_count.is_none() {
        let span = compiled_clauses
            .last()
            .map_or_else(|| base.span(), |clause| clause.span);
        return syn::Error::new(
            span,
            format!(
                "PCX004: `pcx!` expands to more than {MAX_PCX_COMBINATIONS} style combinations; collapse related state into one `match`"
            ),
        )
        .into_compile_error()
        .into();
    }

    let combinations = cartesian_indices(&branch_counts);
    let mut result_arms = Vec::with_capacity(combinations.len());
    for indices in combinations {
        let mut style = base_style.clone();
        for (clause, branch_index) in compiled_clauses.iter().zip(&indices) {
            style = pliego_css_compiler::compose_style_override_with_theme(
                &theme,
                style,
                clause.branches[*branch_index].style.clone(),
            );
        }
        let id = style.id.get();
        result_arms.push(quote! {
            (#(#indices,)*) => #id
        });
    }

    let choice_names = (0..compiled_clauses.len())
        .map(|index| {
            format_ident!(
                "__pliego_pcx_choice_{index}",
                span = proc_macro2::Span::mixed_site()
            )
        })
        .collect::<Vec<_>>();
    let selectors = compiled_clauses
        .into_iter()
        .map(|clause| clause.selector)
        .collect::<Vec<_>>();

    quote! {{
        #(
            let #choice_names: usize = #selectors;
        )*
        match (#(#choice_names,)*) {
            #(#result_arms),*,
            _ => 0_u128,
        }
    }}
    .into()
}

fn compile_clause(
    theme: &ThemeRegistry,
    expression: Expr,
) -> core::result::Result<CompiledClause, TokenStream> {
    match expression {
        Expr::If(expression) => compile_if_clause(theme, expression),
        Expr::Match(expression) => compile_match_clause(theme, expression),
        other => Err(syn::Error::new_spanned(
            other,
            "`pcx!` clauses must be complete `if ... else ...` or `match ...` expressions",
        )
        .into_compile_error()
        .into()),
    }
}

fn compile_if_clause(
    theme: &ThemeRegistry,
    expression: ExprIf,
) -> core::result::Result<CompiledClause, TokenStream> {
    let span = expression.span();
    let ExprIf {
        cond,
        then_branch,
        else_branch,
        ..
    } = expression;
    let then_literal = match visible_block_literal(&then_branch) {
        Ok(literal) => literal,
        Err(error) => return Err(error.into_compile_error().into()),
    };
    let Some((_, else_expression)) = else_branch else {
        return Err(syn::Error::new_spanned(
            then_branch,
            "`pcx!` requires an `else` branch with a visible string literal",
        )
        .into_compile_error()
        .into());
    };
    let Expr::Block(else_block) = *else_expression else {
        return Err(syn::Error::new_spanned(
            else_expression,
            "PCX001: the `else` branch must be a block containing one visible string literal",
        )
        .into_compile_error()
        .into());
    };
    let else_literal = match visible_block_literal(&else_block.block) {
        Ok(literal) => literal,
        Err(error) => return Err(error.into_compile_error().into()),
    };

    let then_style = compile_visible_literal(theme, &then_literal)?;
    let else_style = compile_visible_literal(theme, &else_literal)?;

    Ok(CompiledClause {
        selector: quote! {
            if #cond { 0usize } else { 1usize }
        },
        branches: vec![
            CompiledBranch {
                literal: then_literal,
                style: then_style,
            },
            CompiledBranch {
                literal: else_literal,
                style: else_style,
            },
        ],
        span,
    })
}

fn compile_match_clause(
    theme: &ThemeRegistry,
    expression: ExprMatch,
) -> core::result::Result<CompiledClause, TokenStream> {
    let span = expression.span();
    let ExprMatch { expr, arms, .. } = expression;
    if arms.is_empty() {
        return Err(
            syn::Error::new_spanned(expr, "`pcx!` match requires at least one arm")
                .into_compile_error()
                .into(),
        );
    }

    let mut selector_arms = Vec::with_capacity(arms.len());
    let mut branches = Vec::with_capacity(arms.len());
    for (index, arm) in arms.into_iter().enumerate() {
        let Arm {
            attrs,
            pat,
            guard,
            body,
            ..
        } = arm;
        if !attrs.is_empty() || guard.is_some() {
            return Err(syn::Error::new_spanned(
                pat,
                "`pcx!` match arms do not yet support attributes or guards",
            )
            .into_compile_error()
            .into());
        }
        let literal = match visible_expression_literal(&body) {
            Ok(literal) => literal,
            Err(error) => return Err(error.into_compile_error().into()),
        };
        let style = compile_visible_literal(theme, &literal)?;
        selector_arms.push(quote! {
            #pat => #index
        });
        branches.push(CompiledBranch { literal, style });
    }

    Ok(CompiledClause {
        selector: quote! {
            match #expr {
                #(#selector_arms),*
            }
        },
        branches,
        span,
    })
}

fn validate_cross_clause_conflicts(clauses: &[CompiledClause]) -> Result<()> {
    let semantic_clauses = clauses
        .iter()
        .map(|clause| {
            clause
                .branches
                .iter()
                .map(|branch| branch.style.clone())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let Err(conflict) = pliego_css_compiler::analyze_cross_clause_conflicts(&semantic_clauses)
    else {
        return Ok(());
    };
    let slots = conflict
        .slots
        .iter()
        .map(|slot| format!("{slot:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let literal = &clauses[conflict.right_clause].branches[conflict.right_branch].literal;
    Err(syn::Error::new_spanned(
        literal,
        format!(
            "PCX003: independent clauses {} and {} can both assign [{slots}] under the same condition; express the combined state space in one `match`",
            conflict.left_clause + 1,
            conflict.right_clause + 1,
        ),
    ))
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

fn visible_block_literal(block: &Block) -> Result<LitStr> {
    let [Stmt::Expr(expression, None)] = block.stmts.as_slice() else {
        return Err(syn::Error::new_spanned(
            block,
            "PCX001: each conditional branch must contain exactly one visible string literal",
        ));
    };
    visible_expression_literal(expression)
}

fn visible_expression_literal(expression: &Expr) -> Result<LitStr> {
    let Expr::Lit(ExprLit {
        lit: Lit::Str(literal),
        ..
    }) = expression
    else {
        return Err(syn::Error::new_spanned(
            expression,
            "PCX001: conditional branches must be visible Rust string literals",
        ));
    };
    Ok(literal.clone())
}

fn compile_visible_literal(
    theme: &ThemeRegistry,
    literal: &LitStr,
) -> core::result::Result<SemanticStyle, TokenStream> {
    let source = literal.value();
    let syntax = pliego_css_parser::parse_style_list(&source)
        .map_err(|diagnostic| compile_error(literal, diagnostic))?;
    pliego_css_compiler::lower_style_with_theme(theme, &syntax)
        .map_err(|diagnostic| compile_error(literal, diagnostic))
}

fn active_theme(literal: &LitStr) -> core::result::Result<Arc<ThemeRegistry>, TokenStream> {
    let theme = if let Some(path) = std::env::var_os(THEME_PATH_ENV) {
        let path = PathBuf::from(path);
        cached_theme(literal, path)?
    } else {
        static SEED: OnceLock<Arc<ThemeRegistry>> = OnceLock::new();
        Arc::clone(SEED.get_or_init(|| Arc::new(ThemeRegistry::seed())))
    };

    if let Some(expected) = std::env::var_os(THEME_ID_ENV) {
        let expected = expected.to_string_lossy();
        let parsed = parse_theme_id(&expected).ok_or_else(|| {
            theme_error(
                literal,
                &format!(
                    "`{THEME_ID_ENV}` must contain exactly 32 hexadecimal characters, found `{expected}`"
                ),
            )
        })?;
        if parsed != theme.id().get() {
            return Err(theme_error(
                literal,
                &format!(
                    "theme artifact identity {:032x} does not match `{THEME_ID_ENV}` value {parsed:032x}",
                    theme.id().get(),
                ),
            ));
        }
    }
    Ok(theme)
}

fn cached_theme(
    literal: &LitStr,
    path: PathBuf,
) -> core::result::Result<Arc<ThemeRegistry>, TokenStream> {
    static THEMES: OnceLock<Mutex<BTreeMap<PathBuf, Arc<ThemeRegistry>>>> = OnceLock::new();
    let cache = THEMES.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(theme) = cache
        .lock()
        .map_err(|_| theme_error(literal, "theme cache lock is poisoned"))?
        .get(&path)
        .cloned()
    {
        return Ok(theme);
    }

    let bytes = fs::read(&path).map_err(|error| {
        theme_error(
            literal,
            &format!(
                "could not read `{THEME_PATH_ENV}` artifact `{}`: {error}",
                path.display()
            ),
        )
    })?;
    let theme = Arc::new(ThemeRegistry::from_bytes(&bytes).map_err(|error| {
        theme_error(
            literal,
            &format!(
                "could not decode `{THEME_PATH_ENV}` artifact `{}`: {error}",
                path.display()
            ),
        )
    })?);
    cache
        .lock()
        .map_err(|_| theme_error(literal, "theme cache lock is poisoned"))?
        .insert(path, Arc::clone(&theme));
    Ok(theme)
}

fn parse_theme_id(value: &str) -> Option<u128> {
    (value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| u128::from_str_radix(value, 16).ok())
        .flatten()
}

fn theme_error(literal: &LitStr, message: &str) -> TokenStream {
    syn::Error::new(literal.span(), format!("PliegoCSS theme error: {message}"))
        .into_compile_error()
        .into()
}

fn compile_error(literal: &LitStr, diagnostic: pliego_css_ir::Diagnostic) -> TokenStream {
    let code = diagnostic.code.as_str();
    let message = diagnostic.message;
    let span = diagnostic.span;
    let suggestion = diagnostic
        .suggestion
        .map_or_else(String::new, |suggestion| format!("; try `{suggestion}`"));
    syn::Error::new(
        literal.span(),
        format!(
            "{code}: {message} at bytes {}..{}{suggestion}",
            span.start, span.end
        ),
    )
    .into_compile_error()
    .into()
}
