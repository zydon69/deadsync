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
