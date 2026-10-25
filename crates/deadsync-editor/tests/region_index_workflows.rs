use deadsync_editor::region_index::RegionIndex;

#[test]
fn indexed_regions_match_brute_force_with_overlaps_and_crossing_boundaries() {
    let mut regions: Vec<_> = (0..5000u64)
        .map(|i| (i * 7, i * 7 + (i * 31) % 997))
        .collect();
    regions.push((0, 100000));
    regions.reverse();
    let index = RegionIndex::new(regions.clone()).unwrap();
    regions.sort_unstable();
    for start in (0..40000).step_by(311) {
        let end = start + 137;
        let expected: Vec<_> = regions
            .iter()
            .copied()
            .filter(|&(a, b)| a <= end && b >= start)
            .collect();
        assert_eq!(index.visible(start, end, 4096).unwrap(), expected);
    }
}

#[test]
fn dense_overviews_obey_budget_and_keep_deterministic_order() {
    let index = RegionIndex::new((0..50000).map(|i| (i, 100000)).collect()).unwrap();
    for budget in [1, 17, 4096] {
        let visible = index.visible(75000, 80000, budget).unwrap();
        assert_eq!(visible.len(), budget);
        assert_eq!(visible.first(), Some(&(0, 100000)));
        assert_eq!(visible.last(), Some(&((budget - 1) as u64, 100000)));
    }
    assert!(index.visible(100001, u64::MAX, 4096).unwrap().is_empty());
}

#[test]
fn integer_edges_empty_indices_and_invalid_ranges_are_explicit() {
    let index = RegionIndex::new(vec![(0, u64::MAX), (u64::MAX, u64::MAX)]).unwrap();
    assert_eq!(
        index.visible(u64::MAX, u64::MAX, 4096).unwrap(),
        vec![(0, u64::MAX), (u64::MAX, u64::MAX)]
    );
    assert_eq!(index.visible(0, 0, 4096).unwrap(), vec![(0, u64::MAX)]);
    assert!(
        RegionIndex::default()
            .visible(0, u64::MAX, 4096)
            .unwrap()
            .is_empty()
    );
    assert!(RegionIndex::new(vec![(1, 0)]).is_err());
    for (a, b, n) in [(1, 0, 1), (0, 0, 0), (0, 0, 4097)] {
        assert!(index.visible(a, b, n).is_err());
    }
}

#[test]
fn final_region_bound_uses_cached_maximum_including_crossing_regions() {
    assert_eq!(RegionIndex::default().last_row(), 0);
    assert_eq!(
        RegionIndex::new(vec![(0, 10000), (48, 96), (200, 300)])
            .unwrap()
            .last_row(),
        10000
    );
}
