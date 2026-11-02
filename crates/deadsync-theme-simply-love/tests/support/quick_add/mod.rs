use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn init() {
    if super::FRENCH {
        i18n::init(deadsync_assets::language::load_for_tests("fr"));
    } else {
        i18n::init_for_tests();
    }
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
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
fn serialized(s: &State) -> Vec<u8> {
    s.workspace().file().session.serialize().unwrap()
}
const SOURCE: &[u8] = b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;";
fn text(s: &State, key: &str) -> bool {
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let expected = i18n::tr("Edition", key);
    actors.iter().any(|a|matches!(a,deadlib_present::actors::Actor::Text{content,..}if content.as_ref()==expected.as_ref()))
}
fn quick(s: &mut State) {
    press(s, KeyCode::F10);
    press(s, KeyCode::F5);
}
fn prepare(s: &mut State) {
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::Enter);
}
#[test]
fn source_speed_default_add_is_reviewed_saved_and_undoable() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SOURCE).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    quick(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    prepare(&mut s);
    assert!(text(&s, "QuickAddPrepared"));
    assert_eq!(serialized(&s), SOURCE);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(text(&s, "QuickAddApplied"));
    let d = deadsync_editor::timing_edit::speed::Draft::capture(
        &s.workspace().file().session,
        0,
        deadsync_editor::timing_edit::Scope::Chart,
    )
    .unwrap();
    assert_eq!(
        d.speeds().values()[&0].period,
        deadsync_editor::timing_edit::speed::Period::Rows(48)
    );
    let after = serialized(&s);
    ctrl(&mut s, KeyCode::KeyS);
    assert_eq!(serialized(&State::open(&path, 0).unwrap()), after);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(serialized(&s), SOURCE);
}
#[test]
fn occupied_row_is_refused_in_local_language_and_cannot_apply() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SOURCE).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    quick(&mut s);
    prepare(&mut s);
    assert!(text(&s, "QuickAddOccupied"));
    assert_eq!(serialized(&s), SOURCE);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(text(&s, "NoteOperationPrepareFirst"));
    assert_eq!(serialized(&s), SOURCE);
}
#[test]
fn cancelling_prepared_quick_add_makes_no_history_gesture() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SOURCE).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    quick(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    prepare(&mut s);
    assert!(text(&s, "QuickAddPrepared"));
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    assert_eq!(serialized(&s), SOURCE);
}
