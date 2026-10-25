use deadsync_editor::{
    document::{Document, Format},
    generation::{
        config::{BracketDetermination, BracketMethod, ExpressedConfig},
        job::{ChartOptions, ChartTask},
        marker_edit::Draft,
        metadata::{ChartData, DYNAMIC, Definition, Guid, PATTERN_BALANCED, Registry, SIXTEENTHS},
        pattern_job::{Options, Span, Task as PatternTask},
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
    let mut bytes = b"#TITLE:Caf\xe9;#VENDOR:keep;#BPMS:0=120;#OFFSET:0.25;".to_vec();
    for _ in 0..2 {
        bytes.extend_from_slice(match format {Format::Sm=>b"#NOTES:dance-single:source:Hard:9:0,0,0,0,0:",Format::Ssc=>b"#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#OFFSET:-0.125;#NOTES:"});
        bytes.extend_from_slice(b"1000\n0100\n0010\n0001;");
    }
    let mut source = Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap());
    let mut data = ChartData::existing_or_initial(source.document(), 0).unwrap();
    let marker = Definition {
        length: 96,
        random_seed: -7,
        pattern_config_guid: SIXTEENTHS.parse().unwrap(),
        performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
        start_position_inclusive: true,
        end_position_inclusive: true,
        extra: BTreeMap::from([("FutureMarker".into(), serde_json::json!(true))]),
        ..Definition::default()
    };
    data.patterns = Some(BTreeMap::from([(0, marker.clone()), (192, marker)]));
    data.extra
        .insert("FutureChart".into(), serde_json::json!("keep"));
    data.save(&mut source, 0).unwrap();
    source
}
fn profile(folder: &std::path::Path) -> Profile {
    let config = ExpressedConfig {
        bracket_parsing_determination: BracketDetermination::UseDefaultMethod,
        default_bracket_parsing_method: BracketMethod::NoBrackets,
        ..ExpressedConfig::default()
    };
    let profile=Profile::parse(Kind::Expressed,&serde_json::to_vec(&serde_json::json!({"Guid":"11111111-2222-3333-4444-555555555555","Name":"Custom interpretation","Config":config,"Future":{"keep":true}})).unwrap()).unwrap();
    profile.export_new(folder).unwrap();
    profile
}
fn select(source: &Session, draft: &mut Draft, folder: &std::path::Path, id: &Guid) {
    let catalog = draft
        .profile_task(source, 0, Some(folder))
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    draft
        .select_generation_profiles(
            source,
            0,
            &catalog,
            &SIXTEENTHS.parse().unwrap(),
            &PATTERN_BALANCED.parse().unwrap(),
            id,
        )
        .unwrap();
}
#[test]
fn chart_interpretation_is_one_gesture_for_all_markers_preserving_notes_offsets_and_other_charts() {
    for format in [Format::Sm, Format::Ssc] {
        let mut source = source(format);
        source
            .edit_notes(
                1,
                "Other chart",
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
        let grid = source.grid(0).unwrap().clone();
        let data = ChartData::read(source.document(), 0).unwrap().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let profile = profile(dir.path());
        let mut draft = Draft::at_cursor(&source, 0, 0).unwrap();
        select(&source, &mut draft, dir.path(), profile.guid());
        assert!(draft.has_changes());
        assert!(draft.clear_task(&source, 0, None).is_err());
        assert_eq!(source.serialize().unwrap(), before);
        draft.apply(&mut source, 0).unwrap();
        let after = source.serialize().unwrap();
        let actual = ChartData::read(source.document(), 0).unwrap().unwrap();
        assert_eq!(actual.expressed_chart_config, *profile.guid());
        assert_eq!(actual.patterns, data.patterns);
        assert_eq!(actual.extra, data.extra);
        assert_eq!(actual.music_offset, data.music_offset);
        assert_eq!(
            actual.should_use_chart_music_offset,
            data.should_use_chart_music_offset
        );
        assert_eq!(source.grid(0).unwrap(), &grid);
        let registry = Loaded::directory(dir.path()).unwrap().registry;
        for row in [0, 192] {
            assert_eq!(
                actual.options(row, &registry).unwrap().expressed,
                registry.expressed[profile.guid()]
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
        fs::write(&path, &after).unwrap();
        let reopened = FileSession::open(&path).unwrap();
        assert_eq!(
            ChartData::read(reopened.session.document(), 0)
                .unwrap()
                .unwrap()
                .expressed_chart_config,
            *profile.guid()
        );
    }
}
#[test]
fn f3_f4_f5_workers_resolve_the_same_custom_chart_expression_and_preserve_profile_files() {
    let mut source = source(Format::Ssc);
    let dir = tempfile::tempdir().unwrap();
    let custom = profile(dir.path());
    let mut draft = Draft::at_cursor(&source, 0, 0).unwrap();
    select(&source, &mut draft, dir.path(), custom.guid());
    draft.apply(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    let registry = Loaded::directory(dir.path()).unwrap().registry;
    fs::write(dir.path().join("ecc-broken.json"), b"invalid").unwrap();
    let options = ChartOptions::balanced("dance-double");
    let mut explicit = options.clone();
    explicit.expressed = registry.expressed[custom.guid()].clone();
    let expected = ChartTask::capture(&source, 0, explicit)
        .unwrap()
        .run(&AtomicBool::new(false), |_| {})
        .unwrap();
    let task = ChartTask::capture(&source, 0, options)
        .unwrap()
        .with_chart_expression(Some(dir.path()))
        .unwrap();
    let preview = task.run(&AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(preview.expression_profile(), Some(custom.guid()));
    assert_eq!(preview.grid(), expected.grid());
    assert_eq!(preview.profile_issues().len(), 1);
    let options = Options {
        span: Span {
            start: 240,
            end: 288,
            include_start: true,
            include_end: true,
        },
        seed: 42,
        pattern: registry.patterns[&SIXTEENTHS.parse().unwrap()].clone(),
        performed: registry.performed[&PATTERN_BALANCED.parse().unwrap()].clone(),
        expressed: ChartOptions::balanced("dance-single").expressed,
        ignore_preceding_distribution: false,
    };
    let mut explicit = options.clone();
    explicit.expressed = registry.expressed[custom.guid()].clone();
    let expected = PatternTask::capture(&source, 0, explicit)
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    let preview = PatternTask::capture_persistent(&source, 0, options, Some(dir.path()))
        .unwrap()
        .with_chart_expression()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    assert_eq!(preview.grid(), expected.grid());
    assert_eq!(source.serialize().unwrap(), before);
    preview.apply(&mut source, 0).unwrap();
    assert_eq!(
        ChartData::read(source.document(), 0)
            .unwrap()
            .unwrap()
            .expressed_chart_config,
        *custom.guid()
    );
    let path = dir.path().join("song.ssc");
    fs::write(&path, source.serialize().unwrap()).unwrap();
    let reopened = FileSession::open(&path).unwrap();
    let preview = PatternTask::capture_saved(&reopened.session, 0, 240, &registry)
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    assert_eq!(preview.grid(), expected.grid());
    assert_eq!(
        fs::read(dir.path().join(custom.filename())).unwrap(),
        custom.serialize().unwrap()
    );
    assert_eq!(
        fs::read(dir.path().join("ecc-broken.json")).unwrap(),
        b"invalid"
    );
}
#[test]
fn unknown_expression_and_stale_catalogs_do_not_partially_stage_profile_changes() {
    let mut source = source(Format::Ssc);
    let before = source.serialize().unwrap();
    let catalog = Task::capture(&source, 0, None)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    let mut draft = Draft::at_cursor(&source, 0, 0).unwrap();
    let pattern: Guid = "b3779494-f8c2-4a76-be45-b5f6bfcb8fc0".parse().unwrap();
    let performed: Guid = PATTERN_BALANCED.parse().unwrap();
    assert!(
        draft
            .select_generation_profiles(
                &source,
                0,
                &catalog,
                &pattern,
                &performed,
                &Guid::default()
            )
            .is_err()
    );
    assert!(!draft.has_changes());
    assert_eq!(
        draft.definition.pattern_config_guid,
        SIXTEENTHS.parse().unwrap()
    );
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
    assert!(
        draft
            .select_generation_profiles(
                &source,
                0,
                &catalog,
                &pattern,
                &performed,
                &DYNAMIC.parse().unwrap()
            )
            .is_err()
    );
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
    let mut data = ChartData::read(source.document(), 0).unwrap().unwrap();
    data.expressed_chart_config = Guid::default();
    data.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    assert!(
        ChartTask::capture(&source, 0, ChartOptions::balanced("dance-double"))
            .unwrap()
            .with_chart_expression(None)
            .unwrap()
            .run(&AtomicBool::new(false), |_| {})
            .is_err()
    );
    assert_eq!(source.serialize().unwrap(), before);
}
#[test]
fn custom_expression_byte_guards_block_apply_preserving_redo_and_deletion_keeps_chart_expression() {
    let mut source = source(Format::Ssc);
    let dir = tempfile::tempdir().unwrap();
    let custom = profile(dir.path());
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
    let mut draft = Draft::at_cursor(&source, 0, 0).unwrap();
    select(&source, &mut draft, dir.path(), custom.guid());
    let path = dir.path().join(custom.filename());
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
    let mut draft = Draft::at_cursor(&source, 0, 0).unwrap();
    let registry = Registry::builtin();
    let catalog = Task::capture(&source, 0, None)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    draft
        .select_generation_profiles(
            &source,
            0,
            &catalog,
            registry.patterns.keys().next().unwrap(),
            registry.performed.keys().next().unwrap(),
            &DYNAMIC.parse().unwrap(),
        )
        .unwrap();
    draft.delete(&mut source, 0).unwrap();
    assert_eq!(
        ChartData::read(source.document(), 0)
            .unwrap()
            .unwrap()
            .expressed_chart_config,
        *custom.guid()
    );
}
