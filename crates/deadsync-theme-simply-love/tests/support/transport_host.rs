#![allow(dead_code)] // Shared targets exercise different parts of this host.
//! Deterministic native transport host. Assets/leases are acknowledgements from
//! a simulated host; these fixtures never open a physical audio device.
use deadlib_present::actors::{Actor, SizeSpec, SpriteSource};
use deadsync_gameplay::{EditorAssetReadiness, GameplayAudioSnapshot};
use deadsync_theme_simply_love::screens::edition::State;
use std::{path::Path, time::Instant};

pub fn configure_projection(state: &mut State, path: &Path, stop_scrolling: bool) {
    let mut preferences = if path.exists() {
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap()).unwrap()
    } else {
        serde_json::json!({"Version": 1})
    };
    preferences["SnapIndex"] = 1.into();
    preferences["Scroll"] = serde_json::json!({
        "SpacingMode": "ConstantRow", "RowBasedPixelsPerRow": 1,
        "TimeBasedPixelsPerSecond": 300, "VariablePixelsPerSecondAtDefaultBPM": 300,
        "StopPlaybackWhenScrolling": stop_scrolling,
    });
    std::fs::write(path, serde_json::to_vec(&preferences).unwrap()).unwrap();
    state.configure_audio_preferences(path).unwrap();
    state.configure_spacing_preference(
        deadsync_editor::scroll_preferences::spacing::Preference::ConstantRow,
    );
}
pub fn host_snapshot(state: &State) -> GameplayAudioSnapshot {
    let mut snapshot = GameplayAudioSnapshot::default();
    let owner = state.editor_asset_owner();
    let paths = state.editor_asset_paths();
    snapshot.editor.owner = owner;
    snapshot.editor.output.available = true;
    snapshot.editor.output.leased = true;
    snapshot.editor.assets = EditorAssetReadiness {
        owner,
        registration: state.editor_asset_registration(),
        main: paths[0].is_some(),
        dedicated_preview: paths[1].is_some(),
    };
    snapshot
}
pub fn publish(state: &mut State) -> u64 {
    state.take_audio_commands();
    state.update_audio(host_snapshot(state));
    state.take_audio_commands();
    state.editor_asset_owner()
}
pub fn snapshot(state: &State, seconds: f64) -> GameplayAudioSnapshot {
    let mut snapshot = host_snapshot(state);
    snapshot.stream_clock.valid_at = Instant::now();
    snapshot.stream_clock.has_music_mapping = true;
    snapshot.stream_clock.music_nanos = (seconds * 1e9).round() as i64;
    snapshot
}
pub fn heads(state: &State) -> Vec<[f32; 2]> {
    let mut actors = Vec::new();
    state.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|actor| match actor {
            Actor::Sprite {
                z: 8004,
                offset,
                source: SpriteSource::TextureStatic(key),
                uv_rect: Some(uv),
                size: [SizeSpec::Px(width), SizeSpec::Px(height)],
                rot_z_deg,
                cropleft,
                cropright,
                croptop,
                cropbottom,
                visible: true,
                ..
            } if key.ends_with("editor-grooveauthor/atlas.png")
                && (uv[0] - 589.0 / 1280.0).abs() < 1e-6
                && (uv[1] - 1.0 / 1280.0).abs() < 1e-6 =>
            {
                let delta = [
                    (cropleft - cropright) * width * 0.5,
                    (croptop - cropbottom) * height * 0.5,
                ];
                let (sin, cos) = rot_z_deg.rem_euclid(360.0).to_radians().sin_cos();
                Some([
                    offset[0] - (cos * delta[0] + sin * delta[1] - delta[0]),
                    offset[1] - (-sin * delta[0] + cos * delta[1] - delta[1]),
                ])
            }
            _ => None,
        })
        .collect()
}
