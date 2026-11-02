use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    document::{Document, Format},
    generation::metadata::{ChartData, Definition},
    history::Session,
    notes::{Kind, Note, Position},
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
fn text(s: &State) -> String {
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
        visit(actor, &mut out);
    }
    out
}
fn tr(en: &str, fr: &str) -> String {
    if LANGUAGE == "fr" { fr } else { en }.into()
}
fn p(row: u64) -> Position {
    Position {
        player: 0,
        lane: 0,
        row,
    }
}
fn setup(format: Format, mode: &str) -> (tempfile::TempDir, std::path::PathBuf, State, Vec<u8>) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    i18n::set_locale(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let chart = if format == Format::Sm {
        "#NOTES:dance-single::Hard:9::0000;"
    } else {
        "#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:0000;"
    };
    let bytes = format!(
        "#VERSION:0.83;#TITLE:Shift;#VENDOR:keep;#BPMS:0=120;#TIMESIGNATURES:0=4=4;{chart}"
    );
    let mut s = Session::new(Document::parse(bytes.as_bytes(), format).unwrap());
    if mode == "mixed" || mode == "overflow" {
        s.edit_notes(0, "Note", [(p(0), Some(Note::tap()))])
            .unwrap();
        let mut data = ChartData::existing_or_initial(s.document(), 0).unwrap();
        data.patterns = Some(BTreeMap::from([(
            0,
            Definition {
                // Shift scenarios need valid generation identities; unknown
                // profiles are repaired by the independent open-song workflow.
                pattern_config_guid: deadsync_editor::generation::metadata::SIXTEENTHS.parse().unwrap(),
                performed_chart_config_guid: deadsync_editor::generation::metadata::PATTERN_BALANCED.parse().unwrap(),
                random_seed: 42,
                length: if mode == "overflow" {
                    i32::MAX as u64
                } else {
                    96
                },
                ..Definition::default()
            },
        )]));
        data.save(&mut s, 0).unwrap();
    }
    if mode == "collision" {
        s.edit_notes(
            0,
            "Collision",
            [
                (
                    p(0),
                    Some(Note {
                        kind: Kind::Hold,
                        end: Some(192),
                        attributes: "".into(),
                        tail_attributes: "".into(),
                    }),
                ),
                (p(240), Some(Note::tap())),
            ],
        )
        .unwrap();
    }
    let baseline = s.serialize().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(if format == Format::Sm {
        "song.sm"
    } else {
        "song.ssc"
    });
    fs::write(&path, &baseline).unwrap();
    let state = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
    (dir, path, state, baseline)
}
fn open(s: &mut State) {
    control(s, KeyCode::KeyE, true);
    assert!(text(s).contains(&tr("Ctrl/Cmd+Left/Right", "Ctrl/Cmd+Gauche/Droite")));
}
fn patterns(s: &State) -> BTreeMap<u64, Definition> {
    ChartData::read(s.workspace().file().session.document(), 0)
        .unwrap()
        .unwrap()
        .patterns
        .unwrap()
}
#[test]
fn repeated_shift_preserves_only_transformed_choices_for_copy_paste_undo_redo_and_reopen() {
    for format in [Format::Sm, Format::Ssc] {
        let (_dir, path, mut s, before) = setup(format, "mixed");
        open(&mut s);
        control(&mut s, KeyCode::ArrowRight, false);
        assert!(s.workspace().grid().get(p(48)).is_some());
        assert_eq!(patterns(&s).keys().copied().collect::<Vec<_>>(), [48]);
        assert!(text(&s).contains(&tr("2/4 selected", "2/4 choisis")), "{}", text(&s));
        assert!(text(&s).contains(&tr(
            "Initial nodes kept: 2.",
            "Nœuds initiaux conservés : 2."
        )));
        let first = s.workspace().file().session.serialize().unwrap();
        control(&mut s, KeyCode::ArrowRight, false);
        assert!(s.workspace().grid().get(p(96)).is_some());
        assert!(s.workspace().grid().get(p(48)).is_none());
        let second = s.workspace().file().session.serialize().unwrap();
        control(&mut s, KeyCode::KeyC, false);
        press(&mut s, KeyCode::PageDown);
        control(&mut s, KeyCode::KeyV, true);
        assert!(s.workspace().grid().get(p(192)).is_some());
        assert!(s.workspace().grid().get(p(288)).is_none());
        assert_eq!(patterns(&s).keys().copied().collect::<Vec<_>>(), [96, 192]);
        assert_eq!(
            s.workspace().file().session.document().text(None, "BPMS"),
            Some("0=120".into())
        );
        let after = s.workspace().file().session.serialize().unwrap();
        control(&mut s, KeyCode::KeyZ, false);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), second);
        control(&mut s, KeyCode::KeyZ, false);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), first);
        control(&mut s, KeyCode::KeyZ, false);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), before);
        for _ in 0..3 {
            control(&mut s, KeyCode::KeyY, false);
        }
        assert_eq!(s.workspace().file().session.serialize().unwrap(), after);
        let output = s.workspace().file().session.serialize_for_output().unwrap();
        control(&mut s, KeyCode::KeyS, false);
        assert_eq!(fs::read(&path).unwrap(), output);
        let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
        assert_eq!(
            reopened.workspace().file().session.serialize().unwrap(),
            output
        );
    }
}
#[test]
fn negative_shift_deletes_whole_events_keeps_initial_nodes_and_clears_transformed_choice() {
    let (_dir, _path, mut s, before) = setup(Format::Ssc, "mixed");
    open(&mut s);
    control(&mut s, KeyCode::ArrowLeft, false);
    assert_eq!(s.workspace().grid().iter().count(), 0);
    assert!(patterns(&s).is_empty());
    assert!(text(&s).contains(&tr(
        "Deleted outside valid positions: 2.",
        "Supprimés hors des positions valides : 2."
    )));
    assert!(text(&s).contains(&tr("0/2 selected", "0/2 choisis")));
    let after = s.workspace().file().session.serialize().unwrap();
    control(&mut s, KeyCode::KeyC, false);
    assert!(text(&s).contains(&tr(
        "Choose at least one event.",
        "Choisissez au moins un événement."
    )));
    control(&mut s, KeyCode::ArrowRight, false);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), after);
    press(&mut s, KeyCode::Escape);
    control(&mut s, KeyCode::KeyZ, false);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), before);
}
#[test]
fn initial_nodes_only_are_an_exclusive_zero_change_choice() {
    let (_dir, _path, mut s, before) = setup(Format::Ssc, "immutable");
    let revision = s.workspace().file().session.revision();
    open(&mut s);
    control(&mut s, KeyCode::ArrowRight, false);
    assert!(text(&s).contains(&tr("Moved: 0.", "Déplacés : 0.")));
    assert!(text(&s).contains(&tr(
        "Initial nodes kept: 2.",
        "Nœuds initiaux conservés : 2."
    )));
    assert!(text(&s).contains(&tr("0/2 selected", "0/2 choisis")));
    assert_eq!(s.workspace().file().session.serialize().unwrap(), before);
    assert_eq!(s.workspace().file().session.revision(), revision);
    control(&mut s, KeyCode::KeyC, false);
    assert!(text(&s).contains(&tr(
        "Choose at least one event.",
        "Choisissez au moins un événement."
    )));
}
#[test]
fn shifting_a_selected_tap_force_trims_an_unselected_hold_and_undo_restores_both() {
    for format in [Format::Sm, Format::Ssc] {
        let (_dir, path, mut s, before) = setup(format, "collision");
        press(&mut s, KeyCode::PageDown);
        press(&mut s, KeyCode::ArrowDown);
        assert_eq!(s.workspace().row(), 240);
        open(&mut s);
        control(&mut s, KeyCode::ArrowLeft, false);
        assert_eq!(s.workspace().grid().get(p(0)).unwrap().end, Some(180));
        assert_eq!(s.workspace().grid().get(p(192)).unwrap().kind, Kind::Tap);
        assert!(s.workspace().grid().get(p(240)).is_none());
        assert!(text(&s).contains(&tr("1/1 selected", "1/1 choisis")));
        press(&mut s, KeyCode::Escape);
        let after = s.workspace().file().session.serialize().unwrap();
        control(&mut s, KeyCode::KeyZ, false);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), before);
        control(&mut s, KeyCode::KeyY, false);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), after);
        let output = s.workspace().file().session.serialize_for_output().unwrap();
        control(&mut s, KeyCode::KeyS, false);
        let reopened = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
        assert_eq!(
            reopened.workspace().file().session.serialize().unwrap(),
            output
        );
    }
}
#[test]
fn invalid_shift_is_localized_atomic_and_keeps_the_original_choices() {
    let (_dir, _path, mut s, before) = setup(Format::Ssc, "overflow");
    let revision = s.workspace().file().session.revision();
    open(&mut s);
    control(&mut s, KeyCode::ArrowRight, false);
    let visible = text(&s);
    assert!(visible.contains(&tr(
        "Unable to shift this choice. No events were changed.",
        "Impossible de déplacer cette sélection. Aucun événement n’a été modifié."
    )));
    assert!(visible.contains(&tr("4/4 selected", "4/4 choisis")));
    assert!(!visible.contains("{moved}"));
    assert_eq!(s.workspace().file().session.serialize().unwrap(), before);
    assert_eq!(s.workspace().file().session.revision(), revision);
}
