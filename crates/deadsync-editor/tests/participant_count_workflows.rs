use deadsync_editor::{
    document::{Document, Format},
    generation::metadata::ChartData,
    history::Session,
    notes::{Note, Position},
};
use std::sync::Arc;
fn session(layout: &str, format: Format) -> Session {
    let columns = deadsync_editor::notes::layout_columns(layout).unwrap();
    let note = "0".repeat(columns as usize);
    let source = if format == Format::Ssc {
        format!("#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:{layout};#NOTES:{note};")
    } else {
        format!("#NOTES:{layout}::Hard:9:0,0,0,0,0:{note};")
    };
    Session::new(Document::parse(Arc::<[u8]>::from(source.as_bytes()), format).unwrap())
}
#[test]
fn participant_addition_metadata_and_empty_blocks_round_trip_and_undo_in_both_formats() {
    for format in [Format::Sm, Format::Ssc] {
        for layout in ["dance-routine", "pump-routine", "smx-team"] {
            let mut s = session(layout, format);
            let initial_players = 2;
            assert_eq!(s.grid(0).unwrap().players(), initial_players);
            let before = s.serialize().unwrap();
            assert!(s.set_participants(0, 3).unwrap());
            assert_eq!(s.grid(0).unwrap().players(), 3);
            assert_eq!(
                ChartData::read(s.document(), 0)
                    .unwrap()
                    .unwrap()
                    .max_player_index,
                2
            );
            let added = s.serialize().unwrap();
            let mut reopened =
                Session::new(Document::parse(Arc::<[u8]>::from(added.clone()), format).unwrap());
            assert_eq!(reopened.grid(0).unwrap().players(), 3);
            s.undo().unwrap();
            assert_eq!(s.serialize().unwrap(), before);
            assert_eq!(s.grid(0).unwrap().players(), initial_players);
            s.redo().unwrap();
            assert_eq!(s.serialize().unwrap(), added);
        }
    }
}
#[test]
fn count_reduction_retains_owners_with_notes_and_can_remove_empty_trailing_players() {
    let mut s = session("dance-routine", Format::Ssc);
    s.set_participants(0, 4).unwrap();
    let pos = Position {
        row: 0,
        lane: 0,
        player: 2,
    };
    s.edit_notes(0, "Third player note", [(pos, Some(Note::tap()))])
        .unwrap();
    assert!(s.set_participants(0, 1).unwrap());
    assert_eq!(s.grid(0).unwrap().players(), 3);
    assert!(s.grid(0).unwrap().get(pos).is_some());
    let before = s.serialize().unwrap();
    assert!(!s.set_participants(0, 0).unwrap());
    assert_eq!(s.serialize().unwrap(), before);
    s.undo().unwrap();
    assert_eq!(s.grid(0).unwrap().players(), 4);
    assert!(s.grid(0).unwrap().get(pos).is_some());
    s.undo().unwrap();
    assert!(s.grid(0).unwrap().get(pos).is_none());
    s.undo().unwrap();
    assert_eq!(s.grid(0).unwrap().players(), 2);
}
#[test]
fn metadata_declared_empty_players_load_without_rewriting_notes_and_invalid_requests_preserve_redo()
{
    let mut s = session("dance-routine", Format::Ssc);
    ChartData {
        max_player_index: 2,
        ..Default::default()
    }
    .save(&mut s, 0)
    .unwrap();
    let source = s.serialize().unwrap();
    let mut s =
        Session::new(Document::parse(Arc::<[u8]>::from(source.clone()), Format::Ssc).unwrap());
    assert_eq!(s.grid(0).unwrap().players(), 3);
    assert_eq!(s.serialize().unwrap(), source);
    s.set_participants(0, 4).unwrap();
    s.undo().unwrap();
    let redo = s.redo_label().unwrap().to_owned();
    assert!(s.set_participants(0, u32::MAX).is_err());
    assert!(s.set_participants(0, i32::MAX as u32).is_err());
    assert_eq!(s.redo_label(), Some(redo.as_str()));
    assert_eq!(s.serialize().unwrap(), source);
    for layout in ["dance-single", "dance-couple", "pump-couple"] {
        let mut fixed = session(layout, Format::Ssc);
        let before = fixed.serialize().unwrap();
        assert!(fixed.set_participants(0, 3).is_err());
        assert_eq!(fixed.serialize().unwrap(), before);
    }
}

#[test]
fn declared_empty_participants_keep_original_note_bytes_after_note_and_count_undo() {
    for format in [Format::Sm, Format::Ssc] {
        for layout in ["dance-routine", "pump-routine", "smx-team"] {
            for players in [1, 3] {
                let mut original = session(layout, format);
                ChartData {
                    max_player_index: players - 1,
                    ..Default::default()
                }
                .save(&mut original, 0)
                .unwrap();
                let bytes = original.serialize().unwrap();
                let mut resident = Session::new(
                    Document::parse(Arc::<[u8]>::from(bytes.clone()), format).unwrap(),
                );
                assert_eq!(resident.grid(0).unwrap().players(), players);
                assert_eq!(resident.serialize().unwrap(), bytes);
                let position = Position {
                    player: players - 1,
                    lane: 0,
                    row: 48,
                };
                resident
                    .edit_notes(0, "Temporary note", [(position, Some(Note::tap()))])
                    .unwrap();
                let edited = resident.serialize().unwrap();
                let mut reopened = Session::new(
                    Document::parse(Arc::<[u8]>::from(edited.clone()), format).unwrap(),
                );
                assert_eq!(reopened.grid(0).unwrap().get(position), Some(&Note::tap()));
                assert_eq!(reopened.grid(0).unwrap().players(), players);
                resident.undo().unwrap();
                assert_eq!(resident.serialize().unwrap(), bytes);
                resident.redo().unwrap();
                assert_eq!(resident.serialize().unwrap(), edited);
                resident.undo().unwrap();
                resident.set_participants(0, players + 1).unwrap();
                let expanded = resident.serialize().unwrap();
                let mut reopened = Session::new(
                    Document::parse(Arc::<[u8]>::from(expanded.clone()), format).unwrap(),
                );
                assert_eq!(reopened.grid(0).unwrap().players(), players + 1);
                resident.undo().unwrap();
                assert_eq!(resident.serialize().unwrap(), bytes);
                resident.redo().unwrap();
                assert_eq!(resident.serialize().unwrap(), expanded);
            }
        }
    }
}

#[test]
fn ownership_failure_rolls_back_participant_addition_and_metadata_in_the_public_workspace() {
    use deadsync_editor::{
        transform::Transform,
        workspace::{Command, Workspace},
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let source = b"#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:10000000\n&\n10000000;";
    std::fs::write(&path, source).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    assert!(
        workspace
            .change_note_owners(3, Transform::AssignPlayer(2))
            .is_err()
    );
    assert_eq!(workspace.grid().players(), 2);
    assert_eq!(workspace.file().session.serialize().unwrap(), source);
    assert!(!workspace.file().session.is_dirty());
    assert!(workspace.file().session.undo_label().is_none());
    assert!(
        workspace
            .change_note_owners(3, Transform::SwapPlayers(0, 2))
            .unwrap()
    );
    assert_eq!(workspace.grid().players(), 3);
    workspace.execute(Command::SelectPlayer(2)).unwrap();
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.grid().players(), 2);
    assert!(workspace.player() < 2);
    assert_eq!(workspace.file().session.serialize().unwrap(), source);
}
