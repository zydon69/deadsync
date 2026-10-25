//! Native editor shortcut -> domain command -> saved file -> reopen. No user
//! data is touched, and no synthetic key event is described as physical input.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::notes::Position;
use deadsync_theme_simply_love::screens::edition::State;
use std::{fs, time::Instant};

const SONG: &[u8] = b"#TITLE:Test;#NOTEDATA:;#STEPSTYPE:dance-routine;#DIFFICULTY:Hard;#METER:9;#NOTES:\n10000000\n00000000\n00000000\n00000000\n&\n00001000\n00000000\n00000000\n00000000\n;";

fn key(state: &mut State, code: KeyCode, pressed: bool) -> bool {
    state.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    })
}
fn press(state: &mut State, code: KeyCode) -> bool {
    let closed = key(state, code, true);
    key(state, code, false);
    closed
}
fn ctrl(state: &mut State, code: KeyCode) {
    key(state, KeyCode::SuperLeft, true);
    press(state, code);
    key(state, KeyCode::SuperLeft, false);
}

#[test]
fn keyboard_edit_undo_redo_and_save_are_observable_after_reopening() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open_selected(&path, "dance-routine", "Hard", "", 9).unwrap();
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Digit2);
    assert!(
        screen
            .workspace()
            .grid()
            .get(Position {
                player: 0,
                lane: 1,
                row: 48
            })
            .is_some()
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert!(!screen.workspace().file().session.is_dirty());
    ctrl(&mut screen, KeyCode::KeyY);
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert!(
        reopened
            .workspace()
            .grid()
            .get(Position {
                player: 0,
                lane: 1,
                row: 48
            })
            .is_some()
    );
    assert!(press(&mut screen, KeyCode::Escape));
}

#[test]
fn unsaved_close_requires_an_explicit_discard_confirmation() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::Digit2);
    assert!(!press(&mut screen, KeyCode::Escape));
    assert!(!press(&mut screen, KeyCode::Enter)); // default: keep editing
    assert!(screen.workspace().file().session.is_dirty());
    assert!(!press(&mut screen, KeyCode::Escape));
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    assert!(!press(&mut screen, KeyCode::Enter)); // request discard, not yet confirmed
    assert!(press(&mut screen, KeyCode::Enter));
    assert_eq!(fs::read(&path).unwrap(), SONG);
}

#[test]
fn participant_shortcut_edits_player_two_and_renders_both_owners() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::KeyP);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Digit7);
    assert!(
        screen
            .workspace()
            .grid()
            .get(Position {
                player: 1,
                lane: 6,
                row: 48
            })
            .is_some()
    );
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    let arrows: Vec<_> = actors
        .iter()
        .filter_map(|actor| match actor {
            Actor::Sprite { tint, z: 8004, .. } => Some(*tint),
            _ => None,
        })
        .collect();
    assert!(
        arrows.iter().any(|color| color[2] > color[0]),
        "P1 blue arrows must be present"
    );
    assert!(
        arrows.iter().any(|color| color[0] > color[2]),
        "P2 pink arrows must be present"
    );
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert!(
        reopened
            .workspace()
            .grid()
            .get(Position {
                player: 1,
                lane: 6,
                row: 48
            })
            .is_some()
    );
}

#[test]
fn failed_save_in_close_prompt_preserves_edits_and_does_not_close() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::Digit2);
    fs::write(&path, b"external changes").unwrap();
    assert!(!press(&mut screen, KeyCode::Escape));
    press(&mut screen, KeyCode::ArrowDown);
    assert!(!press(&mut screen, KeyCode::Enter));
    assert!(screen.workspace().file().session.is_dirty());
    assert_eq!(fs::read(&path).unwrap(), b"external changes");
}

const SINGLE: &[u8] = b"#VERSION:0.83;#TITLE:Test;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000\n0100\n0010\n0001;";
fn start_generation(screen: &mut State) {
    press(screen, KeyCode::F3);
    press(screen, KeyCode::ArrowRight); // dance-single -> dance-double
    for _ in 0..3 {
        press(screen, KeyCode::ArrowDown);
    }
    press(screen, KeyCode::Enter);
}
fn finish_generation(screen: &mut State) {
    let deadline = Instant::now() + std::time::Duration::from_secs(30);
    while screen.is_generating() {
        screen.update();
        assert!(
            Instant::now() < deadline,
            "Native generation did not finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
#[test]
fn generation_shortcut_previews_without_editing_then_applies_undoes_and_saves() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SINGLE).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    start_generation(&mut screen);
    press(&mut screen, KeyCode::Digit1); // no editing during calculation or preview
    finish_generation(&mut screen);
    assert_eq!(screen.generation_report().unwrap().notes, 4);
    assert_eq!(
        screen.workspace().file().session.document().charts().len(),
        1
    );
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        SINGLE
    );
    assert_eq!(fs::read(&path).unwrap(), SINGLE);
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(actors.iter().any(|actor| matches!(actor, Actor::Text {content, z:8021,..} if content.contains("dance-double") && !content.contains("{notes}"))));
    press(&mut screen, KeyCode::PageDown);
    press(&mut screen, KeyCode::Home);
    press(&mut screen, KeyCode::Enter); // explicit application
    assert!(screen.generation_report().is_none());
    assert_eq!(screen.workspace().chart(), 1);
    assert_eq!(screen.workspace().grid().columns(), 8);
    assert_eq!(fs::read(&path).unwrap(), SINGLE);
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(screen.workspace().chart(), 0);
    assert_eq!(screen.workspace().grid().columns(), 4);
    ctrl(&mut screen, KeyCode::KeyY);
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 1).unwrap();
    assert_eq!(reopened.workspace().grid().columns(), 8);
    assert_eq!(reopened.workspace().grid().iter().count(), 4);
}
#[test]
fn preview_discard_and_worker_cancel_leave_source_and_history_untouched() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SINGLE).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    start_generation(&mut screen);
    finish_generation(&mut screen);
    assert!(screen.generation_report().is_some());
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    assert!(screen.generation_report().is_none());
    assert!(!screen.workspace().file().session.is_dirty());
    start_generation(&mut screen);
    press(&mut screen, KeyCode::Escape);
    finish_generation(&mut screen);
    assert!(screen.generation_report().is_none());
    assert_eq!(screen.workspace().file().session.revision(), 0);
    assert_eq!(fs::read(&path).unwrap(), SINGLE);
}
#[test]
fn unsupported_generation_reports_failure_without_modifying_multiplayer_chart() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    start_generation(&mut screen);
    finish_generation(&mut screen);
    assert!(screen.generation_report().is_none());
    assert_eq!(screen.workspace().grid().players(), 2);
    assert_eq!(screen.workspace().file().session.revision(), 0);
    assert_eq!(fs::read(&path).unwrap(), SONG);
}

fn start_pattern(screen: &mut State) {
    press(screen, KeyCode::F4);
    for _ in 0..7 {
        press(screen, KeyCode::ArrowDown);
    }
    press(screen, KeyCode::Enter);
}
#[test]
fn pattern_shortcut_uses_selection_previews_applies_and_saves_one_undo_gesture() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SINGLE).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Space);
    press(&mut screen, KeyCode::ArrowDown);
    assert_eq!(screen.workspace().selection(), Some((48, 96)));
    start_pattern(&mut screen);
    press(&mut screen, KeyCode::Digit1); // blocked during calculation
    finish_generation(&mut screen);
    let (report, nps) = screen
        .pattern_report()
        .expect("Pattern preview should be ready");
    assert_eq!(report.removed, 2);
    assert!(nps > 0.0);
    assert!(screen.generation_report().is_none());
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        SINGLE
    );
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(actors.iter().any(|a|matches!(a,Actor::Text{content,z:8021,..} if content.contains("5 steps") && content.contains("1.000 - 2.000") && !content.contains("{removed}"))));
    assert!(actors.iter().any(|a|matches!(a,Actor::Text{content,z:8021,..} if content.as_ref()=="Apply notes and keep this pattern marker")));
    press(&mut screen, KeyCode::PageDown);
    press(&mut screen, KeyCode::Home);
    press(&mut screen, KeyCode::Enter);
    assert!(screen.pattern_report().is_none());
    assert_eq!(screen.workspace().chart(), 0);
    assert_eq!(
        screen.workspace().file().session.document().charts().len(),
        1
    );
    assert_eq!(
        screen.workspace().file().session.undo_label(),
        Some("Generate pattern")
    );
    assert_eq!(screen.workspace().grid().iter().count(), 7);
    assert_eq!(fs::read(&path).unwrap(), SINGLE);
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        SINGLE
    );
    ctrl(&mut screen, KeyCode::KeyY);
    ctrl(&mut screen, KeyCode::KeyS);
    assert_eq!(
        State::open(&path, 0)
            .unwrap()
            .workspace()
            .grid()
            .iter()
            .count(),
        7
    );
    let mut reopened = State::open(&path, 0).unwrap();
    assert!(
        deadsync_editor::generation::metadata::ChartData::read(
            reopened.workspace().file().session.document(),
            0
        )
        .unwrap()
        .unwrap()
        .patterns
        .as_ref()
        .unwrap()
        .contains_key(&48)
    );
    press(&mut reopened, KeyCode::ArrowDown);
    press(&mut reopened, KeyCode::F5);
    finish_generation(&mut reopened);
    assert!(reopened.pattern_report().is_some());
}
#[test]
fn pattern_discard_cancel_and_unsupported_chart_preserve_source() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SINGLE).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    start_pattern(&mut screen);
    finish_generation(&mut screen);
    assert!(screen.pattern_report().is_some());
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(actors.iter().any(|a|matches!(a,Actor::Text{content,z:8021,..} if content.contains("17 steps") && content.contains("0.000 - 4.000"))));

    // A transient small viewport during resize must not panic in the fixed
    // pattern-region highlight's clamp operation.
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        100.0, 100.0,
    ));
    actors.clear();
    screen.actors(&mut actors);
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    assert!(screen.pattern_report().is_none());
    start_pattern(&mut screen);
    press(&mut screen, KeyCode::Enter); // cancels even if compute finished before next update
    finish_generation(&mut screen);
    assert!(screen.pattern_report().is_none());
    assert_eq!(screen.workspace().file().session.revision(), 0);
    assert_eq!(fs::read(&path).unwrap(), SINGLE);
    fs::write(&path, SONG).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    start_pattern(&mut screen);
    finish_generation(&mut screen);
    assert!(screen.pattern_report().is_none());
    assert_eq!(screen.workspace().file().session.serialize().unwrap(), SONG);
}
#[test]
fn pattern_menu_settings_and_exclusive_endpoints_reach_the_generation_task() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SINGLE).unwrap();
    let mut screen =
        State::open_with_profiles(&path, 0, &dir.path().join("AutogenConfigs")).unwrap();
    press(&mut screen, KeyCode::Space);
    press(&mut screen, KeyCode::ArrowDown); // selection 0..48
    press(&mut screen, KeyCode::F1);
    for _ in 0..6 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    // Pattern rows: range, subdivision, profile, seed, start, end, distribution, generate.
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowLeft); // 1/12: 16 rows
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight); // no candles
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowLeft); // signed seed -1
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight); // exclude start
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight); // exclude end
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight); // ignore distribution
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    for expected in [
        "Subdivision: 1/12",
        "Random seed: -1",
        "Include start: No",
        "Include end: No",
        "Ignore preceding lane distribution: Yes",
    ] {
        assert!(
            actors
                .iter()
                .any(|a| matches!(a,Actor::Text{content,z:8021,..} if content.as_ref()==expected)),
            "{expected}"
        );
    }
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    finish_generation(&mut screen);
    assert_eq!(screen.pattern_report().unwrap().0.removed, 0);
    press(&mut screen, KeyCode::Enter);
    let grid = screen.workspace().grid();
    for row in [0, 16, 32, 48, 96, 144] {
        assert!(grid.iter().any(|(p, _)| p.row == row), "Missing row {row}");
    }
    assert_eq!(grid.iter().count(), 6);
    use deadsync_editor::{
        document::{Document, Format},
        generation::{
            config::{PatternConfig, PerformedConfig, PerformedPreset},
            job::ChartOptions,
            pattern_job::{Options, Span, Task},
        },
        history::Session,
    };
    let source =
        Session::new(Document::parse(std::sync::Arc::<[u8]>::from(SINGLE), Format::Ssc).unwrap());
    let expected = Task::capture(
        &source,
        0,
        Options {
            span: Span {
                start: 0,
                end: 48,
                include_start: false,
                include_end: false,
            },
            seed: -1,
            expressed: ChartOptions::balanced("dance-single").expressed,
            performed: PerformedConfig::preset(PerformedPreset::PatternNoCandles),
            pattern: PatternConfig {
                beat_sub_division: 3,
                ..PatternConfig::sixteenths()
            },
            ignore_preceding_distribution: true,
        },
    )
    .unwrap()
    .run(&std::sync::atomic::AtomicBool::new(false), |_, _| {})
    .unwrap();
    assert_eq!(grid, expected.grid());
    ctrl(&mut screen, KeyCode::KeyS);
    let mut reopened =
        State::open_with_profiles(&path, 0, &dir.path().join("AutogenConfigs")).unwrap();
    let metadata = deadsync_editor::generation::metadata::ChartData::read(
        reopened.workspace().file().session.document(),
        0,
    )
    .unwrap()
    .unwrap();
    let marker = &metadata.patterns.as_ref().unwrap()[&0];
    assert_eq!(marker.random_seed, -1);
    assert!(!marker.start_position_inclusive && !marker.end_position_inclusive);
    press(&mut reopened, KeyCode::F5);
    finish_generation(&mut reopened);
    assert!(reopened.pattern_report().is_some());
}

#[test]
fn pattern_panel_has_explicit_english_and_french_translations() {
    let en = deadsync_assets::language::load_for_tests("en");
    let fr = deadsync_assets::language::load_for_tests("fr");
    for key in [
        "GeneratePattern",
        "PatternRange",
        "PatternSubdivision",
        "PatternIncludeStart",
        "PatternIncludeEnd",
        "PatternIgnoreDistribution",
        "PatternYes",
        "PatternNo",
        "PatternBalanced",
        "PatternNoCandles",
        "PatternNoInward",
        "PatternNoTransitionLimit",
        "PatternSummary",
        "PatternNoWarnings",
        "PatternRelaxed",
        "PatternUnreachable",
        "PatternApply",
        "PatternApplied",
        "PatternGenerating",
        "RegeneratePattern",
        "PatternMarker",
        "PatternProfileIssues",
        "PatternApplyPersistent",
        "EditPatternMarker",
        "MarkerStart",
        "MarkerLength",
        "MarkerSave",
        "MarkerDelete",
        "MarkerConfirmDelete",
        "MarkerHelp",
        "MarkerEdited",
        "MarkerDeleted",
    ] {
        let english = en
            .fallback
            .get("Edition")
            .unwrap()
            .get(key)
            .unwrap_or_else(|| panic!("Missing English {key}"));
        let french = fr
            .active
            .get("Edition")
            .unwrap()
            .get(key)
            .unwrap_or_else(|| panic!("Missing French {key}"));
        assert!(!english.is_empty() && !french.is_empty());
        assert_ne!(
            english, french,
            "French must not fall back to English for {key}"
        );
    }
}

#[test]
fn saved_pattern_shortcut_renders_markers_regenerates_exact_settings_and_preserves_metadata() {
    use deadsync_editor::{
        document::{Document, Format},
        generation::{
            metadata::{ChartData, Definition, PATTERN_BALANCED, Registry, SIXTEENTHS},
            pattern_job::Task,
        },
        history::Session,
    };
    use std::{
        collections::BTreeMap,
        sync::{Arc, atomic::AtomicBool},
    };
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let mut source = Session::new(Document::parse(Arc::<[u8]>::from(SINGLE), Format::Ssc).unwrap());
    let definition = Definition {
        pattern_config_guid: SIXTEENTHS.parse().unwrap(),
        performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
        length: 48,
        random_seed: -7,
        start_position_inclusive: true,
        end_position_inclusive: true,
        ignore_preceding_distribution: true,
        extra: BTreeMap::from([("UnknownMarker".into(), serde_json::json!("keep"))]),
    };
    let data = ChartData {
        patterns: Some(BTreeMap::from([
            (48, definition.clone()),
            (
                192,
                Definition {
                    length: 208,
                    ..definition
                },
            ),
        ])),
        ..ChartData::default()
    };
    data.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    let metadata = source.document().text(Some(0), "GAChartData").unwrap();
    let expected = Task::capture_saved(&source, 0, 48, &Registry::builtin())
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, &before).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::ArrowDown);
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert_eq!(
        actors
            .iter()
            .filter(|a| matches!(a,Actor::Text{content,z:8006,..} if content.as_ref()=="Pattern"))
            .count(),
        2
    );
    press(&mut screen, KeyCode::F5);
    finish_generation(&mut screen);
    assert_eq!(screen.pattern_report().unwrap().1, expected.average_nps());
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    press(&mut screen, KeyCode::Enter);
    assert_eq!(screen.workspace().grid(), expected.grid());
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "GAChartData")
            .unwrap(),
        metadata
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    ctrl(&mut screen, KeyCode::KeyY);
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(reopened.workspace().grid(), expected.grid());
    assert_eq!(
        reopened
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "GAChartData")
            .unwrap(),
        metadata
    );
}
#[test]
fn saved_pattern_command_without_a_marker_is_read_only() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SINGLE).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F5);
    press(&mut screen, KeyCode::F1);
    for _ in 0..7 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter); // same action through the optional menu
    assert!(!screen.is_generating());
    assert!(screen.pattern_report().is_none());
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        SINGLE
    );
}

#[test]
fn saved_pattern_uses_custom_profiles_from_directory_on_the_background_worker() {
    use deadsync_editor::{
        document::{Document, Format},
        generation::{
            config::PatternConfig,
            metadata::{ChartData, Definition, PATTERN_BALANCED, Registry},
            pattern_job::Task,
            profiles::Profile,
        },
        history::Session,
    };
    use std::{
        collections::BTreeMap,
        sync::{Arc, atomic::AtomicBool},
    };
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let configs = dir.path().join("AutogenConfigs");
    let profile = Profile::new_pattern(
        "Custom twelfths",
        PatternConfig {
            beat_sub_division: 3,
            ..PatternConfig::sixteenths()
        },
    )
    .unwrap();
    profile.export_new(&configs).unwrap();
    fs::write(configs.join("pc-broken.json"), b"broken").unwrap();
    let mut registry = Registry::builtin();
    profile.register(&mut registry).unwrap();
    let mut source = Session::new(Document::parse(Arc::<[u8]>::from(SINGLE), Format::Ssc).unwrap());
    ChartData {
        patterns: Some(BTreeMap::from([(
            48,
            Definition {
                pattern_config_guid: profile.guid().clone(),
                performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                length: 48,
                random_seed: 42,
                start_position_inclusive: true,
                end_position_inclusive: true,
                ..Definition::default()
            },
        )])),
        ..ChartData::default()
    }
    .save(&mut source, 0)
    .unwrap();
    let before = source.serialize().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, &before).unwrap();
    let expected = Task::capture_saved(&source, 0, 48, &registry)
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    let mut screen = State::open_with_profiles(&path, 0, &configs).unwrap();
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::F5);
    finish_generation(&mut screen);
    assert!(screen.pattern_report().is_some());
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(actors.iter().any(|a|matches!(a,Actor::Text{content,z:8021,..} if content.contains("Profile files ignored: 1."))));
    press(&mut screen, KeyCode::Enter);
    assert_eq!(screen.workspace().grid(), expected.grid());
    ctrl(&mut screen, KeyCode::KeyS);
    let mut reopened = State::open_with_profiles(&path, 0, &configs).unwrap();
    press(&mut reopened, KeyCode::ArrowDown);
    press(&mut reopened, KeyCode::F5);
    finish_generation(&mut reopened);
    assert!(reopened.pattern_report().is_some());
    assert_eq!(fs::read(configs.join("pc-broken.json")).unwrap(), b"broken");
}

fn marker_song() -> Vec<u8> {
    use deadsync_editor::{
        document::{Document, Format},
        generation::metadata::{ChartData, Definition, PATTERN_BALANCED, SIXTEENTHS},
        history::Session,
    };
    let mut session =
        Session::new(Document::parse(std::sync::Arc::<[u8]>::from(SINGLE), Format::Ssc).unwrap());
    let data = ChartData {
        patterns: Some(std::collections::BTreeMap::from([(
            0,
            Definition {
                pattern_config_guid: SIXTEENTHS.parse().unwrap(),
                performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                length: 96,
                start_position_inclusive: true,
                end_position_inclusive: true,
                extra: std::collections::BTreeMap::from([(
                    "FutureMarker".into(),
                    serde_json::json!("keep"),
                )]),
                ..Definition::default()
            },
        )])),
        ..ChartData::default()
    };
    data.save(&mut session, 0).unwrap();
    session.serialize().unwrap()
}
#[test]
fn marker_inspector_edits_all_basic_settings_undoes_saves_reopens_and_regenerates() {
    use deadsync_editor::generation::metadata::ChartData;
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let before = marker_song();
    fs::write(&path, &before).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    let grid = screen.workspace().grid().clone();
    press(&mut screen, KeyCode::F6);
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(
        actors
            .iter()
            .any(|a| matches!(a,Actor::Text{content,..} if content.as_ref()=="Start beat: 0.000"))
    );
    // Start, length, seed and all three flags. Opening/adjusting stays read-only.
    for index in 0..6 {
        press(&mut screen, KeyCode::ArrowRight);
        if index < 5 {
            press(&mut screen, KeyCode::ArrowDown);
        }
    }
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    let after = screen.workspace().file().session.serialize().unwrap();
    assert_ne!(before, after);
    assert_eq!(screen.workspace().row(), 48);
    assert_eq!(screen.workspace().grid(), &grid);
    let data = ChartData::read(screen.workspace().file().session.document(), 0)
        .unwrap()
        .unwrap();
    let marker = &data.patterns.as_ref().unwrap()[&48];
    assert_eq!(marker.length, 144);
    assert_eq!(marker.random_seed, 1);
    assert!(
        !marker.start_position_inclusive
            && !marker.end_position_inclusive
            && marker.ignore_preceding_distribution
    );
    assert_eq!(marker.extra["FutureMarker"], serde_json::json!("keep"));
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    ctrl(&mut screen, KeyCode::KeyY);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        after
    );
    ctrl(&mut screen, KeyCode::KeyS);
    let mut reopened = State::open(&path, 0).unwrap();
    assert_eq!(
        reopened.workspace().file().session.serialize().unwrap(),
        after
    );
    press(&mut reopened, KeyCode::ArrowDown);
    press(&mut reopened, KeyCode::F5);
    finish_generation(&mut reopened);
    assert!(reopened.pattern_report().is_some());
    press(&mut reopened, KeyCode::Enter);
    assert_eq!(
        ChartData::read(reopened.workspace().file().session.document(), 0)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()[&48],
        *marker
    );
}
#[test]
fn marker_inspector_discard_menu_delete_confirmation_and_undo_keep_notes() {
    use deadsync_editor::generation::metadata::ChartData;
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let before = marker_song();
    fs::write(&path, &before).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    let grid = screen.workspace().grid().clone();
    press(&mut screen, KeyCode::F6);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::Escape);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    // Open inspector through the simple menu, request deletion, cancel it.
    press(&mut screen, KeyCode::F1);
    for _ in 0..8 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    for _ in 0..7 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(
        actors.iter().any(
            |a| matches!(a,Actor::Text{content,..} if content.contains("Delete this marker?"))
        )
    );
    press(&mut screen, KeyCode::Escape);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    press(&mut screen, KeyCode::F6);
    for _ in 0..7 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Enter);
    assert!(
        ChartData::read(screen.workspace().file().session.document(), 0)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()
            .is_empty()
    );
    assert_eq!(screen.workspace().grid(), &grid);
    actors.clear();
    screen.actors(&mut actors);
    assert!(
        !actors
            .iter()
            .any(|a| matches!(a,Actor::Text{content,z:8006,..} if content.as_ref()=="Pattern"))
    );
    let after = screen.workspace().file().session.serialize().unwrap();
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    ctrl(&mut screen, KeyCode::KeyY);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        after
    );
    ctrl(&mut screen, KeyCode::KeyS);
    assert_eq!(fs::read(&path).unwrap(), after);
    // No marker is a read-only error, including after reload.
    let mut reopened = State::open(&path, 0).unwrap();
    press(&mut reopened, KeyCode::F6);
    assert_eq!(
        reopened.workspace().file().session.serialize().unwrap(),
        after
    );
}
