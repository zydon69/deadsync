use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::audio_preferences::{Field, FieldValue, File};
use deadsync_gameplay::{
    EditorBufferControl as Control, EditorOutputAck, EditorOutputOutcome as Outcome,
    EditorTickKind, GameplayAudioCommand as Command, GameplayAudioSnapshot,
};
use deadsync_theme_simply_love::{
    i18n,
    screens::edition::{
        State,
        decoder_transport::{Ack, Phase},
    },
};
use std::{fs, time::Instant};
fn key(s: &mut State, code: KeyCode, pressed: bool) -> bool {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    })
}
fn press(s: &mut State, code: KeyCode) {
    key(s, code, true);
    key(s, code, false);
}
fn ctrl(s: &mut State, code: KeyCode) {
    key(s, KeyCode::SuperLeft, true);
    press(s, code);
    key(s, KeyCode::SuperLeft, false);
}
fn setup(size: i32, couple: bool) -> (tempfile::TempDir, State, u64) {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854., 480.,
    ));
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("music.ogg"), b"command-only fixture").unwrap();
    let (kind, notes) = if couple {
        ("dance-couple", "10000000\n00000001\n10000000\n00000001")
    } else {
        ("dance-single", "1000\n0100\n0010\n0001")
    };
    let source = format!(
        "#VERSION:0.83;#TITLE:Audio;#MUSIC:music.ogg;#SAMPLESTART:0;#SAMPLELENGTH:1;#OFFSET:0;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#NOTEDATA:;#STEPSTYPE:{kind};#NOTES:{notes};"
    );
    fs::write(d.path().join("song.ssc"), source).unwrap();
    fs::write(d.path().join("prefs.json"),serde_json::to_vec(&serde_json::json!({"Version":1,"Audio":{"DspBufferSize":size},"KeyBinds":{"ToggleAssistTick":[["F13"]],"ToggleBeatTick":[["F14"]],"New":[["F15"]],"CloseEditor":[["F16"]],"Close":[["F17"]]}})).unwrap()).unwrap();
    let mut s = State::open(&d.path().join("song.ssc"), 0).unwrap();
    s.configure_audio_preferences(&d.path().join("prefs.json"))
        .unwrap();
    let commands = s.take_audio_commands();
    let owner = commands
        .iter()
        .find_map(|c| match c {
            Command::EditorBeginAudio { owner, .. } => Some(*owner),
            _ => None,
        })
        .unwrap();
    (d, s, owner)
}
fn host(state: &State, owner: u64) -> GameplayAudioSnapshot {
    let mut s = GameplayAudioSnapshot::default();
    s.stream_clock.valid_at = Instant::now();
    s.editor.owner = owner;
    let paths = state.editor_asset_paths();
    s.editor.assets = deadsync_gameplay::EditorAssetReadiness {
        owner,
        registration: state.editor_asset_registration(),
        main: paths[0].is_some(),
        dedicated_preview: paths[1].is_some(),
    };
    s.editor.tick_generation = 1;
    s.editor.sample_rate = 48000;
    s.editor.assist_duration = Some(0.2);
    s.editor.beat_duration = Some(0.2);
    s.editor.output.available = true;
    s.editor.output.leased = true;
    s.editor.output.period_control = Control::Adjustable;
    s.editor.output.count_control = Control::Unsupported;
    s.editor.output.requested_period = Some(512);
    s.editor.output.negotiated_period = Some(512);
    s
}
fn ready(s: &mut State, owner: u64) {
    s.update_audio(host(s, owner));
    assert!(
        !s.take_audio_commands()
            .iter()
            .any(|c| matches!(c, Command::EditorInitializeOutput { .. }))
    );
}
fn settings(s: &mut State) {
    for code in [KeyCode::F11, KeyCode::F6, KeyCode::F5] {
        press(s, code);
    }
}
fn text(s: &State, needle: &str) -> bool {
    let mut actors = Vec::new();
    s.actors(&mut actors);
    actors
        .iter()
        .any(|actor| matches!(actor,Actor::Text{content,..} if content.contains(needle)))
}
#[test]
fn all_sixteen_source_fields_are_reachable_localized_and_cancel_has_no_effect() {
    let (_d, mut s, owner) = setup(512, false);
    ready(&mut s, owner);
    let original = s.audio_preferences();
    settings(&mut s);
    for key_name in [
        "AudioNameRate",
        "AudioNameMain",
        "AudioNameMusic",
        "AudioNameFadeIn",
        "AudioNameFadeOut",
        "AudioNameAssist",
        "AudioNameSkipAssist",
        "AudioNameAssistVolume",
        "AudioNameAssistAttack",
        "AudioNameBeat",
        "AudioNameSkipBeat",
        "AudioNameBeatVolume",
        "AudioNameBeatAttack",
        "AudioNameOffset",
        "AudioNameDspCount",
        "AudioNameDspSize",
    ] {
        let label = i18n::tr("Edition", key_name);
        assert_ne!(label.as_ref(), key_name);
        assert!(text(&s, &label), "missing {key_name}");
        press(&mut s, KeyCode::ArrowRight);
    }
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.audio_preferences(), original);
    assert!(!s.workspace().file().session.is_dirty());
}
#[test]
fn dsp_save_undo_redo_persist_immediately_and_wait_for_next_context_without_reopen() {
    let (d, mut s, owner) = setup(512, false);
    ready(&mut s, owner);
    settings(&mut s);
    for _ in 0..15 {
        press(&mut s, KeyCode::ArrowRight);
    }
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    for code in [KeyCode::ArrowDown, KeyCode::ArrowDown, KeyCode::Enter] {
        press(&mut s, code);
    }
    assert_eq!(
        File::open(&d.path().join("prefs.json"))
            .unwrap()
            .audio()
            .value(Field::DspBufferSize),
        FieldValue::Int(513)
    );
    assert!(text(&s, &i18n::tr("Edition", "AudioDspRestartRequired")));
    assert!(!s.take_audio_commands().iter().any(|c| matches!(
        c,
        Command::EditorInitializeOutput { .. } | Command::EditorRestoreOutputLease { .. }
    )));
    // Apply returns to the excerpt inspector. Leave both nested inspectors
    // before sending the global history shortcuts.
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(
        s.audio_preferences().value(Field::DspBufferSize),
        FieldValue::Int(512)
    );
    assert_eq!(
        File::open(&d.path().join("prefs.json"))
            .unwrap()
            .audio()
            .value(Field::DspBufferSize),
        FieldValue::Int(512)
    );
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(
        s.audio_preferences().value(Field::DspBufferSize),
        FieldValue::Int(513)
    );
    assert_eq!(
        File::open(&d.path().join("prefs.json"))
            .unwrap()
            .audio()
            .value(Field::DspBufferSize),
        FieldValue::Int(513)
    );
    assert!(!s.take_audio_commands().iter().any(|c| matches!(
        c,
        Command::EditorInitializeOutput { .. } | Command::EditorRestoreOutputLease { .. }
    )));
    assert!(!s.workspace().file().session.is_dirty());
}
#[test]
fn initialization_waits_for_ack_filters_unsupported_fields_and_both_couple_players_tick() {
    let (_d, mut s, owner) = setup(768, true);
    press(&mut s, KeyCode::Enter);
    s.update_audio(host(&s, owner));
    let commands = s.take_audio_commands();
    let id = commands
        .iter()
        .find_map(|c| match c {
            Command::EditorInitializeOutput {
                owner: actual,
                id,
                period_frames,
                num_buffers,
            } => {
                assert_eq!(*actual, owner);
                assert_eq!(*period_frames, Some(768));
                assert_eq!(*num_buffers, None);
                Some(*id)
            }
            _ => None,
        })
        .unwrap();
    assert!(!commands.iter().any(|c| matches!(
        c,
        Command::EditorPlayMusic { .. }
            | Command::EditorStartMusic { .. }
            | Command::EditorResumeMusic { .. }
    )));
    let mut snapshot = host(&s, owner);
    snapshot.editor.output.requested_period = Some(768);
    snapshot.editor.output.negotiated_period = Some(768);
    snapshot.editor.output.ack = Some(EditorOutputAck {
        owner,
        id,
        outcome: Outcome::Applied,
    });
    s.update_audio(snapshot);
    assert!(
        s.take_audio_commands()
            .iter()
            .any(|c| matches!(c,Command::EditorStartMusic{owner:actual,..} if *actual==owner))
    );
    press(&mut s, KeyCode::F13);
    press(&mut s, KeyCode::F14);
    s.take_audio_commands();
    assert_eq!(
        s.audio_preferences().value(Field::UseAssistTick),
        FieldValue::Bool(true)
    );
    assert_eq!(
        s.audio_preferences().value(Field::UseBeatTick),
        FieldValue::Bool(true)
    );
    s.bind_decoder_generation(Some(42));
    s.set_decoder_status(
        42,
        Some(Ack {
            generation: 42,
            opened: true,
            phase: Phase::Producing,
        }),
    );
    let mut snapshot = host(&s, owner);
    snapshot.editor.tick_generation = 4;
    snapshot.editor.output.requested_period = Some(768);
    snapshot.editor.output.negotiated_period = Some(768);
    snapshot.stream_clock.has_music_mapping = true;
    snapshot.stream_clock.music_nanos = 0;
    s.set_decoder_stream_time(42, snapshot.stream_clock.valid_at, 0.);
    s.update_audio(snapshot);
    let commands = s.take_audio_commands();
    for kind in [EditorTickKind::Assist, EditorTickKind::Beat] {
        assert!(commands.iter().any(|c|matches!(c,Command::EditorTickWindow{owner:actual,music_generation:42,kind:actual_kind,samples,..} if *actual==owner&&*actual_kind==kind&&samples.as_ref()==[0,24000])),"both participants and beat schedule must remain present");
    }
}
#[test]
fn real_editor_exit_waits_for_restore_ack_before_releasing_scoped_gains() {
    let (_d, mut s, owner) = setup(512, false);
    ready(&mut s, owner);
    press(&mut s, KeyCode::F16);
    let commands = s.take_audio_commands();
    let id = commands
        .iter()
        .find_map(|c| match c {
            Command::EditorRestoreOutputLease { owner: actual, id } => {
                assert_eq!(*actual, owner);
                Some(*id)
            }
            _ => None,
        })
        .unwrap();
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::EditorEndAudio { .. }))
    );
    let mut snapshot = host(&s, owner);
    snapshot.editor.output.leased = false;
    snapshot.editor.output.ack = Some(EditorOutputAck {
        owner,
        id,
        outcome: Outcome::Applied,
    });
    s.update_audio(snapshot);
    assert!(
        s.take_audio_commands()
            .iter()
            .any(|c| matches!(c,Command::EditorEndAudio{owner:actual} if *actual==owner))
    );
}
#[test]
fn new_document_and_no_song_reuse_the_lazy_published_audio_session() {
    for (code, no_song) in [(KeyCode::F15, false), (KeyCode::F17, true)] {
        let (_d, mut s, owner) = setup(512, false);
        ready(&mut s, owner);
        press(&mut s, code);
        let deadline = Instant::now() + std::time::Duration::from_secs(4);
        while s.workspace().file().existing_path().is_some() {
            s.update();
            assert!(
                Instant::now() < deadline,
                "document publication did not finish"
            );
            std::thread::yield_now();
        }
        if no_song {
            assert!(!s.workspace().has_chart());
            assert!(text(&s, &i18n::tr("Edition", "FileNoSongHelp")));
        }
        let commands = s.take_audio_commands();
        assert!(
            !commands.iter().any(|c| matches!(
                c,
                Command::EditorBeginAudio { .. }
                    | Command::EditorEndAudio { .. }
                    | Command::EditorInitializeOutput { .. }
                    | Command::EditorRestoreOutputLease { .. }
            )),
            "document publication must not restart the audio context"
        );
        assert!(
            commands
                .iter()
                .any(|c| matches!(c,Command::EditorUpdateAudio{owner:actual,..} if *actual==owner)),
            "the published state retains the original owner"
        );
        s.update_audio(host(&s, owner));
        assert!(
            !s.take_audio_commands()
                .iter()
                .any(|c| matches!(c, Command::EditorInitializeOutput { .. }))
        );
    }
}
#[test]
fn failed_unavailable_restore_requires_explicit_second_close_and_never_claims_restoration() {
    let (_d, mut s, owner) = setup(512, false);
    ready(&mut s, owner);
    press(&mut s, KeyCode::F16);
    let id = s
        .take_audio_commands()
        .into_iter()
        .find_map(|c| match c {
            Command::EditorRestoreOutputLease { owner: actual, id } => {
                assert_eq!(actual, owner);
                Some(id)
            }
            _ => None,
        })
        .unwrap();
    let mut unavailable = host(&s, owner);
    unavailable.editor.output.available = false;
    unavailable.editor.output.ack = Some(EditorOutputAck {
        owner,
        id,
        outcome: Outcome::FailedUnavailable,
    });
    s.update_audio(unavailable);
    assert!(text(
        &s,
        &i18n::tr("Edition", "AudioOutputCloseWithoutAudio")
    ));
    assert!(!s.take_audio_commands().iter().any(|c| matches!(
        c,
        Command::EditorEndAudio { .. } | Command::EditorAbandonOutputLease { .. }
    )));
    press(&mut s, KeyCode::F16);
    let commands = s.take_audio_commands();
    let abandoned = commands
        .iter()
        .position(|c| matches!(c,Command::EditorAbandonOutputLease{owner:actual} if *actual==owner))
        .unwrap();
    let ended = commands
        .iter()
        .position(|c| matches!(c,Command::EditorEndAudio{owner:actual} if *actual==owner))
        .unwrap();
    assert!(abandoned < ended);
    assert!(!commands.iter().any(|c| matches!(
        c,
        Command::EditorRestoreOutputLease { .. } | Command::EditorInitializeOutput { .. }
    )));
}

#[test]
fn conflicting_audio_save_preserves_external_preferences_and_reports_localized_reason() {
    let (directory, mut state, owner) = setup(512, false);
    ready(&mut state, owner);
    settings(&mut state);
    for _ in 0..15 {
        press(&mut state, KeyCode::ArrowRight);
    }
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowRight);
    let path = directory.path().join("prefs.json");
    let mut external: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    external["Audio"]["DspBufferSize"] = 1024.into();
    external["Vendor"] = serde_json::json!({"preserve":"external value"});
    let bytes = serde_json::to_vec(&external).unwrap();
    fs::write(&path, &bytes).unwrap();
    let song = fs::read(directory.path().join("song.ssc")).unwrap();
    for code in [KeyCode::ArrowDown, KeyCode::ArrowDown, KeyCode::Enter] {
        press(&mut state, code);
    }
    assert!(text(
        &state,
        &i18n::tr("Edition", "DiagnosticExternalChange")
    ));
    assert_eq!(
        state.audio_preferences().value(Field::DspBufferSize),
        FieldValue::Int(512)
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(fs::read(directory.path().join("song.ssc")).unwrap(), song);
    assert!(!state.workspace().file().session.is_dirty());
    assert!(!state.take_audio_commands().iter().any(|command| matches!(
        command,
        Command::EditorInitializeOutput { .. } | Command::EditorRestoreOutputLease { .. }
    )));
}
