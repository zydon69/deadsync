use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    playback::Clock,
};

fn session(timing: &str) -> Session {
    let source = format!(
        "#VERSION:0.83;#BPMS:0=120;#OFFSET:0.25;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0.1}};#NOTEDATA:;#STEPSTYPE:dance-single;{timing}#NOTES:1000;"
    );
    Session::new(Document::parse(source.into_bytes(), Format::Ssc).unwrap())
}
fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-11, "{actual} != {expected}");
}
#[test]
fn continuous_cursor_keeps_fractional_rows_and_negative_lead_in_without_editing() {
    let session = session("");
    let before = session.serialize().unwrap();
    let revision = session.revision();
    let clock = Clock::capture(&session, 0).unwrap();
    for row in [-48.5, -0.5, 0.0, 0.125, 48.5, 500.25] {
        near(clock.fractional_chart_time(row).unwrap(), row / 96.0);
        let music = clock.fractional_music_time(row).unwrap();
        near(music, row / 96.0 - 0.15);
        near(clock.position(music).unwrap(), row);
    }
    assert_eq!(session.serialize().unwrap(), before);
    assert_eq!(session.revision(), revision);
    assert!(!session.is_dirty());
}
#[test]
fn equal_row_precedes_pauses_but_fractional_row_after_it_observes_pause_and_warp() {
    let session = session("#STOPS:1=1;#DELAYS:2=0.25;#WARPS:3=1;");
    let clock = Clock::capture(&session, 0).unwrap();
    for (row, time) in [
        (47.5, 47.5 / 96.0),
        (48.0, 0.5),
        (48.5, 1.5 + 0.5 / 96.0),
        (96.0, 2.0),
        (96.5, 2.25 + 0.5 / 96.0),
        (144.0, 2.75),
        (160.5, 2.75),
        (192.5, 2.75 + 0.5 / 96.0),
    ] {
        near(clock.fractional_chart_time(row).unwrap(), time);
    }
    for row in [0, 48, 96, 144, 192, 480] {
        assert_eq!(
            clock.fractional_chart_time(row as f64).unwrap(),
            clock.chart_time(row).unwrap()
        );
    }
}
#[test]
fn fractional_cursor_uses_latest_working_clock_and_rejects_nonfinite_queries() {
    let mut session = session("");
    let old = Clock::capture(&session, 0).unwrap();
    session.set_text(Some(0), "BPMS", "0=60,1=180").unwrap();
    let current = Clock::capture(&session, 0).unwrap();
    near(old.fractional_chart_time(48.5).unwrap(), 48.5 / 96.0);
    near(
        current.fractional_chart_time(48.5).unwrap(),
        1.0 + 0.5 / 144.0,
    );
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(current.fractional_chart_time(value).is_err());
        assert!(current.fractional_music_time(value).is_err());
    }
}
