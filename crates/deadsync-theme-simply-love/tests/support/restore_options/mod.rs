use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
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
fn open(s: &mut State) {
    press(s, KeyCode::F1);
    // General options remains menu entry30; newer tools follow it.
    for _ in 0..30 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
}
#[test]
fn full_source_restore_is_localized_atomic_and_undoes_all_owners() {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let root = tempfile::tempdir().unwrap();
    let song = root.path().join("song.ssc");
    std::fs::write(&song,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let prefs = root.path().join("prefs.json");
    std::fs::write(&prefs,r#"{"Version":1,"Options":{"UndoHistorySize":32,"NewSongSyncOffset":0.1,"OpenSongSyncOffset":0.25,"RenderNotes":false,"RecentFilesHistorySize":7,"StepColorMethodValue":"999","SuppressExternalSongModificationNotification":true,"MiscEventAreaWidth":123,"HideSongBackground":true,"MaxEventsToDraw":11,"ResetLayout":"Expanded","Future":17},"KeyBinds":{"Undo":[["F14"]],"Redo":[["F15"]]}}"#).unwrap();
    let mut state = State::open_with_preferences(&song, 0, &prefs).unwrap();
    let before = state.options_snapshot();
    assert_eq!(before.recent_maximum, 7, "loaded preferences must reach the file menu cache");
    let bytes = state.workspace().file().session.serialize().unwrap();
    let clock = deadsync_editor::playback::Clock::capture(&state.workspace().file().session, 0)
        .unwrap()
        .music_time(48)
        .unwrap();
    open(&mut state);
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let label = i18n::tr("Edition", "GeneralAllDefaults");
    assert!(
        actors
            .iter()
            .any(|a| matches!(a,Actor::Text{content,..}if content.as_str()==&*label))
    );
    for _ in 0..3 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.options_snapshot(), before.restored());
    assert!(state.render_policy().notes);
    assert_eq!(state.misc_event_area_width(), 100.);
    assert!(!state.song_background_policy().hide);
    assert_eq!(state.workspace().file().session.history_capacity(), 1024);
    press(&mut state, KeyCode::Escape);
    press(&mut state, KeyCode::F14);
    assert_eq!(state.options_snapshot(), before);
    assert_eq!(state.workspace().file().session.history_capacity(), 32);
    press(&mut state, KeyCode::F15);
    assert_eq!(state.options_snapshot(), before.restored());
    assert_eq!(state.workspace().file().session.serialize().unwrap(), bytes);
    assert!(!state.workspace().file().session.is_dirty());
    assert_eq!(
        deadsync_editor::playback::Clock::capture(&state.workspace().file().session, 0)
            .unwrap()
            .music_time(48)
            .unwrap(),
        clock
    );
    let disk: serde_json::Value = serde_json::from_slice(&std::fs::read(&prefs).unwrap()).unwrap();
    assert_eq!(disk["Options"]["Future"], 17);
    assert_eq!(disk["Options"]["MaxEventsToDraw"], 11);
    assert_eq!(disk["Options"]["ResetLayout"], "Expanded");
}
