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
fn open_conversion(s: &mut State) {
    open_transform(s, 9);
    press(s, KeyCode::Enter);
}
#[test]
fn keyboard_note_conversion_cancels_applies_preserves_existing_lengths_and_saves() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000\n0200\n0000\n0300\n,\n0010;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    let p = |row, lane| Position {
        row,
        lane,
        player: 0,
    };
    open_conversion(&mut s);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    assert!(actors.iter().any(|actor|matches!(actor,deadlib_present::actors::Actor::Text {content,..} if content.contains("Type source :"))));
    assert!(!actors.iter().any(|actor|matches!(actor,deadlib_present::actors::Actor::Text {content,..} if content.contains("{value}"))));
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    open_conversion(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.workspace().grid().get(p(0, 0)).unwrap().kind, Kind::Hold);
    assert_eq!(s.workspace().grid().get(p(0, 0)).unwrap().end, Some(24));
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::KeyY);
    let before_second = s.workspace().file().session.serialize().unwrap();
    press(&mut s, KeyCode::ArrowDown);
    assert_eq!(s.workspace().row(), 48);
    open_conversion(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.workspace().grid().get(p(48, 1)).unwrap().kind, Kind::Roll);
    assert_eq!(s.workspace().grid().get(p(48, 1)).unwrap().end, Some(144));
    assert!(s.workspace().grid().get(p(192, 2)).is_some());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        before_second
    );
    ctrl(&mut s, KeyCode::KeyY);
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    assert_eq!(
        reopened.workspace().grid().get(p(0, 0)).unwrap().end,
        Some(24)
    );
    assert_eq!(
        reopened.workspace().grid().get(p(48, 1)).unwrap().kind,
        Kind::Roll
    );
    assert_eq!(
        reopened.workspace().grid().get(p(48, 1)).unwrap().end,
        Some(144)
    );
}

#[test]
fn failed_conversion_keeps_the_panel_editable_and_can_be_repaired_without_touching_source() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000\n1000\n0000\n0000\n0000\n0000\n0000\n0000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    open_conversion(&mut s);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert!(!s.workspace().file().session.is_dirty());
    assert!(s.workspace().file().session.undo_label().is_none());
    let mut actors = Vec::new();
    s.actors(&mut actors);
    assert!(actors.iter().any(|actor|matches!(actor,deadlib_present::actors::Actor::Text {content,..} if content.contains("Type source :"))));
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    let p = Position {
        row: 0,
        lane: 0,
        player: 0,
    };
    assert_eq!(s.workspace().grid().get(p).unwrap().end, Some(16));
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    let redo = s
        .workspace()
        .file()
        .session
        .redo_label()
        .unwrap()
        .to_owned();
    open_conversion(&mut s);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowRight);
    }
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert_eq!(
        s.workspace().file().session.redo_label(),
        Some(redo.as_str())
    );
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let message = i18n::tr("Edition", "NoteConversionNoChange");
    assert!(actors.iter().any(|actor| matches!(actor, deadlib_present::actors::Actor::Text { content, .. } if content.as_ref() == message.as_ref())));
    assert_eq!(fs::read(path).unwrap(), source);
}
