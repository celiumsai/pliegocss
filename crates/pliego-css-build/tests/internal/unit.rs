//! Internal build bridge tests.
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

const DTCG_RESOLVER_FIXTURE: &str = r##"
{
  "version": "2025.10",
  "name": "build bridge themes",
  "$defs": {
    "foundation": {
      "color": {
        "$type": "color",
        "brand": {
          "$value": {
            "colorSpace": "srgb",
            "components": [0.4, 0.5, 0.6],
            "alpha": 1
          }
        },
        "action": {"$value": "{color.brand}"}
      }
    },
    "light": {
      "color": {
        "$type": "color",
        "brand": {
          "$value": {
            "colorSpace": "srgb",
            "components": [0.9, 0.9, 0.9],
            "alpha": 1
          }
        }
      }
    },
    "dark": {
      "color": {
        "$type": "color",
        "brand": {
          "$value": {
            "colorSpace": "srgb",
            "components": [0.1, 0.1, 0.1],
            "alpha": 1
          }
        }
      }
    }
  },
  "sets": {
    "foundation": {
      "sources": [{"$ref": "#/$defs/foundation"}]
    }
  },
  "modifiers": {
    "appearance": {
      "contexts": {
        "light": [{"$ref": "#/$defs/light"}],
        "dark": [{"$ref": "#/$defs/dark"}]
      },
      "default": "light"
    }
  },
  "resolutionOrder": [
    {"$ref": "#/sets/foundation"},
    {"$ref": "#/modifiers/appearance"}
  ]
}
"##;

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn create(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "pliego-css-build-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary test directory");
        Self(path)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_config(root: &Path) -> PathBuf {
    let config = root.join("pliego.theme.toml");
    fs::write(
        &config,
        r##"
schema = 1
extends = "seed"

[tokens.color]
brand = "#3366ff"

[breakpoints]
tablet = "52rem"
"##,
    )
    .expect("write config fixture");
    config
}

fn write_dtcg_resolver(root: &Path) -> PathBuf {
    let resolver = root.join("product.resolver.json");
    fs::write(&resolver, DTCG_RESOLVER_FIXTURE).expect("write DTCG Resolver fixture");
    resolver
}

fn decode_artifact(artifact: &ThemeArtifact) -> ThemeRegistry {
    let bytes = fs::read(artifact.path()).expect("read artifact");
    ThemeRegistry::from_bytes(&bytes).expect("decode artifact")
}

fn token_value<'a>(registry: &'a ThemeRegistry, name: &str) -> &'a str {
    registry
        .tokens()
        .iter()
        .find(|token| token.name == name)
        .map(|token| token.value.as_str())
        .expect("fixture token")
}

#[test]
fn theme_environment_names_are_frozen() {
    assert_eq!(THEME_PATH_ENV, "PLIEGO_CSS_THEME_PATH");
    assert_eq!(THEME_ID_ENV, "PLIEGO_CSS_THEME_ID");
}

#[test]
fn helper_writes_a_self_validating_content_addressed_artifact() {
    let root = TempDirectory::create("artifact");
    let config = write_config(&root.0);
    let output = root.0.join("out");

    let artifact = write_theme_artifact(&config, &output).expect("write artifact");
    let bytes = fs::read(artifact.path()).expect("read artifact");
    let decoded = ThemeRegistry::from_bytes(&bytes).expect("decode artifact");

    assert_eq!(decoded.id(), artifact.theme_id());
    assert_eq!(artifact.theme_id_hex().len(), 32);
    assert_eq!(
        artifact.path().file_name().and_then(|name| name.to_str()),
        Some(
            format!(
                "{ARTIFACT_PREFIX}-v{THEME_BINARY_FORMAT_VERSION}-{}.bin",
                artifact.theme_id_hex()
            )
            .as_str()
        )
    );
}

#[test]
fn repeated_helper_calls_keep_the_same_artifact() {
    let root = TempDirectory::create("stable");
    let config = write_config(&root.0);
    let output = root.0.join("out");

    let first = write_theme_artifact(&config, &output).expect("first artifact");
    let second = write_theme_artifact(&config, &output).expect("second artifact");

    assert_eq!(first, second);
    assert_eq!(fs::read_dir(output).expect("list output").count(), 1);
}

#[test]
fn path_resolution_is_relative_to_manifest_and_preserves_absolute_paths() {
    let manifest = Path::new("workspace/example");
    assert_eq!(
        resolve_config_path(manifest, Path::new("theme.toml")),
        manifest.join("theme.toml")
    );

    let absolute = env::current_dir().unwrap().join("theme.toml");
    assert_eq!(resolve_config_path(manifest, &absolute), absolute);
}

#[test]
fn invalid_configuration_has_build_context() {
    let root = TempDirectory::create("invalid");
    let config = root.0.join("bad.toml");
    fs::write(&config, "schema = 99\nextends = \"seed\"\n").unwrap();

    let error = write_theme_artifact(config, root.0.join("out")).unwrap_err();
    assert!(matches!(error, BuildError::Config(_)));
    assert!(
        error
            .to_string()
            .contains("failed to configure PliegoCSS theme")
    );
}

#[test]
fn dtcg_defaults_and_dark_input_write_only_the_selected_registry() {
    let root = TempDirectory::create("dtcg-selection");
    let resolver = write_dtcg_resolver(&root.0);
    let output = root.0.join("out");

    let default = write_dtcg_theme_artifact(&resolver, &[], &output).expect("default artifact");
    let explicit_light = write_dtcg_theme_artifact(&resolver, &[("appearance", "light")], &output)
        .expect("light artifact");
    let dark = write_dtcg_theme_artifact(&resolver, &[("appearance", "dark")], &output)
        .expect("dark artifact");

    assert_eq!(default, explicit_light);
    assert_ne!(default.theme_id(), dark.theme_id());
    assert_eq!(fs::read_dir(&output).expect("list output").count(), 2);

    let light_registry = decode_artifact(&default);
    let dark_registry = decode_artifact(&dark);
    assert_ne!(
        token_value(&light_registry, "brand"),
        token_value(&dark_registry, "brand")
    );
    assert_eq!(
        token_value(&light_registry, "action"),
        token_value(&light_registry, "brand")
    );
    assert_eq!(
        token_value(&dark_registry, "action"),
        token_value(&dark_registry, "brand")
    );
}

#[test]
fn dtcg_inputs_are_case_insensitive_without_erasing_duplicate_entries() {
    let root = TempDirectory::create("dtcg-inputs");
    let resolver = write_dtcg_resolver(&root.0);
    let output = root.0.join("out");

    let lower = write_dtcg_theme_artifact(&resolver, &[("appearance", "dark")], &output)
        .expect("lowercase selection");
    let folded = write_dtcg_theme_artifact(&resolver, &[("APPEARANCE", "DaRk")], &output)
        .expect("case-folded selection");
    assert_eq!(lower, folded);

    for inputs in [
        &[("appearance", "light"), ("appearance", "dark")][..],
        &[("Appearance", "light"), ("appearance", "dark")][..],
    ] {
        let error = write_dtcg_theme_artifact(&resolver, inputs, root.0.join("rejected"))
            .expect_err("duplicate input must fail");
        assert!(matches!(error, BuildError::Dtcg(_)));
        assert!(error.to_string().contains("DTCG"));
    }
    assert!(!root.0.join("rejected").exists());
}

#[test]
fn dtcg_invalid_context_and_document_keep_build_error_context() {
    let root = TempDirectory::create("dtcg-errors");
    let resolver = write_dtcg_resolver(&root.0);

    let invalid_context = write_dtcg_theme_artifact(
        &resolver,
        &[("appearance", "high-contrast")],
        root.0.join("invalid-context"),
    )
    .expect_err("unknown context must fail");
    assert!(matches!(invalid_context, BuildError::Dtcg(_)));
    assert!(
        invalid_context
            .to_string()
            .contains("failed to configure PliegoCSS DTCG theme")
    );

    let unknown_modifier = write_dtcg_theme_artifact(
        &resolver,
        &[("density", "compact")],
        root.0.join("unknown-modifier"),
    )
    .expect_err("unknown modifier must fail");
    assert!(matches!(unknown_modifier, BuildError::Dtcg(_)));

    let malformed = root.0.join("malformed.resolver.json");
    fs::write(&malformed, "{not-json").expect("write malformed fixture");
    let invalid_document =
        write_dtcg_theme_artifact(&malformed, &[], root.0.join("invalid-document"))
            .expect_err("malformed Resolver must fail");
    assert!(matches!(invalid_document, BuildError::Dtcg(_)));
    assert!(!root.0.join("invalid-context").exists());
    assert!(!root.0.join("unknown-modifier").exists());
    assert!(!root.0.join("invalid-document").exists());
}

#[test]
fn repeated_dtcg_helper_calls_keep_the_same_artifact() {
    let root = TempDirectory::create("dtcg-stable");
    let resolver = write_dtcg_resolver(&root.0);
    let output = root.0.join("out");

    let first = write_dtcg_theme_artifact(&resolver, &[("appearance", "dark")], &output)
        .expect("first artifact");
    let first_metadata = fs::metadata(first.path()).expect("first artifact metadata");
    let second = write_dtcg_theme_artifact(&resolver, &[("appearance", "dark")], &output)
        .expect("second artifact");
    let second_metadata = fs::metadata(second.path()).expect("second artifact metadata");

    assert_eq!(first, second);
    assert_eq!(first_metadata.len(), second_metadata.len());
    assert_eq!(fs::read_dir(output).expect("list output").count(), 1);
}

#[test]
fn theme_macro_grammar_accepts_legacy_defaults_and_ordered_literal_inputs() {
    let legacy = || {
        crate::theme!("pliego.theme.toml");
    };
    let defaults = || {
        crate::theme!(tokens = "product.resolver.json", inputs = {});
    };
    let ordered_duplicates = || {
        crate::theme!(
            tokens = "product.resolver.json",
            inputs = {
                "appearance" => "light",
                "appearance" => "dark",
            },
        );
    };
    let _ = std::hint::black_box((legacy, defaults, ordered_duplicates));
}
