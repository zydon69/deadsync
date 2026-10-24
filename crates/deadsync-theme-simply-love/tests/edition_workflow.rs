//! Native editor shortcut -> domain command -> saved file -> reopen. No user
//! data is touched, and no synthetic key event is described as physical input.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::notes::Position;
use deadsync_theme_simply_love::screens::edition::State;
use std::{fs, time::Instant};

const SONG: &[u8] = b"#TITLE:Test;#NOTEDATA:;#STEPSTYPE:dance-routine;#DIFFICULTY:Hard;#METER:9;#NOTES:\n10000000\n00000000\n00000000\n00000000\n&\n00001000\n00000000\n00000000\n00000000\n;";

fn key(state: &mut State, code: KeyCode, pressed: bool) -> bool {
    state.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    })
}
fn press(state: &mut State, code: KeyCode) -> bool {
    let closed = key(state, code, true);
    key(state, code, false);
    closed
}
fn ctrl(state: &mut State, code: KeyCode) {
    key(state, KeyCode::SuperLeft, true);
    press(state, code);
    key(state, KeyCode::SuperLeft, false);
}

#[test]
fn keyboard_edit_undo_redo_and_save_are_observable_after_reopening() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open_selected(&path, "dance-routine", "Hard", "", 9).unwrap();
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Digit2);
    assert!(
        screen
            .workspace()
            .grid()
            .get(Position {
                player: 0,
                lane: 1,
                row: 48
            })
            .is_some()
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert!(!screen.workspace().file().session.is_dirty());
    ctrl(&mut screen, KeyCode::KeyY);
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert!(
        reopened
            .workspace()
            .grid()
            .get(Position {
                player: 0,
                lane: 1,
                row: 48
            })
            .is_some()
    );
    assert!(press(&mut screen, KeyCode::Escape));
}

#[test]
fn unsaved_close_requires_an_explicit_discard_confirmation() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::Digit2);
    assert!(!press(&mut screen, KeyCode::Escape));
    assert!(!press(&mut screen, KeyCode::Enter)); // default: keep editing
    assert!(screen.workspace().file().session.is_dirty());
    assert!(!press(&mut screen, KeyCode::Escape));
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    assert!(!press(&mut screen, KeyCode::Enter)); // request discard, not yet confirmed
    assert!(press(&mut screen, KeyCode::Enter));
    assert_eq!(fs::read(&path).unwrap(), SONG);
}

#[test]
fn participant_shortcut_edits_player_two_and_renders_both_owners() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::KeyP);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Digit7);
    assert!(
        screen
            .workspace()
            .grid()
            .get(Position {
                player: 1,
                lane: 6,
                row: 48
            })
            .is_some()
    );
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    let arrows: Vec<_> = actors
        .iter()
        .filter_map(|actor| match actor {
            Actor::Sprite { tint, z: 8004, .. } => Some(*tint),
            _ => None,
        })
        .collect();
    assert!(
        arrows.iter().any(|color| color[2] > color[0]),
        "P1 blue arrows must be present"
    );
    assert!(
        arrows.iter().any(|color| color[0] > color[2]),
        "P2 pink arrows must be present"
    );
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert!(
        reopened
            .workspace()
            .grid()
            .get(Position {
                player: 1,
                lane: 6,
                row: 48
            })
            .is_some()
    );
}

#[test]
fn failed_save_in_close_prompt_preserves_edits_and_does_not_close() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::Digit2);
    fs::write(&path, b"external changes").unwrap();
    assert!(!press(&mut screen, KeyCode::Escape));
    press(&mut screen, KeyCode::ArrowDown);
    assert!(!press(&mut screen, KeyCode::Enter));
    assert!(screen.workspace().file().session.is_dirty());
    assert_eq!(fs::read(&path).unwrap(), b"external changes");
}
