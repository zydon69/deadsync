use deadsync_editor::minimap_pattern_region;
use deadsync_editor::{
    Error,
    document::{Document, Format},
    history::Session,
    minimap_event_policy, playback,
};
#[test]
fn source_pattern_start_after_tap_and_end_before_equal_row_delay_stop_and_tempo() {
    for (tempo, events) in [
        ("0=120", "#DELAYS:1=0.25,2=0.5;"),
        ("0=120", "#STOPS:1=0.25,2=1;"),
        ("0=120,2=180", "#DELAYS:1=0.25;#STOPS:2=0.5;"),
    ] {
        let source = format!(
            "#VERSION:0.83;#BPMS:{tempo};#NOTEDATA:;#STEPSTYPE:dance-single;{events}#NOTES:1000;"
        );
        let session = Session::new(Document::parse(source.as_bytes(), Format::Ssc).unwrap());
        let clock = playback::Clock::capture(&session, 0).unwrap();
        let pattern = minimap_pattern_region::capture(&clock, 48, 48).unwrap();
        assert_eq!((pattern.start_row, pattern.end_row), (48, 96));
        assert_eq!(pattern.start_time, clock.note_time(48).unwrap());
        assert_eq!(pattern.end_time, clock.fractional_chart_time(96.0).unwrap());
        if events.contains("2=0.5;") && !events.contains("#STOPS:2=0.5;") {
            assert!(
                pattern.end_time < clock.note_time(96).unwrap(),
                "end excludes equal-row Delay"
            );
        }
        if events.contains("#DELAYS:1=0.25") {
            assert!(
                pattern.start_time > clock.fractional_chart_time(48.0).unwrap(),
                "Pattern event begins after delay/tap phase"
            );
        }
        if events.contains("#STOPS:") {
            assert!(
                pattern.end_time < clock.note_time(97).unwrap(),
                "end excludes equal-row Stop"
            );
        }
        assert!(!session.is_dirty());
        assert!(minimap_pattern_region::capture(&clock, u64::MAX, 1).is_err());
    }
}
