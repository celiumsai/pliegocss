//! Build-time theme, DTCG adapter, token-graph, and budget configuration for `PliegoCSS`.
//!
//! TOML schema 1 and the DTCG 2025.10 same-document resolver are versioned contracts. Direct parser
//! use remains an exact-version advanced tooling surface; applications normally use the build and
//! command-line adapters once the corresponding selection surface is available.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use pliego_css_ir::{BreakpointId, TokenKind};
use pliego_css_theme::{BreakpointDefinition, ThemeError, ThemeRegistry, TokenDefinition};
use serde::Deserialize;

mod budget;
mod dtcg;
mod dtcg_resolver;
mod token_graph;

#[cfg(feature = "compatibility-data")]
#[doc(hidden)]
pub mod compatibility_data;

pub use budget::{
    AppliedBudgetException, BudgetError, BudgetEvaluation, BudgetEvaluationReport,
    BudgetMeasurements, BudgetMetric, BudgetObservation, BudgetPolicy, BudgetSubject,
    BudgetSubjectKind, CssSpecificity, evaluate_budget_policy, parse_budget_policy,
};
#[cfg(feature = "css-analysis")]
pub use budget::{
    CssBudgetInventory, CssBudgetMetrics, CssLayerBudgetMetrics, measure_css_budgets,
};
pub use dtcg::{
    DTCG_EXTENSION_KEY, DTCG_FORMAT_VERSION, DtcgError, DtcgReport, DtcgTheme, export_dtcg,
    parse_dtcg_path, parse_dtcg_str,
};
pub use dtcg_resolver::{
    DTCG_RESOLVER_PROFILE, DtcgResolver, parse_dtcg_resolver_path, parse_dtcg_resolver_str,
};
pub use token_graph::{
    TOKEN_GRAPH_VERSION, TokenExpression, TokenGraph, TokenGraphAdapter, TokenGraphError,
    TokenGraphProjection, TokenGraphReference, TokenGraphSource, TokenGraphTheme, TokenGraphToken,
    TokenReferenceSyntax, parse_token_graph,
};

/// Theme configuration schema supported by this crate.
pub const SCHEMA_VERSION: u32 = 1;
const MAX_CONFIG_BYTES: usize = 16 * 1024 * 1024;

/// Errors returned while loading a theme configuration.
#[non_exhaustive]
#[derive(Debug)]
pub enum ConfigError {
    /// The configuration file could not be read.
    Read {
        /// Path that failed to load.
        path: PathBuf,
        /// Underlying I/O error.
        source: io::Error,
    },
    /// The input is not valid TOML for the supported schema.
    Parse(toml::de::Error),
    /// The declared schema version is unsupported.
    UnsupportedSchema(u32),
    /// The requested base theme is unsupported.
    UnsupportedExtension(String),
    /// A token or breakpoint name cannot be normalized safely.
    InvalidName {
        /// Configuration table containing the name.
        table: &'static str,
        /// Original name from the TOML document.
        name: String,
        /// Human-readable reason.
        reason: &'static str,
    },
    /// Two input names normalize to the same canonical name.
    DuplicateName {
        /// Configuration table containing the duplicate.
        table: &'static str,
        /// Canonical kebab-case name.
        name: String,
    },
    /// A token or breakpoint value is unsafe or unsupported.
    InvalidValue {
        /// Configuration table containing the value.
        table: &'static str,
        /// Canonical token or breakpoint name.
        name: String,
        /// Human-readable reason.
        reason: &'static str,
    },
    /// The normalized definitions violate the theme registry contract.
    Registry(ThemeError),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(formatter, "failed to read `{}`: {source}", path.display())
            }
            Self::Parse(source) => write!(formatter, "invalid theme TOML: {source}"),
            Self::UnsupportedSchema(version) => write!(
                formatter,
                "unsupported theme schema {version}; expected {SCHEMA_VERSION}"
            ),
            Self::UnsupportedExtension(name) => {
                write!(
                    formatter,
                    "unsupported base theme `{name}`; expected `seed`"
                )
            }
            Self::InvalidName {
                table,
                name,
                reason,
            } => write!(formatter, "invalid name `{name}` in [{table}]: {reason}"),
            Self::DuplicateName { table, name } => {
                write!(formatter, "duplicate canonical name `{name}` in [{table}]")
            }
            Self::InvalidValue {
                table,
                name,
                reason,
            } => write!(
                formatter,
                "invalid value for `{name}` in [{table}]: {reason}"
            ),
            Self::Registry(source) => write!(formatter, "invalid theme registry: {source}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse(source) => Some(source),
            Self::Registry(source) => Some(source),
            Self::UnsupportedSchema(_)
            | Self::UnsupportedExtension(_)
            | Self::InvalidName { .. }
            | Self::DuplicateName { .. }
            | Self::InvalidValue { .. } => None,
        }
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(error: toml::de::Error) -> Self {
        Self::Parse(error)
    }
}

impl From<ThemeError> for ConfigError {
    fn from(error: ThemeError) -> Self {
        Self::Registry(error)
    }
}

/// Parses a TOML theme and constructs its normalized registry.
///
/// The format is intentionally build-only: configuration text does not need to
/// be shipped with an application at runtime.
///
/// # Errors
///
/// Returns [`ConfigError`] for malformed TOML, unsupported schema/base values,
/// unsafe names or values, and invalid registry definitions.
pub fn parse_str(source: &str) -> Result<ThemeRegistry, ConfigError> {
    let document: ThemeDocument = toml::from_str(source)?;
    document.build()
}

/// Reads and parses a TOML theme from `path`.
///
/// # Errors
///
/// Returns [`ConfigError`] when the file cannot be read or has invalid theme
/// configuration.
pub fn parse_path(path: impl AsRef<Path>) -> Result<ThemeRegistry, ConfigError> {
    let path = path.as_ref();
    let source =
        dtcg::read_bounded_utf8_document(path, "theme TOML exceeds 16 MiB").map_err(|error| {
            ConfigError::Read {
                path: path.to_path_buf(),
                source: io::Error::new(io::ErrorKind::InvalidInput, error.to_string()),
            }
        })?;
    debug_assert!(source.len() <= MAX_CONFIG_BYTES);
    parse_str(&source)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeDocument {
    schema: u32,
    extends: String,
    #[serde(default)]
    tokens: TokenTables,
    #[serde(default)]
    breakpoints: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct TokenTables {
    color: BTreeMap<String, String>,
    spacing: BTreeMap<String, String>,
    font_family: BTreeMap<String, String>,
    font_size: BTreeMap<String, String>,
    font_weight: BTreeMap<String, String>,
    line_height: BTreeMap<String, String>,
    letter_spacing: BTreeMap<String, String>,
    radius: BTreeMap<String, String>,
    shadow: BTreeMap<String, String>,
}

impl ThemeDocument {
    fn build(self) -> Result<ThemeRegistry, ConfigError> {
        if self.schema != SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedSchema(self.schema));
        }
        if self.extends != "seed" {
            return Err(ConfigError::UnsupportedExtension(self.extends));
        }

        let seed = ThemeRegistry::seed();
        let mut tokens = seed.tokens().to_vec();
        let mut breakpoints = seed.breakpoints().to_vec();

        self.apply_tokens(&mut tokens)?;
        apply_breakpoints(&mut breakpoints, self.breakpoints)?;

        ThemeRegistry::from_definitions(tokens, breakpoints).map_err(ConfigError::Registry)
    }

    fn apply_tokens(&self, tokens: &mut Vec<TokenDefinition>) -> Result<(), ConfigError> {
        let tables = [
            (TokenKind::Color, "tokens.color", &self.tokens.color),
            (TokenKind::Spacing, "tokens.spacing", &self.tokens.spacing),
            (
                TokenKind::FontFamily,
                "tokens.font-family",
                &self.tokens.font_family,
            ),
            (
                TokenKind::FontSize,
                "tokens.font-size",
                &self.tokens.font_size,
            ),
            (
                TokenKind::FontWeight,
                "tokens.font-weight",
                &self.tokens.font_weight,
            ),
            (
                TokenKind::LineHeight,
                "tokens.line-height",
                &self.tokens.line_height,
            ),
            (
                TokenKind::LetterSpacing,
                "tokens.letter-spacing",
                &self.tokens.letter_spacing,
            ),
            (TokenKind::Radius, "tokens.radius", &self.tokens.radius),
            (TokenKind::Shadow, "tokens.shadow", &self.tokens.shadow),
        ];
        for (kind, table, values) in tables {
            apply_token_table(tokens, kind, table, values)?;
        }
        Ok(())
    }
}

fn apply_token_table(
    definitions: &mut Vec<TokenDefinition>,
    kind: TokenKind,
    table: &'static str,
    values: &BTreeMap<String, String>,
) -> Result<(), ConfigError> {
    let mut normalized = BTreeMap::new();
    for (name, value) in values {
        let canonical = normalize_name(table, name)?;
        let value = normalize_value(table, &canonical, value)?;
        if normalized.insert(canonical.clone(), value).is_some() {
            return Err(ConfigError::DuplicateName {
                table,
                name: canonical,
            });
        }
    }

    for (name, value) in normalized {
        if let Some(existing) = definitions
            .iter_mut()
            .find(|definition| definition.kind == kind && definition.name == name)
        {
            *existing = TokenDefinition::with_id(kind, existing.id, name, value);
        } else {
            definitions.push(TokenDefinition::new(kind, name, value));
        }
    }
    Ok(())
}

fn apply_breakpoints(
    definitions: &mut Vec<BreakpointDefinition>,
    values: BTreeMap<String, String>,
) -> Result<(), ConfigError> {
    let table = "breakpoints";
    let mut normalized = BTreeMap::new();
    for (name, value) in values {
        let canonical = normalize_name(table, &name)?;
        let value = normalize_breakpoint(table, &canonical, &value)?;
        if normalized.insert(canonical.clone(), value).is_some() {
            return Err(ConfigError::DuplicateName {
                table,
                name: canonical,
            });
        }
    }

    let mut next_id = definitions
        .iter()
        .map(|definition| definition.id.get())
        .max()
        .unwrap_or_default()
        .checked_add(1)
        .ok_or(ConfigError::InvalidValue {
            table,
            name: "<registry>".to_owned(),
            reason: "breakpoint ID space is exhausted",
        })?;
    for (name, min_width) in normalized {
        if let Some(existing) = definitions
            .iter_mut()
            .find(|definition| definition.name == name)
        {
            *existing =
                BreakpointDefinition::new(existing.id, existing.cascade_rank, name, min_width);
        } else {
            definitions.push(BreakpointDefinition::new(
                BreakpointId::new(next_id),
                0,
                name,
                min_width,
            ));
            next_id = next_id.checked_add(1).ok_or(ConfigError::InvalidValue {
                table,
                name: "<registry>".to_owned(),
                reason: "breakpoint ID space is exhausted",
            })?;
        }
    }
    rank_breakpoints(definitions).map_err(|error| ConfigError::InvalidValue {
        table,
        name: error.name,
        reason: error.reason,
    })
}

#[derive(Debug)]
struct BreakpointOrderError {
    name: String,
    reason: &'static str,
}

fn rank_breakpoints(definitions: &mut [BreakpointDefinition]) -> Result<(), BreakpointOrderError> {
    let mut ranked = Vec::with_capacity(definitions.len());
    let mut common_unit = None;
    for (index, definition) in definitions.iter().enumerate() {
        let Some((number, unit)) = breakpoint_number_and_unit(&definition.min_width) else {
            return Err(BreakpointOrderError {
                name: definition.name.clone(),
                reason: "breakpoint must be a positive CSS length in px, rem, em, ch, or vw",
            });
        };
        if let Some(expected) = common_unit {
            if expected != unit {
                return Err(BreakpointOrderError {
                    name: definition.name.clone(),
                    reason: "all breakpoints must use one common unit so cascade order is unambiguous",
                });
            }
        } else {
            common_unit = Some(unit);
        }
        ranked.push((index, number, definition.name.clone(), definition.id));
    }
    ranked.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
    });
    for (rank, (index, _, _, _)) in ranked.into_iter().enumerate() {
        definitions[index].cascade_rank =
            u16::try_from(rank).map_err(|_| BreakpointOrderError {
                name: "<registry>".to_owned(),
                reason: "breakpoint cascade rank space is exhausted",
            })?;
    }
    Ok(())
}

fn breakpoint_number_and_unit(value: &str) -> Option<(f64, &str)> {
    let split = value
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let number = number
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite() && *number > 0.0)?;
    matches!(unit, "px" | "rem" | "em" | "ch" | "vw").then_some((number, unit))
}

fn normalize_name(table: &'static str, name: &str) -> Result<String, ConfigError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ConfigError::InvalidName {
            table,
            name: String::new(),
            reason: "name must not be empty",
        });
    }

    let mut normalized = String::with_capacity(name.len());
    let mut separator = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !normalized.is_empty() {
                normalized.push('-');
            }
            separator = false;
            normalized.push(character.to_ascii_lowercase());
        } else if character == '-' || character == '_' || character.is_ascii_whitespace() {
            separator = true;
        } else {
            return Err(ConfigError::InvalidName {
                table,
                name: name.to_owned(),
                reason: "use only ASCII letters, digits, hyphens, underscores, or spaces",
            });
        }
    }

    if separator || normalized.is_empty() {
        return Err(ConfigError::InvalidName {
            table,
            name: name.to_owned(),
            reason: "name must not end with a separator",
        });
    }
    Ok(normalized)
}

fn normalize_value(table: &'static str, name: &str, value: &str) -> Result<String, ConfigError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ConfigError::InvalidValue {
            table,
            name: name.to_owned(),
            reason: "value must not be empty",
        });
    }
    if value.chars().any(char::is_control)
        || value.contains([';', '{', '}'])
        || value.contains("/*")
        || value.contains("*/")
        || value.to_ascii_lowercase().contains("!important")
    {
        return Err(ConfigError::InvalidValue {
            table,
            name: name.to_owned(),
            reason: "value contains CSS rule delimiters, comments, controls, or `!important`",
        });
    }
    Ok(value.to_owned())
}

fn normalize_breakpoint(
    table: &'static str,
    name: &str,
    value: &str,
) -> Result<String, ConfigError> {
    let value = normalize_value(table, name, value)?;
    let split = value
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let valid_number = number
        .parse::<f64>()
        .is_ok_and(|number| number.is_finite() && number > 0.0);
    if !valid_number || !matches!(unit, "px" | "rem" | "em" | "ch" | "vw") {
        return Err(ConfigError::InvalidValue {
            table,
            name: name.to_owned(),
            reason: "breakpoint must be a positive CSS length in px, rem, em, ch, or vw",
        });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const MINIMAL: &str = "schema = 1\nextends = \"seed\"\n";

    #[test]
    fn reordered_configuration_has_the_same_theme_id() {
        let first = parse_str(
            r##"
schema = 1
extends = "seed"

[tokens.color]
brand = "#3366ff"
canvas = "#ffffff"

[tokens.spacing]
gutter = "1.5rem"

[breakpoints]
tablet = "48rem"
wide = "80rem"
"##,
        )
        .expect("first theme");
        let second = parse_str(
            r##"
extends = "seed"
schema = 1

[breakpoints]
wide = "80rem"
tablet = "48rem"

[tokens.spacing]
gutter = "1.5rem"

[tokens.color]
canvas = "#ffffff"
brand = "#3366ff"
"##,
        )
        .expect("second theme");

        assert_eq!(first.id(), second.id());
    }

    #[test]
    fn custom_tokens_and_breakpoints_are_normalized() {
        let registry = parse_str(
            r##"
schema = 1
extends = "seed"

[tokens.color]
Brand_Blue = "#3366ff"

[tokens.font-family]
Display_Font = "Inter, sans-serif"

[breakpoints]
Content_Wide = "72rem"
"##,
        )
        .expect("custom theme");

        assert!(
            registry
                .token_by_name(TokenKind::Color, "brand-blue")
                .is_some()
        );
        assert!(
            registry
                .token_by_name(TokenKind::FontFamily, "display-font")
                .is_some()
        );
        assert!(registry.breakpoint_by_name("content-wide").is_some());
    }

    #[test]
    fn breakpoint_ids_do_not_determine_narrow_to_wide_cascade_ranks() {
        let registry = parse_str(
            r#"
schema = 1
extends = "seed"

[breakpoints]
content-wide = "72rem"
tablet = "52rem"
"#,
        )
        .expect("ordered breakpoints");
        let tablet = registry.breakpoint_by_name("tablet").expect("tablet");
        let content_wide = registry
            .breakpoint_by_name("content-wide")
            .expect("content-wide");

        assert!(
            content_wide.id < tablet.id,
            "IDs remain stable by canonical name"
        );
        assert!(
            tablet.cascade_rank < content_wide.cascade_rank,
            "cascade rank follows 52rem before 72rem"
        );
    }

    #[test]
    fn mixed_breakpoint_units_fail_closed_instead_of_guessing_cascade_order() {
        let error = parse_str(
            r#"
schema = 1
extends = "seed"

[breakpoints]
tablet = "900px"
"#,
        )
        .expect_err("mixed units must be ambiguous");
        assert!(matches!(error, ConfigError::InvalidValue { .. }));
        assert!(error.to_string().contains("one common unit"));
    }

    #[test]
    fn seed_tokens_can_be_overridden() {
        let registry = parse_str(
            r##"
schema = 1
extends = "seed"

[tokens.color]
accent = "#ff00aa"
"##,
        )
        .expect("override theme");

        let accent = registry
            .token_by_name(TokenKind::Color, "accent")
            .expect("accent token");
        assert_eq!(accent.value, "#ff00aa");
    }

    #[test]
    fn unsupported_schema_is_clear() {
        let error = parse_str("schema = 2\nextends = \"seed\"\n").expect_err("schema must fail");
        assert!(matches!(error, ConfigError::UnsupportedSchema(2)));
        assert!(error.to_string().contains("expected 1"));
    }

    #[test]
    fn invalid_name_is_rejected() {
        let error = parse_str(
            r##"
schema = 1
extends = "seed"

[tokens.color]
"brand/blue" = "#3366ff"
"##,
        )
        .expect_err("invalid name must fail");
        assert!(matches!(error, ConfigError::InvalidName { .. }));
    }

    #[test]
    fn duplicate_normalized_name_is_rejected() {
        let error = parse_str(
            r##"
schema = 1
extends = "seed"

[tokens.color]
brand_blue = "#3366ff"
brand-blue = "#2244cc"
"##,
        )
        .expect_err("normalized duplicate must fail");
        assert!(matches!(error, ConfigError::DuplicateName { .. }));
    }

    #[test]
    fn invalid_value_is_rejected() {
        let error = parse_str(
            r#"
schema = 1
extends = "seed"

[tokens.shadow]
card = "0 1px black; color: red"
"#,
        )
        .expect_err("unsafe value must fail");
        assert!(matches!(error, ConfigError::InvalidValue { .. }));
    }

    #[test]
    fn invalid_breakpoint_value_is_rejected() {
        for value in ["wide", "5.rem"] {
            let error = parse_str(&format!(
                r#"
schema = 1
extends = "seed"

[breakpoints]
tablet = "{value}"
"#
            ))
            .expect_err("invalid breakpoint must fail");
            assert!(matches!(
                error,
                ConfigError::InvalidValue { .. } | ConfigError::Registry(_)
            ));
        }
    }

    #[test]
    fn token_values_must_match_their_typed_namespace() {
        for source in [
            "[tokens.color]\nbrand = \"1rem\"",
            "[tokens.spacing]\ngutter = \"red\"",
            "[tokens.font-weight]\nbook = \"heavy-ish\"",
        ] {
            let error = parse_str(&format!("schema = 1\nextends = \"seed\"\n\n{source}\n"))
                .expect_err("wrong token domain must fail");
            assert!(matches!(
                error,
                ConfigError::Registry(ThemeError::InvalidDefinition { .. })
            ));
        }
    }

    #[test]
    fn breakpoint_cannot_shadow_a_builtin_condition_variant() {
        let error = parse_str(
            r#"
schema = 1
extends = "seed"

[breakpoints]
hover = "48rem"
"#,
        )
        .expect_err("reserved condition name must fail");

        assert!(matches!(
            error,
            ConfigError::Registry(ThemeError::ReservedBreakpointName { .. })
        ));
        assert!(error.to_string().contains("reserved"));
    }

    #[test]
    fn configuration_can_be_loaded_from_a_path() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("pliego-theme-{nonce}.toml"));
        fs::write(&path, MINIMAL).expect("write fixture");
        let registry = parse_path(&path).expect("parse fixture path");
        fs::remove_file(path).expect("remove fixture");
        assert_eq!(registry.id(), ThemeRegistry::seed().id());
    }

    #[test]
    fn configuration_path_rejects_oversized_files() {
        let path =
            std::env::temp_dir().join(format!("pliego-theme-large-{}.toml", std::process::id()));
        fs::write(&path, vec![b' '; 16 * 1024 * 1024 + 1]).unwrap();
        let error = parse_path(&path).expect_err("oversized TOML must fail");
        fs::remove_file(path).unwrap();
        assert!(error.to_string().contains("16 MiB"));
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn configuration_path_rejects_links() {
        let root = std::env::temp_dir().join(format!("pliego-theme-link-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("target.toml");
        let link = root.join("link.toml");
        fs::write(&target, MINIMAL).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        #[cfg(windows)]
        if std::os::windows::fs::symlink_file(&target, &link).is_err() {
            fs::remove_dir_all(root).unwrap();
            return;
        }
        let error = parse_path(&link).expect_err("linked TOML must fail");
        fs::remove_dir_all(root).unwrap();
        assert!(error.to_string().contains("symbolic link or reparse point"));
    }
}
