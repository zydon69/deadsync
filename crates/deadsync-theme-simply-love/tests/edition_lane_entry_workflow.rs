//! Production State keyboard paths for the prepared detached note-entry bridge.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::Actor,
    space::{Metrics, set_current_metrics},
};
use deadsync_editor::notes::{Kind, Position};
use deadsync_gameplay::GameplayAudioSnapshot;
use deadsync_theme_simply_love::{
    i18n,
    screens::edition::{
        State,
        decoder_transport::{Ack, Phase},
    },
};
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
fn tap_preview_is_detached_and_release_creates_one_undoable_edit() {
    let _g = SERIAL.lock().unwrap();
    let (_d, mut s, source) = open(false, "0000");
    let revision = s.workspace().file().session.revision();
    key(&mut s, KeyCode::Digit1, true);
    assert!(ghost(&s));
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert_eq!(s.workspace().file().session.revision(), revision);
    key(&mut s, KeyCode::Digit1, false);
    assert_eq!(
        s.workspace().grid().get(p(0, 0, 0)).unwrap().kind,
        Kind::Tap
    );
    assert_eq!(s.workspace().file().session.revision(), revision + 1);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::KeyY);
    assert!(s.workspace().grid().get(p(0, 0, 0)).is_some());
}
#[test]
fn held_navigation_creates_hold_backtracks_to_tap_and_left_shift_changes_roll() {
    let _g = SERIAL.lock().unwrap();
    let (_d, mut s, source) = open(false, "0000");
    key(&mut s, KeyCode::Digit1, true);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    assert!(ghost(&s));
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    key(&mut s, KeyCode::ShiftLeft, true);
    key(&mut s, KeyCode::Digit1, false);
    key(&mut s, KeyCode::ShiftLeft, false);
    let n = s.workspace().grid().get(p(0, 0, 0)).unwrap();
    assert_eq!(n.kind, Kind::Roll);
    assert_eq!(n.end, Some(96));
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    key(&mut s, KeyCode::Digit2, true);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    key(&mut s, KeyCode::Digit2, false);
    assert_eq!(
        s.workspace().grid().get(p(0, 1, 96)).unwrap().kind,
        Kind::Tap
    );
}
#[test]
fn cancel_panels_modifiers_and_focus_loss_discard_drafts_before_release() {
    let _g = SERIAL.lock().unwrap();
    for boundary in [
        "escape",
        "properties",
        "menu",
        "minimap",
        "control",
        "alt",
        "focus",
    ] {
        let (_d, mut s, source) = open(false, "0000");
        key(&mut s, KeyCode::Digit1, true);
        press(&mut s, KeyCode::ArrowDown);
        match boundary {
            "escape" => press(&mut s, KeyCode::Escape),
            "properties" => press(&mut s, KeyCode::F11),
            "menu" => press(&mut s, KeyCode::F1),
            "minimap" => ctrl(&mut s, KeyCode::KeyM),
            "control" => {
                key(&mut s, KeyCode::ControlLeft, true);
                key(&mut s, KeyCode::ControlLeft, false);
            }
            "alt" => {
                key(&mut s, KeyCode::AltLeft, true);
                key(&mut s, KeyCode::AltLeft, false);
            }
            _ => s.release_input(),
        };
        key(&mut s, KeyCode::Digit1, false);
        assert_eq!(
            s.workspace().file().session.serialize().unwrap(),
            source,
            "{boundary}"
        );
        assert_eq!(s.workspace().file().session.revision(), 0, "{boundary}");
    }
}
#[test]
fn routine_chord_preserves_per_press_participant_and_independent_atomic_undo() {
    let _g = SERIAL.lock().unwrap();
    let (_d, mut s, source) = open(true, "00000000\n&\n00000000");
    key(&mut s, KeyCode::Digit1, true);
    press(&mut s, KeyCode::KeyP);
    assert_eq!(s.workspace().player(), 1);
    key(&mut s, KeyCode::Digit5, true);
    press(&mut s, KeyCode::ArrowDown);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    key(&mut s, KeyCode::Digit1, false);
    key(&mut s, KeyCode::Digit5, false);
    assert_eq!(s.workspace().grid().get(p(0, 0, 0)).unwrap().end, Some(48));
    assert_eq!(s.workspace().grid().get(p(1, 4, 0)).unwrap().end, Some(48));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(s.workspace().grid().get(p(1, 4, 0)).is_none());
    assert!(s.workspace().grid().get(p(0, 0, 0)).is_some());
}
#[test]
fn source_playing_and_advance_commit_immediately_while_explicit_hold_tool_is_preserved() {
    let _g = SERIAL.lock().unwrap();
    let (_d, mut s, _) = open(false, "0000");
    press(&mut s, KeyCode::Enter);
    s.bind_decoder_generation(Some(1));
    s.set_decoder_status(
        1,
        Some(Ack {
            generation: 1,
            phase: Phase::Producing,
            opened: true,
        }),
    );
    let mut audio = GameplayAudioSnapshot::default();
    audio.stream_clock.valid_at = Instant::now();
    audio.stream_clock.stream_seconds = 0.25;
    audio.stream_clock.has_music_mapping = true;
    audio.stream_clock.music_nanos = 250_000_000;
    audio.stream_clock.music_seconds_per_second = 1.0;
    s.update_audio(audio);
    key(&mut s, KeyCode::Digit1, true);
    assert!(s.is_playing());
    assert_eq!(
        s.workspace().grid().get(p(0, 0, 48)).unwrap().kind,
        Kind::Tap
    );
    key(&mut s, KeyCode::Digit1, false);
    let (_d, mut s, _) = open(false, "0000");
    press(&mut s, KeyCode::KeyA);
    key(&mut s, KeyCode::Digit1, true);
    assert_eq!(s.workspace().row(), 48);
    assert_eq!(s.workspace().grid().get(p(0, 0, 0)).unwrap().end, None);
    key(&mut s, KeyCode::Digit1, false);
    press(&mut s, KeyCode::Tab);
    assert_eq!(s.workspace().tool(), Kind::Hold);
    key(&mut s, KeyCode::Digit2, true);
    assert_eq!(s.workspace().grid().get(p(0, 1, 48)).unwrap().end, Some(96));
    key(&mut s, KeyCode::Digit2, false);
}
