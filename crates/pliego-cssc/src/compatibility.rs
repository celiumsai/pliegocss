use std::collections::BTreeMap;

use pliego_css_build::artifacts::{CompatibilityProfile, build_compatibility_policy};
use pliego_css_ir::{SemanticStyle, is_classified_selector_transform};

use super::{CliFailure, Provenance};

pub(crate) fn run_compatibility(targets: CompatibilityProfile) -> Result<(), CliFailure> {
    let policy = build_compatibility_policy(targets).map_err(CliFailure::tool)?;
    print!("{policy}");
    Ok(())
}

pub(crate) fn enforce_compatibility_policy(
    targets: CompatibilityProfile,
    styles: &BTreeMap<Vec<u8>, (SemanticStyle, Vec<Provenance>)>,
) -> Result<(), String> {
    if !targets.rejects_unclassified_css() {
        return Ok(());
    }
    let mut violations = Vec::new();
    for (style, origins) in styles.values() {
        let violation = if !style.arbitrary_properties.is_empty() {
            Some(("CMP002", "arbitrary properties"))
        } else if style
            .selectors
            .iter()
            .any(|selector| !is_classified_selector_transform(selector))
        {
            Some(("CMP003", "arbitrary selectors"))
        } else if !style.arbitrary_values.is_empty() {
            Some(("CMP001", "arbitrary values"))
        } else {
            None
        };
        let Some((code, feature)) = violation else {
            continue;
        };
        let origin = origins.iter().min_by(|left, right| {
            (
                left.file.as_deref(),
                left.byte_start,
                left.byte_end,
                left.reason.as_str(),
                left.source.as_str(),
            )
                .cmp(&(
                    right.file.as_deref(),
                    right.byte_start,
                    right.byte_end,
                    right.reason.as_str(),
                    right.source.as_str(),
                ))
        });
        let (file, start, end, label) = origin.map_or_else(
            || (None, None, None, "unknown origin".to_owned()),
            |origin| {
                (
                    origin.file.clone(),
                    origin.byte_start,
                    origin.byte_end,
                    origin.reason.clone(),
                )
            },
        );
        violations.push((file, start, end, code, feature, label));
    }
    violations.sort();
    if let Some((file, start, end, code, feature, label)) = violations.into_iter().next() {
        let location = match (file, start, end) {
            (Some(file), Some(start), Some(end)) => format!("{file} [bytes {start}..{end})"),
            _ => label,
        };
        return Err(format!(
            "{code}: target profile `baseline-widely` rejects unclassified {feature} at {location}; use a typed utility/token or select `--targets none` as an explicit unmanaged escape hatch"
        ));
    }
    Ok(())
}
