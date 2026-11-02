use deadsync_editor::{
    audio_preferences::publication::EditBinding,
    document::{Document, Format},
    history::Session,
};
#[test]
fn deferred_preference_binding_is_not_restored_by_undo_or_redo() {
    let mut session =
        Session::new(Document::parse(b"#TITLE:Binding;".as_slice(), Format::Sm).unwrap());
    let token = EditBinding::capture(&session);
    assert!(token.is_current(&session));
    let before = deadsync_editor::dark_preferences::Preferences::default();
    let mut after = before;
    after.set_show(true);
    session.set_dark_preferences(before, after, None).unwrap();
    assert!(!token.is_current(&session));
    let second = EditBinding::capture(&session);
    session.undo().unwrap();
    assert!(!token.is_current(&session));
    assert!(!second.is_current(&session));
    session.redo().unwrap();
    assert!(!token.is_current(&session));
    assert!(!second.is_current(&session));
}
#[test]
fn exact_color_component_value_avoids_display_rounding_and_keeps_scalar_source_inputs() {
    use deadsync_editor::{note_color_inspector::Draft, note_color_preferences::Preferences};
    use std::sync::Arc;
    for value in [0.12345678_f32, 1e-7, 1.0, 0.0] {
        let mut draft = Draft::capture(Arc::new(Preferences::default())).unwrap();
        draft.set_component(0, value).unwrap();
        let exact = draft.component_value(0).unwrap();
        assert_eq!(exact.to_bits(), value.to_bits());
        let typed = exact.to_string().parse::<f32>().unwrap();
        assert!(!draft.set_component(0, typed).unwrap());
    }
    let mut draft = Draft::capture(Arc::new(Preferences::default())).unwrap();
    draft.select(25).unwrap();
    draft.set_component(0, 12.345678).unwrap();
    assert_eq!(draft.component_value(0), Some(12.345678));
    let before = draft.staged();
    assert!(draft.set_component(0, f32::INFINITY).is_err());
    assert!(draft.set_component(4, 1.0).is_err());
    assert_eq!(*draft.staged(), *before);
}
#[test]
fn source_manual_speed_keeps_raw_float_while_actual_manager_bounds_effective_value() {
    use deadsync_editor::{
        scroll_preferences::Preferences,
        zoom::{Manager, SpacingMode},
    };
    let mut p = Preferences::default();
    let mut zoom = Manager::default();
    for value in [-1.0_f32, 0.0, 321.123456, f32::MAX] {
        p.set_speed_input(0, value).unwrap();
        assert_eq!(p.speed(0), Some(f64::from(value)));
        zoom.set_spacing(SpacingMode::ConstantTime, p.speed(0).unwrap())
            .unwrap();
        assert_eq!(
            zoom.spacing(SpacingMode::ConstantTime),
            f64::from(value).clamp(10.0, 100000.0)
        );
    }
    let before = p;
    assert!(p.set_speed_input(0, f32::INFINITY).is_err());
    assert!(p.set_speed_input(3, 1.0).is_err());
    assert_eq!(p, before);
}
