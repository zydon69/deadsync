use deadsync_editor::{
    document::{Document, Format},
    generation::{
        config::{ExpressedConfig, PatternConfig, PerformedConfig},
        pattern_job::{Options, Span, Task},
    },
    history::Session,
    notes::{Note, Position},
    storage::FileSession,
};
use std::sync::{Arc, atomic::AtomicBool};
const SONG:&[u8]=b"#VERSION:0.83;#BPMS:0=120;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000\n0100\n0010\n0001\n1000\n0100\n0010\n0001;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000;";
fn session() -> Session {
    Session::new(Document::parse(Arc::<[u8]>::from(SONG), Format::Ssc).unwrap())
}
fn options() -> Options {
    Options {
        span: Span {
            start: 48,
            end: 96,
            include_start: true,
            include_end: true,
        },
        seed: 42,
        expressed: ExpressedConfig::default(),
        performed: PerformedConfig::balanced(),
        pattern: PatternConfig::sixteenths(),
        ignore_preceding_distribution: false,
    }
}

#[test]
fn pattern_generation_preserves_scheduled_attacks_and_peer_charts() {
    use deadsync_editor::attacks::placement::Timeline;
    let input = String::from_utf8(SONG.to_vec()).unwrap().replace(
        "#VENDOR:keep;",
        "#VENDOR:keep;#ATTACKS:TIME=0.5:LEN=2:MODS=drunk:TIME=8:LEN=1:MODS=dizzy;",
    );
    let mut source = Session::new(Document::parse(input.as_bytes(), Format::Ssc).unwrap());
    let before = source.serialize().unwrap();
    let expected: Vec<Vec<_>> = (0..2)
        .map(|chart| {
            Timeline::capture(&source, chart)
                .unwrap()
                .markers()
                .iter()
                .map(|m| (m.row, m.modifiers.clone()))
                .collect()
        })
        .collect();
    let generated = Task::capture(&source, 0, options())
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    assert!(generated.apply(&mut source, 0).unwrap());
    let saved = source.serialize().unwrap();
    let reopened = Session::new(Document::parse(saved.clone(), Format::Ssc).unwrap());
    for chart in 0..2 {
        let actual: Vec<_> = Timeline::capture(&reopened, chart)
            .unwrap()
            .markers()
            .iter()
            .map(|m| (m.row, m.modifiers.clone()))
            .collect();
        assert_eq!(actual, expected[chart]);
    }
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
    source.redo().unwrap();
    assert_eq!(source.serialize().unwrap(), saved);
}
#[test]
fn preview_applies_once_undoes_and_saves_through_real_session() {
    let mut source = session();
    let before = source.serialize().unwrap();
    let preview = Task::capture(&source, 0, options())
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    assert_eq!(source.serialize().unwrap(), before);
    assert_eq!(preview.generated().steps.len(), 5);
    assert_eq!(preview.deletions().removed, 3);
    assert!(preview.average_nps() > 0.0);
    let expected = preview.grid().clone();
    assert!(preview.apply(&mut source, 0).unwrap());
    assert_eq!(source.grid(0).unwrap(), &expected);
    assert_eq!(source.undo_label(), Some("Generate pattern"));
    assert!(preview.apply(&mut source, 0).is_err());
    let after = source.serialize().unwrap();
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), before);
    source.redo().unwrap();
    assert_eq!(source.serialize().unwrap(), after);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    std::fs::write(&path, &before).unwrap();
    let other_chart = source.grid(1).unwrap().clone();
    let output = source.serialize_for_output().unwrap();
    let mut file = FileSession::open(&path).unwrap();
    file.session = source;
    file.save().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), output);
    let mut reopened = FileSession::open(&path).unwrap();
    let resident = Document::parse(after.as_slice(), Format::Ssc).unwrap();
    for tag in resident.tags() {
        assert_eq!(reopened.session.document().raw_property(tag.chart, &tag.name), resident.raw_property(tag.chart, &tag.name), "Resident field changed: {:?}/{}", tag.chart, tag.name);
    }
    assert_eq!(reopened.session.grid(0).unwrap(), &expected);
    assert_eq!(reopened.session.grid(1).unwrap(), &other_chart);
    assert_eq!(reopened.session.document().charts().len(), 2);
    assert!(String::from_utf8_lossy(&after).contains("#VENDOR:keep;"));
}
#[test]
fn pattern_preview_rejects_other_sessions_charts_and_modified_revisions() {
    let mut source = session();
    let preview = Task::capture(&source, 0, options())
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    let mut other = session();
    assert!(preview.apply(&mut other, 0).is_err());
    assert!(preview.apply(&mut source, 1).is_err());
    source
        .edit_notes(
            0,
            "Edit",
            [(
                Position {
                    player: 0,
                    row: 180,
                    lane: 0,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    assert!(preview.apply(&mut source, 0).is_err());
    source.undo().unwrap();
    assert!(preview.is_current(&source));
    preview.apply(&mut source, 0).unwrap();
}
#[test]
fn cancellation_and_invalid_spans_never_mutate_source() {
    let source = session();
    assert!(
        Task::capture(&source, 0, options())
            .unwrap()
            .run(&AtomicBool::new(true), |_, _| {})
            .is_err()
    );
    let mut invalid = options();
    invalid.span.end = invalid.span.start;
    invalid.span.include_start = false;
    assert!(Task::capture(&source, 0, invalid).is_err());
    assert!(Task::capture(&source, 9, options()).is_err());
    assert_eq!(source.serialize().unwrap(), SONG);
}
#[test]
fn endpoints_are_relative_to_marker_and_empty_patterns_are_rejected() {
    let span = Span {
        start: 5,
        end: 30,
        include_start: true,
        include_end: true,
    };
    assert_eq!(span.steps(12).unwrap(), (5, 29, 3));
    assert_eq!(
        Span {
            include_start: false,
            ..span
        }
        .steps(12)
        .unwrap(),
        (17, 29, 2)
    );
    assert_eq!(
        Span {
            include_end: false,
            ..span
        }
        .steps(12)
        .unwrap(),
        (5, 29, 3)
    );
    assert_eq!(
        Span {
            end: 29,
            include_end: false,
            ..span
        }
        .steps(12)
        .unwrap(),
        (5, 17, 2)
    );
    assert!(
        Span {
            end: 5,
            include_end: false,
            ..span
        }
        .steps(12)
        .is_err()
    );
    assert!(span.steps(0).is_err());
    assert!(
        Span {
            end: i32::MAX as u64 + 1,
            ..span
        }
        .steps(12)
        .is_err()
    );
}

#[test]
fn worker_preview_and_cancelled_ready_result_use_bounded_shared_lifecycle() {
    use deadsync_editor::generation::job::Phase;
    use std::time::{Duration, Instant};
    let source = session();
    let mut job = Task::capture(&source, 0, options())
        .unwrap()
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while job.progress().phase != Phase::Ready {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    job.cancel();
    loop {
        if let Some(result) = job.poll() {
            assert!(result.is_err());
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(job.poll().is_none());
    let mut job = Task::capture(&source, 0, options())
        .unwrap()
        .spawn()
        .unwrap();
    loop {
        if let Some(result) = job.poll() {
            assert!(result.unwrap().is_current(&source));
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(source.serialize().unwrap(), SONG);
}

#[test]
fn empty_surroundings_and_crossing_holds_are_prepared_without_partial_edits() {
    use deadsync_editor::notes::{Grid, Kind};
    for hold in [false, true] {
        let mut grid = Grid::new(4, 1).unwrap();
        if hold {
            grid.set(
                Position {
                    player: 0,
                    row: 0,
                    lane: 0,
                },
                Some(Note {
                    kind: Kind::Hold,
                    end: Some(192),
                    ..Note::tap()
                }),
            )
            .unwrap();
        }
        let mut bytes =
            b"#VERSION:0.83;#BPMS:0=150;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:"
                .to_vec();
        bytes.extend(grid.serialize().unwrap());
        bytes.push(b';');
        let mut source =
            Session::new(Document::parse(Arc::<[u8]>::from(bytes.clone()), Format::Ssc).unwrap());
        let mut opts = options();
        opts.span.start = if hold { 96 } else { 0 };
        opts.span.end = 144;
        let preview = Task::capture(&source, 0, opts)
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {})
            .unwrap();
        assert_eq!(source.serialize().unwrap(), bytes);
        assert_eq!(preview.deletions().shortened_holds, usize::from(hold));
        if hold {
            assert_eq!(
                preview
                    .grid()
                    .get(Position {
                        player: 0,
                        row: 0,
                        lane: 0
                    })
                    .unwrap()
                    .end,
                Some(84)
            );
        }
        preview.apply(&mut source, 0).unwrap();
        assert_eq!(source.grid(0).unwrap(), preview.grid());
        source.undo().unwrap();
        assert_eq!(source.serialize().unwrap(), bytes);
    }
}

#[test]
fn workspace_accepts_only_the_selected_chart_and_saves_one_gesture() {
    use deadsync_editor::workspace::{Command, Workspace};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    std::fs::write(&path, SONG).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    let preview = workspace
        .pattern_task(options())
        .unwrap()
        .run(&AtomicBool::new(false), |_, _| {})
        .unwrap();
    workspace.execute(Command::SelectChart(1)).unwrap();
    assert!(workspace.apply_generated_pattern(&preview).is_err());
    workspace.execute(Command::SelectChart(0)).unwrap();
    workspace.apply_generated_pattern(&preview).unwrap();
    assert_eq!(workspace.grid(), preview.grid());
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.file().session.serialize().unwrap(), SONG);
    workspace.execute(Command::Redo).unwrap();
    workspace.execute(Command::Save).unwrap();
    assert_eq!(Workspace::open(&path, 0).unwrap().grid(), preview.grid());
}
