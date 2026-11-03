use deadsync_editor::{
    document::{Document, Format},
    generation::{
        config::PatternConfig,
        metadata::{ChartData, Definition, Guid},
        pattern_batch::{Preview, Task},
        profiles::{Kind as ProfileKind, Profile},
    },
    history::Session,
    notes::{Grid, Kind, Note, Position},
    storage::FileSession,
    workspace::{Command, Workspace},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
fn fixture(name: &str) -> (Value, Value) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    (
        serde_json::from_slice(
            &fs::read(root.join(format!("pattern-batch-{name}.request.json"))).unwrap(),
        )
        .unwrap(),
        serde_json::from_slice(
            &fs::read(root.join(format!("pattern-batch-{name}.expected.json"))).unwrap(),
        )
        .unwrap(),
    )
}
fn source(input: &Value, format: Format, profiles: &Path) -> Session {
    let mut grid = Grid::new(4, 1).unwrap();
    for note in input["Notes"].as_array().unwrap() {
        grid.set(
            Position {
                player: 0,
                row: note[0].as_u64().unwrap(),
                lane: note[1].as_u64().unwrap() as u16,
            },
            Some(Note {
                kind: match note[2].as_str().unwrap() {
                    "1" => Kind::Tap,
                    "2" => Kind::Hold,
                    "4" => Kind::Roll,
                    "M" => Kind::Mine,
                    "F" => Kind::Fake,
                    "L" => Kind::Lift,
                    "K" => Kind::Keysound,
                    _ => panic!("Unknown test note"),
                },
                end: note.get(3).and_then(Value::as_u64),
                ..Note::tap()
            }),
        )
        .unwrap();
    }
    let mut bytes = b"#TITLE:Batch;#VENDOR:keep;#BPMS:0=120;".to_vec();
    for _ in 0..2 {
        bytes.extend_from_slice(match format {Format::Sm=>b"#NOTES:dance-single:source:Hard:9:0,0,0,0,0:",Format::Ssc=>b"#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#VENDOR_CHART:keep;#NOTES:"});
        bytes.extend_from_slice(&grid.serialize().unwrap());
        bytes.push(b';');
    }
    let mut source = Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap());
    for (kind, tag) in [
        ("tempo", "BPMS"),
        ("signature", "TIMESIGNATURES"),
        ("delay", "DELAYS"),
        ("stop", "STOPS"),
        ("warp", "WARPS"),
        ("fake", "FAKES"),
    ] {
        let values: Vec<_> = input["Timing"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event[1] == kind)
            .map(|event| {
                let beat = event[0].as_f64().unwrap() / 48.0;
                if kind == "signature" {
                    format!("{beat}={}={}", event[2], event[3])
                } else {
                    let value = event[2].as_f64().unwrap()
                        / if kind == "warp" || kind == "fake" {
                            48.0
                        } else {
                            1.0
                        };
                    format!("{beat}={value}")
                }
            })
            .collect();
        if !values.is_empty() {
            source.set_text(None, tag, &values.join(",")).unwrap();
        }
    }
    let performed=Profile::parse(ProfileKind::Performed,&serde_json::to_vec(&json!({"Guid":"11111111-2222-3333-4444-555555555555","Name":"Reference performed config","Config":input["PerformedConfig"]})).unwrap()).unwrap();
    performed.export_new(profiles).unwrap();
    let mut configurations = BTreeMap::new();
    let mut markers = BTreeMap::new();
    for marker in input["Schedule"].as_array().unwrap() {
        let spacing = marker[2].as_u64().unwrap();
        let guid = configurations
            .entry(spacing)
            .or_insert_with(|| {
                let mut config: PatternConfig =
                    serde_json::from_value(input["PatternConfig"].clone()).unwrap();
                config.beat_sub_division = (48 / spacing) as u32;
                let profile =
                    Profile::new_pattern(&format!("Reference spacing {spacing}"), config).unwrap();
                profile.export_new(profiles).unwrap();
                profile.guid().clone()
            })
            .clone();
        let row = marker[0].as_u64().unwrap();
        markers.insert(
            row,
            Definition {
                length: marker[1].as_u64().unwrap(),
                random_seed: 42 + (row % 11) as i32,
                ignore_preceding_distribution: input["IgnoreRows"]
                    .as_array()
                    .is_some_and(|rows| rows.iter().any(|item| item.as_u64() == Some(row))),
                pattern_config_guid: guid,
                performed_chart_config_guid: performed.guid().clone(),
                start_position_inclusive: marker[3].as_bool().unwrap(),
                end_position_inclusive: marker[4].as_bool().unwrap(),
                ..Definition::default()
            },
        );
    }
    ChartData {
        patterns: Some(markers),
        expressed_chart_config: "0c0ba200-8f90-4060-8912-e9ea65831ebc".parse().unwrap(),
        ..ChartData::default()
    }
    .save(&mut source, 0)
    .unwrap();
    source
}
fn run(source: &Session, profiles: &Path) -> Preview {
    Task::capture_all(source, 0, Some(profiles))
        .unwrap()
        .run(&AtomicBool::new(false), |_| {})
        .unwrap()
}
#[test]
fn sequential_pattern_sets_match_original_context_generation_clipping_and_insertion() {
    for name in [
        "disjoint",
        "adjacent",
        "overlap",
        "nested-short",
        "empty-next",
        "empty-next-context",
        "empty-next-double",
        "empty-final",
        "all-empty",
        "unsorted",
        "off-grid",
        "single-row",
        "spacing-one",
        "delayed-origin",
        "delayed-origin-sm",
        "ignore-distribution",
        "shared-transitions",
    ] {
        let (input, expected) = fixture(name);
        for format in input["Formats"].as_array().unwrap().iter().map(|format| {
            match format.as_str().unwrap() {
                "sm" => Format::Sm,
                "ssc" => Format::Ssc,
                _ => panic!("Unknown fixture format"),
            }
        }) {
            let folder = tempfile::tempdir().unwrap();
            let mut source = source(&input, format, folder.path());
            let before = source.serialize().unwrap();
            let other = source.grid(1).unwrap().clone();
            let result = run(&source, folder.path());
            assert!(
                (result.average_nps() - expected["Nps"].as_f64().unwrap()).abs() < 1e-9,
                "{name} {format:?} NPS: actual {} expected {}",
                result.average_nps(),
                expected["Nps"]
            );
            // The complete original worker, including constructor enumeration,
            // is the output oracle. The older Stages fixture executes a reconstructed
            // single-enumeration loop and remains only a helper/NPS comparison.
            assert_eq!(
                result.entries().len(),
                input["Schedule"].as_array().unwrap().len()
            );
            for actual in result.entries() {
                assert!(actual.error().is_none(), "{name}: {:?}", actual.error());
            }
            let worker_root =
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pattern-worker");
            let worker: Value = serde_json::from_slice(
                &fs::read(worker_root.join(format!("pattern-batch-{name}.expected.json"))).unwrap(),
            )
            .unwrap();
            assert_eq!(worker["Before"], worker["Undo"], "{name} original Undo");
            assert_eq!(worker["NoteData"], worker["Redo"], "{name} original Redo");
            let expected_grid =
                Grid::parse(worker["NoteData"].as_str().unwrap().as_bytes(), 4).unwrap();
            assert_eq!(
                result.grid(),
                &expected_grid,
                "{name} {format:?} complete exported grid"
            );
            assert_eq!(
                result.report().notes,
                result
                    .entries()
                    .iter()
                    .filter_map(|entry| entry.generated())
                    .map(|generated| generated.steps.len())
                    .sum::<usize>(),
                "{name} emitted tap count (reports stay per selected marker)"
            );
            if name == "adjacent" {
                assert_ne!(
                    worker["NoteData"], expected["NoteData"],
                    "source constructor double enumeration is observable"
                );
            }
            assert_eq!(source.serialize().unwrap(), before);
            let changed = result.apply(&mut source, 0).unwrap();
            assert_eq!(changed, result.has_changes());
            assert_eq!(source.grid(0).unwrap(), result.grid());
            assert_eq!(source.grid(1).unwrap(), &other);
            let after = source.serialize().unwrap();
            let mut reopened =
                Session::new(Document::parse(Arc::<[u8]>::from(after.clone()), format).unwrap());
            assert_eq!(reopened.grid(0).unwrap(), result.grid());
            if changed {
                source.undo().unwrap();
                assert_eq!(source.serialize().unwrap(), before);
                source.redo().unwrap();
                assert_eq!(source.serialize().unwrap(), after);
            }
        }
    }
}

#[test]
fn pattern_set_save_reopen_and_workspace_history_preserve_markers_and_other_charts() {
    let (input, _) = fixture("overlap");
    for format in [Format::Sm, Format::Ssc] {
        let folder = tempfile::tempdir().unwrap();
        let configs = folder.path().join("configs");
        let source = source(&input, format, &configs);
        let path = folder.path().join(format!("song.{}", format.extension()));
        let before = source.serialize().unwrap();
        fs::write(&path, &before).unwrap();
        let mut workspace = Workspace::open(&path, 0).unwrap();
        let document = workspace.file().session.document();
        let descriptor = &document.charts()[1];
        let other_chart = Grid::parse_layout(
            document.value(descriptor.notes.clone()),
            &descriptor.steps_type,
        )
        .unwrap();
        let metadata =
            serde_json::to_value(ChartData::read(workspace.file().session.document(), 0).unwrap())
                .unwrap();
        let result = workspace
            .all_patterns_task(Some(&configs))
            .unwrap()
            .run(&AtomicBool::new(false), |_| {})
            .unwrap();
        workspace.execute(Command::SelectChart(1)).unwrap();
        assert!(workspace.apply_pattern_set(&result).is_err());
        workspace.execute(Command::SelectChart(0)).unwrap();
        assert!(workspace.apply_pattern_set(&result).unwrap());
        assert_eq!(
            workspace.file().session.undo_label(),
            Some("Regenerate pattern set")
        );
        assert_eq!(
            serde_json::to_value(ChartData::read(workspace.file().session.document(), 0).unwrap())
                .unwrap(),
            metadata
        );
        let after = workspace.file().session.serialize().unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);
        workspace.execute(Command::Undo).unwrap();
        assert_eq!(workspace.file().session.serialize().unwrap(), before);
        workspace.execute(Command::Redo).unwrap();
        let output = workspace.file().session.serialize_for_output().unwrap();
        workspace.execute(Command::Save).unwrap();
        assert_eq!(fs::read(&path).unwrap(), output);
        let mut reopened = FileSession::open(&path).unwrap();
        // Native Save inserts the required song ownership metadata. Every
        // resident field and the unrelated chart must still be preserved.
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
        assert_eq!(reopened.session.grid(0).unwrap(), result.grid());
        assert_eq!(reopened.session.grid(1).unwrap(), &other_chart);
        assert_eq!(
            serde_json::to_value(ChartData::read(reopened.session.document(), 0).unwrap()).unwrap(),
            metadata
        );
    }
}
#[test]
fn cancellation_at_start_between_patterns_and_ready_never_changes_the_song() {
    let (input, _) = fixture("disjoint");
    let folder = tempfile::tempdir().unwrap();
    let source = source(&input, Format::Ssc, folder.path());
    let before = source.serialize().unwrap();
    assert!(
        Task::capture_all(&source, 0, Some(folder.path()))
            .unwrap()
            .run(&AtomicBool::new(true), |_| {})
            .is_err()
    );
    for ready in [false, true] {
        let cancelled = AtomicBool::new(false);
        assert!(
            Task::capture_all(&source, 0, Some(folder.path()))
                .unwrap()
                .run(&cancelled, |progress| {
                    if (ready && progress.phase == deadsync_editor::generation::job::Phase::Ready)
                        || (!ready && progress.completed == 1)
                    {
                        cancelled.store(true, Ordering::Relaxed);
                    }
                })
                .is_err()
        );
        assert_eq!(source.serialize().unwrap(), before);
    }
    let mut job = Task::capture_all(&source, 0, Some(folder.path()))
        .unwrap()
        .spawn()
        .unwrap();
    job.cancel();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if let Some(result) = job.poll() {
            assert!(result.is_err());
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(source.serialize().unwrap(), before);
}
#[test]
fn stale_foreign_and_missing_marker_requests_are_read_only_and_selection_is_normalized() {
    let (input, _) = fixture("disjoint");
    let folder = tempfile::tempdir().unwrap();
    let mut source = source(&input, Format::Ssc, folder.path());
    let before = source.serialize().unwrap();
    let task = Task::capture(&source, 0, [144, 48, 48], Some(folder.path())).unwrap();
    source.set_text(None, "TITLE", "Changed").unwrap();
    let changed = source.serialize().unwrap();
    let result = task.run(&AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(
        result
            .entries()
            .iter()
            .map(|entry| entry.row)
            .collect::<Vec<_>>(),
        vec![48, 144]
    );
    assert!(result.apply(&mut source, 0).is_err());
    assert_eq!(source.serialize().unwrap(), changed);
    let mut foreign =
        Session::new(Document::parse(Arc::<[u8]>::from(before.clone()), Format::Ssc).unwrap());
    assert!(result.apply(&mut foreign, 0).is_err());
    assert_eq!(foreign.serialize().unwrap(), before);
    assert!(Task::capture(&source, 0, [], None).is_err());
    assert!(Task::capture(&source, 99, [48], None).is_err());
    assert!(
        Task::capture(&source, 0, [49], Some(folder.path()))
            .unwrap()
            .run(&AtomicBool::new(false), |_| {})
            .is_err()
    );
    assert_eq!(source.serialize().unwrap(), changed);
}
#[test]
fn failures_keep_other_results_and_deleted_regions_require_explicit_acceptance() {
    let (input, _) = fixture("disjoint");
    let folder = tempfile::tempdir().unwrap();
    let mut source = source(&input, Format::Ssc, folder.path());
    let mut metadata = ChartData::read(source.document(), 0).unwrap().unwrap();
    metadata
        .patterns
        .as_mut()
        .unwrap()
        .get_mut(&48)
        .unwrap()
        .performed_chart_config_guid = Guid::default();
    metadata.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    let result = run(&source, folder.path());
    assert_eq!(result.report().failed, 1);
    assert_eq!(result.report().generated, 1);
    assert!(
        result.entries()[0]
            .error()
            .unwrap()
            .contains("Missing performed")
    );
    assert!(result.report().deletions.removed > 0);
    assert_eq!(source.serialize().unwrap(), before);
    result.apply(&mut source, 0).unwrap();
    assert_eq!(source.grid(0).unwrap(), result.grid());
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
    metadata
        .patterns
        .as_mut()
        .unwrap()
        .get_mut(&144)
        .unwrap()
        .performed_chart_config_guid = Guid::default();
    metadata.save(&mut source, 0).unwrap();
    let before = source.serialize().unwrap();
    let result = run(&source, folder.path());
    assert_eq!(result.report().failed, 2);
    assert_eq!(result.report().generated, 0);
    assert_eq!(source.serialize().unwrap(), before);
    assert!(result.apply(&mut source, 0).unwrap());
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
}
#[test]
fn empty_pattern_set_acceptance_is_a_noop_that_preserves_redo() {
    let (input, _) = fixture("all-empty");
    let folder = tempfile::tempdir().unwrap();
    let mut source = source(&input, Format::Ssc, folder.path());
    source.set_text(None, "TITLE", "Redo this").unwrap();
    source.undo().unwrap();
    let before = source.serialize().unwrap();
    let label = source.redo_label().unwrap().to_owned();
    let result = run(&source, folder.path());
    assert_eq!(result.report().empty, 2);
    assert!(!result.has_changes());
    assert!(!result.apply(&mut source, 0).unwrap());
    assert_eq!(source.serialize().unwrap(), before);
    assert_eq!(source.redo_label(), Some(label.as_str()));
    assert!(source.redo().unwrap());
}
#[test]
fn all_patterns_share_the_loaded_profile_snapshot_and_report_unrelated_invalid_files() {
    let (input, _) = fixture("disjoint");
    let folder = tempfile::tempdir().unwrap();
    let source = source(&input, Format::Ssc, folder.path());
    fs::write(folder.path().join("pcc-invalid.json"), b"invalid").unwrap();
    let performed = fs::read_dir(folder.path())
        .unwrap()
        .map(Result::unwrap)
        .map(|file| file.path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("pcc-")
                && path.file_name().unwrap() != "pcc-invalid.json"
        })
        .unwrap();
    let expected = run(&source, folder.path());
    let mut removed = false;
    let result = Task::capture_all(&source, 0, Some(folder.path()))
        .unwrap()
        .run(&AtomicBool::new(false), |progress| {
            if progress.completed == 1 && !removed {
                fs::remove_file(&performed).unwrap();
                removed = true;
            }
        })
        .unwrap();
    assert!(removed);
    assert_eq!(result.report().generated, 2);
    assert_eq!(result.profile_issues().len(), 1);
    assert_eq!(result.grid(), expected.grid());
    assert_eq!(run(&source, folder.path()).report().failed, 2);
}

#[test]
fn single_marker_task_matches_original_action_constructor_and_undo_redo() {
    let (mut input, _) = fixture("adjacent");
    input["Schedule"].as_array_mut().unwrap().truncate(1);
    let expected: Value = serde_json::from_str(include_str!(
        "fixtures/pattern-worker/pattern-single-adjacent.expected.json"
    ))
    .unwrap();
    for format in [Format::Sm, Format::Ssc] {
        let folder = tempfile::tempdir().unwrap();
        let mut session = source(&input, format, folder.path());
        let before = session.serialize().unwrap();
        let registry = deadsync_editor::generation::profiles::Loaded::directory(folder.path())
            .unwrap()
            .registry;
        let row = input["Schedule"][0][0].as_u64().unwrap();
        let options = ChartData::read(session.document(), 0)
            .unwrap()
            .unwrap()
            .options(row, &registry)
            .unwrap();
        let preview = deadsync_editor::generation::pattern_job::Task::capture(&session, 0, options)
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .unwrap();
        let expected_grid =
            Grid::parse(expected["NoteData"].as_str().unwrap().as_bytes(), 4).unwrap();
        assert_eq!(preview.grid(), &expected_grid, "{format:?}");
        assert_eq!(session.serialize().unwrap(), before);
        assert!(preview.apply(&mut session, 0).unwrap());
        let after = session.serialize().unwrap();
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), before);
        session.redo().unwrap();
        assert_eq!(session.serialize().unwrap(), after);
    }
}
