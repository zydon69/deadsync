use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_gameplay::GameplayAudioCommand;
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
fn setup() -> (tempfile::TempDir, State,u64) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("music.ogg"), b"command-only audio fixture").unwrap();
    fs::write(d.path().join("song.ssc"), b"#VERSION:0.83;#MUSIC:music.ogg;#OFFSET:0;#BPMS:0=120;#SAMPLESTART:0;#SAMPLELENGTH:1;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let mut s = State::open(&d.path().join("song.ssc"), 0).unwrap();
    transport_host::configure_projection(&mut s, &d.path().join("prefs.json"), false);
    let owner = transport_host::publish(&mut s);
    (d, s,owner)
}
#[test]
fn live_shortcuts_undo_and_pause_resume_keep_fractional_music_position() {
    let (d, mut s,owner) = setup();
    let original = s.workspace().file().session.serialize().unwrap();
    press(&mut s, KeyCode::Enter);
    s.take_audio_commands();
    s.update_audio(transport_host::snapshot(&s, 0.126));
    let stale = transport_host::snapshot(&s, 9.0);
    assert_eq!(s.workspace().row(), 12);
    modified(&mut s, KeyCode::ShiftLeft, KeyCode::ArrowRight);
    assert_eq!(s.audio_preferences().music_rate(), 1.1);
    assert_eq!(
        s.take_audio_commands(),
        vec![gains(owner),GameplayAudioCommand::EditorSetMusicRate{owner,rate:1.1}]
    );
    s.update_audio(stale);
    assert_eq!(s.workspace().row(), 12);
    let mut unmapped = transport_host::snapshot(&s, 0.0);
    unmapped.stream_clock.has_music_mapping = false;
    s.update_audio(unmapped);
    assert!(s.is_playing());
    s.update_audio(transport_host::snapshot(&s, 0.126));
    assert_eq!(s.workspace().row(), 12);
    modified(&mut s, KeyCode::SuperLeft, KeyCode::KeyZ);
    assert_eq!(
        s.take_audio_commands(),
        vec![gains(owner),GameplayAudioCommand::EditorSetMusicRate{owner,rate:1.0}]
    );
    assert!(s.is_playing());
    modified(&mut s, KeyCode::SuperLeft, KeyCode::KeyY);
    assert_eq!(
        s.take_audio_commands(),
        vec![gains(owner),GameplayAudioCommand::EditorSetMusicRate{owner,rate:1.1}]
    );
    assert!(s.is_playing());
    s.update_audio(transport_host::snapshot(&s, 0.126));
    press(&mut s, KeyCode::Enter);
    assert_eq!(
        s.take_audio_commands(),
        vec![GameplayAudioCommand::EditorPauseMusic {owner,
            seconds: 0.126
        }]
    );
    modified(&mut s, KeyCode::ShiftLeft, KeyCode::ArrowRight);
    assert_eq!(s.take_audio_commands(),vec![gains(owner)]);
    press(&mut s, KeyCode::Enter);
    assert!(
        matches!(s.take_audio_commands().as_slice(), [GameplayAudioCommand::EditorResumeMusic {owner:actual,seconds, rate, ..}] if *actual==owner && *seconds == 0.126 && *rate == 1.2)
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
    let (d, mut s,owner) = setup();
    for code in [
        KeyCode::F11,
        KeyCode::F6,
        KeyCode::F5,
        KeyCode::Tab,
        KeyCode::ArrowRight,
    ] {
        press(&mut s, code);
    }
    assert_eq!(s.audio_preferences().music_rate(), 1.0);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let prefix = &i18n::tr("Edition","AudioNameRate");
    assert!(actors.iter().any(|actor| matches!(actor, Actor::Text {content, ..} if content.contains(prefix.as_ref()))));
    assert!(actors.iter().any(|actor| matches!(actor, Actor::Text {content, ..} if content.contains("1.001"))));
    for code in [KeyCode::ArrowDown, KeyCode::ArrowDown, KeyCode::Enter] {
        press(&mut s, code);
    }
    assert_eq!(s.audio_preferences().music_rate(), 1.001);
    assert_eq!(s.take_audio_commands(),vec![gains(owner)]);
    let saved = fs::read(d.path().join("prefs.json")).unwrap();
    for code in [
        KeyCode::F5,
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
        matches!(s.take_audio_commands().as_slice(), [GameplayAudioCommand::EditorPlayPreview {owner:actual,rate, looping: true, ..}] if *actual==owner && *rate == 1.0)
    );
}
#[test]
fn conflicting_preferences_refuse_live_change_and_undo_without_stopping_transport() {
    let (d, mut s, _owner) = setup();
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
    let (d, mut s,owner) = setup();
    let path = d.path().join("prefs.json");
    modified(&mut s, KeyCode::ShiftLeft, KeyCode::ArrowRight);
    assert_eq!(s.take_audio_commands(),vec![gains(owner)]);
    for code in [
        KeyCode::F11,
        KeyCode::F6,
        KeyCode::F5,
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
    // Runtime and disk are independent preference owners: committing only
    // the rate retains the live fade while preserving the external disk fade.
    assert_eq!(s.audio_preferences().fade_out(), 1.5);
    assert_eq!(s.take_audio_commands(),vec![gains(owner)]);
    let persisted = File::open(&path).unwrap().audio();
    assert_eq!(persisted.fade_out(), 2.5);
    assert_eq!(persisted.music_rate(), s.audio_preferences().music_rate());
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
        vec![gains(owner),GameplayAudioCommand::EditorSetPreviewFades {
                owner,
                fade_in: 0.0,
                fade_out: 2.5
            },
            GameplayAudioCommand::EditorSetMusicRate{owner,rate:1.5}]
    );
}

fn gains(owner:u64)->GameplayAudioCommand {
    GameplayAudioCommand::EditorUpdateAudio{owner,gains:deadsync_gameplay::EditorAudioGains{main:1.,music:0.5,assist: 0.0,beat: 0.0,
        },
    }}
