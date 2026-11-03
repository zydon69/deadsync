use deadsync_editor::notes;
use deadsync_editor::minimap_event_policy;
use minimap_event_policy::*;
use notes::Kind;
#[test]
fn original_event_classes_do_not_invent_keysound_attack_taps() {
    for kind in [Kind::Tap, Kind::Fake, Kind::Lift] {
        assert_eq!(note_shape(kind), Some(Shape::Short));
    }
    assert_eq!(note_shape(Kind::Mine), Some(Shape::Mine));
    assert_eq!(note_shape(Kind::Hold), Some(Shape::Sustain { roll: false }));
    assert_eq!(note_shape(Kind::Roll), Some(Shape::Sustain { roll: true }));
    for kind in [Kind::Keysound, Kind::Attack] {
        assert_eq!(note_shape(kind), None);
    }
}
#[test]
fn source_budget_counts_crossing_sustains_and_checks_after_add() {
    let mut budget = Budget::new(2, 1);
    assert!(!budget.stop_after(Kind::Attack, false));
    assert_eq!(budget.count(), 1);
    assert!(!budget.stop_after(Kind::Tap, false));
    assert_eq!(budget.count(), 2);
    assert!(budget.stop_after(Kind::Roll, false));
    assert_eq!(budget.count(), 3);
    let mut empty = Budget::new(0, 0);
    assert!(empty.stop_after(Kind::Mine, false));
    assert_eq!(empty.count(), 1);
    assert!(Budget::new(100, 0).stop_after(Kind::Hold, true));
    assert!(Budget::new(0, 2).stop_after(Kind::Attack, false));
}
#[test]
fn marker_visibility_and_adjacency_use_rows_not_time_equality() {
    assert!(!show_label(true, 0));
    assert!(!show_label(false, 48));
    assert!(show_label(true, 48));
    let p = |start_row, end_row, start_time, end_time| Pattern {
        start_row,
        end_row,
        start_time,
        end_time,
    };
    assert_eq!(
        merge_adjacent([
            p(0, 48, 0.0, 1.0),
            p(48, 96, 1.0, 2.0),
            p(120, 144, 2.0, 3.0)
        ]),
        vec![p(0, 96, 0.0, 2.0), p(120, 144, 2.0, 3.0)]
    );
    assert_eq!(
        merge_adjacent([p(0, 96, 0.0, 1.0), p(48, 144, 1.0, 2.0)]).len(),
        2
    );
}
