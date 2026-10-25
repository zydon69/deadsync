use deadsync_editor::{
    generation::{
        editor_timing::EditorTimeline,
        timing::{Event, EventKind, GenerationTiming},
    },
    notes::Grid,
    spacing::{Helper, Speeds},
    zoom::SpacingMode,
};
use serde_json::Value;
use std::{fs, path::Path, sync::atomic::AtomicBool};
fn close(a: f64, b: f64) {
    assert!(
        a == b || a.to_bits().abs_diff(b.to_bits()) <= 2,
        "{a:?} != {b:?}"
    );
}
fn timing(value: &Value) -> GenerationTiming {
    GenerationTiming::new(
        value["Timing"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| Event {
                row: e[0].as_i64().unwrap(),
                kind: match e[1].as_str().unwrap() {
                    "tempo" => EventKind::Tempo(e[2].as_f64().unwrap()),
                    "scroll" => EventKind::Scroll(e[2].as_f64().unwrap()),
                    "stop" => EventKind::Stop(e[2].as_f64().unwrap()),
                    "delay" => EventKind::Delay(e[2].as_f64().unwrap()),
                    "warp" => EventKind::Warp(e[2].as_i64().unwrap()),
                    "signature" => EventKind::TimeSignature,
                    _ => panic!("Unexpected timing fixture"),
                },
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn initialized_sections_match_original_refresh_methods_including_late_first_values() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    for name in [
        "baseline",
        "late-tempo-and-scroll",
        "late-negative-scroll",
        "late-tempo",
        "zero-scroll",
        "overlapping-warp-stop",
    ] {
        let request: Value = serde_json::from_slice(
            &fs::read(root.join(format!("spacing-sections-{name}.request.json"))).unwrap(),
        )
        .unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(format!("spacing-sections-{name}.expected.json"))).unwrap(),
        )
        .unwrap();
        let timeline = EditorTimeline::build(
            &timing(&request),
            &Grid::new(4, 1).unwrap(),
            None,
            &AtomicBool::new(false),
        )
        .unwrap();
        let sections: Vec<_> = timeline.spacing_sections().collect();
        let expected = expected.as_array().unwrap();
        assert_eq!(sections.len(), expected.len(), "{name}");
        let mut helper = Helper::new(SpacingMode::Variable, false);
        for (section, e) in sections.into_iter().zip(expected) {
            for (actual, key) in [
                (section.mapping.row(), "Row"),
                (section.mapping.time(), "Time"),
                (section.mapping.warp(), "Warp"),
                (section.mapping.stop(), "Stop"),
                (section.mapping.seconds_per_row(), "SecondsPerRow"),
                (section.mapping.rows_per_second(), "RowsPerSecond"),
                (section.tempo, "Tempo"),
                (section.scroll, "Scroll"),
            ] {
                close(actual, e[key].as_f64().unwrap());
            }
            let rate = section.spacing_rate();
            assert_eq!(rate.row, section.mapping.row());
            assert_eq!(rate.time, section.mapping.time());
            assert_eq!(rate.scroll, section.scroll);
            helper.update(rate, Speeds::default(), 1.0, 1.0).unwrap();
            close(
                helper.pixels_per_second(),
                300.0 * (e["Tempo"].as_f64().unwrap() / 300.0) * e["Scroll"].as_f64().unwrap(),
            );
            assert_eq!(helper.negative_scroll(), section.scroll < 0.0);
        }
    }
}
#[test]
fn absent_scroll_keeps_default_and_late_first_scroll_backfills_prior_rates() {
    for scroll in [None, Some(0.0), Some(-2.0), Some(1.75)] {
        let mut events = vec![
            Event {
                row: 0,
                kind: EventKind::TimeSignature,
            },
            Event {
                row: 48,
                kind: EventKind::Tempo(123.456),
            },
        ];
        if let Some(value) = scroll {
            events.push(Event {
                row: 96,
                kind: EventKind::Scroll(value),
            });
        }
        let timing = GenerationTiming::new(events).unwrap();
        let timeline = EditorTimeline::build(
            &timing,
            &Grid::new(4, 1).unwrap(),
            None,
            &AtomicBool::new(false),
        )
        .unwrap();
        for section in timeline.spacing_sections() {
            assert_eq!(section.tempo, 123.456);
            assert_eq!(section.scroll, scroll.unwrap_or(1.0));
            close(section.mapping.seconds_per_row(), 60.0 / 123.456 / 48.0);
        }
    }
}
