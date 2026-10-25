use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    chart_text::{Field, Text},
    clipboard::{Error, TextClipboard},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
#[derive(Default)]
struct Board {
    value: String,
    reads: usize,
    writes: usize,
    fail_read: Option<Error>,
    fail_write: bool,
}
impl TextClipboard for Board {
    fn read_text(&mut self) -> Result<String, Error> {
        self.reads += 1;
        self.fail_read.map_or_else(|| Ok(self.value.clone()), Err)
    }
    fn write_text(&mut self, text: &str) -> Result<(), Error> {
        self.writes += 1;
        if self.fail_write {
            Err(Error::Unavailable)
        } else {
            self.value = text.to_owned();
            Ok(())
        }
    }
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
fn texts(s: &State) -> Vec<String> {
    let mut actors = Vec::new();
    s.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|a| {
            if let Actor::Text { content, .. } = a {
                Some(content.to_string())
            } else {
                None
            }
        })
        .collect()
}
fn has(s: &State, value: &str) -> bool {
    texts(s).iter().any(|v| v.contains(value))
}
const SONG: &str = "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#CHARTNAME:Été 日本語;#NOTES:1000;";
fn setup() -> (tempfile::TempDir, State) {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    press(&mut s, KeyCode::F7);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::Enter);
    (dir, s)
}
#[test]
fn copy_cut_and_paste_use_selected_unicode_and_keep_atomic_local_and_document_history() {
    let (dir, mut s) = setup();
    let mut board = Board::default();
    ctrl(&mut s, KeyCode::KeyC);
    assert!(s.process_text_clipboard(&mut board));
    assert_eq!(board.value, "Été 日本語");
    assert!(has(&s, "Selection copied"));
    assert!(has(&s, "Name: [Été 日本語]|"));
    ctrl(&mut s, KeyCode::KeyX);
    assert!(s.process_text_clipboard(&mut board));
    assert!(has(&s, "Name: |"));
    assert_eq!(board.value, "Été 日本語");
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(has(&s, "Name: [Été 日本語]|"));
    board.value = "Japon : 日本語;\\,\n🎵\t".to_owned();
    key(&mut s, KeyCode::SuperLeft, true);
    press(&mut s, KeyCode::KeyV);
    assert!(s.process_text_clipboard(&mut board));
    key(&mut s, KeyCode::SuperLeft, false);
    assert!(has(&s, "Name: Japon : 日本語;\\,🎵|"));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(has(&s, "Name: [Été 日本語]|"));
    ctrl(&mut s, KeyCode::KeyY);
    assert!(has(&s, "Name: Japon : 日本語;\\,🎵|"));
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        SONG.as_bytes()
    );
    press(&mut s, KeyCode::Escape);
    for _ in 0..6 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    assert_eq!(
        Text::read(s.workspace().file().session.document(), 0)
            .unwrap()
            .get(Field::Name),
        "Japon : 日本語;\\,🎵"
    );
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        SONG.as_bytes()
    );
    ctrl(&mut s, KeyCode::KeyY);
    ctrl(&mut s, KeyCode::KeyS);
    let reopen = State::open(&dir.path().join("song.ssc"), 0).unwrap();
    assert_eq!(
        Text::read(reopen.workspace().file().session.document(), 0)
            .unwrap()
            .get(Field::Name),
        "Japon : 日本語;\\,🎵"
    );
    assert_eq!((board.reads, board.writes), (1, 2));
}
#[test]
fn failures_and_empty_pastes_preserve_selection_value_and_redo() {
    let (_dir, mut s) = setup();
    let mut board = Board {
        fail_write: true,
        ..Board::default()
    };
    ctrl(&mut s, KeyCode::KeyX);
    assert!(s.process_text_clipboard(&mut board));
    assert!(has(&s, "System clipboard unavailable"));
    assert!(has(&s, "Name: [Été 日本語]|"));
    for failure in [Error::Unavailable, Error::NoText] {
        board.fail_read = Some(failure);
        ctrl(&mut s, KeyCode::KeyV);
        assert!(s.process_text_clipboard(&mut board));
        assert!(has(&s, "Name: [Été 日本語]|"));
    }
    assert!(has(&s, "clipboard contains no supported text"));
    board.fail_read = None;
    assert!(s.text_input("Edited"));
    ctrl(&mut s, KeyCode::KeyZ);
    for empty in ["", "\n\t\0"] {
        board.value = empty.into();
        ctrl(&mut s, KeyCode::KeyV);
        assert!(s.process_text_clipboard(&mut board));
        assert!(has(&s, "Name: [Été 日本語]|"));
    }
    ctrl(&mut s, KeyCode::KeyY);
    assert!(has(&s, "Name: Edited|"));
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        SONG.as_bytes()
    );
    let null_path = _dir.path().join("legacy.ssc");
    let legacy = SONG.replace("Été 日本語", "Legacy\0name");
    fs::write(&null_path, &legacy).unwrap();
    let mut null_state = State::open(&null_path, 0).unwrap();
    press(&mut null_state, KeyCode::F7);
    press(&mut null_state, KeyCode::ArrowUp);
    press(&mut null_state, KeyCode::ArrowUp);
    press(&mut null_state, KeyCode::Enter);
    let calls = (board.reads, board.writes);
    for code in [KeyCode::KeyC, KeyCode::KeyX] {
        ctrl(&mut null_state, code);
        assert!(!null_state.process_text_clipboard(&mut board));
        assert!(has(&null_state, "selection contains a null character"));
    }
    assert_eq!((board.reads, board.writes), calls);
    assert_eq!(
        null_state.workspace().file().session.serialize().unwrap(),
        legacy.as_bytes()
    );
}
#[test]
fn stale_requests_composition_empty_selection_and_note_shortcuts_never_touch_text_io() {
    let (_dir, mut s) = setup();
    let mut board = Board::default();
    ctrl(&mut s, KeyCode::KeyX);
    press(&mut s, KeyCode::Tab);
    assert!(!s.process_text_clipboard(&mut board));
    key(&mut s, KeyCode::ShiftLeft, true);
    press(&mut s, KeyCode::Tab);
    key(&mut s, KeyCode::ShiftLeft, false);
    ctrl(&mut s, KeyCode::KeyV);
    assert!(s.text_input("Changed"));
    assert!(!s.process_text_clipboard(&mut board));
    ctrl(&mut s, KeyCode::KeyV);
    s.text_preedit("未確定");
    assert!(!s.process_text_clipboard(&mut board));
    ctrl(&mut s, KeyCode::KeyC);
    assert!(!s.process_text_clipboard(&mut board));
    s.text_preedit("");
    press(&mut s, KeyCode::End);
    for code in [KeyCode::KeyC, KeyCode::KeyX] {
        ctrl(&mut s, code);
        assert!(!s.process_text_clipboard(&mut board));
    }
    key(&mut s, KeyCode::SuperLeft, true);
    s.raw_key(&RawKeyboardEvent {
        code: KeyCode::KeyV,
        pressed: true,
        repeat: true,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    key(&mut s, KeyCode::SuperLeft, false);
    assert!(!s.process_text_clipboard(&mut board));
    ctrl(&mut s, KeyCode::KeyV);
    press(&mut s, KeyCode::Escape);
    assert!(!s.process_text_clipboard(&mut board));
    press(&mut s, KeyCode::Escape);
    for code in [KeyCode::KeyC, KeyCode::KeyX, KeyCode::KeyV] {
        ctrl(&mut s, code);
        assert!(!s.process_text_clipboard(&mut board));
    }
    assert_eq!((board.reads, board.writes), (0, 0));
    assert_eq!(s.workspace().grid().iter().count(), 1);
}

#[test]
fn releasing_one_command_modifier_keeps_other_held_modifiers_active() {
    for (held, released) in [
        (KeyCode::SuperLeft, KeyCode::ControlLeft),
        (KeyCode::ControlRight, KeyCode::ControlLeft),
        (KeyCode::SuperRight, KeyCode::SuperLeft),
    ] {
        let (_dir, mut state) = setup();
        let mut board = Board {
            value: "Mixed modifiers".into(),
            ..Board::default()
        };
        key(&mut state, held, true);
        key(&mut state, released, true);
        key(&mut state, released, false);
        press(&mut state, KeyCode::KeyV);
        assert!(state.process_text_clipboard(&mut board));
        assert!(has(&state, "Mixed modifiers"));
        key(&mut state, held, false);
        press(&mut state, KeyCode::KeyV);
        assert!(!state.process_text_clipboard(&mut board));
        assert_eq!(board.reads, 1);
    }
}

#[test]
fn releasing_left_shift_does_not_cancel_selection_while_right_shift_is_held() {
    let (_dir, mut state) = setup();
    let mut board = Board::default();
    press(&mut state, KeyCode::Home);
    key(&mut state, KeyCode::ShiftLeft, true);
    key(&mut state, KeyCode::ShiftRight, true);
    key(&mut state, KeyCode::ShiftLeft, false);
    press(&mut state, KeyCode::ArrowRight);
    key(&mut state, KeyCode::ShiftRight, false);
    ctrl(&mut state, KeyCode::KeyC);
    assert!(state.process_text_clipboard(&mut board));
    assert_eq!(board.value, "É");
}

#[test]
fn focus_loss_releases_all_editor_modifiers_and_pending_io_without_discarding_text() {
    let (_dir, mut state) = setup();
    let original = state.workspace().file().session.serialize().unwrap();
    let mut board = Board {
        value: "Unexpected paste".into(),
        ..Board::default()
    };
    key(&mut state, KeyCode::SuperLeft, true);
    key(&mut state, KeyCode::ShiftRight, true);
    press(&mut state, KeyCode::KeyV);
    state.release_input();
    assert!(!state.process_text_clipboard(&mut board));
    assert_eq!(board.reads, 0);
    press(&mut state, KeyCode::KeyV);
    assert!(!state.process_text_clipboard(&mut board));
    assert!(state.text_input("Draft retained"));
    assert!(has(&state, "Draft retained"));
    assert!(!has(&state, "Unexpected paste"));
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original
    );
    state.text_preedit("composition");
    state.release_input();
    assert!(state.text_input("!"));
    assert!(has(&state, "Draft retained!"));
    press(&mut state, KeyCode::Home);
    press(&mut state, KeyCode::ArrowRight);
    ctrl(&mut state, KeyCode::KeyC);
    assert!(!state.process_text_clipboard(&mut board)); // no Shift selection survives
    assert_eq!(board.writes, 0);
}
