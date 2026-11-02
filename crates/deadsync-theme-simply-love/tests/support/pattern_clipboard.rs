use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    Error,
    chart_order::{Orderer, Snapshot},
    document::{Document, Format},
    generation::metadata::{ChartData, Definition},
    history::Session,
    notes::Position,
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{collections::BTreeMap, fs, time::Instant};

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
    if shift {
        key(s, KeyCode::ShiftLeft, false);
    }
    key(s, KeyCode::SuperLeft, false);
}
fn p(lane: u16, row: u64) -> Position {
    Position {
        player: 0,
        lane,
        row,
    }
}
fn definition() -> Definition {
    Definition {
        pattern_config_guid: "03de57bd-4329-4de6-a722-a171c11fdd16".parse().unwrap(),
        performed_chart_config_guid: "41a7f76e-596a-4fbe-bf6e-6a77a898e4a1".parse().unwrap(),
        length: 96,
        random_seed: 42,
        start_position_inclusive: true,
        end_position_inclusive: false,
        ignore_preceding_distribution: true,
        ..Default::default()
    }
}
fn setup(
    target: Option<&str>,
    lane: u16,
) -> (tempfile::TempDir, std::path::PathBuf, State, Vec<u8>) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    i18n::set_locale(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let layout = if target.is_some() {
        "dance-double"
    } else {
        "dance-single"
    };
    let columns = if target.is_some() { 8 } else { 4 };
    let mut notes = vec![b'0'; columns];
    notes[lane as usize] = b'1';
    let mut text = format!(
        "#VERSION:0.83;#TITLE:Clipboard;#BPMS:0=120;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:{layout};#DIFFICULTY:Hard;#METER:9;#NOTES:{};",
        String::from_utf8(notes).unwrap()
    );
    if let Some(target) = target {
        let n = if target == "dance-single" { 4 } else { 8 };
        text += &format!(
            "#NOTEDATA:;#STEPSTYPE:{target};#DIFFICULTY:Hard;#METER:9;#NOTES:{};",
            "0".repeat(n)
        );
    }
    let mut session = Session::new(Document::parse(text.as_bytes(), Format::Ssc).unwrap());
    let mut data = ChartData::default();
    data.patterns = Some(BTreeMap::from([(0, definition())]));
    data.save(&mut session, 0).unwrap();
    if target == Some("dance-routine") {
        ChartData::default().save(&mut session, 1).unwrap();
    }
    let baseline = session.serialize().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, &baseline).unwrap();
    let state = State::open_selected(&path, layout, "Hard", "", 9).unwrap();
    (dir, path, state, baseline)
}
fn texts(s: &State) -> String {
    fn visit(actor: &Actor, out: &mut String) {
        match actor {
            Actor::Text { content, .. } => {
                out.push_str(content);
                out.push('\n');
            }
            Actor::Frame { children, .. } => {
                for child in children {
                    visit(child, out)
                }
            }
            _ => {}
        }
    }
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let mut out = String::new();
    for actor in &actors {
        visit(actor, &mut out)
    }
    out
}
fn tr(en: &str, fr: &str) -> String {
    if LANGUAGE == "fr" { fr } else { en }.into()
}
fn open_event_picker(s: &mut State) {
    if texts(s).lines().any(|line| {
        line == i18n::tr("Edition", "EventSelectionTitle").as_ref()
            || line == i18n::tr("Edition", "MarkerSave").as_ref()
    }) {
        press(s, KeyCode::Escape);
    }
    control(s, KeyCode::KeyE, true);
}
fn select(s: &mut State, mixed: bool) {
    open_event_picker(s);
    control(s, KeyCode::KeyA, true);
    press(s, KeyCode::End);
    press(s, KeyCode::Space);
    assert!(
        texts(s).contains(&tr("Pattern marker", "Marqueur de pattern")),
        "{}",
        texts(s)
    );
    if mixed {
        press(s, KeyCode::ArrowUp);
        press(s, KeyCode::Space);
    }
}
struct NativeOrder;
impl Orderer for NativeOrder {
    fn order(&mut self, snapshot: &Snapshot) -> Result<Vec<usize>, Error> {
        snapshot.indices_with(str::cmp)
    }
}
fn next(s: &mut State) {
    control(s, KeyCode::Tab, false);
    assert!(s.process_chart_order(&mut NativeOrder));
}

#[test]
fn marker_only_keyboard_copy_paste_save_and_one_step_undo_redo_preserve_notes() {
    let (_dir, path, mut s, baseline) = setup(None, 0);
    select(&mut s, false);
    control(&mut s, KeyCode::KeyC, false);
    press(&mut s, KeyCode::PageDown);
    control(&mut s, KeyCode::KeyV, true);
    let data = ChartData::read(s.workspace().file().session.document(), 0)
        .unwrap()
        .unwrap();
    assert_eq!(data.patterns.as_ref().unwrap()[&192], definition());
    assert_eq!(data.patterns.as_ref().unwrap()[&0], definition());
    assert!(s.workspace().grid().get(p(0, 0)).is_some());
    assert!(s.workspace().grid().get(p(0, 192)).is_none());
    assert!(texts(&s).contains(&tr(
        "Selected events pasted.",
        "Événements sélectionnés collés."
    )));
    let after = s.workspace().file().session.serialize().unwrap();
    control(&mut s, KeyCode::KeyZ, false);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), baseline);
    control(&mut s, KeyCode::KeyY, false);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), after);
    let output = s.workspace().file().session.serialize_for_output().unwrap();
    control(&mut s, KeyCode::KeyS, false);
    assert_eq!(fs::read(&path).unwrap(), output);
    let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    assert_eq!(
        ChartData::read(reopened.workspace().file().session.document(), 0)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()[&192],
        definition()
    );
}

#[test]
fn marker_cut_delete_and_empty_copy_preserve_generated_notes_and_the_previous_clipboard() {
    let (_dir, _path, mut s, baseline) = setup(None, 0);
    select(&mut s, false);
    control(&mut s, KeyCode::KeyX, false);
    assert!(s.workspace().grid().get(p(0, 0)).is_some());
    assert!(
        !ChartData::read(s.workspace().file().session.document(), 0)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()
            .contains_key(&0)
    );
    control(&mut s, KeyCode::KeyZ, false);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), baseline);
    select(&mut s, false);
    press(&mut s, KeyCode::Delete);
    assert!(s.workspace().grid().get(p(0, 0)).is_some());
    control(&mut s, KeyCode::KeyZ, false);
    open_event_picker(&mut s);
    control(&mut s, KeyCode::KeyA, true);
    control(&mut s, KeyCode::KeyC, false);
    assert!(texts(&s).contains(&tr(
        "Choose at least one event.",
        "Choisissez au moins un événement."
    )));
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::PageDown);
    control(&mut s, KeyCode::KeyV, true);
    assert_eq!(
        ChartData::read(s.workspace().file().session.document(), 0)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()[&192],
        definition()
    );
    assert!(s.workspace().grid().get(p(0, 192)).is_none());
}

#[test]
fn mixed_paste_into_one_player_routine_reports_skipped_patterns_and_retains_redo_on_noop() {
    let (_dir, _path, mut s, baseline) = setup(Some("dance-routine"), 0);
    select(&mut s, true);
    control(&mut s, KeyCode::KeyC, false);
    next(&mut s);
    assert_eq!(s.workspace().chart(), 1);
    assert_eq!(s.workspace().grid().players(), 1);
    control(&mut s, KeyCode::KeyV, true);
    assert!(s.workspace().grid().get(p(0, 0)).is_some());
    assert!(
        ChartData::read(s.workspace().file().session.document(), 1)
            .unwrap()
            .unwrap()
            .patterns
            .is_none()
    );
    assert!(texts(&s).contains(&tr(
        "1 unsupported pattern markers",
        "1 marqueurs non pris en charge"
    )));
    control(&mut s, KeyCode::KeyZ, false);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), baseline);
    let redo = s
        .workspace()
        .file()
        .session
        .redo_label()
        .unwrap()
        .to_owned();
    next(&mut s);
    assert_eq!(s.workspace().chart(), 0);
    select(&mut s, false);
    control(&mut s, KeyCode::KeyC, false);
    next(&mut s);
    control(&mut s, KeyCode::KeyV, true);
    assert!(texts(&s).contains(&tr("No events changed.", "Aucun événement modifié.")));
    assert_eq!(s.workspace().file().session.serialize().unwrap(), baseline);
    assert_eq!(
        s.workspace().file().session.redo_label(),
        Some(redo.as_str())
    );
}

#[test]
fn cross_layout_paste_keeps_marker_and_reports_only_notes_outside_destination_lanes() {
    let (_dir, _path, mut s, baseline) = setup(Some("dance-single"), 7);
    select(&mut s, true);
    control(&mut s, KeyCode::KeyC, false);
    next(&mut s);
    assert_eq!(s.workspace().chart(), 1);
    control(&mut s, KeyCode::KeyV, true);
    assert_eq!(
        ChartData::read(s.workspace().file().session.document(), 1)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()[&0],
        definition()
    );
    assert_eq!(s.workspace().grid().iter().count(), 0);
    assert!(texts(&s).contains(&tr(
        "1 notes outside the destination lanes",
        "1 notes hors des colonnes de destination"
    )));
    assert!(!texts(&s).contains("{patterns}"));
    assert!(!texts(&s).contains("{notes}"));
    control(&mut s, KeyCode::KeyZ, false);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), baseline);
}
