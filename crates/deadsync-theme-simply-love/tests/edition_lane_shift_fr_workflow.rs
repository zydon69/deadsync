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
#[test]
fn keyboard_column_shift_drops_or_wraps_edges_and_supports_undo_save_and_reopen() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1002\n0000\n0003\n0000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    open_transform(&mut s, 4);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    assert!(actors.iter().any(|actor| matches!(actor,deadlib_present::actors::Actor::Text {content,..} if content.contains("Décaler à gauche"))));
    press(&mut s, KeyCode::Enter);
    let pos = |lane| Position {
        row: 0,
        lane,
        player: 0,
    };
    assert!(s.workspace().grid().get(pos(0)).is_none());
    assert_eq!(s.workspace().grid().get(pos(2)).unwrap().end, Some(96));
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(s.workspace().grid().get(pos(2)).unwrap().end, Some(96));
    ctrl(&mut s, KeyCode::KeyZ);
    open_transform(&mut s, 7);
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.workspace().grid().get(pos(0)).unwrap().end, Some(96));
    assert_eq!(s.workspace().grid().get(pos(1)).unwrap().kind, Kind::Tap);
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    assert_eq!(
        reopened.workspace().grid().get(pos(0)).unwrap().end,
        Some(96)
    );
    assert!(reopened.workspace().grid().get(pos(1)).is_some());
}
