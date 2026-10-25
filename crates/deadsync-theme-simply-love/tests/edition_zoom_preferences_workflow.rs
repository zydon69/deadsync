use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::Actor,
    space::{Metrics, set_current_metrics},
};
use deadsync_editor::{audio_preferences::File, zoom_preferences::Preferences};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    time::{Duration, Instant},
};
fn key(state: &mut State, code: KeyCode, pressed: bool) {
    state.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(state: &mut State, code: KeyCode) {
    key(state, code, true);
    key(state, code, false);
}
fn open(state: &mut State) {
    key(state, KeyCode::ShiftLeft, true);
    press(state, KeyCode::KeyZ);
    key(state, KeyCode::ShiftLeft, false);
}
fn down(state: &mut State, count: usize) {
    for _ in 0..count {
        press(state, KeyCode::ArrowDown);
    }
}
fn texts(state: &State) -> Vec<String> {
    let mut actors = Vec::new();
    state.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|a| match a {
            Actor::Text { content, .. } => Some(content.to_string()),
            _ => None,
        })
        .collect()
}
fn setup() -> (tempfile::TempDir, State, std::path::PathBuf, Vec<u8>) {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;".to_vec();
    fs::write(&path, &source).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    let prefs = dir.path().join("prefs.json");
    state.configure_audio_preferences(&prefs).unwrap();
    (dir, state, prefs, source)
}
#[test]
fn french_settings_stage_save_reopen_and_drive_real_zoom_without_song_changes() {
    let (dir, mut state, path, source) = setup();
    open(&mut state);
    assert!(
        texts(&state)
            .iter()
            .any(|s| s.contains("Animation du zoom : 0.100 s"))
    );
    press(&mut state, KeyCode::Digit0);
    down(&mut state, 1);
    press(&mut state, KeyCode::ArrowRight);
    down(&mut state, 1);
    press(&mut state, KeyCode::ArrowLeft);
    down(&mut state, 1);
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.zoom_preferences(), Preferences::default());
    assert!(!path.exists());
    down(&mut state, 2);
    press(&mut state, KeyCode::Enter);
    let prefs = state.zoom_preferences();
    assert_eq!(prefs.duration(), 0.0);
    assert!((prefs.multiplier() - 1.3).abs() < 1e-9);
    assert_eq!(prefs.size_cap(), 0.99);
    assert!(prefs.limited());
    assert_eq!(File::open(&path).unwrap().zoom(), prefs);
    let mut reopened = State::open(&dir.path().join("song.ssc"), 0).unwrap();
    reopened.configure_audio_preferences(&path).unwrap();
    assert_eq!(reopened.zoom_preferences(), prefs);
    assert_eq!(reopened.spacing_zoom(), 0.99);
    assert_eq!(state.spacing_zoom(), 0.99);
    assert!(
        texts(&state)
            .iter()
            .any(|s| s == "Réglages du zoom enregistrés.")
    );
    state.configure_audio_preferences(&path).unwrap();
    assert_eq!(state.zoom_preferences(), prefs);
    key(&mut state, KeyCode::ControlLeft, true);
    press(&mut state, KeyCode::ArrowUp);
    key(&mut state, KeyCode::ControlLeft, false);
    state.update_with_time(Instant::now() + Duration::from_secs(1));
    assert_eq!(state.spacing_zoom(), 0.99);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
    assert!(!state.workspace().file().session.is_dirty());
}
#[test]
fn cancellation_defaults_and_fine_steps_remain_staged() {
    let (_dir, mut state, path, _) = setup();
    open(&mut state);
    key(&mut state, KeyCode::ShiftLeft, true);
    press(&mut state, KeyCode::ArrowRight);
    key(&mut state, KeyCode::ShiftLeft, false);
    assert!(texts(&state).iter().any(|s| s.contains("0.101 s")));
    press(&mut state, KeyCode::Escape);
    assert_eq!(state.zoom_preferences(), Preferences::default());
    assert!(!path.exists());
    open(&mut state);
    press(&mut state, KeyCode::ArrowRight);
    down(&mut state, 4);
    press(&mut state, KeyCode::Enter);
    assert!(texts(&state).iter().any(|s| s.contains("0.100 s")));
    down(&mut state, 2);
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.zoom_preferences(), Preferences::default());
    assert!(!path.exists());
}
#[test]
fn external_changes_refuse_save_and_keep_reviewable_draft_without_applying() {
    let (_dir, mut state, path, _) = setup();
    open(&mut state);
    press(&mut state, KeyCode::ArrowRight);
    let external = r#"{"Version":1,"Zoom":{"ZoomMultiplier":2}}"#;
    fs::write(&path, external).unwrap();
    down(&mut state, 5);
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.zoom_preferences(), Preferences::default());
    assert_eq!(state.spacing_zoom(), 1.0);
    assert!(texts(&state).iter().any(|s| s.contains("0.110 s")));
    assert_eq!(fs::read_to_string(&path).unwrap(), external);
    press(&mut state, KeyCode::Escape);
    assert_eq!(fs::read_to_string(&path).unwrap(), external);
}
