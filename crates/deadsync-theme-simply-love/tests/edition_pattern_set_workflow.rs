//! Production keyboard dispatch, background worker and actor tree for saved sets.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::{Actor, SizeSpec};
use deadsync_editor::{
    document::{Document, Format},
    generation::{
        config::PatternConfig,
        metadata::{ChartData, Definition, EIGHTHS, PATTERN_BALANCED, SIXTEENTHS},
        pattern_batch::Task,
        profiles::Profile,
    },
    history::Session,
    notes::Grid,
};
use deadsync_theme_simply_love::screens::edition::State;
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
const SONG:&[u8]=b"#VERSION:0.83;#TITLE:Pattern set;#BPMS:0=150;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000\n0100\n0010\n0001;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Easy;#METER:3;#NOTES:0001;";
fn init() {
    deadsync_theme_simply_love::i18n::init_for_tests();
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn key(screen: &mut State, code: KeyCode, pressed: bool) {
    screen.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(screen: &mut State, code: KeyCode) {
    key(screen, code, true);
    key(screen, code, false);
}
fn down(screen: &mut State, count: usize) {
    for _ in 0..count {
        press(screen, KeyCode::ArrowDown);
    }
}
fn ctrl(screen: &mut State, code: KeyCode) {
    key(screen, KeyCode::SuperLeft, true);
    press(screen, code);
    key(screen, KeyCode::SuperLeft, false);
}
fn finish(screen: &mut State) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while screen.is_generating() {
        screen.update();
        assert!(
            Instant::now() < deadline,
            "Pattern set worker did not finish"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn texts(screen: &State) -> Vec<String> {
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    actors
        .into_iter()
        .filter_map(|actor| match actor {
            Actor::Text { content, .. } => Some(content.to_string()),
            _ => None,
        })
        .collect()
}
fn contains(screen: &State, text: &str) -> bool {
    texts(screen).iter().any(|content| content.contains(text))
}
fn source() -> Session {
    let mut source = Session::new(Document::parse(Arc::<[u8]>::from(SONG), Format::Ssc).unwrap());
    let marker = |length, seed, profile: &str| Definition {
        length,
        random_seed: seed,
        pattern_config_guid: profile.parse().unwrap(),
        performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
        start_position_inclusive: true,
        end_position_inclusive: true,
        ..Definition::default()
    };
    ChartData {
        patterns: Some(BTreeMap::from([
            (48, marker(96, 42, SIXTEENTHS)),
            (120, marker(96, -7, EIGHTHS)),
        ])),
        ..ChartData::default()
    }
    .save(&mut source, 0)
    .unwrap();
    source
}
fn save(source: &Session, path: &Path) -> Vec<u8> {
    let bytes = source.serialize().unwrap();
    fs::write(path, &bytes).unwrap();
    bytes
}
fn start(screen: &mut State) {
    press(screen, KeyCode::F9);
    down(screen, 5);
    press(screen, KeyCode::Enter);
}
#[test]
fn f9_all_markers_preview_matches_core_navigates_applies_one_history_action_and_saves() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source = source();
    let before = save(&source, &path);
    let expected = Task::capture_all(&source, 0, None)
        .unwrap()
        .run(&AtomicBool::new(false), |_| {})
        .unwrap();
    let mut screen = State::open(&path, 0).unwrap();
    let preferences = dir.path().join("preview-layout.json");
    fs::write(
        &preferences,
        br#"{"Version":1,"Receptors":{"CenterHorizontally":true}}"#,
    )
    .unwrap();
    screen.configure_audio_preferences(&preferences).unwrap();
    press(&mut screen, KeyCode::F1);
    down(&mut screen, 12);
    press(&mut screen, KeyCode::Enter);
    let mut help_actors = Vec::new();
    screen.actors(&mut help_actors);
    assert!(
        help_actors
            .iter()
            .any(|actor| matches!(actor, Actor::Text { z: 8050, .. }))
    );
    press(&mut screen, KeyCode::Escape); // Close the modal help before authoring.
    start(&mut screen);
    press(&mut screen, KeyCode::Digit1);
    finish(&mut screen);
    let (preview, index) = screen.pattern_set_preview().unwrap();
    assert_eq!(index, 0);
    assert_eq!(preview.grid(), expected.grid());
    assert_eq!(preview.report(), expected.report());
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(screen.workspace().row(), 0); // Opening a detached preview does not seek.
    press(&mut screen, KeyCode::ArrowRight);
    assert_eq!(screen.workspace().row(), 120);
    press(&mut screen, KeyCode::ArrowLeft);
    assert_eq!(screen.workspace().row(), 48);
    assert!(contains(
        &screen,
        "Patterns: 2 ready, 0 empty, 0 failed / 2"
    ));
    assert!(contains(&screen, "Beat 1.000 | Seed 42"));
    assert!(contains(&screen, "Apply all note changes"));
    let mut actors = Vec::new();
    screen.actors(&mut actors);
    assert!(
        actors
            .iter()
            .any(|actor| matches!(actor, Actor::Sprite { z: 8004, .. }))
    );
    assert!(actors.iter().any(|actor|matches!(actor,Actor::Text{content,z:8021,wrap_width_pixels:Some(_),max_height:Some(_),..} if content.contains("Patterns:"))));
    // The larger review summary must leave a usable chart viewport.
    for width in [1280.0, 640.0] {
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            width, 480.0,
        ));
        screen.set_ui_dpi_viewport(width as u32, 480);
        let mut actors = Vec::new();
        screen.actors(&mut actors);
        assert!(
            !actors
                .iter()
                .any(|actor| matches!(actor, Actor::Text { z: 8050, .. }))
        );
        let (panel_x, panel_y, panel_width, panel_height) = actors
            .iter()
            .find_map(|actor| match actor {
                Actor::Sprite {
                    offset,
                    size: [SizeSpec::Px(w), SizeSpec::Px(h)],
                    z: 8020,
                    ..
                } => Some((offset[0], offset[1], *w, *h)),
                _ => None,
            })
            .unwrap();
        assert!(panel_x - panel_width * 0.5 >= 0.0 && panel_x + panel_width * 0.5 <= width);
        assert!(panel_y - panel_height * 0.5 >= 0.0 && panel_y + panel_height * 0.5 <= 480.0);
        let arrows: Vec<_> = actors
            .iter()
            .filter_map(|actor| match actor {
                Actor::Sprite {
                    offset,
                    size: [SizeSpec::Px(w), SizeSpec::Px(h)],
                    z: 8004,
                    ..
                } => Some((offset[0], offset[1], *w, *h)),
                _ => None,
            })
            .collect();
        assert!(!arrows.is_empty());
        if width >= 800.0 {
            assert_eq!(panel_width, 360.0);
            assert!(
                arrows
                    .iter()
                    .all(|(x, _, w, _)| x + w * 0.5 < panel_x - panel_width * 0.5)
            );
            assert!(actors.iter().any(|actor|matches!(actor,Actor::Text{content,offset,max_width:Some(max),..} if content.as_ref()=="Apply all note changes" && offset[0]==panel_x && *max==330.0)));
        } else {
            assert!(
                arrows
                    .iter()
                    .any(|(_, y, _, h)| y + h * 0.5 < panel_y - panel_height * 0.5)
            );
        }
    }
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    screen.set_ui_dpi_viewport(854, 480);
    press(&mut screen, KeyCode::ArrowLeft);
    assert_eq!(screen.pattern_set_preview().unwrap().1, 1);
    assert_eq!(screen.workspace().row(), 120);
    assert!(contains(&screen, "Beat 2.500 | Seed -7"));
    press(&mut screen, KeyCode::ArrowRight);
    assert_eq!(screen.pattern_set_preview().unwrap().1, 0);
    press(&mut screen, KeyCode::End);
    assert!(screen.workspace().row() >= 216);
    press(&mut screen, KeyCode::Home);
    press(&mut screen, KeyCode::Enter);
    assert!(screen.pattern_set_preview().is_none());
    assert_eq!(screen.workspace().grid(), expected.grid());
    assert_eq!(
        screen.workspace().file().session.undo_label(),
        Some("Regenerate pattern set")
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
    let output = screen
        .workspace()
        .file()
        .session
        .serialize_for_output()
        .unwrap();
    ctrl(&mut screen, KeyCode::KeyS);
    let reopened = State::open(&path, 0).unwrap();
    assert_eq!(reopened.workspace().grid(), expected.grid());
    assert_eq!(fs::read(&path).unwrap(), output);
    let mut other = deadsync_editor::storage::FileSession::open(&path).unwrap();
    assert_eq!(
        other.session.grid(1).unwrap(),
        &Grid::parse(b"0001", 4).unwrap()
    );
}
#[test]
fn selector_all_none_individual_and_selected_area_are_read_only_until_acceptance() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scope.ssc");
    let source = source();
    let before = save(&source, &path);
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F9);
    assert!(contains(&screen, "Include this marker: Yes"));
    down(&mut screen, 3);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Preview regeneration of 0 markers"));
    down(&mut screen, 2);
    press(&mut screen, KeyCode::Enter);
    assert!(!screen.is_generating());
    assert!(contains(&screen, "Select at least one"));
    for _ in 0..4 {
        press(&mut screen, KeyCode::ArrowUp);
    }
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Include this marker: Yes"));
    down(&mut screen, 4);
    press(&mut screen, KeyCode::Enter);
    finish(&mut screen);
    assert_eq!(
        screen
            .pattern_set_preview()
            .unwrap()
            .0
            .entries()
            .iter()
            .map(|entry| entry.row)
            .collect::<Vec<_>>(),
        vec![48]
    );
    press(&mut screen, KeyCode::Escape);
    // Pick only the other marker; a held toggle must not deselect it again.
    press(&mut screen, KeyCode::F9);
    down(&mut screen, 3);
    press(&mut screen, KeyCode::Enter);
    for _ in 0..3 {
        press(&mut screen, KeyCode::ArrowUp);
    }
    press(&mut screen, KeyCode::ArrowRight);
    assert!(contains(&screen, "Marker 2/2"));
    down(&mut screen, 1);
    press(&mut screen, KeyCode::Enter);
    screen.raw_key(&RawKeyboardEvent {
        code: KeyCode::ArrowRight,
        pressed: true,
        repeat: true,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    assert!(contains(&screen, "Include this marker: Yes"));
    key(&mut screen, KeyCode::ArrowRight, false);
    down(&mut screen, 4);
    press(&mut screen, KeyCode::Enter);
    finish(&mut screen);
    assert_eq!(
        screen
            .pattern_set_preview()
            .unwrap()
            .0
            .entries()
            .iter()
            .map(|entry| entry.row)
            .collect::<Vec<_>>(),
        vec![120]
    );
    press(&mut screen, KeyCode::Escape);
    press(&mut screen, KeyCode::Home);
    press(&mut screen, KeyCode::ArrowDown); // Select beat 1 through beat 2.
    press(&mut screen, KeyCode::Space);
    press(&mut screen, KeyCode::ArrowDown);
    assert_eq!(
        screen
            .workspace()
            .source_row_selection_at(screen.workspace().row() as f64)
            .unwrap(),
        Some((48, 96)),
        "{:?}",
        texts(&screen)
    );
    press(&mut screen, KeyCode::F9);
    down(&mut screen, 4);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Preview regeneration of 1 markers"));
    down(&mut screen, 1);
    press(&mut screen, KeyCode::Enter);
    finish(&mut screen);
    assert_eq!(
        screen
            .pattern_set_preview()
            .unwrap()
            .0
            .entries()
            .iter()
            .map(|entry| entry.row)
            .collect::<Vec<_>>(),
        vec![48]
    );
    press(&mut screen, KeyCode::Escape);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    press(&mut screen, KeyCode::F9);
    down(&mut screen, 3);
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::ArrowUp);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Preview regeneration of 2 markers"));
    press(&mut screen, KeyCode::Escape);
    assert_eq!(fs::read(&path).unwrap(), before);
}
#[test]
fn partial_and_complete_failures_keep_clear_warning_visible_and_apply_one_reversible_change() {
    init();
    for failed in [1, 2] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("failed.ssc");
        let mut source = source();
        let profiles = dir.path().join("profiles");
        let unavailable = Profile::parse(
            deadsync_editor::generation::profiles::Kind::Performed,
            &serde_json::to_vec(&serde_json::json!({
                "Guid": "12345678-9abc-4def-8123-456789abcdef",
                "Name": "Unavailable after opening",
                "Config": deadsync_editor::generation::config::PerformedConfig::preset(
                    deadsync_editor::generation::config::PerformedPreset::Balanced),
            }))
            .unwrap(),
        )
        .unwrap();
        let profile_file = unavailable.export_new(&profiles).unwrap();
        let mut data = ChartData::read(source.document(), 0).unwrap().unwrap();
        for (_, marker) in data.patterns.as_mut().unwrap().iter_mut().take(failed) {
            marker.performed_chart_config_guid = unavailable.guid().clone();
        }
        data.save(&mut source, 0).unwrap();
        let before = save(&source, &path);
        let mut screen = State::open_with_profiles(&path, 0, &profiles).unwrap();
        // File opening repairs missing IDs. Make the profile unavailable only
        // after that validated open, as a real external removal can do.
        fs::remove_file(profile_file.path()).unwrap();
        start(&mut screen);
        finish(&mut screen);
        assert_eq!(
            screen.pattern_set_preview().unwrap().0.report().failed,
            failed
        );
        let error = screen.pattern_set_preview().unwrap().0.entries()[0]
            .error()
            .unwrap();
        assert!(contains(&screen, error));
        assert!(contains(
            &screen,
            "Applying also clears the selected regions whose generation failed."
        ));
        assert!(contains(&screen, "Apply all note changes"));
        assert_eq!(
            screen.workspace().file().session.serialize().unwrap(),
            before
        );
        press(&mut screen, KeyCode::Enter);
        assert!(screen.pattern_set_preview().is_none());
        assert_ne!(
            screen.workspace().file().session.serialize().unwrap(),
            before
        );
        ctrl(&mut screen, KeyCode::KeyZ);
        assert_eq!(
            screen.workspace().file().session.serialize().unwrap(),
            before
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
#[test]
fn submenu_preserves_single_pattern_action_and_batch_cancel_discard_and_no_area_keep_source() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("menu.ssc");
    let source = source();
    let before = save(&source, &path);
    let mut screen = State::open(&path, 0).unwrap();
    press(&mut screen, KeyCode::F1);
    down(&mut screen, 7);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Regenerate several patterns (F9)"));
    down(&mut screen, 1);
    press(&mut screen, KeyCode::Enter);
    down(&mut screen, 4);
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "Mark an area"));
    assert!(contains(&screen, "Preview regeneration of 2 markers"));
    down(&mut screen, 1);
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Escape);
    finish(&mut screen);
    assert!(screen.pattern_set_preview().is_none());
    start(&mut screen);
    finish(&mut screen);
    down(&mut screen, 1);
    press(&mut screen, KeyCode::Enter);
    assert!(screen.pattern_set_preview().is_none());
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    // Set preview preserves its entry cursor. Navigate explicitly to the first
    // saved marker before exercising the single-pattern command.
    press(&mut screen, KeyCode::Home);
    ctrl(&mut screen, KeyCode::KeyP);
    assert_eq!(screen.workspace().row(), 48);
    press(&mut screen, KeyCode::Escape); // Close the navigated marker inspector.
    press(&mut screen, KeyCode::F1);
    down(&mut screen, 7);
    press(&mut screen, KeyCode::Enter);
    press(&mut screen, KeyCode::Enter);
    finish(&mut screen);
    assert!(screen.pattern_report().is_some());
    press(&mut screen, KeyCode::Escape);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        before
    );
    let missing = dir.path().join("missing.ssc");
    fs::write(&missing, SONG).unwrap();
    let mut screen = State::open(&missing, 0).unwrap();
    press(&mut screen, KeyCode::F9);
    assert!(contains(&screen, "No saved pattern markers"));
    assert!(!screen.is_generating());
}
#[test]
fn empty_markers_show_noop_and_keep_redo_and_custom_profiles_drive_the_worker() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profiles.ssc");
    let configs = dir.path().join("configs");
    let profile = Profile::new_pattern(
        "Custom twelfths",
        PatternConfig {
            beat_sub_division: 3,
            ..PatternConfig::sixteenths()
        },
    )
    .unwrap();
    let file = profile.export_new(&configs).unwrap();
    let profile_before = fs::read(file.path()).unwrap();
    let mut source = source();
    let mut data = ChartData::read(source.document(), 0).unwrap().unwrap();
    for marker in data.patterns.as_mut().unwrap().values_mut() {
        marker.pattern_config_guid = profile.guid().clone();
    }
    data.save(&mut source, 0).unwrap();
    let before = save(&source, &path);
    let expected = Task::capture_all(&source, 0, Some(&configs))
        .unwrap()
        .run(&AtomicBool::new(false), |_| {})
        .unwrap();
    let mut screen = State::open_with_profiles(&path, 0, &configs).unwrap();
    start(&mut screen);
    finish(&mut screen);
    assert_eq!(
        screen.pattern_set_preview().unwrap().0.grid(),
        expected.grid()
    );
    press(&mut screen, KeyCode::Enter);
    ctrl(&mut screen, KeyCode::KeyS);
    assert_eq!(fs::read(file.path()).unwrap(), profile_before);
    for marker in data.patterns.as_mut().unwrap().values_mut() {
        marker.length = 0;
        marker.start_position_inclusive = false;
        marker.end_position_inclusive = false;
    }
    data.save(&mut source, 0).unwrap();
    save(&source, &path);
    let mut screen = State::open_with_profiles(&path, 0, &configs).unwrap();
    press(&mut screen, KeyCode::Digit2);
    let edited = screen.workspace().file().session.serialize().unwrap();
    ctrl(&mut screen, KeyCode::KeyZ);
    let unchanged = screen.workspace().file().session.serialize().unwrap();
    start(&mut screen);
    finish(&mut screen);
    assert_eq!(screen.pattern_set_preview().unwrap().0.report().empty, 2);
    assert!(contains(&screen, "Empty marker: no steps to generate."));
    press(&mut screen, KeyCode::Enter);
    assert!(contains(&screen, "No note changes to apply."));
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        unchanged
    );
    ctrl(&mut screen, KeyCode::KeyY);
    assert_eq!(
        screen.workspace().file().session.serialize().unwrap(),
        edited
    );
    assert_ne!(before, unchanged);
}
