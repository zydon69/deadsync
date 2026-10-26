use deadsync_editor::{Error, minimap_palette};
#[path = "../src/note_color_preferences.rs"]
mod note_color_preferences;
use note_color_preferences::Preferences;
#[test]
fn source_keys_and_vector_objects_preserve_distinct_groups_and_unknown_values() {
    let mut note=serde_json::json!({"QuarterColor":{"X":0.25,"Y":0.5,"Z":0.75},"Player1Color":{"X":0.9,"Y":0.2,"Z":0.1},"ArrowUIColorMultiplier":1.25,"ArrowUISelectedColorMultiplier":7.0,"SelectionColorMultiplier":1.8,"HeldColorMultiplier":1.6,"UseRowBasedColoringForPiu":true,"UnknownNote":99}).as_object().unwrap().clone();
    let mut options = serde_json::json!({"StepColorMethodValue":"Beat","UnknownOptions":42})
        .as_object()
        .unwrap()
        .clone();
    let settings = Preferences::read(&note, &options).unwrap();
    assert_eq!(settings.colors.subdivisions[0], [0.25, 0.5, 0.75]);
    assert_eq!(settings.colors.players[1], [0.9, 0.2, 0.1]);
    assert_eq!(settings.colors.normal_multiplier, 1.25);
    assert_eq!(settings.colors.selected_multiplier, 7.0);
    assert_eq!(settings.selection_multiplier, 1.8);
    assert_eq!(settings.held_multiplier, 1.6);
    assert!(settings.piu_row_based);
    assert_eq!(
        settings.step_color_method,
        minimap_palette::ColorMethod::Beat
    );
    settings.write(&mut note, &mut options).unwrap();
    assert_eq!(note["UnknownNote"], 99);
    assert_eq!(options["UnknownOptions"], 42);
    assert!(!note.contains_key("StepColorMethodValue"));
    assert!(!options.contains_key("QuarterColor"));
    let reopened = Preferences::read(&note, &options).unwrap();
    assert_eq!(reopened.colors.players, settings.colors.players);
    assert_eq!(reopened.colors.subdivisions, settings.colors.subdivisions);
}
#[test]
fn invalid_rgb_types_bounds_and_nonfinite_multipliers_are_rejected() {
    for invalid in [
        serde_json::json!({"QuarterColor":[0.0,1.0,0.0]}),
        serde_json::json!({"QuarterColor":{"X":2.0,"Y":0.0,"Z":0.0}}),
        serde_json::json!({"ArrowUIColorMultiplier":"bad"}),
    ] {
        assert!(Preferences::read(invalid.as_object().unwrap(), &Default::default()).is_err());
    }
    let mut settings = Preferences::default();
    settings.held_multiplier = f32::INFINITY;
    assert!(settings.validate().is_err());
    assert_eq!(
        Preferences::read(
            &Default::default(),
            serde_json::json!({"StepColorMethodValue":"Future"})
                .as_object()
                .unwrap()
        )
        .unwrap()
        .step_color_method,
        minimap_palette::ColorMethod::Stepmania
    );
}
