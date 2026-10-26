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
#[test]
fn keyboard_paste_shortens_a_conflicting_hold_undoes_redoes_and_saves_the_result() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:2000\n0000\n3000\n0000\n,\n1000\n0000\n0000\n0000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    press(&mut s, KeyCode::End);
    assert_eq!(s.workspace().row(), 192);
    ctrl(&mut s, KeyCode::KeyC);
    press(&mut s, KeyCode::Home);
    press(&mut s, KeyCode::ArrowDown);
    assert_eq!(s.workspace().row(), 48);
    ctrl(&mut s, KeyCode::KeyV);
    let head = Position {
        row: 0,
        lane: 0,
        player: 0,
    };
    let pasted = Position { row: 48, ..head };
    assert_eq!(s.workspace().grid().get(head).unwrap().end, Some(36));
    assert_eq!(s.workspace().grid().get(pasted).unwrap().kind, Kind::Tap);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(s.workspace().grid().get(head).unwrap().end, Some(36));
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    assert_eq!(reopened.workspace().grid().get(head).unwrap().end, Some(36));
    assert!(reopened.workspace().grid().get(pasted).is_some());
}
