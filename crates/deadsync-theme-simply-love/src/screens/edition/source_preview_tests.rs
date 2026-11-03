//! Exercise the source callback through physical keys; audio is queued, not decoded.
use super::*;
use std::time::Instant;
use deadsync_editor::history::file_intent::IntentGuard;
use deadsync_editor::key_binding_preferences::{
    Preferences,
    source_profile::{self, Platform},
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
fn state(chartless: bool) -> (tempfile::TempDir, State,u64) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preview.ssc");
    // Only a real existing resource path is needed to review/queue playback.
    // No claim of decoder or audio-device validation is made by this fixture.
    std::fs::write(root.path().join("sample.ogg"), b"queued resource").unwrap();
    let mut text = "#VERSION:0.83;#TITLE:Preview;#MUSIC:sample.ogg;#SAMPLESTART:1.5;#SAMPLELENGTH:4;#BPMS:0=120;".to_owned();
    if !chartless {
        text.push_str("#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;");
    }
    std::fs::write(&path, text).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    let preferences = source_profile::preset(&Preferences::default(), Platform::Other)
        .unwrap()
        .preferences;
    state.configure_key_binding_preferences(Arc::new(preferences));
    let commands=state.take_audio_commands();
    let owners:Vec<_>=commands.iter().filter_map(|command|match command{deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio{owner,..}=>Some(*owner),_=>None}).collect();
    assert_eq!(owners.len(),1);let owner=owners[0];assert_ne!(owner,0);
    let mut snapshot=deadsync_gameplay::GameplayAudioSnapshot::default();snapshot.editor.owner=owner;snapshot.editor.output.available=true;snapshot.editor.output.leased=true;snapshot.editor.output.period_control=deadsync_gameplay::EditorBufferControl::Adjustable;snapshot.editor.output.requested_period=Some(512);snapshot.editor.output.negotiated_period=Some(512);snapshot.editor.assets=deadsync_gameplay::EditorAssetReadiness{owner,registration:state.editor_asset_registration(),main:true,dedicated_preview:true};state.update_audio(snapshot);
    assert!(state.take_audio_commands().is_empty());
    (root, state,owner)
}
#[test]
fn source_preview_key_toggles_exact_sample_without_document_edits_en_fr() {
    for language in ["en", "fr"] {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        for chartless in [false, true] {
            let (root, mut state,owner) = state(chartless);
            let before = state.workspace.file().session.serialize().unwrap();
            let token = IntentGuard::capture(&state.workspace.file().session);
            let undo = state
                .workspace
                .file()
                .session
                .undo_label()
                .map(str::to_owned);
            press(&mut state, KeyCode::KeyP);
            let commands=state.take_audio_commands();
            let plays: Vec<_> = commands.iter()
                .filter_map(|command| {
                    if let deadsync_gameplay::GameplayAudioCommand::EditorPlayPreview {
                        owner:actual,
                        kind:deadsync_gameplay::EditorAssetKind::Main,
                        path,
                        cut,
                        looping,
                        rate,
                    } = command
                    {
                        assert_eq!(*actual,owner);
                        Some((path, cut, *looping, *rate))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(plays.len(), 1);
            assert_eq!(plays[0].0, &root.path().join("sample.ogg").canonicalize().unwrap());
            assert_eq!(plays[0].1.start_sec, 1.5);
            assert_eq!(plays[0].1.length_sec, 4.0);
            assert!(plays[0].2);
            assert_eq!(plays[0].3, 1.0);
            assert!(state.playback.is_some());
            assert!(state.take_audio_commands().is_empty());
            press(&mut state, KeyCode::KeyP);
            assert!(state.playback.is_none());
            assert!(matches!(state.take_audio_commands().as_slice(),[deadsync_gameplay::GameplayAudioCommand::EditorStopMusic{owner:actual}] if *actual==owner));
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert_eq!(
                state
                    .workspace
                    .file()
                    .session
                    .undo_label()
                    .map(str::to_owned),
                undo
            );
            assert!(token.is_current(&state.workspace.file().session));
        }
    }
}
#[test]
fn preview_rebinding_disables_old_key_and_invalid_media_never_queues_playback() {
    i18n::init_for_tests();
    let (root, mut state,owner) = state(false);
    let mut preferences = state.command_bindings.preferences.as_ref().clone();
    preferences
        .commands
        .insert("TogglePreview".into(), vec![vec!["F13".into()]]);
    state.configure_key_binding_preferences(Arc::new(preferences));
    press(&mut state, KeyCode::KeyP);
    assert!(!state.take_audio_commands().iter().any(|command| matches!(
        command,
        deadsync_gameplay::GameplayAudioCommand::EditorPlayPreview { .. }
    )));
    press(&mut state, KeyCode::F13);
    assert!(state.playback.is_some());
    assert!(matches!(state.take_audio_commands().as_slice(),[deadsync_gameplay::GameplayAudioCommand::EditorPlayPreview{owner:actual,looping:true,..}] if *actual==owner));
    press(&mut state, KeyCode::F13);
    std::fs::remove_file(root.path().join("sample.ogg")).unwrap();
    state.take_audio_commands();
    press(&mut state, KeyCode::F13);
    assert!(state.playback.is_none());
    assert!(!state.take_audio_commands().iter().any(|command| matches!(
        command,
        deadsync_gameplay::GameplayAudioCommand::EditorPlayPreview { .. }
    )));
}

#[test]
fn source_preview_stops_chart_transport_then_starts_sample_instead_of_only_pausing() {
    i18n::init_for_tests();
    let (_root, mut state,owner) = state(false);
    press(&mut state, KeyCode::Space);
    assert!(state.chart_transport_is_playing());
    state.take_audio_commands();
    press(&mut state, KeyCode::KeyP);
    assert!(state.is_playing());
    assert!(!state.chart_transport_is_playing());
    let commands=state.take_audio_commands();
    let stop = commands.iter()
        .position(|command| matches!(command, deadsync_gameplay::GameplayAudioCommand::EditorPauseMusic{owner:actual,..} if *actual==owner))
        .unwrap();
    let play = commands.iter().position(|command| {
        matches!(command, deadsync_gameplay::GameplayAudioCommand::EditorPlayPreview { owner:actual,looping: true, cut, .. } if *actual==owner && cut.start_sec == 1.5 && cut.length_sec == 4.0)
    }).unwrap();
    assert!(stop < play);
    assert!(state.paused_playback.is_none());
}
#[test]
fn source_music_rate_two_keeps_live_preview_descriptor_and_phase_at_normal_rate(){
    i18n::init_for_tests();let (_root,mut state,owner)=state(false);
    let mut audio=state.audio_preferences;audio.set_music_rate(2.).unwrap();state.commit_audio_preferences(audio).unwrap();state.take_audio_commands();
    press(&mut state,KeyCode::KeyP);let commands=state.take_audio_commands();
    assert!(commands.iter().any(|command|matches!(command,deadsync_gameplay::GameplayAudioCommand::EditorPlayPreview{owner:actual,rate,..}if *actual==owner&&*rate==1.)));
    let mut audio=state.audio_preferences;audio.set_field(deadsync_editor::audio_preferences::Field::PreviewFadeInTime,deadsync_editor::audio_preferences::FieldValue::Float(0.5)).unwrap();state.commit_audio_preferences(audio).unwrap();
    let commands=state.take_audio_commands();assert!(commands.iter().any(|command|matches!(command,deadsync_gameplay::GameplayAudioCommand::EditorSetPreviewFades{owner:actual,fade_in,..}if *actual==owner&&*fade_in==0.5)));
    assert!(!commands.iter().any(|command|matches!(command,deadsync_gameplay::GameplayAudioCommand::EditorSetMusicRate{..}|deadsync_gameplay::GameplayAudioCommand::EditorPlayPreview{..}|deadsync_gameplay::GameplayAudioCommand::EditorSeekMusic{..}|deadsync_gameplay::GameplayAudioCommand::EditorStopMusic{..})));
    assert!(state.is_playing());assert!(!state.chart_transport_is_playing());
}
