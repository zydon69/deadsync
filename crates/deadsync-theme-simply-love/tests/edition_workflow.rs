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

const SINGLE: &[u8] = b"#VERSION:0.83;#TITLE:Test;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000\n0100\n0010\n0001;";
fn start_generation(screen: &mut State) {
    press(screen, KeyCode::F3);
    press(screen, KeyCode::ArrowRight); // dance-single -> dance-double
    for _ in 0..3 {
        press(screen, KeyCode::ArrowDown);
    }
    press(screen, KeyCode::Enter);
}
fn finish_generation(screen: &mut State) {
    let deadline = Instant::now() + std::time::Duration::from_secs(30);
    while screen.is_generating() {
        screen.update();
        assert!(
            Instant::now() < deadline,
            "Native generation did not finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
#[test]
fn generation_shortcut_previews_without_editing_then_applies_undoes_and_saves() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SINGLE).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    start_generation(&mut screen);
    press(&mut screen, KeyCode::Digit1); // no editing during calculation or preview
    finish_generation(&mut screen);
    assert_eq!(screen.generation_report().unwrap().notes, 4);
    assert_eq!(
        screen.workspace().file().session.document().charts().len(),
        1
    );
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        SINGLE
    );
    assert_eq!(fs::read(&path).unwrap(), SINGLE);
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(actors.iter().any(|actor| matches!(actor, Actor::Text {content, z:8021,..} if content.contains("dance-double") && !content.contains("{notes}"))));
    press(&mut screen, KeyCode::PageDown);
    press(&mut screen, KeyCode::Home);
    press(&mut screen, KeyCode::Enter); // explicit application
    assert!(screen.generation_report().is_none());
    assert_eq!(screen.workspace().chart(), 1);
    assert_eq!(screen.workspace().grid().columns(), 8);
    assert_eq!(fs::read(&path).unwrap(), SINGLE);
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(screen.workspace().chart(), 0);
    assert_eq!(screen.workspace().grid().columns(), 4);
    ctrl(&mut screen, KeyCode::KeyY);
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 1).unwrap();
    assert_eq!(reopened.workspace().grid().columns(), 8);
    assert_eq!(reopened.workspace().grid().iter().count(), 4);
}
#[test]
fn preview_discard_and_worker_cancel_leave_source_and_history_untouched() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SINGLE).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    start_generation(&mut screen);
    finish_generation(&mut screen);
    assert!(screen.generation_report().is_some());
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    assert!(screen.generation_report().is_none());
    assert!(!screen.workspace().file().session.is_dirty());
    start_generation(&mut screen);
    press(&mut screen, KeyCode::Escape);
    finish_generation(&mut screen);
    assert!(screen.generation_report().is_none());
    assert_eq!(screen.workspace().file().session.revision(), 0);
    assert_eq!(fs::read(&path).unwrap(), SINGLE);
}
#[test]
fn unsupported_generation_reports_failure_without_modifying_multiplayer_chart() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    start_generation(&mut screen);
    finish_generation(&mut screen);
    assert!(screen.generation_report().is_none());
    assert_eq!(screen.workspace().grid().players(), 2);
    assert_eq!(screen.workspace().file().session.revision(), 0);
    assert_eq!(fs::read(&path).unwrap(), SONG);
}
