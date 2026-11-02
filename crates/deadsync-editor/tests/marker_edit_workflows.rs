use deadsync_editor::{
    document::{Document, Format},
    generation::{
        marker_edit::Draft,
        metadata::{ChartData, Definition, PATTERN_BALANCED, Registry, SIXTEENTHS},
        pattern_job::Task,
    },
    history::Session,
    notes::{Note, Position},
    storage::FileSession,
};
use std::{
    collections::BTreeMap,
    fs,
    sync::{Arc, atomic::AtomicBool},
};
fn session(format: Format) -> Session {
    let mut bytes = b"#TITLE:Caf\xe9;#VENDOR:keep;#OFFSET:0.25;#BPMS:0=120;".to_vec();
    for _ in 0..2 {
        bytes.extend_from_slice(match format {
            Format::Sm => b"#NOTES:dance-single:source:Hard:9:0,0,0,0,0:",
            Format::Ssc => b"#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#OFFSET:-0.125;#NOTES:",
        });
        bytes.extend_from_slice(b"1000\n0100\n0010\n0001;");
    }
    let mut session = Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap());
    let marker = Definition {
        pattern_config_guid: SIXTEENTHS.parse().unwrap(),
        performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
        length: 96,
        random_seed: -7,
        start_position_inclusive: true,
        end_position_inclusive: true,
        extra: BTreeMap::from([("FutureMarker".into(), serde_json::json!({"keep":true}))]),
        ..Definition::default()
    };
    let mut data = ChartData::existing_or_initial(session.document(), 0).unwrap();
    data.extra
        .insert("FutureChart".into(), serde_json::json!([1, 2, 3]));
    data.patterns = Some(BTreeMap::from([(0, marker.clone()), (192, marker)]));
    data.save(&mut session, 0).unwrap();
    session
}
#[test]
fn marker_properties_move_save_reopen_regenerate_and_undo_without_touching_notes() {
    for format in [Format::Sm, Format::Ssc] {
        let mut source = session(format);
        if format == Format::Ssc {
            // Match the editor's conversion step for this unversioned fixture
            // before invoking timing-dependent regeneration.
            deadsync_editor::timing_edit::migration::Draft::capture(&source)
                .unwrap()
                .apply(&mut source)
                .unwrap();
        }
        source
            .edit_notes(
                1,
                "Other chart",
                [(
                    Position {
                        player: 0,
                        lane: 3,
                        row: 12,
                    },
                    Some(Note::tap()),
                )],
            )
            .unwrap();
        let before = source.serialize().unwrap();
        let grid = source.grid(0).unwrap().clone();
        let original = ChartData::read(source.document(), 0).unwrap().unwrap();
        let mut draft = Draft::at_cursor(&source, 0, 48).unwrap();
        draft.start = 48;
        draft.definition.length = 72;
        draft.definition.random_seed = i32::MIN;
        draft.definition.start_position_inclusive = false;
        draft.definition.end_position_inclusive = false;
        draft.definition.ignore_preceding_distribution = true;
        let revision = source.revision();
        assert!(draft.apply(&mut source, 0).unwrap());
        let after = source.serialize().unwrap();
        assert_eq!(source.grid(0).unwrap(), &grid);
        let data = ChartData::read(source.document(), 0).unwrap().unwrap();
        assert_eq!(data.patterns.as_ref().unwrap()[&48], draft.definition);
        assert_eq!(
            data.patterns.as_ref().unwrap()[&192],
            original.patterns.as_ref().unwrap()[&192]
        );
        assert_eq!(data.extra, original.extra);
        assert_eq!(data.music_offset, original.music_offset);
        assert_eq!(
            data.should_use_chart_music_offset,
            original.should_use_chart_music_offset
        );
        assert_eq!(data.expressed_chart_config, original.expressed_chart_config);
        source.undo().unwrap();
        assert_eq!(source.revision(), revision);
        assert_eq!(source.serialize().unwrap(), before);
        source.redo().unwrap();
        assert_eq!(source.serialize().unwrap(), after);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(if format == Format::Sm {
            "song.sm"
        } else {
            "song.ssc"
        });
        fs::write(&path, &before).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        let mut reopened_draft = Draft::at_cursor(&file.session, 0, 0).unwrap();
        reopened_draft.start = draft.start;
        reopened_draft.definition = draft.definition.clone();
        reopened_draft.apply(&mut file.session, 0).unwrap();
        assert_eq!(file.session.serialize().unwrap(), after);
        let output = file.session.serialize_for_output().unwrap();
        let peer = file.session.grid(1).unwrap().clone();
        file.save().unwrap();
        assert_eq!(fs::read(&path).unwrap(), output);
        let mut reopened = FileSession::open(&path).unwrap();
        let resident = Document::parse(after.as_slice(), format).unwrap();
        for tag in resident.tags() {
            assert_eq!(
                reopened
                    .session
                    .document()
                    .raw_property(tag.chart, &tag.name),
                resident.raw_property(tag.chart, &tag.name),
                "Resident field changed: {:?}/{}",
                tag.chart,
                tag.name
            );
        }
        assert_eq!(reopened.session.grid(0).unwrap(), &grid);
        assert_eq!(reopened.session.grid(1).unwrap(), &peer);
        assert_eq!(
            serde_json::to_value(
                ChartData::read(reopened.session.document(), 0)
                    .unwrap()
                    .unwrap()
            )
            .unwrap(),
            serde_json::to_value(&data).unwrap()
        );
        let preview = Task::capture_saved(&reopened.session, 0, 48, &Registry::builtin())
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .unwrap();
        preview.apply(&mut reopened.session, 0).unwrap();
        assert_eq!(reopened.session.grid(0).unwrap(), preview.grid());
        assert_eq!(
            ChartData::read(reopened.session.document(), 0)
                .unwrap()
                .unwrap()
                .patterns
                .unwrap()[&48],
            draft.definition
        );
    }
}
#[test]
fn deleting_marker_retains_notes_other_markers_and_unknown_metadata() {
    for format in [Format::Sm, Format::Ssc] {
        let mut source = session(format);
        let before = source.serialize().unwrap();
        let grid = source.grid(0).unwrap().clone();
        let mut draft = Draft::at_cursor(&source, 0, 0).unwrap();
        // Pending draft changes must not change which persisted marker is deleted.
        draft.start = 192;
        draft.definition.length = u64::MAX;
        draft.delete(&mut source, 0).unwrap();
        let data = ChartData::read(source.document(), 0).unwrap().unwrap();
        assert_eq!(
            data.patterns
                .as_ref()
                .unwrap()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            [192]
        );
        assert_eq!(data.extra["FutureChart"], serde_json::json!([1, 2, 3]));
        assert_eq!(source.grid(0).unwrap(), &grid);
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), before);
        source.redo().unwrap();
        Draft::at_cursor(&source, 0, 192)
            .unwrap()
            .delete(&mut source, 0)
            .unwrap();
        assert!(
            ChartData::read(source.document(), 0)
                .unwrap()
                .unwrap()
                .patterns
                .unwrap()
                .is_empty()
        );
        assert!(Draft::at_cursor(&source, 0, 0).is_err());
    }
}
#[test]
fn unchanged_discarded_foreign_stale_and_wrong_chart_drafts_preserve_history() {
    let mut source = session(Format::Ssc);
    let before = source.serialize().unwrap();
    let revision = source.revision();
    let draft = Draft::at_cursor(&source, 0, 0).unwrap();
    assert!(!draft.apply(&mut source, 0).unwrap());
    assert_eq!(source.revision(), revision);
    assert_eq!(source.serialize().unwrap(), before);
    assert!(draft.apply(&mut source, 1).is_err());
    assert!(draft.delete(&mut source, 1).is_err());
    let mut foreign = session(Format::Ssc);
    assert!(draft.delete(&mut foreign, 0).is_err());
    let mut discarded = Draft::at_cursor(&source, 0, 0).unwrap();
    discarded.start = 30;
    drop(discarded);
    assert_eq!(source.serialize().unwrap(), before);
    source
        .edit_notes(
            0,
            "Edit",
            [(
                Position {
                    player: 0,
                    lane: 2,
                    row: 24,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let after = source.serialize().unwrap();
    assert!(draft.apply(&mut source, 0).is_err());
    assert!(draft.delete(&mut source, 0).is_err());
    assert_eq!(source.serialize().unwrap(), after);
    source.undo().unwrap();
    assert!(!draft.apply(&mut source, 0).unwrap());
    source.redo().unwrap();
    assert_eq!(source.serialize().unwrap(), after);
}
#[test]
fn collisions_and_overflow_fail_atomically_without_clearing_redo() {
    let mut source = session(Format::Ssc);
    source
        .edit_notes(
            0,
            "Edit",
            [(
                Position {
                    player: 0,
                    lane: 2,
                    row: 24,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let after = source.serialize().unwrap();
    source.undo().unwrap();
    let before = source.serialize().unwrap();
    let mut draft = Draft::at_cursor(&source, 0, 0).unwrap();
    draft.start = 192;
    assert!(draft.apply(&mut source, 0).is_err());
    draft.start = i32::MAX as u64;
    assert!(draft.apply(&mut source, 0).is_err());
    draft.start = 0;
    draft.definition.length = u64::MAX;
    assert!(draft.apply(&mut source, 0).is_err());
    assert_eq!(source.serialize().unwrap(), before);
    source.redo().unwrap();
    assert_eq!(source.serialize().unwrap(), after);
}
#[test]
fn overlapping_markers_choose_latest_start_and_allow_zero_length_at_exact_row() {
    let mut source = session(Format::Ssc);
    let mut draft = Draft::at_cursor(&source, 0, 0).unwrap();
    draft.definition.length = 240;
    draft.apply(&mut source, 0).unwrap();
    assert_eq!(Draft::at_cursor(&source, 0, 192).unwrap().start, 192);
    assert_eq!(Draft::at_cursor(&source, 0, 241).unwrap().start, 192);
    assert!(Draft::at_cursor(&source, 0, 289).is_err());
    let mut draft = Draft::at_cursor(&source, 0, 192).unwrap();
    draft.definition.length = 0;
    draft.apply(&mut source, 0).unwrap();
    assert_eq!(Draft::at_cursor(&source, 0, 192).unwrap().start, 192);
    assert_eq!(Draft::at_cursor(&source, 0, 193).unwrap().start, 0);
}

#[test]
fn inspected_generation_admission_uses_captured_marker_and_rejects_pending_stale_foreign_or_deleted()
 {
    let mut source = session(Format::Sm);
    let before = source.serialize().unwrap();
    let mut draft = Draft::at_cursor(&source, 0, 192).unwrap();
    assert!(draft.generation_task(&source, 0, None).is_ok());
    assert!(draft.generation_task(&source, 1, None).is_err());
    assert_eq!(source.serialize().unwrap(), before);
    draft.start = 240;
    assert!(draft.generation_task(&source, 0, None).is_err());
    assert_eq!(source.serialize().unwrap(), before);
    draft.start = 192;
    source.set_text(None, "TITLE", "New revision").unwrap();
    let changed = source.serialize().unwrap();
    assert!(draft.generation_task(&source, 0, None).is_err());
    assert_eq!(source.serialize().unwrap(), changed);
    let fresh = Draft::at_cursor(&source, 0, 192).unwrap();
    fresh.delete(&mut source, 0).unwrap();
    let removed = source.serialize().unwrap();
    assert!(fresh.generation_task(&source, 0, None).is_err());
    assert_eq!(source.serialize().unwrap(), removed);
    let foreign = session(Format::Sm);
    assert!(fresh.generation_task(&foreign, 0, None).is_err());
}
