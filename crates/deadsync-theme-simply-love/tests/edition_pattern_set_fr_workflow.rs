//! Separate process so changing the locale cannot race English UI workflows.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    document::{Document, Format},
    generation::metadata::{ChartData, Definition, PATTERN_BALANCED, SIXTEENTHS},
    history::Session,
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    collections::BTreeMap,
    fs,
    sync::Arc,
    time::{Duration, Instant},
};
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
        .filter_map(|actor| match actor {
            Actor::Text { content, .. } => Some(content.to_string()),
            _ => None,
        })
        .collect()
}
#[test]
fn french_pattern_set_selector_errors_preview_and_acceptance_have_resolved_labels() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let bytes=b"#VERSION:0.83;#TITLE:Essai;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000\n0100\n0010\n0001;";
    let mut source =
        Session::new(Document::parse(Arc::<[u8]>::from(bytes.as_slice()), Format::Ssc).unwrap());
    ChartData {
        patterns: Some(BTreeMap::from([(
            48,
            Definition {
                length: 48,
                random_seed: 42,
                pattern_config_guid: SIXTEENTHS.parse().unwrap(),
                performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                start_position_inclusive: true,
                end_position_inclusive: true,
                ..Definition::default()
            },
        )])),
        ..ChartData::default()
    }
    .save(&mut source, 0)
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fr.ssc");
    fs::write(&path, source.serialize().unwrap()).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    press(&mut state, KeyCode::F9);
    assert!(
        texts(&state)
            .iter()
            .any(|text| text == "Inclure ce marqueur : Oui")
    );
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowRight);
    assert!(
        texts(&state)
            .iter()
            .any(|text| text == "Inclure ce marqueur : Non")
    );
    for _ in 0..4 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    assert!(!state.is_generating());
    assert!(
        texts(&state)
            .iter()
            .any(|text| text.contains("Sélectionnez au moins un marqueur"))
    );
    for _ in 0..4 {
        press(&mut state, KeyCode::ArrowUp);
    }
    press(&mut state, KeyCode::Enter);
    for _ in 0..4 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    let deadline = Instant::now() + Duration::from_secs(30);
    while state.is_generating() {
        state.update();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(state.pattern_set_preview().is_some());
    let labels = texts(&state);
    assert!(labels.iter().any(|text| text.contains("Motifs : 1 prêts")));
    assert!(
        labels
            .iter()
            .any(|text| text == "Appliquer toutes les modifications de notes")
    );
    assert!(labels.iter().all(|text| !text.contains('{')
        && !text.contains("PatternSet")
        && !text.contains("Include this marker")));
    press(&mut state, KeyCode::Enter);
    assert!(
        texts(&state)
            .iter()
            .any(|text| text.contains("Motifs acceptés"))
    );
}
