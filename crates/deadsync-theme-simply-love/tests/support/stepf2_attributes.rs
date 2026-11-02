use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    document::Format,
    notes::{Kind, Position},
    stepf2::{Attributes, Visibility},
};
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
fn text(s: &mut State, value: &str) {
    ctrl(s, KeyCode::KeyA);
    assert!(s.text_input(value));
}
fn init() {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn open_inspector(s: &mut State) {
    press(s, KeyCode::F1);
    for _ in 0..4 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
    for _ in 0..11 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
}
fn source(format: Format, sustain: bool) -> String {
    let notes = if sustain {
        "{x|s|1|9}[7]00000000Y\n0000000000\n{3|v|0|_}[8]000000000\n0000000000"
    } else {
        "X00000000Y"
    };
    match format {
        Format::Sm => format!(
            "#TITLE:Keep;#VENDOR:untouched;#BPMS:0=120;#NOTES:pump-routine::Hard:9:0,0,0,0,0:{notes};"
        ),
        Format::Ssc => format!(
            "#VERSION:0.83;#TITLE:Keep;#VENDOR:untouched;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:pump-routine;#DIFFICULTY:Hard;#METER:9;#NOTES:{notes};"
        ),
    }
}
fn labels(s: &State) -> Vec<String> {
    let mut actors = Vec::new();
    s.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|actor| match actor {
            Actor::Text { content, .. } => Some(content.to_string()),
            _ => None,
        })
        .collect()
}
fn has(s: &State, fragment: &str) -> bool {
    labels(s).iter().any(|label| label.contains(fragment))
}
fn localized(en: &'static str, fr: &'static str) -> &'static str {
    if LANGUAGE == "fr" { fr } else { en }
}
fn p() -> Position {
    Position {
        row: 0,
        lane: 0,
        player: 0,
    }
}
fn apply(s: &mut State) {
    press(s, KeyCode::PageDown);
    press(s, KeyCode::ArrowUp);
    press(s, KeyCode::Enter);
}

#[test]
fn typed_head_and_tail_share_one_raw_draft_and_one_reversible_saved_gesture() {
    init();
    for format in [Format::Sm, Format::Ssc] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("song.{}", format.extension()));
        let original = source(format, true);
        fs::write(&path, &original).unwrap();
        let mut s = State::open_selected(&path, "pump-routine", "Hard", "", 9).unwrap();
        open_inspector(&mut s);
        assert!(has(&s, localized("Visibility:", "Visibilité :")));
        assert!(has(&s, localized("Endpoint: Head", "Extrémité : Tête")));
        press(&mut s, KeyCode::ArrowDown);
        press(&mut s, KeyCode::ArrowRight); // tail
        assert!(has(&s, localized("Endpoint: Tail", "Extrémité : Fin")));
        press(&mut s, KeyCode::ArrowDown);
        press(&mut s, KeyCode::ArrowRight); // vanish -> hidden
        press(&mut s, KeyCode::ArrowDown);
        press(&mut s, KeyCode::ArrowRight); // fake
        press(&mut s, KeyCode::ArrowDown);
        text(&mut s, "%"); // reserved
        press(&mut s, KeyCode::ArrowDown);
        assert!(has(&s, "{N|h|1|%}[8]"));
        press(&mut s, KeyCode::F2); // raw view, tail
        assert!(has(&s, localized("Tail attributes:", "Attributs de fin :")));
        press(&mut s, KeyCode::F2); // typed view, same tail
        assert!(has(&s, "{N|h|1|%}[8]"));
        assert_eq!(
            s.workspace().file().session.serialize().unwrap(),
            original.as_bytes()
        );
        press(&mut s, KeyCode::PageUp);
        press(&mut s, KeyCode::ArrowRight);
        assert!(has(&s, localized("Column: 1", "Colonne : 1"))); // pending lane protected
        apply(&mut s);
        let note = s.workspace().grid().get(p()).unwrap();
        assert_eq!(note.kind, Kind::Hold);
        assert_eq!(note.end, Some(96));
        assert_eq!(&*note.attributes, "{N|s|1|9}[7]");
        assert_eq!(&*note.tail_attributes, "{N|h|1|%}[8]");
        let after = s.workspace().file().session.serialize().unwrap();
        ctrl(&mut s, KeyCode::KeyZ);
        assert_eq!(
            s.workspace().file().session.serialize().unwrap(),
            original.as_bytes()
        );
        ctrl(&mut s, KeyCode::KeyY);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), after);
        let output = s.workspace().file().session.serialize_for_output().unwrap();
        ctrl(&mut s, KeyCode::KeyS);
        assert_eq!(fs::read(&path).unwrap(), output);
        let reopened = State::open_selected(&path, "pump-routine", "Hard", "", 9).unwrap();
        assert_eq!(
            &*reopened
                .workspace()
                .grid()
                .get(p())
                .unwrap()
                .tail_attributes,
            "{N|h|1|%}[8]"
        );
    }
}

#[test]
fn malformed_raw_and_reserved_fields_remain_recoverable_and_fake_taps_stay_coherent() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let original = source(Format::Ssc, false);
    fs::write(&path, &original).unwrap();
    let mut s = State::open_selected(&path, "pump-routine", "Hard", "", 9).unwrap();
    open_inspector(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    assert!(has(&s, localized("Endpoint: Head", "Extrémité : Tête"))); // taps cannot select tail
    press(&mut s, KeyCode::ArrowDown);
    for _ in 0..4 {
        press(&mut s, KeyCode::ArrowRight);
    }
    assert!(has(
        &s,
        localized("Visibility: Normal", "Visibilité : Normale")
    ));
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight); // fake tap
    assert!(has(
        &s,
        localized("Fake flag: Yes", "Indicateur fake : Oui")
    ));
    press(&mut s, KeyCode::ArrowDown);
    text(&mut s, "||");
    press(&mut s, KeyCode::ArrowDown); // invalid reserved keeps current text focus
    assert!(has(&s, localized("requires one valid", "exige un seul")));
    text(&mut s, "_");
    press(&mut s, KeyCode::ArrowDown); // raw
    text(&mut s, "{N|h|1");
    press(&mut s, KeyCode::F2); // raw view can retain incomplete suffix
    press(&mut s, KeyCode::F2); // cannot reenter typed view yet
    assert!(has(
        &s,
        localized("Invalid StepF2 suffix", "Suffixe StepF2 invalide")
    ));
    text(&mut s, "{N|h|1|_}[11]");
    press(&mut s, KeyCode::F2);
    assert!(has(
        &s,
        localized("Visibility: Hidden", "Visibilité : Masquée")
    ));
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        original.as_bytes()
    );
    apply(&mut s);
    let note = s.workspace().grid().get(p()).unwrap();
    assert_eq!(note.kind, Kind::Fake);
    let (attributes, trailing) = Attributes::read(&note.attributes).unwrap().unwrap();
    assert!(attributes.fake);
    assert_eq!(attributes.visibility, Visibility::Hidden);
    assert_eq!(trailing, "[11]");
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        original.as_bytes()
    );
}

#[test]
fn cancelling_all_typed_changes_and_noop_apply_leave_song_and_history_untouched() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let original = source(Format::Ssc, false);
    fs::write(&path, &original).unwrap();
    let mut s = State::open_selected(&path, "pump-routine", "Hard", "", 9).unwrap();
    open_inspector(&mut s);
    apply(&mut s);
    assert!(s.workspace().file().session.undo_label().is_none());
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    open_inspector(&mut s);
    for _ in 0..3 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::F2);
    text(&mut s, "{N|v|1|%}[9]");
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        original.as_bytes()
    );
    assert!(s.workspace().file().session.undo_label().is_none());
    assert_eq!(fs::read(&path).unwrap(), original.as_bytes());
}

#[test]
fn ordinary_note_inspector_keeps_its_raw_five_row_keyboard_contract() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let original = b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;";
    fs::write(&path, original).unwrap();
    let mut s = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    open_inspector(&mut s);
    press(&mut s, KeyCode::F2);
    assert!(!has(&s, localized("Visibility:", "Visibilité :")));
    press(&mut s, KeyCode::ArrowDown);
    text(&mut s, "[7]");
    press(&mut s, KeyCode::ArrowDown);
    assert!(!s.wants_text_input());
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert_eq!(&*s.workspace().grid().get(p()).unwrap().attributes, "[7]");
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), original);
}

#[test]
fn changing_inspector_view_invalidates_queued_clipboard_before_native_io() {
    struct Clipboard {
        reads: usize,
    }
    impl deadsync_editor::clipboard::TextClipboard for Clipboard {
        fn read_text(&mut self) -> Result<String, deadsync_editor::clipboard::Error> {
            self.reads += 1;
            Ok("{N|h|1|%}[99]".into())
        }
        fn write_text(&mut self, _: &str) -> Result<(), deadsync_editor::clipboard::Error> {
            Ok(())
        }
    }
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let original = source(Format::Ssc, false);
    fs::write(&path, &original).unwrap();
    let mut s = State::open_selected(&path, "pump-routine", "Hard", "", 9).unwrap();
    open_inspector(&mut s);
    press(&mut s, KeyCode::F2);
    ctrl(&mut s, KeyCode::KeyV);
    press(&mut s, KeyCode::F2);
    let mut clipboard = Clipboard { reads: 0 };
    assert!(!s.process_text_clipboard(&mut clipboard));
    assert_eq!(clipboard.reads, 0);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        original.as_bytes()
    );
}

fn open_properties(s: &mut State) {
    press(s, KeyCode::F1);
    for _ in 0..9 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
}

#[test]
fn legacy_double_conversion_is_explicit_localized_reversible_and_saved_as_routine() {
    init();
    for format in [Format::Sm, Format::Ssc] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("song.{}", format.extension()));
        let original = source(format, false)
            .replace("pump-routine", "pump-double")
            .replace("X00000000Y", "X000000000");
        fs::write(&path, &original).unwrap();
        let mut s = State::open_selected(&path, "pump-double", "Hard", "", 9).unwrap();
        assert_eq!(s.workspace().grid().players(), 1);
        assert_eq!(
            s.workspace().file().session.serialize().unwrap(),
            original.as_bytes()
        );
        open_properties(&mut s);
        assert!(has(
            &s,
            localized("Convert co-op chart", "Convertir la chart coop")
        ));
        press(&mut s, KeyCode::ArrowUp); // final conversion action
        press(&mut s, KeyCode::Enter);
        assert_eq!(
            s.workspace().file().session.document().charts()[0].steps_type,
            "pump-routine"
        );
        assert_eq!(s.workspace().grid().players(), 2);
        let after = s.workspace().file().session.serialize().unwrap();
        assert!(has(
            &s,
            localized("converted to Pump Routine", "convertie en Pump Routine")
        ));
        ctrl(&mut s, KeyCode::KeyZ);
        assert_eq!(
            s.workspace().file().session.document().charts()[0].steps_type,
            "pump-double"
        );
        assert_eq!(s.workspace().grid().players(), 1);
        assert_eq!(
            s.workspace().file().session.serialize().unwrap(),
            original.as_bytes()
        );
        ctrl(&mut s, KeyCode::KeyY);
        let output = s.workspace().file().session.serialize_for_output().unwrap();
        ctrl(&mut s, KeyCode::KeyS);
        assert_eq!(fs::read(&path).unwrap(), output);
        let reopened = State::open_selected(&path, "pump-routine", "Hard", "", 9).unwrap();
        assert_eq!(reopened.workspace().grid().players(), 2);
    }
}

#[test]
fn layout_conversion_protects_pending_properties_and_is_hidden_for_standard_double() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let original = source(Format::Ssc, false).replace("pump-routine", "pump-double");
    fs::write(&path, &original).unwrap();
    let mut s = State::open_selected(&path, "pump-double", "Hard", "", 9).unwrap();
    open_properties(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight); // pending offset override
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::Enter);
    assert!(has(
        &s,
        localized("pending chart properties", "propriétés en attente")
    ));
    assert_eq!(
        s.workspace().file().session.document().charts()[0].steps_type,
        "pump-double"
    );
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        original.as_bytes()
    );
    let standard = original.replace("X00000000Y", "1000000000");
    fs::write(&path, &standard).unwrap();
    let mut s = State::open_selected(&path, "pump-double", "Hard", "", 9).unwrap();
    open_properties(&mut s);
    assert!(!has(
        &s,
        localized("Convert co-op chart", "Convertir la chart coop")
    ));
}
