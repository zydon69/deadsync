use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::chart_text::{Field, Text};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
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
fn texts(s: &State) -> Vec<String> {
    let mut a = Vec::new();
    s.actors(&mut a);
    a.into_iter()
        .filter_map(|a| {
            if let Actor::Text { content, .. } = a {
                Some(content.to_string())
            } else {
                None
            }
        })
        .collect()
}
fn has(s: &State, value: &str) -> bool {
    texts(s).iter().any(|text| text.contains(value))
}
fn init() {
    i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn open(s: &mut State) {
    press(s, KeyCode::F7);
    press(s, KeyCode::ArrowUp);
    press(s, KeyCode::ArrowUp);
    press(s, KeyCode::Enter);
}
fn apply(s: &mut State) {
    press(s, KeyCode::Escape);
    for _ in 0..6 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
}
fn text(s: &State, field: Field) -> String {
    Text::read(s.workspace().file().session.document(), 0)
        .unwrap()
        .get(field)
        .to_owned()
}
const SSC: &str = "#VERSION:0.83;#BPMS:0=120;#CREDIT:Song;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#CHARTNAME:Old;#DESCRIPTION:Desc;#NOTES:1000;";
#[test]
fn unicode_ime_reserved_characters_fields_stage_once_and_persist_with_undo_redo() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    open(&mut s);
    assert!(s.wants_text_input());
    s.text_preedit("日本");
    assert!(has(&s, "Name: 日本"));
    press(&mut s, KeyCode::Enter);
    assert!(s.wants_text_input());
    assert!(s.text_input("Été 日本語 🎵"));
    assert!(has(&s, "Name: Été 日本語 🎵"));
    assert_eq!(text(&s, Field::Name), "Old");
    assert!(s.text_input("!"));
    press(&mut s, KeyCode::Backspace);
    assert!(has(&s, "Name: Été 日本語 🎵"));
    key(&mut s, KeyCode::SuperLeft, true);
    assert!(!s.text_input("shortcut"));
    key(&mut s, KeyCode::SuperLeft, false);
    press(&mut s, KeyCode::ArrowDown);
    assert!(s.text_input("A:B;C\\D // E=F,#G"));
    press(&mut s, KeyCode::ArrowDown);
    assert!(s.text_input("Zydon"));
    press(&mut s, KeyCode::ArrowDown);
    assert!(s.text_input("Pad"));
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        SSC.as_bytes()
    );
    apply(&mut s);
    assert!(!s.wants_text_input());
    for (field, value) in
        Field::ALL
            .into_iter()
            .zip(["Été 日本語 🎵", "A:B;C\\D // E=F,#G", "Zydon", "Pad"])
    {
        assert_eq!(text(&s, field), value);
    }
    let after = s.workspace().file().session.serialize().unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), SSC);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        SSC.as_bytes()
    );
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), after);
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(text(&reopened, Field::Name), "Été 日本語 🎵");
    assert_eq!(text(&reopened, Field::Description), "A:B;C\\D // E=F,#G");
}
#[test]
fn cancel_controls_back_navigation_and_empty_credit_follow_source_persistence() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    open(&mut s);
    assert!(!s.text_input("\n\t"));
    assert!(s.text_input("discard"));
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        SSC.as_bytes()
    );
    assert!(!s.text_input("outside focus"));
    open(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Delete);
    assert!(has(&s, "Credit: "));
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    assert!(!s.wants_text_input());
    press(&mut s, KeyCode::Enter);
    for _ in 0..6 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(text(&reopened, Field::Credit), "Song");
    assert_eq!(reopened.workspace().grid().iter().count(), 1);
}
#[test]
fn sm_focus_skips_unavailable_fields_and_description_is_escaped_and_saved() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.sm");
    let song = "#BPMS:0=120;#NOTES2:dance-single:Old:Hard:9::1000;";
    fs::write(&path, song).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    open(&mut s);
    assert!(s.wants_text_input());
    assert!(has(&s, "Name: Unavailable in SM"));
    assert!(s.text_input("Été : 日本語"));
    press(&mut s, KeyCode::ArrowDown);
    assert!(!s.wants_text_input());
    press(&mut s, KeyCode::ArrowUp);
    assert!(s.wants_text_input());
    apply(&mut s);
    assert_eq!(text(&s, Field::Description), "Été : 日本語");
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(text(&reopened, Field::Description), "Été : 日本語");
}

#[test]
fn cursor_selection_ime_and_local_undo_edit_the_middle_without_touching_document_history() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    open(&mut s);
    assert!(has(&s, "Name: [Old]|"));
    press(&mut s, KeyCode::Home);
    press(&mut s, KeyCode::ArrowRight);
    s.text_preedit("日");
    assert!(has(&s, "Name: O日|ld"));
    press(&mut s, KeyCode::ArrowLeft);
    assert!(s.text_input("日本"));
    assert!(has(&s, "Name: O日本|ld"));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(has(&s, "Name: O|ld"));
    ctrl(&mut s, KeyCode::KeyY);
    assert!(has(&s, "Name: O日本|ld"));
    key(&mut s, KeyCode::ShiftLeft, true);
    press(&mut s, KeyCode::Home);
    key(&mut s, KeyCode::ShiftLeft, false);
    assert!(has(&s, "Name: |[O日本]ld"));
    assert!(s.text_input("É"));
    assert!(has(&s, "Name: É|ld"));
    press(&mut s, KeyCode::Delete);
    assert!(has(&s, "Name: É|d"));
    ctrl(&mut s, KeyCode::KeyA);
    assert!(s.text_input("e\u{301}👩‍👩‍👧‍👦🇫🇷"));
    press(&mut s, KeyCode::Home);
    press(&mut s, KeyCode::Delete);
    assert!(has(&s, "Name: |👩‍👩‍👧‍👦🇫🇷"));
    press(&mut s, KeyCode::End);
    press(&mut s, KeyCode::Backspace);
    assert!(has(&s, "Name: 👩‍👩‍👧‍👦|"));
    press(&mut s, KeyCode::Tab);
    assert!(has(&s, "Description: [Desc]|"));
    key(&mut s, KeyCode::ShiftLeft, true);
    press(&mut s, KeyCode::Tab);
    key(&mut s, KeyCode::ShiftLeft, false);
    assert!(has(&s, "Name: [👩‍👩‍👧‍👦]|"));
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        SSC.as_bytes()
    );
    apply(&mut s);
    assert_eq!(text(&s, Field::Name), "👩‍👩‍👧‍👦");
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        SSC.as_bytes()
    );
}

#[test]
fn long_text_viewport_follows_repeated_cursor_and_preserves_unfocused_literal_pipes() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    open(&mut s);
    let long = format!("Début{}Fin||", "日本語".repeat(40));
    assert!(s.text_input(&long));
    assert!(has(&s, "Name: ..."));
    assert!(has(&s, "Fin|||"));
    press(&mut s, KeyCode::Home);
    assert!(has(&s, "Name: |Début"));
    s.raw_key(&RawKeyboardEvent {
        code: KeyCode::ArrowRight,
        pressed: true,
        repeat: true,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    key(&mut s, KeyCode::ArrowRight, false);
    assert!(has(&s, "Name: D|ébut"));
    press(&mut s, KeyCode::Tab);
    assert!(has(&s, "Fin||"));
    assert!(!has(&s, "Fin|||"));
    apply(&mut s);
    assert_eq!(text(&s, Field::Name), long);
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(text(&reopened, Field::Name), long);
}

#[test]
fn word_navigation_selection_and_field_redo_shortcuts_remain_modal() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SSC).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    open(&mut s);
    assert!(s.text_input("one two three"));
    ctrl(&mut s, KeyCode::ArrowLeft);
    assert!(has(&s, "Name: one two |three"));
    ctrl(&mut s, KeyCode::Backspace);
    assert!(has(&s, "Name: one |three"));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(has(&s, "Name: one two |three"));
    key(&mut s, KeyCode::SuperLeft, true);
    key(&mut s, KeyCode::ShiftLeft, true);
    press(&mut s, KeyCode::KeyZ);
    key(&mut s, KeyCode::ShiftLeft, false);
    key(&mut s, KeyCode::SuperLeft, false);
    assert!(has(&s, "Name: one |three"));
    let mut actors = Vec::new();
    s.actors(&mut actors);
    assert!(actors.iter().any(|a| matches!(a, Actor::Text {content,max_width:Some(width),..} if content.to_string().starts_with("Name:") && *width==490.0)));
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        SSC.as_bytes()
    );
}

#[test]
fn legacy_multiline_field_rejects_partial_invalid_edits_without_losing_other_staged_fields() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let song = SSC.replace("#DESCRIPTION:Desc;", "#DESCRIPTION:Line1\nLine2;");
    fs::write(&path, &song).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    open(&mut s);
    assert!(s.text_input("New name"));
    press(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::End);
    assert!(s.text_input("invalid"));
    assert!(has(&s, "This edit contains a line break"));
    press(&mut s, KeyCode::Tab);
    assert!(s.wants_text_input());
    assert!(has(&s, "Description:"));
    press(&mut s, KeyCode::Escape);
    assert!(has(&s, "Invalid field edit discarded"));
    for _ in 0..6 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    assert_eq!(text(&s, Field::Name), "New name");
    assert_eq!(text(&s, Field::Description), "Line1\nLine2");
    open(&mut s);
    press(&mut s, KeyCode::Tab);
    assert!(s.text_input("Single line"));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(has(&s, "Description: [Line1\\nLine2]|"));
    apply(&mut s);
    assert_eq!(text(&s, Field::Description), "Line1\nLine2");
}
