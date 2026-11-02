use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::{scroll_preferences::spacing::Preference, zoom::SpacingMode};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::time::Instant;
fn press(state: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
#[test]
fn keyboard_restore_all_scroll_owners_and_undo_redo_preserve_song_and_raw_unknown() {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    let root = tempfile::tempdir().unwrap();
    let song = root.path().join("song.ssc");
    std::fs::write(
        &song,
        b"#VERSION:0.83;#TITLE:Native;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let prefs = root.path().join("prefs.json");
    std::fs::write(&prefs,br#"{"Version":1,"Scroll":{"SpacingMode":"99","Reverse":true,"TimeBasedPixelsPerSecond":450,"RowBasedPixelsPerRow":3,"VariablePixelsPerSecondAtDefaultBPM":600,"ScrollWheelRows":24,"ScrollWheelTime":0.125,"StopPlaybackWhenScrolling":true,"StopPlaybackWhenDraggingScrollBars":true,"ShowScrollControlPreferencesWindow":true,"Future":17},"Zoom":{"ZoomMultiplier":1.5,"ScrollInterpolationDuration":0.2,"SizeCap":0.75,"LimitZoomToSize":true},"Waveform":{"RowBasedWaveFormScrollMode":"CurrentTempo","EnableWaveForm":false},"KeyBinds":{"OpenScrollSettings":[["F14"]],"Undo":[["F15"]],"Redo":[["F16"]]}}"#).unwrap();
    let mut state = State::open_with_preferences(&song, 0, &prefs).unwrap();
    let before = state.scroll_snapshot();
    let bytes = state.workspace().file().session.serialize().unwrap();
    assert_eq!(before.spacing, Preference::Unknown(99));
    assert_eq!(state.spacing_mode(), SpacingMode::ConstantTime);
    assert!(state.display_reversed());
    press(&mut state, KeyCode::F14);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.scroll_snapshot(), before.restored());
    assert_eq!(state.spacing_preference(), Preference::ConstantTime);
    assert!(!state.display_reversed());
    assert_eq!(
        state.workspace().file().session.undo_label(),
        Some("Restore Scroll defaults")
    );
    let mut actors = Vec::new();
    state.actors(&mut actors);
    assert!(
        texts(&actors)
            .iter()
            .any(|t| t.contains(&*i18n::tr("Edition", "ScrollAllDefaultsSaved")))
    );
    press(&mut state, KeyCode::Escape);
    press(&mut state, KeyCode::F15);
    assert_eq!(state.scroll_snapshot(), before);
    assert_eq!(state.spacing_preference(), Preference::Unknown(99));
    assert!(state.display_reversed());
    press(&mut state, KeyCode::F16);
    assert_eq!(state.scroll_snapshot(), before.restored());
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&prefs).unwrap()).unwrap();
    assert_eq!(json["Scroll"]["Future"], 17);
    assert_eq!(json["Scroll"]["ShowScrollControlPreferencesWindow"], true);
    assert_eq!(json["Waveform"]["EnableWaveForm"], false);
    assert_eq!(state.workspace().file().session.serialize().unwrap(), bytes);
    assert!(!state.workspace().file().session.is_dirty());
}
fn texts(actors: &[deadlib_present::actors::Actor]) -> Vec<String> {
    use deadlib_present::actors::Actor;
    let mut result = Vec::new();
    for actor in actors {
        match actor {
            Actor::Text { content, .. } => result.push(content.as_str().to_owned()),
            Actor::Frame { children, .. } => result.extend(texts(children)),
            Actor::SharedFrame { children, .. } | Actor::SharedTransform { children, .. } => {
                result.extend(texts(children))
            }
            _ => {}
        }
    }
    result
}
