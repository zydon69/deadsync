include!("support/chart_order_transport.rs");
#[path = "support/transport_host.rs"]
mod transport_host;
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
    complete_chart_navigation(&mut s);
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
    press(&mut state, KeyCode::ArrowUp);
    press(&mut state, KeyCode::Digit0);
    press(&mut state, KeyCode::Enter);
    assert!(
        texts(&state)
            .iter()
            .any(|text| text.contains("Saisissez un niveau entier"))
    );
    for _ in 0..9 {
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
    press(&mut s, KeyCode::ArrowUp);
    assert!(texts(&s).iter().any(|text| text == "Textes et musique..."));
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

#[test]
fn french_music_picker_clear_and_cancel_labels_are_resolved_without_saving() {
    struct Cancel;
    impl deadsync_editor::music_file::Picker for Cancel {
        fn choose(
            &mut self,
            _: &deadsync_editor::music_file::Browse,
        ) -> Result<Option<std::path::PathBuf>, deadsync_editor::Error> {
            Ok(None)
        }
    }
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let song =
        "#VERSION:0.83;#MUSIC:song.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path, song).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    press(&mut s, KeyCode::F7);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::ArrowUp);
    press(&mut s, KeyCode::Enter);
    for _ in 0..4 {
        press(&mut s, KeyCode::Tab);
    }
    assert!(s.wants_text_input());
    assert!(s.text_input("Été.ogg"));
    assert!(
        texts(&s)
            .iter()
            .any(|text| text.contains("Musique : Été.ogg|"))
    );
    press(&mut s, KeyCode::F2);
    assert!(s.process_music_picker(&mut Cancel));
    assert!(
        texts(&s)
            .iter()
            .any(|text| text.contains("Sélection audio annulée"))
    );
    press(&mut s, KeyCode::F3);
    assert!(
        texts(&s)
            .iter()
            .any(|text| text.contains("Remplacement audio effacé"))
    );
    assert_eq!(
        &*i18n::tr("Edition", "MusicPickerTitle"),
        "Choisir un fichier audio"
    );
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        song.as_bytes()
    );
}

#[test]
fn french_song_properties_pages_text_music_and_cancel_use_shared_translated_controls() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let song = "#VERSION:0.83;#TITLE:Ancien;#ARTIST:A;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path, song).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    press(&mut s, KeyCode::F11);
    assert!(
        texts(&s)
            .iter()
            .any(|v| v == "Textes et musique du morceau...")
    );
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(s.text_input("Été 日本語"));
    assert!(texts(&s).iter().any(|v| v == "Titre : Été 日本語|"));
    press(&mut s, KeyCode::PageDown);
    let second = texts(&s);
    assert!(second.iter().any(|v| v.contains("Champs 2/3")),
        "{second:?}"
    );
    assert!(
        second
            .iter()
            .any(|v| v.contains("Translittération de l’artiste"))
    );
    press(&mut s, KeyCode::ArrowDown);
    assert!(s.wants_text_input());
    assert!(
        texts(&s)
            .iter()
            .any(|v| v.contains("Musique du morceau : | (aucune musique pour le morceau)"))
    );
    press(&mut s, KeyCode::F3);
    assert!(
        texts(&s)
            .iter()
            .any(|v| v.contains("Musique du morceau effacée"))
    );
    press(&mut s, KeyCode::Escape);
    assert!(
        texts(&s)
            .iter()
            .any(|v| v.contains("Textes du morceau préparés"))
    );
    press(&mut s, KeyCode::Escape);
    assert_eq!(
        s.workspace().file().session.serialize().unwrap(),
        song.as_bytes()
    );
}

#[test]
fn french_song_music_offset_and_itg_explanation_are_keyboard_accessible() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let song =
        "#VERSION:0.83;#BPMS:0=120;#OFFSET:0.125;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path, song).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    press(&mut state, KeyCode::F11);
    assert!(
        texts(&state)
            .iter()
            .any(|s| s.contains("Décalage musique du morceau (OFFSET) : 0.125000 s"))
    );
    assert!(
        texts(&state)
            .iter()
            .any(|s| s == i18n::tr("Edition", "SongPropertiesHelp").as_ref())
    );
    press(&mut state, KeyCode::Digit0);
    for _ in 0..5 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    assert!(
        texts(&state)
            .iter()
            .any(|s| s.contains("9 ms ajoutées au décalage musique"))
    );
    assert!(texts(&state).iter().any(|s| s.contains("0.134000 s")));
    press(&mut state, KeyCode::Escape);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        song.as_bytes()
    );
}

#[test]
fn french_end_hint_explains_song_end_in_both_formats() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    for sm in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(if sm { "song.sm" } else { "song.ssc" });
        fs::write(&path,if sm {"#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;"} else {"#VERSION:0.83;#BPMS:0=120;#LASTSECONDHINT:12.5;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"}).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        press(&mut state, KeyCode::F11);
        press(&mut state, KeyCode::F6);
        press(&mut state, KeyCode::Tab);
        press(&mut state, KeyCode::Tab);
        let labels = texts(&state);
        assert!(
            labels
                .iter()
                .any(|s| s.contains("continuer après la dernière note"))
        );
        assert!(labels.iter().any(|s|s.contains(if sm {"Indication de fin : 0.000000 s"}else{"Indication de fin : 12.500000 s"})),"{labels:?}");
        assert!(!labels.iter().any(|s|s.contains("indisponible en SM")));
    }
}

#[test]
fn french_waveform_preferences_show_translated_controls_and_apply_without_song_edits() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source = "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    state.raw_key(&RawKeyboardEvent {
        code: KeyCode::ShiftLeft,
        pressed: true,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    press(&mut state, KeyCode::KeyW);
    state.raw_key(&RawKeyboardEvent {
        code: KeyCode::ShiftLeft,
        pressed: false,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    let labels = texts(&state).join("\n");
    for expected in [
        "Afficher à l’ouverture de l’éditeur : Oui",
        "Activer le décodage",
        "Largeur des canaux : 90.0",
        "Intensité de la zone dense : 6.00",
        "Défilement de la forme d’onde : Tempo le plus fréquent",
        "Enregistrer les préférences de forme d’onde",
        "Abandonner et revenir",
    ] {
        assert!(labels.contains(expected), "{expected}: {labels}");
    }
    for _ in 0..4 { press(&mut state, KeyCode::ArrowDown); }
    press(&mut state, KeyCode::ArrowRight);
    assert!(texts(&state).iter().any(|line| line.contains("Défilement de la forme d’onde : Tempo courant")));
    press(&mut state, KeyCode::ArrowRight);
    assert!(texts(&state).iter().any(|line| line.contains("Tempo courant et multiplicateurs")));
    press(&mut state, KeyCode::Home);
    for _ in 0..10 { press(&mut state, KeyCode::ArrowDown); }
    press(&mut state, KeyCode::Enter);
    assert!(
        texts(&state)
            .iter()
            .any(|line| line == i18n::tr("Edition", "WaveformSettingsSaved").as_ref()),
        "{:?}",
        texts(&state)
    );
    assert_eq!(
        state.waveform_preferences().scroll_mode(),
        deadsync_editor::waveform_preferences::Preferences::default().scroll_mode()
    );
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source.as_bytes()
    );
}

#[test]
fn french_waveform_colours_are_paginated_and_keep_edits_staged_until_parent_save() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.0,480.0,
    ));
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("song.ssc");
    let source="#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path,source).unwrap();let mut state=State::open(&path,0).unwrap();
    state.raw_key(&RawKeyboardEvent {code:KeyCode::ShiftLeft,pressed:true,repeat:false,timestamp:Instant::now(),host_nanos:0,
    });
    press(&mut state,KeyCode::KeyW);
    state.raw_key(&RawKeyboardEvent {code:KeyCode::ShiftLeft,pressed:false,repeat:false,timestamp:Instant::now(),host_nanos:0,
    });
    press(&mut state,KeyCode::F2);
    assert!(texts(&state).iter().any(|v|v.contains("Plus sombre que la zone dense")));
    assert!(texts(&state).iter().any(|v|v.contains("Couleurs de forme d’onde 1/2")));
    press(&mut state,KeyCode::ArrowRight);
    assert!(texts(&state).iter().any(|v|v.contains("Identique à la zone dense")));
    for _ in 0..13 {press(&mut state,KeyCode::ArrowDown);}
    assert!(texts(&state).iter().any(|v|v.contains("Couleurs de forme d’onde 2/2")));
    assert!(texts(&state).iter().any(|v|v.contains("Fond, opacité : 0.000")));
    press(&mut state,KeyCode::ArrowRight);press(&mut state,KeyCode::Escape);press(&mut state,KeyCode::Escape);
    assert_eq!(state.waveform_preferences().palette(),Default::default());
    assert_eq!(state.workspace().file().session.serialize().unwrap(),source.as_bytes());
}

#[test]
fn french_transport_messages_distinguish_pause_and_resume_without_editing_chart() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("music.ogg"), b"command-only asset").unwrap();
    let path = dir.path().join("pause.ssc");
    let source = "#VERSION:0.83;#MUSIC:music.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    transport_host::publish(&mut state);
    press(&mut state, KeyCode::Enter);
    assert!(
        texts(&state)
            .iter()
            .any(|s| s == i18n::tr("Edition", "PlaybackPlaying").as_ref())
    );
    state.update_audio(transport_host::snapshot(&state, 0.126));
    press(&mut state, KeyCode::Enter);
    assert!(
        texts(&state)
            .iter()
            .any(|s| s == "Lecture en pause. Entrée : reprendre à la même position.")
    );
    assert!(!state.is_playing());
    press(&mut state, KeyCode::Enter);
    assert!(state.is_playing());
    assert!(
        texts(&state)
            .iter()
            .any(|s| s == i18n::tr("Edition", "PlaybackPlaying").as_ref())
    );
    press(&mut state, KeyCode::Enter);
    state.take_audio_commands();
    let position = state.chart_focus_time().unwrap();
    press(&mut state, KeyCode::F1);
    assert!(!state.is_playing());
    assert_eq!(state.chart_focus_time().unwrap(), position);
    assert!(state.take_audio_commands().is_empty());
    assert!(!state.workspace().file().session.is_dirty());
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}
