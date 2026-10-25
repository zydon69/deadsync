use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn press(state: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
fn texts(state: &State) -> Vec<String> {
    let mut actors = Vec::new();
    state.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|actor| {
            if let Actor::Text { content, .. } = actor {
                Some(content.to_string())
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn french_duplicate_menu_and_status_are_resolved_without_writing_the_song() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let song = "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Été;#NOTES:1000;";
    fs::write(&path, song).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    press(&mut state, KeyCode::F1);
    for _ in 0..15 {
        press(&mut state, KeyCode::ArrowDown);
    }
    assert!(
        texts(&state)
            .iter()
            .any(|text| text == "Dupliquer la partition actuelle")
    );
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.workspace().chart(), 1);
    assert!(
        texts(&state)
            .iter()
            .any(|text| text.contains("Partition dupliquée."))
    );
    assert_eq!(
        state
            .workspace()
            .file()
            .session
            .document()
            .text(Some(1), "DESCRIPTION")
            .as_deref(),
        Some("Été")
    );
    assert_eq!(fs::read_to_string(path).unwrap(), song);
}

#[test]
fn french_empty_chart_layout_summary_cancel_and_create_labels_are_resolved() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let song = "#VERSION:0.83;#BPMS:0=137.5;#NOTEDATA:;#STEPSTYPE:dance-single;#TIMESIGNATURES:0=3=4;#NOTES:1000;";
    fs::write(&path, song).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    for pressed in [true, false] {
        state.raw_key(&RawKeyboardEvent {
            code: KeyCode::SuperLeft,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
        if pressed {
            press(&mut state, KeyCode::KeyN);
        }
    }
    let labels = texts(&state);
    for expected in [
        "Mode : dance-single",
        "Créer une partition vide",
        "Tempo initial : 137.5 BPM, signature : 3/4",
        "Aucune note ni aucun motif",
    ] {
        assert!(
            labels.iter().any(|text| text.contains(expected)),
            "missing {expected}: {labels:?}"
        );
    }
    assert!(!labels.iter().any(|text| text.contains('{')));
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.workspace().chart(), 1);
    assert_eq!(state.workspace().grid().iter().count(), 0);
    assert!(
        texts(&state)
            .iter()
            .any(|text| text.contains("Partition vide créée."))
    );
    assert_eq!(fs::read_to_string(path).unwrap(), song);
}

#[test]
fn french_last_chart_delete_empty_view_undo_and_new_chart_remain_translated() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let song = "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Été;#DIFFICULTY:Hard;#NOTES:1000;";
    fs::write(&path, song).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    for code in [KeyCode::SuperLeft, KeyCode::ShiftLeft] {
        s.raw_key(&RawKeyboardEvent {
            code,
            pressed: true,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
    press(&mut s, KeyCode::Backspace);
    for code in [KeyCode::ShiftLeft, KeyCode::SuperLeft] {
        s.raw_key(&RawKeyboardEvent {
            code,
            pressed: false,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
    let labels = texts(&s);
    assert!(
        labels
            .iter()
            .any(|text| text.contains("Supprimer la dernière partition"))
    );
    assert!(
        labels
            .iter()
            .any(|text| text == "Supprimer cette partition")
    );
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(!s.workspace().has_chart());
    assert!(
        texts(&s)
            .iter()
            .any(|text| text == "Aucune partition active")
    );
    assert!(!texts(&s).iter().any(|text| text.contains('{')));
    press(&mut s, KeyCode::F1);
    assert!(
        texts(&s)
            .iter()
            .any(|text| text == "Créer une partition vide")
    );
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(s.workspace().has_chart());
    assert_eq!(s.workspace().grid().iter().count(), 0);
    assert_eq!(fs::read_to_string(path).unwrap(), song);
}

#[test]
fn french_chart_difficulty_rating_validation_and_pending_timing_are_translated() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let song = "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;";
    fs::write(&path, song).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    press(&mut state, KeyCode::F7);
    let labels = texts(&state);
    assert!(
        labels
            .iter()
            .any(|text| text.contains("Difficulté : Difficile")),
        "{labels:?}"
    );
    assert!(labels.iter().any(|text| text.contains("Niveau : 9")));
    press(&mut state, KeyCode::ArrowUp);
    press(&mut state, KeyCode::ArrowUp);
    press(&mut state, KeyCode::Digit0);
    press(&mut state, KeyCode::Enter);
    assert!(
        texts(&state)
            .iter()
            .any(|text| text.contains("Saisissez un niveau entier"))
    );
    for _ in 0..8 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    assert!(
        texts(&state)
            .iter()
            .any(|text| text.contains("propriétés de la partition avant"))
    );
    press(&mut state, KeyCode::Escape);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        song.as_bytes()
    );
    assert_eq!(fs::read_to_string(path).unwrap(), song);
}

#[test]
fn french_chart_text_panel_labels_input_and_staging_remain_translated() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let song =
        "#VERSION:0.83;#BPMS:0=120;#CREDIT:Auteur;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path, song).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    press(&mut s, KeyCode::F7);
    press(&mut s, KeyCode::ArrowUp);
    assert!(
        texts(&s)
            .iter()
            .any(|text| text == "Nom, description et crédit...")
    );
    press(&mut s, KeyCode::Enter);
    assert!(s.text_input("Été"));
    let labels = texts(&s);
    for expected in [
        "Nom : Été|",
        "Description : ",
        "Crédit : Auteur",
        "Style : ",
        "Retour aux propriétés de la partition",
    ] {
        assert!(
            labels.iter().any(|text| text == expected),
            "{expected}: {labels:?}"
        );
    }
    assert!(!labels.iter().any(|text| text.contains('{')));
    press(&mut s, KeyCode::Escape);
    assert!(
        texts(&s)
            .iter()
            .any(|text| text.contains("Textes préparés."))
    );
    press(&mut s, KeyCode::Escape);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        song.as_bytes()
    );
}

#[test]
fn french_clipboard_failures_are_translated_without_changing_the_chart() {
    struct Unavailable;
    impl deadsync_editor::clipboard::TextClipboard for Unavailable {
        fn read_text(&mut self) -> Result<String, deadsync_editor::clipboard::Error> {
            Err(deadsync_editor::clipboard::Error::NoText)
        }
        fn write_text(&mut self, _: &str) -> Result<(), deadsync_editor::clipboard::Error> {
            Err(deadsync_editor::clipboard::Error::Unavailable)
        }
    }
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let song =
        "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#CHARTNAME:Été;#NOTES:1000;";
    fs::write(&path, song).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    press(&mut s, KeyCode::F7);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::Enter);
    for (code, label) in [
        (KeyCode::KeyX, "Presse-papiers système indisponible"),
        (KeyCode::KeyV, "ne contient pas de texte pris en charge"),
    ] {
        s.raw_key(&RawKeyboardEvent {
            code: KeyCode::SuperLeft,
            pressed: true,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
        press(&mut s, code);
        assert!(s.process_text_clipboard(&mut Unavailable));
        s.raw_key(&RawKeyboardEvent {
            code: KeyCode::SuperLeft,
            pressed: false,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
        assert!(texts(&s).iter().any(|text| text.contains(label)));
    }
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        song.as_bytes()
    );
}
