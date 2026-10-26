use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    notes::{Kind, Note, Position},
    pad::{LAYOUTS, Pad},
    transform::{self, Transform},
};
use std::sync::Arc;
fn p(row: u64, lane: u16, player: u32) -> Position {
    Position { row, lane, player }
}
fn hold(end: u64) -> Note {
    Note {
        kind: Kind::Hold,
        end: Some(end),
        attributes: Arc::from(""),
        tail_attributes: Arc::from(""),
    }
}
#[test]
fn every_reference_symmetry_retains_unmapped_lanes_and_undoes_destination_side_effects() {
    let mut changed = 0;
    let mut skipped = 0;
    for layout in LAYOUTS {
        let pad = Pad::builtin(layout).unwrap();
        for kind in [0, 1, 2] {
            for lane in 0..pad.arrows.len() as u16 {
                let target = match kind {
                    0 => pad.mirror[lane as usize],
                    1 => pad.flip[lane as usize],
                    _ => pad.mirror[lane as usize].and_then(|lane| pad.flip[lane as usize]),
                };
                let source = format!(
                    "#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:{layout};#NOTES:{};",
                    "0".repeat(pad.arrows.len())
                );
                let mut s = Session::new(
                    Document::parse(Arc::<[u8]>::from(source.as_bytes()), Format::Ssc).unwrap(),
                );
                s.edit_notes(0, "Selected", [(p(48, lane, 0), Some(hold(144)))])
                    .unwrap();
                if let Some(target) = target.filter(|target| *target != lane) {
                    s.edit_notes(0, "Destination", [(p(0, target, 0), Some(hold(96)))])
                        .unwrap();
                }
                let before = s.serialize().unwrap();
                let transform = match kind {
                    0 => Transform::Mirror,
                    1 => Transform::Flip,
                    _ => Transform::MirrorAndFlip,
                };
                let applied = transform::apply(&mut s, 0, 48, 48, None, transform).unwrap();
                match target {
                    Some(target) if target != lane => {
                        changed += 1;
                        assert!(applied);
                        assert!(s.grid(0).unwrap().get(p(48, lane, 0)).is_none());
                        assert_eq!(
                            s.grid(0).unwrap().get(p(0, target, 0)).unwrap().end,
                            Some(36)
                        );
                        assert_eq!(
                            s.grid(0).unwrap().get(p(48, target, 0)).unwrap().end,
                            Some(144)
                        );
                        let after = s.serialize().unwrap();
                        s.undo().unwrap();
                        assert_eq!(s.serialize().unwrap(), before);
                        s.redo().unwrap();
                        assert_eq!(s.serialize().unwrap(), after);
                    }
                    _ => {
                        skipped += 1;
                        assert!(!applied);
                        assert_eq!(s.serialize().unwrap(), before);
                    }
                }
            }
        }
    }
    assert!(changed > 100);
    assert!(skipped > 0);
}
#[test]
fn routine_symmetry_preserves_selected_owner_and_repairs_other_owner_destination_conflicts() {
    let source = b"#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000\n&\n00000000;";
    let mut s =
        Session::new(Document::parse(Arc::<[u8]>::from(source.as_slice()), Format::Ssc).unwrap());
    let target = Pad::builtin("dance-routine").unwrap().mirror[0].unwrap();
    s.edit_notes(
        0,
        "Source",
        [
            (p(48, 0, 1), Some(Note::tap())),
            (p(0, target, 0), Some(hold(96))),
        ],
    )
    .unwrap();
    let before = s.serialize().unwrap();
    transform::apply(&mut s, 0, 48, 48, Some(1), Transform::Mirror).unwrap();
    assert!(s.grid(0).unwrap().get(p(48, target, 1)).is_some());
    assert_eq!(
        s.grid(0).unwrap().get(p(0, target, 0)).unwrap().end,
        Some(36)
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
