//! Generic-CSS conservative usage observation contracts.

use pliego_css_usage::{
    GenericCssUsageStatus, build_generic_css_usage_report, parse_generic_css_usage_report,
};

fn identity(hex: char) -> String {
    format!("sha256:{}", hex.to_string().repeat(64))
}

#[test]
fn generic_css_observation_is_positive_only_and_absence_stays_unknown() {
    let first = identity('1');
    let second = identity('2');
    let bytes = build_generic_css_usage_report(
        [(second.clone(), 1), (first.clone(), 2)],
        [second.clone()],
        "chrome-local-computed-style-session-1",
    )
    .unwrap();
    let report = parse_generic_css_usage_report(&bytes).unwrap();
    assert_eq!(report.entries().len(), 2);
    assert_eq!(report.entries()[0].identity(), first);
    assert_eq!(report.entries()[0].status(), GenericCssUsageStatus::Unknown);
    assert_eq!(report.entries()[1].identity(), second);
    assert_eq!(
        report.entries()[1].status(),
        GenericCssUsageStatus::Observed
    );
    assert_eq!(
        bytes,
        build_generic_css_usage_report(
            [(identity('2'), 1), (identity('1'), 2)],
            [identity('2')],
            "chrome-local-computed-style-session-1",
        )
        .unwrap()
    );
}

#[test]
fn generic_css_observation_rejects_out_of_universe_and_duplicate_positive_evidence() {
    let first = identity('a');
    let outside = identity('b');
    assert!(build_generic_css_usage_report([(first.clone(), 1)], [outside], "scope").is_err());
    assert!(
        build_generic_css_usage_report([(first.clone(), 1)], [first.clone(), first], "scope",)
            .is_err()
    );
}
