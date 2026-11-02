//! Migration review through the production keyboard and actor adapter.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn init() {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn key(screen: &mut State, code: KeyCode, pressed: bool) {
    screen.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(screen: &mut State, code: KeyCode) {
    key(screen, code, true);
    key(screen, code, false);
}
fn ctrl(screen: &mut State, code: KeyCode) {
    key(screen, KeyCode::SuperLeft, true);
    press(screen, code);
    key(screen, KeyCode::SuperLeft, false);
}
fn bytes(name: &str) -> Vec<u8> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../deadsync-editor/tests/fixtures/grooveauthor");
    let request: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join(format!("legacy-timing-{name}.request.json"))).unwrap(),
    )
    .unwrap();
    request["Content"].as_str().unwrap().as_bytes().to_vec()
}
fn serialized(screen: &State) -> Vec<u8> {
    screen.workspace().file().session.serialize().unwrap()
}
fn contains(screen: &State, value: &str) -> bool {
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    actors
        .iter()
        .any(|actor| matches!(actor,Actor::Text{content,..} if content.contains(value)))
}
#[test]
fn f10_reviews_local_timing_then_converts_once_undoes_redoes_saves_and_reopens() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let before = bytes("mixed");
    fs::write(&path, &before).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    assert!(contains(&screen, "SSC 0.60 -> 0.70"));
    assert!(contains(&screen, "local timing in 2 charts"));
    assert!(contains(&screen, "written in 1 charts"));
    assert_eq!(serialized(&screen), before);
    assert_eq!(fs::read(&path).unwrap(), before);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "SSC timing converted"));
    let after = serialized(&screen);
    assert_ne!(after, before);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(2), "BPMS")
            .as_deref(),
        Some("0=150")
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), before);
    key(&mut screen, KeyCode::ShiftLeft, true);
    ctrl(&mut screen, KeyCode::KeyZ);
    key(&mut screen, KeyCode::ShiftLeft, false);
    assert_eq!(serialized(&screen), after);
    let output = screen
        .workspace()
        .file()
        .session
        .serialize_for_output()
        .unwrap();
    ctrl(&mut screen, KeyCode::KeyS);
    assert_eq!(fs::read(&path).unwrap(), output);
    let mut reopened = State::open(&path, 0).unwrap();
    press(&mut reopened, KeyCode::F10);
    assert!(contains(&reopened, "Timing: this chart"));
    assert!(!contains(&reopened, "Convert timing and keep editing"));
}
#[test]
fn cancel_and_escape_leave_source_file_and_redo_untouched() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let before = bytes("mixed");
    fs::write(&path, &before).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::Digit2);
    ctrl(&mut screen, KeyCode::KeyZ);
    let redo = screen
        .workspace()
        .file()
        .session
        .redo_label()
        .unwrap()
        .to_owned();
    press(&mut screen, KeyCode::F10);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Timing conversion discarded"));
    assert_eq!(serialized(&screen), before);
    assert_eq!(
        screen.workspace().file().session.redo_label(),
        Some(redo.as_str())
    );
    press(&mut screen, KeyCode::F10);
    press(&mut screen, KeyCode::Escape);
    assert_eq!(serialized(&screen), before);
    assert_eq!(
        screen.workspace().file().session.redo_label(),
        Some(redo.as_str())
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}
#[test]
fn generation_and_properties_route_to_conversion_before_creating_jobs_or_offset_drafts() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let before = bytes("mixed");
    fs::write(&path, &before).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F3);
    for _ in 0..3 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(!screen.is_generating());
    assert!(contains(&screen, "Convert timing and keep editing"));
    assert_eq!(serialized(&screen), before);
    press(&mut screen, KeyCode::Escape);
    press(&mut screen, KeyCode::F7);
    assert!(contains(&screen, "Convert timing and keep editing"));
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::F7);
    assert!(contains(&screen, "Apply chart properties"));
    assert_eq!(fs::read(&path).unwrap(), before);
}
#[test]
fn missing_version_review_inserts_supported_version_and_materializes_inherited_default_tempos() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let before = bytes("missing-version");
    fs::write(&path, &before).unwrap();
    let mut screen = State::open(&path, 2).unwrap();
    press(&mut screen, KeyCode::F10);
    assert!(contains(&screen, "SSC not specified -> 0.83"));
    press(&mut screen, KeyCode::Enter);
    let document = screen.workspace().file().session.document();
    assert_eq!(document.text(None, "VERSION").as_deref(), Some("0.83"));
    assert_eq!(
        document.text(Some(2), "BPMS").as_deref(),
        Some("0=150,4=180")
    );
    assert_eq!(document.text(Some(2), "STOPS").as_deref(), Some("1=0.25"));
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(serialized(&screen), before);
}
