//! Native shortcuts emit the actual gameplay audio commands; snapshots drive
//! cursor movement. Synthetic tests do not claim hardware/audio-device output.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot};
use deadsync_theme_simply_love::screens::edition::State;
use std::{
    fs,
    time::{Duration, Instant},
};
const SONG: &str = "#VERSION:0.83;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#MUSIC:music.ogg;#OFFSET:0.25;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0000\n0000\n0000;";
fn key(state: &mut State, code: KeyCode) -> bool {
    state.raw_key(&RawKeyboardEvent {
        code,
        pressed: true,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    })
}
fn setup() -> (tempfile::TempDir, State) {
    deadsync_theme_simply_love::i18n::init_for_tests();
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("music.ogg"), b"fixture asset").unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let state = State::open(&path, 0).unwrap();
    (dir, state)
}
fn snapshot(seconds: f64) -> GameplayAudioSnapshot {
    let mut snapshot = GameplayAudioSnapshot::default();
    snapshot.stream_clock.valid_at = Instant::now();
    snapshot.stream_clock.has_music_mapping = true;
    snapshot.stream_clock.music_nanos = (seconds * 1e9).round() as i64;
    snapshot
}
#[test]
fn enter_plays_real_asset_and_mapped_audio_moves_cursor_then_enter_stops() {
    let (dir, mut state) = setup();
    assert!(!key(&mut state, KeyCode::Enter));
    assert!(state.is_playing());
    let commands = state.take_audio_commands();
    assert_eq!(commands.len(), 1);
    match &commands[0] {
        GameplayAudioCommand::PlayMusic {
            path,
            cut,
            looping,
            rate,
        } => {
            assert_eq!(path, &dir.path().join("music.ogg").canonicalize().unwrap());
            assert_eq!(cut.start_sec, -0.25);
            assert!(!looping);
            assert_eq!(*rate, 1.0);
        }
        other => panic!("unexpected command: {other:?}"),
    }
    assert!(state.take_audio_commands().is_empty());
    state.update_audio(snapshot(0.75));
    assert_eq!(state.workspace().row(), 96);
    assert!(!state.workspace().file().session.is_dirty());
    key(&mut state, KeyCode::Enter);
    assert!(!state.is_playing());
    assert_eq!(
        state.take_audio_commands(),
        vec![GameplayAudioCommand::StopMusic]
    );
    state.update_audio(snapshot(9.0));
    assert_eq!(state.workspace().row(), 96);
    key(&mut state, KeyCode::Enter);
    match &state.take_audio_commands()[0] {
        GameplayAudioCommand::PlayMusic { cut, .. } => assert_eq!(cut.start_sec, 0.75),
        other => panic!("unexpected resume: {other:?}"),
    }
}
#[test]
fn stale_unmapped_and_negative_audio_do_not_invent_cursor_progress() {
    let (_dir, mut state) = setup();
    let mut stale = snapshot(9.0);
    stale.stream_clock.valid_at = Instant::now() - Duration::from_secs(1);
    key(&mut state, KeyCode::Enter);
    state.take_audio_commands();
    state.update_audio(stale);
    assert_eq!(state.workspace().row(), 0);
    state.update_audio(GameplayAudioSnapshot::default());
    assert_eq!(state.workspace().row(), 0);
    assert!(state.is_playing());
    state.update_audio(snapshot(-0.5));
    assert_eq!(state.workspace().row(), 0);
    state.update_audio(snapshot(0.25));
    assert_eq!(state.workspace().row(), 48);
    state.update_audio(GameplayAudioSnapshot::default());
    assert!(!state.is_playing());
    assert_eq!(
        state.take_audio_commands(),
        vec![GameplayAudioCommand::StopMusic]
    );
}
#[test]
fn editing_stops_transport_before_mutating_and_close_stops_without_dirtying_file() {
    let (_dir, mut state) = setup();
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Digit2);
    let commands = state.take_audio_commands();
    assert!(matches!(
        commands.as_slice(),
        [
            GameplayAudioCommand::PlayMusic { .. },
            GameplayAudioCommand::StopMusic
        ]
    ));
    assert!(!state.is_playing());
    assert!(state.workspace().file().session.is_dirty());
    let (_dir, mut clean) = setup();
    key(&mut clean, KeyCode::Enter);
    clean.take_audio_commands();
    assert!(key(&mut clean, KeyCode::Escape));
    assert!(!clean.is_playing());
    assert_eq!(
        clean.take_audio_commands(),
        vec![GameplayAudioCommand::StopMusic]
    );
    assert!(!clean.workspace().file().session.is_dirty());
}
#[test]
fn missing_audio_emits_no_play_and_space_still_selects() {
    let (dir, mut state) = setup();
    fs::remove_file(dir.path().join("music.ogg")).unwrap();
    key(&mut state, KeyCode::Enter);
    assert!(!state.is_playing());
    assert!(state.take_audio_commands().is_empty());
    key(&mut state, KeyCode::Space);
    key(&mut state, KeyCode::ArrowDown);
    assert_eq!(state.workspace().selection(), Some((0, 48)));
    key(&mut state, KeyCode::Space);
    assert_eq!(state.workspace().selection(), None);
}

#[test]
fn f11_sync_apply_changes_audio_cut_and_supports_undo_save_reopen() {
    let (dir, mut state) = setup();
    key(&mut state, KeyCode::Enter);
    state.take_audio_commands();
    key(&mut state, KeyCode::F11);
    assert!(!state.is_playing());
    assert_eq!(
        state.take_audio_commands(),
        vec![GameplayAudioCommand::StopMusic]
    );
    key(&mut state, KeyCode::ArrowRight);
    key(&mut state, KeyCode::ArrowDown);
    key(&mut state, KeyCode::Enter);
    assert_eq!(
        deadsync_editor::song_properties::sync_offset(state.workspace().file().session.document())
            .unwrap(),
        0.001
    );
    assert!(state.workspace().file().session.is_dirty());
    key(&mut state, KeyCode::NumpadEnter);
    match &state.take_audio_commands()[0] {
        GameplayAudioCommand::PlayMusic { cut, .. } => assert_eq!(cut.start_sec, -0.25 + 0.001),
        other => panic!("unexpected audio command {other:?}"),
    }
    fn ctrl(state: &mut State, code: KeyCode) {
        key(state, KeyCode::ControlLeft);
        key(state, code);
        state.raw_key(&RawKeyboardEvent {
            code: KeyCode::ControlLeft,
            pressed: false,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
    ctrl(&mut state, KeyCode::KeyZ);
    assert!(!state.is_playing());
    assert!(!state.workspace().file().session.is_dirty());
    ctrl(&mut state, KeyCode::KeyY);
    ctrl(&mut state, KeyCode::KeyS);
    let reopened = State::open(&dir.path().join("song.ssc"), 0).unwrap();
    assert_eq!(
        deadsync_editor::song_properties::sync_offset(
            reopened.workspace().file().session.document()
        )
        .unwrap(),
        0.001
    );
    assert_eq!(
        reopened
            .workspace()
            .file()
            .session
            .document()
            .text(None, "OFFSET")
            .unwrap(),
        "0.25"
    );
}
#[test]
fn f11_cancel_and_unchanged_apply_leave_document_clean() {
    let (_dir, mut state) = setup();
    let bytes = state.workspace().file().session.serialize().unwrap();
    key(&mut state, KeyCode::F11);
    key(&mut state, KeyCode::ArrowRight);
    key(&mut state, KeyCode::Escape);
    assert_eq!(state.workspace().file().session.serialize().unwrap(), bytes);
    key(&mut state, KeyCode::F11);
    key(&mut state, KeyCode::ArrowDown);
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.workspace().file().session.serialize().unwrap(), bytes);
    assert!(!state.workspace().file().session.is_dirty());
}
