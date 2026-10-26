use deadsync_editor::{
    document::{Document, Format},
    history::{Clipboard, Session},
    notes::{Grid, Kind, Note, Position},
};
use std::sync::Arc;
fn pos(row: u64, lane: u16, player: u32) -> Position {
    Position { row, lane, player }
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
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;"
                    .as_slice(),
            ),
            Format::Ssc,
        )
        .unwrap(),
    )
}
fn clip(notes: Vec<(Position, Note)>, players: u32) -> Clipboard {
    let mut grid = Grid::new(4, players).unwrap();
    for (p, n) in notes {
        grid.set(p, Some(n)).unwrap();
    }
    Clipboard::copy(&grid, 0, 192, None).unwrap()
}
#[test]
fn tap_through_hold_shortens_by_twelve_rows_and_undo_restores_the_exact_source() {
    for roll in [false, true] {
        for row in [1, 12, 24, 96] {
            let mut s = session();
            s.edit_notes(0, "Original", [(pos(0, 0, 0), Some(hold(96, roll)))])
                .unwrap();
            let before = s.serialize().unwrap();
            let clipboard = clip(vec![(pos(0, 0, 0), Note::tap())], 1);
            assert!(clipboard.paste_force(&mut s, 0, row, 0).unwrap());
            let old = s.grid(0).unwrap().get(pos(0, 0, 0)).unwrap();
            if row <= 12 {
                assert_eq!(old.kind, Kind::Tap);
                assert_eq!(old.end, None);
            } else {
                assert_eq!(old.kind, if roll { Kind::Roll } else { Kind::Hold });
                assert_eq!(old.end, Some(row - 12));
            }
            assert_eq!(
                s.grid(0).unwrap().get(pos(row, 0, 0)).unwrap().kind,
                Kind::Tap
            );
            let after = s.serialize().unwrap();
            s.undo().unwrap();
            assert_eq!(s.serialize().unwrap(), before);
            s.redo().unwrap();
            assert_eq!(s.serialize().unwrap(), after);
        }
    }
}
#[test]
fn incoming_hold_removes_heads_including_its_tail_and_replaces_same_row_sustains() {
    let mut s = session();
    s.edit_notes(
        0,
        "Original",
        [
            (pos(12, 0, 0), Some(hold(24, false))),
            (pos(36, 0, 0), Some(Note::tap())),
            (pos(48, 0, 0), Some(hold(96, true))),
            (pos(12, 1, 0), Some(Note::tap())),
        ],
    )
    .unwrap();
    let before = s.serialize().unwrap();
    clip(vec![(pos(0, 0, 0), hold(36, false))], 1)
        .paste_force(&mut s, 0, 12, 0)
        .unwrap();
    assert_eq!(s.grid(0).unwrap().get(pos(12, 0, 0)).unwrap().end, Some(48));
    assert!(s.grid(0).unwrap().get(pos(36, 0, 0)).is_none());
    assert!(s.grid(0).unwrap().get(pos(48, 0, 0)).is_none());
    assert!(s.grid(0).unwrap().get(pos(12, 1, 0)).is_some());
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn physical_lane_conflicts_cross_participants_and_mixed_out_of_range_owners_clamp_to_zero() {
    let mut target=Session::new(Document::parse(Arc::<[u8]>::from(b"#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:20000000\n00000000\n30000000\n00000000\n&\n00000000;".as_slice()),Format::Ssc).unwrap());
    let original = target.serialize().unwrap();
    let mut source = Grid::new(8, 1).unwrap();
    source.set(pos(0, 0, 0), Some(Note::tap())).unwrap();
    Clipboard::copy(&source, 0, 0, None)
        .unwrap()
        .paste_force(&mut target, 0, 24, 1)
        .unwrap();
    assert_eq!(
        target.grid(0).unwrap().get(pos(0, 0, 0)).unwrap().end,
        Some(12)
    );
    assert!(target.grid(0).unwrap().get(pos(24, 0, 1)).is_some());
    target.undo().unwrap();
    assert_eq!(target.serialize().unwrap(), original);
    let mut single = session();
    clip(
        vec![(pos(0, 1, 0), Note::tap()), (pos(12, 2, 1), Note::tap())],
        2,
    )
    .paste_force(&mut single, 0, 0, 0)
    .unwrap();
    assert!(single.grid(0).unwrap().get(pos(0, 1, 0)).is_some());
    assert!(single.grid(0).unwrap().get(pos(12, 2, 0)).is_some());
}
#[test]
fn overflow_and_invalid_active_player_leave_source_and_redo_unchanged() {
    let mut s = session();
    s.edit_notes(0, "Original", [(pos(0, 0, 0), Some(Note::tap()))])
        .unwrap();
    s.undo().unwrap();
    let before = s.serialize().unwrap();
    let label = s.redo_label().unwrap().to_owned();
    let clipboard = clip(vec![(pos(1, 0, 0), Note::tap())], 1);
    assert!(clipboard.paste_force(&mut s, 0, u64::MAX, 0).is_err());
    assert!(clipboard.paste_force(&mut s, 0, 0, 99).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.redo_label(), Some(label.as_str()));
}
