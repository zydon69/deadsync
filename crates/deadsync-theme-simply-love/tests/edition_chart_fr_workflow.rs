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
