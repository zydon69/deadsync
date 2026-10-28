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
fn open(s: &mut State) {
    key(s, KeyCode::SuperLeft, true);
    key(s, KeyCode::ShiftLeft, true);
    press(s, KeyCode::KeyE);
    key(s, KeyCode::ShiftLeft, false);
    key(s, KeyCode::SuperLeft, false);
    press(s, KeyCode::F3);
}
fn texts(s: &State) -> Vec<String> {
    let mut actors = Vec::new();
    s.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|a| {
            if let deadlib_present::actors::Actor::Text { content, .. } = a {
                Some(content.to_string())
            } else {
                None
            }
        })
        .collect()
}
fn pos(row: u64, lane: u16, player: u32) -> Position {
    Position { row, lane, player }
}
#[test]
fn preset_matches_whole_chart_and_cut_only_changes_checked_kind_with_undo() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0M00\n00F0\n000L;";
    fs::write(&path, source).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowRight); // Mines
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight); // Replace
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(!s.workspace().file().session.is_dirty());
    assert!(texts(&s).iter().all(|text| !text.contains("{value}")));
    ctrl(&mut s, KeyCode::KeyX);
    assert!(s.workspace().grid().get(pos(48, 1, 0)).is_none());
    assert!(s.workspace().grid().get(pos(0, 0, 0)).is_some());
    assert!(s.workspace().grid().get(pos(96, 2, 0)).is_some());
    assert_eq!(fs::read(&path).unwrap(), source);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
#[test]
fn source_add_mode_keeps_prior_choice_and_cancel_never_changes_source() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0M00\n00F0\n000L;";
    fs::write(&path, source).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    press(&mut s, KeyCode::F3);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowRight);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter); // source Add, prior Tap stays checked
    press(&mut s, KeyCode::Delete);
    assert!(s.workspace().grid().get(pos(0, 0, 0)).is_none());
    assert!(s.workspace().grid().get(pos(48, 1, 0)).is_none());
    assert!(s.workspace().grid().get(pos(96, 2, 0)).is_some());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
#[test]
fn routine_player_filter_selects_exact_owner_without_touching_other_player() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("routine.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:10000000\n0M000000\n&\n00000001\n000000M0;";
    fs::write(&path, source).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    assert_eq!(s.workspace().grid().players(), 2);
    open(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowRight); // mines
    press(&mut s, KeyCode::ArrowDown);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowRight);
    } // explicit P2
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight); // Replace
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::Delete);
    assert!(s.workspace().grid().get(pos(96, 6, 1)).is_none());
    assert!(s.workspace().grid().get(pos(96, 1, 0)).is_some());
    assert!(s.workspace().grid().get(pos(0, 7, 1)).is_some());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
