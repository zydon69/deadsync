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
fn keyboard_row_shift_drops_negative_heads_and_replaces_a_destination_note_with_a_hold() {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:0100\n0200\n0300\n0000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    let p = |row| Position {
        row,
        lane: 1,
        player: 0,
    };
    open_transform(&mut s, 2);
    press(&mut s, KeyCode::Enter);
    assert!(s.workspace().grid().get(p(0)).is_none());
    assert_eq!(s.workspace().grid().get(p(48)).unwrap().end, Some(96));
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    press(&mut s, KeyCode::ArrowDown);
    assert_eq!(s.workspace().row(), 48);
    open_transform(&mut s, 2);
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.workspace().grid().get(p(0)).unwrap().kind, Kind::Hold);
    assert_eq!(s.workspace().grid().get(p(0)).unwrap().end, Some(48));
    assert!(s.workspace().grid().get(p(48)).is_none());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::KeyY);
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    assert_eq!(reopened.workspace().grid().get(p(0)).unwrap().end, Some(48));
    assert!(reopened.workspace().grid().get(p(48)).is_none());
}
