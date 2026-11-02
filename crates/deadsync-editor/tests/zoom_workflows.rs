use deadsync_editor::zoom::{Manager, SpacingMode};
use serde_json::Value;
fn mode(value: &Value) -> SpacingMode {
    match value.as_str().unwrap() {
        "time" => SpacingMode::ConstantTime,
        "row" => SpacingMode::ConstantRow,
        "variable" => SpacingMode::Variable,
        _ => panic!("invalid fixture mode"),
    }
}
fn snapshot(manager: &Manager, expected: &Value, captured: Option<bool>, context: &str) {
    for (name, actual) in [
        ("SpacingZoom", manager.spacing_zoom()),
        ("SizeZoom", manager.size_zoom()),
        ("SizeCap", manager.size_cap()),
        ("Time", manager.spacing(SpacingMode::ConstantTime)),
        ("Row", manager.spacing(SpacingMode::ConstantRow)),
        ("Variable", manager.spacing(SpacingMode::Variable)),
    ] {
        let reference = expected[name].as_f64().unwrap();
        assert!(
            actual == reference || actual.to_bits().abs_diff(reference.to_bits()) <= 2,
            "{context} {name}: {actual:?} != original {reference:?}"
        );
    }
    assert_eq!(captured, expected["Captured"].as_bool(), "{context}");
}
#[test]
fn original_csharp_zoom_manager_traces_match_interpolation_bounds_and_spacing_notifications() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    for name in [
        "interpolation",
        "bounds",
        "spacing",
        "fractional-input",
        "pending-limit",
        "external-spacing",
        "duration-and-ulp",
        "float-epsilon",
        "clock-resolution",
    ] {
        let request: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("zoom-{name}.request.json"))).unwrap(),
        )
        .unwrap();
        let expected: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("zoom-{name}.expected.json"))).unwrap(),
        )
        .unwrap();
        let mut manager = Manager::default();
        snapshot(&manager, &expected["Initial"], None, name);
        for (index, op) in request["Operations"].as_array().unwrap().iter().enumerate() {
            let captured = match op[0].as_str().unwrap() {
                "zoom" => {
                    manager.set_zoom(op[1].as_f64().unwrap()).unwrap();
                    None
                }
                "cap" => {
                    manager.set_size_cap(op[1].as_f64().unwrap()).unwrap();
                    None
                }
                "limit" => {
                    manager.set_limit_to_size(op[1].as_bool().unwrap()).unwrap();
                    None
                }
                "duration" => {
                    manager
                        .set_interpolation_duration(op[1].as_f64().unwrap())
                        .unwrap();
                    None
                }
                "multiplier" => {
                    manager
                        .set_zoom_multiplier(op[1].as_f64().unwrap())
                        .unwrap();
                    None
                }
                "spacing" => {
                    manager
                        .set_spacing(mode(&op[1]), op[2].as_f64().unwrap())
                        .unwrap();
                    None
                }
                "update" => {
                    manager.update(op[1].as_f64().unwrap()).unwrap();
                    None
                }
                "scroll" => Some(
                    manager
                        .process_input(
                            op[1].as_f64().unwrap(),
                            op[2].as_f64().unwrap() as f32,
                            op[3].as_bool().unwrap(),
                            op[4].as_bool().unwrap(),
                            mode(&op[5]),
                        )
                        .unwrap(),
                ),
                _ => panic!("unknown fixture operation"),
            };
            snapshot(
                &manager,
                &expected["Stages"][index],
                captured,
                &format!("{name} operation {index}: {op}"),
            );
        }
    }
}
#[test]
fn invalid_zoom_preferences_clock_and_input_are_rejected_atomically() {
    let mut manager = Manager::default();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let before = manager.clone();
        assert!(manager.set_zoom(value).is_err());
        assert!(manager.set_size_cap(value).is_err());
        assert!(
            manager
                .set_spacing(SpacingMode::ConstantRow, value)
                .is_err()
        );
        assert!(manager.set_interpolation_duration(value).is_err());
        assert!(manager.set_zoom_multiplier(value).is_err());
        assert!(manager.update(value).is_err());
        assert!(
            manager
                .process_input(value, 1.0, true, false, SpacingMode::ConstantTime)
                .is_err()
        );
        assert_eq!(manager, before);
    }
    for delta in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let before = manager.clone();
        assert!(
            manager
                .process_input(1.0, delta, true, false, SpacingMode::ConstantTime)
                .is_err()
        );
        assert_eq!(manager, before);
    }
    for (duration, multiplier) in [(-0.1, 0.9), (1.1, 4.1)] {
        let before = manager.clone();
        assert!(manager.set_interpolation_duration(duration).is_err());
        assert!(manager.set_zoom_multiplier(multiplier).is_err());
        assert_eq!(manager, before);
    }
}
#[test]
fn spacing_and_size_zoom_are_independent_and_uncaptured_input_does_not_change_state() {
    let mut manager = Manager::default();
    let initial = manager.clone();
    assert!(
        !manager
            .process_input(0.0, 1.0, false, false, SpacingMode::ConstantTime)
            .unwrap()
    );
    assert!(
        !manager
            .process_input(0.0, 1e-8, true, true, SpacingMode::ConstantTime)
            .unwrap()
    );
    assert_eq!(manager, initial);
    manager.set_zoom(2000.0).unwrap();
    assert_eq!(manager.spacing_zoom(), 2000.0);
    assert_eq!(manager.size_zoom(), 1.0);
    manager.set_size_cap(0.25).unwrap();
    assert_eq!(manager.spacing_zoom(), 2000.0);
    assert_eq!(manager.size_zoom(), 0.25);
    manager.set_limit_to_size(true).unwrap();
    assert_eq!(manager.spacing_zoom(), 0.25);
    manager.set_zoom(-1.0).unwrap();
    assert_eq!(manager.spacing_zoom(), 0.000001);
}
