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
fn keyboard_general_controls_resize_history_and_persist_future_new_open_preferences_with_undo() {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let root = tempfile::tempdir().unwrap();
    let song = root.path().join("song.ssc");
    std::fs::write(&song,"#VERSION:0.83;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0.25};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let prefs = root.path().join("prefs.json");
    std::fs::write(&prefs,r#"{"Version":1,"Options":{"UndoHistorySize":32,"Future":"keep","StartupStepGraphs":["dance_single"]},"KeyBinds":{"Undo":[["F14"]],"Redo":[["F15"]]}}"#).unwrap();
    let mut s = State::open_with_preferences(&song, 0, &prefs).unwrap();
    assert_eq!(s.workspace().file().session.history_capacity(), 32);
    let source = s.workspace().file().session.serialize().unwrap();
    let clock = deadsync_editor::playback::Clock::capture(&s.workspace().file().session, 0)
        .unwrap()
        .music_time(48)
        .unwrap();
    open(&mut s);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let label = i18n::tr("Edition", "GeneralVisibleDefaults");
    assert!(
        actors
            .iter()
            .any(|a| matches!(a,Actor::Text{content,..}if content.as_str()==&*label))
    );
    press(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::ArrowRight);
    assert_eq!(s.workspace().file().session.history_capacity(), 64);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::F14);
    assert_eq!(s.workspace().file().session.history_capacity(), 32);
    press(&mut s, KeyCode::F15);
    assert_eq!(s.workspace().file().session.history_capacity(), 64);
    // Select NewSync, then OpenSync. DDR/ITG buttons are real keyboard actions.
    open(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::KeyD);
    assert_eq!(s.general_options().new_sync, 0.);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::ArrowRight);
    assert!((s.general_options().open_sync - 0.010).abs() < 1e-12);
    press(&mut s, KeyCode::Escape);
    assert_eq!(
        deadsync_editor::playback::Clock::capture(&s.workspace().file().session, 0)
            .unwrap()
            .music_time(48)
            .unwrap(),
        clock
    );
    assert!(!s.workspace().file().session.is_dirty());
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    let file = deadsync_editor::audio_preferences::File::open(&prefs).unwrap();
    assert_eq!(file.new_song_sync_offset().unwrap(), 0.);
    assert!((file.opening().unwrap().load_context().open_sync_offset() - 0.010).abs() < 1e-12);
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&prefs).unwrap()).unwrap();
    assert_eq!(json["Options"]["Future"], "keep");
    assert_eq!(
        json["Options"]["StartupStepGraphs"],
        serde_json::json!(["dance_single"])
    );
    let reopened = State::open_with_preferences(&song, 0, &prefs).unwrap();
    assert_eq!(reopened.general_options(), s.general_options());
    assert_eq!(reopened.workspace().file().session.history_capacity(), 64);
}
