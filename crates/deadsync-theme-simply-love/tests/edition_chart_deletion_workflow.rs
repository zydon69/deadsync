use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_gameplay::GameplayAudioCommand;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
const SONG: &str = "#VERSION:0.83;#TITLE:Keep;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#DIFFICULTY:Hard;#METER:9;#NOTES:10000000\n&\n00001000;";
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
fn delete(s: &mut State) {
    key(s, KeyCode::SuperLeft, true);
    key(s, KeyCode::ShiftLeft, true);
    press(s, KeyCode::Backspace);
    key(s, KeyCode::ShiftLeft, false);
    key(s, KeyCode::SuperLeft, false);
}
fn texts(s: &State) -> Vec<String> {
    let mut actors = Vec::new();
    s.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|a| {
            if let Actor::Text { content, .. } = a {
                Some(content.to_string())
            } else {
                None
            }
        })
        .collect()
}
fn setup() -> (tempfile::TempDir, State) {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let state = State::open(&path, 0).unwrap();
    (dir, state)
}
#[test]
fn final_chart_confirmation_cancel_empty_render_undo_redo_and_modal_safety() {
    let (_dir, mut s) = setup();
    delete(&mut s);
    assert!(
        texts(&s)
            .iter()
            .any(|t| t.contains("Delete the last chart"))
    );
    press(&mut s, KeyCode::Enter);
    assert!(s.workspace().has_chart());
    assert!(!s.workspace().file().session.is_dirty());
    press(&mut s, KeyCode::F10);
    delete(&mut s);
    assert_eq!(s.workspace().file().session.document().charts().len(), 1);
    press(&mut s, KeyCode::Escape);
    delete(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(!s.workspace().has_chart());
    let labels = texts(&s);
    assert!(labels.iter().any(|t| t == "No active chart"));
    assert!(!labels.iter().any(|t| t.contains('{')));
    let empty = s.workspace().file().session.serialize().unwrap();
    for code in [
        KeyCode::Digit1,
        KeyCode::Enter,
        KeyCode::PageDown,
        KeyCode::Delete,
        KeyCode::F3,
        KeyCode::F4,
        KeyCode::F5,
        KeyCode::F6,
        KeyCode::F7,
        KeyCode::F8,
        KeyCode::F9,
        KeyCode::F10,
        KeyCode::F12,
    ] {
        press(&mut s, code);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), empty);
        texts(&s);
    }
    press(&mut s, KeyCode::F11);
    texts(&s);
    press(&mut s, KeyCode::Escape);
    assert!(!s.workspace().has_chart());
    assert_eq!(s.workspace().file().session.serialize().unwrap(), empty);
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(s.workspace().has_chart());
    assert_eq!(s.workspace().grid().players(), 2);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        SONG.as_bytes()
    );
    ctrl(&mut s, KeyCode::KeyY);
    assert!(!s.workspace().has_chart());
    ctrl(&mut s, KeyCode::KeyN);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(s.workspace().has_chart());
    assert_eq!(s.workspace().grid().iter().count(), 0);
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(!s.workspace().has_chart());
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(s.workspace().has_chart());
}
#[test]
fn save_reopen_cached_selection_empty_file_menu_and_new_chart_are_usable() {
    let (dir, mut s) = setup();
    delete(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    ctrl(&mut s, KeyCode::KeyS);
    assert!(!s.workspace().file().session.is_dirty());
    let path = dir.path().join("song.ssc");
    let saved = fs::read(&path).unwrap();
    assert!(!String::from_utf8_lossy(&saved).contains("#NOTEDATA"));
    let mut reopened = State::open_selected(&path, "dance-routine", "Hard", "", 9).unwrap();
    assert!(!reopened.workspace().has_chart());
    texts(&reopened);
    press(&mut reopened, KeyCode::F1);
    assert!(texts(&reopened).iter().any(|t| t == "Create empty chart"));
    press(&mut reopened, KeyCode::Enter);
    press(&mut reopened, KeyCode::ArrowDown);
    press(&mut reopened, KeyCode::Enter);
    assert!(reopened.workspace().has_chart());
    press(&mut reopened, KeyCode::Digit2);
    ctrl(&mut reopened, KeyCode::KeyS);
    let edited = State::open(&path, 0).unwrap();
    assert_eq!(edited.workspace().grid().iter().count(), 1);
}
#[test]
fn deletion_stops_audio_and_selects_remaining_chart_then_restores_original_focus() {
    let (dir, _) = setup();
    let path = dir.path().join("music.ssc");
    let song = "#VERSION:0.83;#MUSIC:music.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-double;#DIFFICULTY:Easy;#NOTES:00001000;";
    fs::write(&path, song).unwrap();
    fs::write(dir.path().join("music.ogg"), b"fixture asset").unwrap();
    let mut s = State::open(&path, 1).unwrap();
    press(&mut s, KeyCode::Enter);
    assert!(s.is_playing());
    s.take_audio_commands();
    delete(&mut s);
    assert!(!s.is_playing());
    assert_eq!(
        s.take_audio_commands(),
        vec![GameplayAudioCommand::StopMusic]
    );
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.workspace().chart(), 0);
    assert_eq!(s.workspace().grid().columns(), 4);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().chart(), 1);
    assert_eq!(s.workspace().grid().columns(), 8);
    assert_eq!(fs::read_to_string(path).unwrap(), song);
}
