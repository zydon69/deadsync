use deadsync_editor::{
    notes::Position,
    workspace::{Command, Workspace},
};
fn fixture() -> (tempfile::TempDir, std::path::PathBuf, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("players.ssc");
    std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000\n&\n00000000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;").unwrap();
    let w = Workspace::open(&path, 0).unwrap();
    (dir, path, w)
}
#[test]
fn focus_restores_chart_player_and_places_note_for_that_owner() {
    let (_dir, _path, mut w) = fixture();
    let before = w.file().session.serialize().unwrap();
    w.execute(Command::SelectPlayer(1)).unwrap();
    w.execute(Command::SelectChart(1)).unwrap();
    assert_eq!(w.player(), 0);
    w.execute(Command::SelectChart(0)).unwrap();
    assert_eq!(w.player(), 1);
    assert_eq!(w.file().session.serialize().unwrap(), before);
    w.execute(Command::Place(0)).unwrap();
    assert!(
        w.grid()
            .get(Position {
                player: 1,
                lane: 0,
                row: 0
            })
            .is_some()
    );
    assert!(
        w.grid()
            .get(Position {
                player: 0,
                lane: 0,
                row: 0
            })
            .is_none()
    );
    w.execute(Command::Undo).unwrap();
    assert_eq!(w.player(), 1);
    assert_eq!(w.file().session.serialize().unwrap(), before);
}
#[test]
fn explicit_same_player_choice_clears_peers_but_closing_a_view_does_not() {
    let (_dir, _path, mut w) = fixture();
    w.execute(Command::SelectPlayer(1)).unwrap();
    w.clear_active_chart();
    w.execute(Command::SelectChart(0)).unwrap();
    assert_eq!(w.player(), 1);
    w.execute(Command::SelectChart(1)).unwrap();
    w.execute(Command::SelectPlayer(0)).unwrap();
    w.execute(Command::SelectChart(0)).unwrap();
    assert_eq!(w.player(), 0);
}
#[test]
fn duplicate_structural_history_and_reopened_document_do_not_alias_player_cache() {
    let (_dir, path, mut w) = fixture();
    w.execute(Command::SelectPlayer(1)).unwrap();
    let original = w.file().session.chart_ids()[0].clone();
    let duplicate = w.duplicate_chart().unwrap();
    assert_ne!(w.file().session.chart_ids()[duplicate], original);
    assert_eq!(w.player(), 1);
    w.execute(Command::Undo).unwrap();
    assert_eq!(w.player(), 1);
    w.execute(Command::Redo).unwrap();
    assert_eq!(w.player(), 1);
    w.execute(Command::SelectChart(1)).unwrap();
    assert_eq!(w.player(), 0);
    w.execute(Command::SelectChart(0)).unwrap();
    assert_eq!(w.player(), 1);
    let reopened = Workspace::open(&path, 0).unwrap();
    assert_eq!(reopened.player(), 0);
}

#[test]
fn deleted_chart_cache_is_removed_and_undo_reinitializes_from_current_global_player() {
    let (_dir, _path, mut w) = fixture();
    w.execute(Command::SelectPlayer(1)).unwrap();
    let draft = w.deletion_draft().unwrap();
    w.delete_chart(&draft).unwrap();
    assert_eq!(w.player(), 0);
    w.execute(Command::Undo).unwrap();
    assert_eq!(w.chart(), 0);
    assert_eq!(w.player(), 0);
    w.execute(Command::SelectPlayer(1)).unwrap();
    w.execute(Command::Redo).unwrap();
    assert_eq!(w.player(), 0);
}
