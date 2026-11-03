#[path = "support/row_geometry.rs"]
mod row_geometry;
#[path = "support/source_sprite_geometry.rs"]
mod source_sprite_geometry;
#[allow(dead_code)]
#[path = "support/transport_host.rs"]
mod transport_host;
// Production keyboard/transport/actor flow; synthetic snapshots do not certify PCM or GPU output.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::{Actor, SpriteSource},
    space::{Metrics, set_current_metrics},
};
use deadsync_editor::{
    Error,
    chart_order::{Orderer, Snapshot},
};
use deadsync_theme_simply_love::{
    i18n,
    screens::edition::{
        State,
        decoder_transport::{Ack, Phase},
    },
};
use std::{
    fs,
    sync::Mutex,
    time::{Duration, Instant},
};
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
fn ctrl(state: &mut State, code: KeyCode, shift: bool) {
    key(state, KeyCode::SuperLeft, true);
    if shift {
        key(state, KeyCode::ShiftLeft, true);
    }
    press(state, code);
    key(state, KeyCode::ShiftLeft, false);
    key(state, KeyCode::SuperLeft, false);
}
struct Native;
impl Orderer for Native {
    fn order(&mut self, snapshot: &Snapshot) -> Result<Vec<usize>, Error> {
        snapshot.indices_with(str::cmp)
    }
}
fn setup() -> (tempfile::TempDir, State, Vec<u8>) {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("music.wav"), b"command-only fixture").unwrap();
    let source=b"#VERSION:0.83;#MUSIC:music.wav;#SAMPLESTART:0;#SAMPLELENGTH:1;#BPMS:0=120;#OFFSET:0;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:20F0\n0100\n3000\n0000;#NOTEDATA:;#STEPSTYPE:dance-double;#BPMS:0=60;#OFFSET:0.25;#NOTES:00002000\n10000000\n00003000\n00000000;".to_vec();
    fs::write(dir.path().join("song.ssc"), &source).unwrap();
    let mut state = State::open(&dir.path().join("song.ssc"), 0).unwrap();
    row_geometry::configure_centered(&mut state, dir.path());
    (dir, state, source)
}
fn both(state: &mut State) {
    ctrl(state, KeyCode::KeyB, true);
    ctrl(state, KeyCode::Tab, false);
    assert!(state.process_chart_order(&mut Native));
    ctrl(state, KeyCode::KeyB, true);
    ctrl(state, KeyCode::PageDown, false);
    assert_eq!(state.workspace().chart(), 0);
}
fn snapshot(state: &mut State, generation: u64, phase: Phase, stream: f32, music: Option<f64>) {
    snapshot_at(state, generation, phase, stream, music, Instant::now());
}
fn snapshot_at(
    state: &mut State,
    generation: u64,
    phase: Phase,
    stream: f32,
    music: Option<f64>,
    at: Instant,
) {
    state.set_decoder_status(
        generation,
        Some(Ack {
            generation,
            phase,
            opened: !matches!(phase, Phase::Pending | Phase::Failed),
        }),
    );
    let mut s = transport_host::host_snapshot(state);
    if phase == Phase::Pending || phase == Phase::Failed {
        s.editor.assets.main = false;
    }
    s.stream_clock.valid_at = at;
    s.stream_clock.stream_seconds = stream;
    s.stream_clock.has_music_mapping = music.is_some();
    s.stream_clock.music_nanos = (music.unwrap_or_default() * 1e9).round() as i64;
    s.stream_clock.music_seconds_per_second = 1.0;
    state.update_audio(s);
}
#[derive(Debug)]
struct Sprite {
    x: f32,
    y: f32,
    z: i16,
    tint: [f32; 4],
    receptor: bool,
}
fn actors(state: &State) -> Vec<Actor> {
    let mut out = Vec::new();
    state.actors(&mut out);
    out
}
fn sprites(state: &State) -> Vec<Sprite> {
    fn walk(a: &Actor, out: &mut Vec<Sprite>) {
        match a {
            Actor::Sprite {
                offset,
                z,
                tint,
                source,
                ..
            } => out.push(Sprite {
                x: source_sprite_geometry::center(a).unwrap_or(*offset)[0],
                y: source_sprite_geometry::center(a).unwrap_or(*offset)[1],
                z: *z,
                tint: *tint,
                receptor: match source {
                    SpriteSource::TextureStatic(key) => {
                        key.ends_with("editor-grooveauthor/atlas.png")
                    }
                    SpriteSource::Texture(key) | SpriteSource::TextureHandle { key, .. } => {
                        key.ends_with("editor-grooveauthor/atlas.png")
                    }
                    _ => false,
                },
            }),
            Actor::Frame { children, .. } => {
                for a in children {
                    walk(a, out)
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for a in actors(state) {
        walk(&a, &mut out)
    }
    out
}
fn has(values: &[Sprite], x: f32, y: f32, z: i16) -> bool {
    values
        .iter()
        .any(|s| (s.x - x).abs() < 0.01 && (s.y - y).abs() < 0.01 && s.z == z)
}
#[test]
fn pending_generations_show_idle_receptors_without_consuming_chart_notes() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, source) = setup();
    let initial = sprites(&state);
    assert_eq!(
        initial.iter().filter(|s| s.receptor && s.z == 8001).count(),
        4
    );
    transport_host::publish(&mut state);
    press(&mut state, KeyCode::Enter);
    state.take_audio_commands();
    state.bind_decoder_generation(Some(1));
    snapshot(&mut state, 1, Phase::Pending, 1.0, Some(0.25));
    state.update_with_time(Instant::now() + Duration::from_secs(1));
    let values = sprites(&state);
    assert!(!values.iter().any(|s| s.receptor && s.z >= 8009));
    assert!(
        has(&values, 355.0, 136.0, 8004),
        "hold is not consumed by pending request: {values:?}"
    );
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
}
#[test]
fn all_resident_charts_use_their_own_validated_clock_for_receptors_and_hold_cutoff() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, source) = setup();
    both(&mut state);
    transport_host::publish(&mut state);
    press(&mut state, KeyCode::Enter);
    state.take_audio_commands();
    state.bind_decoder_generation(Some(1));
    // Keep the render frame exactly at the acknowledged 0.5s musical sample.
    let now = Instant::now();
    snapshot_at(&mut state, 1, Phase::Producing, 1.0, Some(0.5), now);
    state.update_with_time(now);
    let values = sprites(&state);
    assert!(
        has(&values, 355.0, 160.0, 8009),
        "focused hold feedback {values:?}"
    );
    assert!(
        has(&values, 91.0, 160.0, 8009),
        "inactive Double hold feedback {values:?}"
    );
    assert!(has(&values, 355.0, 160.0, 8004));
    assert!(has(&values, 91.0, 160.0, 8004));
    let focused = values
        .iter()
        .find(|s| s.receptor && s.z == 8001 && (s.x - 355.0).abs() < 0.01)
        .unwrap();
    let inactive = values
        .iter()
        .find(|s| s.receptor && s.z == 8001 && (s.x - 91.0).abs() < 0.01)
        .unwrap();
    assert!((focused.tint[0] - 1.0).abs() < 0.001);
    assert!((inactive.tint[0] - 0.8).abs() < 0.001);
    // Preserve the mapped transport snapshot when pausing; publication of a
    // synthetic unmapped zero clock would change the position being tested.
    press(&mut state, KeyCode::Enter);
    state.update_with_time(now + Duration::from_millis(100));
    let paused = sprites(&state);
    assert!(!paused.iter().any(|s| s.receptor && s.z == 8010));
    assert!(has(&paused, 355.0, 112.0, 8004),
        "paused heads: {:?}",
        paused.iter().filter(|s| s.z == 8004).collect::<Vec<_>>());
    assert!(has(&paused, 91.0, 124.0, 8004));
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
}
#[test]
fn passed_taps_trigger_hit_feedback_while_fake_notes_remain_visible_and_pause_does_not_advance_audio()
 {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, _source) = setup();
    transport_host::publish(&mut state);
    press(&mut state, KeyCode::Enter);
    state.take_audio_commands();
    state.bind_decoder_generation(Some(1));
    let now = Instant::now() + Duration::from_secs(1);
    snapshot(&mut state, 1, Phase::Producing, 1.0, Some(0.25));
    state.update_with_time(now);
    snapshot(&mut state, 1, Phase::Producing, 1.26, Some(0.51));
    state.update_with_time(now + Duration::from_millis(10));
    let values = sprites(&state);
    assert!(
        values
            .iter()
            .any(|s| s.receptor && s.z == 8010 && (s.x - 403.0).abs() < 0.01)
    );
    assert!(
        !values
            .iter()
            .any(|s| s.z == 8004 && (s.x - 403.0).abs() < 0.01)
    );
    fn has_fake_marker(actor:&Actor)->bool {
        match actor {
            Actor::Sprite{z:8004,uv_rect:Some(uv),visible:true,..} =>
                uv.iter().zip([523u32,1239,563,1279]).all(|(a,b)|(*a-b as f32/1280.0).abs()<1e-6),
            Actor::Frame{children,..}=>children.iter().any(has_fake_marker),
            _=>false,
        }
    }
    assert!(actors(&state).iter().any(has_fake_marker),"fake note survives consumption with its original source marker");
    transport_host::publish(&mut state);
    press(&mut state, KeyCode::Enter);
    let time = state.chart_focus_time().unwrap();
    state.update_with_time(now + Duration::from_secs(5));
    assert_eq!(state.chart_focus_time().unwrap(), time);
    assert!(!sprites(&state).iter().any(|s| s.receptor && s.z >= 8009));
}
fn open_help(state: &mut State) {
    press(state, KeyCode::F1);
    for _ in 0..12 {
        press(state, KeyCode::ArrowDown);
    }
    press(state, KeyCode::Enter);
}
#[test]
fn held_lane_releases_through_panel_help_or_focus_loss_and_only_focus_receives_edits() {
    let _guard = SERIAL.lock().unwrap();
    for boundary in ["panel", "help", "focus_loss"] {
        let (_dir, mut state, _source) = setup();
        both(&mut state);
        let inactive = state
            .workspace()
            .file()
            .session
            .loaded_grid(1)
            .unwrap()
            .clone();
        key(&mut state, KeyCode::Digit4, true);
        let now = Instant::now() + Duration::from_secs(1);
        state.update_with_time(now);
        let values = sprites(&state);
        assert!(has(&values, 499.0, 160.0, 8009), "{boundary}: {values:?}");
        assert!(
            !values
                .iter()
                .any(|s| s.receptor && s.z >= 8009 && s.x < 331.0)
        );
        match boundary {
            "panel" => press(&mut state, KeyCode::F11),
            "help" => open_help(&mut state),
            _ => {
                key(&mut state, KeyCode::AltLeft, true);
                state.release_input();
            }
        }
        key(&mut state, KeyCode::Digit4, false);
        state.update_with_time(now + Duration::from_millis(300));
        if boundary == "help" {
            press(&mut state, KeyCode::Escape);
        }
        assert!(
            !sprites(&state).iter().any(|s| s.receptor && s.z >= 8009),
            "{boundary}"
        );
        assert_eq!(
            state.workspace().file().session.loaded_grid(1).unwrap(),
            &inactive
        );
        if boundary == "focus_loss" {
            // Release the physical modifier quarantined by focus loss before
            // starting a fresh lane gesture.
            key(&mut state, KeyCode::AltLeft, false);
            key(&mut state, KeyCode::Digit4, true);
            state.update_with_time(now + Duration::from_millis(310));
            assert!(has(&sprites(&state), 499.0, 160.0, 8009));
            key(&mut state, KeyCode::Digit4, false);
        }
    }
}
#[test]
fn digits_owned_by_help_settings_minimap_or_modifiers_never_arm_manual_feedback() {
    let _guard = SERIAL.lock().unwrap();
    for owner in ["help", "settings", "minimap", "control", "shift", "alt"] {
        let (_dir, mut state, source) = setup();
        match owner {
            "help" => open_help(&mut state),
            "settings" => ctrl(&mut state, KeyCode::KeyM, true),
            "minimap" => ctrl(&mut state, KeyCode::KeyM, false),
            "control" => key(&mut state, KeyCode::ControlLeft, true),
            "shift" => key(&mut state, KeyCode::ShiftLeft, true),
            _ => key(&mut state, KeyCode::AltLeft, true),
        }
        key(&mut state, KeyCode::Digit4, true);
        // Close the owner while the digit is still physically held. A wrongly
        // armed manual rim would remain visible indefinitely, not just flash.
        match owner {
            "help" | "settings" | "minimap" => press(&mut state, KeyCode::Escape),
            "control" => key(&mut state, KeyCode::ControlLeft, false),
            "shift" => key(&mut state, KeyCode::ShiftLeft, false),
            _ => key(&mut state, KeyCode::AltLeft, false),
        }
        state.update_with_time(Instant::now() + Duration::from_secs(1));
        let values = sprites(&state);
        assert!(
            !values.iter().any(|s| s.receptor && s.z >= 8009),
            "{owner}: {values:?}"
        );
        if matches!(owner, "help" | "settings" | "minimap") {
            assert_eq!(
                state.workspace().file().session.serialize().unwrap(),
                source
            );
        }
        key(&mut state, KeyCode::Digit4, false);
    }
}
