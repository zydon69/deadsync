use deadsync_editor::generation::{
    config::{PatternConfig, PerformedConfig},
    metadata::Registry,
    profile_controls::{Control, Editor},
    profiles::Config,
};
use serde_json::Value;
fn leaves(v: &Value) -> usize {
    match v {
        Value::Object(m) => m.values().map(leaves).sum(),
        Value::Array(a) => a.iter().map(leaves).sum(),
        _ => 1,
    }
}
#[test]
fn every_serialized_field_of_every_preset_has_a_control_and_roundtrips() {
    let registry = Registry::builtin();
    let configs = registry
        .patterns
        .values()
        .cloned()
        .map(Config::Pattern)
        .chain(registry.performed.values().cloned().map(Config::Performed))
        .chain(registry.expressed.values().cloned().map(Config::Expressed));
    let mut count = 0;
    for config in configs {
        let editor = Editor::new(&config).unwrap();
        assert_eq!(editor.fields().len(), leaves(&config.json().unwrap()));
        assert_eq!(editor.finish().unwrap(), config);
        for (i, field) in editor.fields().iter().enumerate() {
            assert!(editor.value(i).is_some());
            if let Control::Choice(values) = field.control {
                let mut copy = Editor::new(&config).unwrap();
                let before = copy.value(i).unwrap().clone();
                for _ in 0..values.len() {
                    copy.step(i, true, false).unwrap();
                }
                assert_eq!(copy.value(i), Some(&before));
            }
        }
        count += 1;
    }
    assert!(count >= 10);
}
#[test]
fn pattern_choices_numbers_validation_and_repair_preserve_typed_configuration() {
    let mut editor = Editor::new(&Config::Pattern(PatternConfig::sixteenths())).unwrap();
    assert_eq!(editor.fields().len(), 16);
    let i = editor
        .fields()
        .iter()
        .position(|f| f.name() == "SameArrowStepWeight")
        .unwrap();
    let before = editor.value(i).cloned();
    assert!(editor.set_text(i, "2147483648").is_err());
    assert_eq!(editor.value(i), before.as_ref());
    editor.set_text(i, "-1").unwrap();
    assert!(editor.finish().is_err());
    editor.set_text(i, "42").unwrap();
    let Config::Pattern(config) = editor.finish().unwrap() else {
        panic!()
    };
    assert_eq!(config.same_arrow_step_weight, 42);
    assert!(editor.set_text(0, "8").is_err());
}
#[test]
fn inherited_boolean_cycles_and_nonfinite_numbers_are_not_silently_normalized() {
    let mut editor = Editor::new(&Config::Performed(PerformedConfig::default())).unwrap();
    let i = editor
        .fields()
        .iter()
        .position(|f| f.name() == "SpeedTighteningEnabled")
        .unwrap();
    assert_eq!(editor.value(i), Some(&Value::Null));
    editor.step(i, true, false).unwrap();
    assert_eq!(editor.value(i), Some(&Value::Bool(false)));
    editor.step(i, true, false).unwrap();
    assert_eq!(editor.value(i), Some(&Value::Bool(true)));
    editor.step(i, true, false).unwrap();
    assert_eq!(editor.value(i), Some(&Value::Null));
    let n = editor
        .fields()
        .iter()
        .position(|f| f.name() == "SpeedMinTimeSeconds")
        .unwrap();
    let before = editor.value(n).cloned();
    for invalid in ["NaN", "inf", "1e999", "bad"] {
        assert!(editor.set_text(n, invalid).is_err());
        assert_eq!(editor.value(n), before.as_ref());
    }
    editor.set_text(n, "0.125").unwrap();
    assert_eq!(editor.value(n).and_then(Value::as_f64), Some(0.125));
}
