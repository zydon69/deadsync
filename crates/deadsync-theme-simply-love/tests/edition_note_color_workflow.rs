#[allow(dead_code)]
#[path = "support/transport_host.rs"]
mod transport_host;
// Shared application NoteColor DTO drives the actual State hold renderer.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::{Actor, SpriteSource},
    space::{Metrics, set_current_metrics},
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
fn open(layout: &str, notes: &str) -> (tempfile::TempDir, State, Vec<u8>) {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("music.wav"), b"command-only").unwrap();
    let source=format!("#VERSION:0.83;#MUSIC:music.wav;#BPMS:0=120;#OFFSET:0;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#NOTEDATA:;#STEPSTYPE:{layout};#NOTES:{notes};").into_bytes();
    fs::write(d.path().join("song.ssc"), &source).unwrap();
    let s = State::open(&d.path().join("song.ssc"), 0).unwrap();
    (d, s, source)
}
fn play(s: &mut State) {
    transport_host::publish(s);
    press(s, KeyCode::Enter);
    s.bind_decoder_generation(Some(1));
    s.set_decoder_status(
        1,
        Some(Ack {
            generation: 1,
            phase: Phase::Producing,
            opened: true,
        }),
    );
    let mut snapshot = transport_host::host_snapshot(s);
    snapshot.stream_clock.valid_at = Instant::now();
    snapshot.stream_clock.stream_seconds = 0.25;
    snapshot.stream_clock.has_music_mapping = true;
    snapshot.stream_clock.music_nanos = 250_000_000;
    snapshot.stream_clock.music_seconds_per_second = 1.0;
    s.update_audio(snapshot);
    s.update_with_time(Instant::now() + Duration::from_millis(10));
}
#[derive(Debug)]
struct Sprite {
    xy: [f32; 2],
    tint: [f32; 4],
    uv: [f32; 4],
}
fn sprites(s: &State) -> Vec<Sprite> {
    fn walk(a: &Actor, out: &mut Vec<Sprite>) {
        match a {
            Actor::Sprite {
                offset,
                tint,
                uv_rect: Some(uv),
                source,
                z,
                ..
            } if *z == 8004 => {
                let atlas = match source {
                    SpriteSource::TextureStatic(k) => k.ends_with("editor-grooveauthor/atlas.png"),
                    SpriteSource::Texture(k) | SpriteSource::TextureHandle { key: k, .. } => {
                        k.ends_with("editor-grooveauthor/atlas.png")
                    }
                    _ => false,
                };
                if atlas {
                    out.push(Sprite {
                        xy: *offset,
                        tint: *tint,
                        uv: *uv,
                    });
                }
            }
            Actor::Frame { children, .. } => {
                for a in children {
                    walk(a, out)
                }
            }
            _ => {}
        }
    }
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let mut out = Vec::new();
    for a in &actors {
        walk(a, &mut out);
    }
    out
}
fn body<'a>(sprites: &'a [Sprite], x: u32, y: u32, h: u32) -> Vec<&'a Sprite> {
    sprites
        .iter()
        .filter(|s| {
            (s.uv[0] - x as f32 / 1280.0).abs() < 1e-6
                && s.uv[1] >= y as f32 / 1280.0 - 1e-6
                && s.uv[3] <= (y + h) as f32 / 1280.0 + 1e-6
        })
        .collect()
}

fn configure(
    s: &mut State,
    dir: &std::path::Path,
    p: deadsync_editor::note_color_preferences::Preferences,
) {
    let (mut note, mut options) = (serde_json::Map::new(), serde_json::Map::new());
    p.write(&mut note, &mut options).unwrap();
    let path = dir.join("colors.json");
    fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({"Version":1,"NoteColor":note,"Options":options,"Scroll":{"SpacingMode":"ConstantRow","RowBasedPixelsPerRow":1}}))
            .unwrap(),
    )
    .unwrap();
    s.configure_audio_preferences(&path).unwrap();
}
fn expected(rgb: [u8; 3]) -> [f32; 3] {
    rgb.map(|n| f32::from(n) / 255.0)
}
#[test]
fn custom_hold_and_roll_follow_shared_rgb_and_selected_held_xna_variants() {
    let _guard = SERIAL.lock().unwrap();
    for (head, roll) in [('2', false), ('4', true)] {
        let notes = format!("{head}000\n0000\n3000\n0000");
        let (dir, mut state, source) = open("dance-single", &notes);
        let mut p = deadsync_editor::note_color_preferences::Preferences::default();
        p.colors.hold = [0.1, 0.2, 0.3];
        p.colors.roll = [0.3, 0.2, 0.1];
        p.held_multiplier = 2.0;
        p.selection_multiplier = 1.5;
        // UI multipliers must never leak into the large chart.
        p.colors.normal_multiplier = 7.0;
        p.colors.selected_multiplier = 8.0;
        configure(&mut state, dir.path(), p);
        let fill_y = if roll { 653 } else { 457 };
        let initial = sprites(&state);
        let initial = body(&initial, 1, fill_y, 128);
        assert!(!initial.is_empty());
        let raw = if roll { [76, 51, 25] } else { [25, 51, 76] };
        assert!(initial.iter().all(|s| s.tint[..3] == expected(raw)));
        press(&mut state, KeyCode::Space);
        press(&mut state, KeyCode::ArrowDown);
        assert_eq!(state.workspace().selection(), Some((0, 48)));
        let selected = sprites(&state);
        let rim = body(&selected, 1, 261, 128);
        assert!(!rim.is_empty(), "source selected rim remains visible");
        assert!(
            rim.iter().all(|s| s.tint == [1.0; 4]),
            "rim is untinted white"
        );
        let selected = body(&selected, 1, fill_y, 128);
        assert!(!selected.is_empty());
        let rgb = if roll { [114, 76, 38] } else { [38, 76, 114] };
        assert!(
            selected.iter().all(|s| s.tint[..3] == expected(rgb)),
            "selected {roll}: expected {:?}, observed {:?}",
            expected(rgb),
            selected.iter().map(|s| s.tint).collect::<Vec<_>>()
        );
        // Home retains the anchor; clear selection through its actual toggle.
        press(&mut state, KeyCode::Space);
        assert_eq!(state.workspace().selection(), None);
        // The event picker owns explicit choices independently of the row anchor.
        key(&mut state, KeyCode::SuperLeft, true);
        key(&mut state, KeyCode::ShiftLeft, true);
        press(&mut state, KeyCode::KeyE);
        key(&mut state, KeyCode::ShiftLeft, false);
        key(&mut state, KeyCode::SuperLeft, false);
        press(&mut state, KeyCode::Escape);
        press(&mut state, KeyCode::Home);
        play(&mut state);
        let held = sprites(&state);
        let held = body(&held, 1, fill_y, 128);
        assert!(!held.is_empty());
        let rgb = if roll { [153, 102, 51] } else { [51, 102, 153] };
        assert!(
            held.iter().all(|s| s.tint[..3] == expected(rgb)),
            "held {roll}: expected {:?}, observed {:?}, selection {:?}",
            expected(rgb),
            held.iter().map(|s| s.tint).collect::<Vec<_>>(),
            state.workspace().selection()
        );
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            source
        );
    }
}
#[test]
fn routine_custom_players_blend_while_couple_keeps_ordinary_color() {
    let _guard = SERIAL.lock().unwrap();
    for layout in ["dance-routine", "dance-couple"] {
        let (dir, mut state, source) = open(
            layout,
            "20000000\n00000000\n30000000\n00000000\n&\n00002000\n00000000\n00003000\n00000000",
        );
        let mut p = deadsync_editor::note_color_preferences::Preferences::default();
        p.colors.hold = [0.2, 0.2, 0.2];
        p.colors.routine_alpha = 1.0;
        p.colors.players[0] = [1.0, 0.0, 0.0];
        p.colors.players[1] = [0.0, 1.0, 0.0];
        configure(&mut state, dir.path(), p);
        let all = sprites(&state);
        let bodies = body(&all, 1, 457, 128);
        assert!(!bodies.is_empty());
        if layout == "dance-couple" {
            assert!(bodies.iter().all(|s| s.tint[..3] == expected([51; 3])));
        } else {
            assert!(bodies.iter().any(|s| s.tint[..3] == expected([255, 0, 0])));
            assert!(bodies.iter().any(|s| s.tint[..3] == expected([0, 255, 0])));
        }
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            source
        );
    }
}
#[test]
fn piu_custom_saturation_affects_body_but_preserves_cap_note_tint() {
    let _guard = SERIAL.lock().unwrap();
    let (dir, mut state, source) = open("pump-single", "02000\n00000\n03000\n00000");
    let mut p = deadsync_editor::note_color_preferences::Preferences::default();
    p.colors.piu_top = [0.2, 0.6, 0.4];
    p.colors.piu_hold_saturation = 0.0;
    configure(&mut state, dir.path(), p);
    let all = sprites(&state);
    // Zero HSV saturation yields the value channel in all body components.
    assert!(all.iter().any(|s| s.tint[..3] == expected([153; 3])));
    // PIU cap/head keep their original, non-desaturated lane tint.
    assert!(all.iter().any(|s| s.tint[..3] == expected([51, 153, 102])));
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
}
#[test]
fn resident_charts_use_their_own_signature_phase_for_note_color() {
    use deadsync_editor::chart_order::{Orderer, Snapshot};
    struct Native;
    impl Orderer for Native {
        fn order(&mut self, s: &Snapshot) -> Result<Vec<usize>, deadsync_editor::Error> {
            s.indices_with(str::cmp)
        }
    }
    fn control(s: &mut State, code: KeyCode, shift: bool) {
        key(s, KeyCode::SuperLeft, true);
        if shift {
            key(s, KeyCode::ShiftLeft, true);
        }
        press(s, code);
        key(s, KeyCode::ShiftLeft, false);
        key(s, KeyCode::SuperLeft, false);
    }
    let _guard = SERIAL.lock().unwrap();
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("phase.ssc");
    let notes = "0000\n2000\n0000\n3000\n0000\n0000\n0000\n0000";
    let source=format!("#VERSION:0.83;#BPMS:0=120;#OFFSET:0;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:A;#TIMESIGNATURES:0=4=4;#NOTES:{notes};#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:B;#TIMESIGNATURES:0=4=4,0.5=3=8;#NOTES:{notes};").into_bytes();
    fs::write(&path, &source).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    let mut p = deadsync_editor::note_color_preferences::Preferences::default();
    p.step_color_method = deadsync_editor::minimap_palette::ColorMethod::Note;
    p.colors.subdivisions[0] = [1.0, 0.0, 0.0];
    p.colors.subdivisions[1] = [0.0, 1.0, 0.0];
    configure(&mut state, dir.path(), p);
    control(&mut state, KeyCode::KeyB, true);
    control(&mut state, KeyCode::Tab, false);
    assert!(state.process_chart_order(&mut Native));
    let rendered = sprites(&state);
    let red: Vec<_> = rendered
        .iter()
        .filter(|s| s.tint[..3] == expected([255, 0, 0]))
        .collect();
    let green: Vec<_> = rendered
        .iter()
        .filter(|s| s.tint[..3] == expected([0, 255, 0]))
        .collect();
    assert!(
        !red.is_empty(),
        "signature at the same note row resets B to quarter phase"
    );
    assert!(!green.is_empty(), "A retains eighth-note phase at row24");
    assert_ne!(
        red[0].xy[0], green[0].xy[0],
        "distinct resident fields retain independent phases"
    );
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
}
