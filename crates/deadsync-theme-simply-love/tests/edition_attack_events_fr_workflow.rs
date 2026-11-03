include!("support/chart_order_transport.rs");
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
    if code == KeyCode::Tab { complete_chart_navigation(s); }
}
fn open(s: &mut State) {
    press(s, KeyCode::F10);
    // Native resident timing is chart-scoped in both SM and SSC. These cases
    // intentionally edit the shared song attacks, so select that scope.
    press(s, KeyCode::ArrowLeft);
    press(s, KeyCode::F6);
    // Source attack memory starts without a selected event. These legacy
    // editing scenarios explicitly choose the first shared event when needed.
    if s.workspace().file().session.selected_attack_row(s.workspace().chart(), Scope::Song)
        .is_none()
        && !read(s).attacks().events().is_empty()
    {
        row(s, 1);
        press(s, KeyCode::ArrowRight);
        row(s, 0);
    }
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
    let accepted = s.text_input(value);
    if !accepted {
        let mut actors = Vec::new();
        s.actors(&mut actors);
        let labels: Vec<_> = actors
            .iter()
            .filter_map(|actor| match actor {
                deadlib_present::actors::Actor::Text { content, .. } => Some(content.as_ref()),
                _ => None,
            })
            .collect();
        panic!(
            "attack field{index} rejected {value:?}; text focus={}, labels={labels:?}",
            s.wants_text_input()
        );
    }
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
        let output = s.workspace().file().session.serialize_for_output().unwrap();
        ctrl(&mut s, KeyCode::KeyS);
        assert_eq!(fs::read(&path).unwrap(), output);
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
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
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
    row(&mut s, 1);
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
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
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
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
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
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
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
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
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
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
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

#[test]
fn attack_event_clipboard_crosses_charts_with_destination_clock_conflicts_and_one_undo() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clipboard.ssc");
    let source = b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=0.5:LEN=2:MODS=100% Dizzy,50% Drunk;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Challenge;#METER:10;#BPMS:0=240;#ATTACKS:TIME=1:LEN=1:MODS=100% Reverse;#NOTES:0100;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    press(&mut s, KeyCode::F10);
    press(&mut s, KeyCode::F6);
    row(&mut s, 1);
    press(&mut s, KeyCode::ArrowRight);
    field(&mut s, 4, "Écran clair");
    row(&mut s, 1);
    ctrl(&mut s, KeyCode::KeyC);
    // Copy includes staged fields without applying them to the source chart.
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::Tab);
    assert_eq!(s.workspace().chart(), 1);
    ctrl(&mut s, KeyCode::KeyJ);
    assert_eq!(s.workspace().row(), 192);
    ctrl(&mut s, KeyCode::Enter);
    row(&mut s, 1);
    ctrl(&mut s, KeyCode::KeyV);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    action(&mut s, 12);
    let session = &s.workspace().file().session;
    let target = deadsync_editor::attacks::placement::Timeline::capture(session, 1).unwrap();
    assert_eq!(target.markers().len(), 1);
    assert_eq!(target.markers()[0].row, 192);
    assert_eq!(target.markers()[0].chart_time, 1.0);
    assert_eq!(target.markers()[0].modifiers.len(), 2);
    assert_eq!(target.markers()[0].modifiers[0].name, "Écran clair");
    assert_eq!(target.markers()[0].modifiers[1].name, "Drunk");
    assert_eq!(
        deadsync_editor::attacks::placement::Timeline::capture(session, 0)
            .unwrap()
            .markers()[0]
            .modifiers[0]
            .name,
        "Dizzy"
    );
    let pasted = session.serialize().unwrap();
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), pasted);
    let output = s.workspace().file().session.serialize_for_output().unwrap();
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open_selected(&path, "dance-single", "Challenge", "", 10).unwrap();
    assert_eq!(
        reopened.workspace().file().session.serialize_for_output().unwrap(),
        output
    );
    // Cutting the whole event remains staged, and cancelling restores it.
    ctrl(&mut s, KeyCode::Enter);
    row(&mut s, 1);
    ctrl(&mut s, KeyCode::KeyX);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), pasted);
    ctrl(&mut s, KeyCode::Enter);
    row(&mut s, 1);
    ctrl(&mut s, KeyCode::KeyX);
    // Paste after cut at the same cursor restores the snapshot without losing modifiers.
    ctrl(&mut s, KeyCode::KeyV);
    action(&mut s, 12);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), pasted);
}

#[test]
fn attack_range_selection_copy_cut_delete_and_select_all_are_atomic_keyboard_gestures() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    fn extend(s: &mut State, code: KeyCode) {
        s.raw_key(&RawKeyboardEvent {
            code: KeyCode::ShiftLeft,
            pressed: true,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
        press(s, code);
        s.raw_key(&RawKeyboardEvent {
            code: KeyCode::ShiftLeft,
            pressed: false,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("range.sm");
    let source = b"#BPMS:0=120;#ATTACKS:TIME=0:LEN=1:MODS=100% Dizzy:TIME=1:LEN=2:MODS=50% Drunk:TIME=2:LEN=3:MODS=100% Reverse;#NOTES:dance-single::Hard:9::1000,0100;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    open(&mut s);
    row(&mut s, 1);
    ctrl(&mut s, KeyCode::KeyV); // Empty clipboard must leave the draft intact.
    extend(&mut s, KeyCode::ArrowRight);
    extend(&mut s, KeyCode::ArrowRight);
    extend(&mut s, KeyCode::ArrowLeft); // Shrink back to the first two attacks.
    let mut actors = Vec::new();
    s.actors(&mut actors);
    assert!(actors.iter().any(|a| matches!(a, deadlib_present::actors::Actor::Text { content, .. } if content.contains("[2 sélectionnées]"))));
    ctrl(&mut s, KeyCode::KeyC);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::PageDown);
    assert_eq!(s.workspace().row(), 192);
    open(&mut s);
    row(&mut s, 1);
    ctrl(&mut s, KeyCode::KeyV);
    action(&mut s, 12);
    let placed =
        deadsync_editor::attacks::placement::Timeline::capture(&s.workspace().file().session, 0)
            .unwrap();
    assert_eq!(
        placed.markers().iter().map(|m| m.row).collect::<Vec<_>>(),
        [0, 96, 192, 288]
    );
    assert_eq!(placed.markers()[2].modifiers[0].name, "Dizzy");
    assert_eq!(placed.markers()[3].modifiers[0].name, "Drunk");
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    open(&mut s);
    row(&mut s, 1);
    for _ in 0..3 {
        if s.workspace()
            .file()
            .session
            .selected_attack_row(0, Scope::Song)
            == Some(0)
        {
            break;
        }
        press(&mut s, KeyCode::ArrowLeft);
    }
    assert_eq!(
        s.workspace()
            .file()
            .session
            .selected_attack_row(0, Scope::Song),
        Some(0)
    );
    extend(&mut s, KeyCode::ArrowRight);
    ctrl(&mut s, KeyCode::KeyX);
    action(&mut s, 12);
    assert_eq!(read(&s).attacks().events().len(), 1);
    assert_eq!(read(&s).attacks().events()[0].modifiers[0].name, "Reverse");
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    open(&mut s);
    row(&mut s, 1);
    ctrl(&mut s, KeyCode::KeyA);
    action(&mut s, 11);
    action(&mut s, 12);
    assert!(read(&s).attacks().events().is_empty());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    // A staged time edit may reorder the focused event; selection follows it.
    open(&mut s);
    row(&mut s, 1);
    extend(&mut s, KeyCode::ArrowRight);
    field(&mut s, 3, "3");
    row(&mut s, 1);
    ctrl(&mut s, KeyCode::KeyC);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    open(&mut s);
    row(&mut s, 1);
    ctrl(&mut s, KeyCode::KeyV);
    action(&mut s, 12);
    let placed =
        deadsync_editor::attacks::placement::Timeline::capture(&s.workspace().file().session, 0)
            .unwrap();
    assert_eq!(
        placed.markers().iter().map(|m| m.row).collect::<Vec<_>>(),
        [0, 96, 192, 480]
    );
    assert_eq!(placed.markers()[3].modifiers[0].name, "Drunk");
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    // An ordinary navigation step replaces the range with the focused event.
    open(&mut s);
    row(&mut s, 1);
    ctrl(&mut s, KeyCode::KeyA);
    press(&mut s, KeyCode::ArrowRight);
    ctrl(&mut s, KeyCode::KeyX);
    action(&mut s, 12);
    assert_eq!(read(&s).attacks().events().len(), 2);
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    assert_eq!(read(&reopened).attacks(), read(&s).attacks());
}

#[test]
fn mixed_event_clipboard_shortcuts_cross_charts_and_undo_notes_timing_and_attacks_together() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    fn mixed(s: &mut State, code: KeyCode) {
        s.raw_key(&RawKeyboardEvent {
            code: KeyCode::ShiftLeft,
            pressed: true,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
        ctrl(s, code);
        s.raw_key(&RawKeyboardEvent {
            code: KeyCode::ShiftLeft,
            pressed: false,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixed.ssc");
    let source = b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#BPMS:0=120,4=180;#ATTACKS:TIME=1:LEN=1:MODS=100% Drunk;#NOTES:1000,0200,0300;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Challenge;#METER:10;#BPMS:0=90;#ATTACKS:TIME=0:LEN=1:MODS=100% Reverse;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    press(&mut s, KeyCode::Space);
    press(&mut s, KeyCode::PageDown);
    mixed(&mut s, KeyCode::KeyC);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::Home);
    press(&mut s, KeyCode::PageDown);
    press(&mut s, KeyCode::PageDown);
    assert_eq!(s.workspace().row(), 384);
    mixed(&mut s, KeyCode::KeyV);
    let session = &s.workspace().file().session;
    assert_eq!(
        session.document().text(Some(1), "BPMS").unwrap(),
        "0=90,8=120,12=180"
    );
    let attacks = deadsync_editor::attacks::placement::Timeline::capture(session, 1).unwrap();
    assert_eq!(
        attacks.markers().iter().map(|m| m.row).collect::<Vec<_>>(),
        [0, 480]
    );
    let saved = session.serialize().unwrap();
    let mut parsed = Session::new(Document::parse(saved.clone(), Format::Ssc).unwrap());
    let position = deadsync_editor::notes::Position {
        row: 576,
        lane: 1,
        player: 0,
    };
    assert_eq!(
        parsed.grid(1).unwrap().get(position).unwrap().end,
        Some(768)
    );
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), saved);
    let output = s.workspace().file().session.serialize_for_output().unwrap();
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open_selected(&path, "dance-single", "Challenge", "", 10).unwrap();
    assert_eq!(
        reopened.workspace().file().session.serialize_for_output().unwrap(),
        output
    );
    ctrl(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::Home);
    press(&mut s, KeyCode::Space);
    press(&mut s, KeyCode::PageDown);
    mixed(&mut s, KeyCode::KeyX);
    let session = &s.workspace().file().session;
    assert_eq!(session.document().text(Some(0), "BPMS").unwrap(), "0=120");
    assert!(
        deadsync_editor::attacks::placement::Timeline::capture(session, 0)
            .unwrap()
            .markers()
            .is_empty()
    );
    let mut parsed =
        Session::new(Document::parse(session.serialize().unwrap(), Format::Ssc).unwrap());
    assert_eq!(parsed.grid(0).unwrap().iter().count(), 0);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), saved);
}

#[test]
fn clearing_last_chart_attack_and_mixed_cut_preserve_other_inherited_and_local_charts() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    fn mixed(s: &mut State, code: KeyCode) {
        s.raw_key(&RawKeyboardEvent {
            code: KeyCode::ShiftLeft,
            pressed: true,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
        ctrl(s, code);
        s.raw_key(&RawKeyboardEvent {
            code: KeyCode::ShiftLeft,
            pressed: false,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("inheritance.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=1:LEN=1:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Challenge;#METER:10;#BPMS:0=240;#OFFSET:0.5;#NOTES:0100;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Easy;#METER:5;#ATTACKS:TIME=2:LEN=2:MODS=50% Drunk;#NOTES:0010;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    let check = |s: &State| {
        let session = &s.workspace().file().session;
        for chart in [1, 2] {
            assert!(
                session
                    .document()
                    .raw_property(Some(chart), "BPMS")
                    .is_some()
            );
        }
        for (chart, expected) in [(0, vec![]), (1, vec![288]), (2, vec![192])] {
            let actual =
                deadsync_editor::attacks::placement::Timeline::capture(session, chart).unwrap();
            assert_eq!(
                actual.markers().iter().map(|m| m.row).collect::<Vec<_>>(),
                expected
            );
        }
    };
    ctrl(&mut s, KeyCode::KeyJ);
    ctrl(&mut s, KeyCode::Enter);
    action(&mut s, 11);
    action(&mut s, 12);
    check(&s);
    let cleared = s.workspace().file().session.serialize().unwrap();
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), cleared);
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    check(&reopened);
    ctrl(&mut s, KeyCode::KeyZ);
    press(&mut s, KeyCode::Home);
    press(&mut s, KeyCode::Space);
    press(&mut s, KeyCode::PageDown);
    mixed(&mut s, KeyCode::KeyX);
    check(&s);
    let mut parsed = Session::new(
        Document::parse(
            s.workspace().file().session.serialize().unwrap(),
            Format::Ssc,
        )
        .unwrap(),
    );
    assert_eq!(parsed.grid(0).unwrap().iter().count(), 0);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}

#[test]
fn individual_event_picker_copies_cuts_and_deletes_only_checked_notes_timing_and_attacks() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    fn modified(s: &mut State, key: KeyCode) {
        s.raw_key(&RawKeyboardEvent {
            code: KeyCode::ShiftLeft,
            pressed: true,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
        ctrl(s, key);
        s.raw_key(&RawKeyboardEvent {
            code: KeyCode::ShiftLeft,
            pressed: false,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
    fn range(s: &mut State, end: bool) {
        if s.workspace().selection().is_some() {
            press(s, KeyCode::Space);
        }
        press(s, KeyCode::Home);
        press(s, KeyCode::Space);
        if end {
            press(s, KeyCode::PageDown);
        }
    }
    fn choose_three(s: &mut State) {
        modified(s, KeyCode::KeyE);
        modified(s, KeyCode::KeyA);
        for _ in 0..2 {
            press(s, KeyCode::ArrowDown);
        }
        press(s, KeyCode::Space);
        for _ in 0..3 {
            press(s, KeyCode::ArrowDown);
        }
        press(s, KeyCode::Space);
        press(s, KeyCode::ArrowDown);
        press(s, KeyCode::Space);
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("individual.ssc");
    let source=b"#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#BPMS:0=120,4=180;#STOPS:1=0.5;#LABELS:2=Pick;#ATTACKS:TIME=1.5:LEN=2:MODS=100% Dizzy;#NOTES:1M00,0200,0300;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    range(&mut s, true);
    choose_three(&mut s);
    assert!(!s.wants_text_input());
    assert!(!s.text_input("Unexpected"));
    let mut actors = Vec::new();
    s.actors(&mut actors);
    assert!(actors.iter().any(
        |a| matches!(a,deadlib_present::actors::Actor::Text{content,..} if content.contains("3/9"))
    ));
    ctrl(&mut s, KeyCode::KeyC);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    press(&mut s, KeyCode::Home);
    press(&mut s, KeyCode::PageDown);
    press(&mut s, KeyCode::PageDown);
    modified(&mut s, KeyCode::KeyV);
    let pasted = s.workspace().file().session.serialize().unwrap();
    let mut parsed = Session::new(Document::parse(pasted.clone(), Format::Ssc).unwrap());
    let position = |row, lane| deadsync_editor::notes::Position {
        row,
        lane,
        player: 0,
    };
    assert!(parsed.grid(0).unwrap().get(position(384, 0)).is_some());
    assert!(parsed.grid(0).unwrap().get(position(384, 1)).is_none());
    assert!(parsed.grid(0).unwrap().get(position(576, 1)).is_none());
    assert_eq!(
        parsed.document().text(Some(0), "BPMS").unwrap(),
        "0=120,4=180"
    );
    assert_eq!(parsed.document().text(Some(0), "STOPS").unwrap(), "1=0.5");
    let attacks = deadsync_editor::attacks::placement::Timeline::capture(&parsed, 0).unwrap();
    assert_eq!(
        attacks.markers().iter().map(|m| m.row).collect::<Vec<_>>(),
        [96, 480]
    );
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), pasted);
    range(&mut s, true);
    choose_three(&mut s);
    ctrl(&mut s, KeyCode::KeyX);
    let mut cut = Session::new(
        Document::parse(
            s.workspace().file().session.serialize().unwrap(),
            Format::Ssc,
        )
        .unwrap(),
    );
    assert!(cut.grid(0).unwrap().get(position(0, 0)).is_none());
    assert_eq!(
        cut.grid(0).unwrap().get(position(0, 1)).unwrap().kind,
        deadsync_editor::notes::Kind::Mine
    );
    assert_eq!(
        cut.grid(0).unwrap().get(position(192, 1)).unwrap().end,
        Some(384)
    );
    assert_eq!(
        deadsync_editor::attacks::placement::Timeline::capture(&cut, 0)
            .unwrap()
            .markers()
            .iter()
            .map(|m| m.row)
            .collect::<Vec<_>>(),
        [480]
    );
    assert_eq!(cut.document().text(Some(0), "STOPS").unwrap(), "1=0.5");
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), pasted);
    range(&mut s, false);
    modified(&mut s, KeyCode::KeyE);
    modified(&mut s, KeyCode::KeyA);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Space);
    press(&mut s, KeyCode::Delete);
    let mut deleted = Session::new(
        Document::parse(
            s.workspace().file().session.serialize().unwrap(),
            Format::Ssc,
        )
        .unwrap(),
    );
    assert!(deleted.grid(0).unwrap().get(position(0, 1)).is_none());
    assert!(deleted.grid(0).unwrap().get(position(0, 0)).is_some());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), pasted);
    press(&mut s, KeyCode::Home);
    for _ in 0..4 {
        press(&mut s, KeyCode::PageDown);
    }
    modified(&mut s, KeyCode::KeyV);
    let saved = s.workspace().file().session.serialize().unwrap();
    let mut parsed = Session::new(Document::parse(saved.clone(), Format::Ssc).unwrap());
    assert!(parsed.grid(0).unwrap().get(position(768, 0)).is_some());
    assert!(parsed.grid(0).unwrap().get(position(768, 1)).is_none());
    let output = s.workspace().file().session.serialize_for_output().unwrap();
    ctrl(&mut s, KeyCode::KeyS);
    let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    assert_eq!(
        reopened.workspace().file().session.serialize_for_output().unwrap(),
        output
    );
}
#[test]
fn individual_picker_pagination_cancel_empty_choices_and_protected_delete_are_safe() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    fn modified(s: &mut State, key: KeyCode) {
        s.raw_key(&RawKeyboardEvent {
            code: KeyCode::ShiftLeft,
            pressed: true,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
        ctrl(s, key);
        s.raw_key(&RawKeyboardEvent {
            code: KeyCode::ShiftLeft,
            pressed: false,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("choices.ssc");
    let source=b"#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#BPMS:0=120,4=180;#STOPS:1=0.5;#LABELS:2=Pick;#ATTACKS:TIME=1.5:LEN=2:MODS=100% Dizzy;#NOTES:1M00,0200,0300;";
    fs::write(&path, source).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    press(&mut s, KeyCode::Space);
    press(&mut s, KeyCode::PageDown);
    modified(&mut s, KeyCode::KeyE);
    press(&mut s, KeyCode::End);
    fn flatten(actors: Vec<deadlib_present::actors::Actor>) -> Vec<deadlib_present::actors::Actor> {
        let mut flattened = Vec::new();
        for actor in actors {
            match actor {
                deadlib_present::actors::Actor::Frame { children, .. }
                | deadlib_present::actors::Actor::Camera { children, .. } => {
                    flattened.extend(flatten(children))
                }
                deadlib_present::actors::Actor::SharedFrame { children, .. }
                | deadlib_present::actors::Actor::SharedTransform { children, .. }
                | deadlib_present::actors::Actor::SharedCamera { children, .. } => {
                    flattened.extend(flatten(children.to_vec()))
                }
                deadlib_present::actors::Actor::RetainedFrame { frame, .. } => {
                    flattened.extend(flatten(frame.children().to_vec()))
                }
                deadlib_present::actors::Actor::Shadow { child, .. } => {
                    flattened.extend(flatten(vec![*child]))
                }
                actor => flattened.push(actor),
            }
        }
        flattened
    }
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let actors = flatten(actors);
    assert!(actors.iter().any(
        |a| matches!(a,deadlib_present::actors::Actor::Text{content,..}if content.contains("3/3"))
    ), "{:?}", actors.iter().filter_map(|actor| match actor { deadlib_present::actors::Actor::Text { content, .. } => Some(content.as_ref()), _ => None }).collect::<Vec<_>>());
    let count=actors.iter().filter(|a|matches!(a,deadlib_present::actors::Actor::Text{content,..}if content.starts_with("[x]"))).count();
    assert_eq!(count, 1);
    press(&mut s, KeyCode::Home);
    modified(&mut s, KeyCode::KeyA);
    ctrl(&mut s, KeyCode::KeyC);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    assert!(actors.iter().any(|a|matches!(a,deadlib_present::actors::Actor::Text{content,..}if content.contains("Choisir les événements"))));
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    modified(&mut s, KeyCode::KeyE);
    modified(&mut s, KeyCode::KeyA);
    press(&mut s, KeyCode::Space);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Space);
    press(&mut s, KeyCode::Delete);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
