//! Keyboard-only Snap Off workflows through the native editor bridge.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, sync::Mutex, time::Instant};
static SERIAL: Mutex<()> = Mutex::new(());
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
fn open(signature: &str) -> (tempfile::TempDir, State, Vec<u8>) {
    i18n::init_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snap.ssc");
    fs::write(dir.path().join("music.wav"), b"command-only").unwrap();
    let source=format!("#VERSION:0.83;#MUSIC:music.wav;#OFFSET:0;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#BPMS:0=120;#TIMESIGNATURES:{signature};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000\n0000\n0000\n0000;").into_bytes();
    fs::write(&path, &source).unwrap();
    let state = State::open(&path, 0).unwrap();
    (dir, state, source)
}
#[test]
fn off_cycles_preserve_cursor_and_navigation_uses_departure_signature() {
    let _g = SERIAL.lock().unwrap();
    let (_d, mut state, source) = open("0=4=4,1=3=8");
    press(&mut state, KeyCode::ArrowDown);
    assert_eq!(state.workspace().row(), 48);
    press(&mut state, KeyCode::ArrowLeft);
    assert_eq!(state.workspace().snap(), 9);
    assert_eq!(state.workspace().row(), 48);
    press(&mut state, KeyCode::ArrowDown);
    assert_eq!(state.workspace().row(), 120);
    press(&mut state, KeyCode::ArrowUp);
    assert_eq!(state.workspace().row(), 48);
    press(&mut state, KeyCode::ArrowRight);
    assert_eq!(state.workspace().snap(), 0);
    assert_eq!(state.workspace().row(), 48);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
}
#[test]
fn off_press_release_and_held_cursor_movement_form_one_atomic_edit() {
    let _g = SERIAL.lock().unwrap();
    let (_d, mut state, source) = open("0=3=8");
    press(&mut state, KeyCode::ArrowLeft);
    key(&mut state, KeyCode::Digit1, true);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
    press(&mut state, KeyCode::ArrowDown);
    assert_eq!(state.workspace().row(), 72);
    key(&mut state, KeyCode::Digit1, false);
    let grid = state.workspace().grid();
    let note = grid
        .get(deadsync_editor::notes::Position {
            row: 0,
            lane: 0,
            player: 0,
        })
        .unwrap();
    assert_eq!(note.end, Some(72));
    key(&mut state, KeyCode::SuperLeft, true);
    press(&mut state, KeyCode::KeyZ);
    key(&mut state, KeyCode::SuperLeft, false);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
}
#[test]
fn off_persists_source_index_and_preserves_unknown_application_preferences() {
    let _g = SERIAL.lock().unwrap();
    let (dir, mut state, _) = open("0=4=4");
    let path = dir.path().join("prefs.json");
    fs::write(
        &path,
        b"{\"Version\":1,\"SnapIndex\":1,\"SnapLockIndex\":0,\"Vendor\":42}",
    )
    .unwrap();
    state.configure_audio_preferences(&path).unwrap();
    assert_eq!(state.workspace().snap(), 0);
    press(&mut state, KeyCode::ArrowLeft);
    assert_eq!(state.workspace().snap(), 9);
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["SnapIndex"], 0);
    assert_eq!(json["Vendor"], 42);
    let mut again = State::open(&dir.path().join("snap.ssc"), 0).unwrap();
    again.configure_audio_preferences(&path).unwrap();
    assert_eq!(again.workspace().snap(), 9);
}
#[test]
fn explicit_hold_tool_has_nonzero_duration_when_snap_is_off() {
    let _g = SERIAL.lock().unwrap();
    let (_d, mut state, _) = open("0=4=4");
    press(&mut state, KeyCode::ArrowLeft);
    press(&mut state, KeyCode::Tab);
    press(&mut state, KeyCode::Digit2);
    let grid = state.workspace().grid();
    let note = grid
        .get(deadsync_editor::notes::Position {
            row: 0,
            lane: 1,
            player: 0,
        })
        .unwrap();
    assert_eq!(note.end, Some(1));
}
#[test]
fn conflicting_preference_save_keeps_current_snap_and_cursor() {
    let _guard = SERIAL.lock().unwrap();
    let (dir, mut state, _) = open("0=4=4");
    let path = dir.path().join("prefs.json");
    fs::write(&path, b"{\"Version\":1,\"SnapIndex\":1}").unwrap();
    state.configure_audio_preferences(&path).unwrap();
    press(&mut state, KeyCode::ArrowDown);
    assert_eq!(state.workspace().row(), 48);
    fs::write(&path, b"{\"Version\":1,\"SnapIndex\":2,\"External\":true}").unwrap();
    press(&mut state, KeyCode::ArrowLeft);
    assert_eq!(state.workspace().snap(), 0);
    assert_eq!(state.workspace().row(), 48);
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["SnapIndex"], 2);
    assert_eq!(json["External"], true);
}
#[test]
fn off_during_playback_preserves_fractional_phase_in_seek_command() {
    use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot};
    use deadsync_theme_simply_love::screens::edition::decoder_transport::{Ack, Phase};
    let _guard = SERIAL.lock().unwrap();
    let (dir, mut state, _) = open("0=4=4");
    let path = dir.path().join("prefs.json");
    fs::write(
        &path,
        b"{\"Version\":1,\"SnapIndex\":0,\"Scroll\":{\"StopPlaybackWhenScrolling\":false}}",
    )
    .unwrap();
    state.configure_audio_preferences(&path).unwrap();
    assert_eq!(state.workspace().snap(), 9);
    press(&mut state, KeyCode::Enter);
    assert!(state.is_playing());
    state.bind_decoder_generation(Some(1));
    state.set_decoder_status(
        1,
        Some(Ack {
            generation: 1,
            phase: Phase::Producing,
            opened: true,
        }),
    );
    let time = Instant::now();
    let source_time: f64 = 0.2526041666666667; // row24.25 at120BPM
    let mut snapshot = GameplayAudioSnapshot::default();
    snapshot.stream_clock.valid_at = time;
    snapshot.stream_clock.stream_seconds = source_time as f32;
    snapshot.stream_clock.has_music_mapping = true;
    snapshot.stream_clock.music_nanos = (source_time * 1e9) as i64;
    snapshot.stream_clock.music_seconds_per_second = 1.0;
    state.update_audio(snapshot);
    state.set_decoder_stream_time(1, time, source_time);
    state.update_with_time(time);
    state.take_audio_commands();
    press(&mut state, KeyCode::ArrowDown);
    assert!(state.is_playing());
    let commands = state.take_audio_commands();
    let seek = commands
        .iter()
        .find_map(|c| match c {
            GameplayAudioCommand::SeekMusic(t) => Some(*t),
            _ => None,
        })
        .expect("continued transport seek");
    assert!(
        (seek - (source_time + 2.0)).abs() < 1e-6,
        "Off retains fractional row phase: {seek}"
    );
    assert_eq!(state.workspace().row(), 216);
}
#[test]
fn off_navigation_retains_negative_anchor_while_integer_edit_cursor_clamps() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, source) = open("0=4=4");
    press(&mut state, KeyCode::ArrowLeft);
    press(&mut state, KeyCode::ArrowUp);
    assert_eq!(
        state.workspace().row(),
        0,
        "negative display anchor does not create negative notes"
    );
    press(&mut state, KeyCode::ArrowDown);
    assert_eq!(
        state.workspace().row(),
        0,
        "one measure returns from -192 to0, not to192"
    );
    press(&mut state, KeyCode::ArrowDown);
    assert_eq!(state.workspace().row(), 192);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
}
