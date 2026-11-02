use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::Actor,
    space::{Metrics, set_current_metrics},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{collections::BTreeSet, fs, sync::Mutex, time::Instant};
static SERIAL: Mutex<()> = Mutex::new(());
fn press(state: &mut State, code: KeyCode) -> bool {
    let mut closed = false;
    for pressed in [true, false] {
        closed |= state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
    closed
}
fn setup(language: &str, width: f32, height: f32) -> (tempfile::TempDir, State) {
    i18n::init(deadsync_assets::language::load_for_tests(language));
    set_current_metrics(Metrics::centered(width, height));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("help.ssc");
    fs::write(
        &path,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut state = State::open(&path, 0).unwrap();
    press(&mut state, KeyCode::F1);
    for _ in 0..12 {
        press(&mut state, KeyCode::ArrowDown);
    }
    press(&mut state, KeyCode::Enter);
    (dir, state)
}
fn actors(state: &State) -> Vec<Actor> {
    let mut result = Vec::new();
    state.actors(&mut result);
    result
}
fn title(state: &State) -> String {
    actors(state)
        .into_iter()
        .find_map(|actor| match actor {
            Actor::Text {
                content, z: 8050, ..
            } => Some(content.to_string()),
            _ => None,
        })
        .expect("help page title")
}
#[test]
fn every_help_entry_is_keyboard_reachable_and_bounded_in_english_and_french() {
    let _guard = SERIAL.lock().unwrap();
    for language in ["en", "fr"] {
        for (width, height) in [(854.0, 480.0), (640.0, 360.0)] {
            let (_dir, mut state) = setup(language, width, height);
            let mut pages = BTreeSet::new();
            let mut entries = BTreeSet::new();
            // One help row per configurable action can exceed64 pages on a
            // compact viewport; retain the wrap/end guard with a catalog bound.
            for _ in 0..deadsync_editor::key_binding_preferences::Action::ALL.len() + 64 {
                if !pages.insert(title(&state)) {
                    break;
                }
                for actor in actors(&state) {
                    if let Actor::Text {
                        content,
                        offset,
                        z: 8051,
                        max_width,
                        max_height,
                        wrap_width_pixels,
                        ..
                    } = actor
                    {
                        assert_eq!(max_width, Some(width - 70.0));
                        assert_eq!(max_height, Some(52.0));
                        assert!(wrap_width_pixels.is_some());
                        assert!(offset[1] >= 80.0 && offset[1] + 52.0 <= height - 48.0);
                        entries.insert(content.to_string());
                    }
                }
                press(&mut state, KeyCode::PageDown);
            }
            assert!(pages.len() > 1);
            for line in i18n::tr("Edition", "Help")
                .lines()
                .filter(|line| !line.trim().is_empty())
            {
                assert!(entries.contains(line), "unreachable help entry: {line}");
            }
            for key in [
                "HelpViews",
                "HelpMinimap",
                "HelpChartCopy",
                "HelpEventShift",
            ] {
                assert!(entries.contains(i18n::tr("Edition", key).as_ref()));
            }
            press(&mut state, KeyCode::Home);
            let first = title(&state);
            press(&mut state, KeyCode::End);
            assert_ne!(title(&state), first);
            press(&mut state, KeyCode::Home);
            assert_eq!(title(&state), first);
        }
    }
}
#[test]
fn modal_help_blocks_edits_and_escape_closes_help_without_closing_or_changing_song() {
    let _guard = SERIAL.lock().unwrap();
    for language in ["en", "fr"] {
        let (_dir, mut state) = setup(language, 854.0, 480.0);
        let before = state.workspace().file().session.serialize().unwrap();
        let row = state.workspace().row();
        press(&mut state, KeyCode::Digit1);
        press(&mut state, KeyCode::Delete);
        press(&mut state, KeyCode::ArrowDown);
        assert!(!press(&mut state, KeyCode::Escape));
        assert!(
            !actors(&state)
                .iter()
                .any(|actor| matches!(actor, Actor::Text { z: 8050, .. }))
        );
        assert_eq!(state.workspace().row(), row);
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            before
        );
        assert!(!state.workspace().file().session.is_dirty());
        press(&mut state, KeyCode::Digit1);
        assert!(state.workspace().file().session.is_dirty());
    }
}
