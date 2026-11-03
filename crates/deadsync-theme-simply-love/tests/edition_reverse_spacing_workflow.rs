#[path = "support/prepared_audio.rs"]
mod prepared_audio;
#[path = "support/row_geometry.rs"]
mod row_geometry;
#[allow(dead_code)]
#[path = "support/transport_host.rs"]
mod transport_host;
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::{Actor, SizeSpec},
    space::{Metrics, set_current_metrics},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

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
fn shortcut(state: &mut State, modifier: KeyCode, code: KeyCode) {
    key(state, modifier, true);
    press(state, code);
    key(state, modifier, false);
}
fn reverse(state: &mut State) {
    shortcut(state, KeyCode::ShiftLeft, KeyCode::KeyV);
}
fn actors(state: &State) -> Vec<Actor> {
    let mut a = Vec::new();
    state.actors(&mut a);
    for target in state.take_waveform_render_targets() {
        a.extend(target.children.iter().cloned());
    }
    a
}
fn arrows(state: &State) -> Vec<f32> {
    transport_host::heads(state)
        .into_iter()
        .map(|xy| xy[1])
        .collect()
}
fn hold_bounds(state: &State) -> (f64, f64) {
    let bounds: Vec<_> = actors(state)
        .into_iter()
        .filter_map(|actor| match actor {
            Actor::Sprite {
                offset,
                size: [SizeSpec::Px(w), SizeSpec::Px(h)],
                scale,
                uv_rect: Some(uv),
                rot_z_deg,
                cropleft,
                cropright,
                croptop,
                cropbottom,
                ..
            } if (uv[0] - 1.0 / 1280.0).abs() < 1e-6
                && uv[1] >= 457.0 / 1280.0 - 1e-6
                && uv[3] <= 585.0 / 1280.0 + 1e-6 =>
            {
                let (w, h) = (w * scale[0], h * scale[1]);
                let (sin, cos) = rot_z_deg.to_radians().sin_cos();
                // Cropped-quad offsets are applied in world axes before the
                // already-compensated source rotation, as in the compositor.
                let center = offset[1] + (croptop - cropbottom) * h * 0.5;
                let height = sin.abs() * w * (1.0 - cropleft - cropright)
                    + cos.abs() * h * (1.0 - croptop - cropbottom);
                Some((
                    f64::from(center - height * 0.5),
                    f64::from(center + height * 0.5),
                ))
            }
            _ => None,
        })
        .collect();
    assert!(!bounds.is_empty(), "source hold body is visible");
    (
        bounds.iter().map(|b| b.0).fold(f64::INFINITY, f64::min),
        bounds.iter().map(|b| b.1).fold(f64::NEG_INFINITY, f64::max),
    )
}
fn rects(state: &State, tint: [f32; 4]) -> Vec<(f32, f32)> {
    actors(state)
        .into_iter()
        .filter_map(|a| match a {
            Actor::Sprite {
                tint: t,
                offset,
                size: [_, SizeSpec::Px(h)],
                scale,
                ..
            } if t == tint => Some((offset[1], if h == 0.0 { scale[1] } else { h * scale[1] })),
            _ => None,
        })
        .collect()
}
fn peak_positions(state: &State) -> Vec<f32> {
    let p = state.waveform_preferences().palette();
    actors(state)
        .into_iter()
        .filter_map(|a| match a {
            Actor::Sprite {
                tint,
                offset,
                size: [SizeSpec::Px(w), _],
                scale,
                ..
            } if tint == p.resolved_sparse().rgba() || tint == p.dense().rgba() => {
                let w = if w == 0.0 { scale[0] } else { w * scale[0] };
                (w > 10.0).then_some(offset[1])
            }
            _ => None,
        })
        .collect()
}
#[track_caller]
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 0.001, "{a} != {b}");
}
fn wav(path: &Path) {
    let pcm: Vec<i16> = (0..48000)
        .map(|i| {
            if (26304..26496).contains(&i) {
                30000
            } else {
                0
            }
        })
        .collect();
    let data = pcm.len() as u32 * 2;
    let mut b = Vec::new();
    b.extend(b"RIFF");
    b.extend((36 + data).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(48000u32.to_le_bytes());
    b.extend(96000u32.to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend(data.to_le_bytes());
    for v in pcm {
        b.extend(v.to_le_bytes());
    }
    fs::write(path, b).unwrap();
}
fn mirror_rects(a: &[(f32, f32)], b: &[(f32, f32)]) {
    assert!(!a.is_empty());
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(b) {
        close(f64::from(a.0 + b.0), 500.0);
        close(f64::from(a.1), f64::from(b.1));
        assert!(b.1 > 0.0);
    }
}

#[test]
fn reverse_mirrors_chart_geometry_and_restores_upright_widgets_and_waveform_in_all_modes() {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    use deadsync_editor::zoom::SpacingMode;
    for mode in [
        SpacingMode::ConstantRow,
        SpacingMode::ConstantTime,
        SpacingMode::Variable,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("pulse.wav");
        wav(&audio);
        let audio_before = fs::read(&audio).unwrap();
        let source = "#VERSION:0.83;#BPMS:0=240,1=480;#DELAYS:1=0.125;#STOPS:2=0.125;#OFFSET:0;#MUSIC:pulse.wav;#SAMPLESTART:0.5;#SAMPLELENGTH:0.1;#LABELS:1=repere;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:2000\n0100\n0010\n3001;";
        use deadsync_editor::{
            document::{Document, Format},
            generation::metadata::{ChartData, Definition, PATTERN_BALANCED, SIXTEENTHS},
            history::Session,
        };
        use std::{collections::BTreeMap, sync::Arc};
        let mut session = Session::new(
            Document::parse(Arc::<[u8]>::from(source.as_bytes()), Format::Ssc).unwrap(),
        );
        ChartData {
            patterns: Some(BTreeMap::from([(
                48,
                Definition {
                    pattern_config_guid: SIXTEENTHS.parse().unwrap(),
                    performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                    length: 96,
                    random_seed: 0,
                    start_position_inclusive: true,
                    end_position_inclusive: true,
                    ignore_preceding_distribution: true,
                    extra: BTreeMap::new(),
                },
            )])),
            ..ChartData::default()
        }
        .save(&mut session, 0)
        .unwrap();
        let source = session.serialize().unwrap();
        let path = dir.path().join("reverse.ssc");
        fs::write(&path, &source).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        row_geometry::configure(&mut state, dir.path());
        if mode == SpacingMode::ConstantTime {
            shortcut(&mut state, KeyCode::SuperLeft, KeyCode::KeyT);
        }
        if mode == SpacingMode::Variable {
            key(&mut state, KeyCode::SuperLeft, true);
            shortcut(&mut state, KeyCode::ShiftLeft, KeyCode::KeyT);
            key(&mut state, KeyCode::SuperLeft, false);
        }
        press(&mut state, KeyCode::Space);
        press(&mut state, KeyCode::ArrowDown);
        assert_eq!(state.workspace().selection(), Some((0, 48)));
        prepared_audio::publish_main(&mut state);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !state.waveform_ready() {
            state.update();
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        press(&mut state, KeyCode::Enter);
        assert!(state.is_playing());
        let heads = arrows(&state);
        let holds = hold_bounds(&state);
        let selection = rects(&state, [0.2, 0.6, 0.9, 0.2]);
        let sample = rects(&state, [0.2, 0.85, 0.65, 0.14]);
        let patterns = rects(&state, [0.7, 0.4, 1.0, 0.8]);
        let peaks = peak_positions(&state);
        assert!(!peaks.is_empty());
        let label = |state: &State| {
            actors(state)
                .into_iter()
                .find_map(|a| match a {
                    Actor::Text {
                        content,
                        clip: Some([_, y, _, _]), ..
                    } if content.ends_with(" repere") => Some(y),
                    _ => None,
                })
                .expect("label visible")
        };
        let label_y = label(&state);
        let original = state.workspace().file().session.serialize().unwrap();
        reverse(&mut state);
        assert!(state.display_reversed());
        assert!(state.is_playing());
        let reversed_heads = arrows(&state);
        assert_eq!(heads.len(), reversed_heads.len());
        for (a, b) in heads.iter().zip(&reversed_heads) {
            close(f64::from(a + b), 500.0);
        }
        let reversed_hold = hold_bounds(&state);
        close(holds.0 + reversed_hold.1, 500.0);
        close(holds.1 + reversed_hold.0, 500.0);
        mirror_rects(&selection, &rects(&state, [0.2, 0.6, 0.9, 0.2]));
        mirror_rects(&sample, &rects(&state, [0.2, 0.85, 0.65, 0.14]));
        mirror_rects(&patterns, &rects(&state, [0.7, 0.4, 1.0, 0.8]));
        // Text widgets stay upright. Source Layout wraps Delay and Label at
        // the same row, and the shared collision sweep may move their boxes.
        // Those offsets do not mirror as note geometry does. Both directions
        // retain a visible label, and reversing back must restore its position.
        assert!((105.0..395.0).contains(&label_y));
        assert!((105.0..395.0).contains(&label(&state)));
        let reversed_peaks = peak_positions(&state);
        assert!(!reversed_peaks.is_empty());
        let mean = |v: &[f32]| v.iter().map(|x| f64::from(*x)).sum::<f64>() / v.len() as f64;
        assert!((mean(&peaks) + mean(&reversed_peaks) - 500.0).abs() <= 1.0);
        // Mode switches retain Reverse, modal shortcuts retain focus.
        shortcut(&mut state, KeyCode::SuperLeft, KeyCode::KeyT);
        assert!(state.display_reversed());
        shortcut(&mut state, KeyCode::SuperLeft, KeyCode::KeyT);
        if mode == SpacingMode::Variable {
            key(&mut state, KeyCode::SuperLeft, true);
            shortcut(&mut state, KeyCode::ShiftLeft, KeyCode::KeyT);
            key(&mut state, KeyCode::SuperLeft, false);
        }
        assert_eq!(arrows(&state), reversed_heads);
        press(&mut state, KeyCode::F1);
        reverse(&mut state);
        assert!(state.display_reversed());
        press(&mut state, KeyCode::Escape);
        reverse(&mut state);
        assert!(!state.display_reversed());
        assert_eq!(arrows(&state), heads);
        close(f64::from(label(&state)), f64::from(label_y));
        assert_eq!(state.workspace().row(), 48);
        assert_eq!(state.workspace().selection(), Some((0, 48)));
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            original
        );
        assert!(!state.workspace().file().session.is_dirty());
        assert_eq!(fs::read(&path).unwrap(), source);
        assert_eq!(fs::read(&audio).unwrap(), audio_before);
    }
}
