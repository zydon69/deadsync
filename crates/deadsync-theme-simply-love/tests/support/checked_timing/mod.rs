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
fn open(s: &mut State) {
    key(s, KeyCode::SuperLeft, true);
    key(s, KeyCode::ShiftLeft, true);
    press(s, KeyCode::KeyE);
    key(s, KeyCode::ShiftLeft, false);
    key(s, KeyCode::SuperLeft, false);
}
fn serialized(s: &State) -> Vec<u8> {
    s.workspace().file().session.serialize().unwrap()
}
const SOURCE:&[u8]=b"#VERSION:0.83;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#WARPS:1=1,3=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
fn choose_first_warp(s: &mut State) {
    open(s);
    press(s, KeyCode::F3);
    for _ in 0..8 {
        press(s, KeyCode::ArrowRight);
    } // Misc preset
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::ArrowRight);
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::Enter); // Replace; last match is Warp3
    press(s, KeyCode::Space); // uncheck last warp while keeping Warp1 checked
}
#[test]
fn checked_timing_scope_preserves_unchecked_warp_and_undoes_after_save() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SOURCE).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    choose_first_warp(&mut s);
    press(&mut s, KeyCode::F7);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert_eq!(serialized(&s), SOURCE);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert_eq!(
        s.workspace()
            .file()
            .session
            .document()
            .text(Some(0), "STOPS")
            .as_deref(),
        Some("1=-0.5")
    );
    assert_eq!(
        s.workspace()
            .file()
            .session
            .document()
            .text(Some(0), "WARPS")
            .as_deref(),
        Some("3=1")
    );
    assert_eq!(
        s.workspace()
            .file()
            .session
            .document()
            .text(None, "WARPS")
            .as_deref(),
        Some("1=1,3=1")
    );
    let output = s.workspace()
            .file()
            .session
            .serialize_for_output()
            .unwrap();
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(serialized(&reopened), output);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(serialized(&s), SOURCE);
}
#[test]
fn cancelling_checked_timing_review_returns_to_original_checked_event_picker() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SOURCE).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    choose_first_warp(&mut s);
    press(&mut s, KeyCode::F7);
    press(&mut s, KeyCode::Escape);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let title = i18n::tr("Edition", "EventSelectionTitle");
    assert!(actors.iter().any(|a|matches!(a,deadlib_present::actors::Actor::Text{content,..}if content.as_ref()==title.as_ref())));
    assert_eq!(serialized(&s), SOURCE);
    assert!(!s.workspace().file().session.is_dirty());
    // Picker identities survived Cancel: deleting checked miscellaneous events
    // leaves the unchecked Warp at3 and the existing initial BPM untouched.
    press(&mut s, KeyCode::Delete);
    let document=s.workspace().file().session.document();
    let origin=deadsync_editor::timing_edit::uses_chart_timing(document,0).then_some(0);
    assert_eq!(
        s.workspace()
            .file()
            .session
            .document()
            .text(origin, "WARPS")
            .as_deref(),
        Some("3=1")
    );
    assert_eq!(
        s.workspace()
            .file()
            .session
            .document()
            .text(origin, "BPMS")
            .as_deref(),
        Some("0=120")
    );
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(serialized(&s), SOURCE);
}
