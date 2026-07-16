//! Fixed-seed property contracts for parser, semantic identity, binary IR, and concurrency.

#![forbid(unsafe_code)]

use std::sync::Arc;

use pliego_css_compiler::{
    emit_css_with_theme, encode_style_ir_with_theme, lower_style_with_theme,
    try_encode_style_identity_with_theme, utility_catalog,
};
use pliego_css_ir::SemanticStyle;
use pliego_css_parser::{format_style_list, parse_style_list};
use pliego_css_theme::ThemeRegistry;

const PROPERTY_CASES: usize = 2_048;
const CONCURRENT_CASES: usize = 192;
const THREAD_COUNT: usize = 16;

const CORPUS_SEED: u64 = 0x504c_4945_474f_4353;
const PERMUTATION_SEED: u64 = 0x5354_594c_4549_4431;
const CONCURRENCY_SEED: u64 = 0x434f_4e43_5552_5245;

const VARIANT_PREFIXES: &[&str] = &[
    "",
    "hover:",
    "focus-visible:",
    "md:",
    "dark:",
    "md:hover:",
    "dark:md:hover:",
    "motion-reduce:contrast-more:",
    "[&>p]:",
];

const OUTER_WHITESPACE: &[&str] = &["", " ", "  ", "\t", "\n", " \n\t"];
const ITEM_WHITESPACE: &[&str] = &[" ", "  ", "\t", "\n", " \n ", "\t \t"];

// Every entry owns a distinct semantic slot footprint. Some emit multiple physical
// declarations, but none overrides another entry in this set.
const NON_CONFLICTING: &[&str] = &[
    "antialiased",
    "aspect-square",
    "bg-accent/20",
    "border-2",
    "cursor-pointer",
    "flex",
    "flex-col",
    "font-semibold",
    "gap-4",
    "h-full",
    "items-center",
    "justify-between",
    "leading-6",
    "-m-4",
    "max-w-6xl",
    "min-h-0",
    "min-w-0",
    "opacity-50",
    "outline-2",
    "p-4",
    "resize-y",
    "rounded-lg",
    "shadow-md",
    "-tracking-tight",
    "transition-colors",
    "w-full",
];

#[derive(Debug, Eq, PartialEq)]
struct SemanticContract {
    style_id: u128,
    identity: Vec<u8>,
    css: String,
}

#[derive(Debug, Eq, PartialEq)]
struct FullContract {
    semantic: SemanticContract,
    ir: Vec<u8>,
}

#[derive(Clone, Copy)]
struct FixedRng(u64);

impl FixedRng {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.0 = value;
        value.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn index(&mut self, upper: usize) -> usize {
        assert!(upper > 0, "random index requires a non-empty domain");
        let upper = u64::try_from(upper).expect("usize must fit in u64 on supported targets");
        usize::try_from(self.next_u64() % upper).expect("bounded random index must fit in usize")
    }

    fn shuffle<T>(&mut self, values: &mut [T]) {
        for upper in (1..values.len()).rev() {
            let other = self.index(upper + 1);
            values.swap(upper, other);
        }
    }
}

fn lower(theme: &ThemeRegistry, source: &str) -> SemanticStyle {
    let syntax = parse_style_list(source)
        .unwrap_or_else(|error| panic!("property source `{source}` must parse: {error}"));
    lower_style_with_theme(theme, &syntax)
        .unwrap_or_else(|error| panic!("property source `{source}` must lower: {error}"))
}

fn semantic_contract(
    theme: &ThemeRegistry,
    style: &SemanticStyle,
    source: &str,
) -> SemanticContract {
    SemanticContract {
        style_id: style.id.get(),
        identity: try_encode_style_identity_with_theme(theme, style).unwrap_or_else(|error| {
            panic!("property source `{source}` must encode an identity: {error}")
        }),
        css: emit_css_with_theme(theme, style)
            .unwrap_or_else(|error| panic!("property source `{source}` must emit: {error}")),
    }
}

fn full_contract(theme: &ThemeRegistry, source: &str) -> FullContract {
    let style = lower(theme, source);
    FullContract {
        semantic: semantic_contract(theme, &style, source),
        ir: encode_style_ir_with_theme(theme, &style)
            .unwrap_or_else(|error| panic!("property source `{source}` must encode IR: {error}")),
    }
}

fn catalog_source(rng: &mut FixedRng) -> String {
    let catalog = utility_catalog();
    let descriptor = &catalog[rng.index(catalog.len())];
    let leading = OUTER_WHITESPACE[rng.index(OUTER_WHITESPACE.len())];
    let trailing = OUTER_WHITESPACE[rng.index(OUTER_WHITESPACE.len())];
    let variant = VARIANT_PREFIXES[rng.index(VARIANT_PREFIXES.len())];

    format!("{leading}{variant}{}{trailing}", descriptor.example())
}

fn join_with_random_whitespace(values: &[&str], rng: &mut FixedRng) -> String {
    let mut source = String::new();
    source.push_str(OUTER_WHITESPACE[rng.index(OUTER_WHITESPACE.len())]);
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            source.push_str(ITEM_WHITESPACE[rng.index(ITEM_WHITESPACE.len())]);
        }
        source.push_str(value);
    }
    source.push_str(OUTER_WHITESPACE[rng.index(OUTER_WHITESPACE.len())]);
    source
}

#[test]
fn fixed_seed_corpus_preserves_format_semantics_and_ir_bytes() {
    let theme = ThemeRegistry::seed();
    let mut rng = FixedRng::new(CORPUS_SEED);

    for case_index in 0..PROPERTY_CASES {
        let source = catalog_source(&mut rng);
        let parsed = parse_style_list(&source)
            .unwrap_or_else(|error| panic!("case {case_index} `{source}` must parse: {error}"));
        let formatted_once = format_style_list(&parsed);
        let reparsed = parse_style_list(&formatted_once).unwrap_or_else(|error| {
            panic!("case {case_index} formatted `{formatted_once}` must parse: {error}")
        });
        let formatted_twice = format_style_list(&reparsed);
        let reparsed_twice = parse_style_list(&formatted_twice).unwrap_or_else(|error| {
            panic!("case {case_index} reformatted `{formatted_twice}` must parse: {error}")
        });

        assert_eq!(
            formatted_once, formatted_twice,
            "case {case_index} formatter must be idempotent"
        );
        assert_eq!(
            reparsed, reparsed_twice,
            "case {case_index} canonical parse tree must be stable"
        );

        let authored = lower_style_with_theme(&theme, &parsed).unwrap_or_else(|error| {
            panic!("case {case_index} authored `{source}` must lower: {error}")
        });
        let canonical = lower_style_with_theme(&theme, &reparsed).unwrap_or_else(|error| {
            panic!("case {case_index} canonical `{formatted_once}` must lower: {error}")
        });
        assert_eq!(
            semantic_contract(&theme, &authored, &source),
            semantic_contract(&theme, &canonical, &formatted_once),
            "case {case_index} formatting must preserve semantic identity and CSS"
        );

        let encoded = encode_style_ir_with_theme(&theme, &authored)
            .unwrap_or_else(|error| panic!("case {case_index} `{source}` must encode IR: {error}"));
        let decoded = pliego_css_compiler::decode_style_ir_with_theme(&theme, &encoded)
            .unwrap_or_else(|error| panic!("case {case_index} `{source}` must decode IR: {error}"));
        let reencoded = encode_style_ir_with_theme(&theme, &decoded).unwrap_or_else(|error| {
            panic!("case {case_index} `{source}` decoded IR must re-encode: {error}")
        });

        assert_eq!(decoded, authored, "case {case_index} IR must round-trip");
        assert_eq!(
            reencoded, encoded,
            "case {case_index} encode/decode/encode must be byte exact"
        );
    }
}

#[test]
fn non_conflicting_permutations_have_one_identity_and_css() {
    let theme = ThemeRegistry::seed();
    let mut rng = FixedRng::new(PERMUTATION_SEED);

    for case_index in 0..PROPERTY_CASES {
        let mut selected = NON_CONFLICTING.to_vec();
        rng.shuffle(&mut selected);
        selected.truncate(4 + rng.index(9));

        let mut canonical = selected.clone();
        canonical.sort_unstable();
        let canonical_source = join_with_random_whitespace(&canonical, &mut rng);
        let canonical_style = lower(&theme, &canonical_source);
        let expected = semantic_contract(&theme, &canonical_style, &canonical_source);

        rng.shuffle(&mut selected);
        let permuted_source = join_with_random_whitespace(&selected, &mut rng);
        let permuted_style = lower(&theme, &permuted_source);
        let actual = semantic_contract(&theme, &permuted_style, &permuted_source);

        assert_eq!(
            actual, expected,
            "case {case_index} non-conflicting permutation must preserve StyleId, identity, and CSS"
        );
    }
}

#[test]
fn shared_theme_is_deterministic_across_sixteen_threads() {
    let theme = Arc::new(ThemeRegistry::seed());
    let mut rng = FixedRng::new(CONCURRENCY_SEED);
    let sources = Arc::new(
        (0..CONCURRENT_CASES)
            .map(|_| catalog_source(&mut rng))
            .collect::<Vec<_>>(),
    );
    let expected = sources
        .iter()
        .map(|source| full_contract(theme.as_ref(), source))
        .collect::<Vec<_>>();

    let handles = (0..THREAD_COUNT)
        .map(|_| {
            let theme = Arc::clone(&theme);
            let sources = Arc::clone(&sources);
            std::thread::spawn(move || {
                sources
                    .iter()
                    .map(|source| full_contract(theme.as_ref(), source))
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();

    for (thread_index, handle) in handles.into_iter().enumerate() {
        let actual = handle
            .join()
            .unwrap_or_else(|_| panic!("worker {thread_index} must not panic"));
        for (case_index, (actual_case, expected_case)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(
                actual_case, expected_case,
                "worker {thread_index}, case {case_index} must match sequential output"
            );
        }
    }
}
