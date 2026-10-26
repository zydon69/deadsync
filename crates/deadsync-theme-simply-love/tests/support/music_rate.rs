use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn key(s: &mut State, code: KeyCode, pressed: bool) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(s: &mut State, code: KeyCode) {
    key(s, code, true);
    key(s, code, false);
}
fn modified(s: &mut State, modifier: KeyCode, code: KeyCode) {
    key(s, modifier, true);
    press(s, code);
    key(s, modifier, false);
}
fn snapshot(seconds: f64) -> GameplayAudioSnapshot {
    let mut s = GameplayAudioSnapshot::default();
    s.stream_clock.valid_at = Instant::now();
    s.stream_clock.has_music_mapping = true;
    s.stream_clock.music_nanos = (seconds * 1e9).round() as i64;
    s
}
fn setup() -> (tempfile::TempDir, State) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("music.ogg"), b"command-only audio fixture").unwrap();
    fs::write(d.path().join("song.ssc"), b"#VERSION:0.83;#MUSIC:music.ogg;#OFFSET:0;#BPMS:0=120;#SAMPLESTART:0;#SAMPLELENGTH:1;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let mut s = State::open(&d.path().join("song.ssc"), 0).unwrap();
    s.configure_audio_preferences(&d.path().join("prefs.json"))
        .unwrap();
    (d, s)
}
#[test]
fn live_shortcuts_undo_and_pause_resume_keep_fractional_music_position() {
    let (d, mut s) = setup();
    let original = s.workspace().file().session.serialize().unwrap();
    press(&mut s, KeyCode::Enter);
    s.take_audio_commands();
    s.update_audio(snapshot(0.126));
    let stale = snapshot(9.0);
    assert_eq!(s.workspace().row(), 12);
    modified(&mut s, KeyCode::ShiftLeft, KeyCode::ArrowRight);
    assert_eq!(s.audio_preferences().music_rate(), 1.1);
    assert_eq!(
        s.take_audio_commands(),
        vec![GameplayAudioCommand::SetMusicRate(1.1)]
    );
    s.update_audio(stale);
    assert_eq!(s.workspace().row(), 12);
    let mut unmapped = snapshot(0.0);
    unmapped.stream_clock.has_music_mapping = false;
    s.update_audio(unmapped);
    assert!(s.is_playing());
    s.update_audio(snapshot(0.126));
    assert_eq!(s.workspace().row(), 12);
    modified(&mut s, KeyCode::SuperLeft, KeyCode::KeyZ);
    assert_eq!(
        s.take_audio_commands(),
        vec![GameplayAudioCommand::SetMusicRate(1.0)]
    );
    assert!(s.is_playing());
    modified(&mut s, KeyCode::SuperLeft, KeyCode::KeyY);
    assert_eq!(
        s.take_audio_commands(),
        vec![GameplayAudioCommand::SetMusicRate(1.1)]
    );
    assert!(s.is_playing());
    s.update_audio(snapshot(0.126));
    press(&mut s, KeyCode::Enter);
    assert_eq!(
        s.take_audio_commands(),
        vec![GameplayAudioCommand::StopMusic]
    );
    modified(&mut s, KeyCode::ShiftLeft, KeyCode::ArrowRight);
    assert!(s.take_audio_commands().is_empty());
    press(&mut s, KeyCode::Enter);
    assert!(
        matches!(s.take_audio_commands().as_slice(), [GameplayAudioCommand::PlayMusic {cut, rate, ..}] if cut.start_sec == 0.126 && *rate == 1.2)
    );
    assert!(!s.workspace().file().session.is_dirty());
    assert_eq!(s.workspace().file().session.serialize().unwrap(), original);
    assert_eq!(
        deadsync_editor::audio_preferences::File::open(&d.path().join("prefs.json"))
            .unwrap()
            .audio()
            .music_rate(),
        1.2
    );
}
#[test]
fn typed_rate_panel_stages_saves_cancels_and_sample_preview_stays_at_unity() {
    let (d, mut s) = setup();
    for code in [
        KeyCode::F11,
        KeyCode::F6,
        KeyCode::F5,
        KeyCode::Tab,
        KeyCode::Tab,
        KeyCode::ArrowRight,
    ] {
        press(&mut s, code);
    }
    assert_eq!(s.audio_preferences().music_rate(), 1.0);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let prefix = if LANGUAGE == "fr" {
        "Vitesse de lecture :"
    } else {
        "Music rate:"
    };
    assert!(actors.iter().any(|actor| matches!(actor, Actor::Text {content, ..} if content.starts_with(prefix) && content.contains("1.001"))));
    for code in [KeyCode::ArrowDown, KeyCode::ArrowDown, KeyCode::Enter] {
        press(&mut s, code);
    }
    assert_eq!(s.audio_preferences().music_rate(), 1.001);
    let saved = fs::read(d.path().join("prefs.json")).unwrap();
    for code in [
        KeyCode::F5,
        KeyCode::Tab,
        KeyCode::Tab,
        KeyCode::ArrowRight,
        KeyCode::Escape,
    ] {
        press(&mut s, code);
    }
    assert_eq!(s.audio_preferences().music_rate(), 1.001);
    assert_eq!(fs::read(d.path().join("prefs.json")).unwrap(), saved);
    press(&mut s, KeyCode::F4);
    assert!(
        matches!(s.take_audio_commands().as_slice(), [GameplayAudioCommand::PlayMusic {rate, looping: true, ..}] if *rate == 1.0)
    );
}
#[test]
fn conflicting_preferences_refuse_live_change_and_undo_without_stopping_transport() {
    let (d, mut s) = setup();
    modified(&mut s, KeyCode::ShiftLeft, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Enter);
    s.take_audio_commands();
    let revision = s.workspace().file().session.revision();
    fs::write(d.path().join("prefs.json"), b"External preferences").unwrap();
    modified(&mut s, KeyCode::ShiftLeft, KeyCode::ArrowRight);
    assert_eq!(s.audio_preferences().music_rate(), 1.1);
    assert_eq!(s.workspace().file().session.revision(), revision);
    assert!(s.is_playing());
    assert!(s.take_audio_commands().is_empty());
    modified(&mut s, KeyCode::SuperLeft, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.revision(), revision);
    assert_eq!(s.audio_preferences().music_rate(), 1.1);
    assert!(s.is_playing());
    assert!(s.take_audio_commands().is_empty());
    assert_eq!(
        fs::read(d.path().join("prefs.json")).unwrap(),
        b"External preferences"
    );
}

#[test]
fn panel_commit_preserves_merged_fields_and_reloading_rate_updates_live_transport() {
    use deadsync_editor::audio_preferences::File;
    let (d, mut s) = setup();
    let path = d.path().join("prefs.json");
    modified(&mut s, KeyCode::ShiftLeft, KeyCode::ArrowRight);
    for code in [
        KeyCode::F11,
        KeyCode::F6,
        KeyCode::F5,
        KeyCode::Tab,
        KeyCode::Tab,
        KeyCode::ArrowRight,
    ] {
        press(&mut s, code);
    }
    let mut external = File::open(&path).unwrap();
    let mut audio = external.audio();
    audio.set_fade_out(2.5).unwrap();
    external.set_audio(audio);
    external.save().unwrap();
    for code in [KeyCode::ArrowDown, KeyCode::ArrowDown, KeyCode::Enter] {
        press(&mut s, code);
    }
    assert_eq!(s.audio_preferences().fade_out(), 2.5);
    assert_eq!(s.audio_preferences(), File::open(&path).unwrap().audio());
    for code in [KeyCode::Escape, KeyCode::Escape, KeyCode::Enter] {
        press(&mut s, code);
    }
    assert!(s.is_playing());
    s.take_audio_commands();
    let mut external = File::open(&path).unwrap();
    let mut audio = external.audio();
    audio.set_music_rate(1.5).unwrap();
    external.set_audio(audio);
    external.save().unwrap();
    s.configure_audio_preferences(&path).unwrap();
    assert!(s.is_playing());
    assert_eq!(
        s.take_audio_commands(),
        vec![GameplayAudioCommand::SetMusicRate(1.5)]
    );
}
