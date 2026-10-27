use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
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
fn open(state: &mut State) {
    event(state, KeyCode::SuperLeft, true);
    press(state, KeyCode::F12);
    event(state, KeyCode::SuperLeft, false);
}
fn setup() -> (tempfile::TempDir, State, std::path::PathBuf, Vec<u8>) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path().canonicalize().unwrap();
    fs::create_dir(root_path.join("old")).unwrap();
    fs::create_dir(root_path.join("nouveau été")).unwrap();
    let song = root_path.join("old/song.ssc");
    let bytes=b"#VERSION:0.83;#TITLE:Keep;#MUSIC:music.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0000\n0000\n0000;".to_vec();
    fs::write(&song, &bytes).unwrap();
    fs::write(root_path.join("old/music.wav"), b"command-only asset").unwrap();
    let state = State::open(&song, 0).unwrap();
    let destination = root_path.join("nouveau été/copîe.ssc");
    (root, state, destination, bytes)
}
fn type_destination(state: &mut State, path: &std::path::Path) {
    assert!(state.wants_text_input());
    assert!(state.text_input(path.to_str().unwrap()));
}
fn confirm(state: &mut State) {
    press(state, KeyCode::ArrowUp);
    press(state, KeyCode::Enter);
}
#[test]
fn unicode_ime_destination_review_and_cancel_never_write_or_close() {
    let (_root, mut state, destination, bytes) = setup();
    let source = state.workspace().file().path().to_owned();
    open(&mut state);
    state.text_preedit("été");
    assert!(state.wants_text_input());
    state.text_preedit("");
    type_destination(&mut state, &destination);
    press(&mut state, KeyCode::Enter);
    assert!(!destination.exists());
    assert_eq!(state.workspace().file().path(), source);
    press(&mut state, KeyCode::Escape);
    assert!(!destination.exists());
    assert_eq!(fs::read(source).unwrap(), bytes);
    assert!(!state.workspace().file().session.is_dirty());
}
#[test]
fn explicit_overwrite_review_publishes_same_session_and_preserves_undo() {
    let (_root, mut state, destination, bytes) = setup();
    let source = state.workspace().file().path().to_owned();
    fs::write(&destination, b"reviewed existing target").unwrap();
    press(&mut state, KeyCode::Digit2);
    let ids = state.workspace().file().session.chart_ids().to_vec();
    assert!(state.workspace().file().session.is_dirty());
    open(&mut state);
    type_destination(&mut state, &destination);
    press(&mut state, KeyCode::Enter);
    assert_eq!(fs::read(&destination).unwrap(), b"reviewed existing target");
    confirm(&mut state);
    assert_eq!(state.workspace().file().path(), destination);
    assert_eq!(fs::read(source).unwrap(), bytes);
    assert!(!state.workspace().file().session.is_dirty());
    assert_eq!(state.workspace().file().session.chart_ids(), ids);
    event(&mut state, KeyCode::SuperLeft, true);
    press(&mut state, KeyCode::KeyZ);
    event(&mut state, KeyCode::SuperLeft, false);
    assert!(state.workspace().file().session.is_dirty());
}
#[test]
fn changed_overwrite_target_keeps_working_song_open_and_requires_new_review() {
    let (_root, mut state, destination, bytes) = setup();
    let source = state.workspace().file().path().to_owned();
    fs::write(&destination, b"first baseline").unwrap();
    open(&mut state);
    type_destination(&mut state, &destination);
    press(&mut state, KeyCode::Enter);
    fs::write(&destination, b"new external baseline").unwrap();
    confirm(&mut state);
    assert_eq!(state.workspace().file().path(), source);
    assert_eq!(fs::read(source).unwrap(), bytes);
    assert_eq!(fs::read(destination).unwrap(), b"new external baseline");
    assert!(state.wants_text_input());
}
#[test]
fn cross_format_request_is_rejected_without_implicit_conversion_or_output() {
    let (_root, mut state, destination, _) = setup();
    let different = destination.with_extension("sm");
    open(&mut state);
    type_destination(&mut state, &different);
    press(&mut state, KeyCode::Enter);
    assert!(state.wants_text_input());
    assert!(!different.exists());
}
#[test]
fn save_as_explicitly_stops_chart_transport_and_ignores_old_audio_snapshot() {
    use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot};
    let (_root, mut state, destination, _) = setup();
    press(&mut state, KeyCode::Enter);
    state.take_audio_commands();
    let mut snapshot = GameplayAudioSnapshot::default();
    snapshot.stream_clock.valid_at = Instant::now();
    snapshot.stream_clock.has_music_mapping = true;
    snapshot.stream_clock.music_nanos = 626_000_000;
    state.update_audio(snapshot);
    open(&mut state);
    assert!(!state.is_playing());
    assert_eq!(
        state.take_audio_commands(),
        vec![GameplayAudioCommand::StopMusic]
    );
    let instant = state.chart_focus_time().unwrap();
    type_destination(&mut state, &destination);
    press(&mut state, KeyCode::Enter);
    confirm(&mut state);
    state.update_audio(snapshot);
    assert!(!state.is_playing());
    assert_eq!(
        state.chart_focus_time().unwrap().to_bits(),
        instant.to_bits()
    );
}
