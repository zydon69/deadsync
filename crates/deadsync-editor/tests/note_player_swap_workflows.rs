use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    notes::{Kind, Note, Position},
    transform::{self, Transform},
};
use std::sync::Arc;
fn p(row: u64, lane: u16, player: u32) -> Position {
    Position { row, lane, player }
}
fn session() -> Session {
    Session::new(Document::parse(Arc::<[u8]>::from(b"#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000\n&\n00000000\n&\n00000000;".as_slice()),Format::Ssc).unwrap())
}
#[test]
fn swapping_two_of_three_players_preserves_other_notes_and_exact_undo_redo() {
    let mut s = session();
    let hold = Note {
        kind: Kind::Hold,
        end: Some(96),
        attributes: Arc::from("[7]"),
        tail_attributes: Arc::from("[8]"),
    };
    s.edit_notes(
        0,
        "Source",
        [
            (p(0, 0, 0), Some(hold.clone())),
            (p(0, 2, 2), Some(Note::tap())),
            (p(48, 1, 1), Some(Note::tap())),
            (p(192, 4, 2), Some(Note::tap())),
        ],
    )
    .unwrap();
    let before = s.serialize().unwrap();
    transform::apply(&mut s, 0, 0, 96, None, Transform::SwapPlayers(0, 2)).unwrap();
    assert_eq!(s.grid(0).unwrap().get(p(0, 0, 2)), Some(&hold));
    assert!(s.grid(0).unwrap().get(p(0, 2, 0)).is_some());
    assert!(s.grid(0).unwrap().get(p(48, 1, 1)).is_some());
    assert!(s.grid(0).unwrap().get(p(192, 4, 2)).is_some());
    let after = s.serialize().unwrap();
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
}
#[test]
fn invalid_or_identical_player_pairs_do_not_change_source_or_consume_redo() {
    let mut s = session();
    s.edit_notes(0, "Source", [(p(0, 0, 0), Some(Note::tap()))])
        .unwrap();
    s.edit_notes(0, "Temporary", [(p(48, 1, 1), Some(Note::tap()))])
        .unwrap();
    s.undo().unwrap();
    let before = s.serialize().unwrap();
    let redo = s.redo_label().unwrap().to_owned();
    assert!(!transform::apply(&mut s, 0, 0, 0, None, Transform::SwapPlayers(0, 0)).unwrap());
    for (a, b) in [(0, 3), (u32::MAX, 0)] {
        assert!(transform::apply(&mut s, 0, 0, 0, None, Transform::SwapPlayers(a, b)).is_err());
    }
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.redo_label(), Some(redo.as_str()));
}
