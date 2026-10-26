//! Prepared end-to-end hold render scenarios. Run after applying the shared bridge.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::{Actor, SpriteSource},
    space::{Metrics, set_current_metrics},
};
use deadsync_gameplay::GameplayAudioSnapshot;
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
    let mut snapshot = GameplayAudioSnapshot::default();
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
#[test]
fn dance_and_couple_holds_light_cutoff_and_restore_without_document_edits() {
    let _g = SERIAL.lock().unwrap();
    for (layout, notes) in [
        ("dance-single", "2000\n0000\n3000\n0000"),
        ("dance-couple", "20002000\n00000000\n30003000\n00000000"),
    ] {
        let (_d, mut s, source) = open(layout, notes);
        let idle = sprites(&s);
        let idle = body(&idle, 1, 457, 128);
        assert!(!idle.is_empty(), "{layout}");
        assert!(idle.iter().all(|a| a.tint[..3] == [68.0 / 255.0; 3]));
        play(&mut s);
        let active = sprites(&s);
        let active = body(&active, 1, 457, 128);
        assert!(!active.is_empty());
        assert!(active.iter().all(|a| a.tint[..3] == [102.0 / 255.0; 3]));
        assert!(sprites(&s).iter().any(|a| (a.xy[1] - 160.0).abs() < 1e-5));
        press(&mut s, KeyCode::Enter);
        s.update_with_time(Instant::now() + Duration::from_secs(1));
        let paused = sprites(&s);
        assert!(
            body(&paused, 1, 457, 128)
                .iter()
                .all(|a| a.tint[..3] == [68.0 / 255.0; 3])
        );
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    }
}
#[test]
fn routine_retains_both_participants_and_source_selected_rims() {
    let _g = SERIAL.lock().unwrap();
    let (_d, mut s, source) = open(
        "dance-routine",
        "20000000\n00000000\n30000000\n00000000\n&\n00002000\n00000000\n00003000\n00000000",
    );
    let (_sparse_dir, sparse, _) = open("dance-routine", "20000000\n00000000\n30000000\n00000000");
    let sparse_sprites = sprites(&sparse);
    let sparse_body = body(&sparse_sprites, 1, 457, 128);
    assert!(!sparse_body.is_empty());
    assert!(
        sparse_body.iter().any(|a| a.tint[..3] != [68.0 / 255.0; 3]),
        "Routine P1 still uses its owner palette"
    );
    let before = sprites(&s);
    let before = body(&before, 1, 457, 128);
    assert!(before.len() >= 2);
    let left = before.iter().find(|a| a.xy[0] < 427.0).unwrap();
    let right = before.iter().find(|a| a.xy[0] > 427.0).unwrap();
    assert_ne!(left.tint, right.tint);
    // Native keyboard selection starts with Space; Shift+Down only navigates.
    press(&mut s, KeyCode::Space);
    press(&mut s, KeyCode::ArrowDown);
    assert_eq!(s.workspace().selection(), Some((0, 48)));
    let selected = sprites(&s);
    assert!(
        selected
            .iter()
            .any(|a| (a.uv[0] - 1.0 / 1280.0).abs() < 1e-6
                && a.uv[1] >= 261.0 / 1280.0
                && a.uv[3] <= 389.0 / 1280.0)
    );
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
#[test]
fn solo_reverse_and_piu_use_original_start_caps_and_lane_colors() {
    let _g = SERIAL.lock().unwrap();
    let (_d, mut solo, source) = open("dance-solo", "020000\n000000\n030000\n000000");
    let before = sprites(&solo);
    assert!(!body(&before, 197, 1061, 28).is_empty());
    key(&mut solo, KeyCode::ShiftLeft, true);
    press(&mut solo, KeyCode::KeyV);
    key(&mut solo, KeyCode::ShiftLeft, false);
    let after = sprites(&solo);
    assert!(body(&after, 197, 1061, 28).is_empty());
    assert!(!body(&after, 197, 521, 128).is_empty());
    assert_eq!(solo.workspace().file().session.serialize().unwrap(), source);
    let (_d, mut pump, source) = open("pump-single", "02000\n00000\n03000\n00000");
    let idle = sprites(&pump);
    let idle = body(&idle, 653, 521, 128);
    assert!(!idle.is_empty());
    let idle = idle[0].tint;
    play(&mut pump);
    let active = sprites(&pump);
    let active = body(&active, 653, 521, 128);
    assert!(!active.is_empty());
    assert_ne!(active[0].tint, idle);
    assert_eq!(pump.workspace().file().session.serialize().unwrap(), source);
}
