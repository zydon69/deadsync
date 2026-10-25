use deadsync_editor::{
    document::{Document, Format},
    generation::job::{ChartOptions, ChartTask, Job, Phase},
    history::Session,
    notes::{Note, Position},
    storage::FileSession,
    workspace::{Command, Workspace},
};
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
const SONG: &[u8] = b"#VERSION:0.83;#TITLE:Original;#BPMS:0=120;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Source;#DIFFICULTY:Hard;#METER:9;#NOTES:1000\n0100\n0010\n0001;";
fn session() -> Session {
    Session::new(Document::parse(Arc::<[u8]>::from(SONG), Format::Ssc).unwrap())
}
fn preview(session: &Session) -> deadsync_editor::generation::job::ChartPreview {
    ChartTask::capture(session, 0, ChartOptions::balanced("dance-double"))
        .unwrap()
        .run(&AtomicBool::new(false), |_| {})
        .unwrap()
}
fn finish(
    job: &mut Job,
) -> Result<deadsync_editor::generation::job::ChartPreview, deadsync_editor::Error> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(result) = job.poll() {
            assert!(job.poll().is_none());
            return result;
        }
        assert!(
            Instant::now() < deadline,
            "Generation worker timed out: {:?}",
            job.progress()
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn background_preview_is_read_only_and_applies_saves_and_undoes_through_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    workspace.execute(Command::Place(2)).unwrap();
    let before = workspace.file().session.serialize().unwrap();
    let mut options = ChartOptions::balanced("dance-double");
    options.seed = 42;
    let mut job = workspace.generation_task(options).unwrap().spawn().unwrap();
    let generated = finish(&mut job).unwrap();
    assert_eq!(job.progress().phase, Phase::Ready);
    assert_eq!(workspace.file().session.serialize().unwrap(), before);
    assert_eq!(fs::read(&path).unwrap(), SONG);
    assert_eq!(generated.seed(), 42);
    assert_eq!(generated.target_layout(), "dance-double");
    assert_eq!(generated.report().notes, 5);
    assert!(!generated.report().empty);
    let grid = generated.grid().clone();
    assert_eq!(workspace.apply_generated(&generated).unwrap(), 1);
    assert_eq!(workspace.chart(), 1);
    assert_eq!(workspace.grid(), &grid);
    assert_eq!(
        workspace.file().session.undo_label(),
        Some("Generate chart")
    );
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.chart(), 0);
    assert_eq!(workspace.file().session.serialize().unwrap(), before);
    workspace.grid(); // structural undo must reload the active grid cache.
    workspace.execute(Command::Redo).unwrap();
    workspace.execute(Command::SelectChart(1)).unwrap();
    assert_eq!(workspace.grid(), &grid);
    workspace.execute(Command::Save).unwrap();
    let mut reopened = FileSession::open(&path).unwrap();
    assert_eq!(reopened.session.grid(1).unwrap(), &grid);
    assert!(reopened.session.serialize().unwrap().starts_with(&before));
}
#[test]
fn preview_identity_rejects_equal_revisions_other_sessions_and_recovered_documents() {
    let mut original = session();
    let generated = preview(&original);
    let mut other = session();
    assert_eq!(original.revision(), other.revision());
    assert!(generated.apply(&mut other).is_err());
    assert_eq!(other.serialize().unwrap(), SONG);
    original
        .edit_notes(
            0,
            "Edit",
            [(
                Position {
                    player: 0,
                    lane: 1,
                    row: 0,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let changed = original.serialize().unwrap();
    assert!(generated.apply(&mut original).is_err());
    assert_eq!(original.serialize().unwrap(), changed);
    original.undo().unwrap();
    assert!(generated.is_current(&original));
    generated.apply(&mut original).unwrap();
    assert!(generated.apply(&mut original).is_err());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let generated = preview(&file.session);
    file.save_recovery().unwrap();
    file.recover().unwrap();
    assert_eq!(file.session.revision(), 0);
    assert!(generated.apply(&mut file.session).is_err());
}
#[test]
fn snapshot_is_immutable_and_capture_errors_do_not_change_redo_or_file_bytes() {
    let mut source = session();
    let task = ChartTask::capture(&source, 0, ChartOptions::balanced("dance-double")).unwrap();
    source.set_text(Some(0), "METER", "12").unwrap();
    let generated = task.run(&AtomicBool::new(false), |_| {}).unwrap();
    assert!(!generated.is_current(&source));
    source.undo().unwrap();
    let redo = source.redo_label().unwrap().to_owned();
    assert!(ChartTask::capture(&source, 99, ChartOptions::balanced("dance-double")).is_err());
    assert!(ChartTask::capture(&source, 0, ChartOptions::balanced("dance-couple")).is_err());
    let mut invalid = ChartOptions::balanced("dance-double");
    invalid.performed.arrow_weights.clear();
    assert!(ChartTask::capture(&source, 0, invalid).is_err());
    assert_eq!(source.redo_label(), Some(redo.as_str()));
    assert_eq!(source.serialize().unwrap(), SONG);
}
#[test]
fn cancellation_before_during_and_after_computation_never_publishes_a_preview() {
    let source = session();
    assert!(
        ChartTask::capture(&source, 0, ChartOptions::balanced("dance-double"))
            .unwrap()
            .run(&AtomicBool::new(true), |_| {})
            .is_err()
    );
    for phase in [
        Phase::Preparing,
        Phase::Interpreting,
        Phase::Converting,
        Phase::Assembling,
        Phase::Ready,
    ] {
        let cancelled = AtomicBool::new(false);
        let result = ChartTask::capture(&source, 0, ChartOptions::balanced("dance-double"))
            .unwrap()
            .run(&cancelled, |progress| {
                if progress.phase == phase {
                    cancelled.store(true, Ordering::Relaxed);
                }
            });
        assert!(result.is_err(), "Cancellation at {phase:?}");
    }
    let mut job = ChartTask::capture(&source, 0, ChartOptions::balanced("dance-double"))
        .unwrap()
        .spawn()
        .unwrap();
    job.cancel();
    assert!(finish(&mut job).is_err());
    assert!(job.is_cancelled());
    assert_eq!(source.serialize().unwrap(), SONG);
}
#[test]
fn ignored_keysounds_and_empty_results_are_explicit_in_preview_report() {
    let bytes = b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:K000\n0000\n0000\n0000;";
    let source =
        Session::new(Document::parse(Arc::<[u8]>::from(bytes.as_slice()), Format::Ssc).unwrap());
    let generated = preview(&source);
    assert!(generated.report().empty);
    assert_eq!(generated.report().ignored_keysounds, 1);
    assert_eq!(generated.report().notes, 0);
    assert_eq!(source.document().charts().len(), 1);
}

#[test]
fn snapshot_pipeline_matches_original_file_reader_interpretation_and_generated_notes() {
    use deadsync_editor::{
        generation::config::{BracketDetermination, ExpressedConfig},
        notes::Kind,
    };
    use serde_json::Value;
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    for name in [
        "sm",
        "inherited",
        "own",
        "offset-only",
        "metadata",
        "ending-delay",
        "ending-scroll-delay",
    ] {
        let input: Value = serde_json::from_slice(
            &fs::read(root.join(format!("file-clock-{name}.request.json"))).unwrap(),
        )
        .unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(format!("file-clock-{name}.expected.json"))).unwrap(),
        )
        .unwrap();
        let format = if input["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let source = Session::new(
            Document::parse(
                Arc::<[u8]>::from(input["Content"].as_str().unwrap().as_bytes()),
                format,
            )
            .unwrap(),
        );
        let mut options = ChartOptions::balanced(input["TargetLayout"].as_str().unwrap());
        options.expressed = ExpressedConfig {
            bracket_parsing_determination: BracketDetermination::UseDefaultMethod,
            ..ExpressedConfig::default()
        };
        options.performed = serde_json::from_value(input["PerformedConfig"].clone()).unwrap();
        options.seed = input["Seed"].as_i64().unwrap() as i32;
        let generated = ChartTask::capture(&source, 0, options)
            .unwrap()
            .run(&AtomicBool::new(false), |_| {})
            .unwrap();
        let mut actual = Vec::new();
        for (position, note) in generated.grid().iter() {
            let symbol = match note.kind {
                Kind::Tap => "1",
                Kind::Hold => "2",
                Kind::Roll => "4",
                Kind::Mine => "M",
                Kind::Fake => "F",
                Kind::Lift => "L",
                _ => panic!("Unexpected generated note"),
            };
            actual.push((position.row, u64::from(position.lane), symbol.to_owned()));
            if let Some(end) = note.end {
                actual.push((end, u64::from(position.lane), "3".to_owned()));
            }
        }
        let mut reference = expected["ConvertedNotes"]
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
        assert_eq!(actual, reference, "{name}: source file to generated chart");
        assert_eq!(generated.report().ignored_keysounds, 1);
        assert_eq!(source.document().charts().len(), 1);
    }
}

#[test]
fn applying_requires_selected_source_and_mine_merges_are_disclosed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let mut bytes = SONG.to_vec();
    bytes.extend_from_slice(
        b"#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000\n0000\n0000\n0000;",
    );
    fs::write(&path, &bytes).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    let generated = workspace
        .generation_task(ChartOptions::balanced("dance-double"))
        .unwrap()
        .run(&AtomicBool::new(false), |_| {})
        .unwrap();
    workspace.execute(Command::SelectChart(1)).unwrap();
    assert!(workspace.apply_generated(&generated).is_err());
    assert_eq!(workspace.file().session.serialize().unwrap(), bytes);
    workspace.execute(Command::SelectChart(0)).unwrap();
    assert_eq!(workspace.apply_generated(&generated).unwrap(), 2);
    let bytes = b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:MM00\n00M0\n0000\n0000;";
    let source =
        Session::new(Document::parse(Arc::<[u8]>::from(bytes.as_slice()), Format::Ssc).unwrap());
    let mut options = ChartOptions::balanced("dance-double");
    options.seed = 19;
    let generated = ChartTask::capture(&source, 0, options)
        .unwrap()
        .run(&AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(generated.report().source_notes, 0);
    assert_eq!(generated.report().source_mines, 3);
    assert_eq!(generated.report().mines, 2);
    assert_eq!(generated.report().merged_mines, 1);
    assert_eq!(generated.report().dropped_mines, 0);
}

#[test]
fn cancellation_after_ready_discards_an_already_computed_result() {
    let source = session();
    let mut job = ChartTask::capture(&source, 0, ChartOptions::balanced("dance-double"))
        .unwrap()
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while job.progress().phase != Phase::Ready {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    job.cancel();
    assert!(finish(&mut job).is_err());
    assert_eq!(source.serialize().unwrap(), SONG);
}
