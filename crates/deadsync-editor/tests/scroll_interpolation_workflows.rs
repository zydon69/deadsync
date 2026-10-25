use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    playback::Clock,
    scroll_interpolation::Timeline,
    timing_edit::{Scope, TempoDraft},
};
use serde_json::Value;
use std::{fs, path::Path, sync::Arc};
fn session(text: &str, format: Format) -> Session {
    Session::new(Document::parse(Arc::<[u8]>::from(text.as_bytes()), format).unwrap())
}
fn close(a: f64, b: f64) {
    assert!(
        a == b || a.to_bits().abs_diff(b.to_bits()) <= 2,
        "{a:?} != {b:?}"
    );
}
fn source(value: &Value) -> String {
    let mut text = String::from("#VERSION:0.83;#TITLE:Interpolation;");
    for (tag, kind) in [
        ("BPMS", "tempo"),
        ("STOPS", "stop"),
        ("DELAYS", "delay"),
        ("WARPS", "warp"),
        ("TIMESIGNATURES", "signature"),
    ] {
        let entries: Vec<_> = value["Timing"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e[1] == kind)
            .map(|e| {
                let beat = e[0].as_f64().unwrap() / 48.0;
                if kind == "signature" {
                    format!("{beat}={}={}", e[2], e[3])
                } else if kind == "warp" {
                    format!("{beat}={}", e[2].as_f64().unwrap() / 48.0)
                } else {
                    format!("{beat}={}", e[2])
                }
            })
            .collect();
        if !entries.is_empty() {
            text.push_str(&format!("#{tag}:{};", entries.join(",")));
        }
    }
    text.push_str(&format!(
        "#SPEEDS:{};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        value["Source"].as_str().unwrap()
    ));
    text
}
#[test]
fn timed_interpolation_matches_original_event_phase_and_interpolation_methods() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    for name in [
        "baseline",
        "same-row-delay-stop-warp",
        "tempo-changes",
        "negative-stop-warp",
        "late-first-signed",
        "empty",
    ] {
        let request: Value = serde_json::from_slice(
            &fs::read(root.join(format!("scroll-interpolation-{name}.request.json"))).unwrap(),
        )
        .unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(format!("scroll-interpolation-{name}.expected.json"))).unwrap(),
        )
        .unwrap();
        let source = source(&request);
        let session = session(&source, Format::Ssc);
        let clock = Clock::capture(&session, 0).unwrap();
        let timeline = Timeline::capture(&session, 0).unwrap();
        for origin in expected["Origins"].as_array().unwrap() {
            let row = origin["Row"].as_u64().unwrap();
            let time = origin["Time"].as_f64().unwrap();
            close(clock.speed_event_time(row).unwrap(), time);
            if let Some(captured) = timeline.time_origins().get(&row) {
                close(*captured, time);
            }
        }
        for q in expected["Speeds"]["Initial"]["Queries"].as_array().unwrap() {
            close(
                timeline
                    .rate_at(q["Row"].as_f64().unwrap(), q["Time"].as_f64().unwrap())
                    .unwrap(),
                q["Rate"].as_f64().unwrap(),
            );
        }
        assert!(!session.is_dirty());
        assert_eq!(session.serialize().unwrap(), source.as_bytes());
    }
}
#[test]
fn speed_origin_is_after_same_row_delay_and_before_stop_and_snapshots_are_immutable() {
    let text = "#VERSION:0.83;#BPMS:0=120;#DELAYS:2=0.5;#STOPS:2=0.75;#OFFSET:2;#SPEEDS:0=1=0=0,2=2=1=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let mut session = session(text, Format::Ssc);
    let snapshot = Timeline::capture(&session, 0).unwrap();
    let clock = Clock::capture(&session, 0).unwrap();
    assert_eq!(clock.speed_event_time(96).unwrap(), 1.5);
    assert_eq!(snapshot.time_origins()[&96], 1.5);
    assert_eq!(snapshot.rate_at(96.0, 1.5).unwrap(), 1.0);
    assert_eq!(snapshot.rate_at(96.0, 2.0).unwrap(), 1.5);
    let mut tempo = TempoDraft::capture(&session, 0, Scope::Song).unwrap();
    tempo.set(0, 240.0).unwrap();
    tempo.apply(&mut session, 0).unwrap();
    let updated = Timeline::capture(&session, 0).unwrap();
    assert_eq!(updated.time_origins()[&96], 1.0);
    assert_eq!(snapshot.time_origins()[&96], 1.5);
}
#[test]
fn split_timing_scope_and_unsupported_sm_speed_metadata_follow_reader_semantics() {
    let text = "#VERSION:0.83;#BPMS:0=120;#SPEEDS:0=3=0=0;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=240;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#SPEEDS:0=9=0=0;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let value = session(text, Format::Ssc);
    assert_eq!(
        Timeline::capture(&value, 0)
            .unwrap()
            .rate_at(48.0, 0.5)
            .unwrap(),
        1.0
    );
    assert_eq!(
        Timeline::capture(&value, 1)
            .unwrap()
            .rate_at(48.0, 0.5)
            .unwrap(),
        9.0
    );
    assert_eq!(
        Timeline::capture(&value, 2)
            .unwrap()
            .rate_at(48.0, 0.5)
            .unwrap(),
        3.0
    );
    let sm = session(
        "#BPMS:0=120;#SPEEDS:0=NaN=1=1;#NOTES:dance-single::Hard:1::1000;",
        Format::Sm,
    );
    let speed = Timeline::capture(&sm, 0).unwrap();
    assert_eq!(speed.rate_at(48.0, 0.5).unwrap(), 1.0);
    assert!(speed.time_origins().is_empty());
    assert!(Timeline::capture(&value, 3).is_err());
    assert!(speed.rate_at(f64::NAN, 0.0).is_err());
    assert!(speed.rate_at(0.0, f64::INFINITY).is_err());
}
