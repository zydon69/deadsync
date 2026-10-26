use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::{
    attacks::Draft,
    document::{Document, Format},
    history::Session,
    timing_edit::Scope,
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn press(s: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        s.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
fn ctrl(s: &mut State, code: KeyCode) {
    s.raw_key(&RawKeyboardEvent {
        code: KeyCode::SuperLeft,
        pressed: true,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    press(s, code);
    s.raw_key(&RawKeyboardEvent {
        code: KeyCode::SuperLeft,
        pressed: false,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn open(s: &mut State) {
    press(s, KeyCode::F10);
    // Explicitly select shared scope; the default follows timing's chart scope in SSC.
    if s.workspace().file().session.document().format == Format::Ssc {
        press(s, KeyCode::ArrowLeft);
    }
    press(s, KeyCode::F6);
}
fn row(s: &mut State, index: usize) {
    press(s, KeyCode::PageUp);
    press(s, KeyCode::PageUp);
    for _ in 0..index {
        press(s, KeyCode::ArrowDown);
    }
}
fn field(s: &mut State, index: usize, value: &str) {
    row(s, index);
    ctrl(s, KeyCode::KeyA);
    assert!(s.text_input(value));
}
fn action(s: &mut State, index: usize) {
    row(s, index);
    press(s, KeyCode::Enter);
}
fn read(s: &State) -> Draft {
    Draft::capture(&s.workspace().file().session, 0, Scope::Song).unwrap()
}
#[test]
fn attacks_keyboard_edit_multiple_modifiers_create_delete_cancel_undo_redo_save_and_reopen() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    for format in [Format::Sm, Format::Ssc] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(if format == Format::Sm {
            "song.sm"
        } else {
            "song.ssc"
        });
        let source: &[u8] = if format == Format::Sm {
            b"#TITLE:Source;#BPMS:0=120;#ATTACKS:TIME=0:LEN=2:MODS=100% Dizzy,50% Drunk;#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;"
        } else {
            b"#VERSION:0.83;#TITLE:Source;#BPMS:0=120;#ATTACKS:TIME=0:LEN=2:MODS=100% Dizzy,50% Drunk;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;"
        };
        fs::write(&path, source).unwrap();
        let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
        open(&mut s);
        field(&mut s, 4, "Cancelled");
        press(&mut s, KeyCode::Escape);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
        open(&mut s);
        // Creating an attack at an occupied cursor must not append a modifier silently.
        action(&mut s, 10);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
        // Move to the existing modifier's end before adding a distinct attack at the cursor.
        press(&mut s, KeyCode::F4);
        action(&mut s, 10);
        field(&mut s, 3, "3.5");
        field(&mut s, 4, "Écran clair");
        field(&mut s, 5, "-25.5");
        field(&mut s, 6, "2");
        field(&mut s, 7, "3.25");
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
        action(&mut s, 8);
        field(&mut s, 4, "Reverse");
        field(&mut s, 5, "150");
        field(&mut s, 6, "0.5");
        field(&mut s, 7, "8");
        row(&mut s, 4);
        let mut actors = Vec::new();
        s.actors(&mut actors);
        assert!(actors.iter().any(|actor|matches!(actor,deadlib_present::actors::Actor::Text{content,..} if content.contains("Nom du modificateur :"))));
        assert!(!actors.iter().any(|actor|matches!(actor,deadlib_present::actors::Actor::Text{content,..} if content.contains("{value}"))));
        action(&mut s, 12);
        let draft = read(&s);
        let events = draft.attacks().events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].time, 3.5);
        assert_eq!(events[1].modifiers.len(), 2);
        let first = &events[1].modifiers[0];
        assert_eq!(first.name, "Écran clair");
        assert_eq!(first.level, -25.5);
        assert_eq!(first.speed, 2.0);
        assert_eq!(first.length, 3.25);
        let second = &events[1].modifiers[1];
        assert_eq!(second.name, "Reverse");
        assert_eq!(second.level, 150.0);
        assert_eq!(second.speed, 0.5);
        assert_eq!(second.length, 8.0);
        let after = s.workspace().file().session.serialize().unwrap();
        ctrl(&mut s, KeyCode::KeyZ);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
        ctrl(&mut s, KeyCode::KeyY);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), after);
        ctrl(&mut s, KeyCode::KeyS);
        assert_eq!(fs::read(&path).unwrap(), after);
        let reopened = Session::new(Document::parse(after.clone(), format).unwrap());
        assert_eq!(
            Draft::capture(&reopened, 0, Scope::Song).unwrap().attacks(),
            draft.attacks()
        );
        // Delete one modifier and then one complete attack as a single applied gesture.
        open(&mut s);
        row(&mut s, 1);
        press(&mut s, KeyCode::ArrowRight);
        action(&mut s, 9);
        action(&mut s, 11);
        action(&mut s, 12);
        assert_eq!(read(&s).attacks().events().len(), 1);
        ctrl(&mut s, KeyCode::KeyZ);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), after);
    }
}
#[test]
fn invalid_attack_fields_remain_repairable_and_scope_switch_cannot_discard_pending_changes() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=0:LEN=2:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    open(&mut s);
    field(&mut s, 7, "-1");
    action(&mut s, 12);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    field(&mut s, 7, "5");
    row(&mut s, 0);
    press(&mut s, KeyCode::ArrowRight);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    // The rejected switch retains the song draft and its pending duration.
    action(&mut s, 12);
    assert_eq!(read(&s).attacks().events()[0].modifiers[0].length, 5.0);
    ctrl(&mut s, KeyCode::KeyZ);
    open(&mut s);
    row(&mut s, 0);
    press(&mut s, KeyCode::ArrowRight);
    field(&mut s, 5, "25");
    action(&mut s, 12);
    assert_eq!(read(&s).attacks().events()[0].modifiers[0].level, 100.0);
    let local = Draft::capture(&s.workspace().file().session, 0, Scope::Chart).unwrap();
    assert_eq!(local.attacks().events()[0].modifiers[0].level, 25.0);
    assert!(
        s.workspace()
            .file()
            .session
            .document()
            .raw_property(Some(0), "BPMS")
            .is_none()
    );
}

#[test]
fn attack_timeline_shortcuts_set_duration_and_jump_without_audio_or_early_document_edits() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#OFFSET:0.25;#BPMS:0=120;#ATTACKS:TIME=0.75:LEN=1:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F4);
    assert_eq!(s.workspace().row(), 192);
    field(&mut s, 7, "5");
    press(&mut s, KeyCode::F3);
    press(&mut s, KeyCode::F5);
    assert_eq!(s.workspace().row(), 96);
    press(&mut s, KeyCode::F2);
    press(&mut s, KeyCode::F3);
    press(&mut s, KeyCode::F4);
    assert_eq!(s.workspace().row(), 96);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    action(&mut s, 12);
    assert_eq!(read(&s).attacks().events()[0].time, 0.75);
    assert_eq!(read(&s).attacks().events()[0].modifiers[0].length, 0.0);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}

#[derive(Default)]
struct Clipboard {
    value: String,
    reads: usize,
}
impl deadsync_editor::clipboard::TextClipboard for Clipboard {
    fn read_text(&mut self) -> Result<String, deadsync_editor::clipboard::Error> {
        self.reads += 1;
        Ok(self.value.clone())
    }
    fn write_text(&mut self, value: &str) -> Result<(), deadsync_editor::clipboard::Error> {
        self.value = value.into();
        Ok(())
    }
}
#[test]
fn empty_attack_list_creation_and_clipboard_requests_remain_bound_to_the_active_field() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    open(&mut s);
    row(&mut s, 4);
    assert!(!s.wants_text_input());
    assert!(!s.text_input("Drunk"));
    action(&mut s, 10);
    field(&mut s, 4, "Drunk");
    ctrl(&mut s, KeyCode::KeyA);
    ctrl(&mut s, KeyCode::KeyC);
    let mut clipboard = Clipboard::default();
    assert!(s.process_text_clipboard(&mut clipboard));
    assert_eq!(clipboard.value, "Drunk");
    clipboard.value = "Reverse".into();
    ctrl(&mut s, KeyCode::KeyV);
    press(&mut s, KeyCode::ArrowDown);
    assert!(!s.process_text_clipboard(&mut clipboard));
    assert_eq!(clipboard.reads, 0);
    row(&mut s, 4);
    ctrl(&mut s, KeyCode::KeyA);
    ctrl(&mut s, KeyCode::KeyV);
    assert!(s.process_text_clipboard(&mut clipboard));
    assert_eq!(clipboard.reads, 1);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    action(&mut s, 12);
    let created = read(&s);
    assert_eq!(created.attacks().events()[0].time, 0.0);
    assert_eq!(created.attacks().events()[0].modifiers[0].name, "Reverse");
    assert_eq!(created.attacks().events()[0].modifiers[0].length, 0.5);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}

#[test]
fn timing_menu_entry_and_shortcut_preserve_pending_timing_before_opening_attacks() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    press(&mut s, KeyCode::F10);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::F6);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    assert!(actors.iter().any(|actor|matches!(actor,deadlib_present::actors::Actor::Text{content,..} if content.contains("Appliquez ou annulez les modifications de timing"))));
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::F10);
    for _ in 0..10 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    action(&mut s, 10);
    field(&mut s, 4, "Drunk");
    action(&mut s, 12);
    let local = Draft::capture(&s.workspace().file().session, 0, Scope::Chart).unwrap();
    assert_eq!(local.attacks().events()[0].modifiers[0].name, "Drunk");
    assert!(read(&s).attacks().events().is_empty());
    assert_eq!(
        s.workspace()
            .file()
            .session
            .document()
            .text(None, "BPMS")
            .as_deref(),
        Some("0=120")
    );
    assert!(
        s.workspace()
            .file()
            .session
            .document()
            .raw_property(Some(0), "BPMS")
            .is_none()
    );
}

#[test]
fn attack_transport_uses_resident_rows_with_delay_and_stop_and_preserves_them_on_apply() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#DELAYS:4=1;#STOPS:8=1;#ATTACKS:TIME=5.25:LEN=1:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000,0100;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F5);
    assert_eq!(s.workspace().row(), 312);
    press(&mut s, KeyCode::F4);
    assert_eq!(s.workspace().row(), 384);
    press(&mut s, KeyCode::F3);
    field(&mut s, 4, "Reverse");
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    action(&mut s, 12);
    let session = &s.workspace().file().session;
    let placed = deadsync_editor::attacks::placement::Timeline::capture(session, 0).unwrap();
    assert_eq!(placed.markers()[0].row, 312);
    assert_eq!(placed.markers()[0].modifiers[0].length, 0.75);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}

#[test]
fn empty_attack_keeps_time_editing_keyboard_transport_save_reopen_and_independent_event_deletion() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    for format in [Format::Sm, Format::Ssc] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(if format == Format::Sm {
            "song.sm"
        } else {
            "song.ssc"
        });
        let notes = if format == Format::Sm {
            "#NOTES:dance-single::Hard:9::1000;"
        } else {
            "#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;"
        };
        let source =
            format!("#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=1:LEN=2:MODS=100% Dizzy;{notes}")
                .into_bytes();
        fs::write(&path, &source).unwrap();
        let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
        open(&mut s);
        field(&mut s, 3, "2.25");
        field(&mut s, 4, "123 invalid name");
        action(&mut s, 9);
        press(&mut s, KeyCode::F5);
        assert_eq!(s.workspace().row(), 216);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
        row(&mut s, 4);
        assert!(!s.wants_text_input());
        assert!(!s.text_input("Unexpected"));
        press(&mut s, KeyCode::F3);
        press(&mut s, KeyCode::F4);
        assert_eq!(s.workspace().row(), 216);
        field(&mut s, 3, "3");
        press(&mut s, KeyCode::F2);
        field(&mut s, 3, "NaN");
        press(&mut s, KeyCode::F2);
        press(&mut s, KeyCode::F5);
        assert_eq!(s.workspace().row(), 216);
        action(&mut s, 12);
        let empty = s.workspace().file().session.serialize().unwrap();
        let draft = read(&s);
        assert_eq!(draft.attacks().events().len(), 1);
        assert_eq!(draft.attacks().events()[0].time, 2.25);
        assert!(draft.attacks().events()[0].modifiers.is_empty());
        ctrl(&mut s, KeyCode::KeyS);
        let mut reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
        ctrl(&mut reopened, KeyCode::KeyJ);
        assert_eq!(reopened.workspace().row(), 216);
        ctrl(&mut reopened, KeyCode::Enter);
        row(&mut reopened, 3);
        assert!(reopened.wants_text_input());
        press(&mut reopened, KeyCode::Escape);
        open(&mut s);
        action(&mut s, 8);
        action(&mut s, 12);
        assert_eq!(read(&s).attacks().events()[0].modifiers[0].length, 0.5);
        ctrl(&mut s, KeyCode::KeyZ);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), empty);
        open(&mut s);
        action(&mut s, 11);
        action(&mut s, 12);
        assert!(read(&s).attacks().events().is_empty());
        ctrl(&mut s, KeyCode::KeyZ);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), empty);
        ctrl(&mut s, KeyCode::KeyZ);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    }
}
#[test]
fn new_attack_duration_is_one_beat_at_active_tempo_and_added_modifiers_reuse_first_length() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120,4=240;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    open(&mut s);
    action(&mut s, 10);
    action(&mut s, 12);
    assert_eq!(read(&s).attacks().events()[0].modifiers[0].length, 0.5);
    open(&mut s);
    field(&mut s, 7, "3");
    action(&mut s, 8);
    press(&mut s, KeyCode::F4);
    assert_eq!(s.workspace().row(), 384);
    action(&mut s, 10);
    action(&mut s, 12);
    let draft = read(&s);
    let events = draft.attacks().events();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].modifiers.len(), 2);
    assert_eq!(events[0].modifiers[1].length, 3.0);
    assert_eq!(events[1].modifiers[0].length, 0.25);
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    assert_eq!(read(&reopened).attacks(), draft.attacks());
}
