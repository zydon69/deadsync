//! Original short-note sprites through actual State input, timing and clipping.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::{Actor, SizeSpec, SpriteSource},
    space::{Metrics, set_current_metrics},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, sync::Mutex, time::Instant};
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
fn open(layout: &str, timing: &str, notes: &str) -> (tempfile::TempDir, State, Vec<u8>) {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("short.ssc");
    let source=format!("#VERSION:0.83;#BPMS:0=120;#OFFSET:0;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};{timing}#NOTEDATA:;#STEPSTYPE:{layout};#NOTES:{notes};").into_bytes();
    fs::write(&path, &source).unwrap();
    let state = State::open(&path, 0).unwrap();
    (dir, state, source)
}
fn sprites(s: &State) -> Vec<Actor> {
    fn walk(a: Actor, out: &mut Vec<Actor>) {
        match a {
            Actor::Frame { children, .. } => {
                for a in children {
                    walk(a, out)
                }
            }
            a @ Actor::Sprite {
                z: 8004,
                visible: true,
                ..
            } => {
                let atlas = match &a {
                    Actor::Sprite {
                        source: SpriteSource::TextureStatic(k),
                        ..
                    } => k.ends_with("editor-grooveauthor/atlas.png"),
                    Actor::Sprite {
                        source:
                            SpriteSource::Texture(k) | SpriteSource::TextureHandle { key: k, .. },
                        ..
                    } => k.ends_with("editor-grooveauthor/atlas.png"),
                    _ => false,
                };
                if atlas {
                    out.push(a)
                }
            }
            _ => {}
        }
    }
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let mut out = Vec::new();
    for a in actors {
        walk(a, &mut out)
    }
    out
}
fn rect(a: &Actor, r: [u32; 4]) -> bool {
    matches!(a,Actor::Sprite{uv_rect:Some(uv),..}if uv.iter().zip([r[0],r[1],r[0]+r[2],r[1]+r[3]]).all(|(a,b)|(*a-b as f32/1280.0).abs()<1e-6))
}
fn count(a: &[Actor], r: [u32; 4]) -> usize {
    a.iter().filter(|a| rect(a, r)).count()
}
const HEAD: [u32; 4] = [589, 1, 128, 128];
const MINE: [u32; 4] = [849, 911, 128, 128];
const FAKE: [u32; 4] = [523, 1239, 40, 40];
const LIFT: [u32; 4] = [589, 1173, 40, 40];
#[test]
fn tap_mine_fake_lift_have_source_fill_rim_markers_and_selected_rims() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut s, source) = open("dance-single", "", "1MLF\n0000\n0000\n0000");
    let a = sprites(&s);
    assert_eq!(count(&a, HEAD), 3);
    assert_eq!(count(&a, MINE), 1);
    assert_eq!(count(&a, FAKE), 1);
    assert_eq!(count(&a, LIFT), 1);
    assert_eq!(a.len(), 10);
    press(&mut s, KeyCode::Space);
    press(&mut s, KeyCode::ArrowDown);
    let a = sprites(&s);
    assert_eq!(count(&a, [981, 1, 128, 128]), 3);
    assert_eq!(count(&a, [1109, 781, 128, 128]), 1);
    assert_eq!(count(&a, FAKE), 1);
    assert_eq!(count(&a, LIFT), 1);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
#[test]
fn fake_classification_is_visible_while_stopped_and_wins_over_lift_marker() {
    let _guard = SERIAL.lock().unwrap();
    for (timing, expected) in [
        ("#FAKES:0=1;", true),
        ("#WARPS:0=1;", true),
        ("#STOPS:0=-0.5;", true),
        ("#DELAYS:0=0.1;#WARPS:0=1;", false),
    ] {
        let (_dir, s, source) = open("dance-single", timing, "L000\n0000\n0000\n0000");
        assert!(!s.is_playing());
        let a = sprites(&s);
        assert_eq!(count(&a, FAKE), usize::from(expected), "{timing}");
        assert_eq!(count(&a, LIFT), usize::from(!expected), "{timing}");
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    }
    let (_dir, s, _) = open("dance-single", "#FAKES:0=1;", "0000\n1000\n0000\n0000");
    assert_eq!(count(&sprites(&s), FAKE), 0, "FAKE end row is exclusive");
}
#[test]
fn draft_fake_and_mine_previews_use_same_policy_without_mutating_until_release() {
    let _guard = SERIAL.lock().unwrap();
    let (dir, mut s, source) = open("dance-single", "#FAKES:0=1;", "0000\n0000\n0000\n0000");
    let path = dir.path().join("low-zoom.json");
    fs::write(&path, b"{\"Version\":1,\"Zoom\":{\"SizeCap\":0.015}}").unwrap();
    s.configure_audio_preferences(&path).unwrap();
    assert_eq!(deadsync_editor::receptors::alpha(0.015).unwrap(), 0.5);
    key(&mut s, KeyCode::Digit1, true);
    let a = sprites(&s);
    assert_eq!(count(&a, HEAD), 1);
    assert_eq!(count(&a, FAKE), 1);
    assert!(
        a.iter()
            .all(|a| matches!(a,Actor::Sprite{tint,..}if(tint[3]-0.8).abs()<1e-6))
    );
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    press(&mut s, KeyCode::ArrowDown);
    let sustain = sprites(&s);
    assert!(
        sustain.iter().any(|a| rect(a, [1, 457, 128, 128])),
        "low-zoom draft sustain body uses source atlas"
    );
    assert!(
        sustain
            .iter()
            .all(|a| matches!(a,Actor::Sprite{tint,..}if(tint[3]-0.8).abs()<1e-6)),
        "draft hold ignores ordinary zoom fade"
    );
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    press(&mut s, KeyCode::ArrowUp);
    key(&mut s, KeyCode::ShiftLeft, true);
    let a = sprites(&s);
    assert_eq!(count(&a, MINE), 1);
    assert_eq!(count(&a, FAKE), 1);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    key(&mut s, KeyCode::ShiftLeft, false);
    key(&mut s, KeyCode::Digit1, false);
    assert_eq!(count(&sprites(&s), FAKE), 1);
    assert!(s.workspace().file().session.is_dirty());
    key(&mut s, KeyCode::SuperLeft, true);
    press(&mut s, KeyCode::KeyZ);
    key(&mut s, KeyCode::SuperLeft, false);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
fn tint(a: &Actor) -> [f32; 4] {
    match a {
        Actor::Sprite { tint, .. } => *tint,
        _ => panic!("sprite"),
    }
}
#[test]
fn short_notes_read_shared_xna_colors_and_selected_mine_multiplier() {
    let _guard = SERIAL.lock().unwrap();
    let (dir, mut s, source) = open("dance-single", "", "1M00\n0000\n0000\n0000");
    let mut p = deadsync_editor::note_color_preferences::Preferences::default();
    p.colors.subdivisions[0] = [0.1, 0.2, 0.3];
    p.colors.mine = [0.4, 0.5, 0.6];
    p.selection_multiplier = 1.5;
    p.colors.selected_multiplier = 8.0;
    let (mut note, mut options) = (serde_json::Map::new(), serde_json::Map::new());
    p.write(&mut note, &mut options).unwrap();
    let path = dir.path().join("prefs.json");
    fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({"Version":1,"NoteColor":note,"Options":options}))
            .unwrap(),
    )
    .unwrap();
    s.configure_audio_preferences(&path).unwrap();
    let a = sprites(&s);
    let head = a.iter().find(|a| rect(a, HEAD)).unwrap();
    assert_eq!(tint(head), [25.0 / 255.0, 51.0 / 255.0, 76.0 / 255.0, 1.0]);
    let mine = a.iter().find(|a| rect(a, MINE)).unwrap();
    assert_eq!(
        tint(mine),
        [102.0 / 255.0, 127.0 / 255.0, 153.0 / 255.0, 1.0]
    );
    press(&mut s, KeyCode::Space);
    press(&mut s, KeyCode::ArrowDown);
    let a = sprites(&s);
    let mine = a.iter().find(|a| rect(a, MINE)).unwrap();
    assert_eq!(
        tint(mine),
        [153.0 / 255.0, 191.0 / 255.0, 229.0 / 255.0, 1.0]
    );
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
fn assert_clipped(a: &Actor) {
    let Actor::Sprite {
        offset,
        size,
        rot_z_deg,
        cropleft: cl,
        cropright: cr,
        croptop: ct,
        cropbottom: cb,
        ..
    } = a
    else {
        panic!("sprite")
    };
    let [SizeSpec::Px(w), SizeSpec::Px(h)] = *size else {
        panic!("explicit size")
    };
    let rotation = rot_z_deg.rem_euclid(360.0);
    let (w, h) = if rotation == 90.0 || rotation == 270.0 {
        (h, w)
    } else {
        (w, h)
    };
    let (l, r, t, b) = match rotation {
        0.0 => (*cl, *cr, *ct, *cb),
        90.0 => (*cb, *ct, *cl, *cr),
        180.0 => (*cr, *cl, *cb, *ct),
        270.0 => (*ct, *cb, *cr, *cl),
        _ => panic!("cardinal rotation"),
    };
    let bounds = [
        offset[0] - w / 2.0 + l * w,
        offset[1] - h / 2.0 + t * h,
        offset[0] + w / 2.0 - r * w,
        offset[1] + h / 2.0 - b * h,
    ];
    assert!(
        bounds[0] >= 0.0
            && bounds[2] <= 854.0
            && bounds[1] >= 105.0 - 1e-4
            && bounds[3] <= 395.0 + 1e-4,
        "{bounds:?}"
    );
}
#[test]
fn source_rotated_notes_crossing_top_and_bottom_are_clipped_without_losing_uvs() {
    let _guard = SERIAL.lock().unwrap();
    for (layout, notes) in [
        (
            "dance-single",
            "1000\n0000\n0000\n0000,\n0000\n1000\n0000\n0000",
        ),
        (
            "pump-single",
            "10000\n00000\n00000\n00000,\n00000\n10000\n00000\n00000",
        ),
    ] {
        let (_dir, mut s, _) = open(layout, "", notes);
        let a = sprites(&s);
        assert!(
            a.iter()
                .any(|a| matches!(a,Actor::Sprite{offset,..}if(offset[1]-400.0).abs()<1e-4)),
            "bottom crossing remains emitted"
        );
        for a in &a {
            assert_clipped(a);
        }
        press(&mut s, KeyCode::ArrowRight);
        for _ in 0..3 {
            press(&mut s, KeyCode::ArrowDown);
        }
        assert_eq!(s.workspace().row(), 72);
        let a = sprites(&s);
        assert!(
            a.iter()
                .any(|a| matches!(a,Actor::Sprite{offset,..}if(offset[1]-88.0).abs()<1e-4)),
            "top crossing remains emitted"
        );
        for a in &a {
            assert_clipped(a);
        }
    }
}
