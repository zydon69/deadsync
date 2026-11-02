#[path = "support/source_output_context.rs"]
mod source_output_context;
use deadsync_editor::{
    document::{Document, Format},
    generation::metadata::ChartData,
    history::Session,
    notes::{Kind, Note, Position},
    storage::FileSession,
    workspace::{Command, Workspace},
};

fn p(player: u32, lane: u16, row: u64) -> Position {
    Position { player, lane, row }
}
fn source(format: Format, layout: &str, notes: &str) -> String {
    match format {
        Format::Sm => format!(
            "// untouched header\n#TITLE:Keep;#VENDOR:unknown;#BPMS:0=120;#NOTES:\n // before pump-double\n {layout} // after pump-double\n :desc:Hard:9:0,0,0,0,0:{notes};\n#NOTES:dance-single::Easy:2:0,0,0,0,0:0001;"
        ),
        Format::Ssc => format!(
            "// untouched header\n#VERSION:0.83;#TITLE:Keep;#VENDOR:unknown;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:\n // before pump-double\n {layout} // after pump-double\n ;#DIFFICULTY:Hard;#METER:9;#CUSTOM:keep;#NOTES:{notes};\n#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Easy;#METER:2;#NOTES:0001;"
        ),
    }
}

#[test]
fn explicit_normalization_updates_live_layout_preserves_flags_peers_and_exact_history() {
    for format in [Format::Sm, Format::Ssc] {
        for notes in ["{X|s|1|%}[7]000000000", "XY{Z|h|0|_}[8]1000000"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(format!("song.{}", format.extension()));
            let original = source(format, "pump-double", notes);
            std::fs::write(&path, &original).unwrap();
            let mut file = FileSession::open(&path).unwrap();
            let grid = file.session.grid(0).unwrap().clone();
            assert!(grid.uses_stepf2());
            assert!(!file.session.is_dirty());
            assert_eq!(file.session.serialize().unwrap(), original.as_bytes());
            file.session
                .edit_notes(1, "Peer edit", [(p(0, 0, 48), Some(Note::tap()))])
                .unwrap();
            file.session
                .set_text(None, "TITLE", "Pending title")
                .unwrap();
            let before = file.session.serialize().unwrap();
            let ids = file.session.chart_ids().to_vec();
            assert!(file.session.normalize_stepf2_chart(0).unwrap());
            assert_eq!(
                file.session.document().charts()[0].steps_type,
                "pump-routine"
            );
            assert_eq!(file.session.chart_ids(), ids);
            let expected_players = grid.players().max(2);
            let after_grid = file.session.grid(0).unwrap().clone();
            assert_eq!(after_grid.players(), expected_players);
            assert_eq!(
                after_grid.iter().collect::<Vec<_>>(),
                grid.iter().collect::<Vec<_>>()
            );
            assert_eq!(
                file.session.grid(1).unwrap().get(p(0, 0, 48)).unwrap().kind,
                Kind::Tap
            );
            let after = file.session.serialize().unwrap();
            assert!(String::from_utf8_lossy(&after).contains("// before pump-double"));
            assert!(String::from_utf8_lossy(&after).contains("pump-routine // after pump-double"));
            let revision = file.session.revision();
            assert!(!file.session.normalize_stepf2_chart(0).unwrap());
            assert_eq!(file.session.revision(), revision);
            file.session.undo().unwrap();
            assert_eq!(
                file.session.document().charts()[0].steps_type,
                "pump-double"
            );
            assert_eq!(file.session.grid(0).unwrap(), &grid);
            assert_eq!(file.session.serialize().unwrap(), before);
            file.session.redo().unwrap();
            assert_eq!(file.session.serialize().unwrap(), after);
            file.save().unwrap();
            let mut reopened = FileSession::open(&path).unwrap();
            assert_eq!(
                reopened.session.document().charts()[0].steps_type,
                "pump-routine"
            );
            assert_eq!(reopened.session.grid(0).unwrap(), &after_grid);
            assert_eq!(std::fs::read(&path).unwrap(), after);
        }
    }
}

#[test]
fn routine_defaults_to_two_but_explicit_metadata_can_choose_one_without_losing_owners() {
    for format in [Format::Sm, Format::Ssc] {
        for notes in ["1000000000", "X000000000", "XYZ1000000"] {
            let original = source(format, "pump-routine", notes);
            let mut s = Session::new(Document::parse(original.as_bytes(), format).unwrap());
            let expected = if notes.starts_with("XYZ") { 4 } else { 2 };
            assert_eq!(s.grid(0).unwrap().players(), expected);
            assert_eq!(s.serialize().unwrap(), original.as_bytes());
            let initial = ChartData::existing_or_initial(s.document(), 0).unwrap();
            assert_eq!(initial.max_player_index + 1, expected);
            initial.save(&mut s, 0).unwrap();
            let bytes = s.serialize().unwrap();
            let mut reopened = Session::new(Document::parse(bytes, format).unwrap());
            assert_eq!(reopened.grid(0).unwrap().players(), expected);
            reopened.set_participants(0, 1).unwrap();
            let expected = if notes.starts_with("XYZ") { 4 } else { 1 };
            assert_eq!(reopened.grid(0).unwrap().players(), expected);
            let mut final_session =
                Session::new(Document::parse(reopened.serialize().unwrap(), format).unwrap());
            assert_eq!(final_session.grid(0).unwrap().players(), expected);
        }
    }
}

#[test]
fn normalization_honors_explicit_one_player_metadata_and_rejects_unknown_metadata_atomically() {
    for format in [Format::Sm, Format::Ssc] {
        let original = source(format, "pump-double", "X000000000");
        let mut s = Session::new(Document::parse(original.as_bytes(), format).unwrap());
        ChartData::default().save(&mut s, 0).unwrap();
        assert!(s.normalize_stepf2_chart(0).unwrap());
        assert_eq!(s.grid(0).unwrap().players(), 1);
        let mut reopened = Session::new(Document::parse(s.serialize().unwrap(), format).unwrap());
        assert_eq!(reopened.grid(0).unwrap().players(), 1);

        let mut s = Session::new(Document::parse(original.as_bytes(), format).unwrap());
        let (scope, name) = if format == Format::Ssc {
            (Some(0), "GAChartDataVersion")
        } else {
            (None, "GAChartDataVersionCHART0000")
        };
        s.set_text(scope, name, "999").unwrap();
        let before = s.serialize().unwrap();
        let revision = s.revision();
        assert!(s.normalize_stepf2_chart(0).is_err());
        assert_eq!(s.revision(), revision);
        assert_eq!(s.serialize().unwrap(), before);
        assert_eq!(s.document().charts()[0].steps_type, "pump-double");
    }
}

#[test]
fn workspace_normalization_preserves_cursor_focus_and_clamps_participant_after_undo() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let original = source(Format::Ssc, "pump-double", "X000000000");
    std::fs::write(&path, &original).unwrap();
    let mut w = Workspace::open(&path, 0).unwrap();
    assert!(w.can_normalize_stepf2_chart());
    w.execute(Command::Seek(48)).unwrap();
    w.execute(Command::ToggleSelection).unwrap();
    w.execute(Command::NormalizeStepF2).unwrap();
    assert_eq!(w.chart(), 0);
    assert_eq!(w.row(), 48);
    assert_eq!(w.selection(), Some((48, 48)));
    assert_eq!(w.grid().players(), 2);
    assert!(!w.can_normalize_stepf2_chart());
    w.execute(Command::SelectPlayer(1)).unwrap();
    w.execute(Command::Undo).unwrap();
    assert_eq!(w.player(), 0);
    assert_eq!(w.grid().players(), 1);
    assert_eq!(w.file().session.serialize().unwrap(), original.as_bytes());
    w.execute(Command::Redo).unwrap();
    assert_eq!(w.grid().players(), 2);
}

#[test]
fn standard_double_and_already_routine_normalization_are_noops() {
    for layout in ["pump-double", "pump-routine"] {
        let original = source(Format::Ssc, layout, "1000000000");
        let mut s = Session::new(Document::parse(original.as_bytes(), Format::Ssc).unwrap());
        assert!(!s.normalize_stepf2_chart(0).unwrap());
        assert_eq!(s.serialize().unwrap(), original.as_bytes());
        assert!(s.undo_label().is_none());
    }
}

#[test]
fn nonleading_chart_normalization_and_undo_after_save_preserve_identity_and_both_peers() {
    for format in [Format::Sm, Format::Ssc] {
        let leading = match format {
            Format::Sm => "#NOTES:dance-single:Leading:Easy:3::1000;",
            Format::Ssc => {
                "#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Leading;#DIFFICULTY:Easy;#METER:3;#NOTES:1000;"
            }
        };
        let original = source(format, "pump-double", "X000000000").replacen(
            "#BPMS:0=120;",
            &format!("#BPMS:0=120;{leading}"),
            1,
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("song.{}", format.extension()));
        std::fs::write(&path, &original).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        assert_eq!(file.session.document().charts().len(), 3);
        for (chart, row) in [(0, 48), (2, 96)] {
            file.session
                .edit_notes(chart, "Peer edit", [(p(0, 1, row), Some(Note::tap()))])
                .unwrap();
        }
        let before = file.session.serialize().unwrap();
        let ids = file.session.chart_ids().to_vec();
        let before_grids: Vec<_> = (0..3)
            .map(|chart| file.session.grid(chart).unwrap().clone())
            .collect();
        assert!(file.session.normalize_stepf2_chart(1).unwrap());
        assert_eq!(file.session.chart_ids(), ids);
        assert_eq!(
            file.session.document().charts()[1].steps_type,
            "pump-routine"
        );
        for chart in [0, 2] {
            assert_eq!(file.session.grid(chart).unwrap(), &before_grids[chart]);
        }
        let after = file.session.serialize().unwrap();
        file.save().unwrap();
        assert_eq!(file.session.serialize().unwrap(), after);
        let published_after = std::fs::read(&path).unwrap();
        // Initial source load chooses before sorting (EditorSong777-789).
        // Native lossless output retains physical order: leading source0 is
        // per-type index0. Source-compatible sorted output would use index1.
        assert_eq!(file.session.song_timing_chart().unwrap(), Some(0));
        let context = source_output_context::Context { sync_offset: 0.009, chart_type: "dance_single", chart_index: 0 };
        source_output_context::assert_context(&after, &published_after, format, context);
        assert!(!file.session.is_dirty());
        file.session.undo().unwrap();
        assert!(file.session.is_dirty());
        assert_eq!(file.session.chart_ids(), ids);
        assert_eq!(
            file.session.document().charts()[1].steps_type,
            "pump-double"
        );
        assert_eq!(file.session.serialize().unwrap(), before);
        for chart in 0..3 {
            assert_eq!(file.session.grid(chart).unwrap(), &before_grids[chart]);
        }
        // Undo is in memory until Save. The previously saved file is untouched.
        assert_eq!(std::fs::read(&path).unwrap(), published_after);
        file.session.redo().unwrap();
        assert!(!file.session.is_dirty());
        assert_eq!(file.session.serialize().unwrap(), after);
        file.session.undo().unwrap();
        file.save().unwrap();
        let mut reopened = FileSession::open(&path).unwrap();
        assert_eq!(
            reopened.session.document().charts()[1].steps_type,
            "pump-double"
        );
        for chart in 0..3 {
            assert_eq!(reopened.session.grid(chart).unwrap(), &before_grids[chart]);
        }
        let published_before = std::fs::read(&path).unwrap();
        source_output_context::assert_context(&before, &published_before, format, context);
        assert_eq!(reopened.session.serialize().unwrap(), published_before);
    }
}

#[test]
fn duplicate_ssc_layout_tags_keep_last_value_policy_and_preserve_earlier_tags() {
    // Fumen ChartTypePropertyParser::Parse assigns Type and player defaults on
    // each STEPSTYPE. Before NOTES, the last layout therefore wins. Do not
    // invent a blanket duplicate-tag rejection for these supported sources.
    for earlier in ["dance-single", "pump-double", "pump-routine"] {
        let original = format!(
            "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:{earlier};#STEPSTYPE:pump-double;#DIFFICULTY:Hard;#METER:9;#NOTES:X000000000;"
        );
        let mut session = Session::new(Document::parse(original.as_bytes(), Format::Ssc).unwrap());
        assert_eq!(session.document().charts()[0].steps_type, "pump-double");
        assert_eq!(session.grid(0).unwrap().players(), 1);
        assert!(session.normalize_stepf2_chart(0).unwrap());
        let tags: Vec<_> = session
            .document()
            .tags()
            .iter()
            .filter(|tag| tag.chart == Some(0) && tag.name == "STEPSTYPE")
            .map(|tag| {
                String::from_utf8(session.document().value(tag.value.clone()).to_vec()).unwrap()
            })
            .collect();
        assert_eq!(tags, [earlier, "pump-routine"]);
        let after = session.serialize().unwrap();
        let mut reopened = Session::new(Document::parse(after.clone(), Format::Ssc).unwrap());
        assert_eq!(reopened.document().charts()[0].steps_type, "pump-routine");
        assert_eq!(reopened.grid(0).unwrap().players(), 2);
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), original.as_bytes());
        session.redo().unwrap();
        assert_eq!(session.serialize().unwrap(), after);
    }
}

#[test]
fn pending_fourth_player_notes_survive_unrelated_offset_metadata_and_save_reopen() {
    use deadsync_editor::chart_properties::Draft;
    for format in [Format::Sm, Format::Ssc] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("song.{}", format.extension()));
        let original = source(format, "pump-routine", "X000000000");
        std::fs::write(&path, &original).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        file.session.set_participants(0, 4).unwrap();
        let fourth = p(3, 1, 48);
        file.session
            .edit_notes(0, "Pending fourth player", [(fourth, Some(Note::tap()))])
            .unwrap();
        let before_offset = file.session.serialize().unwrap();
        let mut draft = Draft::capture(&file.session, 0).unwrap();
        draft.set_music_offset(0.125).unwrap();
        draft.set_use_chart_offset(true);
        assert!(draft.apply(&mut file.session, 0).unwrap());
        let data = ChartData::read(file.session.document(), 0)
            .unwrap()
            .unwrap();
        assert_eq!(data.max_player_index, 3);
        assert_eq!(data.music_offset, 0.125);
        assert_eq!(file.session.grid(0).unwrap().players(), 4);
        assert_eq!(
            file.session.grid(0).unwrap().get(fourth).unwrap().kind,
            Kind::Tap
        );
        let after = file.session.serialize().unwrap();
        file.save().unwrap();
        let mut reopened = FileSession::open(&path).unwrap();
        assert_eq!(reopened.session.grid(0).unwrap().players(), 4);
        assert_eq!(
            reopened.session.grid(0).unwrap().get(fourth).unwrap().kind,
            Kind::Tap
        );
        assert_eq!(
            ChartData::read(reopened.session.document(), 0)
                .unwrap()
                .unwrap()
                .max_player_index,
            3
        );
        file.session.undo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), before_offset);
        assert_eq!(file.session.grid(0).unwrap().players(), 4);
        assert!(file.session.grid(0).unwrap().get(fourth).is_some());
        file.session.undo().unwrap();
        file.session.undo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), original.as_bytes());
        assert_eq!(file.session.grid(0).unwrap().players(), 2);
        for _ in 0..3 {
            file.session.redo().unwrap();
        }
        assert_eq!(file.session.serialize().unwrap(), after);
    }
}

#[test]
fn metadata_only_initialization_keeps_opaque_and_malformed_note_bytes_and_routine_default() {
    for format in [Format::Sm, Format::Ssc] {
        for note_text in ["Q000000000", "00000000000"] {
            let original = source(format, "pump-routine", note_text);
            let mut session = Session::new(Document::parse(original.as_bytes(), format).unwrap());
            if note_text.starts_with('Q') {
                let opaque = p(0, 0, 0);
                assert_eq!(session.grid(0).unwrap().players(), 2);
                assert!(session.grid(0).unwrap().get(opaque).is_none());
                assert!(
                    session
                        .edit_notes(0, "Opaque overwrite", [(opaque, Some(Note::tap()))])
                        .is_err()
                );
                assert_eq!(session.serialize().unwrap(), original.as_bytes());
                assert!(session.undo_label().is_none());
            } else {
                assert!(
                    session.grid(0).is_err(),
                    "eleven columns cannot fit pump-routine"
                );
            }
            let notes = session
                .document()
                .value(session.document().charts()[0].notes.clone())
                .to_vec();
            let mut metadata = ChartData::existing_or_initial(session.document(), 0).unwrap();
            assert_eq!(metadata.max_player_index, 1);
            metadata.music_offset = 0.25;
            metadata.should_use_chart_music_offset = true;
            assert!(metadata.save(&mut session, 0).unwrap());
            let after = session.serialize().unwrap();
            let reopened = Session::new(Document::parse(after.clone(), format).unwrap());
            assert_eq!(
                reopened
                    .document()
                    .value(reopened.document().charts()[0].notes.clone()),
                notes
            );
            assert_eq!(
                ChartData::read(reopened.document(), 0)
                    .unwrap()
                    .unwrap()
                    .max_player_index,
                1
            );
            session.undo().unwrap();
            assert_eq!(session.serialize().unwrap(), original.as_bytes());
            session.redo().unwrap();
            assert_eq!(session.serialize().unwrap(), after);
        }
    }
}
