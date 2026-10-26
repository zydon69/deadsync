use deadsync_editor::{
    document::{Document, Format},
    history::{Clipboard, Session},
    notes::{Grid, Kind, Position},
    stepf2::{Attributes, Visibility},
    transform::{self, Transform},
};
use std::sync::Arc;

fn position(row: u64) -> Position {
    Position {
        row,
        lane: 0,
        player: 0,
    }
}

fn session(format: Format, head: char, fake: bool) -> Session {
    let notes = format!(
        "{{{head}|h|{}|%}}[7]00000000Y\n0000000000\n{}\n0000000000",
        u8::from(fake),
        if head == 'x' {
            "{3|v|0|_}[8]000000000"
        } else {
            "0000000000"
        },
    );
    let source = match format {
        Format::Sm => format!(
            "#TITLE:Keep;#VENDOR:untouched;#BPMS:0=120;#NOTES:pump-routine::Hard:9:0,0,0,0,0:{notes};"
        ),
        Format::Ssc => format!(
            "#VERSION:0.83;#TITLE:Keep;#VENDOR:untouched;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:pump-routine;#NOTES:{notes};"
        ),
    };
    Session::new(Document::parse(Arc::<[u8]>::from(source.as_bytes()), format).unwrap())
}

fn assert_reopens(session: &mut Session, format: Format) {
    let expected = session.grid(0).unwrap().clone();
    let bytes = session.serialize().unwrap();
    let mut reopened = Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap());
    assert_eq!(reopened.grid(0).unwrap(), &expected);
}

#[test]
fn explicit_tap_fake_and_hold_conversions_synchronize_only_the_fake_bit() {
    for format in [Format::Sm, Format::Ssc] {
        for (head, fake, from, to, expected_fake) in [
            ('X', false, Kind::Tap, Kind::Fake, true),
            ('X', true, Kind::Fake, Kind::Tap, false),
            ('x', true, Kind::Hold, Kind::Tap, false),
            ('x', false, Kind::Hold, Kind::Fake, true),
            ('X', true, Kind::Fake, Kind::Hold, true),
            ('X', false, Kind::Tap, Kind::Hold, false),
        ] {
            let mut s = session(format, head, fake);
            let before = s.serialize().unwrap();
            assert!(
                transform::apply(
                    &mut s,
                    0,
                    0,
                    0,
                    Some(0),
                    Transform::Convert {
                        from,
                        to,
                        sustain_rows: 24,
                    }
                )
                .unwrap()
            );
            let note = s.grid(0).unwrap().get(position(0)).unwrap();
            assert_eq!(note.kind, to);
            let (attrs, trailing) = Attributes::read(&note.attributes).unwrap().unwrap();
            assert_eq!(attrs.visibility, Visibility::Hidden);
            assert_eq!(attrs.reserved, '%');
            assert_eq!(attrs.fake, expected_fake);
            assert_eq!(trailing, "[7]");
            if to != Kind::Hold {
                assert!(note.tail_attributes.is_empty());
                assert!(note.end.is_none());
            }
            let after = s.serialize().unwrap();
            assert_reopens(&mut s, format);
            s.undo().unwrap();
            assert_eq!(s.serialize().unwrap(), before);
            s.redo().unwrap();
            assert_eq!(s.serialize().unwrap(), after);
        }
    }
}

#[test]
fn force_paste_shortening_preserves_fake_hold_semantics_and_head_decorations() {
    for format in [Format::Sm, Format::Ssc] {
        for fake in [false, true] {
            for row in [1, 12, 24] {
                let mut s = session(format, 'x', fake);
                let before = s.serialize().unwrap();
                let grid = Grid::parse_layout(b"X000000000", "pump-routine").unwrap();
                let clipboard = Clipboard::copy(&grid, 0, 0, Some(0)).unwrap();
                assert!(clipboard.paste_force(&mut s, 0, row, 0).unwrap());
                let note = s.grid(0).unwrap().get(position(0)).unwrap();
                assert_eq!(
                    note.kind,
                    if row > 12 {
                        Kind::Hold
                    } else if fake {
                        Kind::Fake
                    } else {
                        Kind::Tap
                    }
                );
                assert_eq!(note.end, (row > 12).then_some(row.saturating_sub(12)));
                let (attrs, trailing) = Attributes::read(&note.attributes).unwrap().unwrap();
                assert_eq!(attrs.fake, fake);
                assert_eq!(attrs.visibility, Visibility::Hidden);
                assert_eq!(attrs.reserved, '%');
                assert_eq!(trailing, "[7]");
                if row <= 12 {
                    assert!(note.tail_attributes.is_empty());
                } else {
                    assert_eq!(&*note.tail_attributes, "{N|v|0|_}[8]");
                }
                let after = s.serialize().unwrap();
                assert_reopens(&mut s, format);
                s.undo().unwrap();
                assert_eq!(s.serialize().unwrap(), before);
                s.redo().unwrap();
                assert_eq!(s.serialize().unwrap(), after);
            }
        }
    }
}

#[test]
fn unsupported_stepf2_conversions_leave_document_and_history_unchanged() {
    for to in [Kind::Roll, Kind::Lift] {
        let mut s = session(Format::Ssc, 'X', false);
        let before = s.serialize().unwrap();
        let revision = s.revision();
        assert!(
            transform::apply(
                &mut s,
                0,
                0,
                0,
                Some(0),
                Transform::Convert {
                    from: Kind::Tap,
                    to,
                    sustain_rows: 24,
                }
            )
            .is_err()
        );
        assert_eq!(s.revision(), revision);
        assert_eq!(s.serialize().unwrap(), before);
        assert!(s.undo_label().is_none());
    }
}
