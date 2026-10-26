use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    notes::{Grid, Kind, Note, Position, minimap_query::rows_for_chart_time},
    playback::Clock,
};
use std::sync::Arc;
fn pos(player: u32, lane: u16, row: u64) -> Position {
    Position { player, lane, row }
}
#[test]
fn heap_order_keeps_later_tracks_and_crossing_sustains_without_duplicates() {
    let mut grid = Grid::new(4, 2).unwrap();
    grid.set(
        pos(0, 0, 0),
        Some(Note {
            kind: Kind::Hold,
            end: Some(100),
            attributes: Arc::from(""),
            tail_attributes: Arc::from(""),
        }),
    )
    .unwrap();
    for (player, lane, row) in [(0, 0, 120), (0, 1, 50), (1, 3, 49), (1, 2, 50), (0, 2, 70)] {
        grid.set(pos(player, lane, row), Some(Note::tap())).unwrap();
    }
    let query = grid.minimap_chronological(48, 200, 3, 16).unwrap();
    assert_eq!(
        query.crossings.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
        vec![pos(0, 0, 0)]
    );
    assert_eq!(
        query.heads.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
        vec![pos(1, 3, 49), pos(0, 1, 50), pos(1, 2, 50)]
    );
    assert!(!query.complete);
    let mut ties = Grid::new(4, 3).unwrap();
    ties.set(pos(0, 3, 48), Some(Note::tap())).unwrap();
    ties.set(pos(1, 0, 48), Some(Note::tap())).unwrap();
    ties.set(pos(2, 0, 48), Some(Note::tap())).unwrap();
    ties.set(
        pos(0, 0, 48),
        Some(Note {
            kind: Kind::Mine,
            end: None,
            attributes: Arc::from(""),
            tail_attributes: Arc::from(""),
        }),
    )
    .unwrap();
    let ordered = ties.minimap_chronological(48, 48, 10, 16).unwrap();
    assert_eq!(
        ordered.heads.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
        vec![pos(1, 0, 48), pos(2, 0, 48), pos(0, 0, 48), pos(0, 3, 48)],
        "lane before event class before player, as source comparer"
    );
    let full = grid.minimap_chronological(48, 200, 100, 16).unwrap();
    assert!(full.complete);
    assert_eq!(full.heads.len(), 5);
    assert_eq!(
        grid.minimap_chronological(0, 0, 1, 16)
            .unwrap()
            .crossings
            .len(),
        0
    );
    assert!(!grid.minimap_chronological(48, 200, 0, 16).unwrap().complete);
    assert!(grid.minimap_chronological(48, 200, 3, 1).is_err());
    assert!(grid.minimap_chronological(200, 48, 3, 16).is_err());
}
#[test]
fn query_work_does_not_walk_preceding_dense_history_and_handles_last_row() {
    let mut grid = Grid::new(4, 1).unwrap();
    for row in 0..10000 {
        grid.set(pos(0, 0, row), Some(Note::tap())).unwrap();
    }
    grid.set(pos(0, 3, 9999), Some(Note::tap())).unwrap();
    let q = grid.minimap_chronological(9998, 10000, 4, 16).unwrap();
    assert_eq!(q.heads.len(), 3);
    assert!(q.complete);
    assert!(q.tree_queries <= 10);
    grid.set(pos(0, 1, u64::MAX), Some(Note::tap())).unwrap();
    let q = grid
        .minimap_chronological(u64::MAX, u64::MAX, 4, 16)
        .unwrap();
    assert_eq!(q.heads[0].0.row, u64::MAX);
    assert!(q.complete);
}
#[test]
fn time_window_matches_brute_note_phase_through_warps_delays_stops_and_offsets() {
    for timing in [
        "#WARPS:3=2;#STOPS:1=1;#DELAYS:2=0.25;",
        "#STOPS:1=-0.75;#DELAYS:4=0.5;#WARPS:6=1;",
    ] {
        let source = format!(
            "#VERSION:0.83;#OFFSET:0.25;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0.1}};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;{timing}#NOTES:1000;"
        );
        let session = Session::new(Document::parse(source.as_bytes(), Format::Ssc).unwrap());
        let clock = Clock::capture(&session, 0).unwrap();
        let times: Vec<_> = (0..=480).map(|row| clock.note_time(row).unwrap()).collect();
        assert!(times.windows(2).all(|pair| pair[0] <= pair[1]));
        for (start, end) in [
            (-1.0, -0.5),
            (0.0, 0.0),
            (0.2, 0.7),
            (1.0, 3.0),
            (times[160], times[160]),
            (times[288], times[288]),
            (100.0, 101.0),
        ] {
            let expected: Vec<_> = times
                .iter()
                .enumerate()
                .filter(|(_, time)| **time >= start && **time <= end)
                .map(|(row, _)| row as u64)
                .collect();
            let actual = rows_for_chart_time(&clock, start, end, 480).unwrap();
            assert_eq!(
                actual,
                expected
                    .first()
                    .zip(expected.last())
                    .map(|(first, last)| (*first, *last)),
                "timing{timing} [{start},{end}]"
            );
        }
        let time = clock.note_time(160).unwrap();
        let (first, last) = rows_for_chart_time(&clock, time, time, 480)
            .unwrap()
            .unwrap();
        assert!(first <= 160 && last >= 160);
        assert!(rows_for_chart_time(&clock, f64::NAN, 1.0, 480).is_err());
    }
}
