#[path = "support/source_output_context.rs"]
mod source_output_context;
use deadsync_editor::{
    document::{Document, Format},
    generation::{
        editor_timing::EditorTimeline,
        source,
        timing::{Event, EventKind, GenerationTiming},
    },
    history::Session,
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
fn check(bytes: &[u8], format: Format, expected: &Value, name: &str) {
    let cancelled = AtomicBool::new(false);
    for (index, reference) in expected.as_array().unwrap().iter().enumerate() {
        let timing = GenerationTiming::from_simfile(bytes, format, index).unwrap();
        let rows = reference["FileNotes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|note| note["Row"].as_u64().unwrap())
            .collect::<Vec<_>>();
        let times = timing.times(rows.clone()).unwrap();
        let groups = timing.grouped_times(rows).unwrap();
        for ((time, group), note) in times
            .iter()
            .zip(&groups)
            .zip(reference["FileNotes"].as_array().unwrap())
        {
            assert_eq!(
                *time,
                note["Time"].as_f64().unwrap(),
                "{name} file chart {index} row {}",
                note["Row"]
            );
            assert_eq!(
                *group,
                note["GroupTime"].as_f64().unwrap(),
                "{name} group chart {index} row {}",
                note["Row"]
            );
        }
        let selected = source::read(Arc::from(bytes), format, index, &cancelled).unwrap();
        let timeline =
            EditorTimeline::build(&selected.timing, &selected.grid, None, &cancelled).unwrap();
        let editor = &reference["Editor"];
        assert_eq!(
            timeline.heads().len(),
            editor["Heads"].as_array().unwrap().len()
        );
        for (head, node) in timeline
            .heads()
            .iter()
            .zip(editor["Heads"].as_array().unwrap())
        {
            assert_eq!(head.position.row, node["Row"].as_u64().unwrap());
            assert_eq!(
                u64::from(head.position.lane),
                node["Lane"].as_u64().unwrap()
            );
            assert_eq!(
                head.time,
                node["Time"].as_f64().unwrap(),
                "{name} editor chart {index} row {}",
                node["Row"]
            );
            assert_eq!(
                head.fake,
                node["Fake"].as_bool().unwrap(),
                "{name} fake chart {index} row {}",
                node["Row"]
            );
        }
        assert_eq!(
            timeline.step_count() as u64,
            editor["StepCount"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            timeline.chart_end().unwrap(),
            editor["End"].as_f64().unwrap(),
            "{name}"
        );
        for (row, time) in editor["Times"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                timeline.position_time((row * 12) as u64).unwrap(),
                time.as_f64().unwrap(),
                "{name} query chart {index} row {}",
                row * 12
            );
        }
        let last = reference["EditorInput"]["LastStep"].as_u64().unwrap();
        let steps = reference["EditorInput"]["NumSteps"].as_u64().unwrap();
        assert_eq!(
            timeline.pattern_nps(steps, last).unwrap(),
            editor["Nps"].as_f64().unwrap(),
            "{name} NPS"
        );
    }
}
#[test]
fn normalized_source_matches_real_reader_and_original_editor_constructors_fallback_and_dependencies()
 {
    let mut count = 0;
    for file in fs::read_dir(root()).unwrap() {
        let path = file.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("source-timing-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root().join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let bytes = request["Content"].as_str().unwrap().as_bytes();
        let format = if request["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        if expected.get("Error").is_some() {
            assert_eq!(expected["Error"], "ArgumentException");
            let document = Document::parse(Arc::<[u8]>::from(bytes), format).unwrap();
            for chart in 0..document.charts().len() {
                assert!(GenerationTiming::from_simfile(bytes, format, chart).is_err());
            }
        } else {
            check(bytes, format, &expected, &name);
        }
        let source = Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap());
        if name == "source-timing-display-invalid-range.request.json" {
            let draft = deadsync_editor::timing_edit::TempoDraft::capture(
                &source,
                0,
                deadsync_editor::timing_edit::Scope::Chart,
            )
            .unwrap();
            assert_eq!(
                draft.tempos().at(0),
                expected[0]["StartingTempo"].as_f64().unwrap()
            );
        }
        assert_eq!(
            source.serialize().unwrap(),
            bytes,
            "{name} source unchanged"
        );
    }
    assert_eq!(count, 14);
}
#[test]
fn normalization_does_not_rewrite_legacy_bytes_notes_or_history_during_save_and_reopen() {
    let bytes=b"// source\n#VERSION:0.83;#TITLE:caf\xe9;#BPMS:0=120;#WARPS:1=0.00001;#FAKES:2=0.00001;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000\n0100\n0010\n0001;";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, bytes).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    workspace.execute(Command::Place(2)).unwrap();
    workspace.execute(Command::Undo).unwrap();
    let redo = workspace.file().session.redo_label().unwrap().to_owned();
    let selected = source::read(
        Arc::from(workspace.file().session.serialize().unwrap()),
        Format::Ssc,
        0,
        &AtomicBool::new(false),
    )
    .unwrap();
    let timeline = EditorTimeline::build(
        &selected.timing,
        &selected.grid,
        None,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(timeline.heads().len(), 4);
    assert_eq!(workspace.file().session.serialize().unwrap(), bytes);
    assert_eq!(workspace.file().session.redo_label(), Some(redo.as_str()));
    workspace.execute(Command::Save).unwrap();
    assert_eq!(workspace.file().session.serialize().unwrap(), bytes);
    let published = fs::read(&path).unwrap();
    source_output_context::assert_context(
        bytes,
        &published,
        Format::Ssc,
        source_output_context::Context {
            sync_offset: 0.009,
            chart_type: "dance_single",
            chart_index: 0,
        },
    );
    assert_eq!(
        Workspace::open(&path, 0)
            .unwrap()
            .file()
            .session
            .serialize()
            .unwrap(),
        published
    );
}
#[test]
fn explicit_event_constructor_remains_a_single_clock_and_nonfinite_source_is_rejected() {
    let timing = GenerationTiming::new(vec![
        Event {
            row: 0,
            kind: EventKind::Tempo(120.0),
        },
        Event {
            row: 48,
            kind: EventKind::Warp(0),
        },
    ])
    .unwrap();
    assert_eq!(timing.times([0, 48, 96]).unwrap(), [0.0, 0.5, 1.0]);
    for value in ["NaN", "inf", "-inf"] {
        let bytes = format!(
            "#VERSION:0.83;#BPMS:0={value};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
        );
        assert!(GenerationTiming::from_simfile(bytes.as_bytes(), Format::Ssc, 0).is_err());
    }
}

#[test]
fn fractional_source_chart_generation_matches_original_and_applies_saves_reopens_and_undoes() {
    use deadsync_editor::{
        generation::{
            config::{BracketDetermination, ExpressedConfig},
            job::{ChartOptions, ChartTask},
        },
        storage::FileSession,
    };
    for name in ["source-timing-half-rows", "source-timing-tiny-regions"] {
        let request: Value =
            serde_json::from_slice(&fs::read(root().join(format!("{name}.request.json"))).unwrap())
                .unwrap();
        let reference: Value = serde_json::from_slice(
            &fs::read(root().join(format!("{name}.expected.json"))).unwrap(),
        )
        .unwrap();
        let bytes = request["Content"].as_str().unwrap().as_bytes();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.ssc");
        fs::write(&path, bytes).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        let mut options = ChartOptions::balanced("dance-double");
        options.seed = 42;
        options.expressed = ExpressedConfig {
            bracket_parsing_determination: BracketDetermination::UseDefaultMethod,
            ..ExpressedConfig::default()
        };
        options.performed = serde_json::from_value(request["PerformedConfig"].clone()).unwrap();
        let result = ChartTask::capture(&file.session, 0, options)
            .unwrap()
            .run(&AtomicBool::new(false), |_| {});
        let expected = &reference[0]["Editor"]["Conversion"];
        assert_eq!(
            result.is_ok(),
            expected["Success"].as_bool().unwrap(),
            "{name}"
        );
        let preview = result.unwrap();
        let mut actual = preview
            .grid()
            .iter()
            .map(|(position, note)| {
                assert_eq!(note.kind, deadsync_editor::notes::Kind::Tap);
                (position.row, u64::from(position.lane), "1".to_owned())
            })
            .collect::<Vec<_>>();
        let mut expected = expected["Notes"]
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
        expected.sort();
        expected.dedup();
        assert_eq!(actual, expected, "{name}");
        assert_eq!(file.session.serialize().unwrap(), bytes);
        let index = preview.apply(&mut file.session).unwrap();
        let generated = file.session.grid(index).unwrap().clone();
        file.save().unwrap();
        let mut reopened = FileSession::open(&path).unwrap();
        assert_eq!(reopened.session.grid(index).unwrap(), &generated);
        file.session.undo().unwrap();
        assert_eq!(file.session.serialize().unwrap(), bytes);
        assert_eq!(
            fs::read(&path).unwrap(),
            reopened.session.serialize().unwrap()
        );
    }
}
