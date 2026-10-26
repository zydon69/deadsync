use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::Actor,
    space::{Metrics, set_current_metrics},
};
use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn event(s: &mut State, code: KeyCode, pressed: bool) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(s: &mut State, code: KeyCode) {
    event(s, code, true);
    event(s, code, false);
}
fn open(s: &mut State) {
    event(s, KeyCode::ShiftLeft, true);
    press(s, KeyCode::KeyS);
    event(s, KeyCode::ShiftLeft, false);
}
fn texts(s: &State) -> Vec<String> {
    let mut a = Vec::new();
    s.actors(&mut a);
    a.into_iter()
        .filter_map(|a| match a {
            Actor::Text { content, .. } => Some(content.to_string()),
            _ => None,
        })
        .collect()
}
fn has(s: &State, needle: &str) -> bool {
    texts(s).iter().any(|t| t.contains(needle))
}
fn save(s: &mut State) {
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::Enter);
}
fn setup(empty: bool) -> (tempfile::TempDir, State) {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("music.ogg"), b"command asset").unwrap();
    let source = if empty {
        "#VERSION:0.83;#TITLE:Empty;"
    } else {
        "#VERSION:0.83;#MUSIC:music.ogg;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;"
    };
    fs::write(d.path().join("song.ssc"), source).unwrap();
    let s = State::open(&d.path().join("song.ssc"), 0).unwrap();
    (d, s)
}
fn mapped(s: &mut State) {
    let mut a = GameplayAudioSnapshot::default();
    a.stream_clock.valid_at = Instant::now();
    a.stream_clock.has_music_mapping = true;
    a.stream_clock.music_nanos = 126_000_000;
    s.update_audio(a);
}
#[test]
fn french_panel_saves_reloads_and_drives_actual_playback_policy_without_song_edits() {
    let (d, mut s) = setup(false);
    let p = d.path().join("prefs.json");
    let original=b"{\"Version\":1,\"Future\":9,\"Scroll\":{\"Unknown\":{\"a\":1}},\"Audio\":{\"PreviewFadeOutTime\":0.4}}";
    fs::write(&p, original).unwrap();
    s.configure_audio_preferences(&p).unwrap();
    let source = fs::read(d.path().join("song.ssc")).unwrap();
    press(&mut s, KeyCode::Enter);
    s.take_audio_commands();
    open(&mut s);
    assert!(!s.is_playing());
    assert_eq!(
        s.take_audio_commands(),
        vec![GameplayAudioCommand::StopMusic]
    );
    assert!(has(
        &s,
        "Arrêter la lecture lors du déplacement avec les flèches : Non"
    ));
    press(&mut s, KeyCode::ArrowRight);
    assert!(has(&s, "flèches : Oui"));
    assert!(!s.scroll_preferences().stop_playback());
    assert_eq!(fs::read(&p).unwrap(), original);
    save(&mut s);
    assert!(s.scroll_preferences().stop_playback());
    assert!(has(&s, "Préférences de navigation enregistrées."));
    let v: serde_json::Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
    assert_eq!(v["Future"], 9);
    assert_eq!(v["Scroll"]["Unknown"]["a"], 1);
    assert_eq!(v["Audio"]["PreviewFadeOutTime"], 0.4);
    let mut r = State::open(&d.path().join("song.ssc"), 0).unwrap();
    r.configure_audio_preferences(&p).unwrap();
    assert!(r.scroll_preferences().stop_playback());
    press(&mut r, KeyCode::Enter);
    r.take_audio_commands();
    mapped(&mut r);
    press(&mut r, KeyCode::ArrowDown);
    assert!(!r.is_playing());
    assert_eq!(
        r.take_audio_commands(),
        vec![GameplayAudioCommand::StopMusic]
    );
    open(&mut r);
    press(&mut r, KeyCode::ArrowDown);
    press(&mut r, KeyCode::ArrowDown);
    press(&mut r, KeyCode::Enter);
    assert!(has(&r, "flèches : Non"));
    press(&mut r, KeyCode::ArrowDown);
    press(&mut r, KeyCode::Enter);
    assert!(!r.scroll_preferences().stop_playback());
    press(&mut r, KeyCode::Enter);
    r.take_audio_commands();
    press(&mut r, KeyCode::ArrowDown);
    assert!(r.is_playing());
    assert!(matches!(
        r.take_audio_commands().as_slice(),
        [GameplayAudioCommand::SeekMusic(_)]
    ));
    assert!(!s.workspace().file().session.is_dirty());
    assert!(!r.workspace().file().session.is_dirty());
    assert_eq!(fs::read(d.path().join("song.ssc")).unwrap(), source);
}
#[test]
fn draft_cancel_shortcuts_repeat_and_external_conflict_preserve_active_policy() {
    let (d, mut s) = setup(false);
    let p = d.path().join("prefs.json");
    fs::write(&p, b"{\"Version\":1}").unwrap();
    s.configure_audio_preferences(&p).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "flèches : Oui"));
    s.raw_key(&RawKeyboardEvent {
        code: KeyCode::ArrowRight,
        pressed: true,
        repeat: true,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    assert!(has(&s, "flèches : Oui"));
    press(&mut s, KeyCode::Home);
    assert!(has(&s, "flèches : Non"));
    press(&mut s, KeyCode::ArrowLeft);
    press(&mut s, KeyCode::Digit0);
    assert!(has(&s, "flèches : Non"));
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Escape);
    assert!(!s.scroll_preferences().stop_playback());
    assert_eq!(fs::read(&p).unwrap(), b"{\"Version\":1}");
    open(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    let foreign = b"{\"Version\":1,\"Future\":10}";
    fs::write(&p, foreign).unwrap();
    save(&mut s);
    assert!(has(&s, "flèches : Oui"));
    assert!(!s.scroll_preferences().stop_playback());
    assert_eq!(fs::read(&p).unwrap(), foreign);
    press(&mut s, KeyCode::Escape);
    open(&mut s);
    assert!(has(&s, "flèches : Non"));
    event(&mut s, KeyCode::ShiftLeft, true);
    press(&mut s, KeyCode::Tab);
    event(&mut s, KeyCode::ShiftLeft, false);
    press(&mut s, KeyCode::NumpadEnter);
    assert!(has(
        &s,
        "Modifications des préférences de navigation abandonnées."
    ));
    assert!(!s.workspace().file().session.is_dirty());
}
#[test]
fn unavailable_or_future_preferences_remain_reviewable_and_are_never_overwritten() {
    let (d, mut s) = setup(false);
    open(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    save(&mut s);
    assert!(has(
        &s,
        "Le chemin des préférences de l’éditeur n’est pas configuré."
    ));
    assert!(has(&s, "flèches : Oui"));
    press(&mut s, KeyCode::Escape);
    let p = d.path().join("prefs.json");
    let bytes = b"{\"Version\":99}";
    fs::write(&p, bytes).unwrap();
    assert!(s.configure_audio_preferences(&p).is_err());
    open(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    save(&mut s);
    assert!(has(&s, "flèches : Oui"));
    assert_eq!(fs::read(&p).unwrap(), bytes);
    assert!(!s.scroll_preferences().stop_playback());
}
#[test]
fn chartless_workspace_can_save_application_navigation_preferences() {
    let (d, mut s) = setup(true);
    assert!(!s.workspace().has_chart());
    let p = d.path().join("prefs.json");
    s.configure_audio_preferences(&p).unwrap();
    open(&mut s);
    assert!(has(&s, "flèches : Non"));
    press(&mut s, KeyCode::ArrowRight);
    save(&mut s);
    assert!(s.scroll_preferences().stop_playback());
    assert!(p.is_file());
    assert!(!s.workspace().file().session.is_dirty());
}

#[test]
fn navigation_shortcut_hint_has_bounded_width_and_does_not_overlap_status_footer() {
    let (_d, s) = setup(false);
    let mut a = Vec::new();
    s.actors(&mut a);
    let hint=a.into_iter().find(|a|matches!(a,Actor::Text{content,..} if content.contains("Maj+S : réglages de navigation."))).expect("visible shortcut hint");
    match hint {
        Actor::Text {
            offset,
            wrap_width_pixels,
            max_width,
            max_height,
            ..
        } => {
            assert_eq!(offset, [18.0, 410.0]);
            assert_eq!(wrap_width_pixels, Some(1636));
            assert_eq!(max_width, Some(818.0));
            assert_eq!(max_height, Some(20.0));
        }
        _ => unreachable!(),
    }
}
