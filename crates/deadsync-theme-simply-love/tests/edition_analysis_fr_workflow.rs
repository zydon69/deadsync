//! Separate locale process for the native RSSP inspector.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    time::{Duration, Instant},
};
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
fn texts(s: &State) -> Vec<String> {
    let mut actors = Vec::new();
    s.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|a| {
            if let Actor::Text { content, .. } = a {
                Some(content.to_string())
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn french_analysis_menu_stats_density_and_navigation_have_resolved_labels() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let song = "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0000\n0000\n0000,\n0100\n0000\n0000\n0000;";
    fs::write(&path, song).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    press(&mut s, KeyCode::F1);
    assert!(
        texts(&s)
            .iter()
            .any(|t| t.contains("Actions 1/3 - Haut/Bas"))
    );
    for _ in 0..14 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    let limit = Instant::now() + Duration::from_secs(10);
    while s.is_analyzing() {
        s.update();
        assert!(Instant::now() < limit);
        std::thread::yield_now();
    }
    assert!(s.analysis_report().is_some());
    let labels = texts(&s);
    for expected in [
        "Analyse de la partition (RSSP)",
        "Flèches : 2",
        "Pic :",
        "Croisements :",
        "Intervalles continus :",
        "Aucune séquence continue.",
        "Densité par quatre temps",
        "Intervalle 1/2",
        "Entrée : rejoindre",
    ] {
        assert!(
            labels.iter().any(|text| text.contains(expected)),
            "missing {expected}: {labels:?}"
        );
    }
    assert!(!labels.iter().any(|t| t.contains('{')));
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.workspace().row(), 192);
    assert!(!s.workspace().file().session.is_dirty());
    assert_eq!(fs::read_to_string(path).unwrap(), song);
    let unsupported_path = dir.path().join("routine.ssc");
    let unsupported_song = "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:10000000\n00000000\n00000000\n00000000\n&\n00001000\n00000000\n00000000\n00000000;";
    fs::write(&unsupported_path, unsupported_song).unwrap();
    let mut unsupported = State::open(&unsupported_path, 0).unwrap();
    press(&mut unsupported, KeyCode::F12);
    let limit = Instant::now() + Duration::from_secs(10);
    while unsupported.is_analyzing() {
        unsupported.update();
        assert!(Instant::now() < limit);
        std::thread::yield_now();
    }
    assert!(unsupported.analysis_report().is_none());
    assert!(
        texts(&unsupported)
            .iter()
            .any(|text| text == "RSSP ne prend pas en charge le mode dance-routine.")
    );
}
