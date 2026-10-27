//! Production State keyboard paths for the prepared detached note-entry bridge.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::Actor,
    space::{Metrics, set_current_metrics},
};
use deadsync_editor::notes::{Kind, Position};
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
fn ctrl(s: &mut State, code: KeyCode) {
    key(s, KeyCode::SuperLeft, true);
    press(s, code);
    key(s, KeyCode::SuperLeft, false);
}
fn open(routine: bool, notes: &str) -> (tempfile::TempDir, State, Vec<u8>) {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("music.wav"), b"command-only").unwrap();
    let layout = if routine {
        "dance-routine"
    } else {
        "dance-single"
    };
    let source=format!("#VERSION:0.83;#MUSIC:music.wav;#BPMS:0=120;#OFFSET:0;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:{layout};#NOTES:{notes};").into_bytes();
    fs::write(dir.path().join("song.ssc"), &source).unwrap();
    let state = State::open(&dir.path().join("song.ssc"), 0).unwrap();
    (dir, state, source)
}
fn p(player: u32, lane: u16, row: u64) -> Position {
    Position { player, lane, row }
}
fn ghost(s: &State) -> bool {
    fn walk(a: &Actor) -> bool {
        match a {
            Actor::Sprite { z, tint, .. } if *z == 8004 => (tint[3] - 0.8).abs() < 1e-5,
            Actor::Frame { children, .. } => children.iter().any(walk),
            _ => false,
        }
    }
    let mut actors = Vec::new();
    s.actors(&mut actors);
    actors.iter().any(walk)
}
#[test]
fn right_shift_is_a_detached_roll_modifier_and_releasing_one_side_keeps_other_active() {
    let _g = SERIAL.lock().unwrap();
    for initial in [KeyCode::ShiftLeft, KeyCode::ShiftRight] {
        let (_d, mut state, source) = open(false, "0000");
        key(&mut state, initial, true);
        key(&mut state, KeyCode::Digit1, true);
        press(&mut state, KeyCode::ArrowDown);
        assert!(ghost(&state));
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            source
        );
        let other = if initial == KeyCode::ShiftLeft {
            KeyCode::ShiftRight
        } else {
            KeyCode::ShiftLeft
        };
        key(&mut state, other, true);
        key(&mut state, initial, false);
        press(&mut state, KeyCode::ArrowDown);
        key(&mut state, KeyCode::Digit1, false);
        key(&mut state, other, false);
        let note = state.workspace().grid().get(p(0, 0, 0)).unwrap();
        assert_eq!(note.kind, Kind::Roll);
        assert_eq!(note.end, Some(96));
        ctrl(&mut state, KeyCode::KeyZ);
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            source
        );
    }
}
