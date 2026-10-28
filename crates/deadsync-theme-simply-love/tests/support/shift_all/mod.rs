use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn init() {
    if super::FRENCH {
        i18n::init(deadsync_assets::language::load_for_tests("fr"));
    } else {
        i18n::init_for_tests();
    }
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
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
fn open(s: &mut State) {
    key(s, KeyCode::SuperLeft, true);
    key(s, KeyCode::ShiftLeft, true);
    press(s, KeyCode::KeyE);
    key(s, KeyCode::ShiftLeft, false);
    key(s, KeyCode::SuperLeft, false);
}
fn serialized(s: &State) -> Vec<u8> {
    s.workspace().file().session.serialize().unwrap()
}
const SOURCE:&[u8]=b"#VERSION:0.83;#BPMS:0=120,1=180;#TIMESIGNATURES:0=4=4;#LABELS:2=target;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0000\n0000;";
fn text(s: &State, key: &str) -> bool {
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let expected = i18n::tr("Edition", key);
    actors.iter().any(|a|matches!(a,deadlib_present::actors::Actor::Text{content,..}if content.as_ref()==expected.as_ref()))
}
fn menu(s: &mut State) {
    press(s, KeyCode::F1);
    for _ in 0..4 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
    press(s, KeyCode::F4);
}
#[test]
fn global_shift_is_prepared_then_applied_and_undoable_in_local_language() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SOURCE).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    menu(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(text(&s, "ShiftAllPrepared"));
    assert_eq!(serialized(&s), SOURCE);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(text(&s, "ShiftAllApplied"));
    assert_ne!(serialized(&s), SOURCE);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(serialized(&s), SOURCE);
}
#[test]
fn unprepared_and_cancelled_global_shift_leave_document_exact() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SOURCE).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    menu(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(text(&s, "NoteOperationPrepareFirst"));
    press(&mut s, KeyCode::Escape);
    assert_eq!(serialized(&s), SOURCE);
}
