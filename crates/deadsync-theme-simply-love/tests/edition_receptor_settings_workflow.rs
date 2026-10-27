//! Keyboard controls, persistence and non-song undo for receptor effects.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, sync::Mutex, time::Instant};
static SERIAL: Mutex<()> = Mutex::new(());
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
fn control(s: &mut State, code: KeyCode, shift: bool) {
    key(s, KeyCode::SuperLeft, true);
    if shift {
        key(s, KeyCode::ShiftLeft, true);
    }
    press(s, code);
    key(s, KeyCode::ShiftLeft, false);
    key(s, KeyCode::SuperLeft, false);
}
fn open(locale: &str) -> (tempfile::TempDir, State, Vec<u8>, std::path::PathBuf) {
    i18n::init(deadsync_assets::language::load_for_tests(locale));
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#TITLE:Keep;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0000\n0000\n0000;".to_vec();
    fs::write(&song, &source).unwrap();
    let mut state = State::open(&song, 0).unwrap();
    let path = dir.path().join("prefs.json");
    fs::write(&path, b"{\"Version\":1,\"Receptors\":{\"Vendor\":42}}").unwrap();
    state.configure_audio_preferences(&path).unwrap();
    (dir, state, source, path)
}
fn panel(s: &mut State) {
    control(s, KeyCode::KeyH, true);
}
fn down(s: &mut State, n: usize) {
    for _ in 0..n {
        press(s, KeyCode::ArrowDown);
    }
}
#[test]
fn all_eight_controls_cancel_and_defaults_are_detached_in_both_languages() {
    let _guard = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let (_dir, mut s, source, path) = open(locale);
        let before = fs::read(&path).unwrap();
        let original = s.receptor_preferences();
        panel(&mut s);
        for _ in 0..8 {
            press(&mut s, KeyCode::ArrowRight);
            press(&mut s, KeyCode::ArrowDown);
        }
        assert_eq!(s.receptor_preferences(), original);
        assert_eq!(fs::read(&path).unwrap(), before);
        press(&mut s, KeyCode::Enter); // Defaults changes only draft.
        assert_eq!(s.receptor_preferences(), original);
        press(&mut s, KeyCode::Escape);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
        assert!(!s.workspace().file().session.is_dirty());
    }
}
#[test]
fn save_all_flags_is_one_global_undo_and_redo_without_song_dirtying() {
    let _guard = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let (_dir, mut s, source, path) = open(locale);
        panel(&mut s);
        for _ in 0..8 {
            press(&mut s, KeyCode::ArrowRight);
            press(&mut s, KeyCode::ArrowDown);
        }
        press(&mut s, KeyCode::ArrowDown);
        press(&mut s, KeyCode::Enter);
        let after = s.receptor_preferences();
        let mut fields = serde_json::Map::new();
        after.write(&mut fields);
        assert!(
            fields
                .values()
                .all(|v| v == &serde_json::Value::Bool(false))
        );
        assert!(!s.workspace().file().session.is_dirty());
        let revision = s.workspace().file().session.revision();
        panel(&mut s);
        down(&mut s, 9);
        press(&mut s, KeyCode::Enter);
        assert_eq!(
            s.workspace().file().session.revision(),
            revision,
            "no-op save adds no action"
        );
        control(&mut s, KeyCode::KeyZ, false);
        assert_eq!(s.receptor_preferences(), Default::default());
        control(&mut s, KeyCode::KeyY, false);
        assert_eq!(s.receptor_preferences(), after);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
        let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(json["Receptors"]["Vendor"], 42);
    }
}
#[test]
fn save_conflict_keeps_runtime_and_draft_until_cancel() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut s, source, path) = open("en");
    let original = s.receptor_preferences();
    panel(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    fs::write(
        &path,
        b"{\"Version\":1,\"Receptors\":{\"AutoPlayHideArrows\":false,\"Vendor\":99}}",
    )
    .unwrap();
    let external = fs::read(&path).unwrap();
    down(&mut s, 9);
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.receptor_preferences(), original);
    assert_eq!(fs::read(&path).unwrap(), external);
    assert_eq!(s.workspace().file().session.undo_label(), None);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
