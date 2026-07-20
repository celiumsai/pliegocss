use pliego_css_build::artifacts::ReachabilityIndex;

use super::Provenance;

pub(crate) fn cli_provenance(source: &str, reason: &str) -> Provenance {
    Provenance {
        source: source.to_owned(),
        file: None,
        byte_start: None,
        byte_end: None,
        macro_kind: "cli".into(),
        reason: reason.into(),
    }
}

pub(crate) fn provenance_is_reachable(
    index: &ReachabilityIndex,
    provenance: &Provenance,
) -> Result<bool, String> {
    let (Some(file), Some(start), Some(end)) = (
        provenance.file.as_deref(),
        provenance.byte_start,
        provenance.byte_end,
    ) else {
        return Err("origin lacks a source range".into());
    };
    index.origin_is_reachable(file, start, end)
}

pub(crate) fn normalize_provenance(provenance: &mut Vec<Provenance>) {
    provenance.sort_by(|left, right| {
        (
            left.file.as_deref(),
            left.byte_start,
            left.byte_end,
            left.macro_kind.as_str(),
            left.reason.as_str(),
            left.source.as_str(),
        )
            .cmp(&(
                right.file.as_deref(),
                right.byte_start,
                right.byte_end,
                right.macro_kind.as_str(),
                right.reason.as_str(),
                right.source.as_str(),
            ))
    });
    provenance.dedup();
}
