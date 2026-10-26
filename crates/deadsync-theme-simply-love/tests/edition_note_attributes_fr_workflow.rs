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
fn open_inspector(s: &mut State) {
    open_transform(s, 11);
    press(s, KeyCode::Enter);
}
fn text(s: &mut State, value: &str) {
    ctrl(s, KeyCode::KeyA);
    assert!(s.text_input(value));
}
#[test]
fn inspector_keyboard_cancel_validate_correct_undo_redo_and_save() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:2[1]000\n0000\n3[2]000\n0000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    let p = Position {
        row: 0,
        lane: 0,
        player: 0,
    };
    open_inspector(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    text(&mut s, "[7]");
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    open_inspector(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    text(&mut s, ";bad");
    press(&mut s, KeyCode::ArrowDown);
    text(&mut s, "[8]");
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    assert!(!actors.iter().any(|actor|matches!(actor,deadlib_present::actors::Actor::Text{content,..} if content.contains("{value}") || content.contains("{message}"))));
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowUp);
    text(&mut s, "[7]");
    // Field-local undo and redo do not touch chart history.
    ctrl(&mut s, KeyCode::KeyZ);
    ctrl(&mut s, KeyCode::KeyY);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    let note = s.workspace().grid().get(p).unwrap();
    assert_eq!(note.kind, Kind::Hold);
    assert_eq!(note.end, Some(96));
    assert_eq!(&*note.attributes, "[7]");
    assert_eq!(&*note.tail_attributes, "[8]");
    let after = s.workspace().file().session.serialize().unwrap();
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), after);
    ctrl(&mut s, KeyCode::KeyS);
    assert_eq!(fs::read(&path).unwrap(), after);
    let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    assert_eq!(
        &*reopened.workspace().grid().get(p).unwrap().tail_attributes,
        "[8]"
    );
}

#[derive(Default)]
struct Clipboard {
    value: String,
    reads: usize,
}
impl deadsync_editor::clipboard::TextClipboard for Clipboard {
    fn read_text(&mut self) -> Result<String, deadsync_editor::clipboard::Error> {
        self.reads += 1;
        Ok(self.value.clone())
    }
    fn write_text(&mut self, value: &str) -> Result<(), deadsync_editor::clipboard::Error> {
        self.value = value.into();
        Ok(())
    }
}
#[test]
fn columns_missing_notes_pending_changes_unicode_and_stale_clipboard() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1100;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    open_inspector(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    assert!(!s.wants_text_input());
    assert!(!s.text_input("[9]"));
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowLeft);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::PageDown);
    assert!(!s.wants_text_input());
    press(&mut s, KeyCode::PageUp);
    press(&mut s, KeyCode::ArrowDown);
    assert!(s.wants_text_input());
    text(&mut s, "[é]");
    // Pending attributes cannot be silently discarded by switching columns.
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowLeft);
    press(&mut s, KeyCode::ArrowDown);
    let mut clipboard = Clipboard {
        value: "[7]".into(),
        reads: 0,
    };
    ctrl(&mut s, KeyCode::KeyA);
    ctrl(&mut s, KeyCode::KeyV);
    assert!(s.process_text_clipboard(&mut clipboard));
    assert_eq!(clipboard.reads, 1);
    ctrl(&mut s, KeyCode::KeyV);
    press(&mut s, KeyCode::ArrowDown);
    assert!(!s.process_text_clipboard(&mut clipboard));
    assert_eq!(clipboard.reads, 1);
    // Taps have no tail input field.
    assert!(!s.wants_text_input());
    assert!(!s.text_input("[8]"));
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    let p = |lane| Position {
        row: 0,
        lane,
        player: 0,
    };
    assert_eq!(&*s.workspace().grid().get(p(0)).unwrap().attributes, "");
    assert_eq!(&*s.workspace().grid().get(p(1)).unwrap().attributes, "[7]");
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
