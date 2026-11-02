// Synthetic mapped snapshots and real native key paths; no hardware sound claim.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::{
    Error,
    chart_order::{Orderer, Snapshot},
};
use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, sync::Mutex, time::Instant};
static SERIAL: Mutex<()> = Mutex::new(());
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
fn control(state: &mut State, code: KeyCode) {
    key(state, KeyCode::SuperLeft, true);
    press(state, code);
    key(state, KeyCode::SuperLeft, false);
}
struct Native {
    calls: usize,
}
impl Orderer for Native {
    fn order(&mut self, snapshot: &Snapshot) -> Result<Vec<usize>, Error> {
        self.calls += 1;
        snapshot.indices_with(str::cmp)
    }
}
fn setup(stop: bool) -> (tempfile::TempDir, State, Vec<u8>, u64) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    i18n::set_locale(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(dir.path().join("song.ogg"), b"command-only fixture").unwrap();
    let source=format!("#VERSION:0.83;#BPMS:0=120;#OFFSET:0;#MUSIC:song.ogg;#SAMPLESTART:8;#SAMPLELENGTH:2;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#NOTEDATA:;#STEPSTYPE:dance-single;{}#NOTES:1000\n0100\n0010\n0001;#NOTEDATA:;#STEPSTYPE:dance-double;#BPMS:0=60;#OFFSET:0.25;#NOTES:10000000\n01000000\n00100000\n00010000;",if stop{"#STOPS:1=1;#WARPS:3=2;"}else{""}).into_bytes();
    fs::write(&path, &source).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    let owner = publish_audio_fixture(&mut state);
    (dir, state, source, owner)
}
fn snapshot(seconds: f64) -> GameplayAudioSnapshot {
    let mut audio = GameplayAudioSnapshot::default();
    audio.stream_clock.valid_at = Instant::now();
    audio.stream_clock.has_music_mapping = true;
    audio.stream_clock.music_nanos = (seconds * 1e9).round() as i64;
    audio
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10, "{a} != {b}");
}
fn select_target(state: &mut State, native: &mut Native, list: bool) {
    if list {
        control(state, KeyCode::KeyB);
        assert!(state.process_chart_order(native));
        press(state, KeyCode::End);
        press(state, KeyCode::Enter);
    } else {
        control(state, KeyCode::Tab);
        assert!(state.process_chart_order(native));
    }
    assert_eq!(state.workspace().chart(), 1);
}
#[test]
fn playing_and_paused_same_asset_chart_selection_preserves_transport_or_exact_pause() {
    let _guard = SERIAL.lock().unwrap();
    for paused in [false, true] {
        for list in [false, true] {
            let (dir, mut state, source, owner) = setup(false);
            let mut native = Native { calls: 0 };
            press(&mut state, KeyCode::Enter);
            state.take_audio_commands();
            state.update_audio(snapshot(1.01));
            assert_eq!(state.workspace().row(), 96);
            if paused {
                press(&mut state, KeyCode::Enter);
                assert_eq!(
                    state.take_audio_commands(),
                    vec![GameplayAudioCommand::EditorPauseMusic {
                        owner,
                        seconds: 1.01
                    }]
                );
            }
            select_target(&mut state, &mut native, list);
            assert_eq!(state.is_playing(), !paused);
            assert_eq!(state.workspace().row(), 60);
            close(state.chart_focus_time().unwrap(), 1.01);
            assert!(state.take_audio_commands().is_empty());
            state
                .configure_audio_preferences(&dir.path().join("editor.ini"))
                .unwrap();
            close(state.chart_focus_time().unwrap(), 1.01);
            assert!(state.take_audio_commands().iter().all(|command| !matches!(command,
                GameplayAudioCommand::EditorStopMusic { .. }
                | GameplayAudioCommand::EditorPauseMusic { .. }
                | GameplayAudioCommand::EditorStartMusic { .. }
                | GameplayAudioCommand::EditorResumeMusic { .. }
                | GameplayAudioCommand::EditorPlayMusic { .. })));
            if paused {
                press(&mut state, KeyCode::Enter);
                assert!(
                    matches!(state.take_audio_commands().as_slice(),[GameplayAudioCommand::EditorResumeMusic{owner:actual,seconds,..}]if *actual==owner&&(*seconds-1.01).abs()<1e-10)
                );
            }
            assert_eq!(
                state.workspace().file().session.serialize().unwrap(),
                source
            );
            assert_eq!(fs::read(dir.path().join("song.ssc")).unwrap(), source);
            assert!(!state.workspace().file().session.is_dirty());
        }
    }
}
#[test]
fn stopped_negative_and_inside_stop_anchors_survive_multiple_chart_focus_changes() {
    let _guard = SERIAL.lock().unwrap();
    for time in [-0.25, 0.75, 2.75] {
        let (_dir, mut state, _source, owner) = setup(true);
        let mut native = Native { calls: 0 };
        press(&mut state, KeyCode::Enter);
        state.update_audio(snapshot(time));
        // Exercise a genuinely paused transport; same-asset focus changes now
        // retain playing music instead of stopping it implicitly.
        press(&mut state, KeyCode::Enter);
        assert!(!state.is_playing());
        state.take_audio_commands();
        select_target(&mut state, &mut native, false);
        close(state.chart_focus_time().unwrap(), time);
        control(&mut state, KeyCode::Tab);
        assert!(state.process_chart_order(&mut native));
        assert_eq!(state.workspace().chart(), 0);
        close(state.chart_focus_time().unwrap(), time);
        assert_eq!(native.calls, 1);
        assert!(!state.is_playing());
    }
    let (_dir, mut state, _source, owner) = setup(false);
    let mut native = Native { calls: 0 };
    select_target(&mut state, &mut native, false);
    assert_eq!(state.workspace().row(), 12);
    close(state.chart_focus_time().unwrap(), 0.0);
}
#[test]
fn sample_audition_cannot_drive_focus_and_stale_cursor_drops_navigation_before_native_io() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, _source, owner) = setup(false);
    let mut native = Native { calls: 0 };
    let mut output = GameplayAudioSnapshot::default();
    output.editor.owner = owner;
    output.editor.output.available = true;
    output.editor.output.leased = true;
    output.editor.assets = deadsync_gameplay::EditorAssetReadiness {
        owner,
        registration: state.editor_asset_registration(),
        main: true,
        dedicated_preview: false,
    };
    state.update_audio(output);
    state.take_audio_commands();
    press(&mut state, KeyCode::F11);
    press(&mut state, KeyCode::F6);
    press(&mut state, KeyCode::F4);
    assert!(state.is_playing());
    state.update_audio(snapshot(8.5));
    assert_eq!(state.workspace().row(), 0);
    close(state.chart_focus_time().unwrap(), 0.0);
    press(&mut state, KeyCode::Escape);
    press(&mut state, KeyCode::Escape);
    select_target(&mut state, &mut native, false);
    close(state.chart_focus_time().unwrap(), 0.0);
    control(&mut state, KeyCode::Tab);
    press(&mut state, KeyCode::ArrowDown);
    assert!(!state.process_chart_order(&mut native));
    assert_eq!(native.calls, 1);
    assert_eq!(state.workspace().chart(), 1);
}

#[test]
fn explicit_home_resets_negative_or_fractional_anchor_even_when_integral_cursor_is_unchanged() {
    let _guard = SERIAL.lock().unwrap();
    for time in [-0.25, 0.001] {
        let (_dir, mut state, _source, owner) = setup(false);
        let mut native = Native { calls: 0 };
        press(&mut state, KeyCode::Enter);
        state.update_audio(snapshot(time));
        assert_eq!(state.workspace().row(), 0);
        select_target(&mut state, &mut native, false);
        control(&mut state, KeyCode::Tab);
        assert!(state.process_chart_order(&mut native));
        assert_eq!(state.workspace().row(), 0);
        close(state.chart_focus_time().unwrap(), time);
        // Changing quantization without moving the integral cursor preserves
        // musical focus; it is an editor preference rather than an explicit seek.
        press(&mut state, KeyCode::ArrowRight);
        assert_eq!(state.workspace().row(), 0);
        close(state.chart_focus_time().unwrap(), time);
        press(&mut state, KeyCode::Home);
        assert_eq!(state.workspace().row(), 0);
        close(state.chart_focus_time().unwrap(), 0.0);
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
