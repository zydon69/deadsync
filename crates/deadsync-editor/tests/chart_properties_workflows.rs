#[path = "support/source_output_context.rs"]
mod source_output_context;
use deadsync_editor::{
    chart_properties::Draft,
    document::{Document, Format},
    generation::{
        metadata::{ChartData, DYNAMIC, Guid},
        profile_catalog::Task,
        profiles::{Kind, Profile},
    },
    history::Session,
    notes::{Note, Position},
    storage::FileSession,
};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
fn source(format: Format) -> Session {
    let mut bytes = b"#VERSION:0.83;#TITLE:Caf\xe9;#VENDOR:keep;#OFFSET:0.25;#BPMS:0=120;".to_vec();
    for offset in ["-0.125", "-0.375"] {
        bytes.extend_from_slice(match format {
            Format::Sm => b"#NOTES:dance-single:source:Hard:9:0,0,0,0,0:",
            Format::Ssc => b"#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;",
        });
        if format == Format::Ssc {
            bytes.extend_from_slice(format!("#OFFSET:{offset};#NOTES:").as_bytes());
        }
        bytes.extend_from_slice(b"1000\n0100\n0010\n0001;");
    }
    Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap())
}
fn custom(folder: &std::path::Path) -> Profile {
    let profile=Profile::parse(Kind::Expressed,&serde_json::to_vec(&serde_json::json!({"Guid":"11111111-2222-3333-4444-555555555555","Name":"Custom","Config":deadsync_editor::generation::job::ChartOptions::balanced("dance-single").expressed})).unwrap()).unwrap();
    profile.export_new(folder).unwrap();
    profile
}
#[test]
fn properties_without_markers_are_read_only_until_applied_and_save_reopen_in_sm_and_ssc() {
    for format in [Format::Sm, Format::Ssc] {
        let mut source = source(format);
        source
            .edit_notes(
                1,
                "Other",
                [(
                    Position {
                        player: 0,
                        row: 12,
                        lane: 3,
                    },
                    Some(Note::tap()),
                )],
            )
            .unwrap();
        let before = source.serialize().unwrap();
        let notes = source.grid(0).unwrap().clone();
        let dir = tempfile::tempdir().unwrap();
        let profile = custom(dir.path());
        let mut draft = Draft::capture(&source, 0).unwrap();
        assert!(!draft.has_changes());
        assert!(!draft.apply(&mut source, 0).unwrap());
        let catalog = draft
            .profile_task(&source, 0, Some(dir.path()))
            .unwrap()
            .run(&AtomicBool::new(false))
            .unwrap();
        draft
            .select_expression(&source, 0, &catalog, profile.guid())
            .unwrap();
        assert_eq!(source.serialize().unwrap(), before);
        draft.apply(&mut source, 0).unwrap();
        let after = source.serialize().unwrap();
        assert_eq!(source.grid(0).unwrap(), &notes);
        let data = ChartData::read(source.document(), 0).unwrap().unwrap();
        assert_eq!(data.expressed_chart_config, *profile.guid());
        assert!(data.patterns.is_none());
        assert!(ChartData::read(source.document(), 1).unwrap().is_none());
        assert_eq!(
            source.document().text(None, "OFFSET").as_deref(),
            Some("0.25")
        );
        if format == Format::Ssc {
            assert_eq!(
                source.document().text(Some(0), "OFFSET").as_deref(),
                Some("-0.125")
            );
            assert_eq!(
                source.document().text(Some(1), "OFFSET").as_deref(),
                Some("-0.375")
            );
        }
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), before);
        source.redo().unwrap();
        assert_eq!(source.serialize().unwrap(), after);
        let path = dir.path().join(if format == Format::Sm {
            "song.sm"
        } else {
            "song.ssc"
        });
        fs::write(&path, &before).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        let mut draft = Draft::capture(&file.session, 0).unwrap();
        let catalog = draft
            .profile_task(&file.session, 0, Some(dir.path()))
            .unwrap()
            .run(&AtomicBool::new(false))
            .unwrap();
        draft
            .select_expression(&file.session, 0, &catalog, profile.guid())
            .unwrap();
        draft.apply(&mut file.session, 0).unwrap();
        assert_eq!(file.session.serialize().unwrap(), after);
        file.save().unwrap();
        assert_eq!(file.session.serialize().unwrap(), after);
        let published = fs::read(&path).unwrap();
        source_output_context::assert_context_only(&after, &published, format);
        let mut reopened = FileSession::open(&path).unwrap();
        assert_eq!(reopened.session.serialize().unwrap(), published);
        assert_eq!(reopened.session.grid(0).unwrap(), &notes);
        assert_eq!(reopened.session.grid(1).unwrap(), source.grid(1).unwrap());
        assert_eq!(
            Draft::capture(&reopened.session, 0).unwrap().expression(),
            profile.guid()
        );
    }
}
#[test]
fn ssc_offset_switch_and_value_export_with_metadata_in_one_undo_gesture() {
    let mut source = source(Format::Ssc);
    let before = source.serialize().unwrap();
    let mut draft = Draft::capture(&source, 0).unwrap();
    assert!(draft.uses_chart_offset());
    assert_eq!(draft.music_offset(), -0.125);
    assert_eq!(draft.effective_music_offset(), -0.125);
    draft.set_use_chart_offset(false);
    assert_eq!(draft.effective_music_offset(), 0.25);
    draft.apply(&mut source, 0).unwrap();
    let inherited = source.serialize().unwrap();
    assert_eq!(
        source.document().text(Some(0), "OFFSET").as_deref(),
        Some("0.25")
    );
    let data = ChartData::read(source.document(), 0).unwrap().unwrap();
    assert!(!data.should_use_chart_music_offset);
    assert_eq!(data.music_offset, -0.125);
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
    source.redo().unwrap();
    assert_eq!(source.serialize().unwrap(), inherited);
    let mut draft = Draft::capture(&source, 0).unwrap();
    draft.set_use_chart_offset(true);
    draft.set_music_offset(-0.2).unwrap();
    draft.apply(&mut source, 0).unwrap();
    assert_eq!(
        source.document().text(Some(0), "OFFSET").as_deref(),
        Some("-0.2")
    );
    assert_eq!(
        source.document().text(Some(1), "OFFSET").as_deref(),
        Some("-0.375")
    );
    assert_eq!(
        source.document().text(None, "OFFSET").as_deref(),
        Some("0.25")
    );
    let data = ChartData::read(source.document(), 0).unwrap().unwrap();
    assert!(data.should_use_chart_music_offset);
    assert_eq!(data.music_offset, -0.2);
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), inherited);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, &inherited).unwrap();
    let file = FileSession::open(&path).unwrap();
    let draft = Draft::capture(&file.session, 0).unwrap();
    assert!(!draft.uses_chart_offset());
    assert_eq!(draft.music_offset(), -0.125);
    assert_eq!(draft.effective_music_offset(), 0.25);
}
#[test]
fn sm_chart_offset_stays_in_editor_metadata_without_changing_song_or_other_charts() {
    let mut source = source(Format::Sm);
    let before = source.serialize().unwrap();
    let mut draft = Draft::capture(&source, 0).unwrap();
    assert_eq!(draft.effective_music_offset(), 0.25);
    draft.set_music_offset(-0.1).unwrap();
    draft.set_use_chart_offset(true);
    draft.apply(&mut source, 0).unwrap();
    let after = source.serialize().unwrap();
    assert_eq!(
        source.document().text(None, "OFFSET").as_deref(),
        Some("0.25")
    );
    assert!(source.document().text(Some(0), "OFFSET").is_none());
    assert_eq!(
        Draft::capture(&source, 0).unwrap().effective_music_offset(),
        -0.1
    );
    assert_eq!(
        Draft::capture(&source, 1).unwrap().effective_music_offset(),
        0.25
    );
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
    source.redo().unwrap();
    assert_eq!(source.serialize().unwrap(), after);
}
#[test]
fn offset_setter_follows_one_ulp_comparison_and_rejects_nonfinite_values() {
    for value in [1.0, -1.0, 0.0, -0.0] {
        let mut source = source(Format::Ssc);
        let mut data = ChartData::existing_or_initial(source.document(), 0).unwrap();
        data.music_offset = value;
        data.should_use_chart_music_offset = true;
        data.save(&mut source, 0).unwrap();
        let before = source.serialize().unwrap();
        let mut draft = Draft::capture(&source, 0).unwrap();
        if value != 0.0 {
            draft
                .set_music_offset(f64::from_bits(value.to_bits() + 1))
                .unwrap();
            assert!(!draft.has_changes());
            draft
                .set_music_offset(f64::from_bits(value.to_bits() + 2))
                .unwrap();
            assert!(draft.has_changes());
        } else {
            draft.set_music_offset(-value).unwrap();
            assert!(!draft.has_changes());
        }
        let current = draft.music_offset();
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(draft.set_music_offset(invalid).is_err());
            assert_eq!(draft.music_offset(), current);
        }
        assert_eq!(source.serialize().unwrap(), before);
    }
}
#[test]
fn chart_properties_keep_unknown_metadata_and_do_not_rewrite_offset_for_expression_only_edits() {
    let mut source = source(Format::Ssc);
    let mut data = ChartData::existing_or_initial(source.document(), 0).unwrap();
    data.music_offset = -0.2;
    data.extra
        .insert("Future".into(), serde_json::json!({"keep":true}));
    data.save(&mut source, 0).unwrap();
    let mut draft = Draft::capture(&source, 0).unwrap();
    assert_eq!(draft.effective_music_offset(), -0.2);
    let catalog = Task::capture(&source, 0, None)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    let id: Guid = "da3f6e12-49d1-416b-8db6-0ab413f740b6".parse().unwrap();
    draft.select_expression(&source, 0, &catalog, &id).unwrap();
    draft.apply(&mut source, 0).unwrap();
    assert_eq!(
        source.document().text(Some(0), "OFFSET").as_deref(),
        Some("-0.125")
    );
    assert_eq!(
        ChartData::read(source.document(), 0)
            .unwrap()
            .unwrap()
            .extra,
        data.extra
    );
}
#[test]
fn stale_foreign_missing_profiles_and_changed_custom_files_cannot_modify_source_or_redo() {
    let mut source = source(Format::Ssc);
    source
        .edit_notes(
            0,
            "Other",
            [(
                Position {
                    player: 0,
                    row: 12,
                    lane: 3,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let after = source.serialize().unwrap();
    source.undo().unwrap();
    let before = source.serialize().unwrap();
    let mut draft = Draft::capture(&source, 0).unwrap();
    draft.set_music_offset(0.5).unwrap();
    assert!(draft.apply(&mut source, 1).is_err());
    let foreign =
        Session::new(Document::parse(Arc::<[u8]>::from(before.clone()), Format::Ssc).unwrap());
    let catalog = Task::capture(&foreign, 0, None)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert!(
        draft
            .select_expression(&source, 0, &catalog, &DYNAMIC.parse().unwrap())
            .is_err()
    );
    source.redo().unwrap();
    assert!(draft.apply(&mut source, 0).is_err());
    source.undo().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let profile = custom(dir.path());
    let catalog = draft
        .profile_task(&source, 0, Some(dir.path()))
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert!(
        draft
            .select_expression(&source, 0, &catalog, &Guid::default())
            .is_err()
    );
    assert_eq!(draft.expression(), &DYNAMIC.parse().unwrap());
    draft
        .select_expression(&source, 0, &catalog, profile.guid())
        .unwrap();
    let path = dir.path().join(profile.filename());
    let expected = fs::read(&path).unwrap();
    let mut changed = expected.clone();
    changed.push(b'\n');
    fs::write(&path, &changed).unwrap();
    assert!(matches!(
        draft.apply(&mut source, 0),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert_eq!(source.serialize().unwrap(), before);
    source.redo().unwrap();
    assert_eq!(source.serialize().unwrap(), after);
    source.undo().unwrap();
    fs::write(&path, &expected).unwrap();
    draft.apply(&mut source, 0).unwrap();
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
}
#[test]
fn effective_audio_offset_matches_unchanged_grooveauthor_get_music_offset() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("chart-offset-") || !name.ends_with(".request.json") {
            continue;
        }
        let input: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let mut source = source(Format::Ssc);
        source
            .set_text(
                None,
                "OFFSET",
                &input["SongOffset"].as_f64().unwrap().to_string(),
            )
            .unwrap();
        let data = ChartData::from_json(input["Content"].as_str().unwrap()).unwrap();
        data.save(&mut source, 0).unwrap();
        let draft = Draft::capture(&source, 0).unwrap();
        assert_eq!(
            draft.effective_music_offset(),
            expected["EffectiveOffset"].as_f64().unwrap(),
            "{name}"
        );
        count += 1;
    }
    assert_eq!(count, 6);
}
