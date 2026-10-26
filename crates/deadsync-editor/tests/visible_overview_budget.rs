use deadsync_editor::notes::{Grid, Kind, Note, Position};
fn pos(player: u32, lane: u16, row: u64) -> Position {
    Position { player, lane, row }
}
#[test]
fn ample_budget_preserves_track_bin_order_and_first_last_sampling() {
    let mut grid = Grid::new(4, 2).unwrap();
    for (player, lane, rows) in [
        (0, 0, vec![0, 1, 2, 3, 4, 5]),
        (0, 2, vec![0, 4]),
        (1, 1, vec![2, 3]),
    ] {
        for row in rows {
            grid.set(pos(player, lane, row), Some(Note::tap())).unwrap();
        }
    }
    let expected = [
        pos(0, 0, 0),
        pos(0, 0, 2),
        pos(0, 0, 3),
        pos(0, 0, 5),
        pos(0, 2, 0),
        pos(0, 2, 4),
        pos(1, 1, 2),
        pos(1, 1, 3),
    ];
    let full = grid.visible_overview_limited(0, 5, 2, 100).unwrap();
    assert_eq!(
        full.iter()
            .map(|(position, _)| *position)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(full, grid.visible_overview(0, 5, 2).unwrap());
    for limit in 0..=expected.len() + 1 {
        let small = grid.visible_overview_limited(0, 5, 2, limit).unwrap();
        assert!(small.len() <= limit);
        assert_eq!(
            small
                .iter()
                .map(|(position, _)| *position)
                .collect::<Vec<_>>(),
            expected[..limit.min(expected.len())]
        );
        assert_eq!(
            small,
            grid.visible_overview_limited(0, 5, 2, limit).unwrap()
        );
    }
}
#[test]
fn crossing_sustain_precedes_visible_heads_and_survives_small_budget() {
    let mut grid = Grid::new(4, 2).unwrap();
    grid.set(
        pos(0, 0, 0),
        Some(Note {
            kind: Kind::Hold,
            end: Some(96),
            ..Note::tap()
        }),
    )
    .unwrap();
    grid.set(pos(0, 1, 60), Some(Note::tap())).unwrap();
    grid.set(
        pos(1, 2, 0),
        Some(Note {
            kind: Kind::Hold,
            end: Some(192),
            ..Note::tap()
        }),
    )
    .unwrap();
    let one = grid.visible_overview_limited(48, 144, 8, 1).unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].0, pos(0, 0, 0));
    assert_eq!(one[0].1.end, Some(96));
    let full = grid.visible_overview_limited(48, 144, 8, 100).unwrap();
    assert_eq!(
        full.iter()
            .map(|(position, _)| *position)
            .collect::<Vec<_>>(),
        [pos(0, 0, 0), pos(0, 1, 60), pos(1, 2, 0)]
    );
    assert!(grid.visible_overview_limited(100, 0, 8, 0).is_err());
    assert!(grid.visible_overview_limited(0, 100, 0, 0).is_err());
}
#[test]
fn dense_multiplayer_sampling_is_capped_without_modifying_any_notes() {
    let mut grid = Grid::new(8, 32).unwrap();
    for player in 0..32 {
        for lane in 0..8 {
            for row in 0..100 {
                grid.set(pos(player, lane, row), Some(Note::tap())).unwrap();
            }
        }
    }
    let before = grid.clone();
    let visible = grid.visible_overview_limited(0, 99, 64, 17).unwrap();
    assert_eq!(visible.len(), 17);
    assert_eq!(grid, before);
}

#[test]
fn empty_track_bin_queries_are_bounded_even_when_no_note_is_visible() {
    let mut grid = Grid::new(8, 128).unwrap();
    for player in 0..128 {
        for lane in 0..8 {
            grid.set(pos(player, lane, 10000), Some(Note::tap()))
                .unwrap();
        }
    }
    for limit in [0, 1, 2, 17, 100] {
        let result = grid
            .visible_overview_budgeted(0, 99, 64, 1000, limit)
            .unwrap();
        assert!(result.notes.is_empty());
        assert_eq!(result.queries, limit);
        assert!(result.budget_exhausted);
    }
}
#[test]
fn track_bin_query_statistics_and_sufficient_budgets_preserve_existing_results() {
    let mut grid = Grid::new(4, 2).unwrap();
    grid.set(
        pos(0, 0, 0),
        Some(Note {
            kind: Kind::Hold,
            end: Some(96),
            ..Note::tap()
        }),
    )
    .unwrap();
    grid.set(pos(0, 1, 60), Some(Note::tap())).unwrap();
    grid.set(pos(1, 2, 72), Some(Note::tap())).unwrap();
    let full = grid
        .visible_overview_budgeted(48, 144, 8, 100, 100)
        .unwrap();
    assert_eq!(full.notes, grid.visible_overview(48, 144, 8).unwrap());
    assert_eq!(full.queries, 27); // Three tracks, crossing query plus eight bins each.
    assert!(!full.budget_exhausted);
    let first = grid.visible_overview_budgeted(48, 144, 8, 100, 1).unwrap();
    assert_eq!(first.notes.len(), 1);
    assert_eq!(first.notes[0].0, pos(0, 0, 0));
    assert_eq!(first.queries, 1);
    for limit in 0..=27 {
        let result = grid
            .visible_overview_budgeted(48, 144, 8, 100, limit)
            .unwrap();
        assert!(result.queries <= limit);
        assert_eq!(result.notes, &full.notes[..result.notes.len()]);
        let repeated = grid
            .visible_overview_budgeted(48, 144, 8, 100, limit)
            .unwrap();
        assert_eq!(result.notes, repeated.notes);
        assert_eq!(result.queries, repeated.queries);
    }
}
