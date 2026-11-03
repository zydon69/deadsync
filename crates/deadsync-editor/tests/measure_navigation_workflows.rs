use deadsync_editor::{
    timing_edit::{
        Scope,
        signature::{Draft, Signature, Signatures},
    },
    workspace::{Command, SNAP_ROWS, Workspace},
};
use std::fs;
const SONG: &str = "#VERSION:0.83;#BPMS:0=120;#TIMESIGNATURES:0=3=4,4=7=8,8=2=2;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=90;#TIMESIGNATURES:0=5=8;#NOTES:0100;";
fn setup(text: &str) -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, text).unwrap();
    let workspace = Workspace::open(&path, 0).unwrap();
    (dir, workspace)
}
#[test]
fn departure_signature_is_used_across_boundaries_without_rounding_to_measure_start() {
    let map = Signatures::parse("0=3=4,4=7=8,8=2=2").unwrap();
    for (row, previous, next) in [
        (0, -144, 144),
        (49, -95, 193),
        (191, 47, 335),
        (192, 24, 360),
        (193, 25, 361),
        (383, 215, 551),
        (384, 192, 576),
    ] {
        assert_eq!(map.measure_destination(row, true).unwrap(), previous);
        assert_eq!(map.measure_destination(row, false).unwrap(), next);
    }
    let legacy = Signatures::parse("0=5=6,4=7=192").unwrap();
    assert_eq!(legacy.measure_destination(0, false).unwrap(), 160);
    assert_eq!(legacy.measure_destination(192, false).unwrap(), 199);
    assert!(
        legacy
            .measure_destination(i32::MAX as u64 + 1, false)
            .is_err()
    );
}
#[test]
fn measure_movement_ignores_snap_preserves_selection_and_does_not_add_history() {
    let (_dir, mut workspace) = setup(SONG);
    let bytes = workspace.file().session.serialize().unwrap();
    for snap in 0..SNAP_ROWS.len() {
        workspace.execute(Command::SetSnap(snap)).unwrap();
        workspace.execute(Command::Seek(49)).unwrap();
        workspace.execute(Command::NextMeasure).unwrap();
        assert_eq!(workspace.row(), 193);
        workspace.execute(Command::PreviousMeasure).unwrap();
        assert_eq!(workspace.row(), 25); // current signature changed after the first jump
    }
    workspace.execute(Command::Seek(49)).unwrap();
    workspace.execute(Command::ToggleSelection).unwrap();
    workspace.execute(Command::NextMeasure).unwrap();
    assert_eq!(workspace.selection(), Some((49, 193)));
    assert_eq!(workspace.file().session.serialize().unwrap(), bytes);
    assert!(!workspace.file().session.is_dirty());
    assert_eq!(workspace.file().session.undo_label(), None);
}
#[test]
fn effective_chart_signatures_unsaved_edits_and_undo_are_used_immediately() {
    let (_dir, mut workspace) = setup(SONG);
    let mut draft = Draft::capture(&workspace.file().session, 0, Scope::Chart).unwrap();
    draft.set(0, Signature::parse("5/8").unwrap()).unwrap();
    workspace.apply_signatures(&draft).unwrap();
    workspace.execute(Command::NextMeasure).unwrap();
    assert_eq!(workspace.row(), 120);
    workspace.execute(Command::Undo).unwrap();
    workspace.execute(Command::Seek(0)).unwrap();
    workspace.execute(Command::NextMeasure).unwrap();
    assert_eq!(workspace.row(), 144);
    workspace.execute(Command::Redo).unwrap();
    workspace.execute(Command::Seek(0)).unwrap();
    workspace.execute(Command::NextMeasure).unwrap();
    assert_eq!(workspace.row(), 120);
    workspace.execute(Command::SelectChart(1)).unwrap();
    workspace.execute(Command::Seek(49)).unwrap();
    workspace.execute(Command::NextMeasure).unwrap();
    assert_eq!(workspace.row(), 169);
    workspace.execute(Command::Save).unwrap();
    let mut reopened = Workspace::open(workspace.file().require_existing_path().unwrap(), 0).unwrap();
    reopened.execute(Command::NextMeasure).unwrap();
    assert_eq!(reopened.row(), 120);
}
#[test]
fn default_signature_bounds_and_invalid_maps_do_not_corrupt_document() {
    let (_dir, mut workspace) = setup(&SONG.replace("#TIMESIGNATURES:0=3=4,4=7=8,8=2=2;", ""));
    workspace.execute(Command::NextMeasure).unwrap();
    assert_eq!(workspace.row(), 192);
    workspace.execute(Command::PreviousMeasure).unwrap();
    workspace.execute(Command::PreviousMeasure).unwrap();
    assert_eq!(workspace.row(), 0); // current unsigned workspace boundary
    workspace.execute(Command::Seek(i32::MAX as u64)).unwrap();
    workspace.execute(Command::NextMeasure).unwrap();
    assert_eq!(workspace.row(), i32::MAX as u64);
    let (_dir, mut invalid) = setup(&SONG.replace("0=3=4,4=7=8,8=2=2", "NaN=3=4"));
    invalid.execute(Command::Seek(49)).unwrap();
    let bytes = invalid.file().session.serialize().unwrap();
    assert!(invalid.execute(Command::NextMeasure).is_err());
    assert!(invalid.execute(Command::PreviousMeasure).is_err());
    assert_eq!(invalid.row(), 49);
    assert_eq!(invalid.file().session.serialize().unwrap(), bytes);
    assert!(!invalid.file().session.is_dirty());
}
