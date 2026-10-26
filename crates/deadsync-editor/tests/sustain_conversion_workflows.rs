use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    notes::{Kind, Note, Position},
    transform::{self, Transform},
};
use std::sync::Arc;
fn p() -> Position {
    Position {
        row: 48,
        lane: 0,
        player: 0,
    }
}
fn session(note: Note) -> Session {
    let mut s = Session::new(
        Document::parse(
            Arc::<[u8]>::from(
                b"#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;".as_slice(),
            ),
            Format::Ssc,
        )
        .unwrap(),
    );
    s.edit_notes(0, "Source", [(p(), Some(note))]).unwrap();
    s
}
fn sustain(kind: Kind) -> Note {
    Note {
        kind,
        end: Some(96),
        attributes: Arc::from(""),
        tail_attributes: Arc::from(""),
    }
}
#[test]
fn existing_sustain_duration_does_not_evaluate_an_unused_new_duration() {
    for (from, to) in [(Kind::Hold, Kind::Roll), (Kind::Roll, Kind::Hold)] {
        for sustain_rows in [0, 48, u64::MAX] {
            let mut s = session(sustain(from));
            let before = s.serialize().unwrap();
            assert!(
                transform::apply(
                    &mut s,
                    0,
                    48,
                    48,
                    None,
                    Transform::Convert {
                        from,
                        to,
                        sustain_rows
                    }
                )
                .unwrap()
            );
            let note = s.grid(0).unwrap().get(p()).unwrap();
            assert_eq!(note.kind, to);
            assert_eq!(note.end, Some(96));
            s.undo().unwrap();
            assert_eq!(s.serialize().unwrap(), before);
            s.redo().unwrap();
            assert_eq!(s.grid(0).unwrap().get(p()).unwrap().end, Some(96));
        }
    }
}
#[test]
fn new_sustain_requires_a_valid_duration_and_failures_preserve_source_and_redo() {
    let mut s = session(Note::tap());
    s.edit_notes(
        0,
        "Temporary",
        [(Position { lane: 1, ..p() }, Some(Note::tap()))],
    )
    .unwrap();
    s.undo().unwrap();
    let before = s.serialize().unwrap();
    let redo = s.redo_label().unwrap().to_owned();
    for sustain_rows in [0, u64::MAX] {
        assert!(
            transform::apply(
                &mut s,
                0,
                48,
                48,
                None,
                Transform::Convert {
                    from: Kind::Tap,
                    to: Kind::Hold,
                    sustain_rows
                }
            )
            .is_err()
        );
        assert_eq!(s.serialize().unwrap(), before);
        assert_eq!(s.redo_label(), Some(redo.as_str()));
    }
    assert!(
        transform::apply(
            &mut s,
            0,
            48,
            48,
            None,
            Transform::Convert {
                from: Kind::Tap,
                to: Kind::Hold,
                sustain_rows: 24
            }
        )
        .unwrap()
    );
    assert_eq!(s.grid(0).unwrap().get(p()).unwrap().end, Some(72));
}
#[test]
fn converting_to_a_tap_removes_the_sustain_end_and_undo_restores_the_hold() {
    let mut s = session(sustain(Kind::Hold));
    let before = s.serialize().unwrap();
    transform::apply(
        &mut s,
        0,
        48,
        48,
        None,
        Transform::Convert {
            from: Kind::Hold,
            to: Kind::Tap,
            sustain_rows: u64::MAX,
        },
    )
    .unwrap();
    assert_eq!(s.grid(0).unwrap().get(p()).unwrap().end, None);
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
