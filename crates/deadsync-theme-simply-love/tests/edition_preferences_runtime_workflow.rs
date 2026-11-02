use deadlib_present::actors::Actor;
use deadsync_editor::{
    audio_preferences::{Audio, File},
    runtime,
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::fs;

#[test]
fn editor_bootstrap_loads_application_preferences_and_keeps_unknown_version_read_only() {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let prefs = dir.path().join("editor-preferences.json");
    let song = dir.path().join("song.ssc");
    let source = "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&song, source).unwrap();
    let mut file = File::open(&prefs).unwrap();
    let mut audio = Audio::default();
    audio.set_fade_in(0.33).unwrap();
    file.set_audio(audio);
    file.save().unwrap();
    runtime::init_paths(dir.path().join("AutogenConfigs"), prefs.clone()).unwrap();
    let state = State::open(&song, 0).unwrap();
    assert_eq!(state.audio_preferences(), audio);
    fs::write(&prefs, br#"{"Version":2,"Future":true}"#).unwrap();
    let state = State::open(&song, 0).unwrap();
    assert_eq!(state.audio_preferences(), Audio::default());
    let mut actors = Vec::new();
    state.actors(&mut actors);
    assert!(actors.iter().any(|actor| matches!(actor,Actor::Text{content,..} if content.to_string().contains(i18n::tr("Edition", "DiagnosticPreferencesVersion").as_ref()))));
    assert_eq!(fs::read(&prefs).unwrap(), br#"{"Version":2,"Future":true}"#);
    assert_eq!(fs::read_to_string(&song).unwrap(), source);
}
