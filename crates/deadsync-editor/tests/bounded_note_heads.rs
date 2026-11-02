use deadsync_editor::notes::{Grid, Position};
#[test]
fn resumed_head_chunks_equal_complete_track_order_without_duplicates() {
    let grid = Grid::parse_layout(
        b"1000\n0100\n0010\n0001\n,\n1000\n0100\n0010\n0001",
        "dance-single",
    )
    .unwrap();
    let expected: Vec<_> = grid.iter().map(|(position, _)| position).collect();
    for chunk in 1..5 {
        let mut cursor = None;
        let mut actual = Vec::new();
        loop {
            let next: Vec<_> = grid
                .iter_heads_after(cursor)
                .take(chunk)
                .map(|(position, _)| position)
                .collect();
            if next.is_empty() {
                break;
            }
            cursor = next.last().copied();
            actual.extend(next);
        }
        assert_eq!(actual, expected);
    }
    let cursor = Position {
        player: 0,
        lane: 0,
        row: u64::MAX,
    };
    assert!(
        grid.iter_heads_after(Some(cursor))
            .all(|(position, _)| position.lane > 0)
    );
    assert!(
        grid.iter_heads_after(Some(Position {
            player: u32::MAX,
            lane: u16::MAX,
            row: u64::MAX
        }))
        .next()
        .is_none()
    );
}

#[test]
fn chronological_chunks_include_all_lanes_and_players_without_duplicates() {
    use deadsync_editor::notes::Note;
    let mut grid = Grid::new(4, 2).unwrap();
    for row in [0, 48, 96, 192] {
        for player in 0..2 {
            for lane in 0..4 {
                grid.set(Position { row, lane, player }, Some(Note::tap()))
                    .unwrap();
            }
        }
    }
    let mut expected: Vec<_> = grid
        .iter()
        .map(|(p, _)| p)
        .filter(|p| (48..=96).contains(&p.row))
        .collect();
    expected.sort_by_key(|p| (p.row, p.lane, p.player));
    for chunk in 1..10 {
        let mut actual = Vec::new();
        let mut cursor = None;
        loop {
            let next: Vec<_> = grid
                .iter_heads_row_order(48, 96, cursor)
                .take(chunk)
                .map(|(p, _)| p)
                .collect();
            if next.is_empty() {
                break;
            }
            cursor = next.last().copied();
            actual.extend(next);
            assert!(actual.len() <= expected.len());
        }
        assert_eq!(actual, expected);
    }
    assert!(
        grid.iter_heads_row_order(
            48,
            96,
            Some(Position {
                row: u64::MAX,
                lane: 0,
                player: 0
            })
        )
        .next()
        .is_none()
    );
    assert!(grid.iter_heads_row_order(96, 48, None).next().is_none());
}
