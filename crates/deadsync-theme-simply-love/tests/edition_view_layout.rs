#[path = "../src/screens/edition/view_layout.rs"]
mod view_layout;
#[test]
fn natural_width_neighbors_pack_around_focus_without_shrinking_all_tabs() {
    let views =
        view_layout::layout(&[(1, 4), (3, 8), (4, 4)], 3, 427.0, 644.0, 0.0, 854.0).unwrap();
    let focus = views.iter().find(|view| view.focused).unwrap();
    assert_eq!(focus.source, 3);
    assert_eq!((focus.left + focus.field_width * 0.5), 427.0);
    assert_eq!(focus.field_width, 384.0);
    assert_eq!(focus.lane_width, 48.0);
    let left = views.iter().find(|view| view.source == 1).unwrap();
    let right = views.iter().find(|view| view.source == 4).unwrap();
    assert!(left.right() < focus.left);
    assert!(focus.right() < right.left);
    assert!(focus.visible_x(focus.left + focus.field_width * 0.5));
}
#[test]
fn visible_geometry_is_bounded_by_viewport_instead_of_total_open_chart_count() {
    let charts: Vec<_> = (0..10000).map(|index| (index, 4)).collect();
    let views = view_layout::layout(&charts, 5000, 427.0, 644.0, 0.0, 854.0).unwrap();
    assert!(views.len() <= 5);
    assert!(
        views
            .iter()
            .all(|view| view.right() + 30.0 >= 0.0 && view.left - 42.0 <= 854.0)
    );
    assert!(views.iter().any(|view| view.source == 5000));
}
#[test]
fn view_order_can_include_source_gaps_and_repeated_focus_layout_changes() {
    for focused in [1, 7, 42] {
        let views = view_layout::layout(
            &[(42, 8), (1, 4), (7, 10)],
            focused,
            427.0,
            644.0,
            0.0,
            854.0,
        )
        .unwrap();
        assert_eq!(views.iter().filter(|view| view.focused).count(), 1);
        assert_eq!(
            views.iter().find(|view| view.focused).unwrap().source,
            focused
        );
        assert!(views.windows(2).all(|pair| pair[0].order < pair[1].order));
    }
    for arguments in [(f32::NAN, 644.0, 0.0, 854.0), (427.0, 644.0, 854.0, 0.0)] {
        assert!(
            view_layout::layout(
                &[(0, 4)],
                0,
                arguments.0,
                arguments.1,
                arguments.2,
                arguments.3
            )
            .is_err()
        );
    }
    assert!(view_layout::layout(&[(0, 0)], 0, 427.0, 644.0, 0.0, 854.0).is_err());
}
