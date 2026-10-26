use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    notes::{Kind, Note, Position},
    transform::{self, Transform},
};
use std::sync::Arc;
fn pos(row: u64, lane: u16) -> Position {
    Position {
        row,
        lane,
        player: 0,
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
fn all_directions_and_wrap_choices_move_or_remove_edges_in_one_undoable_gesture() {
    for right in [false, true] {
        for wrap in [false, true] {
            let mut s = session();
            s.edit_notes(
                0,
                "Source",
                [
                    (pos(0, 0), Some(Note::tap())),
                    (
                        pos(0, 3),
                        Some(Note {
                            kind: Kind::Roll,
                            end: Some(96),
                            attributes: Arc::from(""),
                            tail_attributes: Arc::from(""),
                        }),
                    ),
                ],
            )
            .unwrap();
            let before = s.serialize().unwrap();
            assert!(
                transform::apply(&mut s, 0, 0, 0, None, Transform::ShiftLanes { right, wrap })
                    .unwrap()
            );
            if right {
                assert!(s.grid(0).unwrap().get(pos(0, 1)).is_some());
                assert_eq!(s.grid(0).unwrap().get(pos(0, 0)).is_some(), wrap);
                if wrap {
                    assert_eq!(s.grid(0).unwrap().get(pos(0, 0)).unwrap().end, Some(96));
                }
            } else {
                assert_eq!(s.grid(0).unwrap().get(pos(0, 2)).unwrap().end, Some(96));
                assert_eq!(s.grid(0).unwrap().get(pos(0, 3)).is_some(), wrap);
            }
            let after = s.serialize().unwrap();
            s.undo().unwrap();
            assert_eq!(s.serialize().unwrap(), before);
            s.redo().unwrap();
            assert_eq!(s.serialize().unwrap(), after);
        }
    }
}
#[test]
fn shifting_into_an_unselected_sustain_uses_force_add_and_restores_every_side_effect() {
    let mut s = session();
    s.edit_notes(
        0,
        "Source",
        [
            (
                pos(0, 1),
                Some(Note {
                    kind: Kind::Hold,
                    end: Some(96),
                    attributes: Arc::from(""),
                    tail_attributes: Arc::from(""),
                }),
            ),
            (pos(48, 0), Some(Note::tap())),
        ],
    )
    .unwrap();
    let before = s.serialize().unwrap();
    transform::apply(
        &mut s,
        0,
        48,
        48,
        None,
        Transform::ShiftLanes {
            right: true,
            wrap: false,
        },
    )
    .unwrap();
    assert_eq!(s.grid(0).unwrap().get(pos(0, 1)).unwrap().end, Some(36));
    assert!(s.grid(0).unwrap().get(pos(48, 1)).is_some());
    assert!(s.grid(0).unwrap().get(pos(48, 0)).is_none());
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn empty_and_invalid_selections_do_not_consume_redo() {
    let mut s = session();
    s.edit_notes(0, "Source", [(pos(0, 0), Some(Note::tap()))])
        .unwrap();
    s.undo().unwrap();
    let label = s.redo_label().unwrap().to_owned();
    assert!(
        !transform::apply(
            &mut s,
            0,
            0,
            192,
            None,
            Transform::ShiftLanes {
                right: false,
                wrap: false
            }
        )
        .unwrap()
    );
    assert!(
        transform::apply(
            &mut s,
            0,
            192,
            0,
            None,
            Transform::ShiftLanes {
                right: false,
                wrap: false
            }
        )
        .is_err()
    );
    assert_eq!(s.redo_label(), Some(label.as_str()));
}
