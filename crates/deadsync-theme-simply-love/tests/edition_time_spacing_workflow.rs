include!("support/chart_order_transport.rs");
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::{Actor, SizeSpec, SpriteSource},
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
// These projection fixtures intentionally start in ConstantRow; native source
// preferences now default to ConstantTime.
fn row_state(path: &Path, chart: usize) -> Result<State, deadsync_editor::Error> {
    let mut state = State::open(path, chart)?;
    let preferences = path
        .parent()
        .unwrap()
        .join("projection-test-preferences.json");
    fs::write(&preferences, br#"{"Version":1,"SnapIndex":1,"Scroll":{"RowBasedPixelsPerRow":1},"Waveform":{"ShowWaveForm":true}}"#)?;
    state.configure_audio_preferences(&preferences)?;
    state.configure_spacing_preference(
        deadsync_editor::scroll_preferences::spacing::Preference::ConstantRow,
    );
    Ok(state)
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
                z: 8004, offset, source, uv_rect: Some(uv),
                size: [SizeSpec::Px(width), SizeSpec::Px(height)],
                rot_z_deg, cropleft, cropright, croptop, cropbottom, ..
            } if matches!(&source, SpriteSource::TextureStatic(key) if key.ends_with("editor-grooveauthor/atlas.png"))
                 && (uv[0] - 589.0 / 1280.0).abs() < 1e-6
                 && (uv[1] - 1.0 / 1280.0).abs() < 1e-6 => {
                // Recover the logical source center, before the renderer's
                // compensation for crop displacement in rotated actor axes.
                let delta = [(cropleft - cropright) * width * 0.5,
                    (croptop - cropbottom) * height * 0.5];
                let (sin, cos) = rot_z_deg.rem_euclid(360.0).to_radians().sin_cos();
                Some(offset[1] - (-sin * delta[0] + cos * delta[1] - delta[1]))
            },
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
    let mut state = row_state(&path, 0).unwrap();
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
    // Source holds use tiled atlas bodies with independent rims. Check the
    // complete temporal span instead of mistaking one tile for a solid body.
    let body = actors(&state)
        .into_iter()
        .filter_map(|actor| match actor {
            Actor::Sprite {
                offset,
                size: [_, SizeSpec::Px(h)],
                scale,
                uv_rect: Some(uv),
                source: SpriteSource::TextureStatic(key),
                visible: true,
                ..
            } if key.ends_with("editor-grooveauthor/atlas.png")
                && (uv[0] - 1.0 / 1280.0).abs() < 1e-6
                && uv[1] >= 457.0 / 1280.0
                && uv[3] <= 585.0 / 1280.0 =>
            {
                let h = height(h, scale);
                Some((offset[1] - h * 0.5, offset[1] + h * 0.5))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(body.len() > 1, "source hold is tiled");
    close(
        body.iter()
            .map(|range| range.0)
            .fold(f32::INFINITY, f32::min) as f64,
        160.0,
    );
    close(
        body.iter()
            .map(|range| range.1)
            .fold(f32::NEG_INFINITY, f32::max) as f64,
        385.0,
    );
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
    let mut state = row_state(&path, 0).unwrap();
    let original = state.workspace().file().session.serialize().unwrap();
    shortcut(&mut state, KeyCode::KeyT);
    state.take_audio_commands();
    let prepared_source = deadlib_audio_decode::prepared::PreparedAudioSource::load_editor(&dir.path().join("pulse.wav"),
        deadlib_audio_decode::DecodeOptions::STEP_MANIA_TIMELINE,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    let registration = state.editor_asset_registration();
    state.observe_editor_main_source(
        state.editor_asset_owner(),
        registration,
        Some((registration, 0)),
        Some(prepared_source),
    );
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
        let rendered = actors(state);
        let display_width = rendered
            .iter()
            .find_map(|actor| match actor {
                Actor::Sprite {
                    source: SpriteSource::RenderTarget { .. },
                    z: 8001,
                    size: [SizeSpec::Px(width), _],
                    scale,
                    ..
                } => Some(width * scale[0]),
                _ => None,
            })
            .expect("waveform has a visible composite");
        let targets = state.take_waveform_render_targets();
        let source = targets
            .iter()
            .find(|target| target.postprocess.is_none())
            .expect("waveform source pass exists before FXAA");
        let ratio = display_width / source.logical_size[0];
        source
            .children
            .iter()
            .filter_map(|actor| match actor {
                Actor::Sprite {
                    tint,
                    offset,
                    size: [SizeSpec::Px(width), _],
                    scale,
                    ..
                } if *tint == palette.resolved_sparse().rgba().map(|v| v.clamp(0.0, 1.0))
                    || *tint == palette.dense().rgba().map(|v| v.clamp(0.0, 1.0)) => {
                    let width = if *width == 0.0 { scale[0] } else {
                        width * scale[0] } * ratio;
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
    let mut state = row_state(&path, 0).unwrap();
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
    let mut state = row_state(&path, 0).unwrap();
    shortcut(&mut state, KeyCode::KeyT);
    assert_eq!(arrows(&state), vec![160.0, 160.0, 160.0, 310.0]);
    let invalid = dir.path().join("invalid.ssc");
    let source = "#VERSION:0.83;#BPMS:0=120;#SPEEDS:0=nan=1=0;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&invalid, source).unwrap();
    let mut state = row_state(&invalid, 0).unwrap();
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
    let mut state = row_state(&path, 0).unwrap();
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
