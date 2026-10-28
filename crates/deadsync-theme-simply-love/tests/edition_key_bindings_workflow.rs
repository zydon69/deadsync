//! Semantic registry, persistence, modal routing and keyboard-only capture.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::space::{Metrics, set_current_metrics};
use deadsync_editor::{audio_preferences::File, key_binding_preferences::Preferences};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
static SERIAL: Mutex<()> = Mutex::new(());
fn event(s: &mut State, code: KeyCode, pressed: bool, repeat: bool, time: Instant) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat,
        timestamp: time,
        host_nanos: 0,
    });
}
fn key(s: &mut State, code: KeyCode, pressed: bool) {
    event(s, code, pressed, false, Instant::now());
}
fn press(s: &mut State, code: KeyCode) {
    key(s, code, true);
    key(s, code, false);
}
fn ctrl(s: &mut State, code: KeyCode, shift: bool) {
    key(s, KeyCode::SuperLeft, true);
    if shift {
        key(s, KeyCode::ShiftLeft, true);
    }
    press(s, code);
    if shift {
        key(s, KeyCode::ShiftLeft, false);
    }
    key(s, KeyCode::SuperLeft, false);
}
fn open(
    locale: &str,
    prefs: Preferences,
) -> (
    tempfile::TempDir,
    State,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    i18n::init(deadsync_assets::language::load_for_tests(locale));
    set_current_metrics(Metrics::centered(854., 480.));
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ssc");
    fs::write(
        &song,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
    )
    .unwrap();
    let path = dir.path().join("prefs.json");
    let mut file = File::open(&path).unwrap();
    file.set_key_bindings(Arc::new(prefs)).unwrap();
    file.save().unwrap();
    let mut s = State::open(&song, 0).unwrap();
    s.configure_audio_preferences(&path).unwrap();
    (dir, s, song, path)
}
#[test]
fn remapped_save_executes_existing_operation_and_old_shortcut_cannot_double_dispatch() {
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let mut prefs = Preferences::default();
        prefs
            .commands
            .insert("Save".into(), vec![vec!["F13".into()]]);
        let (_d, mut s, song, _) = open(locale, prefs);
        let source = fs::read(&song).unwrap();
        press(&mut s, KeyCode::Digit1);
        assert!(s.workspace().file().session.is_dirty());
        ctrl(&mut s, KeyCode::KeyS, false);
        assert_eq!(fs::read(&song).unwrap(), source);
        assert!(s.workspace().file().session.is_dirty());
        press(&mut s, KeyCode::F13);
        assert!(!s.workspace().file().session.is_dirty());
        assert_ne!(fs::read(&song).unwrap(), source);
    }
}
#[test]
fn capture_enter_binding_is_separate_from_confirmation_and_saved_as_one_global_action() {
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let (_d, mut s, song, path) = open(locale, Preferences::default());
        let source = fs::read(&song).unwrap();
        ctrl(&mut s, KeyCode::KeyK, true);
        for _ in 0..2 {
            press(&mut s, KeyCode::ArrowDown);
        }
        press(&mut s, KeyCode::Enter);
        press(&mut s, KeyCode::Enter);
        press(&mut s, KeyCode::Enter);
        for _ in 0..4 {
            press(&mut s, KeyCode::ArrowDown);
        }
        press(&mut s, KeyCode::Enter);
        assert_eq!(
            File::open(&path).unwrap().key_bindings().commands["Save"],
            vec![vec!["Enter"]]
        );
        assert_eq!(fs::read(&song).unwrap(), source);
        assert!(!s.workspace().file().session.is_dirty());
        assert!(
            s.workspace()
                .file()
                .session
                .undo_changes_key_binding_preferences()
        );
        ctrl(&mut s, KeyCode::KeyZ, false);
        assert_eq!(
            File::open(&path).unwrap().key_bindings(),
            Arc::new(Preferences::default())
        );
        ctrl(&mut s, KeyCode::KeyY, false);
        assert_eq!(
            File::open(&path).unwrap().key_bindings().commands["Save"],
            vec![vec!["Enter"]]
        );
    }
}
#[test]
fn source_repeat_comes_from_frame_time_and_os_repeat_does_not_add_an_action() {
    let _g = SERIAL.lock().unwrap();
    let mut prefs = Preferences::default();
    prefs
        .commands
        .insert("MoveToNextMeasure".into(), vec![vec!["F14".into()]]);
    let (_d, mut s, _, _) = open("en", prefs);
    let time = Instant::now();
    event(&mut s, KeyCode::F14, true, false, time);
    let first = s.workspace().row();
    assert!(first > 0);
    event(
        &mut s,
        KeyCode::F14,
        true,
        true,
        time + Duration::from_millis(100),
    );
    assert_eq!(s.workspace().row(), first);
    s.update_with_time(time + Duration::from_millis(501));
    assert_eq!(s.workspace().row(), first * 2);
    event(
        &mut s,
        KeyCode::F14,
        false,
        false,
        time + Duration::from_millis(510),
    );
    s.update_with_time(time + Duration::from_secs(2));
    assert_eq!(s.workspace().row(), first * 2);
}
#[test]
fn modal_and_focus_cleanup_quarantine_held_shortcuts_until_release() {
    let _g = SERIAL.lock().unwrap();
    let mut prefs = Preferences::default();
    prefs
        .commands
        .insert("MoveToNextMeasure".into(), vec![vec!["F14".into()]]);
    let (_d, mut s, _, _) = open("en", prefs);
    let time = Instant::now();
    event(&mut s, KeyCode::F14, true, false, time);
    let first = s.workspace().row();
    press(&mut s, KeyCode::F1);
    s.update_with_time(time + Duration::from_secs(1));
    assert_eq!(s.workspace().row(), first);
    press(&mut s, KeyCode::Escape);
    s.update_with_time(time + Duration::from_secs(2));
    assert_eq!(s.workspace().row(), first);
    key(&mut s, KeyCode::F14, false);
    press(&mut s, KeyCode::F14);
    assert_eq!(s.workspace().row(), first * 2);
    s.release_input();
    event(
        &mut s,
        KeyCode::F14,
        true,
        true,
        time + Duration::from_secs(3),
    );
    s.update_with_time(time + Duration::from_secs(4));
    assert_eq!(s.workspace().row(), first * 2);
}

#[test]
fn paged_help_uses_configured_binding_in_both_languages() {
    use deadlib_present::actors::Actor;
    fn texts(actor: &Actor, out: &mut Vec<String>) {
        match actor {
            Actor::Text { content, .. } => out.push(content.as_str().to_owned()),
            Actor::Frame { children, .. } => children.iter().for_each(|child| texts(child, out)),
            _ => {}
        }
    }
    let _g = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let mut preferences = Preferences::default();
        preferences
            .commands
            .insert("Save".into(), vec![vec!["F13".into()]]);
        let (_d, mut state, _, _) = open(locale, preferences);
        press(&mut state, KeyCode::F1);
        for _ in 0..12 {
            press(&mut state, KeyCode::ArrowDown);
        }
        press(&mut state, KeyCode::Enter);
        let mut all = Vec::new();
        for _ in 0..20 {
            let mut actors = Vec::new();
            state.actors(&mut actors);
            for actor in &actors {
                texts(actor, &mut all);
            }
            press(&mut state, KeyCode::PageDown);
        }
        let action = if locale == "en" {
            "Save"
        } else {
            "Enregistrer"
        };
        assert!(
            all.iter()
                .any(|text| text.contains("F13") && text.contains(action)),
            "{all:?}"
        );
        assert!(
            !all.iter()
                .any(|text| text.contains("Ctrl/Cmd+S:") || text.contains("Ctrl/Cmd+S :"))
        );
    }
}
