use deadsync_editor::{
    document::{Document, Format},
    generation::{
        config::{PatternConfig, PerformedConfig, PerformedPreset},
        marker_edit::Draft,
        metadata::{ChartData, Definition, Guid, PATTERN_BALANCED, Registry, SIXTEENTHS},
        pattern_job::Task as PatternTask,
        profile_catalog::Task,
        profiles::{Kind, Loaded, Profile},
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
fn source(format: Format) -> Session {
    let mut bytes = b"#TITLE:Caf\xe9;#VENDOR:keep;#BPMS:0=120;".to_vec();
    for _ in 0..2 {
        bytes.extend_from_slice(match format {
            Format::Sm => b"#NOTES:dance-single:source:Hard:9:0,0,0,0,0:",
            Format::Ssc => b"#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:",
        });
        bytes.extend_from_slice(b"1000\n0100\n0010\n0001;");
    }
    let mut source = Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap());
    ChartData {
        patterns: Some(BTreeMap::from([(
            0,
            Definition {
                length: 96,
                random_seed: -7,
                pattern_config_guid: SIXTEENTHS.parse().unwrap(),
                performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                start_position_inclusive: true,
                end_position_inclusive: true,
                extra: BTreeMap::from([("FutureMarker".into(), serde_json::json!(true))]),
                ..Definition::default()
            },
        )])),
        extra: BTreeMap::from([("FutureChart".into(), serde_json::json!("keep"))]),
        ..ChartData::default()
    }
    .save(&mut source, 0)
    .unwrap();
    source
}
fn custom_profiles(directory: &std::path::Path) -> (Profile, Profile) {
    let pattern = Profile::new_pattern(
        "Triplets",
        PatternConfig {
            beat_sub_division: 3,
            ..PatternConfig::sixteenths()
        },
    )
    .unwrap();
    pattern.export_new(directory).unwrap();
    let performed=Profile::parse(Kind::Performed,&serde_json::to_vec(&serde_json::json!({"Guid":"11111111-2222-3333-4444-555555555555","Name":"Custom feet","Config":PerformedConfig::preset(PerformedPreset::PatternNoCandles),"Future":{"keep":true}})).unwrap()).unwrap();
    performed.export_new(directory).unwrap();
    (pattern, performed)
}
#[test]
fn catalog_contains_all_builtins_is_read_only_and_keeps_kind_namespaces_and_custom_names() {
    let source = source(Format::Ssc);
    let before = source.serialize().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("absent");
    let catalog = Task::capture(&source, 0, Some(&missing))
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert!(!missing.exists());
    for (kind, count) in [
        (Kind::Pattern, 2),
        (Kind::Performed, 7),
        (Kind::Expressed, 3),
    ] {
        let entries = catalog.entries(kind);
        assert_eq!(entries.len(), count);
        assert!(
            entries
                .iter()
                .all(|entry| entry.builtin.is_some() && entry.name.is_none())
        );
        let mut labels = entries
            .iter()
            .map(|entry| format!("{:?}", entry.builtin))
            .collect::<Vec<_>>();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), count);
    }
    let (pattern, performed) = custom_profiles(&missing);
    fs::write(missing.join("pc-broken.json"), b"invalid").unwrap();
    let unnamed=Profile::parse(Kind::Performed,&serde_json::to_vec(&serde_json::json!({"Guid":pattern.guid(),"Config":PerformedConfig::preset(PerformedPreset::PatternNoInward)})).unwrap()).unwrap();
    unnamed.export_new(&missing).unwrap();
    let catalog = Task::capture(&source, 0, Some(&missing))
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(catalog.issues().len(), 1);
    assert_eq!(
        catalog.entries(Kind::Pattern).last().unwrap().guid,
        *pattern.guid()
    );
    assert!(catalog.entries(Kind::Performed).iter().any(
        |entry| &entry.guid == performed.guid() && entry.name.as_deref() == Some("Custom feet")
    ));
    assert!(
        catalog
            .entries(Kind::Performed)
            .iter()
            .any(|entry| &entry.guid == pattern.guid()
                && entry.name.is_none()
                && entry.builtin.is_none())
    );
    assert_eq!(
        fs::read(missing.join("pc-broken.json")).unwrap(),
        b"invalid"
    );
    assert_eq!(source.serialize().unwrap(), before);
}
#[test]
fn selecting_profiles_stages_one_marker_gesture_preserves_metadata_and_drives_reopened_generation()
{
    for format in [Format::Sm, Format::Ssc] {
        let mut source = source(format);
        let before = source.serialize().unwrap();
        let grid = source.grid(0).unwrap().clone();
        let old = ChartData::read(source.document(), 0).unwrap().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("AutogenConfigs");
        let (pattern, performed) = custom_profiles(&folder);
        let mut draft = Draft::at_cursor(&source, 0, 0).unwrap();
        let catalog = draft
            .profile_task(&source, 0, Some(&folder))
            .unwrap()
            .run(&AtomicBool::new(false))
            .unwrap();
        draft
            .select_profiles(&source, 0, &catalog, pattern.guid(), performed.guid())
            .unwrap();
        assert!(draft.has_changes());
        assert_eq!(source.serialize().unwrap(), before);
        draft.apply(&mut source, 0).unwrap();
        let after = source.serialize().unwrap();
        assert_eq!(source.grid(0).unwrap(), &grid);
        let new = ChartData::read(source.document(), 0).unwrap().unwrap();
        let marker = &new.patterns.as_ref().unwrap()[&0];
        assert_eq!(&marker.pattern_config_guid, pattern.guid());
        assert_eq!(&marker.performed_chart_config_guid, performed.guid());
        assert_eq!(marker.random_seed, -7);
        assert_eq!(marker.extra, old.patterns.as_ref().unwrap()[&0].extra);
        assert_eq!(new.extra, old.extra);
        assert_eq!(new.expressed_chart_config, old.expressed_chart_config);
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
        let mut draft = Draft::at_cursor(&file.session, 0, 0).unwrap();
        let catalog = Task::capture(&file.session, 0, Some(&folder))
            .unwrap()
            .run(&AtomicBool::new(false))
            .unwrap();
        draft
            .select_profiles(&file.session, 0, &catalog, pattern.guid(), performed.guid())
            .unwrap();
        draft.apply(&mut file.session, 0).unwrap();
        assert_eq!(file.session.serialize().unwrap(), after);
        let output = file.session.serialize_for_output().unwrap();
        assert_eq!(file.session.serialize().unwrap(), after);
        file.save().unwrap();
        assert_eq!(file.session.serialize().unwrap(), after);
        assert_eq!(fs::read(&path).unwrap(), output);
        let mut reopened = FileSession::open(&path).unwrap();
        assert_eq!(reopened.session.serialize().unwrap(), output);
        let reopened_metadata = ChartData::read(reopened.session.document(), 0)
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::to_value(reopened_metadata).unwrap(),
            serde_json::to_value(new).unwrap()
        );
        let registry = Loaded::directory(&folder).unwrap().registry;
        let options = ChartData::read(reopened.session.document(), 0)
            .unwrap()
            .unwrap()
            .options(0, &registry)
            .unwrap();
        assert_eq!(options.pattern.beat_sub_division, 3);
        assert_eq!(
            options.performed,
            PerformedConfig::preset(PerformedPreset::PatternNoCandles)
        );
        let preview = PatternTask::capture_saved(&reopened.session, 0, 0, &registry)
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .unwrap();
        preview.apply(&mut reopened.session, 0).unwrap();
        assert_eq!(reopened.session.grid(0).unwrap(), preview.grid());
        assert_eq!(
            fs::read(folder.join(pattern.filename())).unwrap(),
            pattern.serialize().unwrap()
        );
        assert_eq!(
            fs::read(folder.join(performed.filename())).unwrap(),
            performed.serialize().unwrap()
        );
    }
}
#[test]
fn selection_guards_foreign_stale_wrong_kind_missing_and_manually_replaced_identities() {
    let mut source = source(Format::Ssc);
    let catalog = Task::capture(&source, 0, None)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    let mut draft = Draft::at_cursor(&source, 0, 0).unwrap();
    let pattern: Guid = SIXTEENTHS.parse().unwrap();
    let performed: Guid = PATTERN_BALANCED.parse().unwrap();
    let before = source.serialize().unwrap();
    assert!(
        draft
            .select_profiles(&source, 1, &catalog, &pattern, &performed)
            .is_err()
    );
    assert!(
        draft
            .select_profiles(&source, 0, &catalog, &performed, &pattern)
            .is_err()
    );
    assert!(
        draft
            .select_profiles(&source, 0, &catalog, &Guid::default(), &performed)
            .is_err()
    );
    assert!(!draft.has_changes());
    let foreign =
        Session::new(Document::parse(Arc::<[u8]>::from(before.clone()), Format::Ssc).unwrap());
    let foreign_catalog = Task::capture(&foreign, 0, None)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert!(
        draft
            .select_profiles(&source, 0, &foreign_catalog, &pattern, &performed)
            .is_err()
    );
    source
        .edit_notes(
            0,
            "Edit",
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
    assert!(
        draft
            .select_profiles(&source, 0, &catalog, &pattern, &performed)
            .is_err()
    );
    source.undo().unwrap();
    draft
        .select_profiles(&source, 0, &catalog, &pattern, &performed)
        .unwrap();
    assert!(!draft.apply(&mut source, 0).unwrap());
    draft.definition.pattern_config_guid = "b3779494-f8c2-4a76-be45-b5f6bfcb8fc0".parse().unwrap();
    assert!(draft.apply(&mut source, 0).is_err());
    assert_eq!(source.serialize().unwrap(), before);
    source.redo().unwrap();
}
#[test]
fn selected_custom_file_changes_reject_apply_without_clearing_redo_or_altering_profiles() {
    let mut source = source(Format::Ssc);
    let dir = tempfile::tempdir().unwrap();
    let (pattern, performed) = custom_profiles(dir.path());
    source
        .edit_notes(
            0,
            "Edit",
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
    let mut draft = Draft::at_cursor(&source, 0, 0).unwrap();
    let catalog = Task::capture(&source, 0, Some(dir.path()))
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    draft
        .select_profiles(&source, 0, &catalog, pattern.guid(), performed.guid())
        .unwrap();
    let path = dir.path().join(performed.filename());
    let expected = fs::read(&path).unwrap();
    let mut changed = expected.clone();
    changed.push(b'\n');
    fs::write(&path, &changed).unwrap();
    assert!(matches!(
        draft.apply(&mut source, 0),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert_eq!(source.serialize().unwrap(), before);
    assert_eq!(fs::read(&path).unwrap(), changed);
    source.redo().unwrap();
    assert_eq!(source.serialize().unwrap(), after);
    source.undo().unwrap();
    fs::write(&path, &expected).unwrap();
    draft.apply(&mut source, 0).unwrap();
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
}
#[test]
fn unresolved_current_profiles_are_preserved_until_explicit_replacement_and_catalog_cancel_is_read_only()
 {
    let mut source = source(Format::Ssc);
    let mut data = ChartData::read(source.document(), 0).unwrap().unwrap();
    data.patterns
        .as_mut()
        .unwrap()
        .get_mut(&0)
        .unwrap()
        .pattern_config_guid = Guid::default();
    data.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    let mut draft = Draft::at_cursor(&source, 0, 0).unwrap();
    assert!(
        draft
            .profile_task(&source, 0, None)
            .unwrap()
            .run(&AtomicBool::new(true))
            .is_err()
    );
    let catalog = draft
        .profile_task(&source, 0, None)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert!(
        !catalog
            .entries(Kind::Pattern)
            .iter()
            .any(|entry| entry.guid == Guid::default())
    );
    assert!(!draft.has_changes());
    assert_eq!(source.serialize().unwrap(), before);
    draft
        .select_profiles(
            &source,
            0,
            &catalog,
            &SIXTEENTHS.parse().unwrap(),
            &PATTERN_BALANCED.parse().unwrap(),
        )
        .unwrap();
    draft.apply(&mut source, 0).unwrap();
    let registry = Registry::builtin();
    assert!(
        ChartData::read(source.document(), 0)
            .unwrap()
            .unwrap()
            .options(0, &registry)
            .is_ok()
    );
    let before = source.serialize().unwrap();
    let mut job = Task::capture(&source, 0, None).unwrap().spawn().unwrap();
    job.cancel();
    let limit = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if let Some(result) = job.poll() {
            assert!(result.is_err());
            break;
        }
        assert!(std::time::Instant::now() < limit);
        std::thread::yield_now();
    }
    assert!(job.poll().is_none());
    assert_eq!(source.serialize().unwrap(), before);
}
