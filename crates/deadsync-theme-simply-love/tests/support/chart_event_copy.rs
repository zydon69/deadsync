use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    document::{Document, Format},
    generation::metadata::{ChartData, Definition},
    history::Session,
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
fn translated(en: &str, fr: &str) -> String {
    if LANGUAGE == "fr" { fr } else { en }.into()
}
fn setup(format: Format, legacy: bool) -> (tempfile::TempDir, std::path::PathBuf, State, Vec<u8>) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    i18n::set_locale(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let source = if format == Format::Sm {
        "#TITLE:Copy;#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;#NOTES:dance-single::Medium:8::0100;".to_owned()
    } else if legacy {
        "#VERSION:0.6;#TITLE:Copy;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Medium;#METER:8;#NOTES:0100;".to_owned()
    } else {
        "#VERSION:0.83;#TITLE:Copy;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#BPMS:0=180,4=240;#TIMESIGNATURES:0=2=2,4=3=4;#LABELS:1=source;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Medium;#METER:8;#BPMS:0=90,2=110;#TIMESIGNATURES:0=4=4;#LABELS:3=old;#STOPS:3=0.5;#NOTES:0100;#NOTEDATA:;#STEPSTYPE:dance-routine;#DIFFICULTY:Hard;#METER:9;#BPMS:0=100;#LABELS:2=old;#NOTES:00100000;".to_owned()
    };
    let mut session = Session::new(Document::parse(source.as_bytes(), format).unwrap());
    if !legacy && format == Format::Ssc {
        for (chart, row, seed) in [(0, 48, 42), (1, 192, 7), (2, 96, 8)] {
            let mut data = ChartData::existing_or_initial(session.document(), chart).unwrap();
            data.patterns = Some(BTreeMap::from([(
                row,
                Definition {
                    pattern_config_guid: deadsync_editor::generation::metadata::SIXTEENTHS.parse().unwrap(),
                    performed_chart_config_guid: deadsync_editor::generation::metadata::PATTERN_BALANCED.parse().unwrap(),
                    random_seed: seed,
                    length: 96,
                    ..Definition::default()
                },
            )]));
            data.max_player_index = 0;
            data.save(&mut session, chart).unwrap();
        }
    }
    let baseline = session.serialize().unwrap();
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
    control(s, KeyCode::KeyY, true);
    assert!(text(s).contains(&translated(
        "Copy event types between charts",
        "Copier des types entre charts"
    )));
    // Source presets now initially select all StepMania event types; these
    // scenarios deliberately exercise independent explicit choices.
    control(s, KeyCode::KeyA, true);
}
fn choose(s: &mut State, index: usize) {
    press(s, KeyCode::Home);
    for _ in 0..=index {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Space);
}
fn apply(s: &mut State) {
    press(s, KeyCode::End);
    press(s, KeyCode::ArrowUp);
    press(s, KeyCode::Enter);
}
#[test]
fn keyboard_multi_destination_bpm_signature_copy_is_one_undo_and_saves() {
    let (_dir, path, mut s, before) = setup(Format::Ssc, false);
    open(&mut s);
    choose(&mut s, 0);
    choose(&mut s, 1);
    apply(&mut s);
    for chart in [1, 2] {
        assert_eq!(
            s.workspace()
                .file()
                .session
                .document()
                .text(Some(chart), "BPMS"),
            Some("0=180,4=240".into())
        );
        assert_eq!(
            s.workspace()
                .file()
                .session
                .document()
                .text(Some(chart), "TIMESIGNATURES"),
            Some("0=2=2,4=3=4".into())
        );
    }
    assert!(text(&s).contains(&translated("Changed charts: 2.", "Charts modifiées : 2.")));
    let after = s.workspace().file().session.serialize().unwrap();
    control(&mut s, KeyCode::KeyZ, false);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), before);
    control(&mut s, KeyCode::KeyY, false);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), after);
    let output = s.workspace().file().session.serialize_for_output().unwrap();
    control(&mut s, KeyCode::KeyS, false);
    assert_eq!(fs::read(&path).unwrap(), output);
}
#[test]
fn independent_type_and_destination_choices_are_paginated_and_empty_choice_is_guarded() {
    let (_dir, _path, mut s, before) = setup(Format::Ssc, false);
    open(&mut s);
    apply(&mut s);
    assert!(text(&s).contains(&translated(
        "Choose at least one event type.",
        "Choisissez au moins un type d’événement."
    )));
    assert_eq!(s.workspace().file().session.serialize().unwrap(), before);
    choose(&mut s, 10);
    assert!(text(&s).contains("Page 2/3"));
    choose(&mut s, 13);
    apply(&mut s);
    assert_eq!(
        s.workspace()
            .file()
            .session
            .document()
            .text(Some(1), "LABELS"),
        Some("3=old".into())
    );
    assert_eq!(
        s.workspace()
            .file()
            .session
            .document()
            .text(Some(2), "LABELS"),
        Some("1=source".into())
    );
    assert!(text(&s).contains(&translated("Changed charts: 1.", "Charts modifiées : 1.")));
    control(&mut s, KeyCode::KeyZ, false);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), before);
}
#[test]
fn unsupported_patterns_are_reported_and_existing_destination_patterns_are_removed() {
    let (_dir, path, mut s, before) = setup(Format::Ssc, false);
    open(&mut s);
    choose(&mut s, 12);
    apply(&mut s);
    let document = s.workspace().file().session.document();
    let target = ChartData::read(document, 1)
        .unwrap()
        .unwrap()
        .patterns
        .unwrap();
    assert_eq!(target.len(), 1);
    assert_eq!(target[&48].random_seed, 42);
    assert!(
        ChartData::read(document, 2)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()
            .is_empty()
    );
    assert!(text(&s).contains(&translated("Skipped patterns: 1.", "Patterns ignorés : 1.")));
    assert!(!text(&s).contains("{patterns}"));
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
#[test]
fn sm_and_legacy_ssc_explain_shared_event_noop_without_changing_source() {
    for (format, legacy) in [(Format::Sm, false), (Format::Ssc, true)] {
        let (_dir, _path, mut s, before) = setup(format, legacy);
        open(&mut s);
        choose(&mut s, 1);
        apply(&mut s);
        let visible = text(&s);
        assert!(visible.contains(&translated("Changed charts: 0.", "Charts modifiées : 0.")));
        assert!(visible.contains(&translated(
            "Shared song types unchanged:",
            "Types partagés de la chanson inchangés :"
        )));
        assert!(!visible.contains("{shared}"));
        assert_eq!(s.workspace().file().session.serialize().unwrap(), before);
    }
}
