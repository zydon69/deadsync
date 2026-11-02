use deadsync_editor::{
    generation::metadata::{ChartData, Definition},
    key_binding_preferences::{
        Action, Preferences,
        source_profile::{self, Platform},
    },
};
#[test]
fn newly_implemented_source_groups_are_registered_unbound_natively_and_import_original_defaults() {
    let native = Preferences::default();
    let groups: Vec<_> = Action::ALL
        .into_iter()
        .filter(|action| {
            action.is_selection_preset()
                || action.is_source_pattern_action()
                || action.id().starts_with("SnapTo")
                || action.id().starts_with("SetPlayer")
                || matches!(
                    action,
                    Action::MoveToNextPattern | Action::MoveToPreviousPattern
                )
        })
        .collect();
    assert_eq!(groups.len(), 39);
    for platform in [Platform::Mac, Platform::Other] {
        let source = source_profile::preset(&native, platform).unwrap();
        let physical = if platform == Platform::Mac {
            "LeftWindows"
        } else {
            "LeftControl"
        };
        for action in &groups {
            assert!(
                deadsync_editor::key_commands::source::COMMANDS
                    .iter()
                    .any(|command| command.id == action.id())
            );
            assert_eq!(native.commands[action.id()], vec![Vec::<String>::new()]);
            assert!(source.report.applied.iter().any(|id| id == action.id()));
            assert!(!source.preferences.source_blockers.contains_key(action.id()));
        }
        assert_eq!(
            source.preferences.commands["SelectAllNotes"],
            vec![vec![physical, "A"]]
        );
        assert_eq!(
            source.preferences.commands["SelectAllMiscEvents"],
            vec![vec![physical, "LeftAlt", "A"]]
        );
        assert_eq!(
            source.preferences.commands["MoveToNextPattern"],
            vec![vec![physical, "P"]]
        );
        assert_eq!(
            source.preferences.commands["MoveToPreviousPattern"],
            vec![vec![physical, "LeftShift", "P"]]
        );
        assert_eq!(
            source.preferences.commands["RegenerateAllPatternsFixedSeeds"],
            vec![vec!["LeftAlt", "P"]]
        );
        assert_eq!(
            source.preferences.commands["RegenerateSelectedPatternsNewSeeds"],
            vec![vec![physical, "LeftAlt", "LeftShift", "P"]]
        );
        assert!(
            source.report.rejected.is_empty(),
            "{:?}",
            source.report.rejected
        );
        assert!(
            Action::from_id("ClearSelectedPatterns").is_none(),
            "No source Clear callback is registered"
        );
    }
}
#[test]
fn source_pattern_navigation_uses_fractional_boundaries_without_changing_integer_native_navigation()
{
    let data = ChartData {
        patterns: Some(
            [0, 48, 96]
                .into_iter()
                .map(|row| {
                    (
                        row,
                        Definition {
                            length: 24,
                            ..Default::default()
                        },
                    )
                })
                .collect(),
        ),
        ..Default::default()
    };
    for (row, previous, next) in [
        (-0.5, 96, 0),
        (0., 96, 48),
        (0.5, 0, 48),
        (47.5, 0, 48),
        (48., 0, 96),
        (48.5, 48, 96),
        (96., 48, 0),
        (96.5, 96, 0),
    ] {
        assert_eq!(data.adjacent_marker_at(row, false), Some(previous));
        assert_eq!(data.adjacent_marker_at(row, true), Some(next));
    }
    for row in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(data.adjacent_marker_at(row, true), None);
    }
    assert_eq!(data.adjacent_marker(48, false), Some(0));
    assert_eq!(data.adjacent_marker(48, true), Some(96));
    assert_eq!(ChartData::default().adjacent_marker_at(48.5, true), None);
}
#[test]
fn source_pattern_seek_preserves_existing_row_range_and_never_edits_document() {
    use deadsync_editor::{
        document::{Document, Format},
        history::Session,
        workspace::{Command, Workspace},
    };
    let mut session = Session::new(
        Document::parse(
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;".as_slice(),
            Format::Ssc,
        )
        .unwrap(),
    );
    ChartData {
        patterns: Some(
            [0, 48, 96]
                .into_iter()
                .map(|row| {
                    (
                        row,
                        Definition {
                            length: 24,
                            ..Default::default()
                        },
                    )
                })
                .collect(),
        ),
        ..Default::default()
    }
    .save(&mut session, 0)
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("navigation.ssc");
    std::fs::write(&path, session.serialize().unwrap()).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    workspace.execute(Command::ToggleSelection).unwrap();
    workspace.execute(Command::Seek(24)).unwrap();
    let before = workspace.file().session.serialize().unwrap();
    assert_eq!(workspace.selection(), Some((0, 24)));
    assert!(workspace.navigate_saved_pattern_at(48.5, false).unwrap());
    assert_eq!(workspace.row(), 48);
    assert_eq!(workspace.selection(), Some((0, 24)));
    assert_eq!(workspace.file().session.serialize().unwrap(), before);
    assert!(workspace.navigate_saved_pattern_at(f64::NAN, true).is_err());
    assert_eq!(workspace.row(), 48);
}
