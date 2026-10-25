//! GrooveAuthor EventTree.Find{Previous,Next}EventWithLooping uses strict
//! before/after queries, then wraps to Last/First. Navigation never edits data.
use deadsync_editor::{
    timing_edit::{
        Scope,
        label::{Draft, Labels},
    },
    workspace::{Command, Workspace},
};
use std::fs;
const SONG: &str = "#VERSION:0.83;#BPMS:0=120;#LABELS:1=A,3=B,5=C;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=90;#LABELS:2=Other;#NOTES:0100;";

#[test]
fn strict_neighbors_wrap_at_equal_positions_and_extreme_rows() {
    let labels = Labels::parse("1=A,3=B,5=C").unwrap();
    for (row, previous, next) in [
        (0, 240, 48),
        (47, 240, 48),
        (48, 240, 144),
        (49, 48, 144),
        (143, 48, 144),
        (144, 48, 240),
        (145, 144, 240),
        (240, 144, 48),
        (241, 240, 48),
        (u64::MAX, 240, 48),
    ] {
        assert_eq!(
            labels.previous_row(row),
            Some(previous),
            "previous at {row}"
        );
        assert_eq!(labels.next_row(row), Some(next), "next at {row}");
    }
    let singleton = Labels::parse("2=Only").unwrap();
    let empty = Labels::parse("").unwrap();
    for row in [0, 96, u64::MAX] {
        assert_eq!(singleton.previous_row(row), Some(96));
        assert_eq!(singleton.next_row(row), Some(96));
        assert_eq!(empty.previous_row(row), None);
        assert_eq!(empty.next_row(row), None);
    }
}

#[test]
fn workspace_navigation_tracks_scope_unsaved_edits_undo_redo_and_selection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    let source = workspace.file().session.serialize().unwrap();
    workspace.execute(Command::NextLabel).unwrap();
    assert_eq!(workspace.row(), 48);
    workspace.execute(Command::ToggleSelection).unwrap();
    workspace.execute(Command::NextLabel).unwrap();
    assert_eq!(workspace.selection(), Some((48, 144)));
    workspace.execute(Command::PreviousLabel).unwrap();
    assert_eq!(workspace.row(), 48);
    assert_eq!(workspace.file().session.serialize().unwrap(), source);
    assert!(!workspace.file().session.is_dirty());
    let mut draft = Draft::capture(&workspace.file().session, 0, Scope::Chart).unwrap();
    draft.set(96, "Unsaved").unwrap();
    workspace.apply_labels(&draft).unwrap();
    workspace.execute(Command::NextLabel).unwrap();
    assert_eq!(workspace.row(), 96);
    workspace.execute(Command::Undo).unwrap();
    workspace.execute(Command::NextLabel).unwrap();
    assert_eq!(workspace.row(), 144);
    workspace.execute(Command::Redo).unwrap();
    workspace.execute(Command::PreviousLabel).unwrap();
    assert_eq!(workspace.row(), 96);
    workspace.execute(Command::SelectChart(1)).unwrap();
    workspace.execute(Command::NextLabel).unwrap();
    assert_eq!(workspace.row(), 96);
    workspace.execute(Command::PreviousLabel).unwrap();
    assert_eq!(workspace.row(), 96);
    workspace.execute(Command::SelectChart(0)).unwrap();
    workspace.execute(Command::NextLabel).unwrap();
    assert_eq!(workspace.row(), 144);
    workspace.execute(Command::Save).unwrap();
    let mut reopened = Workspace::open(&path, 0).unwrap();
    reopened.execute(Command::Seek(48)).unwrap();
    reopened.execute(Command::NextLabel).unwrap();
    assert_eq!(reopened.row(), 96);
}

#[test]
fn empty_sm_unsupported_tags_and_invalid_maps_preserve_cursor_and_history() {
    let dir = tempfile::tempdir().unwrap();
    for (name, text, fails) in [
        ("empty.ssc", SONG.replace("#LABELS:1=A,3=B,5=C;", ""), false),
        (
            "bad.ssc",
            SONG.replace("#LABELS:1=A,3=B,5=C;", "#LABELS:NaN=bad;"),
            true,
        ),
        (
            "song.sm",
            "#BPMS:0=120;#LABELS:1=Unknown;#NOTES:dance-single::Hard:9::1000;".into(),
            false,
        ),
    ] {
        let path = dir.path().join(name);
        fs::write(&path, text).unwrap();
        let mut workspace = Workspace::open(&path, 0).unwrap();
        let bytes = workspace.file().session.serialize().unwrap();
        workspace.execute(Command::Seek(73)).unwrap();
        for command in [Command::NextLabel, Command::PreviousLabel] {
            assert_eq!(workspace.execute(command).is_err(), fails);
            assert_eq!(workspace.row(), 73);
            assert_eq!(workspace.file().session.serialize().unwrap(), bytes);
            assert!(!workspace.file().session.is_dirty());
        }
    }
}

#[test]
fn chart_with_its_own_timing_does_not_inherit_song_labels_when_its_label_map_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG.replace("#LABELS:2=Other;", "")).unwrap();
    let mut workspace = Workspace::open(&path, 1).unwrap();
    workspace.execute(Command::Seek(73)).unwrap();
    workspace.execute(Command::NextLabel).unwrap();
    assert_eq!(workspace.row(), 73);
    workspace.execute(Command::PreviousLabel).unwrap();
    assert_eq!(workspace.row(), 73);
    workspace.execute(Command::SelectChart(0)).unwrap();
    workspace.execute(Command::NextLabel).unwrap();
    assert_eq!(workspace.row(), 144);
    assert!(!workspace.file().session.is_dirty());
}
