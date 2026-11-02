#[allow(dead_code)]
#[path = "support/transport_host.rs"]
mod transport_host;
// Shared receptor application preferences drive actual chart presentation.
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

fn configure(s: &mut State, dir: &std::path::Path, hide: bool, light: bool, effects: bool) {
    let path = dir.join("receptors.json");
    fs::write(&path,serde_json::to_vec(&serde_json::json!({"Version":1,"Scroll":{"SpacingMode":"ConstantRow","RowBasedPixelsPerRow":1},"Receptors":{"AutoPlayHideArrows":hide,"AutoPlayLightHolds":light,"AutoPlayRimEffect":effects,"AutoPlayGlowEffect":effects,"AutoPlayShrinkEffect":effects,"TapRimEffect":effects,"TapShrinkEffect":effects,"PulseReceptorsWithTempo":effects}})).unwrap()).unwrap();
    s.configure_audio_preferences(&path).unwrap();
}
fn note_count(s: &State, x: f32) -> usize {
    fn count(a: &Actor, x: f32) -> usize {
        match a {
            Actor::Sprite {
                offset,
                z,
                visible: true,
                ..
            } if *z == 8004 && (offset[0] - x).abs() < 1e-3 => 1,
            Actor::Frame { children, .. } => children.iter().map(|a| count(a, x)).sum(),
            _ => 0,
        }
    }
    let mut actors = Vec::new();
    s.actors(&mut actors);
    actors.iter().map(|a| count(a, x)).sum()
}
#[test]
fn hide_arrows_option_keeps_consumed_tap_visible_independently_of_hold_lighting() {
    let _guard = SERIAL.lock().unwrap();
    for hide in [false, true] {
        let (dir, mut s, source) = open("dance-single", "1000\n0000\n0000\n0000");
        configure(&mut s, dir.path(), hide, true, true);
        play(&mut s);
        assert_eq!(note_count(&s, 355.0) > 0, !hide);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    }
}
#[test]
fn light_holds_option_changes_body_but_hide_arrows_still_clips_active_head() {
    let _guard = SERIAL.lock().unwrap();
    for light in [false, true] {
        let (dir, mut s, source) = open("dance-single", "2000\n0000\n3000\n0000");
        configure(&mut s, dir.path(), true, light, true);
        play(&mut s);
        let all = sprites(&s);
        let bodies = body(&all, 1, 457, 128);
        assert!(!bodies.is_empty());
        let value = if light { 102.0 } else { 68.0 } / 255.0;
        assert!(bodies.iter().all(|s| s.tint[..3] == [value; 3]));
        assert!(
            all.iter().any(|s| (s.xy[1] - 160.0).abs() < 1e-5),
            "active head cutoff remains enabled"
        );
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    }
}
#[test]
fn disabled_manual_effects_do_not_create_held_receptor_overlay() {
    fn overlays(s: &State) -> usize {
        fn count(a: &Actor) -> usize {
            match a {
                Actor::Sprite {
                    z, visible: true, ..
                } if *z == 8009 || *z == 8010 => 1,
                Actor::Frame { children, .. } => children.iter().map(count).sum(),
                _ => 0,
            }
        }
        let mut a = Vec::new();
        s.actors(&mut a);
        a.iter().map(count).sum()
    }
    let _guard = SERIAL.lock().unwrap();
    for effects in [false, true] {
        let (dir, mut s, source) = open("dance-single", "0000\n0000\n0000\n0000");
        configure(&mut s, dir.path(), true, true, effects);
        key(&mut s, KeyCode::Digit1, true);
        s.update_with_time(Instant::now() + Duration::from_millis(10));
        assert_eq!(overlays(&s) > 0, effects);
        s.release_input();
        key(&mut s, KeyCode::Digit1, false);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    }
}
