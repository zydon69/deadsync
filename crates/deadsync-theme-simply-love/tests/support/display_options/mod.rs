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
    press(s, KeyCode::ArrowUp);
    press(s, KeyCode::ArrowUp);
    press(s, KeyCode::Enter);
}
#[test]
fn keyboard_display_cancel_apply_undo_and_reopen_preserve_song_and_budgets() {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ssc");
    std::fs::write(&song,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let path = dir.path().join("prefs.json");
    std::fs::write(
        &path,
        r#"{"Version":1,"Options":{"MaxEventsToDraw":37,"Future":"keep"},"KeyBinds":{"Undo":[["F14"]],"Redo":[["F15"]]}}"#,
    )
    .unwrap();
    let mut s = State::open(&song, 0).unwrap();
    s.configure_audio_preferences(&path).unwrap();
    let bytes = s.workspace().file().session.serialize().unwrap();
    open(&mut s);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let label = i18n::tr("Edition", "OptionsDisplayDefaults");
    assert!(
        actors
            .iter()
            .any(|a| matches!(a,Actor::Text{content,..} if content.as_str()==&*label))
    );
    press(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::ArrowRight);
    assert!(s.render_policy().notes);
    press(&mut s, KeyCode::Escape);
    assert!(s.render_policy().notes);
    assert!(s.workspace().file().session.undo_label().is_none());
    open(&mut s);
    press(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::Enter);
    assert!(!s.render_policy().notes);
    assert_eq!(s.render_limits().max_events, 37);
    assert!(!s.workspace().file().session.is_dirty());
    assert_eq!(s.workspace().file().session.serialize().unwrap(), bytes);
    press(&mut s, KeyCode::F14);
    assert!(s.render_policy().notes);
    press(&mut s, KeyCode::F15);
    assert!(!s.render_policy().notes);
    let mut reopened = State::open(&song, 0).unwrap();
    reopened.configure_audio_preferences(&path).unwrap();
    assert_eq!(reopened.render_policy(), s.render_policy());
    assert_eq!(reopened.render_limits().max_events, 37);
}
