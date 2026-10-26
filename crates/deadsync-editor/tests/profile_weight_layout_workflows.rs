use deadsync_editor::generation::{
    config::PerformedConfig, job::TARGET_LAYOUTS, profile_controls::Editor, profiles::Config,
};
#[test]
fn every_layout_can_be_added_without_overwriting_existing_weights_or_other_values() {
    let mut config = PerformedConfig::balanced();
    config.arrow_weights.clear();
    config.facing.max_inward_percentage = 0.25;
    let mut editor = Editor::new(&Config::Performed(config)).unwrap();
    let baseline = editor.fields().len();
    for layout in TARGET_LAYOUTS {
        assert!(!editor.has_arrow_weights(layout).unwrap());
        assert!(editor.set_arrow_weights_presence(layout, true).unwrap());
        assert!(!editor.set_arrow_weights_presence(layout, true).unwrap());
        let Config::Performed(current) = editor.finish().unwrap() else {
            panic!()
        };
        assert_eq!(
            current.arrow_weights[layout],
            PerformedConfig::balanced().arrow_weights[layout]
        );
        assert_eq!(current.facing.max_inward_percentage, 0.25);
    }
    assert!(editor.fields().len() > baseline);
    for layout in TARGET_LAYOUTS {
        assert!(editor.set_arrow_weights_presence(layout, false).unwrap());
        assert!(!editor.set_arrow_weights_presence(layout, false).unwrap());
    }
    assert_eq!(editor.fields().len(), baseline);
}
#[test]
fn removal_inherits_preset_values_and_add_is_a_noop_for_custom_explicit_values() {
    let mut config = PerformedConfig::balanced();
    config
        .arrow_weights
        .insert("dance-single".into(), vec![1, 2, 3, 4]);
    let mut editor = Editor::new(&Config::Performed(config)).unwrap();
    assert!(
        !editor
            .set_arrow_weights_presence("dance-single", true)
            .unwrap()
    );
    let Config::Performed(current) = editor.finish().unwrap() else {
        panic!()
    };
    assert_eq!(current.arrow_weights["dance-single"], vec![1, 2, 3, 4]);
    editor
        .set_arrow_weights_presence("dance-single", false)
        .unwrap();
    let Config::Performed(mut current) = editor.finish().unwrap() else {
        panic!()
    };
    assert!(!current.arrow_weights.contains_key("dance-single"));
    let mut base = PerformedConfig::balanced();
    base.arrow_weights
        .insert("dance-single".into(), vec![4, 3, 2, 1]);
    current.override_of(&base);
    assert_eq!(current.arrow_weights["dance-single"], vec![4, 3, 2, 1]);
}
#[test]
fn descriptor_rebuild_keeps_invalid_staged_ranges_and_invalid_requests_are_atomic() {
    let mut editor = Editor::new(&Config::Performed(PerformedConfig::balanced())).unwrap();
    let index = editor
        .fields()
        .iter()
        .position(|f| f.name() == "MaxInwardPercentage")
        .unwrap();
    editor.set_text(index, "2").unwrap();
    assert!(editor.finish().is_err());
    editor
        .set_arrow_weights_presence("dance-double", false)
        .unwrap();
    editor
        .set_arrow_weights_presence("dance-double", true)
        .unwrap();
    assert_eq!(editor.value(index).unwrap().as_f64(), Some(2.0));
    assert!(editor.finish().is_err());
    let count = editor.fields().len();
    assert!(editor.set_arrow_weights_presence("unknown", true).is_err());
    assert_eq!(editor.fields().len(), count);
    assert_eq!(editor.value(index).unwrap().as_f64(), Some(2.0));
    let mut other = Editor::new(&Config::Pattern(Default::default())).unwrap();
    assert!(
        other
            .set_arrow_weights_presence("dance-single", true)
            .is_err()
    );
}

#[test]
fn edited_panel_weights_match_original_widget_bounds_without_normalizing_imports_on_open() {
    let mut config = PerformedConfig::balanced();
    config.arrow_weights.get_mut("dance-single").unwrap()[0] = 150;
    let mut editor = Editor::new(&Config::Performed(config)).unwrap();
    let index = editor
        .fields()
        .iter()
        .position(|f| f.path() == ["ArrowWeights", "dance-single", "0"])
        .unwrap();
    assert_eq!(editor.value(index).unwrap().as_i64(), Some(150));
    assert!(
        !editor
            .set_arrow_weights_presence("dance-single", true)
            .unwrap()
    );
    assert_eq!(editor.value(index).unwrap().as_i64(), Some(150));
    editor.set_text(index, "-5").unwrap();
    assert_eq!(editor.value(index).unwrap().as_i64(), Some(0));
    editor.step(index, false, false).unwrap();
    assert_eq!(editor.value(index).unwrap().as_i64(), Some(0));
    editor.set_text(index, "105").unwrap();
    assert_eq!(editor.value(index).unwrap().as_i64(), Some(100));
    editor.step(index, true, true).unwrap();
    assert_eq!(editor.value(index).unwrap().as_i64(), Some(100));
}
