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
                // Viewport discovery must retain every directly projected note,
                // including later sections which fold back into the viewport.
                for (top, bottom) in [(110.0, 210.0), (160.0, 160.0), (-100.0, 0.0)] {
                    let spans = cursor.visible_rows(top, bottom, 400).unwrap();
                    assert!(spans.len() <= index.sections().len());
                    for row in 0..=400 {
                        for order in [8, 10, 11, 12] {
                            let section = index.event_active(row as f64, order).unwrap().unwrap();
                            let time = index.sections()[section]
                                .mapping
                                .time_at(row as f64)
                                .unwrap();
                            let mut probe = cursor;
                            let y = probe.event_y(time, row as f64, order).unwrap();
                            if y >= top && y <= bottom {
                                assert!(
                                    spans.iter().any(|s| s.section == section
                                        && s.start <= row
                                        && row <= s.end),
                                    "{name}: row {row}, phase {order}, section {section}, Y {y}, viewport {top}..{bottom}: {spans:?}"
                                );
                            }
                        }
                    }
                }
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
                        "HoldEnd" => Some(
                            cursor
                                .hold_end_y(n(step, "Row"), n(step, "CapHeight"))
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
    for (row, cap) in [(100.0, 0.0), (0.0, -1.0), (0.0, f64::NAN), (f64::NAN, 0.0)] {
        assert!(cursor.hold_end_y(row, cap).is_err());
        assert_eq!(cursor.section(), 0);
        assert_eq!(cursor.rate_anchor().y, before);
    }
    assert!(cursor.move_to(99).is_err());
    assert!(cursor.event_y(f64::NAN, 0.0, 8).is_err());
    assert_eq!(cursor.rate_anchor().y, before);
}

#[test]
fn hold_tail_at_delay_uses_note_phase_and_cap_follows_reverse() {
    let source = b"#VERSION:0.83;#BPMS:0=120;#DELAYS:2=0.5;#STOPS:2=0.75;#SCROLLS:0=-1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let session =
        Session::new(Document::parse(Arc::<[u8]>::from(source.as_slice()), Format::Ssc).unwrap());
    let clock = Clock::capture(&session, 0).unwrap();
    let index = Index::new(clock.spacing_sections().collect()).unwrap();
    for reverse in [false, true] {
        for (mode, delta) in [
            (SpacingMode::ConstantTime, 150.0),
            (SpacingMode::ConstantRow, 0.0),
            (SpacingMode::Variable, 0.0),
        ] {
            let settings = Settings {
                mode,
                reverse,
                speeds: Speeds::default(),
                interpolated_scroll: 1.0,
                zoom: 1.0,
            };
            let focal = Anchor {
                y: 160.0,
                time: clock.chart_time(96).unwrap(),
                row: 96.0,
            };
            let mut cursor = Cursor::begin(&index, focal, settings).unwrap().unwrap();
            let direction = if reverse { -1.0 } else { 1.0 };
            close(
                cursor.hold_end_y(96.0, 7.5).unwrap(),
                160.0 + (delta + 7.5) * direction,
            );
            let note = cursor
                .event_y(clock.note_time(96).unwrap(), 96.0, 11)
                .unwrap();
            close(
                cursor.hold_end_y(96.0, 7.5).unwrap(),
                note + 7.5 * direction,
            );
        }
    }
}

#[test]
fn viewport_retains_disjoint_folded_sections_and_collapsed_domains() {
    let index = Index::new(
        [1.0, -1.0, 1.0]
            .into_iter()
            .enumerate()
            .map(|(i, scroll)| SpacingSection {
                order: 13,
                mapping: RateMapping::new((i * 48) as f64, i as f64 * 0.5, 0.0, 0.0, 1.0 / 96.0)
                    .unwrap(),
                tempo: 120.0,
                scroll,
            })
            .collect(),
    )
    .unwrap();
    let focal = Anchor {
        y: 0.0,
        time: 0.0,
        row: 0.0,
    };
    let settings = Settings {
        mode: SpacingMode::Variable,
        reverse: false,
        speeds: Speeds::default(),
        interpolated_scroll: 1.0,
        zoom: 1.0,
    };
    let cursor = Cursor::begin(&index, focal, settings).unwrap().unwrap();
    let spans = cursor.visible_rows(20.0, 30.0, 200).unwrap();
    for row in [20, 76, 116] {
        assert!(spans.iter().any(|s| s.start <= row && row <= s.end));
    }
    assert!(!spans.iter().any(|s| s.start <= 40 && 40 <= s.end));
    assert_eq!(cursor.section(), 0);
    assert_eq!(cursor.rate_anchor().y, 0.0);
    assert!(cursor.visible_rows(f64::NAN, 0.0, 200).is_err());
    assert!(cursor.visible_rows(1.0, 0.0, 200).is_err());

    let collapsed = Cursor::begin(
        &index,
        focal,
        Settings {
            interpolated_scroll: 0.0,
            ..settings
        },
    )
    .unwrap()
    .unwrap();
    let spans = collapsed.visible_rows(0.0, 0.0, u64::MAX).unwrap();
    assert_eq!(spans.first().unwrap().start, 0);
    assert_eq!(spans.last().unwrap().end, u64::MAX);
    assert!(
        collapsed
            .visible_rows(1.0, 2.0, u64::MAX)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn time_viewport_keeps_entire_warp_and_negative_stop_plateaus_without_row_walks() {
    for (warp, stop, last, expected_end) in [
        (192.0, 0.0, 400, 192),
        (192.0, -0.5, 400, 240),
        (u64::MAX as f64, 0.0, u64::MAX, u64::MAX),
    ] {
        let index = Index::new(vec![SpacingSection {
            order: 17,
            mapping: RateMapping::new(0.0, 0.0, warp, stop, 1.0 / 96.0).unwrap(),
            tempo: 120.0,
            scroll: 1.0,
        }])
        .unwrap();
        for reverse in [false, true] {
            let settings = Settings {
                mode: SpacingMode::ConstantTime,
                reverse,
                speeds: Speeds::default(),
                interpolated_scroll: 1.0,
                zoom: 1.0,
            };
            let cursor = Cursor::begin(
                &index,
                Anchor {
                    y: 160.0,
                    time: 0.0,
                    row: 0.0,
                },
                settings,
            )
            .unwrap()
            .unwrap();
            let spans = cursor.visible_rows(160.0, 160.0, last).unwrap();
            assert_eq!(spans.len(), 1);
            assert_eq!((spans[0].start, spans[0].end), (0, expected_end));
        }
    }
}

#[test]
fn frame_queries_are_stable_and_match_canonical_section_traversal() {
    use deadsync_editor::spacing_geometry::Frame;
    let index = Index::new(
        [1.0, -1.0, 0.0, 0.5]
            .into_iter()
            .enumerate()
            .map(|(i, scroll)| SpacingSection {
                order: 13,
                mapping: RateMapping::new(i as f64 * 48.0, i as f64 * 0.5, 0.0, 0.0, 1.0 / 96.0)
                    .unwrap(),
                tempo: 120.0,
                scroll,
            })
            .collect(),
    )
    .unwrap();
    for reverse in [false, true] {
        for factor in [1.0, -0.5, 0.0] {
            let settings = Settings {
                mode: SpacingMode::Variable,
                reverse,
                speeds: Speeds::default(),
                interpolated_scroll: factor,
                zoom: 1.2,
            };
            let origin = Cursor::begin(
                &index,
                Anchor {
                    y: 160.0,
                    time: 0.75,
                    row: 72.0,
                },
                settings,
            )
            .unwrap()
            .unwrap();
            let frame = Frame::capture(origin).unwrap();
            let mut canonical = origin;
            canonical.move_to(0).unwrap();
            let mut expected = Vec::new();
            for row in 0..=300 {
                for order in [0, 8, 11, 13, 18] {
                    let section = index.event_active(row as f64, order).unwrap().unwrap();
                    canonical.move_to(section).unwrap();
                    let time = index.sections()[section]
                        .mapping
                        .time_at(row as f64)
                        .unwrap();
                    let y = canonical.y_in_section(time, row as f64).unwrap();
                    close(frame.row_y(row as f64, order).unwrap(), y);
                    expected.push((row, order, y));
                }
            }
            for (row, order, y) in expected.into_iter().rev() {
                close(frame.row_y(row as f64, order).unwrap(), y);
            }
            assert_eq!(
                frame.visible_rows(105.0, 395.0, 300).unwrap(),
                origin.visible_rows(105.0, 395.0, 300).unwrap()
            );
            assert!(frame.row_y(f64::NAN, 8).is_err());
        }
    }
}

#[test]
fn sparse_grid_bound_includes_hold_tails_and_all_participants() {
    use deadsync_editor::notes::{Grid, Kind, Note, Position};
    let mut grid = Grid::new(4, 2).unwrap();
    assert_eq!(grid.last_row(), 0);
    let pos = Position {
        row: 10,
        lane: 0,
        player: 1,
    };
    grid.set(
        pos,
        Some(Note {
            kind: Kind::Hold,
            end: Some(1000),
            attributes: Arc::from(""),
            tail_attributes: Arc::from(""),
        }),
    )
    .unwrap();
    grid.set(
        Position {
            row: 200,
            lane: 1,
            player: 0,
        },
        Some(Note::tap()),
    )
    .unwrap();
    assert_eq!(grid.last_row(), 1000);
    grid.set(pos, None).unwrap();
    assert_eq!(grid.last_row(), 200);
}
