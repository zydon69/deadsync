use deadsync_editor::{
    minimap_palette::ColorMethod,
    note_color_inspector::{Draft, merge},
    note_color_preferences::Preferences,
};
use std::sync::Arc;
#[test]
fn field_selection_reuses_shared_arc_and_rgb_edits_clone_only_the_staged_value() {
    let base = Arc::new(Preferences::default());
    let mut draft = Draft::capture(Arc::clone(&base)).unwrap();
    assert!(Arc::ptr_eq(&draft.staged(), &base));
    draft.select(15).unwrap();
    assert!(Arc::ptr_eq(&draft.staged(), &base));
    draft.adjust(1, true, false).unwrap();
    let next = draft.staged();
    assert!(!Arc::ptr_eq(&next, &base));
    assert_eq!(
        base.colors.players[0][1],
        Preferences::default().colors.players[0][1]
    );
    assert_ne!(next.colors.players[0][1], base.colors.players[0][1]);
    assert_eq!(next.colors.subdivisions, base.colors.subdivisions);
    assert!(draft.changed());
    assert!(Arc::ptr_eq(&draft.baseline(), &base));
}
#[test]
fn all_fields_components_and_bounds_are_keyboard_editable_without_crossing_ui_xna_groups() {
    let mut p = Preferences::default();
    p.selection_multiplier = 9.9999;
    p.colors.subdivisions[0] = [1.0, 0.0, 0.5];
    let mut d = Draft::capture(Arc::new(p)).unwrap();
    d.adjust(0, true, false).unwrap();
    assert_eq!(d.staged().colors.subdivisions[0][0], 1.0);
    d.adjust(1, false, false).unwrap();
    assert_eq!(d.staged().colors.subdivisions[0][1], 0.0);
    d.select(25).unwrap();
    d.adjust(0, true, false).unwrap();
    assert_eq!(d.staged().selection_multiplier, 10.0);
    assert_eq!(d.staged().colors.selected_multiplier, 8.0);
    d.select(27).unwrap();
    d.adjust(0, true, false).unwrap();
    assert_eq!(d.staged().selection_multiplier, 10.0);
    assert_eq!(d.staged().held_multiplier, 1.5);
    d.select(31).unwrap();
    d.adjust(0, true, false).unwrap();
    assert!(d.staged().piu_row_based);
    assert!(d.adjust(1, true, false).is_err());
    d.select(33).unwrap();
    d.adjust(0, true, false).unwrap();
    assert_eq!(d.staged().step_color_method, ColorMethod::Note);
    assert!(d.select(35).is_err());
}
#[test]
fn exact_source_presets_own_twelve_base_colors_only_and_defaults_preserve_options() {
    let mut p = Preferences::default();
    p.colors.players[3] = [0.1, 0.2, 0.3];
    p.step_color_method = ColorMethod::Beat;
    let mut d = Draft::capture(Arc::new(p)).unwrap();
    d.select(34).unwrap();
    d.adjust(0, true, false).unwrap();
    assert_eq!(d.staged().colors.subdivisions[0], [0.910, 0.0, 0.0]);
    assert_eq!(d.staged().colors.hold, [0.498, 0.494, 0.498]);
    assert_eq!(d.staged().colors.players[3], [0.1, 0.2, 0.3]);
    assert_eq!(d.staged().step_color_method, ColorMethod::Beat);
    d.adjust(0, true, false).unwrap();
    assert_eq!(d.staged().colors.subdivisions[0], [0.922, 0.125, 0.004]);
    assert_eq!(d.staged().colors.mine, [0.925, 0.0, 0.0]);
    d.defaults().unwrap();
    assert_eq!(d.staged().step_color_method, ColorMethod::Beat);
    assert_eq!(d.staged().colors, Preferences::default().colors);
}
#[test]
fn owned_rgb_component_merge_preserves_external_components_and_other_groups() {
    let mut before = Preferences::default();
    before.colors.hold = [0.1, 0.2, 0.3];
    let mut after = before.clone();
    after.colors.hold[0] = 0.8;
    after.step_color_method = ColorMethod::Beat;
    let mut current = before.clone();
    current.colors.hold[1] = 0.9;
    current.colors.selected_multiplier = 4.0;
    current.colors.players[5] = [0.2, 0.3, 0.4];
    let merged = merge::owned(&current, &before, &after).unwrap();
    assert_eq!(merged.colors.hold, [0.8, 0.9, 0.3]);
    assert_eq!(merged.colors.selected_multiplier, 4.0);
    assert_eq!(merged.colors.players[5], [0.2, 0.3, 0.4]);
    assert_eq!(merged.step_color_method, ColorMethod::Beat);
}
#[test]
fn owned_leaf_conflict_is_atomic_and_noop_merge_preserves_current_preferences() {
    let before = Preferences::default();
    let mut after = before.clone();
    after.colors.players[2][2] = 0.8;
    after.held_multiplier = 3.0;
    let mut current = before.clone();
    current.colors.players[2][2] = 0.9;
    let snapshot = current.clone();
    assert!(matches!(
        merge::owned(&current, &before, &after),
        Err(deadsync_editor::Error::ExternalChange)
    ));
    assert_eq!(current, snapshot);
    assert_eq!(merge::owned(&current, &before, &before).unwrap(), current);
}
