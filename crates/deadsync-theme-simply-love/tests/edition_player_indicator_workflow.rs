//! Selected editing player indicator through the native bridge.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::Actor,
    space::{Metrics, set_current_metrics},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, sync::Mutex, time::Instant};
static SERIAL: Mutex<()> = Mutex::new(());
fn press(s: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        s.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
fn open(layout: &str, notes: &str) -> (tempfile::TempDir, State, Vec<u8>) {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("indicator.ssc");
    let source =
        format!("#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:{layout};#NOTES:{notes};")
            .into_bytes();
    fs::write(&path, &source).unwrap();
    let state = State::open(&path, 0).unwrap();
    (dir, state, source)
}
fn markers(state: &State) -> Vec<([f32; 2], [f32; 4])> {
    fn walk(a: &Actor, out: &mut Vec<([f32; 2], [f32; 4])>) {
        match a {
            Actor::Sprite {
                offset,
                tint,
                uv_rect: Some(uv),
                z,
                visible: true,
                ..
            } if *z == 8002
                && (uv[0] - 589.0 / 1280.0).abs() < 1e-6
                && (uv[1] - 1215.0 / 1280.0).abs() < 1e-6 =>
            {
                out.push((*offset, *tint))
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
    state.actors(&mut actors);
    let mut out = Vec::new();
    for a in &actors {
        walk(a, &mut out)
    }
    out
}
#[test]
fn selected_player_indicator_changes_raw_tint_without_mutating_notes() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, source) = open(
        "dance-couple",
        "10000000\n00000000\n00000000\n00000000\n&\n00001000\n00000000\n00000000\n00000000",
    );
    let first = markers(&state);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].0, [626.5, 160.0]);
    let rgb = state.note_color_preferences().colors.players[0]
        .map(|c| f32::from((c * 255.0) as u8) / 255.0);
    assert_eq!(first[0].1, [rgb[0], rgb[1], rgb[2], 1.0]);
    press(&mut state, KeyCode::KeyP);
    assert_eq!(state.workspace().player(), 1);
    let second = markers(&state);
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].0, first[0].0);
    assert_ne!(second[0].1, first[0].1);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
}
#[test]
fn source_multiplayer_family_displays_indicator_even_when_only_p1_has_notes() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, routine, _) = open("dance-routine", "10000000\n00000000\n00000000\n00000000");
    assert_eq!(markers(&routine).len(), 1);
    let (_dir, double, _) = open("dance-double", "10000000\n00000000\n00000000\n00000000");
    assert!(markers(&double).is_empty());
}
