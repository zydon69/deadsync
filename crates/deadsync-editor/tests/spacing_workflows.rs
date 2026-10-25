use deadsync_editor::{
    spacing::{Anchor, Helper, Rate, Region, Speeds},
    zoom::SpacingMode,
};
use serde_json::Value;
use std::{fs, path::Path};
fn n(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap()
}
fn close(actual: f64, expected: f64) {
    assert!(
        actual == expected
            || (actual.is_finite()
                && expected.is_finite()
                && actual.to_bits().abs_diff(expected.to_bits()) <= 2),
        "{actual:?} != {expected:?}"
    );
}
fn compare(value: f64, expected: &Value, key: &str) {
    close(value, n(expected, key));
}
#[test]
fn original_spacing_helper_matches_all_modes_reverse_previous_rates_and_regions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    for mode in ["constanttime", "constantrow", "variable"] {
        for suffix in ["", "-reverse"] {
            let name = format!("spacing-{mode}{suffix}");
            let request: Value = serde_json::from_slice(
                &fs::read(root.join(format!("{name}.request.json"))).unwrap(),
            )
            .unwrap();
            let expected: Value = serde_json::from_slice(
                &fs::read(root.join(format!("{name}.expected.json"))).unwrap(),
            )
            .unwrap();
            let mode = match request["Mode"].as_str().unwrap() {
                "ConstantTime" => SpacingMode::ConstantTime,
                "ConstantRow" => SpacingMode::ConstantRow,
                "Variable" => SpacingMode::Variable,
                mode => panic!("Unexpected fixture mode: {mode}"),
            };
            let mut helper = Helper::new(mode, request["Reverse"].as_bool().unwrap());
            let stages = request["Stages"].as_array().unwrap();
            assert_eq!(stages.len(), expected.as_array().unwrap().len());
            for (stage, expected) in stages.iter().zip(expected.as_array().unwrap()) {
                let r = &stage["Rate"];
                let rate = Rate {
                    time: n(r, "Time"),
                    row: n(r, "Row"),
                    tempo: n(r, "Tempo"),
                    seconds_per_row: n(r, "SecondsPerRow"),
                    rows_per_second: n(r, "RowsPerSecond"),
                    scroll: n(r, "Scroll"),
                };
                let speeds = &stage["Speeds"];
                helper
                    .update(
                        rate,
                        Speeds {
                            time: n(speeds, "Time"),
                            row: n(speeds, "Row"),
                            variable: n(speeds, "Variable"),
                        },
                        n(stage, "Interpolated"),
                        n(stage, "Zoom"),
                    )
                    .unwrap();
                compare(helper.pixels_per_second(), expected, "Pps");
                compare(helper.pixels_per_row(), expected, "Ppr");
                assert_eq!(
                    helper.negative_scroll(),
                    expected["Negative"].as_bool().unwrap()
                );
                let queries = stage["Queries"].as_array().unwrap();
                assert_eq!(queries.len(), expected["Queries"].as_array().unwrap().len());
                for (query, expected) in queries.iter().zip(expected["Queries"].as_array().unwrap())
                {
                    let a = &query["Anchor"];
                    let anchor = Anchor {
                        y: n(a, "Y"),
                        time: n(a, "Time"),
                        row: n(a, "Row"),
                    };
                    let r = &query["Region"];
                    let region = Region {
                        time: n(r, "Time"),
                        row: n(r, "Row"),
                        time_duration: n(r, "TimeDuration"),
                        row_duration: n(r, "RowDuration"),
                        selection: r["Selection"].as_bool().unwrap(),
                        drawn_y: n(r, "DrawnY"),
                    };
                    let time = n(query, "Time");
                    let row = n(query, "Row");
                    let mapping = deadsync_editor::rate_mapping::RateMapping::new(
                        rate.row,
                        rate.time,
                        stage["Rate"]
                            .get("Warp")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0),
                        stage["Rate"]
                            .get("Stop")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0),
                        rate.seconds_per_row,
                    )
                    .unwrap();
                    let time_to_row = |time| mapping.position_at(time);
                    let row_to_time = |row| mapping.time_at(row);
                    compare(mapping.time_at(row).unwrap(), expected, "RowTime");
                    compare(mapping.position_at(time).unwrap(), expected, "TimeRow");
                    for (previous, key) in [(false, "Location"), (true, "Previous")] {
                        let (time, row) = helper
                            .location(n(query, "Y"), anchor, previous, time_to_row, row_to_time)
                            .unwrap();
                        close(time, expected[key][0].as_f64().unwrap());
                        close(row, expected[key][1].as_f64().unwrap());
                    }
                    compare(helper.y(time, row, anchor).unwrap(), expected, "Y");
                    compare(
                        helper.y_from_rate(time, row, anchor.y).unwrap(),
                        expected,
                        "EventY",
                    );
                    compare(
                        helper.relative_y(time, row, anchor.y).unwrap(),
                        expected,
                        "RelativeY",
                    );
                    compare(
                        helper.row_y(row, anchor.y, row_to_time).unwrap(),
                        expected,
                        "RowY",
                    );
                    compare(
                        helper.region_y(region, anchor.y).unwrap(),
                        expected,
                        "RegionY",
                    );
                    compare(
                        helper.region_height(region, anchor.y).unwrap(),
                        expected,
                        "RegionHeight",
                    );
                    assert_eq!(
                        helper.skip_region(region).unwrap(),
                        expected["Skip"].as_bool().unwrap()
                    );
                    assert_eq!(
                        helper.region_ends_before(region, time, row).unwrap(),
                        expected["Ends"].as_bool().unwrap()
                    );
                }
            }
        }
    }
}
fn rate() -> Rate {
    Rate {
        row: 0.0,
        time: 0.0,
        tempo: 120.0,
        seconds_per_row: 1.0 / 96.0,
        rows_per_second: 96.0,
        scroll: 1.0,
    }
}
#[test]
fn zero_variable_rate_has_forward_positions_but_explicitly_no_unique_inverse() {
    let mut helper = Helper::new(SpacingMode::Variable, true);
    let mut r = rate();
    r.scroll = 0.0;
    helper.update(r, Speeds::default(), 1.0, 1.0).unwrap();
    assert!(!helper.negative_scroll());
    assert_eq!(helper.relative_y(10.0, 100.0, 160.0).unwrap(), 160.0);
    let anchor = Anchor {
        y: 160.0,
        time: 0.0,
        row: 0.0,
    };
    assert!(
        helper
            .location(170.0, anchor, false, |_| Ok(0.0), |_| Ok(0.0))
            .is_err()
    );
    assert!(
        helper
            .location(170.0, anchor, true, |_| Ok(0.0), |_| Ok(0.0))
            .is_ok()
    );
}
#[test]
fn invalid_updates_queries_and_conversion_failures_do_not_mutate_rates() {
    let mut helper = Helper::new(SpacingMode::ConstantTime, false);
    helper.update(rate(), Speeds::default(), 1.0, 1.0).unwrap();
    let original = helper;
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0] {
        assert!(
            helper
                .update(rate(), Speeds::default(), 1.0, value)
                .is_err()
        );
        assert_eq!(helper, original);
        let mut r = rate();
        r.seconds_per_row = value;
        assert!(helper.update(r, Speeds::default(), 1.0, 1.0).is_err());
        assert_eq!(helper, original);
    }
    let anchor = Anchor {
        y: 160.0,
        time: 0.0,
        row: 0.0,
    };
    assert!(helper.y(f64::NAN, 0.0, anchor).is_err());
    assert!(
        helper
            .location(170.0, anchor, false, |_| Ok(f64::NAN), |_| Ok(0.0))
            .is_err()
    );
    assert!(
        helper
            .location(
                170.0,
                anchor,
                false,
                |_| Err(deadsync_editor::Error::InvalidEdit(
                    "timeline unavailable".into()
                )),
                |_| Ok(0.0)
            )
            .is_err()
    );
    assert_eq!(helper, original);
}
#[test]
fn region_edges_are_strict_selection_is_not_skipped_and_reverse_keeps_signed_height() {
    for (mode, height) in [
        (SpacingMode::ConstantTime, -300.0),
        (SpacingMode::ConstantRow, -96.0),
        (SpacingMode::Variable, -60.0),
    ] {
        let mut helper = Helper::new(mode, true);
        helper.update(rate(), Speeds::default(), 1.0, 1.0).unwrap();
        let mut region = Region {
            time: 0.0,
            row: 0.0,
            time_duration: 1.0,
            row_duration: 48.0,
            selection: false,
            drawn_y: 160.0,
        };
        assert!(!helper.region_ends_before(region, 1.0, 48.0).unwrap());
        assert!(helper.region_ends_before(region, 1.01, 49.0).unwrap());
        assert_eq!(helper.region_height(region, 160.0).unwrap(), height);
        assert!(!helper.negative_scroll());
        region.time_duration = 0.0;
        region.row_duration = 0.0;
        assert!(helper.skip_region(region).unwrap());
        region.selection = true;
        assert!(!helper.skip_region(region).unwrap());
    }
}

#[test]
fn rate_dependent_queries_require_initialization_but_explicit_anchor_math_does_not() {
    let helper = Helper::new(SpacingMode::ConstantTime, false);
    let anchor = Anchor {
        y: 160.0,
        time: 0.0,
        row: 0.0,
    };
    assert_eq!(helper.y(1.0, 48.0, anchor).unwrap(), 161.0);
    assert!(helper.y_from_rate(1.0, 48.0, 160.0).is_err());
    assert!(helper.row_y(48.0, 160.0, |_| Ok(1.0)).is_err());
    assert!(
        helper
            .location(170.0, anchor, false, |_| Ok(0.0), |_| Ok(0.0))
            .is_err()
    );
    let region = Region {
        time: 0.0,
        row: 0.0,
        time_duration: 1.0,
        row_duration: 48.0,
        selection: false,
        drawn_y: 160.0,
    };
    assert!(helper.region_y(region, 160.0).is_err());
    assert!(helper.region_height(region, 160.0).is_err());
}
