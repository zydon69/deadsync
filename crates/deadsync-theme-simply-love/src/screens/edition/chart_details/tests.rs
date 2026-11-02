use super::*;
use deadlib_platform::input::RawKeyboardEvent;
use deadsync_editor::{
    key_binding_preferences::{Action, Preferences},
    notes::{Grid, Kind, Note, Position},
};
use std::time::{Duration, Instant};
fn press(state: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
fn fixture(layout: &str) -> (tempfile::TempDir, State) {
    i18n::init_for_tests();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("song.ssc");
    let columns = deadsync_editor::notes::layout_columns(layout).unwrap();
    let routine = matches!(layout, "dance-routine" | "pump-routine" | "smx-team");
    let mut grid = Grid::new(columns, if routine { 2 } else { 1 }).unwrap();
    for (row, lane, kind, end) in [
        (0, 0, Kind::Tap, None),
        (0, columns - 1, Kind::Tap, None),
        (48, 0, Kind::Hold, Some(72)),
        (96, 0, Kind::Roll, Some(120)),
        (144, 0, Kind::Lift, None),
        (192, 0, Kind::Mine, None),
        (240, 0, Kind::Fake, None),
    ] {
        let mut note = Note::tap();
        note.kind = kind;
        note.end = end;
        grid.set(
            Position {
                row,
                lane,
                player: 0,
            },
            Some(note),
        )
        .unwrap();
    }
    if routine {
        grid.set(
            Position {
                row: 0,
                lane: 0,
                player: 1,
            },
            Some(Note::tap()),
        )
        .unwrap();
    }
    let mut source=format!("#VERSION:0.83;#TITLE:Details;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:{layout};#DIFFICULTY:Challenge;#METER:12;#DESCRIPTION:Z;#NOTES:").into_bytes();
    source.extend(grid.serialize().unwrap());
    source.extend(
        format!(
            ";#NOTEDATA:;#STEPSTYPE:{layout};#DIFFICULTY:Easy;#METER:1;#DESCRIPTION:A;#NOTES:{};",
            "0".repeat(usize::from(columns))
        )
        .bytes(),
    );
    std::fs::write(&path, source).unwrap();
    (root, State::open(&path, 0).unwrap())
}
fn open(state: &mut State) {
    let mut bindings = Preferences::default();
    for chords in bindings.commands.values_mut() {
        *chords = vec![vec![]];
    }
    bindings.commands.insert(
        Action::OpenChartProperties.id().to_owned(),
        vec![vec!["F12".into()]],
    );
    bindings.validate().unwrap();
    state.configure_key_binding_preferences(Arc::new(bindings));
    press(state, KeyCode::F12);
    assert!(matches!(state.panel, Some(Panel::ChartProperties(0))));
    press(state, KeyCode::F6);
    assert!(state.chart_details.active);
    assert!(matches!(state.panel, Some(Panel::Analysis)));
    let deadline = Instant::now() + Duration::from_secs(5);
    while state.chart_details_current().is_none() {
        state.poll_chart_details();
        assert!(!state.chart_details.failed, "{}", state.status);
        assert!(
            Instant::now() < deadline,
            "worker timeout: {}",
            state.status
        );
        std::thread::yield_now();
    }
}
fn texts(actors: &[Actor], out: &mut Vec<String>) {
    for actor in actors {
        match actor {
            Actor::Text { content, .. } => out.push(content.as_str().to_owned()),
            Actor::Frame { children, .. } | Actor::Camera { children, .. } => texts(children, out),
            Actor::SharedFrame { children, .. }
            | Actor::SharedTransform { children, .. }
            | Actor::SharedCamera { children, .. } => texts(children, out),
            _ => {}
        }
    }
}
#[test]
fn physical_readouts_include_source_row_nps_counts_and_lane_percentages_all_layouts_en_fr() {
    for language in ["en", "fr"] {
        for layout in deadsync_editor::chart_order::TYPE_ORDER {
            let (_root, mut state) = fixture(layout);
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let bytes = state.workspace.file().session.serialize().unwrap();
            let undo = state
                .workspace
                .file()
                .session
                .undo_label()
                .map(str::to_owned);
            open(&mut state);
            let report = state.chart_details_current().unwrap();
            let routine = matches!(layout, "dance-routine" | "pump-routine" | "smx-team");
            assert_eq!(report.totals.notes, if routine { 6 } else { 5 });
            assert_eq!(report.totals.rows, 4);
            assert_eq!(
                (
                    report.totals.holds,
                    report.totals.rolls,
                    report.totals.lifts,
                    report.totals.mines,
                    report.totals.fakes
                ),
                (1, 1, 1, 1, 1)
            );
            assert_eq!(report.totals.lanes[0], if routine { 5 } else { 4 });
            assert_eq!(report.row_plot.peak, 2.);
            assert_eq!(report.step_plot.peak, if routine { 3. } else { 2.5 });
            let expected = i18n::tr_fmt(
                "Edition",
                "ChartDetailsNps",
                &[
                    ("step", if routine { "3.00" } else { "2.50" }),
                    ("row", "2.00"),
                ],
            );
            let mut rendered = Vec::new();
            for _ in 0..24 {
                let mut actors = Vec::new();
                state.analysis_actors(&mut actors);
                texts(&actors, &mut rendered);
                press(&mut state, KeyCode::ArrowDown);
            }
            assert!(rendered.iter().any(|text| text == expected.as_ref()));
            assert!(
                rendered
                    .iter()
                    .any(|text| text.contains(if language == "fr" {
                        "Colonne 1"
                    } else {
                        "Lane 1"
                    }) && text.contains("100%"))
            );
            assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
            assert_eq!(state.workspace.file().session.undo_label(), undo.as_deref());
            press(&mut state, KeyCode::Escape);
            assert!(matches!(state.panel, Some(Panel::ChartProperties(0))));
        }
    }
}
#[test]
fn source_details_cache_rejects_other_sorted_identity_and_changed_content() {
    let (_root, mut state) = fixture("dance-single");
    open(&mut state);
    let first = state.workspace.file().session.chart_ids()[0].clone();
    let sorted = state.workspace.ordered_charts(|a, b| a.cmp(b)).unwrap();
    assert_eq!(
        sorted,
        vec![1, 0],
        "display order differs from resident identity order"
    );
    assert!(state.focus_chart_at_time(sorted[0], 0.));
    assert_ne!(state.workspace.file().session.chart_ids()[1], first);
    assert!(
        state.chart_details_current().is_none(),
        "same source container is not same chart identity"
    );
    state.poll_chart_details();
    let deadline = Instant::now() + Duration::from_secs(5);
    while state.chart_details_current().is_none() {
        state.poll_chart_details();
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(state.chart_details_current().unwrap().totals.notes, 0);
    state.workspace.execute(Command::Place(0)).unwrap();
    assert!(
        state.chart_details_current().is_none(),
        "old immutable result cannot represent edited notes"
    );
}
