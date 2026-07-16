//! Build-script bridge for custom `PliegoCSS` themes.
//!
//! Add this crate under `[build-dependencies]`, create a `build.rs`, and point it
//! at the project's theme configuration:
//!
//! ```no_run
//! pliego_css_build::theme!("pliego.theme.toml");
//! ```
//!
//! Or select one validated permutation from a DTCG Resolver 2025.10 document:
//!
//! ```no_run
//! pliego_css_build::theme!(
//!     tokens = "product.resolver.json",
//!     inputs = {
//!         "appearance" => "dark",
//!         "density" => "compact",
//!     },
//! );
//! ```
//!
//! The bridge resolves relative paths against `CARGO_MANIFEST_DIR`, parses the
//! theme source only at build time, writes a validated canonical binary artifact
//! into `OUT_DIR`, and exports [`THEME_PATH_ENV`] plus [`THEME_ID_ENV`] to the
//! crate being compiled. Runtime and procedural-macro dependency graphs therefore
//! do not need theme-source parsers.
//!
//! The supported candidate surface is [`theme!`], which returns `()`, plus [`THEME_PATH_ENV`] and
//! [`THEME_ID_ENV`] as the versioned handoff names. Helper functions and result types remain hidden
//! implementation hooks used by macro expansion and build-system tests.

#![forbid(unsafe_code)]

#[cfg(feature = "usage-artifacts")]
pub mod artifacts;

use std::env;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use pliego_css_config::{ConfigError, DtcgError, parse_dtcg_resolver_path};
use pliego_css_theme::{THEME_BINARY_FORMAT_VERSION, ThemeBinaryError, ThemeId, ThemeRegistry};

/// Environment variable containing the canonical binary artifact path.
pub const THEME_PATH_ENV: &str = "PLIEGO_CSS_THEME_PATH";

/// Environment variable containing the 32-character lowercase theme identity.
pub const THEME_ID_ENV: &str = "PLIEGO_CSS_THEME_ID";

const ARTIFACT_PREFIX: &str = "pliego-css-theme";
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Result of preparing a canonical theme artifact.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeArtifact {
    path: PathBuf,
    theme_id: ThemeId,
}

impl ThemeArtifact {
    /// Returns the absolute or caller-provided output path of the binary artifact.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the canonical registry identity embedded in the artifact.
    #[must_use]
    pub const fn theme_id(&self) -> ThemeId {
        self.theme_id
    }

    /// Returns the theme identity as exactly 32 lowercase hexadecimal characters.
    #[must_use]
    pub fn theme_id_hex(&self) -> String {
        format!("{:032x}", self.theme_id.get())
    }
}

/// Error returned while preparing a build-time theme artifact.
#[doc(hidden)]
#[non_exhaustive]
#[derive(Debug)]
pub enum BuildError {
    /// Cargo did not provide a required build-script environment variable.
    MissingEnvironment {
        /// Missing variable name.
        name: &'static str,
    },
    /// The theme configuration could not be loaded or validated.
    Config(ConfigError),
    /// The DTCG Resolver document or selected inputs could not be loaded or validated.
    Dtcg(DtcgError),
    /// The validated registry could not be encoded.
    Encode(ThemeBinaryError),
    /// An output filesystem operation failed.
    Io {
        /// Operation being attempted.
        operation: &'static str,
        /// Affected path.
        path: PathBuf,
        /// Underlying I/O error.
        source: io::Error,
    },
    /// A content-addressed artifact already exists with unexpected bytes.
    ArtifactConflict {
        /// Conflicting artifact path.
        path: PathBuf,
    },
    /// Cargo cannot transport a non-UTF-8 artifact path through `rustc-env`.
    NonUtf8ArtifactPath {
        /// Artifact path that cannot be represented losslessly.
        path: PathBuf,
    },
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEnvironment { name } => {
                write!(formatter, "Cargo build environment is missing `{name}`")
            }
            Self::Config(source) => {
                write!(formatter, "failed to configure PliegoCSS theme: {source}")
            }
            Self::Dtcg(source) => {
                write!(
                    formatter,
                    "failed to configure PliegoCSS DTCG theme: {source}"
                )
            }
            Self::Encode(source) => write!(formatter, "failed to encode PliegoCSS theme: {source}"),
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} `{}`: {source}",
                path.display()
            ),
            Self::ArtifactConflict { path } => write!(
                formatter,
                "theme artifact `{}` exists with bytes that do not match its content identity",
                path.display()
            ),
            Self::NonUtf8ArtifactPath { path } => write!(
                formatter,
                "theme artifact path `{}` is not valid UTF-8 and cannot be exported to rustc",
                path.display()
            ),
        }
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(source) => Some(source),
            Self::Dtcg(source) => Some(source),
            Self::Encode(source) => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::MissingEnvironment { .. }
            | Self::ArtifactConflict { .. }
            | Self::NonUtf8ArtifactPath { .. } => None,
        }
    }
}

impl From<ConfigError> for BuildError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<DtcgError> for BuildError {
    fn from(error: DtcgError) -> Self {
        Self::Dtcg(error)
    }
}

impl From<ThemeBinaryError> for BuildError {
    fn from(error: ThemeBinaryError) -> Self {
        Self::Encode(error)
    }
}

/// Configures a theme from a Cargo build script.
///
/// Relative `path` values are resolved against `CARGO_MANIFEST_DIR`. The
/// function emits `cargo:rerun-if-changed`, writes the canonical binary artifact
/// atomically into `OUT_DIR`, and emits `cargo:rustc-env` directives for
/// [`THEME_PATH_ENV`] and [`THEME_ID_ENV`].
///
/// # Errors
///
/// Returns [`BuildError`] for missing Cargo environment, invalid configuration,
/// encoding failures, or output I/O failures.
#[doc(hidden)]
pub fn configure_theme(path: impl AsRef<Path>) -> Result<ThemeArtifact, BuildError> {
    let manifest_dir = required_path_environment("CARGO_MANIFEST_DIR")?;
    let out_dir = required_path_environment("OUT_DIR")?;
    let config_path = resolve_config_path(&manifest_dir, path.as_ref());

    println!("cargo:rerun-if-changed={}", config_path.display());
    let artifact = write_theme_artifact(&config_path, &out_dir)?;
    export_artifact(&artifact)?;
    Ok(artifact)
}

/// Configures one selected DTCG Resolver theme from a Cargo build script.
///
/// Relative `path` values are resolved against `CARGO_MANIFEST_DIR`. Input entries retain their
/// declaration order so exact and case-folded duplicate modifier names fail closed. An empty slice
/// selects every modifier default. The function tracks the exact Resolver document, writes only the
/// selected registry into `OUT_DIR`, and exports the same environment handoff as [`configure_theme`].
///
/// # Errors
///
/// Returns [`BuildError`] for missing Cargo environment, an invalid Resolver or selection,
/// encoding failures, or output I/O failures.
#[doc(hidden)]
pub fn configure_dtcg_theme(
    path: impl AsRef<Path>,
    inputs: &[(&str, &str)],
) -> Result<ThemeArtifact, BuildError> {
    let manifest_dir = required_path_environment("CARGO_MANIFEST_DIR")?;
    let out_dir = required_path_environment("OUT_DIR")?;
    let resolver_path = resolve_config_path(&manifest_dir, path.as_ref());

    println!("cargo:rerun-if-changed={}", resolver_path.display());
    let artifact = write_dtcg_theme_artifact(&resolver_path, inputs, &out_dir)?;
    export_artifact(&artifact)?;
    Ok(artifact)
}

fn export_artifact(artifact: &ThemeArtifact) -> Result<(), BuildError> {
    let artifact_path = artifact
        .path
        .to_str()
        .ok_or_else(|| BuildError::NonUtf8ArtifactPath {
            path: artifact.path.clone(),
        })?;
    println!("cargo:rustc-env={THEME_PATH_ENV}={artifact_path}");
    println!("cargo:rustc-env={THEME_ID_ENV}={}", artifact.theme_id_hex());
    Ok(())
}

/// Parses `config_path` and atomically prepares its canonical artifact in
/// `out_dir` without reading or mutating process environment.
///
/// This helper is useful to build-system adapters that provide their own path
/// context. The artifact filename is content-addressed by [`ThemeId`], so
/// repeated calls with identical input are stable and do not rewrite the file.
///
/// # Errors
///
/// Returns [`BuildError`] for invalid configuration, encoding failures, or
/// output I/O failures.
#[doc(hidden)]
pub fn write_theme_artifact(
    config_path: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
) -> Result<ThemeArtifact, BuildError> {
    let registry = pliego_css_config::parse_path(config_path)?;
    write_registry_artifact(&registry, out_dir.as_ref())
}

/// Parses `resolver_path`, selects one permutation from ordered `inputs`, and atomically prepares
/// its canonical artifact in `out_dir` without reading or mutating process environment.
///
/// Empty `inputs` use Resolver defaults. Entries are passed directly to
/// `DtcgResolver::resolve_entries`, preserving exact and case-folded duplicates so they are
/// rejected rather than silently overwritten.
///
/// # Errors
///
/// Returns [`BuildError`] for an invalid Resolver or selection, encoding failures, or output I/O
/// failures.
#[doc(hidden)]
pub fn write_dtcg_theme_artifact(
    resolver_path: impl AsRef<Path>,
    inputs: &[(&str, &str)],
    out_dir: impl AsRef<Path>,
) -> Result<ThemeArtifact, BuildError> {
    let resolver = parse_dtcg_resolver_path(resolver_path)?;
    let theme = resolver.resolve_entries(inputs.iter().copied())?;
    write_registry_artifact(theme.registry(), out_dir.as_ref())
}

/// Invokes the build-time theme bridge, returns `()`, and stops the build with a clear error on
/// failure.
///
/// This is the concise entrypoint intended for `build.rs`:
///
/// ```no_run
/// pliego_css_build::theme!("pliego.theme.toml");
/// ```
#[macro_export]
macro_rules! theme {
    (tokens = $path:expr, inputs = { $($name:literal => $context:literal),* $(,)? } $(,)?) => {{
        match $crate::configure_dtcg_theme($path, &[$(($name, $context)),*]) {
            Ok(_) => (),
            Err(error) => panic!("PliegoCSS DTCG theme configuration failed: {error}"),
        }
    }};
    ($path:expr $(,)?) => {{
        match $crate::configure_theme($path) {
            Ok(_) => (),
            Err(error) => panic!("PliegoCSS theme configuration failed: {error}"),
        }
    }};
}

fn required_path_environment(name: &'static str) -> Result<PathBuf, BuildError> {
    env::var_os(name)
        .map(PathBuf::from)
        .ok_or(BuildError::MissingEnvironment { name })
}

fn resolve_config_path(manifest_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        manifest_dir.join(path)
    }
}

fn write_registry_artifact(
    registry: &ThemeRegistry,
    out_dir: &Path,
) -> Result<ThemeArtifact, BuildError> {
    fs::create_dir_all(out_dir).map_err(|source| BuildError::Io {
        operation: "create theme output directory",
        path: out_dir.to_path_buf(),
        source,
    })?;

    let bytes = registry.to_bytes()?;
    let id_hex = format!("{:032x}", registry.id().get());
    let path = out_dir.join(format!(
        "{ARTIFACT_PREFIX}-v{THEME_BINARY_FORMAT_VERSION}-{id_hex}.bin"
    ));
    persist_content_addressed(&path, &bytes)?;
    Ok(ThemeArtifact {
        path,
        theme_id: registry.id(),
    })
}

fn persist_content_addressed(path: &Path, bytes: &[u8]) -> Result<(), BuildError> {
    if path.exists() {
        return verify_existing(path, bytes);
    }

    let temporary = create_temporary_sibling(path, bytes)?;
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(source) if path.exists() => {
            let _ = fs::remove_file(&temporary);
            verify_existing(path, bytes).map_err(|error| match error {
                BuildError::ArtifactConflict { .. } => error,
                _ => BuildError::Io {
                    operation: "persist canonical theme artifact",
                    path: path.to_path_buf(),
                    source,
                },
            })
        }
        Err(source) => {
            let _ = fs::remove_file(&temporary);
            Err(BuildError::Io {
                operation: "persist canonical theme artifact",
                path: path.to_path_buf(),
                source,
            })
        }
    }
}

fn create_temporary_sibling(path: &Path, bytes: &[u8]) -> Result<PathBuf, BuildError> {
    for _ in 0..32 {
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary = path.with_extension(format!("tmp-{}-{sequence}", std::process::id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(mut file) => {
                if let Err(source) = file.write_all(bytes).and_then(|()| file.sync_all()) {
                    let _ = fs::remove_file(&temporary);
                    return Err(BuildError::Io {
                        operation: "write temporary theme artifact",
                        path: temporary,
                        source,
                    });
                }
                return Ok(temporary);
            }
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(BuildError::Io {
                    operation: "create temporary theme artifact",
                    path: temporary,
                    source,
                });
            }
        }
    }

    Err(BuildError::Io {
        operation: "allocate temporary theme artifact",
        path: path.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary artifact name collision limit reached",
        ),
    })
}

fn verify_existing(path: &Path, expected: &[u8]) -> Result<(), BuildError> {
    let existing = fs::read(path).map_err(|source| BuildError::Io {
        operation: "read existing theme artifact",
        path: path.to_path_buf(),
        source,
    })?;
    if existing == expected {
        Ok(())
    } else {
        Err(BuildError::ArtifactConflict {
            path: path.to_path_buf(),
        })
    }
}

#[cfg(all(test, not(feature = "package-verify")))]
#[path = "../tests/internal/unit.rs"]
mod tests;
