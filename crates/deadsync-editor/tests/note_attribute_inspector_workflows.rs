use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    note_attributes::{Draft, Field},
    notes::{Kind, Note, Position},
};
use std::sync::Arc;
fn position() -> Position {
    Position {
        row: 0,
        lane: 0,
        player: 0,
    }
}
fn session(format: Format, kind: Kind) -> Session {
    let source: &[u8] = match format {
        Format::Ssc => {
            b"#VERSION:0.83;#CUSTOM:preserve;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
        }
        Format::Sm => b"#CUSTOM:preserve;#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;",
    };
    let mut session = Session::new(Document::parse(Arc::<[u8]>::from(source), format).unwrap());
    session
        .edit_notes(
            0,
            "Fixture",
            [(
                position(),
                Some(Note {
                    kind,
                    end: matches!(kind, Kind::Hold | Kind::Roll).then_some(96),
                    attributes: "[1]".into(),
                    tail_attributes: "".into(),
                }),
            )],
        )
        .unwrap();
    session
}
#[test]
fn attributes_round_trip_all_note_kinds_in_sm_and_ssc_with_exact_undo() {
    for format in [Format::Sm, Format::Ssc] {
        for kind in [
            Kind::Tap,
            Kind::Hold,
            Kind::Roll,
            Kind::Mine,
            Kind::Lift,
            Kind::Fake,
            Kind::Keysound,
            Kind::Attack,
        ] {
            let mut s = session(format, kind);
            let before = s.serialize().unwrap();
            let original = s.grid(0).unwrap().get(position()).unwrap().clone();
            let mut draft = Draft::capture(&mut s, 0, position()).unwrap();
            assert!(!draft.apply(&mut s, 0).unwrap());
            draft.set_text(Field::Head, "[7]").unwrap();
            if original.end.is_some() {
                draft.set_text(Field::Tail, "[8]").unwrap();
            }
            assert!(draft.apply(&mut s, 0).unwrap());
            let after = s.serialize().unwrap();
            let note = s.grid(0).unwrap().get(position()).unwrap().clone();
            assert_eq!(note.kind, original.kind);
            assert_eq!(note.end, original.end);
            assert_eq!(&*note.attributes, "[7]");
            let mut reopened =
                Session::new(Document::parse(Arc::<[u8]>::from(after.clone()), format).unwrap());
            assert_eq!(reopened.grid(0).unwrap().get(position()), Some(&note));
            assert!(String::from_utf8_lossy(&after).contains("#CUSTOM:preserve;"));
            s.undo().unwrap();
            assert_eq!(s.serialize().unwrap(), before);
            let unchanged = Draft::capture(&mut s, 0, position()).unwrap();
            let redo_label = s.redo_label().map(str::to_owned);
            assert!(!unchanged.apply(&mut s, 0).unwrap());
            assert_eq!(s.redo_label(), redo_label.as_deref());
            s.redo().unwrap();
            assert_eq!(s.serialize().unwrap(), after);
        }
    }
}
#[test]
fn invalid_attributes_fail_atomically_and_draft_can_be_corrected() {
    for field in [Field::Head, Field::Tail] {
        for invalid in ["[", ";#TITLE:injected", "//comment", "[3]1"] {
            let mut s = session(Format::Ssc, Kind::Hold);
            let before = s.serialize().unwrap();
            let revision = s.revision();
            let mut draft = Draft::capture(&mut s, 0, position()).unwrap();
            draft.set_text(field, invalid).unwrap();
            assert!(draft.apply(&mut s, 0).is_err(), "{invalid}");
            assert_eq!(s.revision(), revision);
            assert_eq!(s.serialize().unwrap(), before);
            draft.set_text(field, "[5]").unwrap();
            assert!(draft.apply(&mut s, 0).unwrap());
        }
    }
    let mut s = session(Format::Ssc, Kind::Tap);
    let mut draft = Draft::capture(&mut s, 0, position()).unwrap();
    assert!(draft.set_text(Field::Head, "[3]\n").is_err());
    draft.set_text(Field::Tail, "[4]").unwrap();
    assert!(draft.apply(&mut s, 0).is_err());
}
#[test]
fn stale_foreign_missing_and_wrong_chart_drafts_do_not_modify_notes() {
    let mut s = session(Format::Ssc, Kind::Tap);
    assert!(
        Draft::capture(
            &mut s,
            0,
            Position {
                lane: 3,
                ..position()
            }
        )
        .is_err()
    );
    let mut draft = Draft::capture(&mut s, 0, position()).unwrap();
    draft.set_text(Field::Head, "[2]").unwrap();
    assert!(draft.apply(&mut s, 1).is_err());
    let mut foreign = session(Format::Ssc, Kind::Tap);
    assert!(draft.apply(&mut foreign, 0).is_err());
    s.edit_notes(
        0,
        "Other",
        [(
            Position {
                lane: 1,
                ..position()
            },
            Some(Note::tap()),
        )],
    )
    .unwrap();
    let before = s.serialize().unwrap();
    assert!(draft.apply(&mut s, 0).is_err());
    assert_eq!(s.serialize().unwrap(), before);
}

#[test]
fn workspace_inspects_only_current_row_and_active_routine_participant() {
    use deadsync_editor::workspace::{Command, Workspace};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("routine.ssc");
    std::fs::write(
        &path,
        b"#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:10000000&1[3]0000000;",
    )
    .unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    workspace.execute(Command::SelectPlayer(1)).unwrap();
    let mut draft = workspace.note_attributes(0).unwrap();
    assert_eq!(draft.position().player, 1);
    draft.set_text(Field::Head, "[9]").unwrap();
    workspace.apply_note_attributes(&draft).unwrap();
    assert_eq!(
        &*workspace
            .grid()
            .get(Position {
                player: 0,
                ..position()
            })
            .unwrap()
            .attributes,
        ""
    );
    assert_eq!(
        &*workspace
            .grid()
            .get(Position {
                player: 1,
                ..position()
            })
            .unwrap()
            .attributes,
        "[9]"
    );
    workspace.execute(Command::Seek(48)).unwrap();
    assert!(workspace.note_attributes(0).is_err());
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(
        &*workspace
            .grid()
            .get(Position {
                player: 1,
                ..position()
            })
            .unwrap()
            .attributes,
        "[3]"
    );
}
