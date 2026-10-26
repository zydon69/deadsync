use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    note_attributes::{Draft, Field},
    notes::{Grid, Kind, Note, Position},
    stepf2::{self, Attributes, Visibility},
    storage::FileSession,
};

fn pos(player: u32, lane: u16, row: u64) -> Position {
    Position { player, lane, row }
}
fn source(format: Format, notes: &str) -> String {
    match format {
        Format::Sm => format!(
            "// keep source\n#TITLE:StepF2;#VENDOR:unchanged;#BPMS:0=120;#NOTES:pump-routine::Hard:9:0,0,0,0,0:{notes};\n#NOTES:dance-single::Easy:2:0,0,0,0,0:0001;"
        ),
        Format::Ssc => format!(
            "// keep source\n#VERSION:0.83;#TITLE:StepF2;#VENDOR:unchanged;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:pump-routine;#NOTES:{notes};\n#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0001;"
        ),
    }
}

#[test]
fn all_four_owners_and_sustain_tails_follow_the_shared_physical_lane_stream() {
    let grid = stepf2::parse(b"XxYyZz1200\n0303030300\n0000000000\n0000000000").unwrap();
    assert_eq!(grid.players(), 4);
    assert!(grid.uses_stepf2());
    for owner in 0..4 {
        assert_eq!(
            grid.get(pos(owner, owner as u16 * 2, 0)).unwrap().kind,
            Kind::Tap
        );
        let hold = grid.get(pos(owner, owner as u16 * 2 + 1, 0)).unwrap();
        assert_eq!(hold.kind, Kind::Hold);
        assert_eq!(hold.end, Some(48));
    }
    assert_eq!(stepf2::parse(&grid.serialize().unwrap()).unwrap(), grid);
}

#[test]
fn compound_visibility_fake_reserved_and_keysound_extensions_round_trip() {
    let grid = stepf2::parse(
        b"{X|s|1|%}[7]{y|v|1|0}Z1000000\n0{3|h|0|_}[8]00000000\n0000000000\n0000000000",
    )
    .unwrap();
    let fake = grid.get(pos(0, 0, 0)).unwrap();
    assert_eq!(fake.kind, Kind::Fake);
    assert_eq!(&*fake.attributes, "{N|s|1|%}[7]");
    let hold = grid.get(pos(1, 1, 0)).unwrap();
    assert_eq!(&*hold.attributes, "{N|v|1|0}");
    assert_eq!(&*hold.tail_attributes, "{N|h|0|_}[8]");
    assert_eq!(stepf2::parse(&grid.serialize().unwrap()).unwrap(), grid);
    // Simple visibility symbols accepted by Fumen are normalized to a typed
    // decoration; the extra information is not discarded on editing.
    for (symbol, visibility) in [
        ('S', Visibility::Sudden),
        ('V', Visibility::Vanish),
        ('H', Visibility::Hidden),
    ] {
        let grid = stepf2::parse(format!("X{symbol}00000000").as_bytes()).unwrap();
        let note = grid.get(pos(0, 1, 0)).unwrap();
        assert_eq!(
            Attributes::read(&note.attributes)
                .unwrap()
                .unwrap()
                .0
                .visibility,
            visibility
        );
        assert_eq!(stepf2::parse(&grid.serialize().unwrap()).unwrap(), grid);
    }
}

#[test]
fn source_detection_does_not_reinterpret_standard_pump_or_other_layouts() {
    assert!(!stepf2::is_coop(b"1000000000 // X Y Z", "pump-routine"));
    assert!(!stepf2::is_coop(b"1[X]000000000", "pump-routine"));
    assert!(!stepf2::is_coop(b"X000000000", "dance-routine"));
    assert!(stepf2::is_coop(b"{X|n|0|0}000000000", "pump-double"));
    let grid = Grid::parse_layout(b"1000000000", "pump-routine").unwrap();
    assert!(!grid.uses_stepf2());
    assert!(grid.get(pos(0, 0, 0)).is_some());
}

#[test]
fn session_file_save_typed_inspector_and_exact_history_preserve_unedited_data() {
    for format in [Format::Sm, Format::Ssc] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("song.{}", format.extension()));
        let original = source(format, "X{Y|h|0|9}[7]00000000");
        std::fs::write(&path, &original).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        assert_eq!(file.session.grid(0).unwrap().players(), 2);
        assert_eq!(file.session.serialize().unwrap(), original.as_bytes());
        file.save().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original.as_bytes());
        let mut draft = Draft::capture(&mut file.session, 0, pos(1, 1, 0)).unwrap();
        let before = draft.stepf2_attributes(Field::Head).unwrap().unwrap();
        assert_eq!(before.visibility, Visibility::Hidden);
        assert_eq!(before.reserved, '9');
        draft.set_stepf2_attributes(Field::Head, before).unwrap();
        assert!(!draft.apply(&mut file.session, 0).unwrap());
        assert!(draft.stepf2_attributes(Field::Tail).unwrap().is_none());
        draft
            .set_stepf2_attributes(
                Field::Head,
                Attributes {
                    visibility: Visibility::Sudden,
                    fake: true,
                    ..before
                },
            )
            .unwrap();
        assert!(draft.apply(&mut file.session, 0).unwrap());
        let edited = file.session.serialize().unwrap();
        assert!(String::from_utf8_lossy(&edited).contains("{Y|s|1|9}[7]"));
        assert!(String::from_utf8_lossy(&edited).contains("#VENDOR:unchanged;"));
        assert!(String::from_utf8_lossy(&edited).contains("0001;"));
        file.session.undo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), original.as_bytes());
        file.session.redo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), edited);
        file.save().unwrap();
        let mut reopened = FileSession::open(&path).unwrap();
        assert_eq!(
            reopened.session.grid(0).unwrap(),
            file.session.grid(0).unwrap()
        );
        assert_eq!(std::fs::read(&path).unwrap(), edited);
    }
}

#[test]
fn player_reassignment_keeps_typed_decorations_and_undo_restores_source() {
    let original = source(Format::Ssc, "X{Y|v|1|0}[2]Z1000000");
    let mut session = Session::new(Document::parse(original.as_bytes(), Format::Ssc).unwrap());
    let note = session.grid(0).unwrap().get(pos(1, 1, 0)).unwrap().clone();
    session
        .edit_notes(
            0,
            "Reassign",
            [(pos(1, 1, 0), None), (pos(2, 1, 0), Some(note.clone()))],
        )
        .unwrap();
    let encoded = session.serialize().unwrap();
    assert!(String::from_utf8_lossy(&encoded).contains("{Z|v|1|0}[2]"));
    let mut reopened = Session::new(Document::parse(encoded.as_slice(), Format::Ssc).unwrap());
    assert_eq!(reopened.grid(0).unwrap().get(pos(2, 1, 0)), Some(&note));
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), original.as_bytes());
}

#[test]
fn incompatible_exports_fail_before_storage_writes_the_destination() {
    for (owner, kind, end) in [
        (0, Kind::Roll, Some(48)),
        (0, Kind::Lift, None),
        (1, Kind::Mine, None),
        (0, Kind::Keysound, None),
        (0, Kind::Attack, None),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.ssc");
        let original = source(Format::Ssc, "XY00000000");
        std::fs::write(&path, &original).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        file.session
            .edit_notes(
                0,
                "Incompatible note",
                [(
                    pos(owner, 4, 0),
                    Some(Note {
                        kind,
                        end,
                        attributes: "".into(),
                        tail_attributes: "".into(),
                    }),
                )],
            )
            .unwrap();
        assert!(file.save().is_err(), "{owner:?} {kind:?}");
        assert_eq!(std::fs::read(&path).unwrap(), original.as_bytes());
        file.session.undo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), original.as_bytes());
    }
    let mut too_many = Grid::new(10, 5).unwrap();
    too_many.set(pos(4, 0, 0), Some(Note::tap())).unwrap();
    assert!(stepf2::serialize(&too_many).is_err());
}

#[test]
fn interparticipant_lane_collisions_and_ambiguous_player_four_exports_are_rejected() {
    let mut grid = stepf2::parse(b"xY00000000\n3000000000\n0000000000\n0000000000").unwrap();
    grid.set(pos(1, 0, 24), Some(Note::tap())).unwrap();
    assert!(grid.serialize().is_err());
    let mut grid = Grid::new(10, 4).unwrap();
    grid.set(pos(3, 0, 0), Some(Note::tap())).unwrap();
    assert!(stepf2::serialize(&grid).is_err());
}

#[test]
fn malformed_sources_do_not_become_partially_editable_charts() {
    for text in [
        "X00000000",
        "x000000000",
        "3000000000",
        "x000000000\nY000000000",
        "{X|s|1}000000000",
        "{X|q|0|0}000000000",
        "{X|n|2|0}000000000",
        "X000000000&Y000000000",
        "X[7]000000000\n0000000000\n3000000000",
    ] {
        assert!(stepf2::parse(text.as_bytes()).is_err(), "{text}");
    }
    let mut session = Session::new(
        Document::parse(source(Format::Ssc, "x000000000").as_bytes(), Format::Ssc).unwrap(),
    );
    assert!(session.grid(0).is_err());
    assert!(session.loaded_grid(0).is_none());
}

#[test]
fn typed_stepf2_fields_are_unavailable_for_standard_notes() {
    let mut session = Session::new(
        Document::parse(source(Format::Ssc, "1000000000").as_bytes(), Format::Ssc).unwrap(),
    );
    let mut draft = Draft::capture(&mut session, 0, pos(0, 0, 0)).unwrap();
    assert_eq!(draft.stepf2_attributes(Field::Head).unwrap(), None);
    assert!(
        draft
            .set_stepf2_attributes(Field::Head, Attributes::default())
            .is_err()
    );
}

#[test]
fn cross_notation_edits_fail_atomically_and_transient_grids_cannot_export_decorations() {
    let decorated =
        stepf2::parse(b"{X|s|1|0}y00000000\n0{3|h|0|0}00000000\n0000000000\n0000000000").unwrap();
    let fake = decorated.get(pos(0, 0, 0)).unwrap().clone();
    let mut hold = decorated.get(pos(1, 1, 0)).unwrap().clone();
    // Verify a tail decoration alone cannot escape through a standard chart.
    hold.attributes = "".into();
    for note in [fake, hold] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("song.ssc");
        let original = source(Format::Ssc, "1000000000");
        std::fs::write(&path, &original).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        file.session
            .edit_notes(0, "Prior gesture", [(pos(0, 9, 0), Some(Note::tap()))])
            .unwrap();
        file.session.undo().unwrap();
        let revision = file.session.revision();
        let redo = file.session.redo_label().map(str::to_owned);
        assert!(
            file.session
                .edit_notes(
                    0,
                    "Paste decorated StepF2 note",
                    [(pos(0, 0, 0), None), (pos(0, 1, 0), Some(note.clone())),]
                )
                .is_err()
        );
        assert_eq!(file.session.revision(), revision);
        assert_eq!(file.session.redo_label(), redo.as_deref());
        assert_eq!(file.session.serialize().unwrap(), original.as_bytes());
        file.save().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original.as_bytes());
        // Internal projections may retain the decoration temporarily, but
        // canonical serialization must refuse false standard-file semantics.
        let mut projected = Grid::new(10, 1).unwrap();
        projected.set(pos(0, 1, 0), Some(note)).unwrap();
        assert!(projected.serialize().is_err());
    }
}

#[test]
fn compound_special_symbols_merge_normal_visibility_and_refuse_distinct_flags() {
    for (symbol, visibility, conflicting) in [
        ('S', Visibility::Sudden, 'h'),
        ('V', Visibility::Vanish, 's'),
        ('H', Visibility::Hidden, 'v'),
    ] {
        let grid = stepf2::parse(format!("X{{{symbol}|n|0|9}}[7]00000000").as_bytes()).unwrap();
        let note = grid.get(pos(0, 1, 0)).unwrap();
        let (attributes, trailing) = Attributes::read(&note.attributes).unwrap().unwrap();
        assert_eq!(attributes.visibility, visibility);
        assert_eq!(attributes.reserved, '9');
        assert_eq!(trailing, "[7]");
        assert_eq!(stepf2::parse(&grid.serialize().unwrap()).unwrap(), grid);
        let original = source(
            Format::Ssc,
            &format!("X{{{symbol}|{conflicting}|0|0}}00000000"),
        );
        let mut session = Session::new(Document::parse(original.as_bytes(), Format::Ssc).unwrap());
        assert!(session.grid(0).is_err());
        assert!(session.loaded_grid(0).is_none());
        assert_eq!(session.serialize().unwrap(), original.as_bytes());
    }
}

#[test]
fn source_layout_minimum_participants_survive_coop_decode_edit_save_and_reopen() {
    for (layout, minimum) in [("pump-double", 1), ("pump-routine", 2)] {
        for notes in [
            "XSVH000000",
            "X000000000",
            "x000000000\n3000000000\n0000000000\n0000000000",
        ] {
            let grid = Grid::parse_layout(notes.as_bytes(), layout).unwrap();
            assert!(grid.uses_stepf2());
            assert_eq!(grid.players(), minimum, "{layout} {notes}");
            assert_eq!(
                Grid::parse_layout(&grid.serialize().unwrap(), layout).unwrap(),
                grid,
            );
        }
        for format in [Format::Sm, Format::Ssc] {
            let original = source(format, "XSVH000000").replace("pump-routine", layout);
            let directory = tempfile::tempdir().unwrap();
            let path = directory
                .path()
                .join(format!("song.{}", format.extension()));
            std::fs::write(&path, &original).unwrap();
            let mut file = FileSession::open(&path).unwrap();
            assert_eq!(file.session.grid(0).unwrap().players(), minimum);
            assert_eq!(file.session.serialize().unwrap(), original.as_bytes());
            let mut draft = Draft::capture(&mut file.session, 0, pos(0, 0, 0)).unwrap();
            draft
                .set_stepf2_attributes(
                    Field::Head,
                    Attributes {
                        visibility: Visibility::Hidden,
                        ..Attributes::default()
                    },
                )
                .unwrap();
            assert!(draft.apply(&mut file.session, 0).unwrap());
            let edited = file.session.serialize().unwrap();
            file.save().unwrap();
            let mut reopened = FileSession::open(&path).unwrap();
            assert_eq!(reopened.session.document().charts()[0].steps_type, layout);
            assert_eq!(reopened.session.grid(0).unwrap().players(), minimum);
            assert_eq!(
                reopened.session.grid(0).unwrap(),
                file.session.grid(0).unwrap()
            );
            assert_eq!(std::fs::read(&path).unwrap(), edited);
            file.session.undo().unwrap();
            assert_eq!(file.session.serialize().unwrap(), original.as_bytes());
        }
    }
    // Owners still raise the count above either layout's starting minimum.
    for layout in ["pump-double", "pump-routine"] {
        assert_eq!(
            Grid::parse_layout(b"X1Y0000000", layout).unwrap().players(),
            4
        );
    }
    assert!(stepf2::parse_for_layout(b"X000000000", "dance-double").is_err());
}

#[test]
fn invalid_raw_stepf2_suffixes_fail_before_history_and_can_be_corrected() {
    for format in [Format::Sm, Format::Ssc] {
        for field in [Field::Head, Field::Tail] {
            for invalid in ["{N|q|0|0}", "{N|n|2|0}", "{N|s|0}", "{N|n|0|0}{N|h|0|0}"] {
                let original = source(format, "xY00000000\n3000000000\n0000000000\n0000000000");
                let mut session =
                    Session::new(Document::parse(original.as_bytes(), format).unwrap());
                session
                    .edit_notes(0, "Prior gesture", [(pos(0, 9, 0), Some(Note::tap()))])
                    .unwrap();
                session.undo().unwrap();
                let revision = session.revision();
                let undo = session.undo_label().map(str::to_owned);
                let redo = session.redo_label().map(str::to_owned);
                let note = session.grid(0).unwrap().get(pos(0, 0, 0)).unwrap().clone();
                let mut draft = Draft::capture(&mut session, 0, pos(0, 0, 0)).unwrap();
                // Partial/invalid raw text remains editable in the isolated
                // draft, but must never reach song history or source bytes.
                draft.set_text(field, invalid).unwrap();
                assert!(draft.apply(&mut session, 0).is_err(), "{field:?} {invalid}");
                assert_eq!(session.revision(), revision);
                assert_eq!(session.undo_label(), undo.as_deref());
                assert_eq!(session.redo_label(), redo.as_deref());
                assert_eq!(session.grid(0).unwrap().get(pos(0, 0, 0)), Some(&note));
                assert_eq!(session.serialize().unwrap(), original.as_bytes());
                draft.set_text(field, "{N|s|0|9}[7]").unwrap();
                assert!(draft.apply(&mut session, 0).unwrap());
                let edited = session.serialize().unwrap();
                let mut reopened =
                    Session::new(Document::parse(edited.as_slice(), format).unwrap());
                assert_eq!(reopened.grid(0).unwrap(), session.grid(0).unwrap());
                session.undo().unwrap();
                assert_eq!(session.serialize().unwrap(), original.as_bytes());
                session.redo().unwrap();
                assert_eq!(session.serialize().unwrap(), edited);
            }
        }
    }
}

#[test]
fn raw_fake_toggles_keep_note_kind_typed_fields_and_saved_wire_consistent() {
    for format in [Format::Sm, Format::Ssc] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join(format!("song.{}", format.extension()));
        let original = source(format, "XY00000000");
        std::fs::write(&path, &original).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        let mut saved_states = Vec::new();
        for (fake, visibility, suffix, expected_wire) in [
            (true, Visibility::Sudden, "{N|s|1|9}[7]", "{X|s|1|9}[7]"),
            (false, Visibility::Hidden, "{N|h|0|9}[7]", "{X|h|0|9}[7]"),
        ] {
            let mut draft = Draft::capture(&mut file.session, 0, pos(0, 0, 0)).unwrap();
            let previous_kind = draft.note().kind;
            draft.set_text(Field::Head, "{N|s|").unwrap();
            assert_eq!(draft.text(Field::Head), "{N|s|");
            assert_eq!(draft.note().kind, previous_kind);
            assert!(draft.apply(&mut file.session, 0).is_err());
            draft.set_text(Field::Head, suffix).unwrap();
            assert_eq!(draft.note().kind, if fake { Kind::Fake } else { Kind::Tap });
            assert_eq!(
                draft.stepf2_attributes(Field::Head).unwrap(),
                Some(Attributes {
                    visibility,
                    fake,
                    reserved: '9'
                })
            );
            assert!(draft.apply(&mut file.session, 0).unwrap());
            let edited = file.session.serialize().unwrap();
            assert!(String::from_utf8_lossy(&edited).contains(expected_wire));
            file.save().unwrap();
            let mut reopened = FileSession::open(&path).unwrap();
            assert_eq!(
                reopened.session.grid(0).unwrap(),
                file.session.grid(0).unwrap()
            );
            assert_eq!(std::fs::read(&path).unwrap(), edited);
            let readback = Draft::capture(&mut reopened.session, 0, pos(0, 0, 0)).unwrap();
            assert_eq!(
                readback
                    .stepf2_attributes(Field::Head)
                    .unwrap()
                    .unwrap()
                    .fake,
                fake
            );
            saved_states.push(edited);
        }
        file.session.undo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), saved_states[0]);
        file.session.undo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), original.as_bytes());
        file.session.redo().unwrap();
        file.session.redo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), saved_states[1]);
    }
}

#[test]
fn typed_default_fake_without_compound_reflects_the_note_kind() {
    let original = source(Format::Ssc, "XY00000000");
    let mut session = Session::new(Document::parse(original.as_bytes(), Format::Ssc).unwrap());
    session
        .edit_notes(
            0,
            "Fake without compound",
            [(
                pos(0, 0, 0),
                Some(Note {
                    kind: Kind::Fake,
                    end: None,
                    attributes: "[7]".into(),
                    tail_attributes: "".into(),
                }),
            )],
        )
        .unwrap();
    let mut draft = Draft::capture(&mut session, 0, pos(0, 0, 0)).unwrap();
    let attributes = draft.stepf2_attributes(Field::Head).unwrap().unwrap();
    assert!(attributes.fake);
    assert_eq!(attributes.visibility, Visibility::Normal);
    assert_eq!(attributes.reserved, '0');
    draft
        .set_stepf2_attributes(Field::Head, attributes)
        .unwrap();
    assert_eq!(draft.text(Field::Head), "[7]");
    assert!(!draft.apply(&mut session, 0).unwrap());
    draft
        .set_stepf2_attributes(
            Field::Head,
            Attributes {
                fake: false,
                ..attributes
            },
        )
        .unwrap();
    assert_eq!(draft.note().kind, Kind::Tap);
    assert_eq!(draft.text(Field::Head), "{N|n|0|0}[7]");
    assert!(draft.apply(&mut session, 0).unwrap());
}

#[test]
fn externally_inconsistent_fake_flags_are_rejected_instead_of_rewritten() {
    for (kind, suffix) in [(Kind::Tap, "{N|s|1|0}[7]"), (Kind::Fake, "{N|h|0|9}[7]")] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("song.ssc");
        let original = source(Format::Ssc, "XY00000000");
        std::fs::write(&path, &original).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        file.session
            .edit_notes(
                0,
                "External inconsistent note",
                [(
                    pos(0, 0, 0),
                    Some(Note {
                        kind,
                        end: None,
                        attributes: suffix.into(),
                        tail_attributes: "".into(),
                    }),
                )],
            )
            .unwrap();
        let revision = file.session.revision();
        let label = file.session.undo_label().map(str::to_owned);
        let draft = Draft::capture(&mut file.session, 0, pos(0, 0, 0)).unwrap();
        assert!(draft.apply(&mut file.session, 0).is_err());
        assert_eq!(file.session.revision(), revision);
        assert_eq!(file.session.undo_label(), label.as_deref());
        assert!(file.save().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original.as_bytes());
        assert_eq!(
            &*file
                .session
                .grid(0)
                .unwrap()
                .get(pos(0, 0, 0))
                .unwrap()
                .attributes,
            suffix
        );
        file.session.undo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), original.as_bytes());
    }
}
