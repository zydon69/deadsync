use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    notes::{Kind, Position},
};
use std::sync::Arc;

#[test]
fn dance_columns_keep_left_down_up_right_through_parse_edit_save_and_reopen() {
    for (format, source) in [
        (
            Format::Sm,
            "#NOTES:dance-single::Edit:5:0,0,0,0,0:1000\n0100\n0010\n0001;",
        ),
        (
            Format::Ssc,
            "#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;",
        ),
    ] {
        let mut session =
            Session::new(Document::parse(Arc::<[u8]>::from(source.as_bytes()), format).unwrap());
        let positions: Vec<_> = (0..4)
            .map(|lane| Position {
                row: u64::from(lane) * 48,
                lane,
                player: 0,
            })
            .collect();
        for position in &positions {
            assert_eq!(
                session.grid(0).unwrap().get(*position).unwrap().kind,
                Kind::Tap
            );
        }
        // An actual edit exercises the writer, rather than its unchanged-source shortcut.
        let left = positions[0];
        let mut mine = session.grid(0).unwrap().get(left).unwrap().clone();
        mine.kind = Kind::Mine;
        session
            .edit_notes(0, "Left mine", [(left, Some(mine))])
            .unwrap();
        let bytes = session.serialize().unwrap();
        let mut reopened = Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap());
        for position in &positions {
            let expected = if position.lane == 0 {
                Kind::Mine
            } else {
                Kind::Tap
            };
            assert_eq!(
                reopened.grid(0).unwrap().get(*position).unwrap().kind,
                expected
            );
            for other in 0..4 {
                if other != position.lane {
                    assert!(
                        reopened
                            .grid(0)
                            .unwrap()
                            .get(Position {
                                lane: other,
                                ..*position
                            })
                            .is_none()
                    );
                }
            }
        }
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), source.as_bytes());
    }
}
