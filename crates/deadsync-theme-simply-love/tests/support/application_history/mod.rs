use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::time::Instant;
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
#[test]
fn no_song_display_preferences_undo_redo_via_real_bindings_without_creating_a_song() {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    let root = tempfile::tempdir().unwrap();
    let prefs = root.path().join("prefs.json");
    std::fs::write(
        &prefs,
        r#"{"Version":1,"KeyBinds":{"Undo":[["F14"]],"Redo":[["F15"]],"Save":[["F16"]]}}"#,
    )
    .unwrap();
    let mut s = State::without_song(root.path()).unwrap();
    s.configure_audio_preferences(&prefs).unwrap();
    let bytes = s.workspace().file().session.serialize().unwrap();
    assert!(!s.has_open_song());
    assert!(s.workspace().file().existing_path().is_none());
    press(&mut s, KeyCode::F1);
    press(&mut s, KeyCode::F10);
    press(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::Enter);
    assert!(!s.render_policy().notes);
    assert!(s.workspace().file().session.can_undo_application_change());
    press(&mut s, KeyCode::F14);
    assert!(s.render_policy().notes);
    assert!(s.workspace().file().session.can_redo_application_change());
    press(&mut s, KeyCode::F15);
    assert!(!s.render_policy().notes);
    press(&mut s, KeyCode::F16);
    press(&mut s, KeyCode::Digit1);
    assert!(!s.has_open_song());
    assert!(s.workspace().file().existing_path().is_none());
    assert_eq!(s.workspace().file().session.serialize().unwrap(), bytes);
    assert!(!s.workspace().file().session.is_dirty());
    assert!(!root.path().join("untitled.ssc").exists());
}
