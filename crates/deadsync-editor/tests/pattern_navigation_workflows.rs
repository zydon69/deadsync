use deadsync_editor::{
    document::{Document, Format},
    generation::metadata::{ChartData, Definition},
    history::Session,
    workspace::{Command, Workspace},
};
use serde_json::Value;
use std::{fs, path::PathBuf};
const SONG: &str = "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0100;";
fn setup(starts: &[u64]) -> (tempfile::TempDir, Workspace) {
    let mut session = Session::new(Document::parse(SONG.as_bytes(), Format::Ssc).unwrap());
    let data = ChartData {
        patterns: Some(
            starts
                .iter()
                .map(|&row| {
                    (
                        row,
                        Definition {
                            length: 96.min(i32::MAX as u64 - row),
                            ..Default::default()
                        },
                    )
                })
                .collect(),
        ),
        ..Default::default()
    };
    data.save(&mut session, 0).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, session.serialize().unwrap()).unwrap();
    let workspace = Workspace::open(&path, 0).unwrap();
    (dir, workspace)
}
#[test]
fn saved_pattern_neighbors_and_selected_rows_match_original_commands() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("pattern-navigation-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            request["Rows"].as_array().unwrap().len(),
            expected.as_array().unwrap().len()
        );
        let starts = request["Starts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row.as_u64().unwrap())
            .collect::<Vec<_>>();
        let (_dir, mut workspace) = setup(&starts);
        let bytes = workspace.file().session.serialize().unwrap();
        let data = ChartData::read(workspace.file().session.document(), 0)
            .unwrap()
            .unwrap();
        for query in expected.as_array().unwrap() {
            let row = query["Row"].as_u64().unwrap();
            for (forward, key) in [(false, "Previous"), (true, "Next")] {
                workspace.execute(Command::Seek(row)).unwrap();
                let selected = query[key]["Selected"].as_u64();
                assert_eq!(
                    data.adjacent_marker(row, forward),
                    selected,
                    "{name} {row} {key}"
                );
                assert_eq!(
                    workspace.navigate_saved_pattern(forward).unwrap(),
                    selected.is_some()
                );
                assert_eq!(
                    workspace.row() as f64,
                    query[key]["Cursor"].as_f64().unwrap()
                );
                assert_eq!(workspace.file().session.serialize().unwrap(), bytes);
                assert!(!workspace.file().session.is_dirty());
            }
        }
    }
    assert_eq!(count, 4);
}
#[test]
fn navigation_keeps_note_selection_and_reads_unsaved_marker_moves_deletions_and_undo() {
    let (_dir, mut workspace) = setup(&[48, 96, 288]);
    workspace.execute(Command::Seek(24)).unwrap();
    workspace.execute(Command::ToggleSelection).unwrap();
    assert!(workspace.navigate_saved_pattern(true).unwrap());
    assert_eq!(workspace.row(), 48);
    assert_eq!(workspace.selection(), Some((24, 48)));
    let mut draft = workspace.edit_saved_pattern().unwrap();
    draft.start = 72;
    workspace.apply_marker_edit(&draft, false).unwrap();
    workspace.execute(Command::Seek(24)).unwrap();
    workspace.navigate_saved_pattern(true).unwrap();
    assert_eq!(workspace.row(), 72);
    workspace.execute(Command::Undo).unwrap();
    workspace.execute(Command::Seek(24)).unwrap();
    workspace.navigate_saved_pattern(true).unwrap();
    assert_eq!(workspace.row(), 48);
    workspace.execute(Command::Redo).unwrap();
    workspace.execute(Command::Seek(24)).unwrap();
    workspace.navigate_saved_pattern(true).unwrap();
    assert_eq!(workspace.row(), 72);
    let draft = workspace.edit_saved_pattern().unwrap();
    workspace.apply_marker_edit(&draft, true).unwrap();
    workspace.execute(Command::Seek(24)).unwrap();
    workspace.navigate_saved_pattern(true).unwrap();
    assert_eq!(workspace.row(), 96);
    workspace.execute(Command::Undo).unwrap();
    workspace.execute(Command::Seek(24)).unwrap();
    workspace.navigate_saved_pattern(true).unwrap();
    assert_eq!(workspace.row(), 72);
    workspace.execute(Command::SelectChart(1)).unwrap();
    assert!(!workspace.navigate_saved_pattern(true).unwrap());
    assert_eq!(workspace.row(), 72);
    workspace.execute(Command::SelectChart(0)).unwrap();
    workspace.execute(Command::Save).unwrap();
    let mut reopened = Workspace::open(workspace.file().path(), 0).unwrap();
    reopened.navigate_saved_pattern(true).unwrap();
    assert_eq!(reopened.row(), 72);
}
#[test]
fn invalid_metadata_keeps_cursor_selection_and_history_unchanged() {
    let (_dir, mut workspace) = setup(&[]);
    workspace.execute(Command::Seek(24)).unwrap();
    workspace.execute(Command::ToggleSelection).unwrap();
    for forward in [false, true] {
        assert!(!workspace.navigate_saved_pattern(forward).unwrap());
        assert_eq!(workspace.row(), 24);
        assert_eq!(workspace.selection(), Some((24, 24)));
        assert!(!workspace.file().session.is_dirty());
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.ssc");
    fs::write(
        &path,
        SONG.replace(
            "#NOTES:1000;",
            "#GAChartDataVersion:99;#GAChartData:{};#NOTES:1000;",
        ),
    )
    .unwrap();
    let mut invalid = Workspace::open(&path, 0).unwrap();
    invalid.execute(Command::Seek(24)).unwrap();
    invalid.execute(Command::ToggleSelection).unwrap();
    let before = invalid.file().session.serialize().unwrap();
    for forward in [false, true] {
        assert!(invalid.navigate_saved_pattern(forward).is_err());
        assert_eq!(invalid.row(), 24);
        assert_eq!(invalid.selection(), Some((24, 24)));
        assert_eq!(invalid.file().session.serialize().unwrap(), before);
        assert!(!invalid.file().session.is_dirty());
    }
}
