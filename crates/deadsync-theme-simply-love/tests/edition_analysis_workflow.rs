//! Native input -> bounded RSSP snapshot job -> actors and density navigation.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    sync::Mutex,
    time::{Duration, Instant},
};
static LOCK: Mutex<()> = Mutex::new(());
const SONG: &str = "#VERSION:0.83;#BPMS:0=120,4=240;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001,\n1100\n0000\n0000\n0000;";
fn key(s: &mut State, code: KeyCode, pressed: bool) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(s: &mut State, code: KeyCode) {
    key(s, code, true);
    key(s, code, false);
}
fn ctrl(s: &mut State, code: KeyCode) {
    key(s, KeyCode::SuperLeft, true);
    press(s, code);
    key(s, KeyCode::SuperLeft, false);
}
fn setup() -> (tempfile::TempDir, State) {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let state = State::open(&path, 0).unwrap();
    (dir, state)
}
fn finish(s: &mut State) {
    let limit = Instant::now() + Duration::from_secs(10);
    while s.is_analyzing() {
        s.update();
        assert!(Instant::now() < limit, "analysis did not finish");
        std::thread::yield_now();
    }
}
fn texts(s: &State) -> Vec<String> {
    let mut actors = Vec::new();
    s.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|a| {
            if let Actor::Text { content, .. } = a {
                Some(content.to_string())
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn f12_analyzes_unsaved_notes_renders_stats_navigates_density_and_reopens_saved_chart() {
    let _guard = LOCK.lock().unwrap();
    let (dir, mut s) = setup();
    press(&mut s, KeyCode::Digit3); // add another arrow in row zero
    press(&mut s, KeyCode::F12);
    assert!(s.is_analyzing());
    finish(&mut s);
    let report = s.analysis_report().unwrap();
    assert_eq!(report.summary().stats.total_arrows, 7);
    assert_eq!(report.summary().measure_densities.len(), 2);
    let labels = texts(&s);
    for expected in [
        "Chart analysis (RSSP)",
        "Arrows: 7",
        "Peak:",
        "Crossovers:",
        "Density per four beats",
        "Interval 1/2",
    ] {
        assert!(
            labels.iter().any(|text| text.contains(expected)),
            "missing {expected}: {labels:?}"
        );
    }
    assert!(!labels.iter().any(|text| text.contains('{')));
    let mut actors = Vec::new();
    s.actors(&mut actors);
    assert_eq!(
        actors
            .iter()
            .filter(|a| matches!(a, Actor::Sprite { z: 8021, .. }))
            .count(),
        2
    );
    press(&mut s, KeyCode::End);
    assert!(texts(&s).iter().any(|t| t.contains("Interval 2/2")));
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.workspace().row(), 192);
    press(&mut s, KeyCode::Home);
    press(&mut s, KeyCode::F12); // cached summary must follow the new cursor
    assert!(!s.is_analyzing());
    assert!(texts(&s).iter().any(|text| text.contains("Interval 1/2")));
    press(&mut s, KeyCode::Escape);
    assert_eq!(
        fs::read_to_string(dir.path().join("song.ssc")).unwrap(),
        SONG
    );
    ctrl(&mut s, KeyCode::KeyS);
    let path = dir.path().join("song.ssc");
    let mut reopened = State::open(&path, 0).unwrap();
    press(&mut reopened, KeyCode::F12);
    finish(&mut reopened);
    assert_eq!(
        reopened
            .analysis_report()
            .unwrap()
            .summary()
            .stats
            .total_arrows,
        7
    );
    assert!(!reopened.workspace().file().session.is_dirty());
}
#[test]
fn analysis_cache_tracks_revision_menu_pages_open_it_and_refresh_keeps_history() {
    let _guard = LOCK.lock().unwrap();
    let (_dir, mut s) = setup();
    press(&mut s, KeyCode::F1);
    assert!(texts(&s).iter().any(|t| t.contains("Actions 1/3")));
    for _ in 0..14 {
        press(&mut s, KeyCode::ArrowDown);
    }
    assert!(texts(&s).iter().any(|t| t.contains("Actions 2/3")));
    assert!(!texts(&s).iter().any(|t| t == "Resume editing"));
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    assert_eq!(s.analysis_report().unwrap().summary().stats.total_arrows, 6);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::F12);
    assert!(!s.is_analyzing());
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Digit3);
    assert!(s.analysis_report().is_none());
    press(&mut s, KeyCode::F12);
    finish(&mut s);
    assert_eq!(s.analysis_report().unwrap().summary().stats.total_arrows, 7);
    press(&mut s, KeyCode::F12);
    assert!(s.is_analyzing());
    finish(&mut s);
    assert_eq!(
        s.workspace().file().session.undo_label(),
        Some("Place note")
    );
    press(&mut s, KeyCode::Escape);
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(s.analysis_report().is_none());
    press(&mut s, KeyCode::F12);
    finish(&mut s);
    assert_eq!(s.analysis_report().unwrap().summary().stats.total_arrows, 6);
    assert!(!s.workspace().file().session.is_dirty());
}
#[test]
fn cancelled_analysis_cannot_publish_and_unsupported_layout_reports_without_mutation() {
    let _guard = LOCK.lock().unwrap();
    let (dir, mut s) = setup();
    press(&mut s, KeyCode::F12);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::F12);
    finish(&mut s);
    assert!(s.analysis_report().is_none());
    assert!(!s.workspace().file().session.is_dirty());
    press(&mut s, KeyCode::F12);
    finish(&mut s);
    assert!(s.analysis_report().is_some());
    let path = dir.path().join("routine.ssc");
    let song = "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:10000000\n00000000\n00000000\n00000000\n&\n00001000\n00000000\n00000000\n00000000;";
    fs::write(&path, song).unwrap();
    let mut unsupported = State::open(&path, 0).unwrap();
    press(&mut unsupported, KeyCode::F12);
    finish(&mut unsupported);
    assert!(unsupported.analysis_report().is_none());
    assert!(
        texts(&unsupported)
            .iter()
            .any(|t| t.contains("does not support layout"))
    );
    assert_eq!(
        unsupported.workspace().file().session.serialize().unwrap(),
        song.as_bytes()
    );
}
