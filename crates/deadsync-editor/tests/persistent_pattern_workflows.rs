use deadsync_editor::{
    Error,
    document::{Document, Format},
    generation::{
        config::{PatternConfig, PerformedConfig, PerformedPreset},
        job::ChartOptions,
        metadata::{ChartData, Registry},
        pattern_job::{Options, Span, Task},
        profiles::{Kind, Loaded, Profile},
    },
    history::Session,
    notes::{Note, Position},
    storage::FileSession,
};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
fn song(format: Format) -> Vec<u8> {
    let mut bytes = b"// untouched\n#TITLE:Test;#VENDOR:keep;#BPMS:0=120;#OFFSET:0.25;".to_vec();
    for _ in 0..2 {
        bytes.extend_from_slice(match format {
            Format::Sm => b"#NOTES:dance-single:source:Hard:9:0,0,0,0,0:",
            Format::Ssc => b"#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#OFFSET:-0.125;#NOTES:",
        });
        bytes.extend_from_slice(b"1000\n0100\n0010\n0001;");
    }
    bytes
}
fn session(format: Format) -> Session {
    Session::new(Document::parse(Arc::<[u8]>::from(song(format)), format).unwrap())
}
fn options(subdivision: u32) -> Options {
    Options {
        span: Span {
            start: 48,
            end: 96,
            include_start: true,
            include_end: true,
        },
        seed: 42,
        expressed: ChartOptions::balanced("dance-single").expressed,
        performed: PerformedConfig::preset(PerformedPreset::PatternBalanced),
        pattern: PatternConfig {
            beat_sub_division: subdivision,
            ..PatternConfig::sixteenths()
        },
        ignore_preceding_distribution: false,
    }
}
#[test]
fn notes_and_marker_share_one_gesture_preserving_other_pending_charts_and_offsets() {
    for format in [Format::Sm, Format::Ssc] {
        let mut source = session(format);
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
        let preview = Task::capture_persistent(&source, 0, options(4), None)
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .unwrap();
        assert!(preview.persists_marker());
        assert_eq!(source.serialize().unwrap(), before);
        preview.apply(&mut source, 0).unwrap();
        assert_eq!(source.undo_label(), Some("Generate pattern"));
        assert_eq!(source.grid(0).unwrap(), preview.grid());
        assert!(
            source
                .grid(1)
                .unwrap()
                .get(Position {
                    player: 0,
                    row: 12,
                    lane: 3
                })
                .is_some()
        );
        let data = ChartData::read(source.document(), 0).unwrap().unwrap();
        assert_eq!(data.should_use_chart_music_offset, format == Format::Ssc);
        assert_eq!(
            data.music_offset,
            if format == Format::Ssc { -0.125 } else { 0.0 }
        );
        assert_eq!(data.patterns.as_ref().unwrap()[&48].random_seed, 42);
        let after = source.serialize().unwrap();
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), before);
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), song(format));
        source.redo().unwrap();
        source.redo().unwrap();
        assert_eq!(source.serialize().unwrap(), after);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("song.{}", format.extension()));
        fs::write(&path, song(format)).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        file.session = source;
        file.save().unwrap();
        let reopened = FileSession::open(&path).unwrap();
        let regenerated = Task::capture_saved(&reopened.session, 0, 48, &Registry::builtin())
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .unwrap();
        assert_eq!(regenerated.grid(), preview.grid());
    }
}
#[test]
fn custom_profile_creation_is_deferred_and_reacceptance_after_undo_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let profiles = dir.path().join("AutogenConfigs");
    let mut source = session(Format::Ssc);
    let before = source.serialize().unwrap();
    let preview = Task::capture_persistent(&source, 0, options(3), Some(&profiles))
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    assert!(!profiles.exists());
    let mut foreign = session(Format::Ssc);
    assert!(preview.apply(&mut foreign, 0).is_err());
    assert!(!profiles.exists());
    preview.apply(&mut source, 0).unwrap();
    let after = source.serialize().unwrap();
    let loaded = Loaded::directory(&profiles).unwrap();
    assert_eq!(loaded.profiles.len(), 1);
    let data = ChartData::read(source.document(), 0).unwrap().unwrap();
    let definition = &data.patterns.as_ref().unwrap()[&48];
    assert_eq!(
        loaded.registry.patterns[&definition.pattern_config_guid].beat_sub_division,
        3
    );
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
    preview.apply(&mut source, 0).unwrap();
    assert_eq!(source.serialize().unwrap(), after);
    source.undo().unwrap();
    let file = loaded.profiles.values().next().unwrap().path();
    let mut bytes = fs::read(file).unwrap();
    bytes.push(b'\n');
    fs::write(file, &bytes).unwrap();
    assert!(matches!(
        preview.apply(&mut source, 0),
        Err(Error::ExternalChange)
    ));
    assert_eq!(source.serialize().unwrap(), before);
    assert_eq!(source.redo_label(), Some("Generate pattern"));
}
#[test]
fn file_failures_stale_results_and_duplicate_markers_do_not_mutate_the_document() {
    let dir = tempfile::tempdir().unwrap();
    let profiles = dir.path().join("blocked");
    let mut source = session(Format::Ssc);
    let before = source.serialize().unwrap();
    let preview = Task::capture_persistent(&source, 0, options(3), Some(&profiles))
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    fs::write(&profiles, b"external file").unwrap();
    assert!(preview.apply(&mut source, 0).is_err());
    assert_eq!(source.serialize().unwrap(), before);
    assert_eq!(source.revision(), 0);
    assert_eq!(fs::read(&profiles).unwrap(), b"external file");
    source
        .edit_notes(
            0,
            "Edit",
            [(
                Position {
                    row: 180,
                    lane: 0,
                    player: 0,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    assert!(preview.apply(&mut source, 0).is_err());
    source.undo().unwrap();
    let preview = Task::capture_persistent(&source, 0, options(4), None)
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    preview.apply(&mut source, 0).unwrap();
    let saved = source.serialize().unwrap();
    assert!(
        Task::capture_persistent(&source, 0, options(4), None)
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .is_err()
    );
    assert_eq!(source.serialize().unwrap(), saved);
}
#[test]
fn inherited_expression_and_existing_custom_profiles_are_preserved_and_guarded() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grooveauthor/profile-expressed.request.json");
    let input: serde_json::Value = serde_json::from_slice(&fs::read(root).unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let expression = Profile::parse(
        Kind::Expressed,
        input["Content"].as_str().unwrap().as_bytes(),
    )
    .unwrap();
    let file = expression.export_new(dir.path()).unwrap();
    let mut source = session(Format::Ssc);
    ChartData {
        expressed_chart_config: expression.guid().clone(),
        extra: BTreeMap::from([("Future".into(), serde_json::json!({"keep":true}))]),
        ..ChartData::default()
    }
    .save(&mut source, 0)
    .unwrap();
    let before = source.serialize().unwrap();
    let preview = Task::capture_persistent(&source, 0, options(4), Some(dir.path()))
        .unwrap()
        .with_chart_expression()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    let original = fs::read(file.path()).unwrap();
    fs::write(file.path(), b"external change").unwrap();
    assert!(preview.apply(&mut source, 0).is_err());
    assert_eq!(source.serialize().unwrap(), before);
    fs::write(file.path(), original).unwrap();
    preview.apply(&mut source, 0).unwrap();
    let data = ChartData::read(source.document(), 0).unwrap().unwrap();
    assert_eq!(&data.expressed_chart_config, expression.guid());
    assert_eq!(data.extra["Future"]["keep"], true);
}
#[test]
fn atomic_chart_failures_preserve_redo_and_reject_cross_chart_or_structural_notes() {
    let mut source = session(Format::Ssc);
    source
        .edit_notes(
            0,
            "Original",
            [(
                Position {
                    player: 0,
                    row: 12,
                    lane: 2,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    source.undo().unwrap();
    let before = source.serialize().unwrap();
    assert!(
        source
            .edit_chart_atomic(0, "Fail", |temporary| {
                temporary.edit_notes(
                    0,
                    "Note",
                    [(
                        Position {
                            player: 0,
                            row: 12,
                            lane: 2,
                        },
                        Some(Note::tap()),
                    )],
                )?;
                temporary.set_text(Some(0), "NOTES", "unsafe")?;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(source.serialize().unwrap(), before);
    assert_eq!(source.redo_label(), Some("Original"));
    assert!(
        source
            .edit_chart_atomic(0, "Wrong chart", |temporary| {
                temporary.edit_notes(
                    1,
                    "Note",
                    [(
                        Position {
                            player: 0,
                            row: 12,
                            lane: 2,
                        },
                        Some(Note::tap()),
                    )],
                )?;
                Ok(())
            })
            .is_err()
    );
    assert!(
        source
            .edit_chart_atomic(0, "Structural", |temporary| {
                let grid = temporary.grid(0)?.clone();
                let revision = temporary.revision();
                temporary.append_chart(0, "dance-single", &grid, revision, "Clone")?;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(source.serialize().unwrap(), before);
    source.redo().unwrap();
    assert!(
        source
            .grid(0)
            .unwrap()
            .get(Position {
                player: 0,
                row: 12,
                lane: 2
            })
            .is_some()
    );
}

#[test]
fn every_native_subdivision_can_persist_and_regenerate_with_exact_profile_references() {
    let dir = tempfile::tempdir().unwrap();
    let profiles = dir.path().join("AutogenConfigs");
    for subdivision in [1, 2, 3, 4, 6, 8, 12, 16, 48] {
        let mut source = session(Format::Ssc);
        let before = fs::read_dir(&profiles)
            .ok()
            .map(|files| files.count())
            .unwrap_or(0);
        let preview = Task::capture_persistent(&source, 0, options(subdivision), Some(&profiles))
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .unwrap();
        assert_eq!(
            fs::read_dir(&profiles)
                .ok()
                .map(|files| files.count())
                .unwrap_or(0),
            before
        );
        preview.apply(&mut source, 0).unwrap();
        let loaded = Loaded::directory(&profiles).unwrap();
        let data = ChartData::read(source.document(), 0).unwrap().unwrap();
        assert_eq!(
            data.options(48, &loaded.registry)
                .unwrap()
                .pattern
                .beat_sub_division,
            subdivision
        );
        let reopened = Session::new(
            Document::parse(Arc::<[u8]>::from(source.serialize().unwrap()), Format::Ssc).unwrap(),
        );
        let again = Task::capture_saved(&reopened, 0, 48, &loaded.registry)
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .unwrap();
        assert_eq!(again.grid(), preview.grid());
    }
    assert_eq!(Loaded::directory(&profiles).unwrap().profiles.len(), 7);
}
#[test]
fn creating_marker_metadata_keeps_the_original_offset_override_comparison() {
    let song_offset = 0.25f64;
    for (offset, expected) in [
        (song_offset, false),
        (f64::from_bits(song_offset.to_bits() + 1), false),
        (f64::from_bits(song_offset.to_bits() + 2), true),
        (-0.125, true),
    ] {
        let bytes = format!(
            "#OFFSET:{song_offset};#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#OFFSET:{offset};#NOTES:1000;"
        );
        let document = Document::parse(Arc::<[u8]>::from(bytes.into_bytes()), Format::Ssc).unwrap();
        let data = ChartData::existing_or_initial(&document, 0).unwrap();
        assert_eq!(data.should_use_chart_music_offset, expected);
        assert_eq!(data.music_offset, if expected { offset } else { 0.0 });
    }
}

#[test]
fn discarding_and_cancelling_ready_persistent_previews_never_create_profile_files() {
    use deadsync_editor::generation::job::Phase;
    use std::time::{Duration, Instant};
    let dir = tempfile::tempdir().unwrap();
    let profiles = dir.path().join("AutogenConfigs");
    let source = session(Format::Ssc);
    let before = source.serialize().unwrap();
    let preview = Task::capture_persistent(&source, 0, options(3), Some(&profiles))
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    drop(preview);
    assert!(!profiles.exists());
    let mut job = Task::capture_persistent(&source, 0, options(3), Some(&profiles))
        .unwrap()
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while job.progress().phase != Phase::Ready {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!profiles.exists());
    job.cancel();
    assert!(job.poll().unwrap().is_err());
    assert!(job.poll().is_none());
    assert!(!profiles.exists());
    assert_eq!(source.serialize().unwrap(), before);
}
