use deadsync_editor::{
    notes::{Grid, Note, Position},
    workspace::TOOLS,
};
#[test]
fn source_ignored_empty_attributes_and_unknown_symbols_do_not_block_unrelated_edit_or_disappear() {
    for token in ["0[7]", "0{drunk:2}", "0<item>", "Z", "?"] {
        let raw = format!("{token}100\n0000\n0000\n0000");
        let mut grid = Grid::parse(raw.as_bytes(), 4).unwrap();
        assert_eq!(grid.iter().count(), 1, "{token} is not a source lane event");
        let original = grid.clone();
        let pos = Position {
            player: 0,
            lane: 0,
            row: 0,
        };
        assert!(grid.set(pos, Some(Note::tap())).is_err());
        assert_eq!(grid, original);
        grid.set(
            Position {
                player: 0,
                lane: 2,
                row: 48,
            },
            Some(Note::tap()),
        )
        .unwrap();
        let saved = grid.serialize().unwrap();
        let text = std::str::from_utf8(&saved).unwrap();
        assert!(text.contains(token), "Opaque token lost: {text}");
        let again = Grid::parse(&saved, 4).unwrap();
        assert_eq!(again, grid);
        assert_eq!(again.iter().count(), 2);
    }
}
#[test]
fn lane_entry_tools_match_source_editor_events_and_keep_timed_attacks_separate() {
    use deadsync_editor::notes::Kind;
    assert_eq!(
        TOOLS,
        [
            Kind::Tap,
            Kind::Hold,
            Kind::Roll,
            Kind::Mine,
            Kind::Lift,
            Kind::Fake
        ]
    );
    assert!(!TOOLS.contains(&Kind::Attack));
    assert!(!TOOLS.contains(&Kind::Keysound));
}

#[test]
fn fumen_real_parser_fixture_marks_opaque_cells_as_non_events() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/source_opaque_cells.expected.json")).unwrap();
    for case in reference.as_array().unwrap().iter().take(5) {
        assert_eq!(case["valid"], true);
        let raw = format!("{}\n0000\n0000\n0000", case["row"].as_str().unwrap());
        let grid = Grid::parse(raw.as_bytes(), 4).unwrap();
        assert_eq!(
            grid.iter().count(),
            case["events"].as_array().unwrap().len()
        );
        assert_eq!(case["events"][0]["SourceType"], "1");
    }
}

#[test]
fn opaque_cells_survive_session_transforms_duplicate_history_save_and_reopen() {
    use deadsync_editor::{
        storage::FileSession,
        transform::{self, Transform},
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("opaque.sm");
    std::fs::write(
        &path,
        b"#BPMS:0=120;#NOTES:dance-single::Easy:1:0:\n0[7]100\n0000\n0000\n0000;",
    )
    .unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let before = file.session.serialize().unwrap();
    assert!(transform::apply(&mut file.session, 0, 0, 0, None, Transform::ShiftRows(48)).unwrap());
    let moved = file.session.serialize().unwrap();
    assert!(String::from_utf8_lossy(&moved).contains("0[7]"));
    assert!(file.session.undo().unwrap());
    assert_eq!(file.session.serialize().unwrap(), before);
    assert!(file.session.redo().unwrap());
    assert_eq!(file.session.serialize().unwrap(), moved);
    let revision = file.session.revision();
    assert_eq!(file.session.duplicate_chart(0, revision).unwrap(), 1);
    assert_eq!(
        file.session.grid(0).unwrap().clone(),
        file.session.grid(1).unwrap().clone()
    );
    file.save().unwrap();
    let mut reopened = FileSession::open(&path).unwrap();
    assert_eq!(
        reopened.session.grid(0).unwrap().clone(),
        file.session.grid(0).unwrap().clone()
    );
    assert_eq!(
        reopened.session.grid(1).unwrap().clone(),
        file.session.grid(1).unwrap().clone()
    );
    // A transform onto the opaque lane is refused atomically by the actual owner.
    assert!(transform::apply(
        &mut reopened.session,
        0,
        48,
        48,
        None,
        Transform::ShiftRows(-48)
    )
    .unwrap());
    let intact = reopened.session.serialize().unwrap();
    assert!(transform::apply(
        &mut reopened.session,
        0,
        0,
        0,
        None,
        Transform::ShiftLanes {
            right: false,
            wrap: false
        }
    )
    .is_err());
    assert_eq!(reopened.session.serialize().unwrap(), intact);
}
#[test]
fn direct_stepf2_export_does_not_discard_opaque_standard_cells() {
    let grid = Grid::parse(b"0[7]000000000", 10).unwrap();
    assert!(deadsync_editor::stepf2::serialize(&grid).is_err());
}
#[test]
fn participant_reduction_cannot_discard_opaque_cell_owner() {
    use deadsync_editor::{
        document::{Document, Format},
        history::Session,
    };
    let raw=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#DIFFICULTY:Easy;#METER:1;#NOTES:\n00000000\n&\n0[7]0000000;";
    let mut session = Session::new(Document::parse(raw.as_slice(), Format::Ssc).unwrap());
    assert_eq!(session.grid(0).unwrap().minimum_players(), 2);
    let before = session.serialize().unwrap();
    assert!(!session.set_participants(0, 1).unwrap());
    assert_eq!(session.grid(0).unwrap().players(), 2);
    assert_eq!(session.serialize().unwrap(), before);
}
