include!("support/chart_order_transport.rs");
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::{Actor, SizeSpec},
    space::{Metrics, set_current_metrics},
};
use deadsync_editor::{playback::Clock, zoom::SpacingMode};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

fn init() {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
}
fn key(state: &mut State, code: KeyCode, pressed: bool) {
    state.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        timestamp: Instant::now(),
        repeat: false,
        host_nanos: 0,
    });
}
fn press(state: &mut State, code: KeyCode) {
    key(state, code, true);
    key(state, code, false);
}
fn shortcut(state: &mut State, code: KeyCode) {
    key(state, KeyCode::SuperLeft, true);
    press(state, code);
    key(state, KeyCode::SuperLeft, false);
    if code == KeyCode::Tab { complete_chart_navigation(state); }
}
fn actors(state: &State) -> Vec<Actor> {
    let mut a = Vec::new();
    state.actors(&mut a);
    a
}
fn arrows(state: &State) -> Vec<f32> {
    actors(state)
        .into_iter()
        .filter_map(|a| match a {
            Actor::Sprite {
                z: 8004, offset, ..
            } => Some(offset[1]),
            _ => None,
        })
        .collect()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 0.001, "{a} != {b}");
}
fn height(h: f32, scale: [f32; 2]) -> f32 {
    if h == 0.0 { scale[1] } else { h * scale[1] }
}

#[test]
fn keyboard_time_display_projects_notes_and_holds_with_real_pause_phases() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("time.ssc");
    let source = "#VERSION:0.83;#TITLE:Time;#BPMS:0=240,1=480;#DELAYS:1=0.125;#STOPS:2=0.125;#SPEEDS:0=0=1=0;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:2000\n0100\n0010\n3001;";
    fs::write(&path, source).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    let original = state.workspace().file().session.serialize().unwrap();
    let baseline = arrows(&state);
    assert_eq!(baseline, vec![160.0, 208.0, 256.0, 304.0]);
    shortcut(&mut state, KeyCode::KeyT);
    assert_eq!(
        state.spacing_mode(),
        SpacingMode::ConstantTime,
        "{:?}",
        actors(&state)
            .into_iter()
            .filter_map(|a| match a {
                Actor::Text { content, .. } => Some(content.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(arrows(&state), vec![160.0, 272.5, 310.0, 385.0]);
    let hold = actors(&state)
        .into_iter()
        .find_map(|a| match a {
            Actor::Sprite {
                z: 8003,
                offset,
                size: [_, SizeSpec::Px(h)],
                scale,
                ..
            } => Some((offset[1], height(h, scale))),
            _ => None,
        })
        .unwrap();
    close(hold.0 as f64, 272.5);
    close(hold.1 as f64, 225.0);
    // Mode changes are presentation-only; zero SPEEDS does not collapse time display.
    assert_eq!(state.workspace().row(), 0);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original
    );
    assert!(!state.workspace().file().session.is_dirty());
    shortcut(&mut state, KeyCode::ArrowUp);
    state.update_with_time(Instant::now() + Duration::from_secs(1));
    close(state.spacing_zoom(), 1.2);
    assert_eq!(arrows(&state), vec![160.0, 295.0, 340.0]);
    shortcut(&mut state, KeyCode::Digit0);
    shortcut(&mut state, KeyCode::KeyT);
    assert_eq!(arrows(&state), baseline);
    assert_eq!(fs::read(&path).unwrap(), source.as_bytes());
}

fn wav(path: &Path) {
    let pcm: Vec<i16> = (0..48000)
        .map(|i| if (7104..7296).contains(&i) { 30000 } else { 0 })
        .collect();
    let data = pcm.len() as u32 * 2;
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((36 + data).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(48000u32.to_le_bytes());
    bytes.extend(96000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(data.to_le_bytes());
    for v in pcm {
        bytes.extend(v.to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}
#[test]
fn sample_and_decoded_waveform_keep_time_inside_a_delay_instead_of_collapsing_to_a_row() {
    init();
    let dir = tempfile::tempdir().unwrap();
    wav(&dir.path().join("pulse.wav"));
    let path = dir.path().join("delay.ssc");
    let source = "#VERSION:0.83;#TITLE:Delay;#BPMS:0=120;#DELAYS:0=0.5;#OFFSET:0;#MUSIC:pulse.wav;#SAMPLESTART:0.1;#SAMPLELENGTH:0.1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    let original = state.workspace().file().session.serialize().unwrap();
    shortcut(&mut state, KeyCode::KeyT);
    press(&mut state, KeyCode::KeyW);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !state.waveform_ready() {
        state.update();
        assert!(Instant::now() < deadline, "waveform worker did not finish");
        thread::sleep(Duration::from_millis(1));
    }
    let clock = Clock::capture(&state.workspace().file().session, 0).unwrap();
    close(arrows(&state)[0] as f64, 310.0);
    let sample_region = |state: &State| {
        actors(state)
            .into_iter()
            .find_map(|a| match a {
                Actor::Sprite {
                    tint,
                    offset,
                    size: [_, SizeSpec::Px(h)],
                    scale,
                    ..
                } if tint == [0.2, 0.85, 0.65, 0.14] => Some((offset[1], height(h, scale))),
                _ => None,
            })
            .expect("sample inside the delay remains visible")
    };
    let sample = sample_region(&state);
    close(
        sample.0 as f64,
        160.0 + clock.music_to_chart_time(0.15).unwrap() * 300.0,
    );
    close(sample.1 as f64, 30.0);
    let peak_positions = |state: &State| {
        let palette = state.waveform_preferences().palette();
        actors(state)
            .into_iter()
            .filter_map(|a| match a {
                Actor::Sprite {
                    tint,
                    offset,
                    size: [SizeSpec::Px(w), _],
                    scale,
                    ..
                } if tint == palette.resolved_sparse().rgba() || tint == palette.dense().rgba() => {
                    let width = if w == 0.0 { scale[0] } else { w * scale[0] };
                    (width > 10.0).then_some(offset[1])
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let peaks = peak_positions(&state);
    let peak_y = 160.0 + clock.music_to_chart_time(0.15).unwrap() * 300.0;
    assert!(
        !peaks.is_empty(),
        "decoded pulse must be drawn inside the delay"
    );
    assert!(
        peaks.iter().all(|y| (f64::from(*y) - peak_y).abs() < 4.0),
        "{peaks:?}, expected {peak_y}"
    );
    // Staged offsets move both temporal overlays; discarding restores both.
    press(&mut state, KeyCode::F11);
    for _ in 0..4 {
        press(&mut state, KeyCode::ArrowDown);
    }
    for _ in 0..100 {
        press(&mut state, KeyCode::ArrowRight);
    }
    close(sample_region(&state).0 as f64, sample.0 as f64 + 30.0);
    let moved = peak_positions(&state);
    assert!(!moved.is_empty());
    assert!(
        moved
            .iter()
            .all(|y| (f64::from(*y) - peak_y - 30.0).abs() < 4.0)
    );
    press(&mut state, KeyCode::Escape);
    close(sample_region(&state).0 as f64, sample.0 as f64);
    assert!(
        peak_positions(&state)
            .iter()
            .all(|y| (f64::from(*y) - peak_y).abs() < 4.0)
    );
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original
    );
    assert_eq!(fs::read(&path).unwrap(), source.as_bytes());
}

#[test]
fn chart_switch_refreshes_time_geometry_and_panels_keep_shortcut_focus() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("charts.ssc");
    fs::write(&path,"#VERSION:0.83;#BPMS:0=240;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=480;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let mut state = State::open(&path, 0).unwrap();
    shortcut(&mut state, KeyCode::KeyT);
    close(arrows(&state)[1] as f64, 235.0);
    shortcut(&mut state, KeyCode::Tab);
    assert_eq!(state.workspace().chart(), 1);
    close(arrows(&state)[1] as f64, 197.5);
    press(&mut state, KeyCode::F1);
    shortcut(&mut state, KeyCode::KeyT);
    assert_eq!(state.spacing_mode(), SpacingMode::ConstantTime);
    press(&mut state, KeyCode::Escape);
    shortcut(&mut state, KeyCode::KeyT);
    assert_eq!(state.spacing_mode(), SpacingMode::ConstantRow);
    close(arrows(&state)[1] as f64, 208.0);
}

#[test]
fn time_display_keeps_notes_collapsed_by_warp_and_rejects_invalid_timing_cleanly() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("warp.ssc");
    fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#WARPS:0=2;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let mut state = State::open(&path, 0).unwrap();
    shortcut(&mut state, KeyCode::KeyT);
    assert_eq!(arrows(&state), vec![160.0, 160.0, 160.0, 310.0]);
    let invalid = dir.path().join("invalid.ssc");
    let source = "#VERSION:0.83;#BPMS:0=120;#SPEEDS:0=nan=1=0;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&invalid, source).unwrap();
    let mut state = State::open(&invalid, 0).unwrap();
    shortcut(&mut state, KeyCode::KeyT);
    assert_eq!(state.spacing_mode(), SpacingMode::ConstantRow);
    assert!(actors(&state).into_iter().any(|a| matches!(a,Actor::Text { content, .. } if content.contains("Speed values must be finite"))));
    assert!(!state.workspace().file().session.is_dirty());
    assert_eq!(fs::read(&invalid).unwrap(), source.as_bytes());
}

#[test]
fn applied_tempo_and_undo_redo_rebuild_cached_time_geometry() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tempo.ssc");
    let source = "#VERSION:0.83;#BPMS:0=240;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;";
    fs::write(&path, source).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    shortcut(&mut state, KeyCode::KeyT);
    close(arrows(&state)[1] as f64, 235.0);
    press(&mut state, KeyCode::F10);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Delete);
    for code in [KeyCode::Digit4, KeyCode::Digit8, KeyCode::Digit0] {
        press(&mut state, code);
    }
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    for _ in 0..5 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    close(arrows(&state)[1] as f64, 197.5);
    shortcut(&mut state, KeyCode::KeyZ);
    close(arrows(&state)[1] as f64, 235.0);
    key(&mut state, KeyCode::ShiftLeft, true);
    shortcut(&mut state, KeyCode::KeyZ);
    key(&mut state, KeyCode::ShiftLeft, false);
    close(arrows(&state)[1] as f64, 197.5);
    assert_eq!(fs::read(&path).unwrap(), source.as_bytes());
}
