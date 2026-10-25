use deadsync_editor::notes::{Grid, Kind, Note, Position};

fn pos(player: u32, lane: u16, row: u64) -> Position {
    Position { player, lane, row }
}

#[test]
fn detailed_overview_is_identical_to_visible_and_preserves_crossing_sustains() {
    let mut grid = Grid::new(4, 2).unwrap();
    grid.set(
        pos(0, 0, 0),
        Some(Note {
            kind: Kind::Hold,
            end: Some(100),
            ..Note::tap()
        }),
    )
    .unwrap();
    for player in 0..2 {
        for lane in 0..4 {
            for row in 101..200 {
                grid.set(pos(player, lane, row), Some(Note::tap())).unwrap();
            }
        }
    }
    let expected: Vec<_> = grid.visible(50, 200).collect();
    assert_eq!(grid.visible_overview(50, 200, 151).unwrap(), expected);
    assert_eq!(grid.visible_overview(50, 200, 4096).unwrap(), expected);
    assert_eq!(grid.visible_overview(50, 200, 1).unwrap()[0].0.row, 0);
}

#[test]
fn dense_overview_has_bounded_output_per_track_and_retains_first_and_last_heads() {
    let mut grid = Grid::new(2, 2).unwrap();
    for player in 0..2 {
        for lane in 0..2 {
            for row in 0..10000 {
                grid.set(pos(player, lane, row), Some(Note::tap())).unwrap();
            }
        }
    }
    let notes = grid.visible_overview(0, 9999, 10).unwrap();
    assert_eq!(notes.len(), 4 * 10 * 2);
    for player in 0..2 {
        for lane in 0..2 {
            for bin in 0..10 {
                assert!(
                    notes
                        .iter()
                        .any(|(p, _)| *p == pos(player, lane, bin * 1000))
                );
                assert!(
                    notes
                        .iter()
                        .any(|(p, _)| *p == pos(player, lane, bin * 1000 + 999))
                );
            }
        }
    }
    assert_eq!(grid.iter().count(), 40000);
}

#[test]
fn overview_handles_full_integer_domain_and_rejects_invalid_queries() {
    let mut grid = Grid::new(1, 1).unwrap();
    for row in [0, 1, u64::MAX - 1, u64::MAX] {
        grid.set(pos(0, 0, row), Some(Note::tap())).unwrap();
    }
    assert_eq!(
        grid.visible_overview(0, u64::MAX, 2).unwrap(),
        grid.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        grid.visible_overview(u64::MAX, u64::MAX, 1).unwrap().len(),
        1
    );
    for (a, b, n) in [(1, 0, 1), (0, 1, 0), (0, 1, 4097)] {
        assert!(grid.visible_overview(a, b, n).is_err());
    }
}

#[test]
fn overview_keeps_sustain_extending_beyond_its_bin_and_viewport() {
    let mut grid = Grid::new(1, 1).unwrap();
    grid.set(pos(0, 0, 5), Some(Note::tap())).unwrap();
    grid.set(
        pos(0, 0, 10),
        Some(Note {
            kind: Kind::Roll,
            end: Some(2000),
            ..Note::tap()
        }),
    )
    .unwrap();
    let notes = grid.visible_overview(0, 1000, 10).unwrap();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[1].0.row, 10);
    assert_eq!(notes[1].1.end, Some(2000));
    let crossing = grid.visible_overview(100, 1000, 10).unwrap();
    assert_eq!(crossing.len(), 1);
    assert_eq!(crossing[0].0.row, 10);
}
