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
        "MarkerClearNotes",
        "MarkerPrevious",
        "MarkerNext",
        "MarkerPending",
        "MarkerClearing",
        "MarkerClearSummary",
        "MarkerClearApply",
        "MarkerCleared",
        "MarkerProfiles",
        "ProfileLoading",
        "ProfilePattern",
        "ProfilePerformed",
        "ProfileExpression",
        "ProfileStage",
        "ProfileBack",
        "ProfilePickerHelp",
        "ProfileStaged",
        "ProfileMissing",
        "ProfileChooseMissing",
        "ProfileStale",
        "ProfileUnnamed",
        "ProfileSixteenths",
        "ProfileEighths",
        "ProfileDynamic",
        "ProfileAggressiveBrackets",
        "ProfileNoBrackets",
        "ChartProperties",
        "ChartChooseExpression",
        "ChartUseOffset",
        "ChartMusicOffset",
        "ChartEffectiveOffset",
        "ChartApplyProperties",
        "ChartExpressionStaged",
        "ChartPropertiesApplied",
        "ChartPropertiesHelpSsc",
        "ChartPropertiesHelpSm",
        "ChartExpressionPickerHelp",
        "ChartStageExpression",
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

fn inspector_action(screen: &mut State, index: usize) {
    press(screen, KeyCode::F6);
    for _ in 0..index {
        press(screen, KeyCode::ArrowDown);
    }
    press(screen, KeyCode::Enter);
}
#[test]
fn clearing_pattern_notes_previews_discards_cancels_applies_and_saves_with_marker_kept() {
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
    let metadata = ChartData::read(screen.workspace().file().session.document(), 0)
        .unwrap()
        .unwrap();
    inspector_action(&mut screen, 9);
    finish_generation(&mut screen);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(actors.iter().any(|a|matches!(a,Actor::Text{content,..} if content.contains("Remove 3 notes | Shorten 0 holds"))));
    assert!(actors.iter().any(|a|matches!(a,Actor::Text{content,..} if content.as_ref()=="Apply note removal and keep the marker")));
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    inspector_action(&mut screen, 9);
    press(&mut screen, KeyCode::Escape);
    finish_generation(&mut screen);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    inspector_action(&mut screen, 9);
    finish_generation(&mut screen);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(screen.workspace().grid().iter().count(), 1);
    assert!(
        screen
            .workspace()
            .grid()
            .get(Position {
                player: 0,
                row: 144,
                lane: 3
            })
            .is_some()
    );
    let after = screen.workspace().file().session.serialize().unwrap();
    assert_eq!(
        serde_json::to_value(
            ChartData::read(screen.workspace().file().session.document(), 0)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(metadata).unwrap()
    );
    actors.clear();
    screen.actors(&mut actors);
    assert!(
        actors
            .iter()
            .any(|a| matches!(a,Actor::Text{content,z:8006,..} if content.as_ref()=="Pattern"))
    );
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
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(
        reopened.workspace().file().session.serialize().unwrap(),
        after
    );
}
#[test]
fn marker_navigation_wraps_and_pending_drafts_block_navigation_and_clear_actions() {
    use deadsync_editor::{
        document::{Document, Format},
        generation::metadata::ChartData,
        history::Session,
    };
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let mut source = Session::new(
        Document::parse(std::sync::Arc::<[u8]>::from(marker_song()), Format::Ssc).unwrap(),
    );
    let mut data = ChartData::read(source.document(), 0).unwrap().unwrap();
    let definition = data.patterns.as_ref().unwrap()[&0].clone();
    data.patterns.as_mut().unwrap().insert(192, definition);
    data.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, &before).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    inspector_action(&mut screen, 11);
    assert_eq!(screen.workspace().row(), 192);
    // Navigation reopens the inspector for the selected marker at row zero.
    for _ in 0..11 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert_eq!(screen.workspace().row(), 0);
    for _ in 0..10 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert_eq!(screen.workspace().row(), 192);
    press(&mut screen, KeyCode::ArrowRight); // pending start edit, inspector index 0
    for _ in 0..9 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(!screen.is_generating());
    assert_eq!(screen.workspace().row(), 192);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(actors.iter().any(
        |a| matches!(a,Actor::Text{content,..} if content.contains("Apply or discard marker edits"))
    ));
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(screen.workspace().row(), 192);
    press(&mut screen, KeyCode::Escape);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
}

fn choose_visible_profile(screen: &mut State, name: &str) {
    for _ in 0..20 {
        let mut actors = Vec::new();
        screen.actors(&mut actors);
        if actors
            .iter()
            .any(|a| matches!(a,Actor::Text{content,z:8021,..} if content.contains(name)))
        {
            return;
        }
        press(screen, KeyCode::ArrowRight);
    }
    panic!("Profile {name} was not selectable");
}
fn stage_profile_pair(screen: &mut State, pattern: &str, performed: &str) {
    choose_visible_profile(screen, pattern);
    press(screen, KeyCode::ArrowDown);
    choose_visible_profile(screen, performed);
    press(screen, KeyCode::ArrowDown); // chart expression remains unchanged
    press(screen, KeyCode::ArrowDown); // stage all three profile rows
    press(screen, KeyCode::Enter);
}
fn apply_marker_from_profiles(screen: &mut State) {
    // Profile selector returns to its inspector entry (12); save is entry 6.
    for _ in 0..7 {
        press(screen, KeyCode::ArrowDown);
    }
    press(screen, KeyCode::Enter);
}
#[test]
fn profile_selector_loads_custom_names_stages_undoes_saves_reopens_and_drives_f5() {
    use deadsync_editor::generation::{
        config::{PatternConfig, PerformedConfig, PerformedPreset},
        metadata::ChartData,
        pattern_job::Task,
        profiles::{Kind, Loaded, Profile},
    };
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("AutogenConfigs");
    let pattern = Profile::new_pattern(
        "Triplets\npreview",
        PatternConfig {
            beat_sub_division: 3,
            ..PatternConfig::sixteenths()
        },
    )
    .unwrap();
    pattern.export_new(&folder).unwrap();
    let performed=Profile::parse(Kind::Performed,&serde_json::to_vec(&serde_json::json!({"Guid":"11111111-2222-3333-4444-555555555555","Name":"Custom feet","Config":PerformedConfig::preset(PerformedPreset::PatternNoCandles)})).unwrap()).unwrap();
    performed.export_new(&folder).unwrap();
    fs::write(folder.join("pc-broken.json"), b"invalid").unwrap();
    let path = dir.path().join("song.ssc");
    let before = marker_song();
    fs::write(&path, &before).unwrap();
    let mut screen = State::open_with_profiles(&path, 0, &folder).unwrap();
    inspector_action(&mut screen, 12);
    assert!(screen.is_generating());
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    finish_generation(&mut screen);
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(
        actors.iter().any(
            |a| matches!(a,Actor::Text{content,..} if content.contains("Profiles ignored: 1"))
        )
    );
    stage_profile_pair(&mut screen, "Triplets preview", "Custom feet");
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    apply_marker_from_profiles(&mut screen);
    let after = screen.workspace().file().session.serialize().unwrap();
    assert_ne!(before, after);
    let data = ChartData::read(screen.workspace().file().session.document(), 0)
        .unwrap()
        .unwrap();
    let marker = &data.patterns.as_ref().unwrap()[&0];
    assert_eq!(&marker.pattern_config_guid, pattern.guid());
    assert_eq!(&marker.performed_chart_config_guid, performed.guid());
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
    let mut reopened = State::open_with_profiles(&path, 0, &folder).unwrap();
    let registry = Loaded::directory(&folder).unwrap().registry;
    let expected = Task::capture_saved(&reopened.workspace().file().session, 0, 0, &registry)
        .unwrap()
        .run(&std::sync::atomic::AtomicBool::new(false), |_, _| {})
        .unwrap();
    press(&mut reopened, KeyCode::F5);
    finish_generation(&mut reopened);
    assert!(reopened.pattern_report().is_some());
    press(&mut reopened, KeyCode::Enter);
    assert_eq!(reopened.workspace().grid(), expected.grid());
    assert_eq!(
        fs::read(folder.join(pattern.filename())).unwrap(),
        pattern.serialize().unwrap()
    );
    assert_eq!(
        fs::read(folder.join(performed.filename())).unwrap(),
        performed.serialize().unwrap()
    );
    assert_eq!(fs::read(folder.join("pc-broken.json")).unwrap(), b"invalid");
}
#[test]
fn profile_subpanel_back_discard_and_loading_cancel_preserve_prior_draft_and_source() {
    use deadsync_editor::generation::metadata::ChartData;
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let folder = dir.path().join("absent");
    let before = marker_song();
    fs::write(&path, &before).unwrap();
    let mut screen = State::open_with_profiles(&path, 0, &folder).unwrap();
    inspector_action(&mut screen, 12);
    finish_generation(&mut screen);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::Escape);
    apply_marker_from_profiles(&mut screen);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    inspector_action(&mut screen, 12);
    finish_generation(&mut screen);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Escape);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    press(&mut screen, KeyCode::F6);
    press(&mut screen, KeyCode::ArrowRight);
    for _ in 0..12 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Escape);
    finish_generation(&mut screen);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    apply_marker_from_profiles(&mut screen);
    let data = ChartData::read(screen.workspace().file().session.document(), 0)
        .unwrap()
        .unwrap();
    assert!(data.patterns.unwrap().contains_key(&48));
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    assert!(!folder.exists());
}
#[test]
fn missing_profile_references_are_not_substituted_until_the_user_selects_replacements() {
    use deadsync_editor::{
        document::{Document, Format},
        generation::metadata::{ChartData, Registry},
        history::Session,
    };
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let mut source = Session::new(
        Document::parse(std::sync::Arc::<[u8]>::from(marker_song()), Format::Ssc).unwrap(),
    );
    let mut data = ChartData::read(source.document(), 0).unwrap().unwrap();
    let marker = data.patterns.as_mut().unwrap().get_mut(&0).unwrap();
    marker.pattern_config_guid = Default::default();
    marker.performed_chart_config_guid = Default::default();
    data.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, &before).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    inspector_action(&mut screen, 12);
    finish_generation(&mut screen);
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert_eq!(actors.iter().filter(|a|matches!(a,Actor::Text{content,z:8021,..} if content.contains("Missing profile"))).count(),2);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    press(&mut screen, KeyCode::ArrowUp);
    press(&mut screen, KeyCode::ArrowUp);
    press(&mut screen, KeyCode::ArrowUp);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    apply_marker_from_profiles(&mut screen);
    assert!(
        ChartData::read(screen.workspace().file().session.document(), 0)
            .unwrap()
            .unwrap()
            .options(0, &Registry::builtin())
            .is_ok()
    );
}

#[test]
fn chart_expression_selector_is_chart_wide_and_f3_f5_use_the_saved_custom_profile() {
    use deadsync_editor::{
        document::{Document, Format},
        generation::{
            config::{BracketDetermination, BracketMethod, ExpressedConfig},
            job::{ChartOptions, ChartTask},
            metadata::ChartData,
            pattern_job::Task,
            profiles::{Kind, Loaded, Profile},
        },
        history::Session,
    };
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("AutogenConfigs");
    let config = ExpressedConfig {
        bracket_parsing_determination: BracketDetermination::UseDefaultMethod,
        default_bracket_parsing_method: BracketMethod::NoBrackets,
        ..ExpressedConfig::default()
    };
    let custom=Profile::parse(Kind::Expressed,&serde_json::to_vec(&serde_json::json!({"Guid":"11111111-2222-3333-4444-555555555555","Name":"Custom interpretation","Config":config})).unwrap()).unwrap();
    custom.export_new(&folder).unwrap();
    fs::write(folder.join("ecc-broken.json"), b"invalid").unwrap();
    let mut source = Session::new(
        Document::parse(std::sync::Arc::<[u8]>::from(marker_song()), Format::Ssc).unwrap(),
    );
    let mut data = ChartData::read(source.document(), 0).unwrap().unwrap();
    let definition = data.patterns.as_ref().unwrap()[&0].clone();
    data.patterns.as_mut().unwrap().insert(192, definition);
    data.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, &before).unwrap();
    let mut screen = State::open_with_profiles(&path, 0, &folder).unwrap();
    let grid = screen.workspace().grid().clone();
    inspector_action(&mut screen, 12);
    finish_generation(&mut screen);
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(actors.iter().any(|a|matches!(a,Actor::Text{content,z:8021,..} if content.contains("Interpretation (whole chart):"))));
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    choose_visible_profile(&mut screen, "Custom interpretation");
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    apply_marker_from_profiles(&mut screen);
    let after = screen.workspace().file().session.serialize().unwrap();
    let actual = ChartData::read(screen.workspace().file().session.document(), 0)
        .unwrap()
        .unwrap();
    assert_eq!(actual.expressed_chart_config, *custom.guid());
    assert_eq!(actual.patterns, data.patterns);
    assert_eq!(screen.workspace().grid(), &grid);
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
    let mut reopened = State::open_with_profiles(&path, 0, &folder).unwrap();
    let registry = Loaded::directory(&folder).unwrap().registry;
    let expected = Task::capture_saved(&reopened.workspace().file().session, 0, 0, &registry)
        .unwrap()
        .run(&std::sync::atomic::AtomicBool::new(false), |_, _| {})
        .unwrap();
    press(&mut reopened, KeyCode::F5);
    finish_generation(&mut reopened);
    press(&mut reopened, KeyCode::Enter);
    assert_eq!(reopened.workspace().grid(), expected.grid());
    let before_f3 = reopened.workspace().file().session.serialize().unwrap();
    let mut options = ChartOptions::balanced("dance-double");
    options.expressed = registry.expressed[custom.guid()].clone();
    let expected = ChartTask::capture(&reopened.workspace().file().session, 0, options)
        .unwrap()
        .run(&std::sync::atomic::AtomicBool::new(false), |_| {})
        .unwrap();
    start_generation(&mut reopened);
    finish_generation(&mut reopened);
    assert_eq!(
        reopened.generation_expression_profile(),
        Some(custom.guid())
    );
    actors.clear();
    reopened.actors(&mut actors);
    assert!(actors.iter().any(|a|matches!(a,Actor::Text{content,z:8021,..} if content.contains("Profile files ignored: 1"))));
    press(&mut reopened, KeyCode::Enter);
    assert_eq!(reopened.workspace().grid(), expected.grid());
    ctrl(&mut reopened, KeyCode::KeyZ);
    assert_eq!(
        reopened.workspace().file().session.serialize().unwrap(),
        before_f3
    );
    assert_eq!(
        fs::read(folder.join(custom.filename())).unwrap(),
        custom.serialize().unwrap()
    );
    assert_eq!(
        fs::read(folder.join("ecc-broken.json")).unwrap(),
        b"invalid"
    );
}
#[test]
fn missing_chart_expression_requires_replacement_and_discard_keeps_the_unknown_identity() {
    use deadsync_editor::{
        document::{Document, Format},
        generation::metadata::{ChartData, Registry},
        history::Session,
    };
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let mut source = Session::new(
        Document::parse(std::sync::Arc::<[u8]>::from(marker_song()), Format::Ssc).unwrap(),
    );
    let mut data = ChartData::read(source.document(), 0).unwrap().unwrap();
    data.expressed_chart_config = Default::default();
    data.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, &before).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    start_generation(&mut screen);
    finish_generation(&mut screen);
    assert!(screen.generation_report().is_none());
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    inspector_action(&mut screen, 12);
    finish_generation(&mut screen);
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert_eq!(actors.iter().filter(|a|matches!(a,Actor::Text{content,z:8021,..} if content.contains("Missing profile"))).count(),1);
    for _ in 0..3 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    press(&mut screen, KeyCode::ArrowUp);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Escape);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    inspector_action(&mut screen, 12);
    finish_generation(&mut screen);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    apply_marker_from_profiles(&mut screen);
    let actual = ChartData::read(screen.workspace().file().session.document(), 0)
        .unwrap()
        .unwrap();
    assert!(
        Registry::builtin()
            .expressed
            .contains_key(&actual.expressed_chart_config)
    );
}

fn apply_chart_properties(screen: &mut State) {
    for _ in 0..4 {
        press(screen, KeyCode::ArrowDown);
    }
    press(screen, KeyCode::Enter);
}
#[test]
fn f7_properties_without_markers_stage_custom_expression_and_audio_offset_then_save_and_generate() {
    use deadsync_editor::generation::{
        job::{ChartOptions, ChartTask},
        metadata::ChartData,
        profiles::{Kind, Profile},
    };
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("AutogenConfigs");
    let config = ChartOptions::balanced("dance-single").expressed;
    let custom=Profile::parse(Kind::Expressed,&serde_json::to_vec(&serde_json::json!({"Guid":"11111111-2222-3333-4444-555555555555","Name":"No-marker expression","Config":config})).unwrap()).unwrap();
    custom.export_new(&folder).unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SINGLE).unwrap();
    let mut screen = State::open_with_profiles(&path, 0, &folder).unwrap();
    let grid = screen.workspace().grid().clone();
    press(&mut screen, KeyCode::F7);
    press(&mut screen, KeyCode::Enter);
    finish_generation(&mut screen);
    choose_visible_profile(&mut screen, "No-marker expression");
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        SINGLE
    );
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::ArrowDown);
    key(&mut screen, KeyCode::ShiftLeft, true);
    press(&mut screen, KeyCode::ArrowRight);
    key(&mut screen, KeyCode::ShiftLeft, false);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::ArrowDown);
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(actors.iter().any(|a|matches!(a,Actor::Text{content,z:8021,..} if content.as_ref()=="Effective music offset: 0.001100 s")));
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        SINGLE
    );
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    let after = screen.workspace().file().session.serialize().unwrap();
    let data = ChartData::read(screen.workspace().file().session.document(), 0)
        .unwrap()
        .unwrap();
    assert!(data.patterns.is_none());
    assert_eq!(data.expressed_chart_config, *custom.guid());
    assert!(data.should_use_chart_music_offset);
    assert_eq!(data.music_offset, 0.0011);
    assert_eq!(screen.workspace().grid(), &grid);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "OFFSET")
            .as_deref(),
        Some("0.0011")
    );
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        SINGLE
    );
    ctrl(&mut screen, KeyCode::KeyY);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        after
    );
    ctrl(&mut screen, KeyCode::KeyS);
    let mut reopened = State::open_with_profiles(&path, 0, &folder).unwrap();
    assert_eq!(
        reopened
            .workspace()
            .chart_properties()
            .unwrap()
            .effective_music_offset(),
        0.0011
    );
    let mut options = ChartOptions::balanced("dance-double");
    options.expressed = match custom.config() {
        deadsync_editor::generation::profiles::Config::Expressed(config) => config.clone(),
        _ => panic!(),
    };
    let expected = ChartTask::capture(&reopened.workspace().file().session, 0, options)
        .unwrap()
        .run(&std::sync::atomic::AtomicBool::new(false), |_| {})
        .unwrap();
    start_generation(&mut reopened);
    finish_generation(&mut reopened);
    assert_eq!(
        reopened.generation_expression_profile(),
        Some(custom.guid())
    );
    press(&mut reopened, KeyCode::Enter);
    assert_eq!(reopened.workspace().grid(), expected.grid());
    assert_eq!(
        fs::read(folder.join(custom.filename())).unwrap(),
        custom.serialize().unwrap()
    );
}
#[test]
fn chart_property_menu_noop_discard_profile_back_and_loading_cancel_preserve_drafts() {
    use deadsync_editor::generation::metadata::{ChartData, DYNAMIC};
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let folder = dir.path().join("absent");
    fs::write(&path, SINGLE).unwrap();
    let mut screen = State::open_with_profiles(&path, 0, &folder).unwrap();
    press(&mut screen, KeyCode::F1);
    for _ in 0..9 {
        press(&mut screen, KeyCode::ArrowDown);
    }
    press(&mut screen, KeyCode::Enter);
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(actors.iter().any(
        |a| matches!(a,Actor::Text{content,z:8021,..} if content.as_ref()=="Apply chart properties")
    ));
    apply_chart_properties(&mut screen);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        SINGLE
    );
    assert!(!screen.workspace().file().session.is_dirty());
    press(&mut screen, KeyCode::F7);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::Escape);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        SINGLE
    );
    // Holding a numeric arrow repeats offset adjustment; flags never repeat.
    press(&mut screen, KeyCode::F7);
    press(&mut screen, KeyCode::ArrowDown);
    screen.raw_key(&RawKeyboardEvent {
        code: KeyCode::ArrowRight,
        pressed: true,
        repeat: true,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(actors.iter().any(|a| matches!(a, Actor::Text { content, z:8021, .. } if content.as_ref() == "Use chart music offset: No")));
    press(&mut screen, KeyCode::ArrowDown);
    key(&mut screen, KeyCode::ArrowRight, true);
    screen.raw_key(&RawKeyboardEvent {
        code: KeyCode::ArrowRight,
        pressed: true,
        repeat: true,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    key(&mut screen, KeyCode::ArrowRight, false);
    actors.clear();
    screen.actors(&mut actors);
    assert!(actors.iter().any(|a| matches!(a, Actor::Text { content, z:8021, .. } if content.as_ref() == "Chart music offset: 0.000200 s")));
    press(&mut screen, KeyCode::Escape);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        SINGLE
    );
    // Loading cancellation returns to the prior property draft, retaining its offset.
    press(&mut screen, KeyCode::F7);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::ArrowUp);
    press(&mut screen, KeyCode::ArrowUp);
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Escape);
    finish_generation(&mut screen);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        SINGLE
    );
    // Back out of a loaded selector without committing a different expression.
    press(&mut screen, KeyCode::Enter);
    finish_generation(&mut screen);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::Escape);
    apply_chart_properties(&mut screen);
    let data = ChartData::read(screen.workspace().file().session.document(), 0)
        .unwrap()
        .unwrap();
    assert_eq!(data.music_offset, 0.0001);
    assert!(data.should_use_chart_music_offset);
    assert_eq!(data.expressed_chart_config, DYNAMIC.parse().unwrap());
    ctrl(&mut screen, KeyCode::KeyZ);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        SINGLE
    );
    assert!(!folder.exists());
}
#[test]
fn sm_properties_explain_editor_only_offset_and_keep_song_offset_unchanged() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.sm");
    let before=b"#TITLE:Test;#OFFSET:0.25;#BPMS:0=120;#NOTES:dance-single:source:Hard:9:0,0,0,0,0:1000\n0100\n0010\n0001;";
    fs::write(&path, before).unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F7);
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(actors.iter().any(|a|matches!(a,Actor::Text{content,..} if content.contains("SM keeps chart offsets for the editor"))));
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowRight);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::ArrowDown);
    press(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(None, "OFFSET")
            .as_deref(),
        Some("0.25")
    );
    assert!(
        screen
            .workspace()
            .file()
            .session
            .document()
            .text(Some(0), "OFFSET")
            .is_none()
    );
    assert_eq!(
        screen
            .workspace()
            .chart_properties()
            .unwrap()
            .effective_music_offset(),
        0.0001
    );
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(
        reopened
            .workspace()
            .chart_properties()
            .unwrap()
            .effective_music_offset(),
        0.0001
    );
}
