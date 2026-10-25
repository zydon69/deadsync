use deadsync_editor::{
    document::{Document, Format},
    generation::editor_timing::SpacingSection,
    history::Session,
    playback::Clock,
    rate_mapping::RateMapping,
    spacing::{Anchor, Speeds},
    spacing_geometry::Cursor,
    spacing_index::Index,
    spacing_search::Settings,
    zoom::SpacingMode,
};
use serde_json::Value;
use std::{fs, path::Path, sync::Arc};
fn n(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap()
}
fn close(a: f64, b: f64) {
    assert!(
        a == b || a.to_bits().abs_diff(b.to_bits()) <= 2,
        "{a:?} != {b:?}"
    );
}
#[test]
fn geometry_cursor_matches_original_helper_sequences_across_signed_sections() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    for (name, mode) in [
        ("constanttime", SpacingMode::ConstantTime),
        ("constantrow", SpacingMode::ConstantRow),
        ("variable", SpacingMode::Variable),
    ] {
        for sign in ["normal", "negative", "zero"] {
            for suffix in ["", "-reverse"] {
                let name = format!("spacing-geometry-{name}-{sign}{suffix}");
                let request: Value = serde_json::from_slice(
                    &fs::read(root.join(format!("{name}.request.json"))).unwrap(),
                )
                .unwrap();
                let expected: Value = serde_json::from_slice(
                    &fs::read(root.join(format!("{name}.expected.json"))).unwrap(),
                )
                .unwrap();
                let sections = request["Rates"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| SpacingSection {
                        order: r["Order"].as_u64().unwrap() as u8,
                        mapping: RateMapping::new(
                            n(r, "Row"),
                            n(r, "Time"),
                            n(r, "Warp"),
                            n(r, "Stop"),
                            n(r, "SecondsPerRow"),
                        )
                        .unwrap(),
                        tempo: n(r, "Tempo"),
                        scroll: n(r, "Scroll"),
                    })
                    .collect();
                let index = Index::new(sections).unwrap();
                let f = &request["Focal"];
                let focal = Anchor {
                    y: n(f, "Y"),
                    time: n(f, "Time"),
                    row: n(f, "Row"),
                };
                let s = &request["Speeds"];
                let settings = Settings {
                    mode,
                    reverse: request["Reverse"].as_bool().unwrap(),
                    speeds: Speeds {
                        time: n(s, "Time"),
                        row: n(s, "Row"),
                        variable: n(s, "Variable"),
                    },
                    interpolated_scroll: n(&request, "Interpolated"),
                    zoom: n(&request, "Zoom"),
                };
                let mut cursor = Cursor::begin(&index, focal, settings).unwrap().unwrap();
                assert_eq!(
                    cursor.section(),
                    request["Active"].as_u64().unwrap() as usize
                );
                let steps = request["Steps"].as_array().unwrap();
                assert_eq!(steps.len(), expected.as_array().unwrap().len());
                for (step, e) in steps.iter().zip(expected.as_array().unwrap()) {
                    let event_y = match step["Kind"].as_str().unwrap() {
                        "Move" => {
                            cursor
                                .move_to(step["Section"].as_u64().unwrap() as usize)
                                .unwrap();
                            None
                        }
                        "Sample" => Some(
                            cursor
                                .y_in_section(n(step, "Time"), n(step, "Row"))
                                .unwrap(),
                        ),
                        "Event" => Some(
                            cursor
                                .event_y(
                                    n(step, "Time"),
                                    n(step, "Row"),
                                    step["Order"].as_u64().unwrap() as u8,
                                )
                                .unwrap(),
                        ),
                        _ => panic!("Unknown step"),
                    };
                    assert_eq!(cursor.section(), e["Section"].as_u64().unwrap() as usize);
                    close(cursor.rate_anchor().y, n(e, "AnchorY"));
                    match event_y {
                        Some(y) => close(y, n(e, "EventY")),
                        None => assert!(e["EventY"].is_null()),
                    }
                }
            }
        }
    }
}
#[test]
fn note_geometry_uses_delayed_event_time_and_phase_instead_of_cursor_time() {
    let source=b"#VERSION:0.83;#BPMS:0=120;#DELAYS:2=0.5;#STOPS:2=0.75;#WARPS:2=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let session =
        Session::new(Document::parse(Arc::<[u8]>::from(source.as_slice()), Format::Ssc).unwrap());
    let clock = Clock::capture(&session, 0).unwrap();
    let index = Index::new(clock.spacing_sections().collect()).unwrap();
    assert_eq!(clock.chart_time(96).unwrap(), 1.0);
    assert_eq!(clock.note_time(96).unwrap(), 1.5);
    for (mode, expected) in [
        (SpacingMode::ConstantTime, 310.0),
        (SpacingMode::ConstantRow, 160.0),
        (SpacingMode::Variable, 160.0),
    ] {
        let settings = Settings {
            mode,
            reverse: false,
            speeds: Speeds::default(),
            interpolated_scroll: 1.0,
            zoom: 1.0,
        };
        let mut cursor = Cursor::begin(
            &index,
            Anchor {
                y: 160.0,
                time: clock.chart_time(96).unwrap(),
                row: 96.0,
            },
            settings,
        )
        .unwrap()
        .unwrap();
        close(
            cursor
                .event_y(clock.note_time(96).unwrap(), 96.0, 8)
                .unwrap(),
            expected,
        );
    }
}
#[test]
fn empty_geometry_and_failed_traversals_preserve_cursor_state() {
    let settings = Settings {
        mode: SpacingMode::Variable,
        reverse: false,
        speeds: Speeds::default(),
        interpolated_scroll: 1.0,
        zoom: 1.0,
    };
    let focal = Anchor {
        y: 160.0,
        time: 0.125,
        row: 12.0,
    };
    let empty = Index::new(vec![]).unwrap();
    assert!(Cursor::begin(&empty, focal, settings).unwrap().is_none());
    let index = Index::new(vec![
        SpacingSection {
            order: 1,
            mapping: RateMapping::new(0.0, 0.0, 0.0, 0.0, 1.0 / 96.0).unwrap(),
            tempo: 120.0,
            scroll: 1.0,
        },
        SpacingSection {
            order: 13,
            mapping: RateMapping::new(48.0, 0.5, 0.0, 0.0, 60.0 / 1e308 / 48.0).unwrap(),
            tempo: 1e308,
            scroll: 2.0,
        },
    ])
    .unwrap();
    let mut cursor = Cursor::begin(&index, focal, settings).unwrap().unwrap();
    let before = cursor.rate_anchor().y;
    assert!(cursor.move_to(1).is_err());
    assert_eq!(cursor.section(), 0);
    assert_eq!(cursor.rate_anchor().y, before);
    assert!(cursor.event_y(1.0, 100.0, 8).is_err());
    assert_eq!(cursor.section(), 0);
    assert_eq!(cursor.rate_anchor().y, before);
    assert!(cursor.move_to(99).is_err());
    assert!(cursor.event_y(f64::NAN, 0.0, 8).is_err());
    assert_eq!(cursor.rate_anchor().y, before);
}
