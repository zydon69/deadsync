use deadsync_editor::options_preferences::{RenderLimits, RenderPolicy};
#[test]
fn source_four_render_flags_default_true_and_round_trip_without_losing_unknowns() {
    let mut fields = serde_json::Map::new();
    fields.insert("Future".into(), serde_json::json!({"flag":7}));
    assert_eq!(
        RenderPolicy::read(&fields).unwrap(),
        RenderPolicy::default()
    );
    let policy = RenderPolicy {
        notes: false,
        markers: false,
        regions: true,
        misc_events: false,
    };
    policy.write(&mut fields);
    assert_eq!(RenderPolicy::read(&fields).unwrap(), policy);
    assert_eq!(fields["Future"]["flag"], 7);
    fields.insert("RenderRegions".into(), 1.into());
    assert!(RenderPolicy::read(&fields).is_err());
}
#[test]
fn weighted_render_budgets_match_original_full_method_36_cases() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/render-budget.expected.json")).unwrap();
    for c in cases.as_array().unwrap() {
        let n = |key: &str| c[key].as_i64().unwrap() as i32;
        let limits = RenderLimits {
            max_markers: n("MaxMarkers"),
            max_events: n("MaxEvents"),
            max_rate_events: n("MaxRates"),
        };
        let result = limits.per_chart(
            n("Visible"),
            n("Events"),
            n("TotalEvents"),
            n("Rates"),
            n("TotalRates"),
        );
        assert_eq!(result.markers, n("Markers"));
        assert_eq!(result.events, n("DrawEvents"));
        assert_eq!(result.rate_events, n("DrawRates"));
    }
}
#[test]
fn display_fields_and_nonresettable_budgets_match_original_options_including_enum_corner_cases() {
    use deadsync_editor::options_preferences::{BackgroundSize, Display};
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/options.expected.json")).unwrap();
    let keys = [
        "RenderNotes",
        "RenderMarkers",
        "RenderRegions",
        "RenderMiscEvents",
        "MaxMarkersToDraw",
        "MaxEventsToDraw",
        "MaxRateAlteringEventsToProcessPerFrame",
        "MiscEventAreaWidth",
        "HideSongBackground",
        "BackgroundImageSize",
    ];
    for case in reference.as_array().unwrap() {
        let initial = Display::read(case["Input"].as_object().unwrap()).unwrap();
        let changed = Display::read(case["Changed"].as_object().unwrap()).unwrap();
        for (value, expected) in [
            (initial, &case["Normalized"]),
            (changed.restore_defaults(), &case["Restored"]),
        ] {
            let mut actual = serde_json::Map::new();
            value.write(&mut actual);
            for key in keys {
                assert_eq!(
                    &actual[key], &expected[key],
                    "{key} input={}",
                    case["Input"]
                );
            }
        }
    }
    let raw = Display::read(
        serde_json::json!({"BackgroundImageSize":"2"})
            .as_object()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(raw.background.size, BackgroundSize::Unknown(2));
}
