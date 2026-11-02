use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::notes::{Kind, Position};
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
fn conversion(s: &mut State) {
    press(s, KeyCode::F1);
    for _ in 0..4 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
    for _ in 0..9 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
}
fn prepare(s: &mut State) {
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::Enter);
}
fn apply(s: &mut State) {
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::Enter);
}
fn pos(row: u64, lane: u16, player: u32) -> Position {
    Position { row, lane, player }
}
fn text(s: &State, key: &str) -> bool {
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let expected = i18n::tr("Edition", key);
    actors.iter().any(|a|matches!(a,deadlib_present::actors::Actor::Text{content,..}if content.as_ref()==expected.as_ref()))
}
fn serialized(s: &State) -> Vec<u8> {
    s.workspace().file().session.serialize().unwrap()
}
const SOURCE:&[u8]=b"#VERSION:0.83;#TITLE:All;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0M00\n0010\n0000;";
#[test]
fn convert_all_is_reviewed_without_edits_then_applies_cancels_undoes_and_reloads() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SOURCE).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    conversion(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowRight); // Tap→Mine
    press(&mut s, KeyCode::F8);
    prepare(&mut s);
    assert_eq!(serialized(&s), SOURCE);
    assert!(text(&s, "NoteOperationPrepared"));
    press(&mut s, KeyCode::Escape);
    assert_eq!(serialized(&s), SOURCE);
    press(&mut s, KeyCode::F8);
    prepare(&mut s);
    apply(&mut s);
    assert_eq!(
        s.workspace().grid().get(pos(0, 0, 0)).unwrap().kind,
        Kind::Mine
    );
    assert_eq!(
        s.workspace().grid().get(pos(96, 2, 0)).unwrap().kind,
        Kind::Mine
    );
    let after = serialized(&s);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(serialized(&s), SOURCE);
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(serialized(&s), after);
    let output = s.workspace().file().session.serialize_for_output().unwrap();
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(serialized(&reopened), output);
}
#[test]
fn swap_all_is_simultaneous_over_notes_beyond_current_row() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SOURCE).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    conversion(&mut s);
    press(&mut s, KeyCode::F8);
    press(&mut s, KeyCode::ArrowRight);
    prepare(&mut s);
    assert_eq!(serialized(&s), SOURCE);
    apply(&mut s);
    assert_eq!(
        s.workspace().grid().get(pos(0, 0, 0)).unwrap().kind,
        Kind::Mine
    );
    assert_eq!(
        s.workspace().grid().get(pos(48, 1, 0)).unwrap().kind,
        Kind::Tap
    );
    assert_eq!(
        s.workspace().grid().get(pos(96, 2, 0)).unwrap().kind,
        Kind::Mine
    );
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(serialized(&s), SOURCE);
}
#[test]
fn exact_checked_routine_owner_survives_review_without_selecting_same_row_peer() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:10000000\n00000000\n&\n00000001\n00000000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    key(&mut s, KeyCode::SuperLeft, true);
    key(&mut s, KeyCode::ShiftLeft, true);
    press(&mut s, KeyCode::KeyE);
    key(&mut s, KeyCode::ShiftLeft, false);
    key(&mut s, KeyCode::SuperLeft, false);
    press(&mut s, KeyCode::F3);
    press(&mut s, KeyCode::ArrowRight); // Taps
    press(&mut s, KeyCode::ArrowDown);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowRight);
    } // P2
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight); // Replace
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::F8);
    press(&mut s, KeyCode::ArrowRight); // Swap; default scope checked
    prepare(&mut s);
    assert_eq!(serialized(&s), source);
    apply(&mut s);
    assert_eq!(
        s.workspace().grid().get(pos(0, 0, 0)).unwrap().kind,
        Kind::Tap
    );
    assert_eq!(
        s.workspace().grid().get(pos(0, 7, 1)).unwrap().kind,
        Kind::Mine
    );
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(serialized(&s), source);
}
#[test]
fn applying_without_review_or_after_scope_change_never_edits() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SOURCE).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    conversion(&mut s);
    press(&mut s, KeyCode::F8);
    press(&mut s, KeyCode::ArrowRight); // swap
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    assert!(text(&s, "NoteOperationPrepareFirst"));
    assert_eq!(serialized(&s), SOURCE);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::Enter); // prepared
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowRight); // All→Range skipping unavailable checked
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(text(&s, "NoteOperationPrepareFirst"));
    assert_eq!(serialized(&s), SOURCE);
    assert!(s.workspace().file().session.undo_label().is_none());
}
