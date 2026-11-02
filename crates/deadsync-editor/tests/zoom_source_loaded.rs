use deadsync_editor::{
    audio_preferences::File,
    zoom::{Manager, SpacingMode},
    zoom_preferences::Preferences,
};
use serde_json::{Map, Value};
fn prefs(fields: &Map<String, Value>) -> Preferences {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({"Version":1,"Zoom":fields})).unwrap(),
    )
    .unwrap();
    File::open(&path).unwrap().zoom()
}
fn assert_stage(manager: &Manager, expected: &Value, name: &str) {
    for (key, actual) in [
        ("zoom", manager.spacing_zoom()),
        ("size", manager.size_zoom()),
        ("cap", manager.size_cap()),
        ("time", manager.spacing(SpacingMode::ConstantTime)),
        ("row", manager.spacing(SpacingMode::ConstantRow)),
        ("variable", manager.spacing(SpacingMode::Variable)),
        ("duration", manager.interpolation_duration()),
        ("multiplier", manager.zoom_multiplier()),
    ] {
        let expected = expected[key].as_f64().unwrap();
        assert!(
            (actual - expected).abs() <= 1e-10_f64.max(expected.abs() * 1e-10),
            "{name} {key}: {actual} != {expected}"
        );
    }
    assert_eq!(
        manager.limit_to_size(),
        expected["limited"].as_bool().unwrap(),
        "{name}"
    );
}
#[test]
fn loaded_source_zoom_preferences_match_original_constructor_notifications_and_fumen_lerp() {
    let data: Value =
        serde_json::from_str(include_str!("fixtures/zoom_source_loaded.expected.json")).unwrap();
    for case in data.as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let source: Value = serde_json::from_str(case["raw"].as_str().unwrap()).unwrap();
        let mut fields = source["PreferencesScroll"].as_object().unwrap().clone();
        let settings = prefs(&fields);
        let mut manager = Manager::default();
        if case["error"].is_string() {
            let before = manager.clone();
            assert!(settings.initialize(&mut manager).is_err(), "{name}");
            assert_eq!(manager, before);
            continue;
        }
        settings.initialize(&mut manager).unwrap();
        let stages = case["stages"].as_array().unwrap();
        assert_stage(&manager, &stages[0], name);
        for (index, op) in case["ops"].as_array().unwrap().iter().enumerate() {
            match op[0].as_str().unwrap() {
                "duration" => {
                    fields.insert("ScrollInterpolationDuration".into(), op[1].clone());
                    prefs(&fields).apply(&mut manager).unwrap();
                }
                "multiplier" => {
                    fields.insert("ZoomMultiplier".into(), op[1].clone());
                    prefs(&fields).apply(&mut manager).unwrap();
                }
                "cap" => {
                    fields.insert("SizeCap".into(), op[1].clone());
                    prefs(&fields).apply(&mut manager).unwrap();
                }
                "limit" => {
                    fields.insert("LimitZoomToSize".into(), op[1].clone());
                    prefs(&fields).apply(&mut manager).unwrap();
                }
                "zoom" => manager.set_zoom(op[1].as_f64().unwrap()).unwrap(),
                "update" => manager.update(op[1].as_f64().unwrap()).unwrap(),
                "scroll" => {
                    manager
                        .process_input(
                            op[1].as_f64().unwrap(),
                            op[2].as_f64().unwrap() as f32,
                            op[3].as_bool().unwrap(),
                            op[4].as_bool().unwrap(),
                            SpacingMode::ConstantTime,
                        )
                        .unwrap();
                }
                _ => unreachable!(),
            }
            assert_stage(
                &manager,
                &stages[index + 1],
                &format!("{name} operation{index}"),
            );
        }
    }
}
#[test]
fn duration_only_install_keeps_constructor_cap_and_invalid_source_bounds_are_atomic() {
    let mut manager = Manager::default();
    manager.initialize_preferences(2.0, 0.3, 4.0, true).unwrap();
    manager.set_zoom(3.0).unwrap();
    manager.apply_preferences(-2.0, -1.0, 4.0, true).unwrap();
    assert_eq!(manager.size_cap(), 4.0);
    assert_eq!(manager.spacing_zoom(), 3.0);
    manager.apply_preferences(-2.0, -1.0, 2.0, true).unwrap();
    assert_eq!(manager.size_cap(), 1.0);
    assert_eq!(manager.spacing_zoom(), 1.0);
    let mut manager = Manager::default();
    manager
        .initialize_preferences(-2.0, 0.0, -2.0, false)
        .unwrap();
    let before = manager.clone();
    assert!(manager.apply_preferences(-2.0, 0.0, -2.0, true).is_err());
    assert_eq!(manager, before);
}
