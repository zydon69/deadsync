use deadsync_editor::{
    generation::editor_timing::SpacingSection,
    rate_mapping::RateMapping,
    region_selection::{
        geometry::{self, Axis, Range},
        misc_layout::Rect,
    },
    selection_preferences::Overlap,
    spacing::Speeds,
    spacing_index::Index,
    spacing_search::Settings,
    zoom::SpacingMode,
};
use serde_json::Value;
fn n(v: &Value, k: &str) -> f64 {
    v[k].as_f64().unwrap()
}
fn mode(n: u64) -> Overlap {
    match n {
        0 => Overlap::Any,
        1 => Overlap::Center,
        _ => Overlap::All,
    }
}
fn index(scroll: f64) -> Index {
    Index::new(
        [0.0, 48.0, 96.0]
            .into_iter()
            .map(|row| SpacingSection {
                order: 0,
                mapping: RateMapping::new(
                    row,
                    if row == 96.0 { 0.75 } else { row / 96.0 },
                    0.0,
                    0.0,
                    if row == 48.0 { 1.0 / 192.0 } else { 1.0 / 96.0 },
                )
                .unwrap(),
                tempo: if row == 48.0 { 240.0 } else { 120.0 },
                scroll: if row == 96.0 { scroll } else { 1.0 },
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn real_original_helpers_match_exact_and_adjacent_boundaries_rates_and_rectangles() {
    let cases: Value = serde_json::from_str(include_str!(
        "fixtures/key_commands/region-geometry-source.json"
    ))
    .unwrap();
    let index = index(-1.0);
    for case in cases.as_array().unwrap() {
        let mode = mode(case["mode"].as_u64().unwrap());
        for row in case["lanes"].as_array().unwrap() {
            let range = Range::normalized(n(row, "min") + 128.0, n(row, "max") + 128.0).unwrap();
            let actual = geometry::selected_lanes(range, 64.0, 64.0, mode).unwrap();
            assert_eq!(
                actual,
                (
                    row["lo"].as_i64().unwrap() as i32,
                    row["hi"].as_i64().unwrap() as i32
                ),
                "{row} {mode:?}"
            );
        }
        for row in case["ranges"].as_array().unwrap() {
            let r = &row["result"];
            let spacing = match row["spacing"].as_u64().unwrap() {
                0 => SpacingMode::ConstantTime,
                1 => SpacingMode::ConstantRow,
                _ => SpacingMode::Variable,
            };
            let settings = Settings {
                mode: spacing,
                reverse: false,
                speeds: Speeds::default(),
                interpolated_scroll: 1.0,
                zoom: 1.0,
            };
            for (axis, lo, hi) in [
                (Axis::Time, "timeLo", "timeHi"),
                (Axis::Position, "rowLo", "rowHi"),
            ] {
                let actual = geometry::adjusted_range(
                    &index,
                    settings,
                    axis,
                    Range::normalized(n(r, "min"), n(r, "max")).unwrap(),
                    n(r, "half"),
                    mode,
                )
                .unwrap();
                assert_eq!(
                    (actual.min, actual.max),
                    (n(r, lo), n(r, hi)),
                    "{row} {mode:?} {axis:?}"
                );
            }
        }
        for row in case["rects"].as_array().unwrap() {
            let actual = geometry::contains_rect(
                mode,
                Rect {
                    x: n(row, "x"),
                    y: n(row, "y"),
                    width: 40.0 - n(row, "x"),
                    height: 60.0 - n(row, "y"),
                },
                Rect {
                    x: 10.0,
                    y: 20.0,
                    width: 30.0,
                    height: 40.0,
                },
            );
            assert_eq!(actual, row["selected"].as_bool().unwrap(), "{row} {mode:?}");
        }
    }
}
#[test]
fn zero_rate_signed_padding_reverse_and_strict_head_contracts_remain_source_faithful() {
    let index = index(0.0);
    let settings = Settings {
        mode: SpacingMode::Variable,
        reverse: true,
        speeds: Speeds::default(),
        interpolated_scroll: 1.0,
        zoom: 1.0,
    };
    let expanded = geometry::adjusted_range(
        &index,
        settings,
        Axis::Position,
        Range::normalized(96.0, 120.0).unwrap(),
        32.0,
        Overlap::Any,
    )
    .unwrap();
    assert_eq!(expanded.min, f64::NEG_INFINITY);
    assert_eq!(expanded.max, f64::INFINITY);
    assert!(geometry::contains_head(expanded, 96.0));
    let shrunk = geometry::adjusted_range(
        &index,
        settings,
        Axis::Position,
        Range::normalized(96.0, 120.0).unwrap(),
        32.0,
        Overlap::All,
    )
    .unwrap();
    assert!(!geometry::contains_head(shrunk, 96.0));
    let bounds = Range::normalized(12.0, 48.0).unwrap();
    assert!(!geometry::contains_head(bounds, 12.0));
    assert!(geometry::contains_head(bounds, 48.0));
    assert!(!geometry::contains_head(
        Range {
            min: 12.0,
            max: 12.0
        },
        12.0
    ));
}
#[test]
fn native_zoom_geometry_uses_actual_rectangles_and_preserves_source_edge_asymmetry() {
    for zoom in [0.5, 1.0, 1.5] {
        let spacing = 64.0;
        let width = spacing * zoom;
        let left = spacing * 1.5 - width * 0.5;
        let right = left + width;
        let center = spacing * 1.5;
        let lane_range = |lo, hi, mode| {
            geometry::selected_lanes(Range::normalized(lo, hi).unwrap(), spacing, width, mode)
                .unwrap()
        };
        assert!(
            lane_range(left, right, Overlap::All).0 <= 1
                && lane_range(left, right, Overlap::All).1 >= 1
        );
        assert_eq!(
            lane_range(center, center, Overlap::Center).0,
            2,
            "center at region lower bound excludes lane1"
        );
        assert_eq!(
            lane_range(right, right + 1.0, Overlap::Any).0,
            2,
            "right edge exactly at lower bound excludes lane1"
        );
        assert!(
            lane_range(left - 1.0, left, Overlap::Any).1 >= 1,
            "left edge at upper bound includes lane1"
        );
    }
}
#[test]
fn actual_original_misc_scans_replay_repeated_active_rate_layout_and_selection_order() {
    use deadsync_editor::region_selection::misc_layout::{Kind, Layout};
    use std::collections::BTreeSet;
    let cases: Value = serde_json::from_str(include_str!(
        "fixtures/key_commands/region-full-misc-source.json"
    ))
    .unwrap();
    let kinds = [
        Kind::Signature,
        Kind::Stop,
        Kind::Delay,
        Kind::Warp,
        Kind::Tick,
        Kind::Combo,
        Kind::Label,
        Kind::Attack,
        Kind::Preview,
        Kind::EndHint,
        Kind::Pattern,
        Kind::Tempo,
        Kind::Scroll,
        Kind::Speed,
        Kind::Fake,
    ];
    for case in cases.as_array().unwrap() {
        let mut layout = Layout::new(-130.0, 130.0, 100, 2.0).unwrap();
        let mut first_seen = Vec::new();
        let mut seen = BTreeSet::new();
        for call in case["calls"].as_array().unwrap() {
            let id = call["id"].as_u64().unwrap();
            let e = case["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["id"].as_u64() == Some(id))
                .unwrap();
            let key = if case["spacing"].as_u64() == Some(0) {
                n(e, "time")
            } else {
                n(e, "row")
            };
            layout
                .position(
                    id,
                    kinds[e["kind"].as_u64().unwrap() as usize],
                    key,
                    n(call, "y"),
                    n(e, "w"),
                    n(e, "h"),
                )
                .unwrap();
            if seen.insert(id) {
                first_seen.push(id);
            }
        }
        for e in case["events"].as_array().unwrap() {
            let actual = layout.rectangle(e["id"].as_u64().unwrap());
            if e["positioned"] == true {
                let rect = actual.unwrap();
                assert_eq!(
                    (rect.x, rect.y, rect.width, rect.height),
                    (n(e, "x"), n(e, "y"), n(e, "w"), n(e, "h")),
                    "{case}"
                );
            } else {
                assert!(actual.is_none());
            }
        }
        let expected: Vec<_> = case["selected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        let overlap = mode(case["mode"].as_u64().unwrap());
        let region = Rect {
            x: -500.0,
            y: n(case, "top"),
            width: 1000.0,
            height: n(case, "bottom") - n(case, "top"),
        };
        let actual: Vec<_> = first_seen
            .into_iter()
            .filter(|&id| geometry::contains_rect(overlap, region, layout.rectangle(id).unwrap()))
            .collect();
        assert_eq!(actual, expected, "{case}");
    }
}
