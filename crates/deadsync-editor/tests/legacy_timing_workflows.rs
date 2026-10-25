use deadsync_editor::{
    document::{Document, Format},
    generation::{editor_timing::EditorTimeline, source, timing::GenerationTiming},
    history::Session,
    timing_edit::{self, Scope, migration::Draft},
    workspace::{Command, Workspace},
};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor")
}
fn session(bytes: &[u8]) -> Session {
    Session::new(Document::parse(Arc::<[u8]>::from(bytes), Format::Ssc).unwrap())
}
fn mixed() -> Vec<u8> {
    let input: Value =
        serde_json::from_slice(&fs::read(root().join("legacy-timing-mixed.request.json")).unwrap())
            .unwrap();
    input["Content"].as_str().unwrap().as_bytes().to_vec()
}
fn check(source: &mut Session, expected: &Value, name: &str) {
    let bytes = source.serialize().unwrap();
    assert_eq!(
        source.document().charts().len(),
        expected.as_array().unwrap().len()
    );
    for (chart, reference) in expected.as_array().unwrap().iter().enumerate() {
        let timing = GenerationTiming::from_simfile(&bytes, Format::Ssc, chart).unwrap();
        let grid = source.grid(chart).unwrap();
        let timeline = EditorTimeline::build(&timing, grid, None, &AtomicBool::new(false)).unwrap();
        let reference = &reference["Editor"];
        assert_eq!(
            timeline.heads().len(),
            reference["Heads"].as_array().unwrap().len()
        );
        for (head, expected) in timeline
            .heads()
            .iter()
            .zip(reference["Heads"].as_array().unwrap())
        {
            assert_eq!(head.position.row, expected["Row"].as_u64().unwrap());
            assert_eq!(
                head.time,
                expected["Time"].as_f64().unwrap(),
                "{name} chart {chart} row {}",
                head.position.row
            );
            assert_eq!(head.fake, expected["Fake"].as_bool().unwrap(), "{name}");
        }
        assert_eq!(
            timeline.step_count() as u64,
            reference["StepCount"].as_u64().unwrap()
        );
        assert_eq!(
            timeline.chart_end().unwrap(),
            reference["End"].as_f64().unwrap()
        );
        for (row, time) in reference["Times"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                timeline.position_time((row * 12) as u64).unwrap(),
                time.as_f64().unwrap(),
                "{name}"
            );
        }
    }
}
#[test]
fn original_legacy_chart_clocks_survive_reviewed_migration_and_reopen() {
    let mut count = 0;
    for entry in fs::read_dir(root()).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("legacy-timing-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let input: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root().join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let bytes = input["Content"].as_str().unwrap().as_bytes();
        let mut source = session(bytes);
        assert!(timing_edit::migration::needed(source.document()));
        check(&mut source, &expected, &name);
        assert!(source::read(Arc::from(bytes), Format::Ssc, 0, &AtomicBool::new(false)).is_err());
        let draft = Draft::capture(&source).unwrap();
        assert!(draft.has_changes());
        assert!(!draft.charts().is_empty());
        assert_eq!(source.serialize().unwrap(), bytes);
        assert!(draft.apply(&mut source).unwrap());
        assert!(timing_edit::supports_chart_timing(source.document()));
        check(&mut source, &expected, &name);
        let after = source.serialize().unwrap();
        check(&mut session(&after), &expected, &name);
        assert!(
            source::read(
                Arc::from(after.clone()),
                Format::Ssc,
                0,
                &AtomicBool::new(false)
            )
            .is_ok()
        );
        assert!(!Draft::capture(&source).unwrap().has_changes());
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), bytes);
        source.redo().unwrap();
        assert_eq!(source.serialize().unwrap(), after);
    }
    assert_eq!(count, 3);
}
#[test]
fn migration_reports_local_charts_and_materializes_only_missing_or_changed_tempo_bytes() {
    let bytes = mixed();
    let mut source = session(&bytes);
    let draft = Draft::capture(&source).unwrap();
    assert_eq!(draft.previous_version(), Some("0.60"));
    assert_eq!(draft.target_version(), "0.70");
    assert_eq!(
        draft
            .charts()
            .iter()
            .filter(|change| change.previously_local)
            .count(),
        2
    );
    assert_eq!(
        draft
            .charts()
            .iter()
            .filter(|change| change.materializes_tempo)
            .count(),
        1
    );
    let before =
        deadsync_editor::analysis::chart(&source, 1, &rssp::AnalysisOptions::default()).unwrap();
    draft.apply(&mut source).unwrap();
    let after =
        deadsync_editor::analysis::chart(&source, 1, &rssp::AnalysisOptions::default()).unwrap();
    assert_ne!(
        format!("{:?}", before.timing_segments),
        format!("{:?}", after.timing_segments)
    );
    assert_eq!(
        source.document().text(Some(2), "BPMS").as_deref(),
        Some("0=150")
    );
    assert!(source.document().raw_property(Some(0), "BPMS").is_none());
    for tag in timing_edit::TIMING_TAGS {
        assert_eq!(
            source.document().raw_property(None, tag),
            session(&bytes).document().raw_property(None, tag),
            "root {tag}"
        );
    }
}
#[test]
fn conversion_preserves_pending_notes_legacy_encoding_unknown_tags_and_two_history_gestures() {
    let bytes=b"// keep\n#VERSION:0.60;#TITLE:caf\xe9;#BPMS:0=120;#LABELS:0=keep\\;label;#VENDOR:opaque;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:old name;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#OFFSET:0.25;#LABELS:0=caf\xe9;#VENDOR_CHART:keep;#NOTES:0001;";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, bytes).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    workspace.execute(Command::Place(2)).unwrap();
    let notes = workspace.grid().clone();
    let draft = workspace.timing_migration().unwrap();
    workspace.apply_timing_migration(&draft).unwrap();
    assert_eq!(workspace.grid(), &notes);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(
        workspace
            .file()
            .session
            .document()
            .raw_property(Some(1), "LABELS"),
        Some(b"0=caf\xe9".as_slice())
    );
    assert_eq!(
        workspace
            .file()
            .session
            .document()
            .text(Some(1), "VENDOR_CHART")
            .as_deref(),
        Some("keep")
    );
    workspace.execute(Command::Save).unwrap();
    let reopened = Workspace::open(&path, 0).unwrap();
    assert_eq!(reopened.grid(), &notes);
    assert!(timing_edit::supports_chart_timing(
        reopened.file().session.document()
    ));
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.grid(), &notes);
    assert!(!timing_edit::supports_chart_timing(
        workspace.file().session.document()
    ));
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.file().session.serialize().unwrap(), bytes);
}
#[test]
fn version_selection_preserves_the_chart_name_boundary_and_modern_versions_are_no_ops() {
    for (version, target, changes) in [
        (Some("0.60"), "0.70", true),
        (Some("0.6-custom"), "0.70", true),
        (None, "0.83", true),
        (Some("unknown"), "0.83", true),
        (Some("0.70"), "0.70", false),
        (Some("0.83"), "0.83", false),
        (Some("1.25"), "1.25", false),
    ] {
        let bytes = format!(
            "{}#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#CHARTNAME:Named;#DESCRIPTION:Description;#NOTES:1000;",
            version
                .map(|version| format!("#VERSION:{version};"))
                .unwrap_or_default()
        );
        let mut source = session(bytes.as_bytes());
        let old = rssp::parse::parse_version(
            source.document().raw_property(None, "VERSION"),
            rssp::timing::TimingFormat::Ssc,
        );
        source.set_text(None, "TITLE", "redo").unwrap();
        source.undo().unwrap();
        let redo = source.redo_label().unwrap().to_owned();
        let draft = Draft::capture(&source).unwrap();
        assert_eq!(draft.has_changes(), changes);
        assert_eq!(draft.target_version(), target);
        assert_eq!(draft.apply(&mut source).unwrap(), changes);
        let new = rssp::parse::parse_version(
            source.document().raw_property(None, "VERSION"),
            rssp::timing::TimingFormat::Ssc,
        );
        assert_eq!(
            rssp::parse::normalize_chart_name(
                "Named".into(),
                "Description",
                rssp::timing::TimingFormat::Ssc,
                old
            ),
            rssp::parse::normalize_chart_name(
                "Named".into(),
                "Description",
                rssp::timing::TimingFormat::Ssc,
                new
            )
        );
        assert_eq!(
            rssp::parse::normalize_chart_desc_ref(
                "Description",
                rssp::timing::TimingFormat::Ssc,
                old
            ),
            rssp::parse::normalize_chart_desc_ref(
                "Description",
                rssp::timing::TimingFormat::Ssc,
                new
            )
        );
        if !changes {
            assert_eq!(source.redo_label(), Some(redo.as_str()));
            assert_eq!(source.serialize().unwrap(), bytes.as_bytes());
        }
    }
}
#[test]
fn stale_foreign_discarded_or_invalid_migrations_do_not_change_data_or_redo() {
    let bytes = mixed();
    let mut source = session(&bytes);
    source.set_text(None, "TITLE", "redo").unwrap();
    source.undo().unwrap();
    let redo = source.redo_label().unwrap().to_owned();
    let draft = Draft::capture(&source).unwrap();
    let mut foreign = session(&bytes);
    assert!(draft.apply(&mut foreign).is_err());
    assert_eq!(foreign.serialize().unwrap(), bytes);
    drop(draft);
    assert_eq!(source.redo_label(), Some(redo.as_str()));
    let draft = Draft::capture(&source).unwrap();
    source.set_text(None, "TITLE", "changed").unwrap();
    let changed = source.serialize().unwrap();
    assert!(draft.apply(&mut source).is_err());
    assert_eq!(source.serialize().unwrap(), changed);
    let malformed =
        session(b"#VERSION:0.60;#BPMS:0=inf;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;");
    assert!(Draft::capture(&malformed).is_err());
    assert_eq!(
        malformed.document().text(None, "VERSION").as_deref(),
        Some("0.60")
    );
    let sm = Session::new(
        Document::parse(
            Arc::<[u8]>::from(b"#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;".as_slice()),
            Format::Sm,
        )
        .unwrap(),
    );
    assert!(Draft::capture(&sm).is_err());
}
#[test]
fn timing_edits_and_offset_exports_cannot_silently_write_ignored_legacy_local_timing() {
    let bytes = mixed();
    let mut source = session(&bytes);
    assert!(timing_edit::TempoDraft::capture(&source, 0, Scope::Song).is_err());
    assert!(timing_edit::pause::Draft::capture(&source, 0, Scope::Song).is_err());
    assert!(timing_edit::region::Draft::capture(&source, 0, Scope::Song).is_err());
    let mut properties = deadsync_editor::chart_properties::Draft::capture(&source, 0).unwrap();
    properties.set_use_chart_offset(true);
    properties.set_music_offset(0.5).unwrap();
    assert!(properties.apply(&mut source, 0).is_err());
    assert_eq!(source.serialize().unwrap(), bytes);
    let draft = Draft::capture(&source).unwrap();
    draft.apply(&mut source).unwrap();
    assert!(timing_edit::TempoDraft::capture(&source, 0, Scope::Chart).is_ok());
    assert!(properties.apply(&mut source, 0).is_err());
}

#[test]
fn migrated_generation_matches_original_legacy_editor_and_saves_with_reversible_chart_creation() {
    use deadsync_editor::{
        generation::{
            config::{BracketDetermination, ExpressedConfig},
            job::{ChartOptions, ChartTask},
        },
        storage::FileSession,
    };
    let request: Value =
        serde_json::from_slice(&fs::read(root().join("legacy-timing-mixed.request.json")).unwrap())
            .unwrap();
    let expected: Value = serde_json::from_slice(
        &fs::read(root().join("legacy-timing-mixed.expected.json")).unwrap(),
    )
    .unwrap();
    let bytes = request["Content"].as_str().unwrap().as_bytes();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, bytes).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let options = || {
        let mut options = ChartOptions::balanced("dance-double");
        options.seed = 42;
        options.expressed = ExpressedConfig {
            bracket_parsing_determination: BracketDetermination::UseDefaultMethod,
            ..ExpressedConfig::default()
        };
        options.performed = serde_json::from_value(request["PerformedConfig"].clone()).unwrap();
        options
    };
    assert!(
        ChartTask::capture(&file.session, 0, options())
            .unwrap()
            .run(&AtomicBool::new(false), |_| {})
            .is_err()
    );
    assert_eq!(file.session.serialize().unwrap(), bytes);
    Draft::capture(&file.session)
        .unwrap()
        .apply(&mut file.session)
        .unwrap();
    let mut grids = Vec::new();
    for (chart, reference) in expected.as_array().unwrap().iter().enumerate() {
        let result = ChartTask::capture(&file.session, chart, options())
            .unwrap()
            .run(&AtomicBool::new(false), |_| {});
        let reference = &reference["Editor"]["Conversion"];
        assert_eq!(result.is_ok(), reference["Success"].as_bool().unwrap());
        let preview = result.unwrap();
        let mut actual = preview
            .grid()
            .iter()
            .map(|(position, note)| {
                assert_eq!(note.kind, deadsync_editor::notes::Kind::Tap);
                (position.row, u64::from(position.lane), "1".to_owned())
            })
            .collect::<Vec<_>>();
        let mut reference = reference["Notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|note| {
                (
                    note["Row"].as_u64().unwrap(),
                    note["Lane"].as_u64().unwrap(),
                    note["Symbol"].as_str().unwrap().to_owned(),
                )
            })
            .collect::<Vec<_>>();
        actual.sort();
        reference.sort();
        reference.dedup();
        assert_eq!(actual, reference, "chart {chart}");
        let index = preview.apply(&mut file.session).unwrap();
        grids.push((index, preview.grid().clone()));
    }
    assert_eq!(fs::read(&path).unwrap(), bytes);
    file.save().unwrap();
    let mut reopened = FileSession::open(&path).unwrap();
    for (index, grid) in &grids {
        assert_eq!(reopened.session.grid(*index).unwrap(), grid);
    }
    for _ in 0..grids.len() + 1 {
        file.session.undo().unwrap();
    }
    assert_eq!(file.session.serialize().unwrap(), bytes);
}
