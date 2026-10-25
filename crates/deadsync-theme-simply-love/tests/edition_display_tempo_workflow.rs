use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
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
fn texts(s: &State) -> Vec<String> {
    let mut a = Vec::new();
    s.actors(&mut a);
    a.into_iter()
        .filter_map(|a| {
            if let Actor::Text { content, .. } = a {
                Some(content.to_string())
            } else {
                None
            }
        })
        .collect()
}
fn has(s: &State, value: &str) -> bool {
    texts(s).iter().any(|text| text.contains(value))
}
fn init() {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn open(s: &mut State) {
    press(s, KeyCode::F7);
    press(s, KeyCode::ArrowUp);
    press(s, KeyCode::Enter);
}
#[test]
fn keyboard_stages_range_with_other_properties_and_supports_undo_cancel_and_save() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source = "#VERSION:0.83;#BPMS:0=120,4=180;#DISPLAYBPM:*;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    press(&mut s, KeyCode::F7);
    for _ in 0..8 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Digit1);
    press(&mut s, KeyCode::Digit4);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "Display mode: Random"));
    press(&mut s, KeyCode::ArrowRight);
    assert!(has(&s, "Minimum BPM: 120.000000"));
    assert!(has(&s, "Maximum BPM: 180.000000"));
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Digit9);
    press(&mut s, KeyCode::Digit5);
    press(&mut s, KeyCode::Period);
    press(&mut s, KeyCode::Digit5);
    press(&mut s, KeyCode::ArrowDown);
    assert!(has(&s, "Minimum BPM: 95.500000"));
    for _ in 0..2 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
    press(&mut s, KeyCode::Enter);
    assert_eq!(
        s.workspace()
            .file()
            .session
            .document()
            .text(Some(0), "DISPLAYBPM")
            .as_deref(),
        Some("95.500000:180.000000")
    );
    assert_eq!(
        s.workspace()
            .file()
            .session
            .document()
            .text(Some(0), "METER")
            .as_deref(),
        Some("14")
    );
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
    open(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
    open(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    for _ in 0..4 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::Enter);
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(
        reopened
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "DISPLAYBPM")
            .as_deref(),
        Some("120.000000:180.000000")
    );
}
