//! Actual keyboard color drafts, live preview, shared persistence and global undo.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::Actor,
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
fn control(s: &mut State, code: KeyCode, shift: bool) {
    key(s, KeyCode::SuperLeft, true);
    if shift {
        key(s, KeyCode::ShiftLeft, true);
    }
    press(s, code);
    key(s, KeyCode::ShiftLeft, false);
    key(s, KeyCode::SuperLeft, false);
}
fn open(locale: &str) -> (tempfile::TempDir, State, Vec<u8>, std::path::PathBuf) {
    i18n::init(deadsync_assets::language::load_for_tests(locale));
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("colors.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1M00\n0000\n0000\n0000;".to_vec();
    fs::write(&song, &source).unwrap();
    let mut state = State::open(&song, 0).unwrap();
    let path = dir.path().join("prefs.json");
    fs::write(
        &path,
        b"{\"Version\":1,\"NoteColor\":{\"Vendor\":42},\"Options\":{\"VendorOption\":17}}",
    )
    .unwrap();
    state.configure_audio_preferences(&path).unwrap();
    (dir, state, source, path)
}
fn panel(s: &mut State) {
    control(s, KeyCode::KeyU, true);
}
fn down(s: &mut State, n: usize) {
    for _ in 0..n {
        press(s, KeyCode::ArrowDown);
    }
}
fn next_field(s: &mut State, n: usize) {
    for _ in 0..n {
        press(s, KeyCode::PageDown);
    }
}
fn head_tint(s: &State) -> [f32; 4] {
    fn find(a: &Actor) -> Option<[f32; 4]> {
        match a {
            Actor::Sprite {
                z: 8004,
                uv_rect: Some(uv),
                tint,
                ..
            } if (uv[0] - 589.0 / 1280.0).abs() < 1e-6 && (uv[1] - 1.0 / 1280.0).abs() < 1e-6 => {
                Some(*tint)
            }
            Actor::Frame { children, .. } => children.iter().find_map(find),
            _ => None,
        }
    }
    let mut a = Vec::new();
    s.actors(&mut a);
    a.iter().find_map(find).expect("source note fill")
}
#[test]
fn rgb_preview_changes_real_partition_and_cancel_restores_without_disk_or_song_edits() {
    let _guard = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let (_dir, mut s, source, path) = open(locale);
        let before = s.note_color_preferences().clone();
        let bytes = fs::read(&path).unwrap();
        let tint = head_tint(&s);
        panel(&mut s);
        down(&mut s, 1);
        press(&mut s, KeyCode::ArrowRight);
        assert!(
            s.note_color_preferences().colors.subdivisions[0][0] > before.colors.subdivisions[0][0]
        );
        assert_ne!(head_tint(&s), tint);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(!s.workspace().file().session.is_dirty());
        press(&mut s, KeyCode::Escape);
        assert_eq!(s.note_color_preferences(), &before);
        assert_eq!(head_tint(&s), tint);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(s.workspace().file().session.undo_label(), None);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    }
}
#[test]
fn rgb_ui_multiplier_and_signature_option_save_as_one_global_undo_action() {
    let _guard = SERIAL.lock().unwrap();
    for locale in ["en", "fr"] {
        let (_dir, mut s, source, path) = open(locale);
        let before = s.note_color_preferences().clone();
        panel(&mut s);
        down(&mut s, 1);
        press(&mut s, KeyCode::ArrowRight);
        next_field(&mut s, 27);
        press(&mut s, KeyCode::ArrowRight);
        next_field(&mut s, 6);
        press(&mut s, KeyCode::ArrowRight);
        let after = s.note_color_preferences().clone();
        assert_ne!(
            after.colors.normal_multiplier,
            before.colors.normal_multiplier
        );
        assert_eq!(after.selection_multiplier, before.selection_multiplier);
        assert_ne!(after.step_color_method, before.step_color_method);
        down(&mut s, 4);
        press(&mut s, KeyCode::Enter);
        assert!(!s.workspace().file().session.is_dirty());
        let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(json["NoteColor"]["Vendor"], 42);
        assert_eq!(json["Options"]["VendorOption"], 17);
        assert_eq!(json["Options"]["StepColorMethodValue"], "Note");
        let rev = s.workspace().file().session.revision();
        panel(&mut s);
        down(&mut s, 5);
        press(&mut s, KeyCode::Enter);
        assert_eq!(
            s.workspace().file().session.revision(),
            rev,
            "no-op Save creates no history"
        );
        control(&mut s, KeyCode::KeyZ, false);
        assert_eq!(s.note_color_preferences(), &before);
        assert_eq!(s.workspace().file().session.undo_label(), None);
        control(&mut s, KeyCode::KeyY, false);
        assert_eq!(s.note_color_preferences(), &after);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    }
}
#[test]
fn owned_disk_conflict_keeps_live_draft_until_cancel_and_does_not_advance_history() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut s, source, path) = open("en");
    let before = s.note_color_preferences().clone();
    panel(&mut s);
    down(&mut s, 1);
    press(&mut s, KeyCode::ArrowRight);
    let preview = s.note_color_preferences().clone();
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    json["NoteColor"]["QuarterColor"] = serde_json::json!({"X":0.8,"Y":0.2,"Z":0.3});
    let external = serde_json::to_vec(&json).unwrap();
    fs::write(&path, &external).unwrap();
    down(&mut s, 4);
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.note_color_preferences(), &preview);
    assert_eq!(fs::read(&path).unwrap(), external);
    assert_eq!(s.workspace().file().session.undo_label(), None);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.note_color_preferences(), &before);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert_eq!(fs::read(&path).unwrap(), external);
}
