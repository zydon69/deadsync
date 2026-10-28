use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::space::{Metrics, set_current_metrics};
use deadsync_editor::{
    audio_preferences::File,
    clipboard::{Error as ClipboardError, TextClipboard},
    key_binding_preferences::{Preferences, source_profile::Platform},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    sync::{Arc, Mutex},
    time::Instant,
};
static SERIAL: Mutex<()> = Mutex::new(());
fn raw(s: &mut State, code: KeyCode, pressed: bool) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(s: &mut State, code: KeyCode) {
    raw(s, code, true);
    raw(s, code, false);
}
fn settings(s: &mut State) {
    raw(s, KeyCode::ControlLeft, true);
    raw(s, KeyCode::ShiftLeft, true);
    press(s, KeyCode::KeyK);
    raw(s, KeyCode::ShiftLeft, false);
    raw(s, KeyCode::ControlLeft, false);
}
fn save(s: &mut State) {
    for _ in 0..6 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
}
fn undo_source(s: &mut State) {
    let modifier = if Platform::current() == Platform::Mac {
        KeyCode::SuperLeft
    } else {
        KeyCode::ControlLeft
    };
    raw(s, modifier, true);
    press(s, KeyCode::KeyZ);
    raw(s, modifier, false);
}
fn open(
    locale: &str,
) -> (
    tempfile::TempDir,
    State,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    i18n::init(deadsync_assets::language::load_for_tests(locale));
    set_current_metrics(Metrics::centered(854., 480.));
    let d = tempfile::tempdir().unwrap();
    let song = d.path().join("song.ssc");
    let path = d.path().join("prefs.json");
    fs::write(
        &song,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
    )
    .unwrap();
    let mut prefs = Preferences::default();
    prefs
        .commands
        .insert("Save".into(), vec![vec!["F13".into()]]);
    let mut file = File::open(&path).unwrap();
    file.set_key_bindings(Arc::new(prefs)).unwrap();
    file.save().unwrap();
    let mut s = State::open(&song, 0).unwrap();
    s.configure_audio_preferences(&path).unwrap();
    (d, s, song, path)
}
#[derive(Default)]
struct Clipboard {
    value: String,
    reads: usize,
    writes: usize,
}
impl TextClipboard for Clipboard {
    fn read_text(&mut self) -> Result<String, ClipboardError> {
        self.reads += 1;
        Ok(self.value.clone())
    }
    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.writes += 1;
        self.value = text.into();
        Ok(())
    }
}
#[test]
fn source_preset_is_staged_cancelable_and_one_global_undo_restores_custom_native_profile() {
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let (_d, mut s, song, path) = open(locale);
        let original = fs::read(&song).unwrap();
        let prefs = fs::read(&path).unwrap();
        settings(&mut s);
        press(&mut s, KeyCode::F6);
        assert_eq!(fs::read(&path).unwrap(), prefs);
        press(&mut s, KeyCode::Escape);
        assert_eq!(fs::read(&path).unwrap(), prefs);
        settings(&mut s);
        press(&mut s, KeyCode::F6);
        save(&mut s);
        let applied = File::open(&path).unwrap().key_bindings();
        assert_eq!(applied.commands["PlayPause"], vec![vec!["Space"]]);
        assert_eq!(applied.commands["NextNoteTool"], vec![Vec::<String>::new()]);
        assert!(applied.source_import_raw.is_some());
        undo_source(&mut s);
        let restored = File::open(&path).unwrap().key_bindings();
        assert_eq!(restored.commands["Save"], vec![vec!["F13"]]);
        assert!(restored.source_import_raw.is_none());
        assert_eq!(fs::read(&song).unwrap(), original);
        assert!(!s.workspace().file().session.is_dirty());
    }
}
#[test]
fn source_clipboard_import_preserves_raw_unknown_fields_until_review_save_and_undo() {
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let (_d, mut s, song, path) = open(locale);
        let original = fs::read(&song).unwrap();
        let prefs = fs::read(&path).unwrap();
        let text = "{/* exact source comment */\"PreferencesKeyBinds\":{\"Save\":[[\"LeftControl\",\"F20\"]],\"Vendor\":{\"future\":[7,],},},}";
        let mut clipboard = Clipboard {
            value: text.into(),
            ..Default::default()
        };
        settings(&mut s);
        press(&mut s, KeyCode::F7);
        assert!(s.process_text_clipboard(&mut clipboard));
        assert_eq!(clipboard.reads, 1);
        assert_eq!(clipboard.writes, 0);
        assert_eq!(fs::read(&path).unwrap(), prefs);
        save(&mut s);
        let applied = File::open(&path).unwrap().key_bindings();
        assert_eq!(applied.source_import_raw.as_deref(), Some(text));
        assert_eq!(applied.commands["Save"], vec![vec!["LeftControl", "F20"]]);
        undo_source(&mut s);
        assert_eq!(
            File::open(&path).unwrap().key_bindings().commands["Save"],
            vec![vec!["F13"]]
        );
        assert_eq!(fs::read(&song).unwrap(), original);
    }
}
#[test]
fn stale_source_clipboard_request_never_reads_after_profile_replacement_or_panel_close() {
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let (_d, mut s, _, path) = open(locale);
        let prefs = fs::read(&path).unwrap();
        let mut clipboard = Clipboard {
            value: "{\"Save\":[[\"S\"]]}".into(),
            ..Default::default()
        };
        settings(&mut s);
        press(&mut s, KeyCode::F7);
        press(&mut s, KeyCode::F5);
        assert!(!s.process_text_clipboard(&mut clipboard));
        assert_eq!(clipboard.reads, 0);
        press(&mut s, KeyCode::F7);
        press(&mut s, KeyCode::Escape);
        assert!(!s.process_text_clipboard(&mut clipboard));
        assert_eq!(clipboard.reads, 0);
        assert_eq!(fs::read(&path).unwrap(), prefs);
    }
}
