use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::notes::Position;
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
fn picker(s: &mut State) {
    key(s, KeyCode::SuperLeft, true);
    key(s, KeyCode::ShiftLeft, true);
    press(s, KeyCode::KeyE);
    key(s, KeyCode::ShiftLeft, false);
    key(s, KeyCode::SuperLeft, false);
}
fn transforms(s: &mut State) {
    press(s, KeyCode::F1);
    for _ in 0..4 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
    press(s, KeyCode::F8);
}
fn down(s: &mut State, n: usize) {
    for _ in 0..n {
        press(s, KeyCode::ArrowDown);
    }
}
fn pos(row: u64, lane: u16, player: u32) -> Position {
    Position { row, lane, player }
}
fn serialized(s: &State) -> Vec<u8> {
    s.workspace().file().session.serialize().unwrap()
}
fn text(s: &State, key: &str) -> bool {
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let expected = i18n::tr("Edition", key);
    actors.iter().any(|a|matches!(a,deadlib_present::actors::Actor::Text{content,..}if content.as_ref()==expected.as_ref()))
}
#[test]
fn exact_mirror_requires_acknowledging_unchecked_hold_trim_then_saves_and_undoes() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0002\n1100\n0003\n0000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    press(&mut s, KeyCode::ArrowDown);
    assert_eq!(s.workspace().row(), 48);
    picker(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Space); // uncheck same-row lane1 peer
    press(&mut s, KeyCode::F9);
    down(&mut s, 4);
    press(&mut s, KeyCode::Enter);
    assert_eq!(serialized(&s), source);
    down(&mut s, 2);
    press(&mut s, KeyCode::Enter);
    assert!(text(&s, "SelectedTransformMissingReview"));
    assert_eq!(serialized(&s), source);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::Enter); // individual hold-trim acknowledgement
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert_eq!(
        s.workspace().grid().get(pos(0, 3, 0)).unwrap().end,
        Some(36)
    );
    assert!(s.workspace().grid().get(pos(48, 1, 0)).is_some());
    assert!(s.workspace().grid().get(pos(48, 0, 0)).is_none());
    assert!(s.workspace().grid().get(pos(48, 3, 0)).is_some());
    let output = s.workspace().file().session.serialize_for_output().unwrap();
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(serialized(&reopened), output);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(serialized(&s), source);
}
#[test]
fn exact_owner_collision_is_localized_and_never_replaces_a_peer_note() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("routine.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:10000000\n&\nM0000000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    picker(&mut s);
    press(&mut s, KeyCode::F3);
    press(&mut s, KeyCode::ArrowRight); // taps
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight); // current P1
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight); // Replace
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::F9);
    press(&mut s, KeyCode::ArrowLeft);
    press(&mut s, KeyCode::ArrowLeft); // Assign
    down(&mut s, 2);
    press(&mut s, KeyCode::ArrowRight); // target P2
    down(&mut s, 2);
    press(&mut s, KeyCode::Enter);
    assert!(text(&s, "SelectedTransformOwnerCollision"));
    assert_eq!(serialized(&s), source);
    assert!(!s.workspace().file().session.is_dirty());
    assert!(s.workspace().file().session.undo_label().is_none());
    assert!(s.workspace().grid().get(pos(0, 0, 0)).is_some());
    assert!(s.workspace().grid().get(pos(0, 0, 1)).is_some());
}
#[test]
fn selected_boundary_deletion_cancels_without_edit_then_requires_review() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source = b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0001;";
    fs::write(&path, source).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    transforms(&mut s);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowRight);
    }
    down(&mut s, 4);
    press(&mut s, KeyCode::Enter);
    assert_eq!(serialized(&s), source);
    press(&mut s, KeyCode::Escape);
    assert_eq!(serialized(&s), source);
    press(&mut s, KeyCode::F8);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowRight);
    }
    down(&mut s, 4);
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(s.workspace().grid().get(pos(0, 0, 0)).is_none());
    assert!(s.workspace().grid().get(pos(192, 3, 0)).is_some());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(serialized(&s), source);
}
#[test]
fn snap_off_review_shifts_one_beat_and_keeps_measure_navigation_and_note_entry() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source = b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0100;";
    fs::write(&path, source).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    press(&mut s, KeyCode::ArrowLeft);
    assert_eq!(s.workspace().snap(), 9);
    transforms(&mut s);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowLeft);
    } // ShiftLater
    down(&mut s, 4);
    press(&mut s, KeyCode::Enter);
    down(&mut s, 2);
    press(&mut s, KeyCode::Enter);
    assert!(s.workspace().grid().get(pos(48, 0, 0)).is_some());
    assert!(s.workspace().grid().get(pos(0, 0, 0)).is_none());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(serialized(&s), source);
    press(&mut s, KeyCode::ArrowDown);
    assert_eq!(s.workspace().row(), 192);
    press(&mut s, KeyCode::ArrowUp);
    assert_eq!(s.workspace().row(), 0);
    press(&mut s, KeyCode::Digit3);
    assert!(s.workspace().grid().get(pos(0, 2, 0)).is_some());
    assert_eq!(s.workspace().snap(), 9);
}
