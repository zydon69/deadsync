use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::notes::{Kind, Position};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn press(s: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        s.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
fn ctrl(s: &mut State, code: KeyCode) {
    s.raw_key(&RawKeyboardEvent {
        code: KeyCode::SuperLeft,
        pressed: true,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    press(s, code);
    s.raw_key(&RawKeyboardEvent {
        code: KeyCode::SuperLeft,
        pressed: false,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn open_transform(s: &mut State, index: usize) {
    press(s, KeyCode::F1);
    for _ in 0..4 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
    for _ in 0..index {
        press(s, KeyCode::ArrowDown);
    }
}
fn open_owners(s: &mut State) {
    open_transform(s, 10);
    press(s, KeyCode::Enter);
}
#[test]
fn keyboard_assigns_and_swaps_routine_players_with_cancel_undo_save_and_reopen() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#DIFFICULTY:Hard;#METER:9;#NOTES:20000000\n00000000\n30000000\n00000000\n&\n00001000\n00000000\n00000000\n00000000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-routine", "Hard", "", 9).unwrap();
    let p = |lane, player| Position {
        row: 0,
        lane,
        player,
    };
    open_owners(&mut s);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    assert!(actors.iter().any(|actor|matches!(actor,deadlib_present::actors::Actor::Text {content,..} if content.contains("Opération :"))));
    assert!(!actors.iter().any(|actor|matches!(actor,deadlib_present::actors::Actor::Text {content,..} if content.contains("{value}"))));
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    open_owners(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(s.workspace().grid().get(p(0, 0)).is_none());
    assert_eq!(s.workspace().grid().get(p(0, 1)).unwrap().end, Some(96));
    assert!(s.workspace().grid().get(p(4, 1)).is_some());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    open_owners(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.workspace().grid().get(p(0, 1)).unwrap().kind, Kind::Hold);
    assert_eq!(s.workspace().grid().get(p(0, 1)).unwrap().end, Some(96));
    assert!(s.workspace().grid().get(p(4, 0)).is_some());
    assert!(s.workspace().grid().get(p(4, 1)).is_none());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::KeyY);
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open_selected(&path, "dance-routine", "Hard", "", 9).unwrap();
    assert_eq!(
        reopened.workspace().grid().get(p(0, 1)).unwrap().end,
        Some(96)
    );
    assert!(reopened.workspace().grid().get(p(4, 0)).is_some());
}
