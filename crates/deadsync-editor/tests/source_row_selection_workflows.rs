use deadsync_editor::{event_clipboard::Event, workspace::Workspace};
fn fixture() -> (tempfile::TempDir, Workspace) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("song.ssc");
    std::fs::write(&path, "#VERSION:0.83;#BPMS:0=120;#LABELS:1=not selected;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let workspace = Workspace::open(&path, 0).unwrap();
    (directory, workspace)
}
#[test]
fn source_range_finishes_checked_notes_with_exact_nearest_rows_without_moving_focus() {
    let (_directory, mut workspace) = fixture();
    let before = workspace.file().session.serialize().unwrap();
    assert!(
        workspace
            .toggle_source_row_selection_at(0.5)
            .unwrap()
            .is_none()
    );
    workspace.seek_preserving_selection(47).unwrap();
    assert_eq!(
        workspace.selection(),
        Some((0, 47)),
        "active source range follows cursor"
    );
    let selected = workspace
        .toggle_source_row_selection_at(47.6)
        .unwrap()
        .unwrap();
    assert!(workspace.selection().is_none());
    assert_eq!(workspace.row(), 47);
    let events = selected.selected_events().collect::<Vec<_>>();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|event| matches!(event, Event::Note(_))));
    assert_eq!(
        events.iter().map(|event| event.row()).collect::<Vec<_>>(),
        vec![0, 48]
    );
    assert_eq!(workspace.file().session.serialize().unwrap(), before);
    assert!(workspace.file().session.undo_label().is_none());
}
#[test]
fn negative_source_anchor_does_not_accidentally_select_row_zero() {
    let (_directory, mut workspace) = fixture();
    workspace.toggle_source_row_selection_at(-2.6).unwrap();
    let selected = workspace
        .toggle_source_row_selection_at(-1.6)
        .unwrap()
        .unwrap();
    assert_eq!(selected.selected_count(), 0);
    workspace.toggle_source_row_selection_at(-2.6).unwrap();
    let selected = workspace
        .toggle_source_row_selection_at(0.5)
        .unwrap()
        .unwrap();
    assert_eq!(
        selected
            .selected_events()
            .map(|event| event.row())
            .collect::<Vec<_>>(),
        vec![0]
    );
}
