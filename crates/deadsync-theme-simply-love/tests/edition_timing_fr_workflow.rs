//! Separate locale process, avoiding races with English editor workflows.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn press(screen: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        screen.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
fn texts(screen: &State) -> Vec<String> {
    let mut actors = Vec::new();
    screen.actors(&mut actors);
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
fn french_timing_labels_validation_and_acceptance_are_resolved() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(
        &path,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    let labels = texts(&screen);
    for key in [
        "TimingScope",
        "TimingBeat",
        "TimingBpm",
        "TimingSet",
        "TimingRemove",
        "TimingMove",
        "TimingPrevious",
        "TimingNext",
        "TimingAccept",
        "TimingCancel",
    ] {
        let raw = i18n::tr("Edition", key);
        assert_ne!(raw.as_ref(), key);
    }
    assert!(
        labels
            .iter()
            .any(|text| text == "Timing : cette partition (1 partitions, hérité)")
    );
    assert!(
        labels
            .iter()
            .any(|text| text == "Appliquer le timing (1 BPM)")
    );
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Digit0);
    for _ in 0..6 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.starts_with("Saisir un BPM fini"))
    );
    for _ in 0..5 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Digit1);
    press(&mut screen, KeyCode::Digit5);
    press(&mut screen, KeyCode::Digit0);
    for _ in 0..6 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.starts_with("Timing accepté en une seule action annulable"))
    );
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "BPMS")
            .as_deref(),
        Some("0=150")
    );
}

#[test]
fn french_pause_types_durations_errors_and_combined_acceptance_are_translated() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(
        &path,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    press(&mut screen, KeyCode::Tab);
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.starts_with("Arrêts (après les notes)"))
    );
    for key in [
        "PauseSeconds",
        "PauseSet",
        "PauseRemove",
        "PauseMove",
        "PausePrevious",
        "PauseNext",
        "PauseAccept",
        "PauseStaged",
        "PauseInvalidStop",
        "PauseInvalidDelay",
    ] {
        assert_ne!(i18n::tr("Edition", key).as_ref(), key);
    }
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Digit0);
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.starts_with("Saisir une durée d'arrêt finie et non nulle"))
    );
    press(&mut screen, KeyCode::Digit1);
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Tab);
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.starts_with("Délais (avant les notes)"))
    );
    press(&mut screen, KeyCode::Digit0);
    press(&mut screen, KeyCode::Comma);
    press(&mut screen, KeyCode::Digit5);
    press(&mut screen, KeyCode::Enter);
    for _ in 0..6 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.starts_with("Timing accepté"))
    );
    let document = screen.workspace().file().session.document();
    assert_eq!(document.text(Some(0), "STOPS").as_deref(), Some("0=1"));
    assert_eq!(document.text(Some(0), "DELAYS").as_deref(), Some("0=0.5"));
}

#[test]
fn french_region_headings_lengths_validation_and_paired_acceptance_are_translated() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#WARPS:1=1;#FAKES:0=2;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    for _ in 0..3 {
        press(&mut screen, KeyCode::Tab);
    }
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.starts_with("Warps (longueur en temps)"))
    );
    for key in [
        "RegionLength",
        "RegionSet",
        "RegionRemove",
        "RegionMove",
        "RegionPrevious",
        "RegionNext",
        "RegionAccept",
        "RegionStaged",
        "RegionInvalidLength",
        "RegionSscRequired",
    ] {
        assert_ne!(i18n::tr("Edition", key).as_ref(), key);
    }
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Digit0);
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.starts_with("Saisir une longueur positive d'au moins 1/48"))
    );
    press(&mut screen, KeyCode::Delete);
    press(&mut screen, KeyCode::Digit1);
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.starts_with("Zone préparée"))
    );
    press(&mut screen, KeyCode::Tab);
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.starts_with("Zones de fausses notes (longueur en temps)"))
    );
    press(&mut screen, KeyCode::Digit0);
    press(&mut screen, KeyCode::Comma);
    press(&mut screen, KeyCode::Digit5);
    press(&mut screen, KeyCode::Enter);
    for _ in 0..6 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.starts_with("Timing accepté"))
    );
    let document = screen.workspace().file().session.document();
    assert_eq!(document.text(Some(0), "WARPS").as_deref(), Some("0=1,1=1"));
    assert_eq!(document.text(Some(0), "FAKES").as_deref(), Some("0=0.5"));
}

#[test]
fn french_legacy_review_cancel_and_acceptance_are_translated_and_do_not_save_implicitly() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let before =
        b"#VERSION:0.60;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#OFFSET:0.25;#NOTES:1000;";
    fs::write(&path, before).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    let labels = texts(&screen);
    assert!(labels.iter().any(|text| text.contains("SSC 0.60 -> 0.70")));
    assert!(
        labels
            .iter()
            .any(|text| text.contains("timing local existant de 1 partitions"))
    );
    assert!(
        labels
            .iter()
            .any(|text| text == "Convertir le timing et continuer")
    );
    for key in [
        "LegacyTimingHelp",
        "LegacyTimingSummary",
        "LegacyTimingApply",
        "LegacyTimingCancel",
        "LegacyTimingApplied",
        "LegacyTimingDiscarded",
        "LegacyTimingMissingVersion",
    ] {
        assert_ne!(i18n::tr("Edition", key).as_ref(), key);
    }
    press(&mut screen, KeyCode::Escape);
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.starts_with("Conversion du timing abandonnée"))
    );
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    press(&mut screen, KeyCode::F10);
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.starts_with("Timing SSC converti"))
    );
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(None, "VERSION")
            .as_deref(),
        Some("0.70")
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn french_audio_preview_controls_and_status_are_visible() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(dir.path().join("song.ogg"), b"fixture").unwrap();
    fs::write(&path, b"#VERSION:0.83;#MUSIC:song.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0000\n0000\n0000;").unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    assert!(
        texts(&screen)
            .iter()
            .any(|s| s.contains("Entrée : lecture/pause"))
    );
    press(&mut screen, KeyCode::Enter);
    assert!(screen.is_playing());
    assert!(
        texts(&screen)
            .iter()
            .any(|s| s.contains("Lecture. Entrée : mettre en pause"))
    );
    press(&mut screen, KeyCode::Enter);
    assert!(texts(&screen).iter().any(|s| s.contains("Lecture en pause")));
    fs::remove_file(dir.path().join("song.ogg")).unwrap();
    press(&mut screen, KeyCode::Enter);
    assert!(!screen.is_playing());
    assert!(texts(&screen).iter().any(|s| s.contains("Préécoute audio")));
}

#[test]
fn french_song_sync_panel_explains_scope_and_applies_with_translated_labels() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0000\n0000\n0000;").unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F11);
    let labels = texts(&screen);
    assert!(
        labels
            .iter()
            .any(|s| s.contains("Synchro audio de l’éditeur : 0.009000 s"))
    );
    assert!(labels.iter().any(|s| s.contains("Appliquer les propriétés du morceau")));
    assert!(labels.iter().any(|s| s.contains("Préécoute uniquement")));
    press(&mut screen, KeyCode::Digit0);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|s| s.contains("Propriétés du morceau appliquées"))
    );
}

#[test]
fn french_signature_panel_fraction_input_and_acceptance_are_translated() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(
        &path,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    for _ in 0..10 {
        press(&mut screen, KeyCode::Tab);
    }
    let labels = texts(&screen);
    assert!(labels.iter().any(|s| s.contains("Signatures rythmiques")));
    assert!(labels.iter().any(|s| s.contains("Signature : 4/4")));
    assert!(
        labels
            .iter()
            .any(|s| s.contains("Appliquer les signatures"))
    );
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Delete);
    for code in [KeyCode::Digit2, KeyCode::Slash, KeyCode::Digit2] {
        press(&mut screen, code);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|s| s.contains("Signature préparée"))
    );
    for _ in 0..6 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "TIMESIGNATURES")
            .unwrap(),
        "0=2=2"
    );
}

#[test]
fn french_scroll_controls_allow_reverse_and_zero_with_translated_labels() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(
        &path,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    for _ in 0..9 {
        press(&mut screen, KeyCode::Tab);
    }
    let labels = texts(&screen);
    assert!(
        labels
            .iter()
            .any(|s| s.contains("Vitesses de défilement instantanées"))
    );
    assert!(
        labels
            .iter()
            .any(|s| s.contains("Vitesse de défilement : 1x"))
    );
    assert!(
        labels
            .iter()
            .any(|s| s.contains("Appliquer les vitesses de défilement"))
    );
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Delete);
    press(&mut screen, KeyCode::Minus);
    press(&mut screen, KeyCode::Digit1);
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|s| s.contains("Défilement préparé"))
    );
    press(&mut screen, KeyCode::Delete);
    press(&mut screen, KeyCode::Digit0);
    press(&mut screen, KeyCode::Enter);
    for _ in 0..6 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "SCROLLS")
            .unwrap(),
        "0=0"
    );
}

#[test]
fn french_hold_tick_panel_validates_integer_input_and_accepts_zero() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(
        &path,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    for _ in 0..8 {
        press(&mut screen, KeyCode::Tab);
    }
    let labels = texts(&screen);
    assert!(
        labels
            .iter()
            .any(|s| s.contains("Nombre de ticks des holds"))
    );
    assert!(labels.iter().any(|s| s.contains("Ticks par temps : 4")));
    assert!(
        labels
            .iter()
            .any(|s| s.contains("Appliquer les nombres de ticks"))
    );
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Delete);
    press(&mut screen, KeyCode::Minus);
    press(&mut screen, KeyCode::Digit1);
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|s| s.contains("nombre entier de ticks entre"))
    );
    press(&mut screen, KeyCode::Delete);
    press(&mut screen, KeyCode::Digit0);
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|s| s.contains("Nombre de ticks préparé"))
    );
    for _ in 0..6 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "TICKCOUNTS")
            .unwrap(),
        "0=0"
    );
}

#[test]
fn french_combo_pair_labels_validation_and_staged_acceptance_are_translated() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(
        &path,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    for _ in 0..7 {
        press(&mut screen, KeyCode::Tab);
    }
    let labels = texts(&screen);
    assert!(
        labels
            .iter()
            .any(|s| s.contains("Multiplicateurs de combo"))
    );
    assert!(labels.iter().any(|s| s.contains("Réussite / raté : 1x/1x")));
    assert!(
        labels
            .iter()
            .any(|s| s.contains("Appliquer les multiplicateurs de combo"))
    );
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Delete);
    for code in [KeyCode::Digit2, KeyCode::Slash, KeyCode::Digit3] {
        press(&mut screen, code);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|s| s.contains("Saisissez une paire comme 2x/3x"))
    );
    press(&mut screen, KeyCode::Delete);
    for code in [
        KeyCode::Digit2,
        KeyCode::KeyX,
        KeyCode::Slash,
        KeyCode::Digit0,
        KeyCode::KeyX,
    ] {
        press(&mut screen, code);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|s| s.contains("Multiplicateurs de combo préparés"))
    );
    for _ in 0..6 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "COMBOS")
            .unwrap(),
        "0=2=0"
    );
}

#[test]
fn french_interpolated_speed_units_validation_and_acceptance_are_visible() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(
        &path,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    for _ in 0..6 {
        press(&mut screen, KeyCode::Tab);
    }
    let labels = texts(&screen);
    assert!(
        labels
            .iter()
            .any(|s| s.contains("Vitesses de défilement interpolées"))
    );
    assert!(
        labels
            .iter()
            .any(|s| s.contains("Vitesse / transition : 1x/0rows"))
    );
    assert!(
        labels
            .iter()
            .any(|s| s.contains("Appliquer les vitesses interpolées"))
    );
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Delete);
    for code in [
        KeyCode::Digit2,
        KeyCode::KeyX,
        KeyCode::Slash,
        KeyCode::Digit1,
    ] {
        press(&mut screen, code);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|s| s.contains("Saisissez 2x/48rows"))
    );
    press(&mut screen, KeyCode::Delete);
    for code in [KeyCode::Digit2, KeyCode::KeyX] {
        press(&mut screen, code);
    }
    screen.raw_key(&RawKeyboardEvent {
        code: KeyCode::ShiftLeft,
        pressed: true,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    press(&mut screen, KeyCode::Period);
    screen.raw_key(&RawKeyboardEvent {
        code: KeyCode::ShiftLeft,
        pressed: false,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    for code in [
        KeyCode::Digit4,
        KeyCode::Digit8,
        KeyCode::KeyR,
        KeyCode::KeyO,
        KeyCode::KeyZ,
        KeyCode::KeyS,
    ] {
        press(&mut screen, code);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|s| s.contains("Vitesse / transition : 2x/48rows"))
    );
    press(&mut screen, KeyCode::Delete);
    for code in [
        KeyCode::Digit2,
        KeyCode::KeyX,
        KeyCode::Slash,
        KeyCode::Digit0,
        KeyCode::Period,
        KeyCode::Digit5,
        KeyCode::KeyS,
    ] {
        press(&mut screen, code);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(
        texts(&screen)
            .iter()
            .any(|s| s.contains("Vitesse interpolée préparée"))
    );
    for _ in 0..6 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "SPEEDS")
            .unwrap(),
        "0=2=0.5=1"
    );
}

#[test]
fn french_section_labels_use_committed_unicode_text_and_translated_controls() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(
        &path,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    for _ in 0..5 {
        press(&mut screen, KeyCode::Tab);
    }
    let labels = texts(&screen);
    assert!(labels.iter().any(|s| s.contains("Labels de section")));
    assert!(labels.iter().any(|s| s.contains("Label : Nouveau label")));
    assert!(labels.iter().any(|s| s.contains("Appliquer les labels")));
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    screen.text_preedit("é");
    assert!(texts(&screen).iter().any(|s| s.contains("Label : é")));
    assert!(screen.text_input("Étiquette 日本語:fin"));
    press(&mut screen, KeyCode::Enter);
    assert!(texts(&screen).iter().any(|s| s.contains("Label préparé")));
    for _ in 0..6 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "LABELS")
            .unwrap(),
        "0=Étiquette 日本語_fin"
    );
    assert!(texts(&screen).iter().any(|s| s == "Étiquette 日本語_fin"));
}

#[test]
fn timing_menu_keeps_attack_rows_and_commits_attack_export_in_the_same_undo() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    fn ctrl(screen: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            screen.raw_key(&RawKeyboardEvent {
                code: KeyCode::SuperLeft,
                pressed,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            });
            if pressed {
                press(screen, code);
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source=b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=1:LEN=2:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path, source).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F10);
    for _ in 0..2 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    for code in [
        KeyCode::Delete,
        KeyCode::Digit2,
        KeyCode::Digit4,
        KeyCode::Digit0,
    ] {
        press(&mut screen, code);
    }
    for _ in 0..6 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    let timeline = deadsync_editor::attacks::placement::Timeline::capture(
        &screen.workspace().file().session,
        0,
    )
    .unwrap();
    assert_eq!(timeline.markers()[0].row, 96);
    assert_eq!(timeline.markers()[0].chart_time, 0.5);
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(
        deadsync_editor::attacks::placement::Timeline::capture(
            &reopened.workspace().file().session,
            0
        )
        .unwrap()
        .markers()[0]
            .row,
        96
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        source
    );
}
