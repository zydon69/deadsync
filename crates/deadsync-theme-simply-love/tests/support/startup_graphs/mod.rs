use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::options_preferences::General;
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
fn detached_candidates_do_not_start_preload_and_keyboard_set_changes_are_deferred_to_restart() {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    let root = tempfile::tempdir().unwrap();
    let mut owner = State::without_song(root.path()).unwrap();
    let candidate = State::without_song(root.path()).unwrap();
    assert!(!owner.startup_graph_preload_started());
    assert!(!candidate.startup_graph_preload_started());
    let mut empty = General::default();
    empty.startup_graphs.clear();
    owner.configure_general_options(empty).unwrap();
    let _ = owner.update_host_audio(Default::default());
    assert!(owner.startup_graph_preload_started());
    assert!(!owner.startup_graphs_pending_restart());
    // Keep the command-only host separate from the keyboard fixture: no fake
    // DSP acknowledgement is needed to edit the detached UI fixture.
    let mut editor = candidate;
    let bytes = editor.workspace().file().session.serialize().unwrap();
    let prefs = root.path().join("prefs.json");
    std::fs::write(&prefs,r#"{"Version":1,"Options":{"StartupStepGraphs":[]},"KeyBinds":{"Undo":[["F14"]],"Redo":[["F15"]]}}"#).unwrap();
    editor.configure_audio_preferences(&prefs).unwrap();
    press(&mut editor, KeyCode::F1);
    press(&mut editor, KeyCode::F9);
    for _ in 0..6 {
        press(&mut editor, KeyCode::ArrowRight);
    }
    press(&mut editor, KeyCode::Tab);
    press(&mut editor, KeyCode::Enter);
    assert!(editor.general_options().startup_graph_enabled(0));
    assert!(editor.startup_graphs_pending_restart());
    press(&mut editor, KeyCode::Escape);
    press(&mut editor, KeyCode::F14);
    assert!(!editor.general_options().startup_graph_enabled(0));
    assert!(!editor.startup_graphs_pending_restart());
    press(&mut editor, KeyCode::F15);
    assert!(editor.startup_graphs_pending_restart());
    let file = deadsync_editor::audio_preferences::File::open(&prefs).unwrap();
    assert!(file.general_options().startup_graph_enabled(0));
    assert!(!editor.has_open_song());
    assert!(!editor.workspace().file().session.is_dirty());
    assert_eq!(
        editor.workspace().file().session.serialize().unwrap(),
        bytes
    );
}
