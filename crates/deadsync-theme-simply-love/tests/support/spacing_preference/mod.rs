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
fn raw_unknown_uses_source_fallback_and_toggle_ordinal_without_losing_document() {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    let root = tempfile::tempdir().unwrap();
    let song = root.path().join("song.ssc");
    std::fs::write(
        &song,
        b"#VERSION:0.83;#TITLE:Native;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let prefs = root.path().join("prefs.json");
    std::fs::write(&prefs,br#"{"Version":1,"Scroll":{"SpacingMode":"-2","Future":17},"KeyBinds":{"ToggleSpacingMode":[["F14"]]}}"#).unwrap();
    let mut state = State::open_with_preferences(&song, 0, &prefs).unwrap();
    let bytes = state.workspace().file().session.serialize().unwrap();
    assert_eq!(state.spacing_preference(), Preference::Unknown(-2));
    assert_eq!(state.spacing_mode(), SpacingMode::ConstantTime);
    let mut actors = Vec::new();
    state.actors(&mut actors);
    press(&mut state, KeyCode::F14);
    assert_eq!(state.spacing_preference(), Preference::Unknown(-1));
    assert_eq!(state.spacing_mode(), SpacingMode::ConstantTime);
    actors.clear();
    state.actors(&mut actors);
    assert!(texts(&actors).iter().any(|text| text.contains("-1")));
    press(&mut state, KeyCode::F14);
    assert_eq!(state.spacing_preference(), Preference::ConstantTime);
    press(&mut state, KeyCode::F14);
    assert_eq!(state.spacing_preference(), Preference::ConstantRow);
    assert_eq!(state.spacing_mode(), SpacingMode::ConstantRow);
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
