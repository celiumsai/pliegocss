use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use pliego_css_build::artifacts::FindingDocument;
use serde_json::{Map, Value, json};

const SARIF_SCHEMA: &str =
    "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json";

/// Projects a canonical finding document into deterministic SARIF 2.1.0 JSON.
///
/// # Errors
///
/// Returns an error when the canonical finding document cannot be serialized or does not match
/// the finding contract expected by the projection.
pub fn render_sarif(document: &FindingDocument) -> Result<String, String> {
    let canonical: Value = serde_json::from_str(
        &document
            .to_json_pretty()
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let root = canonical
        .as_object()
        .ok_or_else(|| "canonical finding document must be an object".to_owned())?;
    let schema_version = string(root, "schemaVersion")?;
    let command = string(root, "command")?;
    let tool = object(root, "tool")?;
    let tool_name = string(tool, "name")?;
    let tool_version = string(tool, "version")?;
    let findings = root
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| "canonical finding document must contain findings".to_owned())?;

    let codes = findings
        .iter()
        .map(|finding| field_string(finding, "code").map(str::to_owned))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let rules = codes
        .iter()
        .map(|code| {
            json!({
                "id": code,
                "name": code,
                "shortDescription": {"text": format!("PliegoCSS finding {code}")}
            })
        })
        .collect::<Vec<_>>();
    let rule_indices = codes
        .iter()
        .enumerate()
        .map(|(index, code)| (code.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let results = findings
        .iter()
        .map(|finding| result(finding, &rule_indices))
        .collect::<Result<Vec<_>, _>>()?;
    let sarif = json!({
        "$schema": SARIF_SCHEMA,
        "version": "2.1.0",
        "runs": [{
            "tool": {"driver": {
                "name": tool_name,
                "version": tool_version,
                "rules": rules
            }},
            "results": results,
            "properties": {
                "pliegoCssCommand": command,
                "pliegoCssFindingSchemaVersion": schema_version
            }
        }]
    });
    let mut output = serde_json::to_string_pretty(&sarif).map_err(|error| error.to_string())?;
    output.push('\n');
    Ok(output)
}

fn result(finding: &Value, rule_indices: &BTreeMap<&str, usize>) -> Result<Value, String> {
    let code = field_string(finding, "code")?;
    let severity = field_string(finding, "severity")?;
    let message = field_string(finding, "message")?;
    let fingerprint = field_string(finding, "fingerprint")?;
    let mut value = Map::from_iter([
        ("ruleId".into(), json!(code)),
        (
            "ruleIndex".into(),
            json!(
                rule_indices
                    .get(code)
                    .ok_or_else(|| format!("missing SARIF rule for `{code}`"))?
            ),
        ),
        ("level".into(), json!(sarif_level(severity)?)),
        ("message".into(), json!({"text": message})),
        (
            "partialFingerprints".into(),
            json!({"pliegoCssFinding/v1": fingerprint}),
        ),
        ("properties".into(), json!({"pliegoCssFinding": finding})),
    ]);
    if let Some(source) = finding.get("source").filter(|source| !source.is_null()) {
        value.insert("locations".into(), json!([location(source)?]));
        if let Some(source_map) = source.get("sourceMap").filter(|item| !item.is_null()) {
            value.insert(
                "relatedLocations".into(),
                json!([{
                    "id": 1,
                    "message": {"text": "generated artifact source map"},
                    "physicalLocation": physical_location(source_map)?
                }]),
            );
        }
    }
    Ok(Value::Object(value))
}

fn location(source: &Value) -> Result<Value, String> {
    Ok(json!({"physicalLocation": physical_location(source)?}))
}

fn physical_location(source: &Value) -> Result<Value, String> {
    let object = source
        .as_object()
        .ok_or_else(|| "finding source must be an object".to_owned())?;
    let file = string(object, "file")?;
    let start = number(object, "byteStart")?;
    let end = number(object, "byteEnd")?;
    let length = end
        .checked_sub(start)
        .ok_or_else(|| "finding source byte range is reversed".to_owned())?;
    let mut region = Map::from_iter([
        ("byteOffset".into(), json!(start)),
        ("byteLength".into(), json!(length)),
    ]);
    for field in ["startLine", "startColumn", "endLine", "endColumn"] {
        if let Some(value) = object.get(field).filter(|value| !value.is_null()) {
            region.insert(field.into(), value.clone());
        }
    }
    Ok(json!({
        "artifactLocation": {"uri": path_uri(file)},
        "region": region
    }))
}

fn sarif_level(severity: &str) -> Result<&'static str, String> {
    match severity {
        "info" => Ok("note"),
        "warning" => Ok("warning"),
        "error" => Ok("error"),
        value => Err(format!("unsupported finding severity `{value}`")),
    }
}

fn path_uri(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            write!(&mut encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    encoded
}

fn object<'a>(root: &'a Map<String, Value>, field: &str) -> Result<&'a Map<String, Value>, String> {
    root.get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("canonical finding document `{field}` must be an object"))
}

fn string<'a>(root: &'a Map<String, Value>, field: &str) -> Result<&'a str, String> {
    root.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("canonical finding document `{field}` must be a string"))
}

fn number(root: &Map<String, Value>, field: &str) -> Result<u64, String> {
    root.get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("finding source `{field}` must be an unsigned integer"))
}

fn field_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("canonical finding `{field}` must be a string"))
}
