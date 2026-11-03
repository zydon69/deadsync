use deadsync_editor::{
    generation::editor_timing::SpacingSection,
    rate_mapping::RateMapping,
    region_selection::{
        geometry::{self},
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
fn mode_overlap(n: u64) -> Overlap {
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
fn native_scan_executes_original_full_traversal_earlyouts_and_layout_for_216_cases() {
    use deadsync_editor::{
        region_selection::{
            misc_layout::{Kind, Layout},
            misc_scan::{self, Bounds, Item},
        },
        spacing::Anchor,
    };
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
        let index = index(-1.0);
        let mode = match case["spacing"].as_u64().unwrap() {
            0 => SpacingMode::ConstantTime,
            1 => SpacingMode::ConstantRow,
            _ => SpacingMode::Variable,
        };
        let settings = Settings {
            mode,
            reverse: case["reverse"].as_bool().unwrap(),
            speeds: Speeds::default(),
            interpolated_scroll: 1.0,
            zoom: 1.0,
        };
        let mut events = case["events"]
            .as_array()
            .unwrap()
            .iter()
            .collect::<Vec<_>>();
        events.sort_by(|a, b| {
            n(a, "row")
                .total_cmp(&n(b, "row"))
                .then(a["id"].as_u64().cmp(&b["id"].as_u64()))
        });
        let items = events
            .iter()
            .map(|e| Item {
                row: n(e, "row"),
                time: n(e, "time"),
                phase: e["id"].as_u64().unwrap() as u8,
            })
            .collect::<Vec<_>>();
        let rates = (0..3)
            .map(|id| events.iter().position(|e| e["id"].as_u64() == Some(id)))
            .collect::<Vec<_>>();
        let focal = n(case, "focal");
        let min = n(case, "min");
        let mut layout = Layout::new(-130.0, 130.0, 100, 2.0).unwrap();
        let mut calls = vec![];
        let result = misc_scan::scan(
            &index,
            Anchor {
                y: 200.0,
                row: focal,
                time: if focal == 96.0 { 0.75 } else { focal / 96.0 },
            },
            settings,
            Bounds {
                start_time: min / 96.0,
                end_time: (min + 48.0) / 96.0,
                start_row: min,
                end_row: min + 48.0,
            },
            &items,
            &rates,
            |i, y| {
                let e = events[i];
                let id = e["id"].as_u64().unwrap();
                calls.push(serde_json::json!({"id":id,"y":y}));
                layout
                    .position(
                        id,
                        kinds[e["kind"].as_u64().unwrap() as usize],
                        if mode == SpacingMode::ConstantTime {
                            n(e, "time")
                        } else {
                            n(e, "row")
                        },
                        y,
                        80.0,
                        20.0,
                    )
                    .unwrap();
                Ok(layout.maximum_single_row_height(20.0).unwrap())
            },
        )
        .unwrap();
        let expected_calls = case["calls"].as_array().unwrap();
        let positions = |calls: &[Value]| {
            calls.iter().map(|call| (call["id"].as_u64().unwrap(), call["y"].as_f64().unwrap())).collect::<Vec<_>>()
        };
        // Compare exact numeric coordinates and call order. .NET emits whole
        // doubles as JSON integers; serde emits their decimal representation.
        assert_eq!(positions(&calls), positions(expected_calls), "{case}");
        let mut top = result.start_y.unwrap();
        let mut bottom = result.end_y.unwrap();
        if top > bottom {
            std::mem::swap(&mut top, &mut bottom);
        }
        assert_eq!((top, bottom), (n(case, "top"), n(case, "bottom")), "{case}");
        let region = Rect {
            x: -500.0,
            y: top,
            width: 1000.0,
            height: bottom - top,
        };
        let overlap = mode_overlap(case["mode"].as_u64().unwrap());
        let selected = result
            .order
            .iter()
            .filter_map(|&i| {
                let id = events[i]["id"].as_u64().unwrap();
                geometry::contains_rect(overlap, region, layout.rectangle(id).unwrap())
                    .then_some(id)
            })
            .collect::<Vec<_>>();
        assert_eq!(serde_json::json!(selected), case["selected"], "{case}");
        for e in events {
            let rect = layout.rectangle(e["id"].as_u64().unwrap());
            assert_eq!(rect.is_some(), e["positioned"].as_bool().unwrap());
            if let Some(r) = rect {
                assert_eq!(
                    (r.x, r.y, r.width, r.height),
                    (n(e, "x"), n(e, "y"), n(e, "w"), n(e, "h")),
                    "{case}"
                );
            }
        }
    }
}
