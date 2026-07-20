//! Black-box coverage for the DTCG 2025.10 exchange bridge.

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use pliego_css_config::{
    DtcgError, TokenExpression, export_dtcg, parse_dtcg_path, parse_dtcg_str, parse_token_graph,
};
use pliego_css_ir::TokenKind;
use pliego_css_theme::ThemeRegistry;
use serde_json::Value;

#[test]
fn imports_inherited_types_aliases_deprecation_and_metadata() {
    let source = r#"
{
  "$extensions": {"org.example.tool": {"keep": true}},
  "color": {
    "$type": "color",
    "brand": {
      "$value": {
        "colorSpace": "srgb",
        "components": [0.2, 0.4, 0.8],
        "alpha": 1
      },
      "$extensions": {"org.example.token": 42}
    },
    "action": {
      "$value": "{color.brand}",
      "$deprecated": "Use color.brand"
    }
  },
  "spacing": {
    "$type": "dimension",
    "gutter": {"$value": {"value": 1.5, "unit": "rem"}}
  },
  "shadow": {
    "$type": "shadow",
    "card": {
      "$value": {
        "offsetX": {"value": 0, "unit": "px"},
        "offsetY": {"value": 1, "unit": "px"},
        "blur": {"value": 2, "unit": "px"},
        "spread": {"value": 0, "unit": "px"},
        "color": "{color.brand}"
      }
    }
  }
}
"#;

    let theme = parse_dtcg_str(source).expect("valid DTCG theme");
    assert_eq!(theme.report().imported_tokens(), 4);
    assert_eq!(theme.report().aliases(), 2);
    assert_eq!(theme.report().derived_values(), 1);
    assert_eq!(theme.report().deprecated_tokens(), ["color.action"]);
    assert_eq!(
        theme
            .registry()
            .token_by_name(TokenKind::Color, "brand")
            .expect("brand")
            .value,
        "color(srgb 0.2 0.4 0.8)"
    );
    assert_eq!(
        theme
            .registry()
            .token_by_name(TokenKind::Color, "action")
            .expect("alias")
            .value,
        "color(srgb 0.2 0.4 0.8)"
    );
    assert_eq!(
        theme
            .registry()
            .token_by_name(TokenKind::Spacing, "gutter")
            .expect("spacing")
            .value,
        "1.5rem"
    );
    assert_eq!(theme.graph().aliases(), 1);
    assert_eq!(theme.graph().derived_values(), 1);
    assert_eq!(theme.graph().deprecations(), 1);
    assert_eq!(theme.graph().dtcg_inventory["color"], 2);
    let card = theme
        .graph()
        .sources
        .iter()
        .find(|source| source.path == "shadow.card")
        .expect("derived shadow graph node");
    assert_eq!(card.expression, TokenExpression::Derived);
    assert_eq!(card.references[0].target.as_deref(), Some("color.brand"));
    let graph_bytes = theme.graph().to_canonical_json().expect("canonical graph");
    assert_eq!(
        parse_token_graph(&graph_bytes).expect("reparse graph"),
        *theme.graph()
    );

    let canonical = theme.to_json_pretty().expect("serialize");
    let reparsed = parse_dtcg_str(&canonical).expect("reparse");
    assert_eq!(theme.document(), reparsed.document());
    assert_eq!(theme.registry().id(), reparsed.registry().id());
}

#[test]
fn resolves_json_pointer_properties_and_root_tokens() {
    let source = r##"
{
  "foundation": {
    "$type": "number",
    "$root": {"$value": 7},
    "scale": {"$value": [1, 4, 9]}
  },
  "z-index": {
    "overlay": {
      "$type": "number",
      "$ref": "#/foundation/scale/$value/2"
    },
    "base": {"$value": "{foundation.$root}"}
  }
}
"##;

    let theme = parse_dtcg_str(source).expect("pointer theme");
    assert_eq!(theme.report().preserved_tokens().len(), 2);
    assert_eq!(
        theme
            .registry()
            .token_by_name(TokenKind::ZIndex, "overlay")
            .expect("overlay")
            .value,
        "9"
    );
    assert_eq!(
        theme
            .registry()
            .token_by_name(TokenKind::ZIndex, "base")
            .expect("base")
            .value,
        "7"
    );
}

#[test]
fn resolves_json_pointer_properties_at_arbitrary_nested_depth() {
    let source = r##"
{
  "foundation": {
    "$type": "number",
    "scale": {"$value": {"semantic": {"overlay": [3, 7, 11]}}}
  },
  "z-index": {
    "overlay": {
      "$type": "number",
      "$ref": "#/foundation/scale/$value/semantic/overlay/2"
    }
  }
}
"##;

    let theme = parse_dtcg_str(source).expect("deep property pointer theme");
    assert_eq!(
        theme
            .registry()
            .token_by_name(TokenKind::ZIndex, "overlay")
            .expect("overlay")
            .value,
        "11"
    );
}

#[test]
fn rejects_reference_cycles_and_type_mismatches() {
    let cycle = r#"
{
  "color": {
    "$type": "color",
    "a": {"$value": "{color.b}"},
    "b": {"$value": "{color.a}"}
  }
}
"#;
    let error = parse_dtcg_str(cycle).expect_err("cycle must fail");
    assert!(error.to_string().contains("circular reference"));

    let mismatch = r#"
{
  "spacing": {
    "bad": {"$type": "color", "$value": {"value": 1, "unit": "rem"}}
  }
}
"#;
    let error = parse_dtcg_str(mismatch).expect_err("type mismatch must fail");
    assert!(error.to_string().contains("cannot project"));
}

fn alias_chain(length: usize, cycle: bool) -> String {
    let mut tokens = serde_json::Map::new();
    for index in 0..length {
        let target = if index + 1 == length {
            cycle.then(|| "token0000".to_owned())
        } else {
            Some(format!("token{:04}", index + 1))
        };
        let value = target.map_or_else(
            || serde_json::json!(1),
            |target| serde_json::json!(format!("{{number.tokens.{target}}}")),
        );
        tokens.insert(
            format!("token{index:04}"),
            serde_json::json!({"$value": value}),
        );
    }
    serde_json::json!({"number": {"$type": "number", "tokens": tokens}}).to_string()
}

#[test]
fn rejects_deep_acyclic_alias_chains_at_the_explicit_depth_limit() {
    let source = alias_chain(258, false);
    assert!(matches!(
        parse_dtcg_str(&source),
        Err(DtcgError::Limit("alias reference depth exceeds 256 tokens"))
    ));
}

#[test]
fn rejects_deep_alias_cycles_without_recursive_stack_growth() {
    let source = alias_chain(256, true);
    let error = parse_dtcg_str(&source).expect_err("deep cycle must fail safely");
    assert!(matches!(error, DtcgError::Invalid { .. }));
    assert!(error.to_string().contains("circular reference"));
}

#[test]
fn rejects_alias_resolution_that_exhausts_the_explicit_work_budget() {
    const REFERENCES_OVER_BUDGET: usize = 100_001;
    let references = vec![serde_json::json!("{number.base}"); REFERENCES_OVER_BUDGET];
    let source = serde_json::json!({
        "number": {
            "$type": "number",
            "base": {"$value": 1},
            "fanout": {"$value": references}
        }
    })
    .to_string();
    assert!(matches!(
        parse_dtcg_str(&source),
        Err(DtcgError::Limit(
            "alias resolution exceeds 100,000 work units"
        ))
    ));
}

#[test]
fn validates_color_ranges_and_normalizes_font_weight_aliases() {
    let weights = parse_dtcg_str(
        r#"
{
  "font-weight": {
    "$type": "fontWeight",
    "display": {"$value": "extra-bold"},
    "variable": {"$value": 425}
  }
}
"#,
    )
    .expect("valid font weights");
    assert_eq!(
        weights
            .registry()
            .token_by_name(TokenKind::FontWeight, "display")
            .expect("display")
            .value,
        "800"
    );
    assert_eq!(
        weights
            .registry()
            .token_by_name(TokenKind::FontWeight, "variable")
            .expect("variable")
            .value,
        "425"
    );

    let invalid_color = r#"
{
  "color": {
    "bad": {
      "$type": "color",
      "$value": {"colorSpace": "srgb", "components": [1.1, 0, 0]}
    }
  }
}
"#;
    let error = parse_dtcg_str(invalid_color).expect_err("range must fail");
    assert!(error.to_string().contains("outside the `srgb` ranges"));
}

#[test]
fn rejects_group_extends_until_the_profile_can_honor_it() {
    let source = r#"
{
  "color": {
    "$extends": "{base}",
    "$type": "color"
  }
}
"#;
    let error = parse_dtcg_str(source).expect_err("extends must fail closed");
    assert!(error.to_string().contains("`$extends`"));
}

#[test]
fn export_is_lossless_and_inventories_nonstandard_css_values() {
    let registry = ThemeRegistry::seed();
    let exported = export_dtcg(&registry).expect("export seed");
    assert_eq!(registry.id(), exported.registry().id());
    assert!(!exported.report().is_fully_interoperable());
    assert!(
        exported
            .report()
            .unmapped_tokens()
            .iter()
            .any(|path| path == "spacing.full")
    );
    assert!(
        exported
            .report()
            .unmapped_tokens()
            .iter()
            .any(|path| path == "color.current")
    );

    let json = exported.to_json_pretty().expect("serialize export");
    let document: Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(
        document["$extensions"]["io.celiums.pliego-css"]["format"],
        "2025.10"
    );
    let reparsed = parse_dtcg_str(&json).expect("reimport export");
    assert_eq!(registry.id(), reparsed.registry().id());
}

#[test]
fn profile_theme_id_detects_tampering() {
    let exported = export_dtcg(&ThemeRegistry::seed()).expect("export seed");
    let mut document = exported.document().clone();
    document["$extensions"]["io.celiums.pliego-css"]["themeId"] =
        Value::String("00000000000000000000000000000001".into());
    let source = serde_json::to_string(&document).expect("serialize tampered document");
    let error = parse_dtcg_str(&source).expect_err("identity mismatch must fail");
    assert!(error.to_string().contains("does not match derived"));
}

#[test]
fn canonical_name_collisions_fail_instead_of_overwriting() {
    let source = r#"
{
  "spacing": {
    "$type": "dimension",
    "Brand Gap": {"$value": {"value": 1, "unit": "rem"}},
    "brand_gap": {"$value": {"value": 2, "unit": "rem"}}
  }
}
"#;
    let error = parse_dtcg_str(source).expect_err("canonical collision must fail");
    assert!(error.to_string().contains("duplicate canonical"));
}

#[test]
fn path_loader_uses_the_same_contract() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("pliego-dtcg-{nonce}.tokens.json"));
    fs::write(
        &path,
        r#"{"font-weight":{"book":{"$type":"fontWeight","$value":450}}}"#,
    )
    .expect("write fixture");
    let theme = parse_dtcg_path(&path).expect("load fixture");
    fs::remove_file(path).expect("remove fixture");
    assert_eq!(
        theme
            .registry()
            .token_by_name(TokenKind::FontWeight, "book")
            .expect("book")
            .value,
        "450"
    );
}

#[test]
fn path_loader_enforces_size_and_utf8_boundaries() {
    const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let exact = std::env::temp_dir().join(format!("pliego-dtcg-{nonce}.exact-limit.json"));
    let mut exact_bytes = vec![b' '; MAX_DOCUMENT_BYTES];
    exact_bytes[0] = b'{';
    exact_bytes[1] = b'}';
    fs::write(&exact, &exact_bytes).expect("write exact-limit fixture");
    drop(exact_bytes);
    let exact_result = parse_dtcg_path(&exact);
    fs::remove_file(exact).expect("remove exact-limit fixture");
    exact_result.expect("an exact 16 MiB document must remain valid");

    let oversized = std::env::temp_dir().join(format!("pliego-dtcg-{nonce}.oversized.json"));
    fs::File::create(&oversized)
        .expect("create oversized fixture")
        .set_len((MAX_DOCUMENT_BYTES + 1) as u64)
        .expect("size oversized fixture");
    let oversized_result = parse_dtcg_path(&oversized);
    fs::remove_file(oversized).expect("remove oversized fixture");
    assert!(matches!(
        oversized_result,
        Err(DtcgError::Limit("document exceeds 16 MiB"))
    ));

    let non_utf8 = std::env::temp_dir().join(format!("pliego-dtcg-{nonce}.non-utf8.json"));
    fs::write(&non_utf8, [0xff]).expect("write non-UTF-8 fixture");
    let non_utf8_result = parse_dtcg_path(&non_utf8);
    fs::remove_file(non_utf8).expect("remove non-UTF-8 fixture");
    match non_utf8_result.expect_err("non-UTF-8 input must fail") {
        DtcgError::Read { source, .. } => {
            assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
            assert!(source.to_string().contains("not valid UTF-8"));
        }
        error => panic!("expected read error, got {error}"),
    }
}

#[cfg(unix)]
#[test]
fn path_loader_rejects_symbolic_links() {
    use std::os::unix::fs::symlink;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("pliego-dtcg-link-{nonce}"));
    let target = directory.join("target.json");
    let link = directory.join("link.json");
    fs::create_dir(&directory).expect("create fixture directory");
    fs::write(&target, r#"{"font-weight":{"book":{"$value":450}}}"#).expect("write link target");
    symlink(&target, &link).expect("create symbolic link");
    let result = parse_dtcg_path(&link);
    fs::remove_file(link).expect("remove symbolic link");
    fs::remove_file(target).expect("remove link target");
    fs::remove_dir(directory).expect("remove fixture directory");

    match result.expect_err("symbolic link must fail") {
        DtcgError::Read { source, .. } => {
            assert_eq!(source.kind(), std::io::ErrorKind::InvalidInput);
            assert!(
                source
                    .to_string()
                    .contains("symbolic link or reparse point")
            );
        }
        error => panic!("expected read error, got {error}"),
    }
}

#[cfg(windows)]
#[test]
fn path_loader_rejects_windows_file_symlinks_when_available() {
    use std::io::ErrorKind;
    use std::os::windows::fs::symlink_file;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("pliego-dtcg-link-{nonce}"));
    let target = directory.join("target.json");
    let link = directory.join("link.json");
    fs::create_dir(&directory).expect("create fixture directory");
    fs::write(&target, r#"{"font-weight":{"book":{"$value":450}}}"#).expect("write link target");

    match symlink_file(&target, &link) {
        Ok(()) => {
            let result = parse_dtcg_path(&link).expect_err("file symlink must fail");
            match result {
                DtcgError::Read { source, .. } => {
                    assert_eq!(source.kind(), ErrorKind::InvalidInput);
                    assert!(source.to_string().contains("reparse point"));
                }
                error => panic!("expected read error, got {error}"),
            }
            fs::remove_file(&link).expect("remove file symlink");
        }
        Err(error)
            if error.kind() == ErrorKind::PermissionDenied
                || error.raw_os_error() == Some(1314) => {}
        Err(error) => panic!("create file symlink: {error}"),
    }
    fs::remove_file(target).expect("remove link target");
    fs::remove_dir(directory).expect("remove fixture directory");
}

#[test]
fn malformed_extensions_are_rejected() {
    let error = parse_dtcg_str(r#"{"$extensions":[]}"#).expect_err("array must fail");
    assert!(matches!(error, DtcgError::Invalid { .. }));
    assert!(error.to_string().contains("must be an object"));
}
