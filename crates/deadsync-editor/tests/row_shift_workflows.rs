use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    notes::{Kind, Note, Position},
    transform::{self, Transform},
};
use std::sync::Arc;
fn p(row: u64, lane: u16) -> Position {
    Position {
        row,
        lane,
        player: 0,
    }
}
fn hold(end: u64, roll: bool) -> Note {
    Note {
        kind: if roll { Kind::Roll } else { Kind::Hold },
        end: Some(end),
        attributes: Arc::from(""),
        tail_attributes: Arc::from(""),
    }
}
fn session() -> Session {
    Session::new(
        Document::parse(
            Arc::<[u8]>::from(
                b"#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;".as_slice(),
            ),
            Format::Ssc,
        )
        .unwrap(),
    )
}
#[test]
fn negative_heads_are_removed_and_remaining_sustains_keep_duration_with_exact_undo_redo() {
    let mut s = session();
    s.edit_notes(
        0,
        "Source",
        [
            (p(0, 0), Some(Note::tap())),
            (p(12, 1), Some(hold(60, false))),
            (p(48, 2), Some(hold(96, true))),
            (p(240, 3), Some(Note::tap())),
        ],
    )
    .unwrap();
    let before = s.serialize().unwrap();
    transform::apply(&mut s, 0, 0, 192, None, Transform::ShiftRows(-24)).unwrap();
    assert!(s.grid(0).unwrap().get(p(0, 0)).is_none());
    assert!(s.grid(0).unwrap().get(p(12, 1)).is_none());
    let roll = s.grid(0).unwrap().get(p(24, 2)).unwrap();
    assert_eq!(roll.kind, Kind::Roll);
    assert_eq!(roll.end, Some(72));
    assert!(s.grid(0).unwrap().get(p(240, 3)).is_some());
    let after = s.serialize().unwrap();
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
}
#[test]
fn shifted_sustain_shortens_an_unselected_hold_at_destination_in_one_gesture() {
    let mut s = session();
    s.edit_notes(
        0,
        "Source",
        [
            (p(0, 0), Some(hold(192, false))),
            (p(240, 0), Some(hold(300, true))),
            (p(216, 1), Some(Note::tap())),
        ],
    )
    .unwrap();
    let before = s.serialize().unwrap();
    transform::apply(&mut s, 0, 240, 240, None, Transform::ShiftRows(-144)).unwrap();
    assert_eq!(s.grid(0).unwrap().get(p(0, 0)).unwrap().end, Some(84));
    assert_eq!(s.grid(0).unwrap().get(p(96, 0)).unwrap().end, Some(156));
    assert!(s.grid(0).unwrap().get(p(240, 0)).is_none());
    assert!(s.grid(0).unwrap().get(p(216, 1)).is_some());
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn overflow_does_not_publish_partial_changes_or_discard_redo() {
    let mut s = session();
    let head = p(u64::MAX - 16, 0);
    s.edit_notes(0, "Large source", [(head, Some(hold(u64::MAX - 1, false)))])
        .unwrap();
    s.edit_notes(0, "Temporary", [(p(0, 1), Some(Note::tap()))])
        .unwrap();
    s.undo().unwrap();
    let revision = s.revision();
    let redo = s.redo_label().unwrap().to_owned();
    let source = s.grid(0).unwrap().get(head).unwrap().clone();
    assert!(transform::apply(&mut s, 0, 0, u64::MAX, None, Transform::ShiftRows(32)).is_err());
    assert_eq!(s.revision(), revision);
    assert_eq!(s.redo_label(), Some(redo.as_str()));
    assert_eq!(s.grid(0).unwrap().get(head), Some(&source));
}
