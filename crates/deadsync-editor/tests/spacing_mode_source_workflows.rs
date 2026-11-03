use deadsync_editor::{
    audio_preferences::File, scroll_preferences::spacing::Preference, zoom::SpacingMode,
};
use serde_json::Value;
#[test]
fn all_original_converter_factory_toggle_cases_match_the_executed_source() {
    let cases: Value =
        serde_json::from_str(include_str!("fixtures/spacing-mode.expected.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let value = Preference::read_source(Some(&case["Input"]));
        if case["Error"] == true {
            assert!(value.is_err());
            continue;
        }
        let value = value.unwrap();
        assert_eq!(value.raw(), case["Raw"].as_i64().unwrap() as i32);
        assert_eq!(value.source_name(), case["Serialized"].as_str().unwrap());
        assert_eq!(value.next().raw(), case["Toggled"].as_i64().unwrap() as i32);
        let helper = match value.effective() {
            SpacingMode::ConstantTime => "EventSpacingHelperConstantTime",
            SpacingMode::ConstantRow => "EventSpacingHelperConstantRow",
            SpacingMode::Variable => "EventSpacingHelperVariable",
        };
        assert_eq!(helper, case["Helper"].as_str().unwrap());
    }
}
#[test]
fn raw_unknown_survives_native_file_and_unrelated_preference_publication() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preferences.json");
    let bytes =
        br#"{"Version":1,"Scroll":{"SpacingMode":"99","Future":7},"Options":{"Future":17}}"#;
    std::fs::write(&path, bytes).unwrap();
    let mut file = File::open(&path).unwrap();
    assert_eq!(file.spacing_preference(), Preference::Unknown(99));
    assert_eq!(file.spacing_mode(), SpacingMode::ConstantTime);
    assert!(std::fs::read(&path).unwrap() == bytes);
    let mut general = file.general_options().clone();
    general.open_last = false;
    file.set_general_options(general).unwrap();
    file.save().unwrap();
    let stored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored["Scroll"]["SpacingMode"], "99");
    assert_eq!(stored["Scroll"]["Future"], 7);
    file.set_spacing_preference(file.spacing_preference().next());
    file.save().unwrap();
    let reread = File::open(&path).unwrap();
    assert_eq!(reread.spacing_preference(), Preference::ConstantRow);
    assert_eq!(reread.spacing_mode(), SpacingMode::ConstantRow);
    assert_eq!(
        Preference::read_native(Some(&serde_json::json!(1))).unwrap(),
        Preference::ConstantRow
    );
    assert_eq!(
        Preference::read_source(Some(&serde_json::json!(1))).unwrap(),
        Preference::ConstantTime
    );
}
