use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot};
use deadsync_theme_simply_love::{
    i18n,
    screens::edition::{
        State,
        decoder_transport::{Ack, Phase},
    },
};
use std::{fs, time::Instant};

fn event(state: &mut State, code: KeyCode, pressed: bool) {
    state.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(state: &mut State, code: KeyCode) {
    event(state, code, true);
    event(state, code, false);
}
fn setup(offset: f64, rate: f64) -> (tempfile::TempDir, State, u64) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("music.wav"), b"command-only fixture").unwrap();
    fs::write(directory.path().join("song.ssc"), format!("#VERSION:0.83;#MUSIC:music.wav;#OFFSET:{offset};#BPMS:0=120;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;")).unwrap();
    fs::write(
        directory.path().join("preferences.json"),
        format!("{{\"Version\":1,\"Audio\":{{\"MusicRate\":{rate}}}}}"),
    )
    .unwrap();
    let mut state = State::open(&directory.path().join("song.ssc"), 0).unwrap();
    state
        .configure_audio_preferences(&directory.path().join("preferences.json"))
        .unwrap();
    let owner = publish_audio_fixture(&mut state);
    press(&mut state, KeyCode::Enter);
    state.take_audio_commands();
    state.bind_decoder_generation(Some(1));
    (directory, state, owner)
}
fn update(
    state: &mut State,
    generation: u64,
    phase: Option<Phase>,
    stream: f32,
    music: Option<f64>,
    rate: f32,
) {
    state.set_decoder_status(
        generation,
        phase.map(|phase| Ack {
            generation,
            phase,
            opened: !matches!(phase, Phase::Pending | Phase::Failed),
        }),
    );
    let mut snapshot = GameplayAudioSnapshot::default();
    snapshot.stream_clock.valid_at = Instant::now();
    snapshot.stream_clock.stream_seconds = stream;
    snapshot.stream_clock.music_seconds_per_second = rate;
    snapshot.stream_clock.has_music_mapping = music.is_some();
    snapshot.stream_clock.music_nanos = (music.unwrap_or_default() * 1e9).round() as i64;
    state.update_audio(snapshot);
}
#[test]
fn matched_mapped_audio_and_silent_eof_preserve_fractional_pause_resume() {
    let (_dir, mut state, owner) = setup(0.0, 2.0);
    update(
        &mut state,
        1,
        Some(Phase::Exhausted),
        10.0,
        Some(0.126),
        2.0,
    );
    assert_eq!(state.workspace().row(), 12);
    // Unmapped backend snapshots use the editor rate, not their default 1x slope.
    update(&mut state, 1, Some(Phase::Exhausted), 10.25, None, 1.0);
    assert!(state.is_playing());
    assert_eq!(state.workspace().row(), 60);
    press(&mut state, KeyCode::Enter);
    state.take_audio_commands();
    press(&mut state, KeyCode::Enter);
    let commands = state.take_audio_commands();
    assert!(
        matches!(&commands[0], GameplayAudioCommand::EditorResumeMusic {owner:actual,seconds,..} if *actual==owner && (*seconds-0.626).abs()<1e-10)
    );
    assert!(!state.workspace().file().session.is_dirty());
}
#[test]
fn beyond_eof_waits_for_ack_and_counts_only_acknowledged_stream_time_at_two_x() {
    let (_dir, mut state, _owner) = setup(-20.0, 2.0);
    update(&mut state, 1, None, 1.0, None, 1.0);
    update(&mut state, 1, Some(Phase::Pending), 50.0, None, 1.0);
    assert_eq!(state.workspace().row(), 0);
    update(&mut state, 1, Some(Phase::Exhausted), 100.0, None, 1.0);
    assert_eq!(state.workspace().row(), 0);
    update(&mut state, 1, Some(Phase::Exhausted), 100.5, None, 1.0);
    assert_eq!(state.workspace().row(), 96);
    assert!(state.is_playing());
}
#[test]
fn stale_generation_and_failed_decoder_cannot_drive_or_leave_running_transport() {
    let (_dir, mut state, _owner) = setup(0.0, 1.0);
    update(&mut state, 99, Some(Phase::Exhausted), 1.0, Some(50.0), 1.0);
    assert_eq!(state.workspace().row(), 0);
    update(&mut state, 1, Some(Phase::Failed), 2.0, None, 1.0);
    assert!(!state.is_playing());
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let message = i18n::tr("Edition", "PlaybackDecoderFailed");
    assert!(actors.iter().any(|actor| matches!(actor, deadlib_present::actors::Actor::Text {content,..} if content.contains(message.as_ref()))));
    let (_dir, mut unavailable, _owner) = setup(0.0, 1.0);
    unavailable.bind_decoder_generation(None);
    assert!(!unavailable.is_playing());
}
#[test]
fn live_rate_rebinding_reanchors_exact_position_and_rejects_previous_generation() {
    let (_dir, mut state, owner) = setup(0.0, 1.0);
    update(&mut state, 1, Some(Phase::Producing), 1.0, Some(0.126), 1.0);
    event(&mut state, KeyCode::ShiftLeft, true);
    press(&mut state, KeyCode::ArrowRight);
    event(&mut state, KeyCode::ShiftLeft, false);
    assert!(
        state
            .take_audio_commands()
            .iter()
            .any(|command| matches!(command, GameplayAudioCommand::EditorSetMusicRate{owner:actual,..} if *actual==owner))
    );
    state.bind_decoder_generation(Some(2));
    update(&mut state, 1, Some(Phase::Exhausted), 99.0, None, 1.0);
    assert_eq!(state.workspace().row(), 12);
    update(&mut state, 2, Some(Phase::Exhausted), 2.0, None, 1.0);
    assert_eq!(state.workspace().row(), 12);
    update(&mut state, 2, Some(Phase::Exhausted), 2.5, None, 1.0);
    assert_eq!(state.workspace().row(), 64);
}

#[test]
fn stripped_seed_position_does_not_claim_decoder_success_while_missing_or_pending() {
    let (_dir, mut state, _owner) = setup(0.0, 1.0);
    for phase in [None, Some(Phase::Pending)] {
        state.set_decoder_status(
            1,
            phase.map(|phase| Ack {
                generation: 1,
                phase,
                opened: false,
            }),
        );
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.stream_clock.valid_at = Instant::now();
        snapshot.stream_clock.stream_seconds = 50.0;
        // The native adapter strips has_music_mapping when the backend only
        // supplies a seed. Leftover seeded position bytes are not real PCM proof.
        snapshot.stream_clock.has_music_mapping = false;
        snapshot.stream_clock.music_nanos = 50_000_000_000;
        state.update_audio(snapshot);
        assert_eq!(state.workspace().row(), 0);
        assert!(
            state.is_playing(),
            "pending stays requested, without moving"
        );
    }
    update(&mut state, 1, Some(Phase::Failed), 51.0, None, 1.0);
    assert!(!state.is_playing());
    assert_eq!(state.workspace().row(), 0);
}

fn long_clock(state: &mut State, mapped: bool) -> GameplayAudioSnapshot {
    state.set_decoder_status(
        1,
        Some(Ack {
            generation: 1,
            phase: Phase::Exhausted,
            opened: true,
        }),
    );
    let mut snapshot = GameplayAudioSnapshot::default();
    snapshot.stream_clock.valid_at = Instant::now();
    snapshot.stream_clock.stream_seconds = 86_400.0;
    snapshot.stream_clock.has_music_mapping = mapped;
    snapshot.stream_clock.music_nanos = 9_000_000;
    snapshot
}

#[test]
fn precise_editor_stream_time_keeps_sub_float_quantum_motion_after_long_uptime() {
    let (_dir, mut state, _owner) = setup(0.0, 1.0);
    let first = long_clock(&mut state, true);
    state.set_decoder_stream_time(1, first.stream_clock.valid_at, 86_400.0);
    state.update_audio(first);
    assert_eq!(state.workspace().row(), 0);
    let next = long_clock(&mut state, false);
    // At one day the f32 field cannot distinguish these two clock positions.
    assert_eq!(86_400.002_f64 as f32, next.stream_clock.stream_seconds);
    state.set_decoder_stream_time(1, next.stream_clock.valid_at, 86_400.002);
    state.update_audio(next);
    assert_eq!(
        state.workspace().row(),
        1,
        "2 ms crosses the next note row from the exact mapped position"
    );
}

#[test]
fn precise_stream_side_channel_rejects_other_generation_stamp_and_nonfinite_values() {
    use std::time::Duration;
    for mode in 0..4 {
        let (_dir, mut state, _owner) = setup(0.0, 1.0);
        let first = long_clock(&mut state, true);
        state.set_decoder_stream_time(1, first.stream_clock.valid_at, 86_400.0);
        state.update_audio(first);
        let next = long_clock(&mut state, false);
        match mode {
            0 => state.set_decoder_stream_time(99, next.stream_clock.valid_at, 86_401.0),
            1 => state.set_decoder_stream_time(
                1,
                next.stream_clock.valid_at - Duration::from_millis(1),
                86_401.0,
            ),
            2 => state.set_decoder_stream_time(1, next.stream_clock.valid_at, f64::NAN),
            _ => state.set_decoder_stream_time(1, next.stream_clock.valid_at, f64::INFINITY),
        }
        state.update_audio(next);
        assert_eq!(
            state.workspace().row(),
            0,
            "bad f64 side channel falls back to unchanged legacy clock"
        );
        assert!(state.is_playing());
    }
}

#[test]
fn silent_continuation_preserves_backend_float_slope_at_nonbinary_rates() {
    for nominal in [1.1_f64, 0.8_f64] {
        let (_dir, mut state, owner) = setup(0.0, nominal);
        let backend_rate = nominal as f32;
        update(
            &mut state,
            1,
            Some(Phase::Producing),
            1.0,
            Some(0.126),
            backend_rate,
        );
        update(&mut state, 1, Some(Phase::Exhausted), 1.5, None, 1.0);
        let expected = 0.126 + 0.5 * f64::from(backend_rate);
        assert_eq!(state.workspace().row(), (expected * 96.0).floor() as u64);
        press(&mut state, KeyCode::Enter);
        state.take_audio_commands();
        press(&mut state, KeyCode::Enter);
        assert!(
            matches!(&state.take_audio_commands()[0], GameplayAudioCommand::EditorResumeMusic {owner:actual,seconds,..} if *actual==owner && (*seconds-expected).abs()<1e-10)
        );
    }
}

fn publish_audio_fixture(state: &mut State) -> u64 {
    let commands = state.take_audio_commands();
    let owners: Vec<_> = commands
        .iter()
        .filter_map(|c| match c {
            deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio { owner, .. } => Some(*owner),
            _ => None,
        })
        .collect();
    assert_eq!(owners.len(), 1, "one published lazy session");
    assert_ne!(owners[0], 0);
    let mut snapshot = GameplayAudioSnapshot::default();
    let paths = state.editor_asset_paths();
    snapshot.editor.assets = deadsync_gameplay::EditorAssetReadiness {
        owner: owners[0],
        registration: state.editor_asset_registration(),
        main: paths[0].is_some(),
        dedicated_preview: paths[1].is_some(),
    };
    state.update_audio(snapshot);
    assert!(
        !state.take_audio_commands().iter().any(|c| matches!(
            c,
            deadsync_gameplay::GameplayAudioCommand::EditorInitializeOutput { .. }
        )),
        "unavailable command-only host needs no DSP request"
    );
    owners[0]
}
