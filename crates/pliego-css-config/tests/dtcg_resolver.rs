//! Black-box coverage for the bounded DTCG Resolver 2025.10 profile.

use std::collections::BTreeMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use pliego_css_config::{
    DTCG_RESOLVER_PROFILE, DtcgError, DtcgTheme, parse_dtcg_resolver_path, parse_dtcg_resolver_str,
};
use pliego_css_ir::TokenKind;
use serde_json::{Map, Value, json};

const MODIFIER_FIXTURE: &str = r##"
{
  "version": "2025.10",
  "name": "product themes",
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

#[test]
fn resolves_defaults_case_insensitively_and_aliases_after_overrides() {
    let resolver = parse_dtcg_resolver_str(MODIFIER_FIXTURE).expect("valid resolver");
    assert_eq!(DTCG_RESOLVER_PROFILE, "2025.10/same-document-1");
    assert_eq!(resolver.document()["version"], "2025.10");
    assert_eq!(resolver.resolution_count(), 2);
    let adapter = resolver.graph().adapter.as_ref().expect("resolver adapter");
    assert_eq!(adapter.name, "dtcg-resolver");
    assert_eq!(adapter.version, DTCG_RESOLVER_PROFILE);
    assert_eq!(resolver.graph().themes.len(), 2);

    let light = resolver.resolve(&BTreeMap::new()).expect("default context");
    let dark = resolver
        .resolve(&selection("APPEARANCE", "DaRk"))
        .expect("case-insensitive context");

    let light_brand = color_value(&light, "brand");
    let dark_brand = color_value(&dark, "brand");
    assert_ne!(light_brand, dark_brand);
    assert_eq!(color_value(&light, "action"), light_brand);
    assert_eq!(color_value(&dark, "action"), dark_brand);
    assert_eq!(
        light.selections().get("appearance").map(String::as_str),
        Some("light")
    );
    assert_eq!(
        dark.selections().get("appearance").map(String::as_str),
        Some("dark")
    );
    assert_eq!(light.graph(), resolver.graph());
    assert_eq!(dark.graph(), resolver.graph());
    assert!(
        resolver.graph().themes.iter().any(|theme| {
            theme.selections.get("appearance").map(String::as_str) == Some("light")
        })
    );
    assert!(
        resolver.graph().themes.iter().any(|theme| {
            theme.selections.get("appearance").map(String::as_str) == Some("dark")
        })
    );
}

#[test]
fn later_sources_win_and_sets_can_reference_bundled_sets() {
    let resolver = parse_dtcg_resolver_str(
        r##"
{
  "version": "2025.10",
  "$defs": {
    "base": {
      "spacing": {
        "$type": "dimension",
        "gutter": {"$value": {"value": 1, "unit": "rem"}}
      }
    },
    "product": {
      "spacing": {
        "$type": "dimension",
        "gutter": {"$value": {"value": 2, "unit": "rem"}}
      }
    }
  },
  "sets": {
    "core": {"sources": [{"$ref": "#/$defs/base"}]},
    "product": {
      "sources": [
        {"$ref": "#/sets/core"},
        {"$ref": "#/$defs/product"}
      ]
    }
  },
  "modifiers": {},
  "resolutionOrder": [{"$ref": "#/sets/product"}]
}
"##,
    )
    .expect("nested same-document sources");
    let theme = resolver.resolve(&BTreeMap::new()).expect("single theme");
    assert_eq!(resolver.resolution_count(), 1);
    assert_eq!(token_value(&theme, TokenKind::Spacing, "gutter"), "2rem");
}

#[test]
fn inline_sets_and_modifiers_follow_resolution_order() {
    let resolver = parse_dtcg_resolver_str(
        r#"
{
  "version": "2025.10",
  "resolutionOrder": [
    {
      "type": "set",
      "name": "foundation",
      "sources": [{
        "font-weight": {
          "$type": "fontWeight",
          "body": {"$value": 400}
        }
      }]
    },
    {
      "type": "modifier",
      "name": "emphasis",
      "contexts": {
        "normal": [],
        "strong": [{
          "font-weight": {
            "$type": "fontWeight",
            "body": {"$value": 700}
          }
        }]
      },
      "default": "normal"
    }
  ]
}
"#,
    )
    .expect("inline resolver items");

    let normal = resolver
        .resolve(&BTreeMap::new())
        .expect("default inline mode");
    let strong = resolver
        .resolve(&selection("emphasis", "strong"))
        .expect("selected inline mode");
    assert_eq!(token_value(&normal, TokenKind::FontWeight, "body"), "400");
    assert_eq!(token_value(&strong, TokenKind::FontWeight, "body"), "700");
}

#[test]
fn supports_shallow_overrides_next_to_a_same_document_reference() {
    let resolver = parse_dtcg_resolver_str(
        r##"
{
  "version": "2025.10",
  "$defs": {
    "base": {
      "font-weight": {
        "$type": "fontWeight",
        "body": {"$value": 400},
        "display": {"$value": 600}
      }
    }
  },
  "sets": {
    "product": {
      "sources": [{
        "$ref": "#/$defs/base",
        "font-weight": {
          "$type": "fontWeight",
          "body": {"$value": 400},
          "display": {"$value": 800}
        }
      }]
    }
  },
  "modifiers": {},
  "resolutionOrder": [{"$ref": "#/sets/product"}]
}
"##,
    )
    .expect("reference with local override");
    let theme = resolver.resolve(&BTreeMap::new()).expect("single theme");
    assert_eq!(token_value(&theme, TokenKind::FontWeight, "body"), "400");
    assert_eq!(token_value(&theme, TokenKind::FontWeight, "display"), "800");
}

#[test]
fn validates_cycles_in_every_context_not_only_the_default() {
    let error = parse_dtcg_resolver_str(
        r##"
{
  "version": "2025.10",
  "$defs": {
    "base": {
      "color": {
        "$type": "color",
        "a": {"$value": {"colorSpace": "srgb", "components": [1, 1, 1]}},
        "b": {"$value": {"colorSpace": "srgb", "components": [0, 0, 0]}}
      }
    },
    "cycle": {
      "color": {
        "$type": "color",
        "a": {"$value": "{color.b}"},
        "b": {"$value": "{color.a}"}
      }
    }
  },
  "sets": {
    "base": {"sources": [{"$ref": "#/$defs/base"}]}
  },
  "modifiers": {
    "appearance": {
      "contexts": {
        "light": [],
        "dark": [{"$ref": "#/$defs/cycle"}]
      },
      "default": "light"
    }
  },
  "resolutionOrder": [
    {"$ref": "#/sets/base"},
    {"$ref": "#/modifiers/appearance"}
  ]
}
"##,
    )
    .expect_err("cycle in an unselected branch must fail parsing");
    assert!(error.to_string().contains("circular reference"));
}

#[test]
fn selections_are_closed_complete_and_casefold_unique() {
    let source = r##"
{
  "version": "2025.10",
  "$defs": {
    "base": {
      "font-weight": {
        "$type": "fontWeight",
        "body": {"$value": 400}
      }
    }
  },
  "sets": {"base": {"sources": [{"$ref": "#/$defs/base"}]}},
  "modifiers": {
    "density": {
      "contexts": {
        "compact": [],
        "comfortable": []
      }
    }
  },
  "resolutionOrder": [
    {"$ref": "#/sets/base"},
    {"$ref": "#/modifiers/density"}
  ]
}
"##;
    let resolver = parse_dtcg_resolver_str(source).expect("resolver without default");

    resolver
        .resolve(&BTreeMap::new())
        .expect_err("missing modifier must fail");
    resolver
        .resolve(&selection("density", "roomy"))
        .expect_err("unknown context must fail");
    resolver
        .resolve(&selection("motion", "reduced"))
        .expect_err("unknown modifier must fail");

    let duplicate = BTreeMap::from([
        ("Density".to_owned(), "compact".to_owned()),
        ("density".to_owned(), "comfortable".to_owned()),
    ]);
    resolver
        .resolve(&duplicate)
        .expect_err("case-folded duplicate input must fail");
    resolver
        .resolve_entries([("density", "compact"), ("density", "comfortable")])
        .expect_err("exact duplicate input entries must fail");
    resolver
        .resolve(&selection("DENSITY", "COMPACT"))
        .expect("valid selection is case-insensitive");
}

#[test]
fn document_names_must_be_unique_after_ascii_casefolding() {
    for source in [
        r##"
{
  "version": "2025.10",
  "$defs": {
    "base": {"font-weight": {"body": {"$type": "fontWeight", "$value": 400}}}
  },
  "sets": {"base": {"sources": [{"$ref": "#/$defs/base"}]}},
  "modifiers": {
    "Scheme": {
      "contexts": {
        "light": [],
        "dark": []
      }
    },
    "scheme": {
      "contexts": {
        "day": [],
        "night": []
      }
    }
  },
  "resolutionOrder": [
    {"$ref": "#/sets/base"},
    {"$ref": "#/modifiers/Scheme"}
  ]
}
"##,
        r##"
{
  "version": "2025.10",
  "$defs": {
    "base": {"font-weight": {"body": {"$type": "fontWeight", "$value": 400}}}
  },
  "sets": {"base": {"sources": [{"$ref": "#/$defs/base"}]}},
  "modifiers": {
    "scheme": {
      "contexts": {
        "Light": [],
        "light": []
      }
    }
  },
  "resolutionOrder": [
    {"$ref": "#/sets/base"},
    {"$ref": "#/modifiers/scheme"}
  ]
}
"##,
    ] {
        parse_dtcg_resolver_str(source)
            .expect_err("case-folded document collision must fail closed");
    }
}

#[test]
fn rejects_external_and_forbidden_same_document_references() {
    let documents = [
        (
            "external source",
            r##"
{
  "version": "2025.10",
  "sets": {"base": {"sources": [{"$ref": "./base.tokens.json"}]}},
  "modifiers": {},
  "resolutionOrder": [{"$ref": "#/sets/base"}]
}
"##,
        ),
        (
            "modifier used as a source",
            r##"
{
  "version": "2025.10",
  "sets": {"base": {"sources": [{"$ref": "#/modifiers/mode"}]}},
  "modifiers": {
    "mode": {
      "contexts": {
        "light": [],
        "dark": []
      }
    }
  },
  "resolutionOrder": [{"$ref": "#/sets/base"}]
}
"##,
        ),
        (
            "resolution order used as a source",
            r##"
{
  "version": "2025.10",
  "sets": {"base": {"sources": [{"$ref": "#/resolutionOrder/0"}]}},
  "modifiers": {},
  "resolutionOrder": [{"$ref": "#/sets/base"}]
}
"##,
        ),
        (
            "token document used in resolution order",
            r##"
{
  "version": "2025.10",
  "$defs": {"tokens": {}},
  "sets": {},
  "modifiers": {},
  "resolutionOrder": [{"$ref": "#/$defs/tokens"}]
}
"##,
        ),
    ];

    for (label, source) in documents {
        parse_dtcg_resolver_str(source).expect_err(label);
    }
}

#[test]
fn rejects_more_than_256_resolution_permutations() {
    let mut contexts = Map::new();
    for index in 0..257_u16 {
        contexts.insert(format!("context-{index:03}"), json!([]));
    }
    let document = json!({
        "version": "2025.10",
        "$defs": {
            "base": {
                "font-weight": {
                    "$type": "fontWeight",
                    "body": {"$value": 400}
                }
            }
        },
        "sets": {"base": {"sources": [{"$ref": "#/$defs/base"}]}},
    "modifiers": {"mode": {"contexts": Value::Object(contexts)}},
        "resolutionOrder": [
            {"$ref": "#/sets/base"},
            {"$ref": "#/modifiers/mode"}
        ]
    });
    let source = serde_json::to_string(&document).expect("serialize fixture");
    let error = parse_dtcg_resolver_str(&source).expect_err("permutation cap must be enforced");
    assert!(error.to_string().contains("256"));
}

#[test]
fn expansion_budget_stops_exponential_set_dags() {
    let mut sets = Map::new();
    sets.insert(
        "level-00".into(),
        json!({"sources": [{"$ref": "#/$defs/base"}]}),
    );
    for index in 1..=17_u8 {
        let previous = format!("#/sets/level-{:02}", index - 1);
        sets.insert(
            format!("level-{index:02}"),
            json!({"sources": [{"$ref": previous}, {"$ref": previous}]}),
        );
    }
    let document = json!({
        "version": "2025.10",
        "$defs": {
            "base": {
                "font-weight": {
                    "$type": "fontWeight",
                    "body": {"$value": 400}
                }
            }
        },
        "sets": Value::Object(sets),
        "resolutionOrder": [{"$ref": "#/sets/level-17"}]
    });
    let source = serde_json::to_string(&document).expect("serialize expansion fixture");
    let error = parse_dtcg_resolver_str(&source).expect_err("exponential DAG must be bounded");
    assert!(
        error.to_string().contains("65,536")
            || error.to_string().contains("64 MiB")
            || error.to_string().contains("262,144")
    );
}

#[test]
fn cumulative_graph_limit_applies_before_all_permutations_are_retained() {
    let mut token_group = Map::new();
    token_group.insert("$type".into(), Value::String("fontWeight".into()));
    for index in 0..257_u16 {
        token_group.insert(format!("weight-{index:03}"), json!({"$value": 400}));
    }
    let mut contexts = Map::new();
    for index in 0..256_u16 {
        contexts.insert(format!("mode-{index:03}"), json!([]));
    }
    let document = json!({
        "version": "2025.10",
        "$defs": {"tokens": {"font-weight": Value::Object(token_group)}},
        "sets": {"base": {"sources": [{"$ref": "#/$defs/tokens"}]}},
        "modifiers": {"mode": {"contexts": Value::Object(contexts)}},
        "resolutionOrder": [
            {"$ref": "#/sets/base"},
            {"$ref": "#/modifiers/mode"}
        ]
    });
    let source = serde_json::to_string(&document).expect("serialize retention fixture");
    let error = parse_dtcg_resolver_str(&source).expect_err("combined graph must be bounded early");
    assert!(error.to_string().contains("65,536"));
}

#[test]
fn source_named_sources_is_not_misclassified_as_a_resolver_set() {
    let document = json!({
        "version": "2025.10",
        "$defs": {
            "tokens": {
                "sources": {
                    "$type": "fontWeight",
                    "body": {"$value": 400}
                }
            }
        },
        "sets": {"base": {"sources": [{"$ref": "#/$defs/tokens"}]}},
        "resolutionOrder": [{"$ref": "#/sets/base"}]
    });
    let source = serde_json::to_string(&document).expect("serialize sources-group fixture");
    let resolver = parse_dtcg_resolver_str(&source).expect("ordinary DTCG sources group");
    let theme = resolver
        .resolve(&BTreeMap::new())
        .expect("single resolution");
    assert!(
        theme
            .report()
            .preserved_tokens()
            .iter()
            .any(|path| path == "sources.body")
    );
}

#[test]
fn chained_and_uri_encoded_json_pointers_are_typed_and_validated() {
    let document = json!({
        "version": "2025.10",
        "$defs": {
            "foundation/tokens.json": {
                "font-weight": {
                    "$type": "fontWeight",
                    "body": {"$value": 400}
                }
            },
            "order-alias": {"$ref": "#/sets/base"}
        },
        "sets": {
            "base": {
                "sources": [{"$ref": "#/$defs/foundation%7E1tokens.json"}]
            }
        },
        "resolutionOrder": [{"$ref": "#/$defs/order-alias"}]
    });
    let source = serde_json::to_string(&document).expect("serialize pointer fixture");
    let resolver = parse_dtcg_resolver_str(&source).expect("encoded and chained pointers");
    let theme = resolver
        .resolve(&BTreeMap::new())
        .expect("single resolution");
    assert_eq!(token_value(&theme, TokenKind::FontWeight, "body"), "400");

    let invalid = source.replace("%7E1", "~2");
    parse_dtcg_resolver_str(&invalid).expect_err("invalid RFC 6901 escape must fail");
}

#[test]
fn terminal_json_pointer_is_percent_decoded_exactly_once() {
    let document = json!({
        "version": "2025.10",
        "$defs": {
            "tokens": {
                "font-weight": {
                    "$type": "fontWeight",
                    "body": {"$value": 400}
                }
            },
            "order-alias": {"$ref": "#/sets/base%25zz"}
        },
        "sets": {
            "base%zz": {
                "sources": [{"$ref": "#/$defs/tokens"}]
            }
        },
        "resolutionOrder": [{"$ref": "#/$defs/order-alias"}]
    });
    let source = serde_json::to_string(&document).expect("serialize percent fixture");
    let resolver = parse_dtcg_resolver_str(&source).expect("single percent decode");
    let theme = resolver
        .resolve(&BTreeMap::new())
        .expect("single resolution");
    assert_eq!(token_value(&theme, TokenKind::FontWeight, "body"), "400");
}

#[test]
fn resolver_source_hash_binds_overridden_declarations() {
    let make_document = |base: u16| {
        json!({
            "version": "2025.10",
            "$defs": {
                "base": {
                    "font-weight": {
                        "$type": "fontWeight",
                        "body": {"$value": base}
                    }
                },
                "override": {
                    "font-weight": {
                        "$type": "fontWeight",
                        "body": {"$value": 700}
                    }
                }
            },
            "sets": {"base": {"sources": [{"$ref": "#/$defs/base"}]}},
            "modifiers": {
                "mode": {
                    "contexts": {
                        "a": [{"$ref": "#/$defs/override"}],
                        "b": [{"$ref": "#/$defs/override"}]
                    }
                }
            },
            "resolutionOrder": [
                {"$ref": "#/sets/base"},
                {"$ref": "#/modifiers/mode"}
            ]
        })
    };
    let first = parse_dtcg_resolver_str(
        &serde_json::to_string(&make_document(400)).expect("serialize first resolver"),
    )
    .expect("first resolver");
    let second = parse_dtcg_resolver_str(
        &serde_json::to_string(&make_document(500)).expect("serialize second resolver"),
    )
    .expect("second resolver");
    let selection = selection("mode", "a");
    assert_eq!(
        first
            .resolve(&selection)
            .expect("first theme")
            .registry()
            .id(),
        second
            .resolve(&selection)
            .expect("second theme")
            .registry()
            .id()
    );
    assert_ne!(
        first.graph().to_canonical_json().expect("first graph"),
        second.graph().to_canonical_json().expect("second graph")
    );
}

#[test]
fn each_graph_theme_keeps_its_own_dtcg_inventory() {
    let resolver = parse_dtcg_resolver_str(
        r##"
{
  "version": "2025.10",
  "$defs": {
    "base": {
      "font-weight": {
        "$type": "fontWeight",
        "body": {"$value": 400}
      }
    },
    "spacing": {
      "spacing": {
        "$type": "dimension",
        "gutter": {"$value": {"value": 1, "unit": "rem"}}
      }
    }
  },
  "sets": {"base": {"sources": [{"$ref": "#/$defs/base"}]}},
  "modifiers": {
    "mode": {
      "contexts": {
        "plain": [],
        "spacious": [{"$ref": "#/$defs/spacing"}]
      }
    }
  },
  "resolutionOrder": [
    {"$ref": "#/sets/base"},
    {"$ref": "#/modifiers/mode"}
  ]
}
"##,
    )
    .expect("resolver inventory");
    let plain = resolver
        .resolve(&selection("mode", "plain"))
        .expect("plain mode");
    let spacious = resolver
        .resolve(&selection("mode", "spacious"))
        .expect("spacious mode");
    let plain_inventory = &plain
        .graph()
        .theme(plain.selections())
        .expect("plain graph theme")
        .dtcg_inventory;
    let spacious_inventory = &spacious
        .graph()
        .theme(spacious.selections())
        .expect("spacious graph theme")
        .dtcg_inventory;
    assert_eq!(plain_inventory.get("fontWeight"), Some(&1));
    assert!(!plain_inventory.contains_key("dimension"));
    assert_eq!(spacious_inventory.get("fontWeight"), Some(&1));
    assert_eq!(spacious_inventory.get("dimension"), Some(&1));
}

#[test]
fn path_loader_uses_the_same_resolver_contract() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("pliego-resolver-{nonce}.json"));
    fs::write(&path, MODIFIER_FIXTURE).expect("write resolver fixture");
    let resolver = parse_dtcg_resolver_path(&path).expect("parse resolver path");
    fs::remove_file(path).expect("remove fixture");
    assert_eq!(resolver.resolution_count(), 2);
}

#[test]
fn resolver_path_loader_rejects_directories_and_oversized_files() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("pliego-resolver-directory-{nonce}"));
    fs::create_dir(&directory).expect("create resolver directory fixture");
    let directory_result = parse_dtcg_resolver_path(&directory);
    fs::remove_dir(directory).expect("remove resolver directory fixture");
    match directory_result.expect_err("directory input must fail") {
        DtcgError::Read { source, .. } => {
            assert_eq!(source.kind(), std::io::ErrorKind::InvalidInput);
            assert!(source.to_string().contains("regular file"));
        }
        error => panic!("expected read error, got {error}"),
    }

    let oversized = std::env::temp_dir().join(format!("pliego-resolver-{nonce}.oversized.json"));
    fs::File::create(&oversized)
        .expect("create oversized resolver fixture")
        .set_len((16 * 1024 * 1024 + 1) as u64)
        .expect("size oversized resolver fixture");
    let oversized_result = parse_dtcg_resolver_path(&oversized);
    fs::remove_file(oversized).expect("remove oversized resolver fixture");
    assert!(matches!(
        oversized_result,
        Err(DtcgError::Limit("resolver document exceeds 16 MiB"))
    ));
}

fn selection(name: &str, context: &str) -> BTreeMap<String, String> {
    BTreeMap::from([(name.to_owned(), context.to_owned())])
}

fn color_value(theme: &DtcgTheme, name: &str) -> String {
    token_value(theme, TokenKind::Color, name)
}

fn token_value(theme: &DtcgTheme, kind: TokenKind, name: &str) -> String {
    theme
        .registry()
        .token_by_name(kind, name)
        .unwrap_or_else(|| panic!("missing {kind:?} token `{name}`"))
        .value
        .clone()
}
