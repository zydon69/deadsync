use deadsync_editor::{
    notes::Position,
    workspace::{Command, Workspace},
};
use std::fs;
fn setup() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let workspace = Workspace::open(&path, 0).unwrap();
    (dir, workspace)
}
#[test]
fn view_navigation_preserves_selection_and_copy_consumes_original_range() {
    let (_dir, mut workspace) = setup();
    workspace.execute(Command::ToggleSelection).unwrap();
    workspace.execute(Command::Seek(48)).unwrap();
    let before = workspace.file().session.serialize().unwrap();
    workspace.seek_preserving_selection(192).unwrap();
    workspace.seek_preserving_selection(240).unwrap();
    assert_eq!(workspace.row(), 240);
    assert_eq!(workspace.selection(), Some((0, 48)));
    workspace.execute(Command::Copy).unwrap();
    assert_eq!(workspace.selection(), Some((0, 48)));
    assert_eq!(workspace.file().session.serialize().unwrap(), before);
    assert!(!workspace.file().session.is_dirty());
    workspace.execute(Command::ToggleSelection).unwrap();
    workspace.execute(Command::Seek(384)).unwrap();
    workspace.execute(Command::Paste).unwrap();
    let grid = workspace.grid();
    assert!(
        grid.get(Position {
            player: 0,
            lane: 0,
            row: 384
        })
        .is_some()
    );
    assert!(
        grid.get(Position {
            player: 0,
            lane: 1,
            row: 432
        })
        .is_some()
    );
    assert!(
        grid.get(Position {
            player: 0,
            lane: 2,
            row: 480
        })
        .is_none()
    );
}
#[test]
fn ordinary_cursor_moves_reattach_selection_and_new_selection_drops_frozen_endpoint() {
    let (_dir, mut workspace) = setup();
    workspace.execute(Command::ToggleSelection).unwrap();
    workspace.execute(Command::Seek(48)).unwrap();
    workspace.seek_preserving_selection(192).unwrap();
    assert_eq!(workspace.selection(), Some((0, 48)));
    workspace.execute(Command::Move(1)).unwrap();
    assert_eq!(workspace.selection(), Some((0, 240)));
    workspace.seek_preserving_selection(384).unwrap();
    workspace.execute(Command::ToggleSelection).unwrap();
    assert_eq!(workspace.selection(), None);
    workspace.execute(Command::ToggleSelection).unwrap();
    assert_eq!(workspace.selection(), Some((384, 384)));
}

#[test]
fn automatic_advance_and_saved_pattern_navigation_reattach_the_selection_endpoint() {
    let (dir, mut workspace) = setup();
    workspace.execute(Command::ToggleSelection).unwrap();
    workspace.execute(Command::Seek(48)).unwrap();
    workspace.seek_preserving_selection(192).unwrap();
    workspace.execute(Command::ToggleAutoAdvance).unwrap();
    workspace.execute(Command::Place(3)).unwrap();
    assert_eq!(workspace.selection(), Some((0, 240)));

    use deadsync_editor::{
        document::{Document, Format},
        generation::metadata::{ChartData, Definition},
        history::Session,
    };
    let source = fs::read(dir.path().join("song.ssc")).unwrap();
    let mut session = Session::new(Document::parse(source.as_slice(), Format::Ssc).unwrap());
    ChartData {
        patterns: Some(
            [(
                96,
                Definition {
                    length: 48,
                    ..Default::default()
                },
            )]
            .into(),
        ),
        ..Default::default()
    }
    .save(&mut session, 0)
    .unwrap();
    let path = dir.path().join("patterns.ssc");
    fs::write(&path, session.serialize().unwrap()).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    workspace.execute(Command::ToggleSelection).unwrap();
    workspace.execute(Command::Seek(48)).unwrap();
    workspace.seek_preserving_selection(64).unwrap();
    assert!(workspace.navigate_saved_pattern(true).unwrap());
    assert_eq!(workspace.row(), 96);
    assert_eq!(workspace.selection(), Some((0, 96)));
}
