#[path = "support/source_sprite_geometry.rs"]
mod source_sprite_geometry;
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::{Actor, SizeSpec},
    space::{Metrics, set_current_metrics},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    time::{Duration, Instant},
};

fn key(state: &mut State, code: KeyCode, pressed: bool, timestamp: Instant) {
    state.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        timestamp,
        repeat: false,
        host_nanos: 0,
    });
}
fn shortcut(state: &mut State, code: KeyCode, time: Instant) {
    key(state, KeyCode::SuperLeft, true, time);
    key(state, code, true, time);
    key(state, code, false, time);
    key(state, KeyCode::SuperLeft, false, time);
}
fn setup() -> (tempfile::TempDir, State) {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("zoom.ssc");
    fs::write(&path,"#VERSION:0.83;#TITLE:Zoom;#MUSIC:test.wav;#SAMPLESTART:0.5;#SAMPLELENGTH:0.5;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let mut state = State::open(&path, 0).unwrap();
    // This fixture exercises row-space geometry independently of the user's
    // default time-space mode.
    shortcut(&mut state, KeyCode::KeyT, Instant::now());
    assert_eq!(
        state.spacing_mode(),
        deadsync_editor::zoom::SpacingMode::ConstantRow
    );
    state.take_audio_commands();
    (dir, state)
}
fn arrows(state: &State) -> Vec<(f32, f32)> {
    let mut actors = Vec::new();
    state.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|actor| {
            let center = source_sprite_geometry::center(&actor);
            match actor {
                Actor::Sprite {
                    z: 8004,
                    offset,
                    size: [SizeSpec::Px(w), _],
                    scale,
                    uv_rect: Some(uv),
                    ..
                } if (uv[0] - 589.0 / 1280.0).abs() < 1e-6
                    && (uv[1] - 1.0 / 1280.0).abs() < 1e-6 =>
                {
                    Some((center.unwrap_or(offset)[1], w * scale[0]))
                }
                _ => None,
            }
        })
        .collect()
}
fn sample_region(state: &State) -> (f32, f32) {
    let mut actors = Vec::new();
    state.actors(&mut actors);
    actors
        .into_iter()
        .find_map(|actor| match actor {
            Actor::Sprite {
                tint,
                offset,
                size: [_, SizeSpec::Px(h)],
                scale,
                ..
            } if tint == [0.2, 0.85, 0.65, 0.14] => {
                Some((offset[1], if h == 0.0 { scale[1] } else { h * scale[1] }))
            }
            _ => None,
        })
        .expect("sample region is visible")
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 0.0001, "{a} != {b}");
}

#[test]
fn keyboard_zoom_animates_note_positions_and_sizes_without_editing_or_moving_cursor() {
    let (_dir, mut state) = setup();
    let original = state.workspace().file().session.serialize().unwrap();
    let time = Instant::now() + Duration::from_secs(1);
    let initial = arrows(&state);
    let sample = sample_region(&state);
    let mut rendered = Vec::new();
    state.actors(&mut rendered);
    let receptor = rendered
        .iter()
        .find_map(|actor| match actor {
            Actor::Sprite {
                z: 8005,
                offset, ..
            } => Some(f64::from(offset[1])),
            _ => None,
        })
        .expect("cursor line");
    let sample_row = (0.75 - deadsync_editor::song_properties::DEFAULT_SYNC_OFFSET) * 192.0;
    close(sample.0 as f64, receptor + sample_row);
    close(sample.1 as f64, 96.0);
    close(initial[1].0 as f64, receptor + 96.0);
    shortcut(&mut state, KeyCode::ArrowUp, time);
    close(state.spacing_zoom(), 1.0);
    state.update_with_time(time + Duration::from_millis(50));
    close(state.spacing_zoom(), 1.1);
    close(arrows(&state)[1].0 as f64, receptor + 105.6);
    state.update_with_time(time + Duration::from_millis(200));
    close(state.spacing_zoom(), 1.2);
    let enlarged = arrows(&state);
    let sample = sample_region(&state);
    close(sample.0 as f64, receptor + sample_row * 1.2);
    close(sample.1 as f64, 115.2);
    close(enlarged[1].0 as f64, receptor + 115.2);
    close(enlarged[1].1 as f64, initial[1].1 as f64);
    shortcut(
        &mut state,
        KeyCode::ArrowDown,
        time + Duration::from_secs(1),
    );
    state.update_with_time(time + Duration::from_secs(2));
    shortcut(
        &mut state,
        KeyCode::ArrowDown,
        time + Duration::from_secs(3),
    );
    state.update_with_time(time + Duration::from_secs(4));
    close(state.spacing_zoom(), 1.0 / 1.2);
    let reduced = arrows(&state);
    close(reduced[1].0 as f64, receptor + 80.0);
    close(reduced[1].1 as f64, initial[1].1 as f64 / 1.2);
    shortcut(&mut state, KeyCode::Digit0, time + Duration::from_secs(5));
    close(state.spacing_zoom(), 1.0);
    assert_eq!(arrows(&state), initial);
    assert_eq!(state.workspace().row(), 0);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original
    );
    assert!(!state.workspace().file().session.is_dirty());
    assert!(state.take_audio_commands().is_empty());
}

#[test]
fn zoom_limits_render_bounded_actors_and_panels_and_focus_loss_keep_input_isolated() {
    let (_dir, mut state) = setup();
    let time = Instant::now() + Duration::from_secs(1);
    for code in [KeyCode::ArrowUp, KeyCode::ArrowDown] {
        for _ in 0..200 {
            shortcut(&mut state, code, time);
        }
        state.update_with_time(time + Duration::from_secs(1));
        let expected = if code == KeyCode::ArrowUp {
            1000000.0
        } else {
            0.000001
        };
        close(state.spacing_zoom(), expected);
        let mut actors = Vec::new();
        state.actors(&mut actors);
        assert!(actors.len() < 20000, "{} actors", actors.len());
    }
    shortcut(&mut state, KeyCode::Digit0, time);
    key(&mut state, KeyCode::F1, true, time);
    shortcut(&mut state, KeyCode::ArrowUp, time);
    state.update_with_time(time + Duration::from_secs(1));
    close(state.spacing_zoom(), 1.0);
    key(&mut state, KeyCode::Escape, true, time);
    key(&mut state, KeyCode::SuperLeft, true, time);
    state.release_input();
    // The evaluator quarantines physically held keys after focus loss. Release
    // those physical owners before beginning an independent new gesture.
    key(&mut state, KeyCode::F1, false, time);
    key(&mut state, KeyCode::Escape, false, time);
    key(&mut state, KeyCode::SuperLeft, false, time);
    key(&mut state, KeyCode::ArrowDown, true, time);
    key(&mut state, KeyCode::ArrowDown, false, time);
    state.update_with_time(time + Duration::from_secs(2));
    close(state.spacing_zoom(), 1.0);
    let mut rendered = Vec::new();
    state.actors(&mut rendered);
    assert!(state.workspace().row() > 0,
        "{:?}",
        rendered
            .iter()
            .filter_map(|actor| match actor {
                Actor::Text { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
    );
}
